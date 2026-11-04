# Jasper ↔ Rust Native ABI 草案

> 状态：**设计草案，未实现**。当前 crate 是 Rust API，不是稳定 ABI。
> 目标：为 Jasper Runtime 与 Rust 引擎建立可版本化、可批量、可错误处理的边界。

## 1. 原则

1. 固定宽度整数、明确布局的结构；不传 Rust `String`/`Vec`/trait 对象。
2. 不暴露 Rust 私有对象；不传 Jasper GC 可移动对象的裸指针。
3. Panic 不得穿越 FFI；边界统一 catch 并转错误码。
4. 错误可识别、可分类、可映射回 Jasper 组件。
5. 独立 ABI 版本；能力查询 + 兼容策略。
6. 批量操作优先，减少边界切换。
7. 句柄带代际，失效/重复释放可检测。

## 2. 接口族（建议）

| 族 | 操作 |
|---|---|
| Engine | `engine_create` / `engine_destroy` / `engine_query_capabilities` |
| Window | `window_create` / `window_resize` / `window_close` |
| Scene | `scene_create` / `scene_apply_batch` |
| Layout | `layout_compute` / `layout_query` |
| Text | `text_measure` / `text_hit_test` |
| Resource | `resource_create` / `resource_release` |
| Frame | `frame_submit` / `frame_present` |
| Event | `event_poll_batch` |
| Accessibility | `semantics_apply_batch` |
| Diagnostics | `diagnostics_snapshot` |

具体签名在实现阶段冻结；接口按职责设计，不机械映射每个 Rust 方法。

## 3. 句柄与所有权

- Jasper 侧：`Handle { index: u32, generation: u32 }` 一类不透明句柄。
- Rust 侧：句柄表 + 代际；`index` 复用但 `generation` 变化，旧句柄失效。
- 已实现（未导出 FFI）：`src/handle.rs` 的 `Handle`（`repr(C)`，两个 u32，8 字节）、`HandleAllocator` 与 `HandleTable<T>`。槽位代数只增不减：奇数存活、偶数空闲；耗尽值 `HANDLE_GENERATION_RETIRED` 永久退役，不回绕；全零句柄永远无效；过期、外来与重复释放一律拒绝且不改变状态；容量是宿主显式上限，表满或无法增长时把值交还调用方，分配失败不中止进程，释放从不分配；`status` 把句柄分为存活、过期与无效，对应第 6 节的 StaleHandle 与 InvalidHandle。重启的宿主可用 `with_first_generation` 越过上次运行签发的代数。规格见 `verification.md` 的 UI-HANDLE-001。
- 仍未决定：保留/引用计数规则、异步任务与窗口销毁关系、跨进程句柄空间；这些必须在 FFI 冻结前写死。
- Jasper GC 对象：不暴露裸地址；通过受控缓冲区传递；确需零拷贝时单独设计 Pin/生命周期协议。
- Rust 资源（GPU 纹理/字体缓存/窗口）由 Rust 管理；Jasper 只持句柄。

## 4. 场景更新

- 批量 `scene_apply_batch`：插入/更新/删除/样式/文本/资源引用/无障碍语义。
- 每批带 `scene_version`；引擎拒绝过期/未来版本（或按策略合并）。
- 节点 ID 稳定；引擎返回布局/命中/事件时携带节点 ID + 版本。
- 不把 Taffy/Parley/Vello 类型放进 ABI。

## 5. 线程与调度

- 平台窗口事件循环线程有约束；Jasper 逻辑线程与 Rust 渲染线程通过队列/同步协议。
- 推荐流程：Rust 收平台事件 → 规范化队列 → Jasper 消费 → 更新 UI IR → 批量提交 → Rust 布局/绘制 → 提交帧。
- 不在任意 Rust 工作线程直接调用非线程安全的 Jasper 回调。
- UI 更新支持合并、取消过期任务、版本检查。

## 6. 错误模型

- 固定宽度错误码 + 可选结构化详情（批量缓冲）。
- 至少区分：InvalidArgument、InvalidHandle、StaleHandle、Unsupported、ResourceExhausted、DeviceLost、Io、Internal。
- 错误携带操作/节点/版本（能定位回 Jasper 组件）。
- 不把 Rust panic 文本直接暴露给应用；转 Internal + 诊断日志。

## 7. 版本与能力

- `abi_version`（结构/调用协议）+ `capability_bits`（GPU/文字/无障碍/图片等）。
- 能力查询必须反映真实可用能力，不虚报。
- 新增字段走扩展结构/版本；破坏性变更升 major。
- 旧 Jasper Runtime 遇到不支持能力时明确失败/降级，不静默。

## 8. 输入/事件

- `event_poll_batch` 返回结构化事件：指针/触摸/笔/键/文本/组合/滚动/焦点/无障碍动作。
- 事件带节点 ID、时间戳、版本、pointer_id、压力/倾斜等（触控笔不退化）。
- 队列有容量/背压/丢弃标记；UI 卡顿时不能无限堆积。

