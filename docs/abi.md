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
- 创建/保留/释放规则、重复释放、失效检查、异步任务与窗口销毁关系必须在实现前写死。
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
- ABI 与 `.jaso` 模块/宿主能力的对齐。

实现前先冻结本草案，并建立 ABI 一致性测试与模糊测试。
