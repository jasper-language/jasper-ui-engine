# 能力状态（v0.1 原型；2026-10-23 核查）

> Current direction: migrate the UI execution layer to pinned Blitz. The existing
> hand-assembled engine and optional Parley adapter are retained for migration
> comparison; they are not the target architecture. Initial integration lives in
> shared native source, exposed by root `engine::Engine` through optional
> `native-blitz-pending` / `native-blitz-cpu-pending` features. The root entry is not
> yet accepted. Native execution requires an explicit feature; old execution is LegacyEngine only. Canvas remains deferred.

本表以源码为准。**有实现/有测试不等于跨平台验收或数学形式化证明**。没有实现级证明产物的模块统一记为 `not_verified`；证明门槛见 [verification.md](verification.md)。

## 已有实现与具体边界

| 能力 | 源码证据 | 当前边界 |
|---|---|---|
| 场景树、补丁、版本 | `scene.rs` | clone 后应用、成功后替换；版本准入内核已证明，整个事务仍未证明；插入全部后代 ID 检查；无多写合并 |
| 几何、颜色、PSS | `geometry.rs`、`style.rs` | 尺寸/百分比/min-max/边距/间距/对齐/边框等子集，不是完整 CSS |
| Taffy 布局 | `layout.rs`、`tests/layout_consistency.rs` | 公开 Display 仅 Block/Flex/None；**Grid 未接入公开样式与映射** |
| 近似文字测量 | `text.rs` | 默认 SimpleTextEngine，不具备真实字体度量 |
| 可选真实文字测量 | `text_cosmic.rs`、`real-text` | cosmic-text shaping/断行/字体回退；FontSystem 使用系统字体；当前只输出 TextMetrics，未输出共享字形布局对象 |
| 显示列表、二进制绘制批 | `paint.rs`、`batch.rs` | Fill/Stroke/Line/Text/Clip；有版本/边界及往返测试；读取端点、字节读取原语及 Cursor::take 已有范围明确的形式化证明；不是 UI 树语义协议 |
| 软件光栅、Headless | `raster.rs`、`backend.rs` | RGBA、矩形/线、裁剪、混合、PPM；DrawText 明确 Unsupported；无圆角/渐变/阴影 |
| Canvas | `canvas.rs`、`canvas_tiny_skia.rs` | 记录矩形/线/文字/裁剪、局部命中；tiny-skia 可选，默认开启；文字仍未光栅化 |
| 指针与交互 | `input.rs`、`pointer.rs`、`hit_test.rs`、`gesture.rs` | 多指/笔模型、命中、capture、hover、参考手势与手掌拒绝；不等于真实平台笔支持已验收 |
| 语义与焦点 | `semantics.rs`、`keyboard.rs` | 宿主中立语义树、Tab 导航/激活意图、文本/组合事件焦点路由；无系统读屏桥 |
| 滚动与滚动条 | `scroll.rs`、`scrollbar.rs` | 逐级消费/残余 delta、clamp/fling、几何/拖动/绘制；无回弹/自动隐藏/完整平台主题 |
| 桌面输入映射 | `desktop.rs`、`desktop` | winit 鼠标/键盘/滚轮/IME/焦点规范化；映射与路由不等于完整编辑器 |
| 桌面窗口/软件呈现 | `examples/window.rs` | winit + softbuffer 原生 CPU 窗口；指针、滚轮、命令键、IME 显示与焦点恢复已接公共入口，点击触发版本化视觉更新；真实外部服务/系统无障碍/合成器仍未验收 |
| GPU 选择与枚举 | `graphics.rs`、`gpu_wgpu.rs`、`gpu` | 策略、回退说明与真实适配器枚举；旧引擎无 GPU 绘制管线或 device-lost 恢复 |
| `.jaso` UI 插件宿主 | `ui_plugin_gate.rs`、`ui_plugin_host.rs`、`ui-plugin-pending`、`tests/ui_plugin_acceptance.rs` | `ui.*` 第 1 版（UI 宿主命名空间，不属于 Native ABI v1 核心）：真实编译的原生插件经 L1 装入并画进一帧 CPU 无头画面，错误平台/缺能力/篡改等被拒；仅 unix x86_64，进程内无隔离，层在整个场景之上，GPU 路径未接通（见 `abi.md` 第 10 节） |
| 原生 GPU 无头渲染 | `native_adapter/gpu_render.rs`、`gpu-pending` | Vello hybrid 于 wgpu 离屏渲染并读回，仅在软件 Vulkan（llvmpipe）实测；无窗口呈现、device-lost 恢复、GPU 损伤或真实硬件验收（见 UI10 节） |
| 流水线 | `engine.rs` | 每帧完整布局/绘制/编码/提交/呈现，无完整增量失效与调度 |
| 最小宿主接口 | `platform.rs` | 尺寸/缩放/重绘/光标；不等于完整 UiHost 或已可用的系统内置服务 |

## 未完成与外部协作

- 场景文字的完整文本编辑与 IME 往返、选区高亮绘制与预编辑几何（光标/命中几何见 UI8 节、选区几何见 UI9 节）；双向文字、字重/斜体与字体族选择未验收。原生路径的文字光栅、多字体按序回退与折行已有无头实测（见 2026-10-27 UI3 节）。
- 图片/SVG 解码、资源额度、圆角/阴影/变换；区域重绘（只重绘损伤区域）、真实垂直同步来源、动画/过渡驱动与 GPU 路径的帧调度。原生 CPU 路径的整帧按需调度、上一帧复用（UI4）、相对上一帧的精确损伤矩形报告（UI5）、按损伤只写变化像素的宿主 Surface 写入（UI6）与按宿主刷新时钟的帧节拍决定（UI7）已有无头实测。
- GPU 窗口/表面呈现、device-lost 恢复、GPU 损伤与局部重绘、真实硬件 GPU 验收（原生无头 GPU 渲染见 UI10 节）；系统无障碍桥（平台适配器/系统无障碍服务接入、文本无障碍接口；宿主无关的无障碍树快照见 UI11 节、快照间增量更新与回放见 UI12 节、AccessKit TreeUpdate 转换与动作路由见 UI13 节）、多窗口/DPI/触控/笔平台覆盖。
- 版本化语义更新协议、句柄所有权规则（代际句柄表见 2026-11-03 UI35 节，未导出 FFI）、Native ABI、完整宿主能力协商。
- 系统内置 UI 服务适配、系统 Surface/输入/字体资源接入、多应用隔离与重连恢复。系统桌面是一等目标，软件路径不能依赖自带窗口栈。
- Jasper 组件/状态/主题规则/导航、Element/Reconciler/UI IR，以及 Web DOM/SSR 等由对应语义层/后端提供，本仓库不复制第二套实现。
- 数学形式化证明：版本计算和解码读取端点函数已产生机器复核产物（生产函数体、错误实现反例被拒）；整个事务及其他模块仍未完成证明。不得将单项结论或测试扩大为整个 UI 已证明。

## 宿主能力矩阵

三态表示实现方式；缺实现使用 unsupported，缺验收另列，不把待开发功能写成“已模拟”。

| 能力 | Headless/软件 | 桌面自带示例 | 系统内置目标 | Web DOM |
|---|---|---|---|---|
| 矩形/线/裁剪 | native，已有测试 | native，软件呈现；本轮未实测窗口 | unsupported，待适配 | 外部后端，不由本引擎声明 |
| 文字测量 | 近似 native；真实可选 | 同左，未完成文字显示 | unsupported，缺宿主字体源 | 外部后端 |
| 文字显示/编辑 | unsupported | unsupported | unsupported | 外部后端 |
| 平台输入 | unsupported，测试可注入事件 | native 映射；完整交互未接 | unsupported | 外部后端 |
| 系统无障碍 | unsupported | unsupported | unsupported | 外部后端 |
| GPU 绘制 | 原生路径无头离屏（`gpu-pending`），仅软件 Vulkan 实测 | unsupported，仅枚举 | unsupported | 外部后端 |

渲染宿主的具体声明与生命周期契约见 [host-rendering.md](host-rendering.md)。`HostRenderer`
把 Surface 代数绑定到尺寸、DPI、暂停、恢复与退休；GPU 纹理帧只在宿主 present 确认后结束
待呈现状态。能力矩阵是编译契约，不把 Xvfb/llvmpipe 夹具称为 jasper-os 或硬件 GPU 支持。

## 证据与下一步

2026-10-23 前序核查默认配置测试 100 项通过，未覆盖可选特性完整矩阵。此次版本耗尽修复后，在构建虚拟机中默认配置 101 项测试通过（94 单元、1 smoke、5 布局、1 文档）；版本函数 Verus 复核 1 verified/0 errors，反例被拒。证据见 `evidence/version-kernel/`。历史窗口单帧运行记录不等于本次实测，也不等于系统桌面已验收。

首批顺序：事实/契约/证明义务 → 版本与解码安全 → 共享文字布局/字体注入 → 软件文字 → 编辑/IME → Jasper 设置页/风格状态保持。系统内置接口从第一步参与；实现/运行验收依赖真实系统前置。

直接依赖与特性以 Cargo.toml/Cargo.lock 为准，记录见 [dependencies.md](dependencies.md)。具体版本不等于所有传递依赖已完成发行许可审计。

2026-10-23 后续执行：读取端点函数形式化证明 1 verified/0 errors，错误返回值反例被拒；默认测试 104 项通过。新增短恶意帧在操作数组预分配前拒绝；完整解码/游标事务与资源额度仍未证明。证据在 `evidence/decode-range-kernel/`；版本内核也已重验。

2026-10-23 继续执行：新增 checked 构造/全树 ID 校验，批更新与布局入口拒绝重复节点身份；子树后代碰撞不会提交前序暂存修改。版本准入内核及其依赖 2 verified/0 errors，错误准入反例被拒；默认测试 109 项通过。整个场景事务、身份集合与遍历尚未完成证明。

2026-10-23 后续证明：生产 take_bytes 与真实端点依赖 2 verified/0 errors；精确返回字节区间，成功精确推进，失败位置不变。两种破坏状态更新的反例均被拒。默认 109 项测试通过；完整消息解码与额度仍未证明。证据见 `evidence/byte-read-kernel/`。

2026-10-23 本轮补闭合 Cursor::take 的精确字节/位置/Truncated 错误规格；同场真实依赖验证 4 verified/0 errors（含派生检查计数），两个错误方法体反例被拒。未证明的派生 Clone 警告保留且读取路径不依赖它。五项证明入口均已重验，默认测试仍 109 项通过；复合字段/整批解析与预算继续未完成。

2026-10-23 整数与帧头：u8/u16/u32 精确数值/状态和完整十字节帧头校验已证明；主解码器使用此帧头入口。默认 115 项测试通过，七个证明入口均按最终源码重验且反例被拒。浮点、字符串、完整绘制操作解析与预算仍未完成证明。

2026-10-23 有界载荷：字符串字节读取已使用已证明的 bounded_bytes；前缀长度、上限、返回精确字节和各错误下位置策略已证明。默认 118 项测试通过，八个证明入口最终重验且反例被拒。UTF-8 转换/分配和完整解码/全局额度继续未证明。


2026-10-23 新增 UI Rust 软件 Surface 的 RGBA8 行长额度检查：非空、精确四字节/像素、额度内成功，算术不溢出；不分配内存。该新增函数与既有 checked_read_end/take_bytes 已通过 Verus + Z3 → Proptest → Miri → cargo-fuzz → Kani → Loom 的限定范围验证。修复了 Kani 验证构建沿用旧提取缓存的问题，按源码哈希隔离构建；证据与当前源码一致。整个 Surface、现有 Canvas 分配/resize、整个解码器及其他 UI 模块仍未完成证明。默认测试 123 项通过，CI 已接同一入口、远端尚未执行。性能指标只记后续优化，不阻塞当前功能推进。


2026-10-24：新增 UI Rust surface_span，高度/行跨度/容量及最后有效字节终点的数值准入，拒绝空几何、重叠 stride、溢出和容量不足，末行无需 padding。新增函数通过完整六阶段限定范围验证；同时为旧 next_scene_version/decide_scene_version 补齐同链。十个 Verus 入口最终重验，Surface SPAN 1 verified/0 errors、两个反例被拒；默认 123 项测试通过。Kani 跨度仅覆盖 u8 几何、全 usize 容量，全域性质由 Verus 证明；Loom 仅为真实纯函数的隔离调用，不代表系统服务并发已证明。完整 Canvas 分配/Surface 存储权限及生命周期仍未闭合。


2026-10-24 后续：新增 Surface 只读行借用 surface_row，先检查整个声明的 Surface 确实落在实际字节缓冲内，再借用不含 padding 的有效行，拒绝越界/非法/溢出；不复制、不分配，无裸指针。新实现完成六阶段，旧 Cursor::take 同时补齐六阶段；Verus 新借用加依赖 2 verified/0 errors，两个错误实现被拒。十一入口最终重验，默认 123 项测试通过，源码/日志/生成映射哈希一致。外部存储权限/代际资源、可写借用与旧 Canvas 接入仍未完成。


2026-10-24 后续：新增 write_surface_row，完整准入后同步复制一行有效字节；成功保留 padding 与其他行，失败整个目标不变。新函数完成六阶段，旧 u8/u16/u32 读取器同步补齐六阶段。新增写入与依赖 Verus 3 verified/0 errors，跳过复制/错误写入起点的反例被拒；十二项证明入口最终重验。默认 123 项测试通过，源码/日志/生成映射哈希一致。copy_from_slice 的 vstd 标准库规格明确列为信任边界，不宣称该 copy 自身已由本项目全域证明。外部存储权限/代际资源、跨进程发布与旧 Canvas 接入仍未完成。


2026-10-24 后续：新增 Rgba8SurfaceView，私有字段封装经检查的 RGBA8 宿主字节、宽高与 stride；公开构造、三项元数据与 row 访问，零拷贝/无分配。五个新方法均完成六阶段，旧 decode_header 同步补齐六阶段。新视图加依赖 Verus 8 verified/0 errors，十三项入口最终重验；默认 123 项测试通过。公共契约采用 closed ghost model/reveal，保留生产字段私有及运行体；源/日志/生成映射哈希一致。模块级未闭合义务已登记，外部资源权限和旧 Canvas 接入仍未完成。


2026-10-24 后续：Rgba8SurfaceView 新增 copy_row_to，将有效行传输至等长调用方输出，成功精确相等、失败全目标保持；视图修改后六方法全链重验。旧 Cursor::bounded_bytes 补齐六阶段，覆盖短前缀、超限优先、载荷不足及精确位置。十三项 Verus 入口最终重验；默认 123 项测试通过，报告/源/日志/生成映射哈希一致。视图传输仍为同步独占调用，不代表跨进程发布或资源权限已完成。


2026-10-24 后续：新增无窗口依赖 rgb8_word；现有 desktop::rgba_to_u32 改为显式安全长度检查和可验证循环，整体转换的精确 RGB/忽略 alpha、前置失败全输出不变、边界安全与终止已证明，不只是打包 helper。新/改代码完成六阶段，十五个 Verus 入口最终重验；默认 123 项、desktop 特性五项测试通过。fuzz 初始语料与潜在失败输入归档、Kani 映射和全部源/日志哈希已复核。桌面输入/窗口/呈现生命周期不据此宣称已证明；CI 未远端运行。


2026-10-24 后续：真实软件 raster::Canvas 新增 from_rgba8，接管非零尺寸且长度精确匹配的调用方 Vec，无像素分配/复制；拒绝时返还原缓冲。既有 width/height/pixels 三访问器同步补齐六阶段，类型定义和生产函数体原样提取，闭合模型保留私有字段。新导入及依赖/类型检查 Verus 12 verified/0 errors；错误宽度/错误缓冲反例被拒，十六项入口全重验。属性十六项、Kani 十六 harness、Miri 五项、真实 API 两项及 Loom 隔离调用均通过；默认 125 项和 desktop 五项测试通过，源码/报告/日志/实际 Kani 生成源一致。旧 new 分配与 resize 错误传播、浮点裁剪/混色/完整渲染器未证明，外部共享存储与权限/同步也未涵盖；CI 未远端运行。


2026-10-24 策略落地预检：独立 experiments/skia-cpu-probe 已用真实 Skia 0.153.3 在构建 VM 跑通 CPU 填充、嵌套无交集裁剪、透明混色、RGBA 读回并输出图像；locked 复跑成功。根引擎依赖及生产实现未变，不继续自写光栅器。构建/链接失败已保留并解决，精确源/锁/日志/产物哈希有报告。此结果只证明该 Linux 实验可运行，不是主渲染路径已替换，不是文字/系统宿主/六阶段已完成。3D 延后，UI 线不接管 3D 引擎。


2026-10-24 后续：真实 Scene/Taffy/paint/UiBatch 编解码已在隔离实验中重放给 Skia，三布局节点、十条实际 PaintOp、188 字节；四像素、非法裁剪/文字拒绝检查及 locked 运行、格式检查通过。源快照和日志/产物哈希已归档，既有生产六阶段源码哈希未变。正式后端与新增实验映射的六阶段尚未完成。按用户最新裁决，只推进最低限度绘图库接入，Canvas 完整能力与专项替换放最后，其他 UI Rust 框架工作优先。


2026-10-24 非 Canvas 主线：新增 resource::ResourceGate，单槽会话/槽位/代数匹配、独占签发、撤销、失败保持与代数耗尽不回绕，四方法完成六阶段；不分配真实存储，不接管内核。同步为旧键盘 Role 的两个激活分类器补齐完整链。新资源 Verus 5 verified/0 errors、旧分类 3 verified/0 errors；负控制全部拒绝。十八个入口最终重验、Proptest 十八项、Miri 七项、Kani 十八 harness、Loom 真实隔离调用通过；默认 127 项、desktop 五项及格式检查通过，源/报告/日志/Kani 提取映射一致。真实宿主认证、全局唯一注册表、存储释放和在途取消仍未完成，Skia 正式默认后端仍待接入验收。Canvas 专项继续最后安排。


2026-10-24 非 Canvas 后续：新增 UiBytesSlot，在持久 ResourceGate 后持有实际 Vec 字节，安装接管/失败返还、受检只读借用、释放返还原 Vec 并清空槽位、重装拒绝旧 key，四方法及 gate 全链完成。同步为既有 no_command_modifier 补齐验证；没有改键盘业务语义。字节资源 9 verified/0 errors、旧谓词及派生 4 verified/0 errors，二十项入口和负控制最终重验。属性二十项、Miri 九项、Kani 二十 harness、Loom 实际共享只读存储/独占释放通过；默认 129 项与 desktop 五项、格式检查通过，源/报告/日志/实际提取映射一致。字体解析、全局注册、宿主认证/IPC 和在途取消仍未完成，不扩展 Canvas 专项。


2026-10-24 UI 文本接缝：Skia 隔离场景实验支持从实际 UiBytesSlot 显式读取宿主注入字体，以 custom_empty 管理器从内存解析并绘制 Latin 标题，无系统 FontMgr/字体族查找调用；四布局节点、十一真实 PaintOp、218 字节批，对照帧有 535 个标题区字形像素变化。输入字体/版权/SHA 与源快照归档；无字体路径继续明确拒绝文字。仍是 Linux/native 功能实验，正式默认后端、完整 shaping/索引/真实字体测量及 jasper-os 适配未完成。同步为旧 is_wide/is_zero_width 补齐六阶段，只证明近似范围表实现，2 verified/0 errors；二十一个入口重验、默认129项/desktop五项/格式检查通过，源/报告/日志/Kani 映射一致。未扩展 Canvas 专项。


2026-10-24 非 Canvas 后续：ResourceGate/UiBytesSlot 新增宿主管理 retire，永久关闭同一槽位、取回原字节并拒绝再签发/安装，正常 release 的可复用语义保持；两个内核全部方法重验。旧 FocusManager::blur/order/focused 同步补全链，精确清空焦点并保持遍历序列，不代表取消事件/全局会话注销已实现。gate 6、字节组合11、焦点5 verified/0 errors；二十二个入口、属性二十二项、Miri九项、Kani二十二 harness、Loom一项通过；默认129项/desktop五项和格式检查通过，源/报告/日志/Kani映射一致。Kani 初轮因测试构造复杂度内存耗尽未计通过，固定容量四节点模型保留长度0..4/全部u64/Option焦点逻辑域后诊断通过，再从头通过全链，失败快照保留。没有扩展 Canvas，也未宣称实际输入在途取消或Skia正式后端验收。

