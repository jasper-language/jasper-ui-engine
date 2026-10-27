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
| 桌面窗口/软件呈现 | `examples/window.rs` | winit + softbuffer 色块窗口；示例把输入打印到 stderr，尚无状态驱动交互闭环 |
| GPU 选择与枚举 | `graphics.rs`、`gpu_wgpu.rs`、`gpu` | 策略、回退说明与真实适配器枚举；无 GPU 绘制管线或 device-lost 恢复 |
| 流水线 | `engine.rs` | 每帧完整布局/绘制/编码/提交/呈现，无完整增量失效与调度 |
| 最小宿主接口 | `platform.rs` | 尺寸/缩放/重绘/光标；不等于完整 UiHost 或已可用的系统内置服务 |

## 未完成与外部协作

- 场景文字的光标/选区/文本命中几何（根入口尚未暴露）、完整文本编辑与 IME 往返；双向文字、字重/斜体与字体族选择未验收。原生路径的文字光栅、多字体按序回退与折行已有无头实测（见 2026-10-27 UI3 节）。
- 图片/SVG 解码、资源额度、圆角/阴影/变换、按需帧调度与失效缓存。
- GPU 渲染、系统无障碍桥、多窗口/DPI/触控/笔平台覆盖。
- 版本化语义更新协议、代际句柄/所有权、Native ABI、完整宿主能力协商。
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
| GPU 绘制 | unsupported | unsupported，仅枚举 | unsupported | 外部后端 |

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