## 9. 未决问题

- 线格式：C ABI 结构体 vs 版本化二进制批。
- 句柄表归属：引擎全局 vs 每窗口。
- 场景版本冲突策略：拒绝 vs 合并。
- 零拷贝的适用场景与安全边界。
- 无障碍批的跨线程顺序。
- ABI 与 `.jaso` 模块/宿主能力的对齐：本节之外的 `ui.*` 已按第 10 节单独定义，是否提升为公共 ABI 尚未决定。

实现前先冻结本草案，并建立 ABI 一致性测试与模糊测试。

## 10. UI 宿主命名空间 `ui.*`（版本化，已实现第 1 版）

**`ui.*` 是 UI 宿主命名空间，独立版本化，不属于上文 Native ABI v1 核心。** 证明清单（`claude-closeout-proof-backlog.md` 的 CP 系列）闭合后，才决定是否提升为公共 ABI；在此之前第 1–9 节仍是草案，本节不代表它们已冻结。本节不改变 `.jaso` 二进制格式与装载器语义（格式与错误码见 jasper-os 仓库 `jaso/SPEC.md`，format 1），`ui.*` 只是该规范第 11 节允许的命名空间键，由本引擎解释。

实现：`src/ui_plugin_gate.rs`（纯准入规则，始终编译）与 `src/ui_plugin_host.rs`（可选特性 `ui-plugin-pending`，复用 jasper-os 的 `jaso` crate，不复制解析器）。

### 10.1 清单键

| 键 | 取值 | 含义 |
|---|---|---|
| `ui.abi` | `1` | 本命名空间版本；语义变化必须升版本，未知版本拒绝 |
| `ui.kind` | `paint-node` | 插件种类：为一个场景节点产出绘制命令；其他种类拒绝 |
| `ui.paint` | 导出名 | 绘制入口；必须在 `core.exports` 中，不得是 `jaso_init` / `jaso_start` / `jaso_stop` |

能力名：`core.requires` 必须请求 `ui.paint`。这是请求，由调用方授予；未授予报 `capability.denied`，未请求报清单错误。v1 不解释的其他 `ui.*` 键被忽略。

### 10.2 平台与种类

Native、`core.stage = runtime`，平台记录 `target = host`、`abi = c.v1`、`float_abi = hard`。目前只有 unix x86_64 宿主能映射代码，其他宿主明确报 `UnsupportedHost`（其他指令集需要指令缓存维护，尚无测试）。Managed、Hybrid 与 build/boot 文件报 `UnsupportedModule`。

### 10.3 入口

```c
int32_t paint(const UiPaintRequestV1 *request, UiPaintCommandV1 *out, uint32_t capacity);

struct UiPaintRequestV1 { uint32_t version /* = 1 */, size /* = 16 */, node_width, node_height; };
struct UiPaintCommandV1 { uint32_t kind /* 1 = 填充矩形 */, x, y, width, height;
                          uint8_t rgba[4]; uint32_t reserved[2] /* = 0 */; };   /* 32 字节 */
```

- 坐标与节点尺寸都是节点局部设备像素；`capacity` 当前为 64。
- 返回值是写入的命令数，负值或大于 `capacity` 表示失败。
- 命令必须非空且完全落在节点内，**越界拒绝，不裁剪**。任何一条违规，整份输出被拒绝，不部分绘制。
- 模块经 `jaso.host.v1` 表（`log` / `alloc` / `dealloc`）使用宿主能力，不按名字导入符号。

### 10.4 装载、生命周期与绘制

- L1 按 `.jaso` 规范第 12.2 节执行：结构 → 信任 → 平台 → 运行时 → 依赖 → 能力 → 绑定，本引擎在此之后检查 `ui.*`，然后映射、重定位、设置 W^X 权限、`jaso_init`、`jaso_start`。失败不留映射，也不进模块表。
- `stop` 已实现；已停止的插件不再被调用，名字不复用。卸载请求答复「推迟」（`plugin`）或「拒绝」（`system`），模块保持映射。`UiPluginHost` 析构是宿主收尾，不是卸载请求。
- `SceneHostView::render_cpu_with_paint_layers` 把已校验命令画进同一 anyrender 场景，**位于整个场景之上**，不是树内 z 序。它不是视图帧：不读写帧缓存，不改变损伤基准和脏标记。GPU 路径尚未接通。

### 10.5 信任边界

插件在宿主进程内运行，**没有隔离**：信任只来自调用方给出的签名策略，能力是装载时比对的声明，不是沙箱。恶意或有缺陷的已签名插件可以破坏整个进程。入口崩溃、不返回或写坏内存都不在本节保证之内；命令校验只约束它经入口交回的内容。