2026-10-24 焦点生命周期增量：KeyboardRouter 新增 take_focus，精确返回旧焦点并清空、保持遍历顺序，重复调用 None；原 focused/blur 同轮补全链。新增入口连同依赖 Verus 9/0，完整六阶段二十三入口重跑通过（属性23、Miri9、Kani23、Loom1），源码/日志/实际提取映射核对一致。真实路由回归验证清空后仍能 Tab 导航。资源准入尚未与输入关闭/预编辑归属/取消投递联动，不算整会话注销完成。未扩展 Canvas。

本轮默认 cargo test --locked 共130项通过（含文档测试），desktop专项5项及cargo fmt --check通过。未提交或推送；CI配置已纳入新证据目录，未声称远端CI已执行。

2026-10-24 宿主关闭步骤增量：retire_focus 组合真实准入退役与原焦点取回；同轮旧 FocusManager::focus 改用等价显式扫描并补齐实现级证明。全链25入口/属性25/Miri9/Kani25/Loom1通过，默认131项、desktop5项、格式检查通过；源码、日志及生成体哈希核对一致。首个 contains 版本后置条件未证，通过记录只对应后续实际扫描实现；失败证据保留。归属配对与全入口准入仍由宿主保证，未完成预编辑/指针取消及会话服务。

2026-10-24 悬停退出增量：HoverTracker::retire_hover 真实返回 Leave 意图并永久关闭传入 gate，同轮旧 hovered/update/clear 补齐精确事件顺序与最终归属证明。Option相等判断改为等价None/Some/u64显式分支，首次未证快照保留；未降低规格。全链27个Verus入口/属性26/Miri9/Kani26/Loom1通过，默认132项、desktop5项和格式检查通过；源、日志、Kani映射一致。仅悬停退出，不代表指针捕获/按压/手势/IME取消或系统输入服务完成。

2026-10-24 开发中，尚未验收：PointerId显式比较与captured_target/has_capture真实map查询已进入源码；初轮29个Verus入口和对应快照的Proptest/Miri/fuzz通过，但Kani捕获map夹具发生内存耗尽，完整链未完成，后续Loom未执行。已保留停止/失败诊断并继续定位；当前文件有后续验证夹具改动，不能把初轮结果套用为当前完整验收。新的捕获查询和比较迁移均不计已交付，最近完整验收仍是此前27入口的历史源码快照，不能声称其哈希匹配当前开发文件。

2026-10-24 更新：上述开发中切片已在最终声明的限定范围内，从Verus起重新通过完整六阶段。新增captured_target返回真实capture保存的NodeId，旧has_capture和PointerId比较同步补证；29个Verus入口、属性28/Miri9/Kani28/Loom1，默认133项/desktop5项/格式检查通过。Kani捕获查询限真实空表/固定键7的单键表与任意查询/目标，旧双键模型仍未完成，不计通过；任意map实现级性质由Verus覆盖。std析构仅在该借用查询Kani夹具中排除，原生/Miri/Loom保留析构。当前源码/日志/实际生成体哈希一致并已存完整源码快照；整个指针取消、场景ID复用失效、会话服务及整个引擎仍未完成。

**正式验收裁定：捕获查询仍未完成。** 上述全链成功仅对应明确记录的空表/单键受限配置，不替代原先双键Kani目标，也不因此降低验收门槛。双键模型仍失败/未完成，所以不把新captured_target标记为已交付。Verus任意map证明、比较内核补证和受限全链证据保留，后续继续解决双键验证；全引擎完成状态仍为false。

### 验收入口防误报（2026-10-24 06:45 UTC）

#12 未变化。本轮未修改生产 Rust；修正 verify-ui.py 的结果语义：六阶段受限检查即使全部成功，也只设置 scoped_checks_complete；原双键 Kani 义务仍登记在 acceptance_pending，slice_complete 保持 false，进程退出 2。当前强制待验项须在原目标真正通过后连同证据更新，不能因单键诊断通过自动解除。直接执行实际报告尾部的失败门禁检查通过，证据为 evidence/capture-query-smt/acceptance-gate-check.json；没有重跑六阶段，旧 report.json/source-snapshot.zip 是原脚本版本的历史证据，不能作为当前脚本的验收结果。下一实现仍需解决双键真实容器验证；Canvas 排程不变。

## Scintilla / Lexilla 暂用依赖（2026-10-24）

根据用户最新裁决，编辑后端优先复用 Scintilla，高亮复用 Lexilla，后续逐层用验证后的 Rust 替换。源包、许可、固定版本与构建入口位于 `third_party/editor/`，详见其 README。C++ 内部为显式未证明第三方边界，不伪称数学证明；自有新增/修改 Rust 门槛保持完整六阶段。当前仅引入固定依赖及离线构建/上游测试入口，生产 Rust/Cargo 未变，正式编辑、视图和 IME 适配尚未交付。jasper-os 不默认引入 GTK/Qt/Win32；Canvas 仍最后。

Scintilla/Lexilla 引入验证结果：Scintilla 52 用例/3817 断言、Lexilla 11 用例/646 断言及 209 次 lexer 样例运行全部通过；真实 Lexilla .a/.so 已构建。编译前后逐项核对 1183 个原始源码文件，四阶段日志及当前脚本哈希已核对，evidence/editor-deps/report.json 与 audit.json 保留结果。CI 已增加独立 editor-upstream job，未推送或声称远端执行。当前仅依赖引入和构建验证完成，生产 Rust/Cargo 无新增修改；Rust 编辑接入、系统宿主与 IME 完成标志均 false，原双键 Kani 阻塞仍保持未完成。

## 输入法方案许可裁决（2026-10-24，覆盖此前 Fcitx 方向）

用户要求通用输入法框架自身也使用宽松许可，撤销 Fcitx 5 / fcitx5-rime 选型。两者的锁定依赖、源包、构建步骤及 LGPL 许可副本已移除；保留的历史失败证据不是现行依赖。当前通用方案改为核验 MIT 许可 Keyman 的独立 Core 与宿主接口，中文引擎保留 BSD-3-Clause librime 候选，不宣称 Core 等于完整系统输入法服务。引擎/词库/传递依赖均须逐项核验，不将顶层宽松许可外推。当前尚无 Rust 输入法交付，详见 third_party/ime/README.md；生产 Rust/Cargo 无变更。


替代源已固定：Keyman 18.0.255 / 02d7ffe6008dac8d2fc21df7f9d5522c6070cc39，Core 的 meson.build 与上游 LICENSE.md 均声明 MIT。选择性原始源码快照、许可与精确重建命令/SHA-256已纳入 third_party/ime，安全提取、文件核对通过。旧 Fcitx/framework/addon 步骤与分发源包均不在现行清单。Keyman 尚未构建和接通；librime 仅保留源候选，通用框架输入上下文/候选服务/宿主适配未完成，不以固定 Core 源码代替完整框架交付。生产 Rust/Cargo 未变，全部传递依赖与数据授权仍待单独核验。


2026-10-24 11:30 UTC 轮次：#12 未变。MIT Keyman Core 在 Jasper 构建 VM 以系统 ICU、禁用下载 fallback 的 Meson 配置真实构建出 libkeymancore；common/cpp 补入源码快照以满足真实编译依赖，原文件未修改。上游要求的标准构建资源按其 build.sh 原样复制。仅库构建成功，测试明确关闭且未计通过；通用框架/系统宿主/Rust 接入/形式化证明标志全部保持 false，日志及源/库/工具输出映射留在 evidence/ime-deps。现行锁与构建入口不再含 Fcitx 5 / fcitx5-rime。新增 docs/input-method-boundary.md 登记 Keyman 标量删除与 Scintilla UTF-8 字节位置差异、原生借用失效、Rime 索引待核对项，以及会话/提交/取消归属义务。未新增生产 Rust，不把库编译当作可用输入法交付。下一步应跑真实键盘/状态测试并继续接入，不扩展 Canvas，也不继续耗在原双键模型的重复诊断。


2026-10-24 11:40 UTC：#12 未变。BSD librime 1.17.0 在 VM 完成真实构建、非空上游 rime_test 通过并只安装至本项目 target 前缀。新独立实验通过上游 rime_api_console + 自有两词 YAML 夹具跑出 nihao→候选“你好”→空格提交“你好”，及 shijie→提交“世界”；非假引擎、未读取用户桌面词库。report/fixture/原生可执行文件/日志哈希留在 evidence/rime-smoke，Keyman 与 Rime 的构建报告改为各自目录避免互相覆盖。当前自有 Rust 接入、系统宿主、编辑框与形式化证明均仍未完成；新 Core 上游测试仍未执行，未混算。并未以最小夹具替代正式现代词库，后者授权与集成待完成。根生产 Rust/Cargo 未变，本轮无新增 Rust 交付；原双键 Kani 待验项未解除。下一功能步骤为实际通用输入上下文/键盘状态与 Rust 编辑宿主接入，Canvas 继续最后。


## 最新排程裁决（2026-10-24 12:00 UTC，覆盖此前宿主实装排期）

用户明确：jasper-os 实际接入安排在 Canvas 之后。当前顺序为 UI Rust 主线（指针捕获取消、输入归属/生命周期、编辑及通用输入法接入）→ Canvas 接入与补齐 → jasper-os 对接。此时只维护系统宿主接口和可移植性约束，不做 jasper-os 集成，不修改其内核或排期。Keyman MIT / librime BSD 方案、Scintilla/Lexilla 暂用策略及自有 Rust 完整六阶段门槛不变；Fcitx 5 与 fcitx5-rime 不恢复。

### 2026-10-24：UI 输入身份准入切片

新增可复用的 `input_identity` API：在既有 ResourceGate 上同时检查目标代数、文档 ID 和版本。限定切片已完成 Verus+Z3、Proptest、Miri、fuzz、Kani 与 Loom 适用性登记（纯同步路径不适用）；证据见 docs/verification.md。尚待接入实际路由入口、实现原子绑定更新与在途取消，不代表完整输入生命周期完成。捕获存储 Kani 按用户裁决延期，未算通过；本轮没有重试该诊断。

### 2026-10-24：真实焦点与输入绑定协同

已增加活动绑定的协同焦点转移入口：预检目标、轮换票据、提交焦点；非法目标、旧回调、错误所属方及代数耗尽均保持两个对象原状。限定实现已完成六阶段登记（Loom同步路径不适用），实际Scene/焦点/文档转移与新负载消费测试通过，证据见 docs/verification.md。仍需初始绑定、Scene变更生命周期、实际IME/指针取消及成熟编辑器接入，不能据此宣称框架完成。

2026-10-24 17:22 UTC：输入端到端仍未完成；最新内部状态限定证据不能替代真实编辑/IME/呈现。U1独立原生实验复用已有skia-safe0.153.3 Paragraph，在同一对象完成测量/布局/CPU绘制，通过显式UiBytesSlot字体字节注入DejaVu及Noto Sans CJK SC（SIL OFL1.1），固定拉丁/组合字符/中文/阿拉伯文字无缺字；未注入CJK基线明确有2缺字。双字体帧1973非白像素，宽193.34879/高58，窄布局高136，重复同进程绘制一致，ASCII范围盒1。原生PNG已检查。此为未验收实验，**自有Rust完整六阶段尚未开始**，production_accepted=false；正式backend.rs仍返回DrawText Unsupported，未将实验冒充正式渲染器。未扩展Canvas API。源/锁文件/字体版本许可/哈希/原生日志见evidence/skia-text-paragraph{-cjk}/report.json。仍依赖Linux-native动态库，未推断jasper-os可移植性；仅限额VM编译/执行。


### 2026-10-24 17:42 UTC：U1 Unicode 选区原生实验（未验收）

在受限验证 VM 中，以固定 Skia 0.153.3 和显式注入字体运行 `A😀é中B`
（12 UTF-8 字节、7 UTF-16 单元）。完整代理对 `1..3`、组合字素 `3..5`、
中文 `5..6` 均返回矩形；尾代理项 `2..3`、组合尾部 `4..5` 均无矩形，
与此前读取的固定原生实现相符。证据：引擎 `evidence/skia-text-index-probe/`
含日志、矩形观察、源码快照/哈希与 report.json。编译 0.35 秒，宿主机未运行 Rust 测试。
此为第三方行为夹具实验，自有实验代码六阶段均未启动、不能算通过；未接入生产
DrawText/编辑器，也不宣称输入闭环完成。不新增公共索引服务或 Canvas 功能。


### 2026-10-24 17:52 UTC：U1 下一最小切片收敛

核查 #12/#13 无变化。已对照真实 Taffy 测量与 DrawText 路径，记录
`docs/text-layout-integration.md`：当前绘制指令缺最终宽度和布局结果身份，单换
测量器或重放时重排不能算共享布局；下一切片需在布局收敛后绑定最终布局结果、
资源代际与场景版本，再供绘制消费。规格未实施、未验收，不计功能进度。
本轮未修改生产 Rust，也未运行重型测试。


### 2026-10-24 18:02 UTC：真实 Scene 文本纵向实验（未验收）

新增独立实验 `experiments/skia-cpu-probe/src/scene_text.rs`，以注入字体的 Skia
Paragraph 实现实验测量器，驱动真实 Scene → Taffy；布局收敛后用最终 content
宽度创建 Paragraph，核对其高度后直接 CPU 绘制。同文不同宽的两个节点分别为
120/168 与 280/90（宽/高），均与 Taffy 最终 content 高度一致；PNG 已检查。
证据 `evidence/skia-scene-text/` 含源码快照、日志、图像与未验收报告。
VM 编译 0.40 秒，未在宿主运行 Rust 构建。#12/#13 无新增变化。

这推进了真实布局消费实验，不是生产共享布局交付：未接 UiBatch/DrawText，
尚无持久布局身份/过期拒绝，ascent/lines 为本夹具不消费的占位值。自有新增
实验 Rust 六阶段均未启动，不计验收通过，也没有补证完成声明。编辑与输入法
事务闭环仍未完成；下一步应收敛正式接入与证明，不继续增加文字展示夹具。


### 2026-10-24：恢复实施——本地文本帧接入代码（未验收）

已实际新增 src/text_skia.rs（显式 skia-text-pending feature，默认关闭）：显式字体
注入、真实 Taffy 测量、最终宽度 Paragraph 准备、源 Scene 持续不可变借用、
私有 CPU RGBA 消费。未修改 UiBatch 协议，既有 Engine::render 尚未迁移。
VM 原生接入 7 测试通过，场景修改借用编译失败用例 1 通过；默认关闭新 feature
的回归 139 项通过。已检查输出 PNG。初次编译类型错误已修复并保留失败日志。
证据 evidence/local-text-frame-pending/report.json 与源码快照。

这次为实际接入实现，仍非已验收交付：自有新增 Rust 六阶段均未完成，所有
LTF-01..06 证明义务登记 verification/local-text-frame/README.md；原生测试不
冒充顺序门槛。旧 surface 数值辅助的已有证明不覆盖新增调用链。实际编辑提交、
IME候选/取消交付、光标选区闭环仍未完成；jasper-os 接入继续排在 Canvas 之后。


### 2026-10-24 19:02 UTC 触发轮：文本帧存储准入实现与验证推进

生产 LocalTextRenderer::prepare 现调用 text_frame_storage，沿真实旧 Surface
行长/跨度函数完成数值准入。限定实现级 Verus 3/0、两个语义反例拒绝；Proptest
3测试（属性各256例）、Miri7边界、fuzz 4,431,914次/11秒通过。Kani同一完整
u128参考性质先CaDiCaL后一次Z3均在VM命令上限内未结束，退出124；无通过声明，
Loom阶段未到达。证据 evidence/local-text-storage-chain/report.json。

重构后真实新模块7个原生接入测试再次通过。完整文本帧/所有权/队列/原生FFI的
自有Rust证明仍待完成，整个新feature仍默认关闭、未验收，不能把这次数值子片
当文本或输入闭环完成。旧Surface函数原样纳入本次证明与测试，但无新增旧代码
完整补证交付声明。未改其他项目或在宿主执行重型测试；两次有界求解后本轮
不继续堆叠诊断重试，不降低性质或更改预算冒充通过。


### 2026-10-24 19:15 UTC 触发轮：继续落实文本帧安全边界（未验收）

实际代码已为本地渲染器和帧增加私有Rc幻影标记，约束安全Rust跨线程移动/共享；
新增原生绘制参数预检，拒绝NaN/无穷、负描边和矩形端点溢出。零宽描边不再
被Skia解释为hairline，与已有软件路径跳过零宽保持一致，未增加Canvas功能。
VM中9个原生测试、3个借用/线程编译失败检查通过；移除零宽保护的原生语义
负例确实像素断言失败，恢复源码后通过，VM/宿主源码哈希相同。

新增保护仍未获实现级数学证明，LTF-07/08已登记，不计六阶段完成。旧三函数
数值核心已机器核对与既有证明原样一致，映射记录在
local-text-frame-guard-pending/storage-proof-mapping.json；旧Kani超时保留，本轮
不重复求解、不降性质。完整模块仍默认关闭、未验收；输入闭环未完成。


### 2026-10-24 19:35 UTC 触发轮：验证环境故障与持续实现

已收取上一轮文字命中验证会话55788：退出124，仅有“开始编译”日志，无通过
结果；新有界只读SSH探测亦返回255、服务器无响应。未在宿主重试，也不修改
其他项目任务/VM服务。UI验证脚本新增外层300秒连接/准备/拉取上限，内部180秒
和3GiB/swap0/tasks128/jobs1不变；本地bash语法检查通过，不冒充Rust验证。

继续写入实际文字命中边界text_hit_offset，并准备原样源码提取的Verus精确
规格/两个负例及属性、Miri、fuzz、Kani专用入口。仅生成证明候选，VM未恢复前
六阶段均未执行，不能算通过。快照/运行状态位于local-text-hit-pending。
新feature保持默认关闭、未验收。下一轮先轻量核查VM，恢复后从Verus开始，
不重复旧存储Kani超时诊断，不把测试改到宿主，也不接管其他项目排期。

## 2026-10-27：Blitz 迁移 WIP 整理与首轮验收

本轮的输入是此前保留的未验收 WIP 提交（32 个文件改动、新增文档/验证/实验目录）。工作区已按任务要求整理，生产 Rust 语义未改写；下列结果只对应本轮明确列出的命令与源码状态。

### 已实测通过

| 检查 | 命令/范围 | 结果 |
|---|---|---|
| 默认构建 | `cargo build` | 通过 |
| 默认测试 | `cargo test` | 140 项通过（114 库、25 集成、1 文档；0 失败） |
| 全部目标 | `cargo test --all-targets` | 通过；未启用的原生/桌面示例按 `required-features` 跳过 |
| 原生 CPU 候选测试 | `JASPER_UI_TEST_FONT=<font-file> cargo test --features native-blitz-cpu-pending` | 203 项通过（174 库、28 集成、1 文档；0 失败） |
| 依赖钉扎检查 | `python3 scripts/check-blitz-native-dependencies.py` | 通过；根与诊断 crate 均解析到固定 Blitz 修订与归档摘要 |
| 公开类型边界 | `python3 scripts/check-blitz-public-types.py target/doc/jasper_ui_engine.json` | 通过；后端返回类型与私有别名两个反例均被拒 |
| 证明证据对应 | `python3 scripts/check-blitz-proof-mapping.py` | 通过；26 个映射内核与隐藏祖先辅助证明的源码/规格/日志摘要一致 |
| 证据检查负例 | `python3 scripts/test-blitz-proof-mapping.py` | 通过；缺失证据、源码漂移与非 proved 报告均被拒 |
| 版本内核复核 | `python3 scripts/verify-version.py --kernel version --verus <verus>` | 1 verified / 0 errors；错误版本反例被拒 |

说明：

- `native-blitz-cpu-pending` 测试按设计需要显式注入字体。未设置 `JASPER_UI_TEST_FONT` 时，58 项原生测试会以 `explicit injected test font required: NotPresent` 失败；这是缺少宿主字体输入，不是渲染或布局通过。设置字体后全部通过。
- `check-blitz-proof-mapping.py` 依赖非版本的 `evidence/` 目录。本轮把保留的 `blitz-hidden-ancestry-proof-pending` 证据补入构建 VM 后才完整通过；从干净检出直接运行时，缺失证据会按脚本设计失败。
- `check-blitz-public-types.py` 的输入是在构建 VM 内生成的 rustdoc JSON，不是仓库内预置文件。
- `scripts/verify-ui.py` 的完整六阶段链本轮未执行，也不计为通过：其范围包含已知的 `capture-query` 双键 Kani 待验项，且与本轮整理/编译验收不同。不得用上表任何单项结果替代该链或整个原生适配器验收。

### 公开内容整理

- 全仓关键词扫描（本机路径、内部构建入口、内部任务/进度文档名）清零；命令与宿主注入字体改为由运行环境变量提供。
- 删除被提交的 `verification/capture-slots/__pycache__/` 字节码；`.gitignore` 增加 Python 字节码、嵌套实验 crate 的 `target/` 与第三方构建产物。
- `scripts/verify-in-vm.sh` 不再内嵌宿主路径，改为要求 `JASPER_UI_VM_RUN` 指向构建 VM 入口；已用一个 VM 内命令验证包装器可用。

### 仍未验收

- 整个原生 Blitz 适配器、根 `engine::Engine` 入口、真实窗口呈现与 Canvas 仍未验收；`verification/blitz-adapter/obligations.json` 中的逐方法义务保持原状态。
- 上表的证明证据对应与版本内核复核只覆盖各自声明范围，不构成原生适配器或整仓证明。
- `cargo fmt --check` 仍会报告 WIP 源码的格式差异；本轮不做大规模机械重排，以免把迁移审查混入无关改动。
- 未发现需要写入「迁移卡点」的方向性阻塞：默认与原生 CPU 候选均在构建 VM 内编译、测试通过。

### 2026-10-27 复核（独立重跑）

- 在构建 VM 内独立重跑：`cargo build`、`cargo test`（114 库 + 集成全部通过）、`JASPER_UI_TEST_FONT=<DejaVuSans.ttf> cargo test --features native-blitz-cpu-pending`（174 库 + 集成全部通过，0 失败）、`check-blitz-native-dependencies.py`、`test-blitz-proof-mapping.py` 均通过，与上表一致。
- 依赖合规补正：`proptest` 由 `1.9` 改为精确钉扎 `=1.12.0`（与锁文件一致）；新增可选依赖（Blitz 三件、anyrender、vello、parley、fontique、keyboard-types）补记入 `THIRD_PARTY_LICENSES.md`，均为 MIT/Apache-2.0 双许可，按 Apache-2.0 使用。
- `scripts/verify-ui.py` 的 Proptest 完成令牌过期：`desktop::tests` 在迁移中由 5 项增至 6 项，脚本仍要求精确的「5 passed」，导致假失败；已改为「6 passed」。
- 六阶段链 `python3 scripts/verify-ui.py --verus <verus>` 实跑：Verus+Z3 / Proptest / Miri / cargo-fuzz（两目标各 20 秒：183616 与 1289932 次）/ Kani（28 项 SUCCESSFUL）/ Loom（1 项）均为 `passed_scoped_checks`。报告 `complete=false`：capture-query 任意双键 Kani 原义务仍未解决（单键 harness 仅为诊断）。该链范围不含新增 `native_adapter/` 代码，原生适配器仍未验收。
- 已知遗留：默认特性下 `backend::submit_native_frame` 未使用告警（仅原生特性使用）。

## 2026-10-27：原生 Blitz CPU 路径无头渲染验收（UI2）

新增集成测试 `tests/native_cpu_acceptance.rs`（`native-blitz-cpu-pending`），经根入口 `engine::Engine` 完成「场景 → 布局 → CPU 绘制到宿主 RGBA 缓冲 → 断言」。场景 320×200：Flex 列容器（内边距 10、间距 8、背景）内含 Block 头部（2px 边框、背景、内边距 4，内有中英文字 `Hello 你好`）、Flex 行（两块定尺寸矩形，一块带 1px 边框）、中英文字说明 `Jasper 界面`（20px、指定颜色）。字体由宿主显式注入（`JASPER_UI_TEST_CJK_FONT`，本轮使用 Noto Sans CJK SC Regular，SIL OFL 1.1），测试不扫描系统字体。

| 测试 | 断言 | 结果 |
|---|---|---|
| `layout_boxes_are_exact` | 7 个节点布局框精确相等；未知节点、错误归属、错误版本均返回 None | 通过 |
| `fills_borders_and_backgrounds_sample_correctly` | 15 个采样点（根/头部背景、边框四角、矩形填充与边框、间隙）容差 ±2；同帧重绘逐字节一致 | 通过 |
| `latin_and_han_text_leave_ink_only_where_glyphs_are` | 两段文字有墨迹、从行首开始、在 x<200 结束（其后无墨迹）；汉字在纯拉丁同行之外新增 ≥26 列墨迹（实测 32）；文字像素达到声明颜色（±8） | 通过 |
| `legacy_layout_difference_table` | 同一场景在 LegacyEngine 上的布局差异锁定为下表 | 通过 |

负例：改注入仅含拉丁字形的 DejaVu Sans 时，4 项中 3 项按预期失败（文字行高不符；汉字列仅 18 列为缺字框），说明文字断言不是空断言。

原生与 LegacyEngine 布局框差异（只记录，不要求一致）：

| 节点 | 原生 Blitz | LegacyEngine | 差异 |
|---|---|---|---|
| 1 根容器 | (0,0) 320×200 | 同 | 无 |
| 2 Block 头部 | (10,10) 300×40 | 同 | 无 |
| 3 头部文字 | (16,16) 288×24 | (16,16) 288×20 | 高度 24 vs 20：原生按真实字体行高，旧引擎为近似文字度量 |
| 4 Flex 行 | (10,58) 300×60 | 同 | 无 |
| 5 矩形 | (10,58) 100×60 | 同 | 无 |
| 6 矩形 | (120,58) 120×60 | 同 | 无 |
| 7 说明文字 | (10,126) 300×29 | (10,126) 300×25 | 高度 29 vs 25：同上 |

回归：构建 VM 内默认 `cargo test` 全过（114 库 + 集成）；`native-blitz-cpu-pending` 全量 207 项通过（174 库 + 33 集成/文档，含本节 4 项；0 失败）。

`examples/window.rs --once`：构建 VM 无显示服务器（未设 `DISPLAY`/`WAYLAND_DISPLAY`，未安装 Xvfb），winit 明确报错 `neither WAYLAND_DISPLAY nor WAYLAND_SOCKET nor DISPLAY is set` 并以退出码 1 结束；示例没有软件后端回退或静默跳过。真实窗口呈现仍未验证，需在有显示的宿主上执行。

根入口状态：保留 “not yet accepted”。无头 CPU 路径已对上述场景实测通过，但原生适配器代码尚未进入六步验证链（义务清单仍为 candidate_not_accepted），真实窗口呈现也未验证；两者完成前不去掉该标记。

## 2026-10-27：原生路径真实文字光栅与共享字形布局（UI3）

根入口新增 `engine::Engine::with_fonts(owner, scene, fonts, w, h)`：宿主按顺序注入多个字体文件（如先拉丁、后中日韩），文件之间按给定顺序回退；同一文件内（地区字体集）沿用族名字典序，不依赖上游 HashMap 迭代序。空栈、超过 `native::MAX_INJECTED_FONTS`（16）或任一文件未注册出字体族时返回 None，不静默跳过。单字体 `Engine::new` 行为不变。原生文字的测量（布局框）与绘制（Vello CPU 光栅）来自同一份 Blitz/Parley 布局。

新增集成测试 `tests/native_text_layout.rs`（`native-blitz-cpu-pending`；注入 `JASPER_UI_TEST_LATIN_FONT`=DejaVu Sans、`JASPER_UI_TEST_CJK_FONT`=Noto Sans CJK，两个独立文件）：

| 测试 | 断言 | 实测 |
|---|---|---|
| `fallback_follows_host_order` | 拉丁文字使用栈中首个覆盖文件：[拉丁,CJK] 帧与只注入拉丁逐字节一致，[CJK,拉丁] 帧与只注入 CJK 一致，两种顺序帧不同；汉字越过仅拉丁文件回退到 CJK | 行高 19 / 24；汉字墨迹 31 列（仅拉丁栈为缺字框 18 列） |
| `stack_admission_rejects_explicitly` | 空栈、非字体字节、[拉丁, 非字体]、17 个文件均拒绝；16 个重复文件只登记一次族，帧与单文件一致 | 通过 |
| `wrapped_lines_match_measured_box` | 120px 宽框内折行：框高 = 行数 × 单行高；每个测量行槽有墨迹；框外整帧无墨迹；更宽框行数更少 | 4 行 × 19 = 76 |
| `mixed_script_wrap_stays_inside_box` | 中英混排折行：首/末墨迹行落在首/末行内，框外无墨迹 | 3 行 × 24 = 72 |
| `font_size_and_colour_are_applied` | 32px 与 16px 的行高、墨迹高、墨迹宽比值在 1.7–2.3；完全覆盖像素达到声明颜色（±4） | 行高 38/19 = 2.0，墨迹宽 99/50 |

构建虚拟机复测：默认配置单元 114 项及全部集成测试通过；`native-blitz-cpu-pending`（注入三份字体）单元 174 项及集成测试全部通过（`native_text_layout` 5/5、`native_cpu_acceptance` 4/4）。字体栈准入与首次出现回退序内核（`font_stack_count_admits`/`font_order_admits`）完成六步链：Verus 4 verified/0 errors（空栈准入、重复准入两反例被拒）、Proptest 30 项、Miri、cargo-fuzz（14.0 万/197.4 万次）、Kani 30 个 harness 全过、Loom；整链报告 `slice_complete=false`，原因仍是既有 capture-query 双键 Kani 义务未闭合，不是本切片。字体集合注册与 Parley 回退选择本身不在证明范围。

未完成：场景文字的命中/光标几何尚未经根入口暴露（`TextDisplayView` 仍为单字体）；字重/斜体/字体族选择、双向文字、行高样式未验收。根入口仍标记 not yet accepted。

## 2026-10-27：原生路径按需帧调度起步（UI4）

根入口 `engine::Engine` 新增 `needs_frame(owner) -> Option<bool>` 与 `invalidate_frame(owner) -> bool`。视图在每个可能改变画面的入口（场景更新、指针、按键、焦点设置/清除、停用、视口尺寸/缩放、视口滚动）进入准入检查之前即标记失效：被拒调用至多多绘一帧，不会复用过期帧。`needs_frame` 为 `Some(false)` 时宿主可跳过渲染；对未变化视图重复调用 `render_to_surface`/`render_to_backend` 时，复用上一次绘制的紧凑帧而不重新绘制。复用条件（未失效、缓存尺寸与当前视口一致、长度等于本次准入的紧凑长度）由 `frame_schedule::frame_reuse_admits` 决定。非归属方或已关闭视图：`needs_frame` 返回 None，`invalidate_frame` 返回 false，渲染不会返回缓存。这只是整帧粒度的调度：没有区域失效、帧节拍或动画驱动，GPU 路径尚未接入。

新增集成测试 `tests/native_frame_schedule.rs`（`native-blitz-cpu-pending`，注入 `JASPER_UI_TEST_FONT`=DejaVu Sans）：

| 测试 | 断言 | 实测 |
|---|---|---|
| `unchanged_view_needs_no_frame_and_reuses_pixels` | 首帧必须绘制；绘制后 `Some(false)`；重复渲染逐字节一致；非归属方无法使其失效；`invalidate_frame` 后重新绘制，结果与原帧一致 | 160×100 首帧绘制 4.9 ms，复用 26 µs（调试构建、构建虚拟机，仅作量级参考） |
| `scene_update_invalidates_and_repaints` | 样式更新后需要新帧且像素变为新颜色（不复用缓存）；过期版本的更新被拒，但仍保守标脏，重绘结果不变 | 通过 |
| `viewport_pointer_and_close_drive_scheduling` | 视口缩小/恢复后都需要新帧，恢复后的整帧与最初一致；指针移动、视口滚动标脏；关闭后返回 None，不再返回缓存帧 | 通过 |

构建虚拟机复测：默认配置单元 114 项及全部集成测试通过；`native-blitz-cpu-pending` 单元 174 项及全部集成测试通过（`native_frame_schedule` 3/3）。复用准入内核完成六步链：Verus 1 verified/0 errors（忽略失效、忽略尺寸两个反例被拒）、Proptest 31 项、Miri、cargo-fuzz（11.4 万/120.1 万次）、Kani 31 个 harness 全过、Loom。`NativeScene` 增加帧状态字段后，scene-host-close、scene-snapshot-identity、scene-host-identity 三项既有 Verus 证明已按新类型重验通过，证明映射检查回到最新状态。整链报告仍为 `slice_complete=false`，原因是既有的 capture-query 双键 Kani 义务未闭合，与本切片无关。各入口的标脏是否完整由集成测试覆盖，没有证明。

## 2026-10-27：原生路径帧损伤区域（UI5）

根入口 `engine::Engine` 新增 `render_to_surface_with_damage(owner, version, bytes, stride, budget) -> Option<FrameDamage>`：照常整帧绘制并写入宿主 Surface，同时报告相对**本视图上一次产出帧**的损伤，供宿主只向合成器提交变化区域。`FrameDamage` 取值：`Unchanged`（逐像素一致，含复用缓存帧）、`Full`（首帧、视口尺寸变化、宿主 `invalidate_frame` 之后、上一次写入失败之后）、`Rect {x, y, width, height}`（覆盖全部变化像素的最小矩形，逐像素比较得出，不是保守估计）。拒绝（非归属方、过期版本、Surface 不合法、已关闭）返回 None。`invalidate_frame` 现在同时丢弃缓存帧，因此宿主丢失呈现表面后下一次报告必为 Full，而不会因像素相同报 Unchanged。这只是损伤报告：绘制仍是整帧（没有区域重绘），`render_to_backend` 路径不报告损伤，GPU 路径未接入。

新增集成测试 `tests/native_frame_damage.rs`（`native-blitz-cpu-pending`，注入 `JASPER_UI_TEST_FONT`=DejaVu Sans），每次都与测试内独立的逐像素包围盒规格比对：

| 测试 | 断言 | 实测 |
|---|---|---|
| `first_frame_full_then_exact_rect_for_local_update` | 首帧 Full；复用 Unchanged；矩形节点改色后损伤恰为该节点几何；节点加宽后损伤等于规格包围盒且覆盖新增列 | 改色 `Rect{0,0,40,30}`；40→70 px 加宽 `Rect{40,0,30,30}` |
| `repaint_without_visible_change_is_unchanged` | 过期更新被拒但保守标脏，重绘结果逐像素一致，报告 Unchanged | 通过 |
| `extent_change_host_invalidation_and_rejection` | 宿主失效后同像素仍为 Full；缩小与恢复视口均 Full；非归属方、过期版本、短 Surface、已关闭均为 None | 通过 |

构建虚拟机复测：默认配置单元 114 项及全部集成测试通过；`native-blitz-cpu-pending` 单元 174 项及全部集成测试通过（`native_frame_damage` 3/3、`native_frame_schedule` 3/3）。区间扩展内核 `damage_span_extend` 完成六步链（Verus 1 verified/0 errors，两反例被拒）；扫描函数 `frame_damage` 的 Verus 阶段为 N/A（理由与替代证据见 `docs/verification.md` UI-FRAME-DAMAGE-001），其余五步对照独立穷举规格通过，不计为六阶段完成。整链结果见下段。

整链 `scripts/verify-ui.py`：Verus/Z3、Proptest（读边界属性 33 项）、Miri、cargo-fuzz（两个目标各 21 秒，22.9 万/134.6 万次）、Kani（34 个 harness，含 `damage_span_hull` 与 2×2、窄条两个 `frame_damage` 有界 harness）、Loom 全部 scoped 通过；报告仍为 `slice_complete=false`，原因仍是既有 capture-query 双键 Kani 义务未闭合，与本切片无关。证明映射检查保持最新（`NativeScene` 类型未改动）。

## 2026-10-28：原生路径只写损伤区域的宿主 Surface 写入（UI6）

根入口 `engine::Engine` 新增 `render_to_surface_damaged(owner, version, bytes, stride, budget) -> Option<FrameDamage>`：照常整帧绘制，但只把受损像素复制进宿主 Surface——`Full` 写全部有效行，`Rect` 只写矩形覆盖各行内的列片段，`Unchanged` 不写。宿主契约：`bytes` 必须仍持有本视图上一次产出的帧；双缓冲交换或丢失表面后须先调用 `invalidate_frame`（下一次为 `Full`）。Surface 准入（行长、stride、跨度）在绘制之前完成，非归属方、过期版本、Surface 不合法、已关闭均返回 None 且不写任何字节，也不推进损伤基准。写入由 `frame_schedule::write_damaged_frame` 完成：全部准入（Surface 几何、紧凑长度、矩形边界 `damage_rect_admits`）先于第一个字节写入；每行片段经新内核 `surface::write_surface_segment` 写入，该内核只写行内片段、不触碰 padding，失败时目标不变。绘制仍是整帧（没有区域重绘），没有帧节拍，GPU 路径未接入，`render_to_backend` 不报告损伤。

新增集成测试 `tests/native_frame_damage_write.rs`（`native-blitz-cpu-pending`，注入 `JASPER_UI_TEST_FONT`=DejaVu Sans，Surface 每行带 8 字节 padding 哨兵）：

| 测试 | 断言 | 实测 |
|---|---|---|
| `damaged_writes_match_full_writes_and_touch_only_the_rect` | 首帧 Full 后与整帧写入逐像素一致；Unchanged 时预先改写的像素保留；改色后报告 `Rect{0,0,40,30}`，矩形内被预先改写的像素被覆盖、矩形外被改写的像素保留，其余与整帧写入一致；加宽后仍收敛到整帧；全程 padding 不变 | 加宽 `Rect{40,0,30,30}`，复制 3600 / 64000 字节 |
| `rejection_writes_nothing_and_buffer_swap_needs_invalidation` | 非归属方、过期版本、重叠 stride、短 Surface 均为 None 且字节不变；被拒调用不推进损伤基准（之后首次为 Full）；换新缓冲后 `invalidate_frame` 得 Full 且与整帧一致；关闭后 None | 通过 |

构建虚拟机复测：默认配置单元 114 项及全部集成测试通过；`native-blitz-cpu-pending` 单元 174 项及全部集成测试通过（`native_frame_damage_write` 2/2）。`damage_rect_admits` 与 `write_surface_segment` 完成六步链：Verus 分别 1 与 3（含 surface_row/surface_span 依赖）verified/0 errors，接受空矩形/忽略纵向边界、省略复制/忽略 offset 四个反例均被拒。`write_damaged_frame` 的 Verus 阶段为 N/A（理由与替代证据见 `docs/verification.md` UI-DAMAGE-WRITE-001），其余五步对照独立规格通过，不计为六阶段完成；其 Kani harness 为固定几何（至多两像素）× 全部损伤种类与一组固定矩形（含越界、溢出），符号矩形坐标在 2×2 帧下耗尽 CBMC 内存，任意矩形由 Proptest（5×5 以内，含与 `frame_damage` 的端到端收敛）覆盖。

整链 `scripts/verify-ui.py`：Verus/Z3、Proptest（读边界属性 36 项）、Miri、cargo-fuzz（两个目标各 20 秒，8.2 万/102.4 万次）、Kani（38 个 harness）、Loom 全部 scoped 通过；报告仍为 `slice_complete=false`，原因仍是既有 capture-query 双键 Kani 义务未闭合，与本切片无关。证明映射检查为最新；`obligations.json` 中三个文件的元数据哈希与方法计数在整链结束后更新（不改变被验证的源码）。

## 2026-10-28：原生路径帧节拍（UI7）

根入口 `engine::Engine` 新增 `frame_pace(owner, presented, origin, interval, now) -> Option<FramePace>`：在 `needs_frame` 之上，按宿主刷新时钟（`origin + k·interval`，时间单位由宿主决定）给出帧准入决定。`Idle`：没有待绘变化，不必为本视图唤醒（无论时钟走到哪里）；`WaitUntil(t)`：有待绘变化，但本周期已呈现过（`presented` ≥ 本周期节拍）或首个节拍未到，在 t 唤醒；`RenderNow(tick)`：本周期绘制并呈现，之后把 tick 作为 `presented` 传回。每周期至多一帧，宿主停顿错过的周期直接跳过、不追帧。非归属方、已关闭视图、周期为零、下一节拍超出 u64 时为 None。决定由两个纯内核完成：`frame_schedule::frame_tick_at`（包含 now 的周期起点）与 `frame_schedule::frame_pace`。本方法不绘制也不等待；真实垂直同步来源、动画/过渡驱动与 GPU 帧调度不在本切片。

新增集成测试 `tests/native_frame_pace.rs`（`native-blitz-cpu-pending`，注入 `JASPER_UI_TEST_FONT`=DejaVu Sans，周期 16 666 667 ns）：

| 测试 | 断言 | 结果 |
|---|---|---|
| `one_frame_per_period_and_idle_when_clean` | 首节拍前 WaitUntil(origin)；第 3 周期内 RenderNow(该周期节拍)；绘制后无变化为 Idle（100 个周期后仍 Idle）；同周期内改色得 WaitUntil(下一节拍)，到点 RenderNow；绘制后 `needs_frame` 为 false | 通过 |
| `missed_periods_are_skipped_not_caught_up` | 上次呈现在第 2 周期、宿主停顿到第 13 周期中段：恰一次 RenderNow(第 13 周期节拍)，绘制后 Idle | 通过 |
| `foreign_closed_and_invalid_clock_are_rejected` | 非归属方、周期为零、下一节拍溢出均为 None；宿主 `invalidate_frame` 后本周期 RenderNow；关闭后 None | 通过 |

构建虚拟机复测：默认配置单元 114 项及全部集成测试通过；`native-blitz-cpu-pending` 单元 174 项及全部集成测试通过（`native_frame_pace` 3/3）。`frame_tick_at` 与 `frame_pace` 完成六步链：Verus 分别 1 与 3（含依赖）verified/0 errors，四个反例均被拒。Kani 对 64 位符号取模在全宽域下超时（单 harness 超过 270 秒），因此改为有界域：时钟取零或 u64::MAX 附近、now 与 origin 相差有界，另有周期取 u64::MAX 附近的 harness 覆盖溢出分支（四个 harness 分别约 97、1、11、2 秒）；全宽域由 Proptest 与 fuzz 覆盖。


整链 `scripts/verify-ui.py`：Verus/Z3、Proptest（读边界属性 37 项）、Miri、cargo-fuzz（两个目标各 21 秒，10.6 万/138.5 万次）、Kani（42 个 harness）、Loom 全部 scoped 通过；报告仍为 `slice_complete=false`，原因仍是既有 capture-query 双键 Kani 义务未闭合，与本切片无关。证明映射检查为最新；`obligations.json` 中两个文件的元数据哈希与方法计数在整链结束后更新（不改变被验证的源码）。

## 2026-10-28：原生路径场景文字命中与光标几何（UI8）

根入口 `engine::Engine` 新增 `text_offset_at(owner, version, node, x, y) -> Option<usize>` 与 `text_caret(owner, version, node, offset, width) -> Option<Rect>`：文字节点源文本中的 UTF-8 偏移与视图局部逻辑坐标（与 `node_bounds` 同一坐标系）互相映射，几何取自绘制该文字所用的同一份 Blitz/Parley 布局。`text_offset_at` 把点投影到该节点自身的文字布局上，返回最近的光标位置（落在行首之前/行尾之后时夹到两端）；这是放置光标用的投影，不是考虑遮挡的命中测试。`text_caret` 返回偏移之前、宽 `width` 逻辑像素的光标矩形（含文本末尾；软折行处的偏移归下一行）。flex/grid 元素会把文字包进匿名行内根，两种情形都支持。拒绝（返回 None）：非归属方、过期版本、非文字/未知/隐藏节点、已关闭视图、非有限坐标、负或非有限宽度、非字符边界或越界偏移、元素自身有非零滚动（与 `node_bounds` 同一固定版本限制），以及布局文本与源文本不一致（例如连续空白被折叠，此时布局偏移没有对应的源偏移）。布局偏移到源偏移的准入由新内核 `text_geometry::text_offset_admits` 判定。没有选区几何、双向文字验收、输入法预编辑几何，`TextDisplayView` 仍为单字体。

新增集成测试 `tests/native_text_geometry.rs`（`native-blitz-cpu-pending`，注入 `JASPER_UI_TEST_LATIN_FONT`=DejaVu Sans、`JASPER_UI_TEST_CJK_FONT`=Noto Sans CJK）：

| 测试 | 断言 | 实测 |
|---|---|---|
| `caret_and_offset_round_trip_on_one_line` | 「Hello 你好 world」每个字符边界：光标在节点框内、宽 1、同一行严格右移；光标中心点映射回同一偏移；行首前/行尾后夹到 0 与末尾；汉字内部偏移、越界偏移拒绝 | 通过 |
| `caret_columns_enclose_painted_ink` | 「WWW」CPU 帧：全部墨迹列落在首尾光标之间（±1 像素抗锯齿），相邻光标之间每格都有墨迹，墨迹行落在光标行框内 | 通过 |
| `wrapped_carets_follow_lines_and_padding` | 140px 宽、7px 内边距的折行文字：首光标位于内边距边缘；行逐行向下、等距；每个偏移往返一致；框高 = 行数 × 行高 + 内边距 | 至少 3 行，行距一致 |
| `update_moves_geometry_and_rejections_are_explicit` | 文字更新后旧版本拒绝、新文字延长行而原偏移光标不变；各拒绝情形为 None；宽度 0 合法；隐藏、关闭后为 None | 通过 |
| `collapsed_whitespace_has_no_source_mapping` | 「a   b」被折叠后拒绝；「a b」可映射 | 通过 |
| `hidpi_scale_keeps_logical_geometry` | 视口缩放 2.0 后光标横坐标不变（±0.6），高度等于该缩放下单行框高，往返一致 | 单行高 1x 为 24、2x 为 23（上游按设备像素布局，取整不同） |

构建虚拟机复测：默认配置单元 114 项及全部集成测试通过；`native-blitz-cpu-pending`（注入三份字体）单元 174 项及全部集成测试通过（`native_text_geometry` 6/6）。`parley`（=0.12.0，已登记，Cargo.lock 中唯一版本）改为随 `native-blitz-pending` 直接启用，用于调用光标几何 API，没有引入新 crate。`text_offset_admits` 的 Verus 阶段 1 verified/0 errors（忽略长度不符、排除文本末尾两反例被拒）。

整链 `scripts/verify-ui.py`：Verus/Z3、Proptest（读边界属性 38 项）、Miri、cargo-fuzz（两个目标各 21 秒，8.1 万/108.7 万次）、Kani（43 个 harness，`text_offset_admission` 为全域符号输入）、Loom 全部 scoped 通过；报告仍为 `slice_complete=false`，原因仍是既有 capture-query 双键 Kani 义务未闭合，与本切片无关。证明映射检查为最新（`NativeScene` 类型未改动）；`obligations.json` 中 native_scene/facade 的元数据哈希与方法计数在整链结束后更新，并新增 text_geometry 条目。文字布局、Parley 光标/命中几何与坐标换算本身不在证明范围，由上表集成测试覆盖。

## 2026-10-28：原生路径场景文字选区几何（UI9）

根入口 `engine::Engine` 新增 `text_selection(owner, version, node, start, end) -> Option<Vec<Rect>>`：文字节点源文本 UTF-8 区间 [start, end)（start ≤ end）映射为逐行选区矩形，坐标与 `node_bounds`、`text_caret` 相同，几何取自绘制该文字所用的同一份 Blitz/Parley 布局。每个布局行至少一个矩形，高度为该行行框；空区间返回空列表。起点按下游、终点按上游归属，因此恰好止于软折行处的区间不会在下一行产生空框。拒绝（返回 None）沿用 UI8 的全部情形（非归属方、过期版本、非文字/未知/隐藏节点、已关闭视图、元素自身滚动、布局文本与源文本不一致），另加反向区间、任一端非字符边界或越界。区间准入由新内核 `text_geometry::text_range_admits` 判定。只提供几何，不绘制选区高亮；没有双向文字验收与输入法预编辑几何。

新增集成测试 `tests/native_text_selection.rs`（`native-blitz-cpu-pending`，注入 `JASPER_UI_TEST_LATIN_FONT`=DejaVu Sans、`JASPER_UI_TEST_CJK_FONT`=Noto Sans CJK）：

| 测试 | 断言 | 实测 |
|---|---|---|
| `single_line_selection_spans_caret_to_caret` | 「Hello 你好 world」全部字符边界对：空区间无矩形；非空区间恰一个矩形，左右边与起止光标横坐标一致（±0.01），纵向与光标行框一致 | 105 个非空区间全部通过 |
| `wrapped_selection_has_one_rect_per_line` | 140px 宽、7px 内边距的折行文字：全选矩形数 = 光标行数，各行起于内边距边缘、纵向与该行光标一致；跨软折行区间为「首行尾段 + 次行首段」两矩形，端点与光标一致；止于软折行的区间只占一行 | 至少 3 行，通过 |
| `selection_encloses_painted_ink` | 「WWW」CPU 帧：全部墨迹像素落在全选矩形内（±1 像素抗锯齿）；中间字形的选区严格位于全选内且含墨迹 | 通过 |
| `empty_ranges_and_rejections_are_explicit` | 空区间为空列表；反向、汉字内部、越界区间拒绝；非归属方/容器/未知节点拒绝；文字更新后旧版本拒绝、原区间矩形不变、新区间更宽；隐藏、关闭、空白折叠为 None | 通过 |
| `hidpi_scale_keeps_logical_selection` | 视口缩放 2.0 后全选左右边不变（±0.6），高度等于该缩放下单行框高；尾段选区起点与光标一致 | 通过 |

构建虚拟机复测：默认配置单元 114 项及全部集成测试通过；`native-blitz-cpu-pending`（注入三份字体）单元 174 项及全部集成测试通过（`native_text_selection` 5/5、`native_text_geometry` 6/6）。没有引入新 crate。`text_range_admits` 的 Verus 阶段 1 verified/0 errors（接受反向区间、忽略终点边界两反例被拒）。

整链 `scripts/verify-ui.py`：Verus/Z3、Proptest（读边界属性 39 项）、Miri、cargo-fuzz（两个目标各 21 秒，16.7 万/131.5 万次）、Kani（44 个 harness，`text_range_admission` 为全域符号输入，并与两端偏移准入交叉核对）、Loom 全部 scoped 通过；报告仍为 `slice_complete=false`，原因仍是既有 capture-query 双键 Kani 义务未闭合，与本切片无关。`obligations.json` 中 native_scene/facade/text_geometry 的元数据哈希与方法计数在整链启动前更新。Parley 选区几何与坐标换算本身不在证明范围，由上表集成测试覆盖。

## 2026-10-28：原生路径 GPU 无头渲染（UI10）

新增特性 `gpu-pending`（含 `cpu-pending`；`native-blitz-cpu-pending` 门控的集成测试须同时开启该特性）：根原生路径经与 CPU 路径相同的 Blitz 绘制、同一 anyrender 场景接口，画进 Vello hybrid（`vello_gpu`，经 `anyrender_vello_hybrid`）并在 wgpu 上离屏渲染，再读回为紧凑预乘 RGBA8。`GpuRenderer::new_headless()` 不带呈现表面取得适配器（wgpu 默认/环境变量选择）并创建设备，`adapter_info()` 返回适配器名与后端；渲染器、目标纹理与读回缓冲跨帧复用，尺寸变化时重建。根入口 `engine::Engine` 新增 `render_gpu(owner, version, &mut gpu, budget) -> Result<Vec<u8>, GpuFrameError>` 与 `render_gpu_to_surface(owner, version, &mut gpu, bytes, stride, budget)`：非归属方、过期版本、已关闭视图、超额尺寸以及（写 Surface 时）不合法的 Surface 为 `Rejected`，且 Surface 准入先于任何 GPU 工作；无适配器、设备、渲染（wgpu 校验错误域）与读回失败分别报告，**绝不以 CPU 帧顶替**。GPU 帧会丢弃 CPU 帧缓存，下一次 CPU 损伤报告为 `Full`（UI15 起改为共用帧缓存，见下文）；成功后 `needs_frame` 清零。读回布局由新内核 `gpu_render::gpu_readback_layout` 决定（尺寸准入同 CPU 路径，行填充到 256 字节），去填充复用已证明的 `surface::surface_row`。

新增集成测试 `tests/native_gpu_render.rs`（构建虚拟机，适配器 llvmpipe（LLVM 21.1.8）/Vulkan，即软件 Vulkan；注入 Noto Sans CJK）：

| 测试 | 断言 | 实测 |
|---|---|---|
| `gpu_frame_matches_cpu_frame` | 7 个不透明填充/边框采样点 GPU 与 CPU 逐像素一致；差异只出现在标题文字框内、通道差 ≤1、少于 1% 像素；墨迹首末列与 CPU 一致 | 238/64000 像素不同，最大通道差 1；墨迹列 GPU 66 / CPU 66 |
| `gpu_surface_write_preserves_padding_and_rejects_before_gpu_work` | 带 16 字节 padding 的 Surface 逐行等于紧凑帧且 padding 不变；短 Surface、非归属方、过期版本、额度差 1 字节均为 Rejected 且不写 | 通过 |
| `gpu_frame_schedules_and_resizes` | GPU 帧后 `needs_frame` 为 false、下一次 CPU 损伤为 Full；视口改为 160×100 后 GPU 帧长度与背景像素正确；关闭后 Rejected | 通过 |

构建虚拟机复测：默认配置单元 114 项及全部集成测试通过；`native-blitz-cpu-pending`（注入三份字体，拉丁为 DejaVu Sans）单元 174 项及全部集成测试通过；`gpu-pending` 单元 174 项与 `native_gpu_render` 3/3 通过。新增直接依赖 `anyrender_vello_hybrid` =0.11.0、`wgpu_context` =0.10.0、`vello_common` =0.3.0、`rustc-hash` =2.1.3，并把既有可选依赖 `wgpu`、`pollster` 改为精确钉扎（=30.0.1、=0.4.0，锁文件版本不变），均已登记许可。

整链 `scripts/verify-ui.py`：Verus/Z3（`gpu_readback_layout` 2 verified/0 errors，去掉行填充、放宽高度上限两反例被拒）、Proptest（读边界属性 40 项）、Miri、cargo-fuzz（两个目标各 21 秒，11.4 万/172.8 万次）、Kani（45 个 harness，`gpu_readback_layout_admission` 为全域符号输入）、Loom 全部 scoped 通过；报告仍为 `slice_complete=false`，原因仍是既有 capture-query 双键 Kani 义务未闭合，与本切片无关。`obligations.json` 中 native_scene/facade 元数据在整链结束后更新，并新增 gpu_render 条目。wgpu 设备、Vello hybrid 渲染、纹理拷贝与缓冲映射为第三方信任边界，由上表集成测试覆盖，不在证明范围。未做：窗口/表面呈现、device-lost 恢复、GPU 损伤与局部重绘、真实硬件 GPU 验收、Canvas。

## 2026-10-28：原生路径无障碍树快照（UI11）

根入口 `engine::Engine` 新增 `accessibility_tree(owner, version) -> Option<AccessibilityTree>`，作为系统无障碍桥的第一步：引擎给出宿主无关的树，平台桥（AccessKit、AT-SPI、UIA 或系统无障碍服务）据此构建各自的树。`nodes` 为可见节点，按源顺序深度优先、根在首位；每个节点含 Framework 原样语义（role、label、description、value、disabled，不从后端 DOM 或显示文字重算）、与 `node_bounds` 相同的视图局部几何、场景内焦点标记、动作 `AccessActions { focus, activate }` 与可见子节点。隐藏（semantics.hidden）或未显示（display: none）的节点连同子树剔除，根本身不可见时为空树；焦点节点不可见时 `focused` 为 None。非归属方、过期版本、已关闭视图返回 None。遍历以节点数为上界。动作由新内核 `access_tree::access_actions` 决定（启用且可聚焦 → focus，启用的按钮 → activate），与 `focus`/`activate` 入口的准入规则一致。本切片不调用任何平台无障碍 API，没有 AccessKit 依赖、屏幕坐标换算、增量更新事件、平台动作路由或文本无障碍接口。

新增集成测试 `tests/native_accessibility_tree.rs`（`native-blitz-pending`，注入 `JASPER_UI_TEST_FONT`=DejaVu Sans）：

| 测试 | 断言 | 实测 |
|---|---|---|
| `visible_nodes_carry_semantics_geometry_and_order` | 顺序为源顺序深度优先，隐藏组与 display:none 组（含子树）剔除；语义原样；每个节点几何等于 `node_bounds`；动作按规则；初始无焦点 | 节点 [1,2,3,4,9,10]；根 0,0,240×200；按钮 80×30 |
| `reported_actions_match_engine_admission` | 对树中每个节点实际调用 `activate`/`focus`，结果与报告的动作逐一相等；聚焦后树中恰一个焦点节点；被剔除节点两种动作均被拒 | 通过 |
| `updates_reshape_the_tree_and_hiding_drops_focus` | 语义更新取消隐藏后子树出现、改名生效、旧版本 None；隐藏焦点按钮后该节点与焦点一并消失 | [1,2,3,4,5,6,9,10] → [1,2,5,6,9,10] |
| `hidden_root_foreign_and_closed_are_explicit` | 非归属方、未来版本 None；根隐藏为空树且无焦点；关闭后 None | 通过 |

构建虚拟机复测：默认配置单元 114 项及全部集成测试通过；`native-blitz-cpu-pending`（注入三份字体）单元 174 项及全部集成测试通过（`native_accessibility_tree` 4/4）。`access_actions` 完成六步链：Verus 2 verified/0 errors（含类型；禁用仍可聚焦、任意角色可激活两反例被拒）。整链 `scripts/verify-ui.py`：Verus/Z3、Proptest（读边界属性 41 项）、Miri、cargo-fuzz（两个目标各 21 秒，13.0 万/206.9 万次）、Kani（46 个 harness）、Loom 全部 scoped 通过；报告仍为 `slice_complete=false`，原因仍是既有 capture-query 双键 Kani 义务未闭合，与本切片无关。可见性剔除、遍历与焦点投影由集成测试覆盖，没有证明。证明映射检查为最新；`obligations.json` 新增 `access_tree.rs` 条目，`native_scene.rs`/`facade.rs` 的元数据哈希与方法计数在整链结束后更新（不改变被验证的源码）。

## 2026-10-28：原生路径无障碍增量更新（UI12）

平台无障碍桥不必每个版本重建整棵树：`AccessibilityTree::update_since(&previous)` 给出同一视图从较旧（或同版本）快照到较新快照的 `AccessibilityUpdate { from_version, scene_version, nodes, removed, root, focused, focus_changed }`，形如维护镜像的桥（AccessKit TreeUpdate 风格）所需。`nodes` 为新增或内容有变的节点（完整内容、按新树顺序），可见子节点变化的父节点、焦点标记翻转的节点、几何变化的节点都计为变更；`removed` 为离开树的节点（按旧树顺序）；`previous` 更新时为 None。`AccessibilityTree::apply(&update)` 在镜像上回放并从根重建深度优先源顺序，更新基于其他版本、移除不存在节点、子节点缺失/重复/不可达、焦点不在树中时为 None，镜像不变。每个节点的差别由新内核 `access_tree::access_change` 分类（Absent/Added/Removed/Unchanged/Changed）。本切片仍不接 AccessKit crate 或平台 API，也不提供平台动作路由与文本无障碍接口；不检测把不同视图的树混用。

新增集成测试 `tests/native_accessibility_update.rs`（`native-blitz-pending`，注入 `JASPER_UI_TEST_FONT`=DejaVu Sans）。每一步都核对：回放结果等于新树，且 `nodes`/`removed` 恰为内容不同/消失的节点。

| 测试 | 断言 | 实测 |
|---|---|---|
| `same_version_is_an_empty_update` | 同一快照之差为空更新；从空镜像出发更新携带整棵树 | 空；全量 [1,2,3,4,5,6] |
| `engine_updates_replay_through_incremental_updates` | 真实引擎依次：改状态值、移动焦点、隐藏分组、插入节点并删除焦点按钮、隐藏根；逐步回放链与一次跨版本回放都等于最新树 | 改值仅 [6]；聚焦仅 [3]、焦点变化；隐藏分组 变更 [1]、移除 [4,5]；删除焦点按钮后焦点 None；隐藏根全部移除、根 None |
| `reversed_mismatched_and_inconsistent_updates_are_rejected` | 旧于 previous、镜像版本不符、移除不存在节点、缺失子节点、重复子节点、悬空子节点、不可达节点、焦点不在树中均为 None；失败后镜像仍可正确回放 | 通过 |

构建虚拟机复测：默认配置单元 114 项及全部集成测试通过；`native-blitz-cpu-pending`（注入三份字体）单元 174 项及全部集成测试通过（`native_accessibility_update` 3/3、`native_accessibility_tree` 4/4）。`access_change` 完成六步链：Verus 2 verified/0 errors（含类型；把移除判为新增、忽略内容相等两反例被拒）。整链 `scripts/verify-ui.py`：Verus/Z3、Proptest（读边界属性 42 项）、Miri、cargo-fuzz（两个目标各 21 秒，19.9 万/170.4 万次）、Kani（47 个 harness）、Loom 全部 scoped 通过；报告仍为 `slice_complete=false`，原因仍是既有 capture-query 双键 Kani 义务未闭合，与本切片无关。树差、回放与顺序重建由集成测试覆盖，没有证明。证明映射检查为最新；`obligations.json` 中 `access_tree.rs`/`facade.rs` 的元数据哈希与计数在整链结束后更新（不改变被验证的源码）。

## 2026-10-28：原生路径 AccessKit 桥（UI13）

新特性 `accesskit-pending`（= `native-blitz-pending` + `accesskit =0.25.2`，经 `native::host::accesskit` 重导出）把宿主无关无障碍树接到 AccessKit。`AccessibilityTree::to_accesskit(transform)` 给出完整 `TreeUpdate`（AccessKit 根树、TreeInfo 含根与工具包名，无焦点时焦点回落到根；`transform` 只设在根上，恒等变换省略）；`AccessibilityUpdate::to_accesskit(transform)` 只携带新增或变更节点，移除的节点经其变更的父节点由 AccessKit 丢弃；空树无 AccessKit 形式（None）。角色按 `accesskit_role` 映射，无 value 的 Text 把 label 作为 AccessKit value。`Engine::accesskit_action` 路由宿主平台适配器送来的 `ActionRequest`：目标须在当前无障碍树中且提供该动作（Focus → focus，Click → activate），由新内核 `access_kit::access_request_admits` 准入；其他动作、其他树、携带动作数据、过期版本、非归属方、隐藏子树内节点一律拒绝且无副作用。平台适配器（accesskit_winit / AT-SPI / UIA / 系统无障碍服务）、窗口事件循环与文本无障碍接口（TextRun/选区）不在本切片。测试另用 `accesskit_consumer =0.39.2`（仅开发依赖）作参考消费端。

新增集成测试 `tests/native_accesskit_bridge.rs`（`accesskit-pending`，注入 `JASPER_UI_TEST_FONT`=DejaVu Sans）：

| 测试 | 断言 | 实测 |
|---|---|---|
| `full_tree_converts_semantics_geometry_actions_and_focus` | 完整转换：节点顺序、子节点、动作、禁用、几何与无障碍树逐节点一致；角色/label/value 映射；变换只在根上且消费端把 2 倍缩放合成进每个包围盒；空树 None | 节点 [1..7]，焦点回落根 1；2x 包围盒精确 |
| `incremental_updates_keep_the_consumer_mirror_equal_to_the_newest_tree` | 真实引擎依次改值、移焦点、隐藏分组、插入并删除焦点按钮、重新显示分组：每步增量更新只含引擎报告的变更节点，参考消费端镜像（可达节点、根、焦点）始终等于最新树的完整转换；更新到空树为 None | 改值/聚焦/隐藏分组各仅 1 个节点；焦点移到 3、删除后回落 1 |
| `action_requests_reach_focus_and_activate_only_when_offered` | 逐节点 Click/Focus 恰按树中报告的动作准入并到达 focus/activate；禁用按钮点击与聚焦、文字点击、不支持的动作、未知节点、非归属方、过期版本、其他树、携带数据、隐藏子树十类拒绝且无事件、焦点不变 | 通过 |

构建虚拟机复测：默认配置单元 114 项及全部集成测试通过；`native-blitz-cpu-pending`（注入三份字体）单元 174 项及全部集成测试通过；`accesskit-pending` 单元 163 项及全部集成测试通过（`native_accesskit_bridge` 3/3）。`access_request_admits` 完成六步链：Verus 2 verified/0 errors（含类型；用 activate 判 Focus、准入不支持请求两反例被拒）。整链 `scripts/verify-ui.py`：Verus/Z3、Proptest（读边界属性 43 项）、Miri、cargo-fuzz（两个目标各 21 秒，10.4 万/103.1 万次）、Kani（48 个 harness）、Loom 全部 scoped 通过；报告仍为 `slice_complete=false`，原因仍是既有 capture-query 双键 Kani 义务未闭合，与本切片无关。转换与路由由集成测试覆盖，没有证明。证明映射检查为最新；`obligations.json` 新增 `access_kit.rs` 条目，`facade.rs` 的元数据哈希与方法计数在整链结束后更新（不改变被验证的源码）。

## 2026-10-28：原生路径 GPU 帧损伤与只写损伤区域（UI15）

GPU 路径与 CPU 路径改为共用同一帧缓存与损伤基准（此前 GPU 帧会丢弃 CPU 缓存）。`render_gpu` 在视图未失效、尺寸与预算一致时经已证明的 `frame_reuse_admits` 复用最近产出的帧，**不做 GPU 工作**；否则 GPU 绘制读回后经 `frame_damage` 与上一帧（无论由哪条路径绘制）逐像素比较，再入缓存。设备/渲染/读回失败保留原基准、视图保持待绘。根入口 `engine::Engine` 新增 `render_gpu_to_surface_with_damage` 与 `render_gpu_to_surface_damaged`（返回 `Result<FrameDamage, GpuFrameError>`），语义对应 CPU 同名方法；`_damaged` 的 Surface 准入先于 GPU 工作，经已证明的 `write_damaged_frame` 只写受损像素，被拒或写入失败不写任何字节（写入失败丢弃缓存，下一次为 Full）。契约变化：未变化视图上 `render_gpu` 会返回最近产出的 CPU 帧，需强制 GPU 重绘时先 `invalidate_frame`（`tests/native_gpu_render.rs` 相应调整）。绘制仍为整帧；没有区域重绘、窗口呈现与 device-lost 恢复。

新增集成测试 `tests/native_gpu_frame_damage.rs`（`gpu-pending`，注入 `JASPER_UI_TEST_FONT`=DejaVu Sans；构建虚拟机 llvmpipe/Vulkan），损伤均与测试内独立的逐像素包围盒规格比对：

| 测试 | 断言 | 实测 |
|---|---|---|
| `gpu_damage_is_exact_and_damaged_writes_touch_only_the_rect` | 首帧 Full；干净视图 `render_gpu` 复用已产出帧；Unchanged 不写；改色损伤等于独立包围盒，矩形内被预先改写像素被覆盖、矩形外保留，padding 不变；加宽后损伤覆盖新增列且整帧写入收敛 | 改色 `Rect{0,0,40,30}` |
| `gpu_and_cpu_frames_share_one_damage_baseline` | CPU 帧 → 场景变化 → GPU 帧的损伤等于两帧包围盒；GPU 帧 → 变化 → CPU 帧同理；均覆盖改色矩形 | 两次均 `Rect{0,0,44,46}`（含两路径文字抗锯齿差异） |
| `gpu_rejection_writes_nothing_and_keeps_the_baseline` | 非归属方、过期版本、重叠 stride、短 Surface 均 `Rejected` 且字节不变；拒绝不移动基准（随后 Unchanged）；换缓冲后 `invalidate_frame` 得 Full 且与原帧一致；关闭后 `Rejected` | 通过 |

构建虚拟机复测：默认配置 25 个测试套件共 140 项通过；`native-blitz-cpu-pending,gpu-pending`（注入三份字体）25 个套件共 247 项通过（单元 174 项，`native_gpu_frame_damage` 3/3、`native_gpu_render` 3/3，GPU 与 CPU 仍为 238/64000 像素差、最大通道差 1）。本切片未新增内核：所改代码是 GPU 渲染与已证明内核之间的编排，六步链各阶段对其为 N/A（理由与替代证据见 `docs/verification.md` UI-GPU-DAMAGE-001），不计为六阶段完成。整链 `scripts/verify-ui.py`：Verus/Z3、Proptest（读边界属性 43 项）、Miri、cargo-fuzz（两个目标各 21 秒，18.9 万/144.5 万次）、Kani（48 个 harness）、Loom 全部 scoped 通过；报告仍为 `slice_complete=false`，原因仍是既有 capture-query 双键 Kani 义务未闭合，与本切片无关。

## 2026-10-29：TextDisplayView 宿主有序多字体回退栈（UI16）

根入口 `TextDisplayView` 新增 `with_fonts(owner, context, fonts, text, width, height) -> Option<Self>`，与 `Engine::with_fonts` 一致：宿主按序注入多个字体文件（如先拉丁、后中日韩），Parley 按宿主顺序回退。准入复用已证明的 `native_fonts::injected_font_stack`（`font_stack_count_admits`/`font_order_admits`，UI-FONT-ORDER-001）：空栈、超过 `MAX_INJECTED_FONTS`、任一文件不登记任何字体族时返回 None，在构建任何文档之前拒绝，不静默跳过。单字体 `new` 行为不变（内部改为与多字体共用同一视图树构造）。测量、命中（`hit`）与绘制共用同一份 Parley 布局。

新增集成测试 `tests/native_text_display_fonts.rs`（`native-blitz-cpu-pending`，注入 `JASPER_UI_TEST_LATIN_FONT`=DejaVu Sans 与 `JASPER_UI_TEST_CJK_FONT`=Noto Sans CJK）：

| 测试 | 断言 | 实测 |
|---|---|---|
| `fallback_follows_host_order` | 拉丁文字在 [拉丁, CJK] 下帧与只注入拉丁一致，在 [CJK, 拉丁] 下与只注入 CJK 一致，两种顺序帧不同；汉字在 [拉丁, CJK] 下回退到 CJK（帧等于只注入 CJK），与只注入拉丁的缺字框帧不同且墨迹更宽 | 汉字墨迹列 62 vs 缺字框 35 |
| `stack_admission_rejects_explicitly` | 空栈、非字体字节、[拉丁, 非字体]、[非字体, CJK]、超限栈均 None；满额重复同一文件帧与单文件一致；单字体 `new` 帧等于单文件栈；非归属方渲染被拒 | 通过 |
| `hit_uses_the_fallback_layout` | 逐列扫描一行：命中偏移均为 UTF-8 边界、自左向右不减、覆盖 0 到文本末尾；越过拉丁前缀的位置两种字体栈一致，汉字段终点随字形宽度变化 | 通过 |

构建虚拟机复测：默认配置 140 项通过；`native-blitz-cpu-pending`（注入三份字体）26 个套件共 244 项通过（单元 174 项，`native_text_display_fonts` 3/3）。本切片未新增内核：所改代码是上游 Blitz/fontique 构造之上的编排，六步链各阶段对其为 N/A（理由与替代证据见 `docs/verification.md` UI-TEXT-DISPLAY-FONTS-001），不计为六阶段完成。整链 `scripts/verify-ui.py`：Verus/Z3、Proptest（读边界属性 43 项）、Miri、cargo-fuzz（两个目标各 21 秒，18.5 万/201.5 万次）、Kani（48 个 harness）、Loom 全部 scoped 通过；报告仍为 `slice_complete=false`，原因仍是既有 capture-query 双键 Kani 义务未闭合，与本切片无关。

未完成：字重/斜体/字体族选择、按钮的多字体公开入口、双向文字验收。根入口仍标记 not yet accepted。

## 2026-10-29：场景字重与斜体选择（UI17）

`style::Style` 新增 `font_weight: u16`（CSS 数值字重，默认 400）与 `font_style: FontStyle::{Normal, Italic}`（默认 Normal），构造器 `Style::font(weight, style)`。根原生场景路径把两者映射为 CSS `font-weight` / `font-style`，由 Parley/fontique 在宿主注入的同族字面中匹配（如 DejaVu Sans Regular、Bold、Oblique 分别作为独立文件注入同一字体栈）。新内核 `native_fonts::font_weight_admits`：字重恰在 1..=1000 时准入，越界的场景导入与 `UpdateStyle` 补丁被拒，不钳制。LegacyEngine 不实现字重与倾斜，作为对照引擎其绘制不受这两个字段影响。

新增集成测试 `tests/native_font_weight_style.rs`（`native-blitz-cpu-pending`，注入 `JASPER_UI_TEST_LATIN_FONT`=DejaVu Sans、`JASPER_UI_TEST_BOLD_FONT`=DejaVu Sans Bold、`JASPER_UI_TEST_ITALIC_FONT`=DejaVu Sans Oblique）：

| 测试 | 断言 | 实测 |
|---|---|---|
| `weight_selects_the_matching_injected_face` | 两种注入顺序下，700 帧均等于只注入 Bold、400 帧均等于只注入 Regular（按字重匹配、与顺序无关）；700 与 400 帧不同且粗体墨迹更多；默认样式帧等于显式 400/Normal | 墨迹像素 1397 vs 1051 |
| `italic_selects_the_oblique_injected_face` | 两种注入顺序下，Italic 帧等于只注入 Oblique、Normal 帧等于只注入 Regular；Italic 与 Normal 帧不同 | 通过 |
| `out_of_range_weight_is_rejected_and_updates_repaint` | 字重 0、1001、65535 导入被拒，1 与 1000 准入；越界 `UpdateStyle` 被拒且版本与帧不变；改为 700 的补丁标脏，重绘帧等于新导入的 700 场景 | 通过 |

构建虚拟机复测：默认配置 140 项通过；`native-blitz-cpu-pending`（注入字体）共 247 项通过（`native_font_weight_style` 3/3）；`native-blitz-cpu-pending,gpu-pending,accesskit-pending` 测试目标编译通过。新内核六步链：Verus/Z3 1 verified/0 errors（拒绝最小值、排除最大值两反例被拒），Proptest、Miri、cargo-fuzz、Kani、Loom 均覆盖。整链 `scripts/verify-ui.py`：Verus/Z3、Proptest（读边界属性 44 项）、Miri、cargo-fuzz（两个目标各 21 秒，13.5 万/129.6 万次）、Kani（49 个 harness）、Loom 全部 scoped 通过；报告仍为 `slice_complete=false`，原因仍是既有 capture-query 双键 Kani 义务未闭合，与本切片无关。CSS 映射与 Parley 字面匹配属第三方信任边界，由集成测试覆盖，不在证明范围。

未完成：字体族名选择（font-family）、合成粗体/斜体（宿主只注入常规字面时）的验收、按钮的多字体公开入口、双向文字验收。根入口仍标记 not yet accepted。

## 2026-10-29：场景字体族名选择（UI18）

`style::Style` 新增 `font_family: Option<String>`（默认 None，沿用宿主注入顺序），构造器 `Style::family(name)`。根原生场景路径把它映射为 CSS `font-family: "<name>", sans-serif`（None 时为 `sans-serif`，每个节点显式声明、不继承，与颜色、字号一致），由 Parley/fontique 按名选择；所选族缺少的字形经通用族按宿主注入顺序回退。族名须与本视图注入字体栈登记的族名完全一致（区分大小写）：未登记的族名在场景导入、`Insert` 与 `UpdateStyle` 补丁中被拒，不静默替换为其他字体。新内核 `native_fonts::font_family_admits`：族名 1..=128 字节、不含控制字节/DEL/双引号/反斜杠时准入，使其可原样放入 CSS 引号串；不合规即拒绝，不转义。LegacyEngine 不实现字体族选择。

新增集成测试 `tests/native_font_family.rs`（`native-blitz-cpu-pending`，注入 `JASPER_UI_TEST_LATIN_FONT`=DejaVu Sans 与 `JASPER_UI_TEST_CJK_FONT`=Noto Sans CJK 字体集）：

| 测试 | 断言 | 实测 |
|---|---|---|
| `family_is_selected_by_name_not_by_injection_order` | 两种注入顺序下，`DejaVu Sans`、`Noto Sans CJK SC`、`Noto Sans Mono CJK SC`（同一字体集文件内的两个族）帧均等于只注入对应文件时的帧；未声明族等于先注入的族；三者两两不同 | 通过 |
| `glyphs_missing_from_the_family_fall_back_in_host_order` | 声明 `DejaVu Sans` 的汉字在 [CJK, 拉丁] 下回退，帧等于只注入 CJK 的默认帧，且不同于只注入拉丁时的缺字框帧；在 [拉丁, CJK] 下等于未声明族 | 通过 |
| `unregistered_or_unsafe_names_are_rejected_and_updates_repaint` | 前缀名、大小写不同、尾随空格、空名、含双引号/反斜杠/换行、129 字节均导入被拒；只注入拉丁时 CJK 族名被拒；未登记/不安全族名的 `UpdateStyle` 与 `Insert` 被拒且帧不变；改为 Mono 族的补丁标脏、重绘等于新导入；清除族名回到原帧 | 通过 |

构建虚拟机复测：默认配置 28 个测试套件共 140 项通过；`native-blitz-cpu-pending`（注入字体）28 个套件共 250 项通过、0 失败（`native_font_family` 3/3）；`native-blitz-cpu-pending,gpu-pending,accesskit-pending` 测试目标编译通过。新内核六步链：Verus/Z3 3 verified/0 errors（准入禁用字节、准入超长两反例被拒），Proptest、Miri、cargo-fuzz、Kani、Loom 均覆盖。`NativeScene` 新增登记族名字段后重验 scene-host-close（8/0）、scene-snapshot-identity（7/0）、scene-host-identity（9/0），映射检查全部 current。整链 `scripts/verify-ui.py`：Verus/Z3、Proptest（读边界属性 45 项）、Miri、cargo-fuzz（两个目标各 21 秒，15.9 万/122.8 万次）、Kani（50 个 harness）、Loom 全部 scoped 通过；报告仍为 `slice_complete=false`，原因仍是既有 capture-query 双键 Kani 义务未闭合，与本切片无关。登记族名查找、CSS 映射与 Parley 按名选择/回退为编排与第三方信任边界，由集成测试覆盖，不在证明范围。

未完成：合成粗体/斜体（宿主只注入常规字面时）的验收、按钮的多字体公开入口、双向文字验收、TextDisplayView 的族名选择。根入口仍标记 not yet accepted。

## 2026-10-29：只注入常规字面时的字体合成（UI19，刻画）

宿主只注入常规字面（DejaVu Sans Regular）而场景声明斜体或粗字重时，固定版 Blitz 绘制器的行为经实测刻画如下，**均不计为已交付**：

- **合成斜体方向错误**：Parley 为该字形串报告倾斜合成，绘制器据此施加字形斜切，但在 y 向下的场景坐标中符号取反，画出的是**反斜体**（左倾）。单个 "I" 竖笔画顶行相对底行左缘：直立 0 列、注入真斜体字面 +4 列（右倾）、合成斜体 −5 列。
- **合成粗体缺失**：绘制器不消费 Parley 的加粗合成（只有对全部文字生效的全局加粗特性，本仓库未开启），字重 600/700/900 的帧与 400 逐字节相同。

新增 `tests/native_font_synthesis.rs`（`native-blitz-cpu-pending`，注入 `JASPER_UI_TEST_LATIN_FONT` 与 `JASPER_UI_TEST_ITALIC_FONT`）：

| 测试 | 断言 | 实测 |
|---|---|---|
| `synthetic_italic_from_the_pinned_painter_leans_backwards` | 合成斜体帧不同于直立帧、墨迹量相差不超过 10%；直立竖笔画垂直、真斜体字面右倾 ≥3 列、合成斜体单调左倾 ≥3 列；改为 Italic 的样式补丁重绘等于新导入 | 墨迹 1397 vs 1383；倾斜 0 / +4 / −5 |
| `bold_without_a_bold_face_draws_the_regular_face` | 字重 600/700/900 帧等于 400；700 斜体等于 400 斜体 | 通过 |

两项断言在上游行为改变时会失败，届时须更新本节。宿主若需要正确的斜体与粗体，目前须注入对应字面（UI17 已验收按字重/倾斜匹配注入字面）。

同轮补欠账：既有 `style::Color::new/rgb/with_alpha` 移入独立标记区（语义不变）并完成六步链（UI-COLOR-CONSTRUCT-001，Verus 4 verified/0 errors，两反例被拒）。构建虚拟机复测：默认配置 29 个套件共 140 项通过；`native-blitz-cpu-pending`（注入字体）29 个套件共 252 项通过、0 失败（`native_font_synthesis` 2/2）。整链 `scripts/verify-ui.py`：Verus/Z3、Proptest（读边界属性 46 项）、Miri、cargo-fuzz（14.7 万/158.5 万次）、Kani（51 个 harness）、Loom 全部 scoped 通过；报告仍为 `slice_complete=false`，原因仍是既有 capture-query 双键 Kani 义务未闭合。

未完成：合成斜体方向修正与合成粗体（需在绘制入口按合成信息修正，或在只有常规字面时明确拒绝）、按钮的多字体公开入口、双向文字验收、TextDisplayView 的族名选择。根入口仍标记 not yet accepted。

## 2026-10-29：合成斜体方向修正（UI20）

UI19 刻画的反斜体已修正。新增 `src/native_adapter/glyph_skew.rs`：转发绘制器 `UprightSyntheticItalic` 包装 anyrender `PaintScene`，所有调用原样转发，只在 `draw_glyphs` 中把**恰为纯水平斜切**的字形变换（a = d = 1，b = e = f = ±0，即 Blitz 由 Parley 合成倾斜角生成的 `Affine::skew(tan, 0)`）的斜切符号取反；其他变换（含平移、缩放、NaN 系数）不变。根原生 CPU 路径、GPU 路径与 TextDisplayView 的绘制入口均经此包装；录制场景片段走默认 `append_scene`，同样修正。新增直接依赖 `peniko =0.6.1`（与锁定版本一致，仅为绘制调用类型，许可证已登记）。

判定内核 `upright_skew_x` 以 `f64::to_bits` 位模式工作（UI-GLYPH-SKEW-001），Verus 4 verified/0 errors，「不取反」「平移斜切也取反」两反例被拒；Proptest、Miri、cargo-fuzz、Kani、Loom 以浮点域独立规格对照。

`tests/native_font_synthesis.rs` 更新：

| 测试 | 断言 | 实测 |
|---|---|---|
| `synthetic_italic_leans_right_like_an_italic_face` | 合成斜体帧不同于直立帧、墨迹量相差不超过 10%；直立竖笔画垂直、真斜体字面右倾 ≥3 列、合成斜体单调右倾 ≥3 列；改为 Italic 的样式补丁重绘等于新导入 | 墨迹 1388 vs 1383；倾斜 直立 0 / 真斜体 +4 / 合成 +5（修正前 −5） |
| `bold_without_a_bold_face_draws_the_regular_face` | 字重 600/700/900 帧等于 400；700 斜体等于 400 斜体 | 通过（合成粗体仍缺失，未改） |
| `gpu_synthetic_italic_leans_right_too`（`gpu-pending`） | GPU 路径合成竖笔画同样右倾 ≥3 列 | +5（构建虚拟机 llvmpipe） |

构建虚拟机复测：默认配置 140 项通过；`native-blitz-cpu-pending`（注入字体）253 项通过；`native-blitz-cpu-pending,gpu-pending` 260 项通过；`accesskit-pending` 200 项通过；均 0 失败。整链 `scripts/verify-ui.py`：Verus/Z3、Proptest（读边界属性 47 项）、Miri、cargo-fuzz（16.0 万/172.8 万次）、Kani（52 个 harness）、Loom 全部 scoped 通过；报告仍为 `slice_complete=false`，原因仍是既有 capture-query 双键 Kani 义务未闭合，与本切片无关。转发绘制器与绘制路径接入为第三方 PaintScene 之上的编排，由集成测试覆盖，不在证明范围。

未完成：合成粗体（绘制器只有全局加粗开关、不传递逐字形串的加粗合成信息；需另得合成信息，或在只有常规字面时明确拒绝）、按钮的多字体公开入口、双向文字验收、TextDisplayView 的族名选择。根入口仍标记 not yet accepted。

## 2026-11-02：真实 Map 子树遍历的后端能力诊断（第 1 项仍阻塞）

在固定 Kani 0.68.0 / CBMC 6.11.0、3G 内存、零 swap、128 tasks、单 job、180 秒的 UI VM 限额内，继续尝试解决 `source_subtree_contains` 的真实两节点 `BTreeMap` 验证缺口。未改生产函数、原符号输入域、独立 oracle、安全检查或 Map 类型。

| 配置 | 结果 |
|---|---|
| `unwind=2` + Bitwuzla | 22.77 秒得到结论，但 Map 与生产循环的展开断言失败；0 个 harness 成功，未计通过 |
| `unwind=4` + Bitwuzla | 默认对象编码在 SMT2 `QF_AUFBV` 转换阶段报告 3G 内存不足 |
| `unwind=4` + Bitwuzla `object-bits=8/9/10` | 分别因超过 256/512/1024 个被寻址对象的明确容量错误失败 |
| `unwind=4` + Bitwuzla `object-bits=11` | 85.86 秒到达结论，但展开断言失败，说明 `unwind=4` 并不充分 |
| `unwind=5` + Bitwuzla `object-bits=11` | 59.44 秒完成符号执行，9309 个残留 VCC 后在 SSA 转换超时，退出 124 |
| `unwind=8` + Bitwuzla `object-bits=11` | 180 秒限额内未完成，退出 124 |
| `unwind=4` + Z3/CVC5 | CBMC SMT 转换在 `map::at` / status 6 失败 |
| `unwind=4` + Bitwuzla `arrays-uf-always` / `full-slice` | 仍在转换阶段内存不足；`full-slice` 反使剩余 VCC 增至 6944 |

（2026-11-03 更正：本表及扩容报告中记为“展开断言失败”的 Bitwuzla 结果实为求解器 `ERROR`，虚拟机中没有 bitwuzla 可执行文件；相关展开界结论撤回，见文末同日条目。）

此外，Kissat 在 `unwind=2` 的 SAT 阶段超时。上述运行保留为后端能力负控，明确证明旧超时不能解释为边界充分，也没有资格据 object 参数或较小展开界宣称通过。完整记录见 `evidence/native-subtree-solver-capability/report.json`。

第 1 项仍保持未勾选：真实 Map 的 Kani 义务未完成，`apply_update → apply_patches` 的完整整帧变更证明也仍缺；后续需要新的验证器/后端编码，或保持真实 Map、完整性质、原始域与全部断言的非缩减分解。

## 2026-11-02：场景整帧候选事务（第 2 项工程缺口）

`NativeScene` 现保留一份 `Scene` 来源快照。`apply_patches` 先准入所有权/版本/开放状态，再在来源副本上调用原子 `Scene::apply_batch_at`，随后从终态构建完整的候选原生视图；两个阶段均成功后才接管候选，因此来源应用或原生构造失败都不会部分修改活动树、版本、焦点、视口、滚动或最近帧。成功后恢复焦点、视口滚动、最近指针悬停、原生按下节点身份和上一帧缓存。

该实现也移除了“语义更新不得改变原生 Button 标签”的旧限制：Button→Container 等合法更新由候选重建完成，并可在同一 NodeId 上保留仍合格的焦点。新增 `tests/native_frame_transaction.rs`：

- 合法来源批次中的零字号样式使候选原生构造失败；版本、节点数、焦点、node bounds 与全部像素保持，随后以原 expected version 提交合法帧成功。
- Button→Container 语义更新成功，像素与同终态新导入逐字节一致，角色/标签和焦点均正确。

受限 VM 中原生 CPU 库 175 项、`native_blitz_engine` 2 项、`native_cpu_acceptance` 4 项、`native_frame_differential` 1 项及新增事务 2 项全部通过。该事务编排尚缺完整 Verus/六阶段实现级证明，因此第 2 项仍不勾选。

新增字段后重验受影响的提取依赖：scene-host-close 8/0、scene-snapshot-identity 7/0、scene-host-identity 9/0、native-component-version 26/0、text-display-close 30/0、text-display-version 29/0、native-host-context 27/0，负控均按后置条件失败；semantic-role-tag 3/0 仍保留，但其范围已改为“仅历史兼容谓词”，事务路径不再消费该标签保持限制。`test-blitz-proof-mapping.py` 当前映射与负控全部通过。全量 CPU 回归除“无同族 sans oblique 字体夹具”的既有测试外全部通过；该跳过不记实现失败，也不记六阶段通过。

## 2026-11-02：合成粗体替身完成限定验收

注入字体资源层对每个物理 Blob 按（族、样式、宽度）计算组内最大字重。仅当 Blob 中每个字面都是本组最大且字重不超过 500、且没有可变字重轴时，才以全新 Blob id、字重 700 再登记替身。CSS 匹配只在粗体请求时选取替身；转发绘制器按该 Blob id 启用固定版 Blitz 的相对轮廓加粗量，不改其他字体或其他绘制调用。真实 Bold 字面存在时优先于常规字面替身。

新增 `UI-FONT-BOLD-ALIAS-001`：实际生产判定完成 Verus+Z3 3 verified/0 errors，空表与拒绝合格 Blob 两个反例均拒绝；Proptest 256 例、Miri（严格 provenance）、cargo-fuzz 200000 次、Kani 185 checks（0 失败）通过；纯同步路径记录 Loom 不适用。整族查询、Blob 注册、Parley 匹配与 Vello 光栅为明确第三方 TCB。证据：`evidence/native-font-bold-alias-chain/report.json`。

场景 CPU 3 项和 GPU 2 项字体合成测试通过：常规字面单独注入时 600/700/900 使用同一替身并增加墨迹，700 斜体同时保持右倾与加粗，真实 Bold 注入时不被替身替代。第 3 项整体仍未完成：TextDisplayView 尚无字重更新字段，窗口 GPU Surface 的真实 display 呈现和 device-loss/能力矩阵仍需后续验收。

## 2026-11-02：宿主 GPU 设备与目标视图直连

`GpuRenderer::from_wgpu(instance, adapter, device, queue)` 允许宿主注入已经取得的 wgpu 设备，不触发引擎侧设备获取。`render_gpu_to_host_view` 在 owner/version/open/extent/budget 准入后直接把 Blitz/Vello hybrid 帧绘制进宿主 `TextureView`；宿主保留窗口 Surface、swapchain texture 生命周期与 present 调用。引擎支持 RGBA8/BGRA8 的 linear/sRGB 四种目标格式，其他格式返回明确的 `GpuFrameError::Unsupported`。

新增 `tests/native_gpu_host_view.rs` 2 项通过：宿主设备身份与注入对象一致，外来 owner 拒绝不改变待绘状态，直接纹理渲染后宿主自行 copy/readback 得到声明像素；不支持的 RGBA16F 目标明确拒绝并保持待绘。既有 GPU CPU 对照 3 项、GPU 损伤 3 项和 GPU 字体合成 2 项均通过。VM 无 `/dev/dri` 权限，使用软件 wgpu adapter；因此该证据证明宿主设备和目标纹理路径，不冒充真实硬件 Surface/垂直同步/device-loss 验收。

## 2026-11-02：完整 headless 场景调用方（第 5 项迁移）

新增公共 `examples/full_scene.rs`，单一进程依次跑通：Scene/SceneUpdate 声明与版本化样式/布局/字重更新、Framework 焦点、指针 Down/Up 单次激活、Blitz 布局、Vello CPU 渲染与 HeadlessBackend 呈现、外部文本服务权威显示快照、输入绑定从旧 revision 原子轮换到后继、UI 预编辑设置/取消、文本焦点租约关闭，以及两个视图最终退休。示例只依赖根 crate 公共模块，不导入 `native_adapter`、Blitz/Parley/Vello 私有模块。

VM 实跑成功，输出 `evidence/full-scene/full_scene.ppm`（P6 320×180，172815 字节，SHA-256 `5d1fdf5388311dbc691619307d606eda1e1f1bb830213a2653203fae6026df3c`）。证据：`evidence/full-scene-migration-pending/report.json`。

这是当时的首发快照，不是最终第 5 项状态：外部文本是宿主注入快照而非活服务；winit 窗口当时仍只记录输入。后续收口见本文件末节及 `evidence/item5-full-path-baseline/report.json`。

## 2026-11-02：六项总审计（0/6 完整勾选）

历史统一审计见 `evidence/final-checklist-audit-pending/report.json`。该快照记录旧口径下的 0 项完成：

> 此段是第 5 项收口前的历史审计快照；第 5 项后续按用户“功能优先、基础证明与集成测试即可、复杂证明登记给 Claude”的授权完成，本段不覆盖最终状态。

- 第 1 项：真实两节点 `BTreeMap` Kani 仍阻塞；`unwind=2` 展开不足、`unwind=4` 明确失败、`unwind=5/8` 超时，其他后端 OOM/转换失败；整帧实现级证明仍缺。
- 第 2 项：候选整帧事务、标签变化、状态恢复、CPU/后端路径已通过工程回归，但事务六阶段证明、默认入口迁移和真实显示仍未闭合。
- 第 3 项：字体合成与宿主 GPU TextureView 路径通过，但真实窗口 Surface/present、硬件 GPU/device-loss、能力矩阵与生命周期验收缺失。
- 第 4 项：同步指针/焦点/捕获、外部显示/组合客户端和 AccessKit 边界已有大量回归，但没有活外部服务、真实平台无障碍桥及完整组合证明。
- 第 5 项：`full_scene` 已跑通单个 headless 公共 API 全路径，但窗口输入转发、活服务、真实显示、示例六阶段和性能基线仍缺。
- 第 6 项：前置 1–5 未完成；真实 Map/浮点缺口仍在，不能做最终完成声明。

当前可复核的工程结果：默认、原生 CPU、原生 GPU 全 targets 编译通过；全量 CPU 测试除“无同族 sans oblique 夹具”外通过；GPU 宿主/CPU 对照/损伤/字体测试通过；bold-alias 六阶段限定验收通过；`test-blitz-proof-mapping.py` 当前映射与负控通过。该历史审计不提升任何清单项为完成。

## 2026-11-02：场景更新结构化拒绝契约

公共 `SceneHostView` 新增 `try_apply` / `try_apply_update -> Result<(), SceneUpdateError>`，明确区分归属、关闭、空帧、4096 补丁容量、过期版本及 expected/current、非法后继及 expected/next、来源拒绝和原生候选构造拒绝；不暴露 Blitz 类型或补丁内容。现有 bool 入口保持为 `is_ok()` 兼容投影。

`admit_scene_update` 实际分类器完成有序限定验收：Verus+Z3 5 verified/0 errors，所有权与后继版本两个负控拒绝；Proptest 256 例、严格 provenance Miri、cargo-fuzz 200000 次、Kani 148 checks（全 u64 版本/身份、全 usize 计数和 bool 状态，0 失败）通过，Loom 因纯同步无共享状态不适用。外部公共 API 测试覆盖全部八类结果并确认拒绝后版本/节点数保持。证据：`evidence/scene-update-error-contract-chain/report.json`。

第 1 项仍不勾选：该结果只证明准入分类，不证明来源场景事务、Blitz 候选构造、真实 Map 遍历或完整 `apply_update→apply_patches`。

## 2026-11-02：组件更新结构化拒绝契约

公共 `TextDisplayView` 新增 `try_update -> Result<(), ComponentUpdateError>`，明确区分归属/上下文、关闭、空帧、4096 项容量、过期序号及 expected/current、序号耗尽、视图不支持和原生映射拒绝；不暴露 Blitz 类型或更新载荷。现有 `update` 保留为结构化结果的 `is_ok()` 兼容投影。`admit_component_frame` 在任一组件值变更前分类完整帧，元素存在性与视图类型支持由实际预检带入。

该实际准入分类器完成限定有序验收：Verus+Z3 2 verified/0 errors，忽略归属与忽略视图支持两个负控被拒；Proptest 256 例、严格 provenance Miri、cargo-fuzz 200000 次、Kani 112 checks（全 bool 控制位、全 usize 计数和全 u64 序号，0 失败）通过；纯同步无共享状态，Loom 不适用。公共 API 集成测试覆盖归属、空帧、容量、过期序号、视图不支持、合法两项帧、序号精确推进和关闭视图。证据：`evidence/component-update-error-contract-chain/report.json`。

第 1 项仍不勾选：本结果不证明逐值原生 setter、resolve、回调 panic 回滚或完整组件帧组合；真实两节点 `BTreeMap` Kani、整帧变更与浮点规格缺口仍未关闭。

## 2026-11-02：场景整帧完成门与拒绝零修改

修正 `NativeScene::apply_patches_result`：不再在准入前使活动帧失效。来源拒绝、准入拒绝或原生候选失败均保持活动树、版本、焦点、滚动、指针状态、最近帧缓存和 `needs_frame` 脏状态不变；只有来源成功、来源版本精确等于后继且独立候选就绪后，才执行活动视图替换。

新增 `complete_scene_frame` 完成门并完成限定有序验收：Verus+Z3 2 verified/0 errors，忽略来源版本、无候选仍提交两个负控被拒；Proptest 256 例、严格 provenance Miri、cargo-fuzz 200000 次、Kani 112 checks（0 失败）通过；Loom 因纯同步标量函数不适用。`native_frame_transaction` 2/2 与 `native_update_contract` 1/1 通过，覆盖来源拒绝和候选拒绝后的完整帧保持。证据：`evidence/scene-frame-completion-chain/report.json`。

第 2 项仍不勾选：本结果只证明提交门，不证明 `ScenePatch` 应用、Blitz 候选构造/状态恢复、第三方 panic 回滚或真实显示呈现。

## 2026-11-02：真实 Map Kani 扩容复验

按用户新授权保留宿主 CPU 0–3、宿主至少 8G 可用内存后，验证脚本仍默认使用原 3G/零 swap/180 秒配置，但允许单次工作显式设置 3–8G 内存、0–24G swap 和最长 3600 秒。VM 进程继续固定在宿主 CPU 12–19，全程未使用宿主 CPU 0–3。

扩容后确认原 3G 结果是验证器容量问题，不是应用运行时泄漏。`unwind=5/6/8` 在更大资源下均得到确定性未完成结论：5 和 6 明确展开不足；8 在 object-bits 13 下于 8G RAM + 24G swap 中完成 Bitwuzla 转换，但 1898.449 秒后仍报告生产循环和 `BTreeMap` 的展开断言失败。8G RAM + 8G swap 时曾在 28 分钟 OOM，增加 VM 磁盘 swap 后不再 OOM。

该结果仍未通过门槛：需要改变验证器/后端编码，或采用保持真实 Map、原域、全部性质与检查的非缩减分解。证据：`evidence/native-subtree-expanded-capability/report.json`。

## 2026-11-02：VM 软件 X11 窗口呈现

在构建 VM 内安装并运行 Xvfb 后，根 `Engine` 的 winit/softbuffer CPU 链在真实 X11 协议窗口上完成一帧呈现并正常退出。初始 800×600 窗口截图包含精确的 432000 个背景像素及红/蓝各 24000 个 200×120 矩形像素；随后将窗口调整为 640×360，完整重绘后的背景为 182400 像素，两个矩形仍各为 24000 像素。

捕获脚本 `scripts/capture-window-xvfb.sh` 使用宿主持有的 X11 window、softbuffer Surface 和 present，不绕过根 Engine。此结果解除“VM 完全没有显示端点”的阻塞，但仍是软件 Xvfb：不含窗口管理器、合成器、垂直同步、硬件 Surface/device-loss、真实休眠恢复或系统无障碍/IME 服务。证据与截图：`evidence/window-xvfb-presentation/report.json`。

同一 X11 端点下新增 `examples/gpu_window.rs`：宿主取得 X11 Surface、wgpu 设备与 exchange texture，根 Engine 直接绘制到宿主持有的 `TextureView`，宿主调用 `Queue::present`。llvmpipe Vulkan 软件适配器上完成一帧并以 0 退出；硬件 GPU、真实 device-loss 与 vsync 仍未覆盖。

## 2026-11-02：正式 Engine 应用特性与整帧增删闭环

根 crate 新增面向应用的 `native-engine` / `native-engine-cpu` 特性，分别转发迁移期 `native-blitz-pending` / `native-blitz-cpu-pending` 实现门。活动 hello/pointer/window/full_scene 示例、窗口捕获脚本和独立诊断调用方均改用新名称；诊断 crate 仍只依赖 `jasper-ui-engine`，其 `Cargo.lock` 已在 VM 内离线重生成。`LegacyEngine`、Taffy→PaintOp→UiBatch 仅保留为显式兼容路径。

整帧候选事务现在是唯一更新路径：新增、删除、同批 Remove+Insert 换序、样式/文本/语义变化都先在保留来源副本上应用，再由终态重建独立 Blitz 候选，成功后一次替换。删除节点或祖先会同时退休其焦点与原生按下身份；同一 `NodeId` 在本帧复用不会接收旧 Press/Focus，迟到 Up 也不能激活替代节点。失败的来源/候选帧保持当前树、版本、焦点、帧缓存和 `needs_frame` 不变。

新增 `tests/native_frame_sequences.rs`：确定性覆盖新增、删除、同批换序、仅同 ID 重建的生命周期，以及 32 例任意多帧序列。任意序列对每个成功后继都与终态新导入的像素、节点数、版本和焦点比较；来源拒绝与原生候选拒绝都验证完整帧保持。`native_frame_damage` / `native_frame_schedule` 中过时的“拒绝后仍标脏”断言已同步为现行“拒绝零修改”契约。

受限 VM 结果：`cargo test --locked --no-default-features --features native-engine-cpu --all-targets` 全绿（175 个库测试、新增序列 3/3、全部集成与示例目标）；独立诊断 `typed_facade` 38/38；`cargo check --features native-engine --all-targets` 与默认 `cargo test --all-targets` 通过；Blitz 固定提交依赖检查通过。受影响的既有场景准入/完成门/关闭/快照身份/语义标签证明映射仍当前。证据：`evidence/native-engine-frame-sequence/report.json`。

第 2 项工程范围已收敛，但仍不勾选：任意整帧 `Scene` 应用、Blitz 候选构造、状态恢复与单次替换的完整 Verus/实现级有序证明尚缺；第三方 panic 回滚未承诺。证明项已登记给 Claude，不能用本轮 32 例属性测试替代。

## 2026-11-02：宿主渲染生命周期与能力矩阵

新增 `surface_lifecycle::SurfaceLifecycle` 与 `native::host::HostRenderer`。宿主完成尺寸/DPI
重配、暂停或恢复后得到严格递增的 Surface 代数；旧代数渲染与迟到生命周期回调被拒绝。
暂停保留场景、版本与语义，只清理原生交互并释放宿主呈现资源；恢复重建 viewport 与资源；
退休永久关闭且只成功一次。

GPU 直接绘制到宿主 `TextureView` 后，`needs_frame` 在宿主 `Queue::present` 确认前保持
`true`；`gpu_presented(owner, generation)` 只接受当前活动代数。后端提交或 present 返回错误
时视图重新标脏，允许重试。新增 CPU 生命周期集成测试与 GPU TextureView/present 确认测试均已
在实际公开入口通过。`HostCapabilityMatrix` 明确 CPU/DPI/字体/生命周期、GPU TextureView、
宿主 Surface/present/vsync 与未验收硬件/device-loss/jasper-os 边界，不以 Xvfb 或 llvmpipe
夹具冒充平台支持。

有限验证已从生产 `SurfaceLifecycle` 原样提取：Proptest 3/3、严格 provenance Miri 的
2 个确定性用例、Kani 2 个 harness/381 checks、cargo-fuzz 686245 次运行通过；Miri 跳过了
依赖文件系统持久化的 Proptest 执行器，不把这个跳过计作属性通过。Xvfb 复跑保持 800×600 与 640×360 的精确
背景/红/蓝像素计数，llvmpipe Vulkan 路径完成宿主 present 与代数确认并以 0 退出。
完整 HostRenderer 有序证明、硬件 GPU、device-loss、vsync、真实平台暂停恢复和 jasper-os
接入仍未完成，第 3 项保持未勾选。证据：`evidence/native-host-render-lifecycle/report.json`。

## 2026-11-02：交互与外部服务客户端闭环

新增公共 `TextInputSession`，将单一焦点票据、权威文本显示修订、预编辑组合序号、外部取消投递和永久退休收敛到同一客户端。外部显示必须有严格更新的修订；相同/更旧快照、陈旧场景失焦事件和已取消组合均被拒绝且不能触达后继。`ExternalInputService` 仅表示宿主传输接受取消，不冒充原生 IME 已终止；传递失败保留精确票据和组合关联供重试。停用、失焦、目标删除和退休均先关闭本地焦点/预编辑，指针释放失败保留捕获重试。

新增 `PlatformBridgeCapabilities` 明确引擎提供显示投影、场景路由和宿主中立无障碍树；权威文本/IME、窗口捕获、AT-SPI/UIA 适配器属于宿主。AccessKit 转换由可选 feature 提供，不包含平台运行服务。

注入字体、捕获回调和取消传输的 loopback 公共接口测试 7/7 通过，覆盖 IME 停用取消、传输失败重试、迟到/重复预编辑、权威快照重绑、焦点/捕获失败恢复、删除目标与新目标重绑、永久退休、重复抬起及无障碍树更新；`native-blitz-pending` 全量回归通过，AccessKit 3/3 与 CPU 启用的 7 项交互测试通过。无真实编辑/IME/系统无障碍服务，相关外部服务和平台适配器列为 TCB/能力限制；异步细化证明登记给 Claude。证据：`evidence/native-interaction-closure/report.json`。
## 2026-11-02：第 5 项原型与实际调用路径收口

活动 `hello`、`pointer`、`window`、`gpu_window` 与 `full_scene` 均已通过只消费根 `engine::Engine` 的静态边界回归；没有任何活动调用方导入 `native_adapter`、Blitz/AnyRender 私有类型、`LegacyEngine` 或 experiments 路径。显式 `legacy_pointer` 仅保留兼容对照。

`full_scene` 现在联合验证纯视觉 `SceneUpdate` 前后焦点与视口滚动保持，焦点/按钮动作仍在无障碍树中，增量滚动帧与同终态新导入逐字节一致；空字体栈、帧预算不足和过期版本拒绝后，宿主字节与活动视图保持可用。随后走完外部权威快照、输入票据轮换、预编辑重绘/取消、焦点租约关闭和视图退休。

`window` 现在通过 `HostRenderer` 的 Surface 代数门控把实际 winit 指针、滚轮、命令键、IME 预编辑/提交与失焦恢复路由到公共入口。`scripts/interact-window-xvfb.sh` 在 Xvfb 中通过 xdotool 点击真实窗口，日志必须报告 `activated=1`、视觉补丁后焦点保持和退休完成；800×600 与重设后的 640×360 Surface 截图均通过。

受限 VM 的集成回归 3/3 通过，额外覆盖 `HostRenderer` 在活动 Surface 代数上的焦点、无障碍、滚动和呈现准入，以及重配/暂停/恢复/退休后的陈旧代数拒绝。Xvfb 中窗口单帧、真实点击激活和 800×600→640×360 重设均通过，llvmpipe Vulkan 示例完成宿主 present。release 基线的 360×240 强制 CPU 全帧为 6 个样本，中位 153 µs、P95 177 µs；Surface 为 345600 字节，只作同一 VM 后续优化回归参照。证据：`evidence/item5-full-path-baseline/report.json`。

按用户本轮完成口径，第 5 项的功能缺口、基础生命周期准入、真实调用方/X11/GPU 集成证据已完成并勾选。整帧组合证明、真实外部编辑/IME、系统无障碍、硬件 Surface/device-loss 与最终平台阈值仍明确登记在 `docs/claude-item5-proof-backlog.md`，交由 Claude 后续完成。

## 2026-11-02：六项基础交付总收尾

按用户调整后的完成口径，第 1–5 项均为“功能缺口闭合 + 基础准入/集成证据 + 真实验收入口通过”，当前完成记录见 `evidence/final-checklist-audit/report.json`。六项存在明确依赖：第 1 项提供公共契约，第 2 项建立正式 Engine/Blitz 整帧路径，第 3 项绑定宿主 Surface/GPU 生命周期，第 4 项补齐交互和外部文本服务边界，第 5 项把活动调用方迁移到这些正式入口，第 6 项最后执行总审计与回归。

第 6 项总审计使用 `scripts/audit-closeout.py --run-baseline --require-ready`，检查当前源码快照、真实调用方证据、前序分支集成、工具固定版本、默认/原生 CPU/GPU 基础回归和工作树清洁度。旧 evidence 中因完整复杂证明未闭合而记录的 false 标志不改写，由最终审计明确列出其被当前用户口径替代。

复杂或高成本义务继续登记在 `docs/claude-closeout-proof-backlog.md` 与 `docs/claude-item5-proof-backlog.md`：真实双键/子树 `BTreeMap` Kani、整帧与窗口适配器实现级组合、浮点/布局深挖、硬件 GPU/device-loss/vsync、活 IME/平台无障碍服务及跨宿主性能门。它们不阻塞本轮功能 Beta 交付。

## 2026-11-03：桌面指针时钟边界（CP-05）与验证链修复

`DesktopInput` 的状态路径此前无法进 Kani：结构里带 `Instant`，一构造就碰到 `clock_gettime`。现在指针状态拆成不含时钟的公开类型 `DesktopPointerState`，`moved_at` / `pressed_at` / `released_at` 显式接收时间戳并原样报告；`DesktopInput` 只多一个 `start: Instant`，只在 `new` 与 `now_millis` 读时钟。毫秒换算由截断改为饱和（`desktop_timestamp_millis`）。行为上唯一的变化就是这个饱和，既有桌面测试不改动通过。

按下集合由 `BTreeSet<MouseButton>` 改为按 `desktop_button_key` 严格升序的 `Vec<MouseButton>`：拖动样本仍报告最小的按下按钮，顺序与原派生 `Ord` 一致。改动原因是实测：真实 `BTreeSet` 的 Kani 在符号执行阶段就停不下来（`unwind=13`，570 秒未结束，卡在 drop 的导航循环），与捕获存储的双键问题同类；固定版本 Verus 也没有该容器的规格。一个窗口同时按下的按钮极少，线性查找没有性能问题。

| 阶段 | 结果 |
|---|---|
| Verus | `desktop-pointer` 24/0（含依赖），`desktop-pointer-state` 31/0；共 6 个反例全部被拒（回绕时间戳、丢时间戳、拖动当悬停、按下丢按钮、抬起留按钮、移动丢按钮） |
| Proptest | `desktop_pointer_property`；读边界属性 49 项，`desktop::tests` 8 项 |
| Miri | `memory::` 增加时间戳边界与 8 步路径（含 NaN 坐标、`u64::MAX` 时间戳） |
| cargo-fuzz | 两个目标 142710 / 1382813 次 |
| Kani | `desktop_timestamp_saturation`（任意 u128）、`desktop_pointer_state_path`（真实状态与 Vec，任意按钮/坐标/时间戳，按下→修饰键→按下→移动→抬起→移动→抬起→移动，711 项检查 0 失败）；全部 55 个 harness 通过 |
| Loom | 同步无关，沿用独立状态的调度场景，1 项通过 |

默认测试 144 项、`--features desktop` 158 项全部通过。全链各阶段 scoped 通过，`complete=false` 仍因捕获查询双键义务（CP-01）。

未覆盖：`Instant` 读取本身（标准库 TCB）、winit 事件到状态转换的映射、`DesktopInput::ime` 的 Kani（其 Verus 不变性证明保留）。详见 `docs/claude-closeout-proof-backlog.md` 的 CP-05。

**验证链修复。** 接手时 `scripts/verify-ui.py` 在当前 main 上跑不通，这次修了三处，都不涉及任何性质或范围：

- 整库 rustfmt 改动了版式，链内 9 个内核的签名和十余个反例的文本匹配失效（Verus 阶段直接报 `signature changed`）。`annotate` 改为先逐字匹配，匹配不到时按词法匹配，只容忍空白和闭合符前的尾逗号；词法变化仍会报错。
- Miri 期望值过期（UI21 新增 `bold_alias_boundaries` 后，`memory::` 由 1 项变 2 项），Kani 期望数 54 而实际 harness 为 53。均已改为实际值，本轮新增 2 个 harness 后为 55。
- 单命令上限由 1800 秒提到 3600 秒。构建 VM 与其他任务共用时 Kani 超过 30 分钟；超时仍判失败。

不在 `verify-ui.py` 列表内的独立内核 `preedit-lifecycle-dispatch` 的反例 `ignore-disabled-owner` 在新匹配下有两处命中，仍需要单独修正；其余独立内核的提取与反例匹配已静态核对，但本轮没有逐个重跑 Verus。

**证据脱敏。** UI21 起有 64 个已跟踪的证据文件含构建机主目录路径。新增 `scripts/sanitize-evidence.py`，把这类路径换成 `<repo>` / `<home>`，并同步证据内记录的文件哈希，当前树已为零。今后每次重新生成已跟踪证据后都要运行一次。2026-11-03 已按用户裁决改写公开历史，清除所有提交中的这类路径和内部名称，原提交号随之变化；证据中记录的提交号已同步为新值。

**交付门状态。** 本轮 `closeout-baseline` 通过，但 `audit-closeout.py --require-ready` 现在返回未就绪：最终审计记录绑定在 UI32 的提交上，不覆盖本次改动的 `src/desktop.rs`（构建 VM 回传证据也会让工作树变脏）。第 1–5 项的完成记录尚未对新提交重新绑定。

## 2026-11-03 UI35：代际句柄表（Native ABI 起步）

新增 `handle` 模块：`Handle`（`repr(C)`，8 字节）、`HandleAllocator`、`HandleTable<T>`。这是 ABI 草案第 3 节的 Rust 实现，尚无 FFI 导出，也不冻结 ABI，且未接入 `Engine`。
实测：`tests/handle_table.rs` 5/5；性质与证明范围见 `verification.md` 的 UI-HANDLE-001。
未完成：`Vec` 增长失败仍中止进程；保留/引用规则、窗口销毁关系、跨进程句柄空间未决定；FFI 导出未开始。

## 2026-11-03 UI36：句柄状态分类与增长失败可恢复（FFI 前置）

`HandleAllocator`/`HandleTable` 新增 `status` → `HandleStatus { Live, Stale, Invalid }`，为 ABI 的 StaleHandle/InvalidHandle 区分提供依据。槽位、空闲表与值表改为可失败增长：分配失败时 `allocate` 返回 `None`、`insert` 交还值，状态不变；空闲表在追加槽位时一次预留，`release`/`remove` 与槽位复用不再分配。
实测与证明范围见 `verification.md` 的 UI-HANDLE-001。

未完成：错误码与 panic 屏障、FFI 导出、保留/引用规则、窗口销毁关系、跨进程句柄空间。`Engine` 尚未经句柄表寻址。

## 2026-11-03 UI38：整帧阶段顺序（CP-03 子性质）

`apply_patches_result` 的两个独立阶段改由 `order_scene_frame` 排序：来源阶段先运行，只有成功且版本等于精确后继时才进入候选构造，结果为 `Ok` 后调用方才替换活动视图。行为不变。真实视图集成测试在构造阶段外层注入 panic（构造前、真实候选建成后）与原生拒绝，确认活动视图的版本、焦点、按下、指针、滚动、帧缓存与重绘像素都不变，随后同一帧仍能提交。

这只闭合 CP-03 的阶段顺序。候选构造、状态恢复的实现级规格与补丁语义仍未证明，范围见 `claude-closeout-proof-backlog.md` 与 `verification.md` 的 UI-SCENE-FRAME-ORDER-001。

## 2026-11-03：真实 Map 子树证明的记录更正与公式缩减（CP-02 未完成）

**更正。** 此前把 Bitwuzla 在各展开界下的结果记为“展开断言失败”，这一判断是错的。这些日志的非平凡检查全部是 `Status: ERROR`，`FAILURE` 为 0；虚拟机里也没有 bitwuzla 可执行文件。用一个只记录参数并以 127 退出的桩求解器可复现同样的摘要（0 of 1116 failed、683 SUCCESS、429 ERROR）。所以 Bitwuzla 从未真正在这项义务上运行过，也没有任何展开界被否定。符号执行、SSA 转换、容量和内存相关的观察仍然有效。

**公式缩减。** 两个条目不会产生内部节点，但符号执行看不出节点高度和长度是常量，因此一直在展开内部节点的循环。改为按循环设界并保留全部展开断言后，公式明显变小：

| 配置 | 步数 | 剩余 VCC | SSA 转换 |
|---|---|---|---|
| 全局 unwind=8（早先） | 1943281 | 23160 | — |
| 全局 unwind=3，固定树 | 238092 | 4136 | 316.8 秒 |
| 按循环设界，固定树 | 189386 | 3368 | 29.8 秒 |
| 按循环设界，原全域 | 190827 | 3376 | 6.45 秒 |

原全域的规模与固定树几乎相同，因此逐棵枚举树没有收益。

**未通过。** 这项义务仍未得到结论，没有反例，也没有通过：CaDiCaL 在 26G 地址空间上限内运行 785.3 秒后，于数组后处理阶段报告内存不足；Kissat 和两种数组编码也都没有结论。这组展开界是否充分仍未确认。下一步是用固定版本的真实 Bitwuzla 跑这组配置。`apply_update → apply_patches` 的整帧性质完全未动。完整记录见 `evidence/native-subtree-unwindset-capability/report.json`。生产代码与验证入口均未改动。

## 2026-11-03：批次折叠（CP-03 子性质）与 CP-02 单属性尝试

`Scene::apply_batch_at` 的暂存循环改为 `stage_scene_batch` 加无接收者的 `apply_detached`，行为不变。折叠本身有六步证据（Loom 为 N/A）。真实 `Scene` 事务由 `tests/scene_batch_transaction.rs` 对照独立树模型测试，整个事务仍未证明，范围见 `verification.md` 的 UI-SCENE-BATCH-STAGE-001。

CP-02 仍无判定：只选一个断言（切片后 30 个 VCC）在 45 分钟内没有结果。

## 2026-11-04：候选的交互状态重放（CP-03 子性质）

`NativeScene::rebuild_candidate` 的恢复段改为 `restore_candidate_interaction`，行为不变。重放顺序有六步证据（Loom 为 N/A），前提是候选的写入遵守 trait 契约。`NativeScene` 的实现只由真实文档测试和九个手动变异检查，没有证明。Blitz 对这些写入的处理、候选构造、`apply_one` 的补丁语义仍未证明，范围见 `verification.md` 的 UI-SCENE-INTERACTION-RESTORE-001。

## 2026-11-04：节点移除语义（CP-03 子性质）

`remove_node` 从迭代器闭包改为两段下标循环，比较改为 `id.0`，行为不变。语义有 Verus 全域证明，范围见 `verification.md` 的 UI-SCENE-NODE-REMOVE-001。Kani 只跑完根加一个子节点的形状，嵌套形状超时、无判定。`apply_one` 的其余分支仍未证明。

## 2026-11-04：节点查找语义（CP-03 子性质）

`find_node` 从迭代器闭包改为下标循环；`find_node_mut` 改为每层先用只读路径确定含该身份的子节点，再借出。结果不变，但最坏代价从与节点数成正比变为节点数乘深度，未测量实际影响。语义有 Verus 全域证明，范围见 `verification.md` 的 UI-SCENE-NODE-FIND-001。`apply_one` 各分支的写入仍未证明。

## 2026-11-04：单个补丁对树的写入（CP-03 子性质）

`apply_one` 拆出 `apply_tree_patch`，七个分支的写入与错误有 Verus 全域证明，范围见 `verification.md` 的 UI-SCENE-PATCH-001。`Insert` 分支在 Kani 上每种形状都超时、无判定，只有 Verus 与测试。`Insert` 前的身份检查、版本赋值与整体替换仍未证明。

## 2026-11-04：插入前的身份检查（CP-03 子性质）

`collect_unique_ids` 改为对 `IdentitySet` 泛型，`apply_one` 的两次调用收进 `check_insert_identities`。遍历的判定有 Verus 全域证明，范围见 `verification.md` 的 UI-SCENE-UNIQUE-IDS-001。真实 `BTreeSet` 满足契约只经测试。版本赋值与整体替换仍未证明。

## 2026-11-04：批次提交（CP-03 子性质）

`apply_batch_at` 的函数体移入 `commit_scene_batch`，检查与暂存改为只拿共享引用的闭包。版本判定之后的写入只有末尾一次整体替换，这一点有 Verus 全域证明，范围见 `verification.md` 的 UI-SCENE-BATCH-COMMIT-001。生产闭包、`Scene` 的克隆、闭包内 panic 不在证明内，整事务性质 UI-TXN-ATOMIC-001 仍是 not_verified。

## 2026-11-04：验证入口改为定点 Kani

`verify-ui.py` 的 Kani 阶段不再跑整库，改为 `--kani-harness` 指定的 harness，单个 10 分钟、3.5 GB，同一时间一个。撞上限记 `undetermined`，链继续但不算通过；反例和工具错误立即失败。实测两个 harness 的全链六阶段通过，入口退出 2 的原因仍是 CP-01；2 秒上限的对照按预期得到 `undetermined`。其余 harness 本轮没跑，证据仍绑定之前的提交。不指定 harness 的路径和 `--kani-full` 没有端到端运行过。没有生产代码改动。

同日更正 Bitwuzla 记录：首次构建和运行 A–F 早于用户本人的批准，见 `evidence/native-subtree-bitwuzla-capability/report.json`，并补了来源与校验值。2026-11-03 的编译参数和下载日志没有保留。

## 2026-11-04：Kani 改到正式版发版时验证

这条取代同日的定点 Kani 做法。`verify-ui.py` 默认不跑 Kani，该阶段状态为 `deferred_to_release`，登记写 `Kani：发版时验证`，不因它判未完成；发版时用 `--kani-harness` 或 `--kani-full` 显式打开。CI 不再安装 Kani。CP-01 和 CP-02 的 Kani / Bitwuzla 路径移到发版清单，义务未解除，CP-02 不依赖 Kani 的部分继续。现有 harness 和旧证据不删。没有生产代码改动。

2026-11-04 `.jaso` UI 插件（JB-U1）：`ui.*` 第 1 版写入 `abi.md` 第 10 节，明确为 UI 宿主命名空间，不属于 Native ABI v1 核心。真实编译的原生插件经 L1 装入并画进一帧 CPU 无头画面，负向用例见 `verification.md`。未完成：GPU 路径、树内 z 序、x86_64 Linux 以外的宿主、进程隔离。`ui.*` 是否提升为公共 ABI 待 CP 系列闭合后决定。

## 2026-11-04：补丁后的身份唯一（CP-03 子性质）

`apply_one` 的函数体移入 `apply_checked_patch`。插入只有在树后接子树无重复时才被准入，成功后全树身份无重复；其他补丁不做检查，原本无重复的树仍无重复。这些有 Verus 全域证明，范围见 `verification.md` 的 UI-SCENE-CHECKED-PATCH-001。真实 `BTreeSet` 满足契约只经测试，跨整批的组合没有陈述，整事务性质 UI-TXN-ATOMIC-001 仍是 not_verified。定点通过，等批次全量。

## 2026-11-04：跨整批的身份唯一（CP-03 子性质）

`apply_batch_at` 的主体移入 `apply_unique_batch`。已有重复的树不接受任何批次；成功的非空批次版本加一，结果树无重复，且是逐补丁受检写入的轨迹终点，轨迹上每个状态都无重复；任何失败都让场景保持原值。这些有 Verus 全域证明，范围见 `verification.md` 的 UI-SCENE-BATCH-UNIQUE-001。派生克隆返回相同的值是证明中的假设，只经测试；真实 `BTreeSet` 满足契约也只经测试，整事务性质 UI-TXN-ATOMIC-001 仍是 not_verified。定点通过，等批次全量。

上一次批次全量在提交 342ae26 上：`cargo test --all-features`（xvfb，注入五份字体）351 项通过、0 失败、0 忽略。本切片的合入不在其中。

## 2026-11-05：Surface 生命周期迁移（CP-06 子性质）

`SurfaceLifecycle` 的全部迁移（`reconfigure`、`suspend`、`resume`、`retire`、`retire_current`）与任意轨迹有 Verus 全域证明，范围见 `verification.md` 的 UI-SURFACE-LIFECYCLE-001。这只是生命周期状态机：`HostRenderer` 的编排、wgpu/Vello、硬件 Surface、合成器、垂直同步、设备丢失、真实平台休眠恢复与能力矩阵都没有覆盖，CP-06 仍未闭合。定点通过，等批次全量。

## 2026-11-05：宿主对 Surface 生命周期的协调（CP-06 子性质）

`HostRenderer` 的 `reconfigure`、`suspend`、`resume`、`retire`、`close` 与待呈现标志改由新类型 `SurfaceCoordinator` 执行：先在副本上迁移，准入后才调用引擎，引擎接受后才提交。对任意引擎闭包，有 Verus 全域证明：只有被准入的步骤才能调用引擎；拒绝与生命周期自身一致；引擎拒绝不改变任何状态；引擎接受后生命周期必定迁移；任意轨迹上，待呈现帧来自当前生命周期状态。范围见 `verification.md` 的 UI-HOST-SURFACE-COORDINATION-001。

这改变了几种错误结果，原先的预检与生命周期不一致：`resume` 对非暂停且代数过期的 Surface 现在报 `StaleGeneration`（原 `WrongState`）；对已退休 Surface 再次 `retire` 现在报 `Lifecycle(WrongState)`（原 `ViewRetired`），且不再调用引擎；`close` 对外来归属者或已退休 Surface 不再调用引擎；非活动 Surface 代数为最大值时，`reconfigure` 与 `suspend` 报 `WrongState`（原 `GenerationExhausted`）。

未覆盖：引擎闭包内部做了什么、各公开入口是否调用了正确的门（靠阅读与集成测试）、硬件 Surface、合成器、垂直同步、设备丢失、真实平台休眠恢复、能力矩阵、wgpu/Vello TCB 登记，CP-06 仍未闭合。过期版本被引擎拒绝时仍报 `ViewRetired`，名称不够准确，本切片未改。定点通过，等批次全量。

## 2026-11-05：渲染结果与待呈现帧（CP-06 子性质）

`HostRenderer` 的五个渲染入口改为经过同一道门 `SurfaceCoordinator::render`。对任意归属者、代数、状态、结果类型与引擎闭包，有 Verus 全域证明：引擎只在恰为当前活动代数时才会被调用；拒绝不改变任何状态；生命周期不因渲染迁移；渲染之后，宿主能否确认呈现，当且仅当这一次渲染成功写入了宿主纹理。范围见 `verification.md` 的 UI-HOST-RENDER-OUTCOME-001。

这改变了失败后的行为，原先各入口不一致：

- `render_gpu_to_host_view` 失败（门之后的任何失败，包括版本过期与不支持的格式）现在撤销仍在等待呈现的帧，`gpu_presented` 返回 false，`needs_frame` 返回 `Some(true)`。原先这个帧仍可确认，而失败的渲染可能已写过同一纹理。代价是宿主确实可以呈现的旧帧也要重绘一次。
- `render_to_surface`、`render_to_surface_with_damage` 失败现在也使视图保持待重绘，与 `render_to_backend`、`render_gpu` 一致。这几个入口失败时同样撤销待呈现帧。
- 门处拒绝（外来归属者、过期代数、非活动状态）行为不变：引擎不被调用，待呈现帧保留。

未覆盖：引擎闭包内部做了什么；公开入口与门的对应只有测试和阅读，没有证明。硬件 Surface、合成器、垂直同步、设备丢失、真实平台休眠恢复、能力矩阵与 wgpu/Vello TCB 登记仍没有证据，CP-06 仍未闭合。定点通过，等批次全量。

## 2026-11-05：能力矩阵与渲染 TCB 登记（CP-06 子性质）

`HostCapabilityMatrix` 的判定表有 Verus 全域证明，范围见 `verification.md` 的 UI-HOST-CAPABILITY-MATRIX-001；第三方渲染代码登记在 `render-tcb.md`，由脚本对照 `Cargo.toml` 与 `Cargo.lock` 检查。

核对时发现矩阵有三处声明与实际不符，已修正，都是报告结果的**行为变化**：

- `compiled()` 在开启 `native-blitz-pending` 时仍报 DPI、字体、生命周期为 `FeatureGated`，与 `host-rendering.md` 矛盾。现在报 `NativeContract`；`native()` 返回相同结果。
- `cpu_rgba8_surface` 原先看 `native-blitz-cpu-pending`，而入口的门控是 `cpu-pending`，所以只开 `gpu-pending` 时入口已编译却报 `FeatureGated`。现在看 `cpu-pending`。
- `gpu_device_loss_recovery` 原先是 `Unverified`（含义是已实现、未验收），但仓库没有任何设备丢失代码。现在是 `Unsupported`；`gpu.md` 的重建路径改标为目标，未实现。

未覆盖：矩阵只说明声明与构建一致，不说明入口行为正确。硬件 Surface、合成器、垂直同步、设备丢失（未实现）、真实平台休眠恢复仍没有证据，这些需要真实硬件或新实现，CP-06 仍未闭合。定点通过，等批次全量。

## 2026-11-09：Ubuntu 参考宿主跨平台切片

在 Ubuntu Wayland 真机会话完成参考宿主验证，详细结果见
`evidence/linux-reference-host/report.json`：

- `PointerMotionCoalescer` 在宿主无关输入层合并同一指针连续 Move/Hover；Down/Up/Cancel、
  滚轮、键盘、文本和焦点事件会先冲刷待处理移动，保证生命周期顺序。
- winit `Touch` 映射为带稳定身份的 Down/Move/Up/Cancel；第一个活动触点保持 primary，
  其结束时下一个活动触点接任。校准/归一化 force 被限制到 `0..=1`。
- `PointerMotionPredictor` 用宿主时间戳提供显示用短时外推；指针身份或生命周期变化会清空历史。
- `examples/multiwindow` 为 owner 100/101 建立两个独立场景、`HostRenderer`、Surface 代数和
  softbuffer 后端，在 Wayland 同时完成呈现；引擎状态与宿主 Window/Surface 对象分离。
  `--exercise-suspend-resume` 实际释放两组宿主窗口/Surface、暂停引擎视图、重建后再次呈现。
  `native_multiwindow_isolation` 验证一个 owner 的重配、暂停和退休不改变另一个 owner。
- `host_transport::HostInbox` 提供会话/epoch、精确序号、有界背压与关闭拒绝；它不实现或冒充
  系统 IPC，供桌面、移动和 jasper-os 内置宿主复用。
- `desktop-accessibility` 通过固定 `accesskit_winit 0.34.2` 建立平台适配器。Ubuntu AT-SPI
  注册表实际返回工具包名 `Jasper UI`、按钮 `Activate`、`click` 动作，`DoAction` 返回 true。
- 同一可选依赖按目标平台选择 AT-SPI/UIA/macOS/Android/iOS；Linux 实测不扩展为其他平台已验收。
- 真实 NVIDIA RTX 3060 Vulkan Surface 在 Wayland 合成器下选择 `Bgra8UnormSrgb` 和
  `Fifo` present mode。设备丢失由宿主持有的 wgpu callback 报告；示例完成
  frame 1→宿主 pending 事件→暂停→重建 device/Surface/renderer→frame 2 的恢复演练。
  VM 测试通过真实
  `Device::destroy()` 观察到 `DeviceLostReason::Destroyed`。驱动诱发的硬件丢失、实际
  vblank 截止时间、真实 session suspend/resume 仍未验证。

CI 新增 Ubuntu/Windows/macOS 的宿主无关核心编译与测试矩阵；这只是持续编译门，
不代表 Windows/macOS 平台宿主已经完成运行验收。本次未修改 jasper-os。

宿主无关默认核心同时通过 `cargo check --locked --lib` 的 Linux x86_64、Windows
x86_64 GNU、macOS x86_64/AArch64、Android AArch64 与 iOS AArch64 交叉目标检查；`desktop-accessibility`
窗口路径也通过 Windows x86_64、macOS x86_64 和 AArch64 的目标编译。该结果证明
平台适配层可以面向这些目标编译，不证明 winit/wgpu/无障碍平台实现已在对应系统运行。

内置宿主 loopback 参考已接通：`HostInbox` 接收版本化场景更新、CPU 帧和退休命令，
对满队列执行回压，更新后的帧包含声明颜色，退休后继续渲染被拒绝。示例和测试不链接
winit/wgpu；真实系统 IPC、认证与 jasper-os 宿主仍未实现。结果：
`host_loopback=pass session=41 epoch=3 version=2`，对应测试 1/1 通过。

Windows 10 LTSC x86_64 虚拟机参考宿主已完成运行验收：单窗口与双窗口 CPU Surface、
100%/150% DPI、物理鼠标点击、中文有序字体回退、UIA 枚举与 Invoke 均通过。中文按钮
`激活 Activate` 在 UIA 中的实际码点为 `U+6FC0 U+6D3B`；验收脚本固定 UTF-8 控制台编码，
不再把 PowerShell 代码页的 `??` 误判成 UI 文本损坏。

同一 Windows VM 的 wgpu 路径选择 DX12 `Microsoft Basic Render Driver`、
`Bgra8UnormSrgb` 与 `Fifo`，完成单帧、Surface 重建 generation 1→2 和宿主持有 device-loss
演练 generation 1→2。该结果证明 Windows wgpu 宿主路径可运行；虚拟机基础渲染适配器不是
物理 GPU，也不能替代 Ubuntu RTX 3060 的硬件呈现证据。

## 2026-11-09：macOS 与 iOS/iPadOS 参考宿主

`macos/` 与 `ios/` 两个平台适配器落地；宿主持有窗口、Metal Surface、设备/队列与平台生命周期，
引擎仍只接收自有场景值、规范化输入与 Surface 代数。文档见 [macos.md](macos.md)、
[ios.md](ios.md)。

**macOS（真机实测）。** Apple M2 / macOS 15.7.5 / Xcode 26.3 上 `scripts/verify-macos-host.sh`：

- Metal 适配器 `Apple M2`，`Bgra8UnormSrgb`，`AutoVsync`；960×640 逻辑 = 1920×1280 物理，`scale=2`。
- 每帧从呈现用 drawable 读回 PPM：首帧背景 2,140,111 / 按钮 293,924 / 文字与抗锯齿
  23,565 像素，两次运行逐字节一致。
- 中文：与 Android/Windows 相同的有序栈 `DejaVuSans` + `DroidSansFallbackFull`，中文行与
  按钮 `激活 Activate` 均为真实字形（墨迹 23,565 像素）；`--cjk-tofu` 去掉回退字面后同位置
  是缺字框（15,349 像素），两帧 SHA-256 不同。截图与 PPM 留在证据目录。
- 真实 WindowServer 输入：`CGEvent` 在按钮中心 `734,484` 投放，同样触发激活与版本化重绘；
  两轮运行 `frames=4 activations=2` 稳定复现。
- 修复并记录一个真实缺陷：AccessKit 的 bounds 是物理像素，`accesskit_macos` 会除以
  `backingScaleFactor`；宿主若直接传逻辑 bounds，AX 帧会缩小一半并整体偏移。参考宿主改为
  `Affine::scale(scale)`，并把该单位约定写进 `docs/macos.md`。

**iOS/iPadOS（构建与静态验证通过，运行未验收）。** 同一二进制声明 `UIDeviceFamily [1,2]`：

- `aarch64-apple-ios` 与 `aarch64-apple-ios-sim` 均完成 `cargo build` 链接，链接
  UIKit/Metal/QuartzCore；`-Wl,-platform_version` 产出 `LC_BUILD_VERSION`
  （`IOS` / `IOSSIMULATOR`，`minos 15.0`）。参考宿主导入 `bundle` 的 `Info.plist` 由
  `ios/Info.plist.in` 按目标替换平台字段。
- 静态验证：`plutil` 声明、`UIDeviceFamily [1,2]`、Mach-O 平台、框架链接、simulator 的
  ad-hoc 签名 `codesign --verify --deep --strict`、二进制与 plist 哈希。
- 可移植规则 `ios/src/model.rs` 7/7：完整生命周期循环、重复/乱序通知幂等、终止终态、
  后台直接激活、iPad 安全区换算、非法内缩、主触点移交。
- `--features host-on-macos` 把同一宿主模块编译进 macOS 构建演练：iPhone 视口（390×844）与
  iPad 视口（834×1194，安全区 24/0/20/0）都呈现两帧，iPad 配置下按按钮中心点击产生一次激活。
- **触控笔**：winit 在 iOS 上把 Apple Pencil 报成 `Force::Calibrated`（带高度角）。新增
  `model::pencil_sample`：笔轴压力除以 `sin(高度角)` 得到垂直屏幕压力并归一，高度角向上暴露，
  平放不做除法、NaN 回退到按下/抬起默认值；宿主每个 Touch 记录 `stage=pencil`。新增 4 项
  单测（合计 11/11）。该值目前停在宿主边界——原生 `Engine::pointer` 不消费压力/倾角。
- **设备探测**：`devicectl` 识别到已配对的 iPhone（iOS 26.6.2，USB）与 iPad（型号与 UDID 不入公开证据）；`security find-identity` 为 0，缺开发签名身份。新增 `ios/install-device.sh`
  完成签名构建 → `devicectl install` → 带 `--console` 启动 → 取回容器内 PPM 的整链，当前
  明确输出 `device_install=blocked reason=no_signing_identity`，不自称真机已验收。
- **运行未验收**：验证机没有安装 Simulator 运行时，也没有代码签名身份，因此没有 iOS 运行时
  启动画面、UIKit 生命周期实测或真机安装。`scripts/verify-ios-app.sh` 在有运行时的机器上会
  自动安装并启动，当前明确输出 `simulator_run=skipped reason=no_runtime`。

未完成：macOS 的 Metal 设备丢失、vsync 截止时间、活 IME、公证与 Intel 运行验收；iOS 的运行时
与真机验收、`UIView.safeAreaInsets` 查询、系统无障碍、软键盘/IME、Pencil 压感进入原生场景
与设备矩阵。
