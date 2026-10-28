# Rust API（v0.1 原型）

> 这是当前 crate 的公开 API 说明；**不是稳定 ABI**。原型阶段允许破坏性修改。

## 原生 Blitz 工程入口（候选，未验收）

根 crate 的 `engine::Engine` 通过 `native-blitz-pending` 启用，CPU 软件呈现使用 `native-blitz-cpu-pending`。原生适配由根 crate 编译，诊断 crate 仅依赖并消费根入口，直接消费 Blitz 树/样式/事件，不调用显式兼容 `LegacyEngine` 的 Taffy→paint→UiBatch 链。不自动创建窗口、设备或 Surface。

原生适配的唯一生产源码现位于 `src/native_adapter/`；根 crate 不再通过路径引用实验目录。诊断 crate 的 `lib.rs` 仅重导出根公开接口，保留 CLI/外部调用方验收。源码搬迁逐文件保持字节相同，已有规格按实际函数体重核对应；这不补齐原本缺失的实现证明。

Framework 产生 `native::SceneUpdate { expected_version, next_version, patches }`；嵌入宿主单独提供归属身份，调用 `Engine::apply_update`。补丁同步借用，拒绝不消耗版本；不承诺第三方 panic 的事务回滚。应用侧只使用自有声明和事件值，宿主侧提供字体字节、逻辑/物理尺寸、DPI 和呈现缓冲。

原生 CPU 入口 `Engine::render_to_backend` 在归属/版本/存活/预算准入后，复用原生绘制并调用既有 `RenderBackend::submit_rgba8`、成功后才调用 `present`。默认后端明确返回 Unsupported；HeadlessBackend 接管已经绘制的像素，不重放 PaintOp。字节按上游输出原样保存，不提供宿主 alpha/合成转换或外部呈现回滚；旧 UiBatch 序列化格式不变。此新增编排与包装尚未完成有序证明验收。

文字几何（`native-blitz-pending`）：`text_offset_at(owner, version, node, x, y)` 把视图局部逻辑点投影到文字节点自身的布局，返回源文本中最近的 UTF-8 光标偏移（放置光标用，不考虑遮挡）；`text_caret(owner, version, node, offset, width)` 返回该偏移之前的光标矩形，坐标同 `node_bounds`。两者与绘制共用同一份布局；布局文本与源文本不一致（如空白折叠）、非字符边界、过期版本等情形返回 None。`text_selection(owner, version, node, start, end)` 返回源文本 UTF-8 区间 [start, end)（start ≤ end）的逐行选区矩形，同一坐标系与同一布局；空区间返回空列表，反向、非字符边界或越界区间返回 None；只给几何，不绘制选区高亮。

无障碍树（`native-blitz-pending`）：`accessibility_tree(owner, version)` 返回 `AccessibilityTree { scene_version, nodes, focused }`，供平台无障碍桥（AccessKit、AT-SPI、UIA 或系统无障碍服务）构建各自的树。`nodes` 为可见节点，按源顺序深度优先、根在首位；每个 `AccessibilityNode` 含 Framework 原样语义（role、label、description、value、disabled）、与 `node_bounds` 相同的视图局部几何（无几何时为 None）、场景内焦点标记、本引擎准入的动作 `AccessActions { focus, activate }` 与可见子节点列表。隐藏（semantics.hidden）或未显示（display: none）的节点连同其子树剔除；根本身不可见时 `nodes` 为空。非归属方、过期版本或已关闭视图返回 None。本方法不调用任何平台无障碍 API，也不做屏幕坐标换算；增量更新事件、平台动作路由与文本无障碍接口尚未提供。

帧调度（`native-blitz-cpu-pending`）：`needs_frame` / `invalidate_frame` 报告与强制待绘状态；`render_to_surface_with_damage` 报告相对上一帧的 `FrameDamage`；`render_to_surface_damaged` 只把损伤像素写入宿主 Surface；`frame_pace(owner, presented, origin, interval, now)` 按宿主刷新时钟返回 `FramePace::{Idle, WaitUntil(t), RenderNow(tick)}`，每周期至多一帧、不追帧，时钟与垂直同步来源归宿主，本方法不绘制也不等待。

GPU 渲染（`gpu-pending`）：`GpuRenderer::new_headless()` 不带呈现表面取得 wgpu 适配器（由平台/wgpu 环境变量选择）并创建设备，`adapter_info()` 返回适配器名与后端；`render_gpu(owner, version, &mut gpu, budget)` 经与 CPU 路径相同的 Blitz 绘制、在 Vello hybrid 上离屏渲染并读回紧凑预乘 RGBA8；`render_gpu_to_surface(owner, version, &mut gpu, bytes, stride, budget)` 写入宿主软件 Surface，Surface 准入先于任何 GPU 工作。准入失败为 `GpuFrameError::Rejected`；无适配器、设备、渲染、读回失败分别报告，绝不回退 CPU 帧冒充。GPU 帧后 CPU 帧缓存失效（下一次 CPU 损伤为 Full）。不提供窗口呈现与 device-lost 恢复。

`native` 及 Rust 类型名仍为候选，不冻结 Jasper 子命名空间。完整整帧/生命周期/浮点证明尚缺；该入口存在不等于默认后端迁移或六阶段验收完成。

`Engine` 仅在原生 feature 启用时存在；未启用时不会静默选用旧执行链。此前原生名称 `BlitzEngine` 保留为同一实现的兼容重导出。旧引擎明确名为 `LegacyEngine`，旧统计与错误只描述兼容路径。

## 旧入口快速开始（迁移兼容）

```rust
use jasper_ui_engine::backend::HeadlessBackend;
use jasper_ui_engine::engine::LegacyEngine as Engine;
use jasper_ui_engine::geometry::{Edges, Size};
use jasper_ui_engine::scene::{Node, NodeId, Scene};
use jasper_ui_engine::style::{Color, Length, Style};
use jasper_ui_engine::text::SimpleTextEngine;

let scene = Scene::new(
    Node::container(NodeId(1))
        .with_style(
            Style::default()
                .size(Length::Px(320.0), Length::Px(200.0))
                .padding(Edges::all(16.0))
                .gap(12.0)
                .background(Color::rgb(18, 22, 30)),
        )
        .child(
            Node::rect(NodeId(2)).with_style(
                Style::default()
                    .height(Length::Px(36.0))
                    .background(Color::rgb(36, 48, 68)),
            ),
        ),
);

let mut engine = Engine::new(
    Size::new(320.0, 200.0),
    HeadlessBackend::new(320, 200),
    Box::new(SimpleTextEngine),
).with_clear_color(Color::BLACK);

let stats = engine.render(&scene).expect("render");
engine.backend().save_ppm("out.ppm").expect("write");
println!("{} boxes, {} ops, {} bytes", stats.layout_boxes, stats.paint_ops, stats.batch_bytes);
```

## 核心类型

| 类型 | 模块 | 用途 |
|---|---|---|
| `Point` / `Size` / `Rect` / `Edges` | `geometry` | 逻辑像素几何 |
| `Length` | `style` | Auto / Px / Percent |
| `Color` | `style` | RGBA，straight alpha |
| `Style` | `style` | PSS 子集 |
| `NodeId` / `Node` / `NodeKind` / `Scene` | `scene` | 实例场景 |
| `ScenePatch` | `scene` | 增量操作（非事务） |
| `TaffyLayoutEngine` | `layout` | Taffy 布局适配 |
| `LayoutBox` / `LayoutTree` | `layout` | 绝对布局结果 |
| `TextEngine` / `SimpleTextEngine` | `text` | 文本测量接缝 |
| `PaintOp` / `DisplayList` / `paint` | `paint` | 显示列表 |
| `UiBatch` | `batch` | 版本化渲染批 |
| `RenderBackend` / `HeadlessBackend` | `backend` | 后端契约/软件后端 |
| `PlatformHost` / `HeadlessHost` | `platform` | 宿主窗口/DPI/重绘 |
| `PointerEvent` / `PointerTracker` | `input` | 指针/触摸/笔 |
| `ScrollBounds` / `ScrollConfig` / `ScrollController` / `ScrollUpdate` | `scroll` | 滚动范围、wheel 路由、fling 惯性 |
| `Scrollbar` / `ScrollbarConfig` / `ScrollbarDrag` / `Scrollbars` | `scrollbar` | 滚动条几何、命中、拖动、绘制 |
| `Engine` / `FrameStats` | `engine` | 流水线 |

## Scene

```rust
let mut scene = Scene::new(Node::container(NodeId(1)));
scene.apply(ScenePatch::Insert {
    parent: NodeId(1),
    index: 0,
    node: Node::rect(NodeId(2)),
})?;
scene.apply(ScenePatch::UpdateStyle { id: NodeId(2), style: Style::default() })?;
scene.apply(ScenePatch::Remove { id: NodeId(2) })?;
```

- `NodeId` 在场景生命周期内稳定；重复 ID 会报 `SceneError::DuplicateNode`。
- `ScenePatch::SetScroll` 更新节点滚动偏移，走同一套版本/原子批。
- `Scene::version()` 单调递增；`apply` / `apply_batch` **原子**（失败不改变场景/版本）。
- `apply_batch_at(expected, patches)` 拒绝过期批次，返回 `SceneError::StaleVersion`。
- `LayoutTree.scene_version` / `FrameStats.scene_version` 把版本带到宿主。

## Style / Layout

- 尺寸：`Length::Auto/Px/Percent`；`min_*`/`max_*`。
- 布局：`Display::{Block,Flex,None}`、`FlexDirection::{Row,Column}`、`Align`、`Justify`、`gap`、`flex_grow`。
- 间距：`padding`、`margin`、`border`。
- 溢出：`overflow(Overflow::Hidden/Scroll)`；`Scroll` 支持 wheel 路由/clamp/fling 与 overlay 滚动条（见“滚动控制”）。
- 滚动：`Node::scroll(Point)`，或 `ScenePatch::SetScroll { id, offset }`（见“滚动控制”）。
- 文字：`font_size`、`color`（测量参与布局；光栅未实现）。
- 当前不支持阴影/滤镜/变换/虚拟化。

## Paint / UiBatch

```rust
let layout = engine.layout(&scene)?;
let display = jasper_ui_engine::paint::paint(&layout, Color::WHITE);
let batch = jasper_ui_engine::batch::UiBatch::new(display.ops);
let bytes = batch.encode();
let decoded = jasper_ui_engine::batch::UiBatch::decode(&bytes)?;
```

- `UiBatch` 版本 1；magic `JUEB`；解码有边界/长度/操作数检查。
- `DrawText` 进入批处理没问题，但 `HeadlessBackend` 会返回 `Unsupported`（文字光栅未实现）。

## Backend

```rust
pub trait RenderBackend {
    fn resize(&mut self, size: Size);
    fn submit(&mut self, batch: &UiBatch) -> Result<(), BackendError>;
    fn present(&mut self) -> Result<(), BackendError>;
}
```

实现后端时：

- 不要绕过 `UiBatch` 直接读 `Scene`；批处理是唯一入口。
- 不支持的操作返回 `BackendError::Unsupported`，不能静默成功。
- GPU 后端后续实现同一 trait；`HeadlessBackend` 作为降级/测试基线。

## Platform

```rust
pub trait PlatformHost {
    fn size(&self) -> Size;
    fn scale_factor(&self) -> f32;
    fn request_redraw(&mut self);
    fn set_cursor(&mut self, cursor: Cursor) {}
}
```

内置宿主用系统通道/UI 服务实现；自带宿主用 winit 等实现。引擎不依赖具体窗口库。

## Input

```rust
let event = PointerEvent {
    pointer_id: PointerId(1),
    kind: PointerKind::Pen,
    phase: PointerPhase::Down,
    position: Point::new(10.0, 20.0),
    pressure: 0.7,
    tilt: Point::new(10.0, -5.0),
    twist: 45.0,
    contact: Size::new(1.0, 1.0),
    ..Default::default()
};
tracker.track(event)?;
```

- 触摸/笔/鼠标统一为 `PointerEvent`；多点用 `pointer_id`。
- `PointerTracker` 只跟踪按下中的指针；Hover/Move 未按下时忽略。
- 手势、命中测试见下文；IME 组合尚未实现。

## 命中测试与指针路由

```rust
use jasper_ui_engine::hit_test::{hit_test, hit_test_path};
use jasper_ui_engine::pointer::PointerRouter;

let target = hit_test(&layout, Point::new(30.0, 30.0));       // 最上层绘制节点
let path = hit_test_path(&layout, Point::new(30.0, 30.0));   // root -> target

let mut router = PointerRouter::new();
let route = router.route(pointer_event, &layout)?;
// route.target / route.path / route.captured
```

- 命中顺序与绘制顺序一致（后绘制在上层）。
- 透明容器本身不作为命中目标；其有绘制内容的子节点仍可命中。
- Down 记录 target/path；Move/Up/Cancel 使用 capture；多指/多笔独立。
- 手势识别、命中语义（可点击/可聚焦）仍由 Jasper UI 层后续定义。

## 语义与焦点

```rust
use jasper_ui_engine::semantics::{FocusManager, Role, Semantics, SemanticsTree};

let scene = Scene::new(
    Node::rect(NodeId(2)).with_semantics(
        Semantics::default().role(Role::Button).label("OK").focusable(true),
    ),
);
let tree = SemanticsTree::from_scene(&scene);
let mut focus = FocusManager::new(&tree);
focus.focus_next();
```

- `Semantics` 由 Jasper 组件层填写；`SemanticsTree` 只是快照。
- `focusables()` 排除 `hidden`/`disabled`；顺序是 pre-order。
- `FocusManager::handle_key` 只处理 Tab/Shift+Tab（KeyPhase::Down）。
- AccessKit/平台无障碍桥未实现。

## 手势（参考实现）

```rust
use jasper_ui_engine::gesture::{Gesture, GestureRecognizer};

let mut recognizer = GestureRecognizer::default();
let gestures = recognizer.process(&route);   // PointerRouter 的输出
let timed = recognizer.tick(now_ms);         // 推进 long-press
```

- `Gesture`：Tap/LongPress/PanStart/Pan/PanEnd/PinchStart/Pinch/PinchEnd/Cancelled。
- `GestureConfig` 可调 tap/long-press/pan/pinch 阈值。
- 这是宿主中立参考实现；Jasper UI 层可替换手势语义与冲突规则。
- 不会自动处理“点击 vs 滚动”的手势竞技场；需要上层策略。

## 滚动控制

```rust
use jasper_ui_engine::scroll::ScrollController;

let layout = engine.layout(&scene)?;
let mut scroll = ScrollController::default();

// 滚轮/trackpad：正 delta 表示内容上移（offset 增加）。
// 返回从内到外的 0..n 条更新，可一次性原子应用。
let patches: Vec<_> = scroll
    .wheel(&layout, &scene, pointer_position, delta)
    .into_iter()
    .map(|update| update.patch())
    .collect();
if !patches.is_empty() {
    scene.apply_batch(&patches)?;
}

// 惯性：手势 PanEnd 时把释放速度交给 fling，按帧推进。
scroll.begin_fling(node, release_velocity);
for update in scroll.tick(&layout, &scene, dt_seconds) {
    scene.apply(update.patch())?;
}
```

- `ScrollBounds` 由布局树中该节点的直接子节点并集算出；`max_offset()` 是 clamp 上界。
- `wheel` 命中几何盒（透明容器也算），从最近的 `Overflow::Scroll` 祖先开始逐级向外消费；内层到边界后，残余 delta 继续交给外层，返回所有实际移动的节点（从内到外）。
- 每个轴独立拆分残余：例如内层纵向到边界后，纵向 remainder 交给外层；无容器可消费时返回空列表。
- 滚轮输入总会取消该链上的 fling；一次事件的多个 `SetScroll` 可用 `Scene::apply_batch` 合成一个版本。
- `tick` 用指数摩擦衰减速度，到边界或低于 `min_velocity` 即停止；`dt` 上限 0.1s。
- `ScrollUpdate::patch()` 产出普通的 `ScenePatch::SetScroll`，仍走版本/原子批；应用后应重新布局再继续命中/滚动。

### 滚动条

```rust
use jasper_ui_engine::scrollbar::{paint_scrollbars, scrollbars, ScrollbarConfig};

let config = ScrollbarConfig::default();
if let Some(bars) = scrollbars(&layout, &scene, node, config) {
    // 指针按下：只有落在 thumb 上才开始拖动（track 点击暂不跳转）。
    if let Some(drag) = bars.hit(pointer).and_then(|bar| bar.begin_drag(pointer)) {
        // 移动时把 update 走普通补丁；横轴分量保持不变。
        scene.apply(drag.update(pointer, scene.find(node).unwrap().scroll).patch())?;
    }
    // 绘制：先 paint，再追加到同一显示列表。
    let mut display = jasper_ui_engine::paint::paint(&layout, Color::WHITE);
    paint_scrollbars(&mut display, bars, config);
}
```

- `scrollbars` 只对 `Overflow::Scroll` 且该轴确实溢出的节点产出 bar；内容放得下时返回空 `Scrollbars`，不画假 thumb。
- `ScrollbarConfig`：`thickness`/`margin`/`min_thumb` 与 `track_color`/`thumb_color`；默认 6px 宽、24px 最小 thumb。
- `ScrollbarDrag` 保留按下时光标在 thumb 内的抓取偏移，拖动不会跳；越界自动 clamp 到 `max_offset()`。
- `paint_scrollbars` 会带上布局里的祖先裁剪（`PushClip/PopClip`），不新增场景节点。
- 未做：track 点击跳转、自动隐藏/淡出、过冲回弹、平台主题。

## Engine

- `LegacyEngine::new(size, backend, text)`：构造。
- `.with_clear_color(color)`：每帧清屏色。
- `.render(&scene)`：完整帧；返回 `FrameStats`。
- `.layout(&scene)`：只算布局（命中测试/诊断用）。
- `.resize(size)`：更新尺寸并让后端 resize。

## 错误

- `SceneError`：节点不存在/重复/删根/非文本。
- `LayoutError`：Taffy 或缺失节点。
- `BackendError`：Unsupported/Invalid/Io。
- `EngineError`：Layout/Backend。

## 稳定性

v0.1 原型：类型/字段/函数都可能改。稳定 ABI 见 [abi.md](abi.md)（尚未实现）。
# 场景有效性补充（2026-10-23）

外部输入优先通过 `Scene::try_new(root) -> Result<Scene, SceneError>` 构造；`Scene::validate()` 检查全树节点 ID 唯一性。兼容 `Scene::new` 和公开 `root` 保留，可能形成非法树；直接 find/contains 前调用方须确认输入已验证。布局入口与批更新入口重新校验；插入检查所有后代与当前暂存树，不能只看子树根 ID。非法输入不自动修复。

批次失败保持原场景与版本，包括先成功暂存文本修改、再插入重复后代的情况。匹配版本的空批不推进版本，但仍检查树有效性；陈旧版本优先拒绝，非空版本耗尽明确拒绝。同批先移除再插入同一 ID 允许，先插入重复 ID 再移除不允许。

版本准入内核已具备机器验证的数学证明；整个事务和节点唯一性实现尚未完成证明，不能以这些 API 或测试代替证明。细节见 [verification.md](verification.md)。


## 软件 Surface 行额度（Rust）

`jasper_ui_engine::surface::rgba8_row_bytes(width, max_bytes)` 对非空 RGBA8 紧密行返回精确字节数，仅当行长不超宿主给定额度时成功。零宽和超额返回 `None`，不分配内存，无窗口/GPU/系统字体依赖。例如宽度 16、额度 64 返回 `Some(64)`，额度 63 返回 `None`。

这是 UI Rust 宿主 Surface 接入的首个数值检查；不是完整 Surface 类型。高度、stride、总存储跨度、权限与生命周期仍需另行校验，现有 Canvas 构造/resize 尚未改接此接口。不要将行额度检查解释成整个帧缓冲分配已经安全。


`surface::surface_span(row_bytes, height, stride, capacity)` 校验非空几何、`stride >= row_bytes`，返回最后有效字节之后的终点：`(height - 1) * stride + row_bytes`。超额或算术溢出均返回 `None`。末行不要求补齐尾部 padding，例如行长 16、高度 3、stride 20、容量 56 成功，容量 55 拒绝；单行只需容量 16。

这只是格式无关的数值跨度检查；RGBA8 调用方先取得 `rgba8_row_bytes`。capacity 必须来自宿主可信的真实存储信息，不能以客户端自报容量代替存储、权限、生命周期及会话归属检查。旧 Canvas 构造/resize 尚未改接这套准入。


`surface::surface_row(bytes, row_bytes, height, stride, row)` 在验证整个声明 Surface 都能放入真实字节切片后，返回指定行的只读借用（不含 padding）。任何非法几何、容量不足、溢出或 row 越界均返回 `None`。即使第一行本身能放入缓冲，整个声明 Surface 不足时仍拒绝。

返回值生命周期绑定到输入 `bytes`，不分配、不复制；有效字节必须由宿主合法提供给 Rust。该 API 不接受裸指针，不代替 IPC 资源归属/权限/代际检查，也没有接管 OS 存储服务。


`surface::write_surface_row(bytes, row_bytes, height, stride, row, source)` 要求源切片恰为一行有效字节，完整 Surface 和行号均合法时同步复制一行，返回 true。成功只修改该行的有效字节，保留 padding 和其他行；任何准入失败返回 false 且目标保持不变。函数不分配内存，调用方必须提供合法且独占的 Rust 可变切片。

例如 `[1,2,170,187,3,4]`、行长 2、高度 2、stride 4、row 1，写入 `[9,8]` 后为 `[1,2,170,187,9,8]`。这不是跨进程原子发布操作，不代替资源权限、撤权、同步或生命周期管理。


`surface::Rgba8SurfaceView::new(bytes, width, height, stride)` 对真实宿主字节创建受检只读 RGBA8 视图。构造拒绝零宽/零高、stride 小于有效行宽、溢出或整体容量不足；布局字段私有且无可变访问器。公开 `width()` / `height()` / `stride()` 与 `row(index)`，后者只返回有效字节，不含 padding。

对象不分配、不复制，也不持有外部资源句柄。行借用绑定输入字节的生命周期，视图对象结束作用域后仍可使用行借用，前提是输入字节仍然有效。合法存储/权限/外部同步仍由宿主兑现 Rust 引用前提；该类型不是跨 IPC 资源所有权的替代品。


`Rgba8SurfaceView::copy_row_to(row, destination)` 将指定行的有效字节传输到调用方提供的缓冲，要求目标长度恰为有效行长。成功返回 true，目标精确等于源行；非法行或长度不匹配返回 false，目标完全保持原值。该方法不分配，也不复制源 Surface 的 padding；源视图保持只读，不代表 IPC 原子发布或外部权限校验。


`surface::rgb8_word(red, green, blue)` 产生数值 `0x00RRGGBB`，高字节为零；不混色、不定义原生内存字节序。该纯核心函数不依赖 desktop/winit，系统内置宿主也可按自身像素格式契约选用。

现有 `desktop::rgba_to_u32(pixels, out)` 使用同一编码；完整长度不匹配时输出完全保持，匹配时精确转换每个 RGB、保持既有忽略 alpha 的语义。转换不分配；普通同步写入不等同跨进程原子发布。只有 desktop 适配器依赖其可选平台库，核心编码没有引入窗口依赖。


## 宿主自备像素存储的 Canvas（Rust）

`jasper_ui_engine::raster::Canvas::from_rgba8(width, height, pixels: Vec<u8>) -> Result<Canvas, Vec<u8>>` 接管调用方已拥有的紧密 RGBA8 像素存储。只接受非零宽高且长度精确等于数学整数 `4 × width × height`；不足、额外尾部字节、零尺寸或无法表示的布局均拒绝并原样返还缓冲。成功保留像素，裁剪栈为空，`width()` / `height()` / `pixels()` 可直接读取；现有软件绘图方法可继续使用该 Canvas。

例如 `Canvas::from_rgba8(2, 2, vec![0; 16])` 成功，15 或 17 字节均拒绝。这条入口不分配或复制像素存储，所有权通过 Rust Vec 转移；它与会把零尺寸归一化的旧 `Canvas::new` 明确区分。输入不是裸指针或外部共享映射，必须已经是调用方合法拥有的 Vec；宿主负责此前的分配预算，不代表 IPC 权限、撤权或同步已经实现。旧 `new` 与后端 resize 的受检分配/错误传播仍待完成。


## UI 资源槽准入（Rust）

`resource::ResourceGate::new(session, slot)` 建立空闲单槽；`issue(caller_session)` 签发下一代 `ResourceKey`，`allows(caller_session, key)` 检查当前身份，`revoke(caller_session, key)` 关闭准入。错误调用不改变状态，耗尽不回绕，不分配实际资源。宿主必须提供可信会话身份、唯一且不重置的 gate；key 不是秘密能力凭证。完整边界见 resource-admission.md，真实资源服务/IPC/释放与在途取消尚未实现。


`resource::UiBytesSlot` 在同一持久身份后持有真实 Vec 字节：`install(caller_session, bytes)` 接管或原样返还，`bytes(caller_session, key)` 返回受检只读借用，`release(caller_session, key)` 成功时取回原 Vec。拒绝保持槽位内容，重装不接受旧代 key。适合后续 UI 字体等字节资源接入，不解析字体或创建 OS 句柄；调用方预算、可信会话和唯一槽位生命周期边界继续保留。


持有槽位的宿主可调用 `ResourceGate::retire()` 永久禁止该 gate 再次签发；`UiBytesSlot::retire()` 同时取回当前 Vec。重复调用保持退役，不能用 install 重新打开；正常可复用释放仍使用 release。该管理操作只用于宿主拥有的对象，不是公开给任意 IPC 客户端的授权接口，也不完成全局注销或在途取消。

### 取回失焦目标（Rust）

`KeyboardRouter::take_focus(&mut self) -> Option<NodeId>` 返回调用前的焦点并清空焦点；重复调用返回 None，保持原焦点遍历顺序，后续 Tab 仍可导航。同步方法不分配新列表，供宿主构造失焦意图；返回值不是 IME 预编辑归属，不代表取消事件已投递，也不封闭后续输入准入。原 focused/blur API 行为保持。实现验收状态以匹配当前源码的 keyboard-focus-kernel 与 validation-chain 报告为准。

### 宿主准入与焦点关闭步骤（Rust）

`KeyboardRouter::retire_focus(&mut self, gate: &mut ResourceGate) -> Option<NodeId>` 先永久退役传入 gate，再取回并清除焦点；保留遍历顺序，重复关闭返回 None。宿主必须唯一配对同一会话的 gate 与 router，并在全部事件入口检查 gate；方法不会替调用方识别归属，也不自动包装现有 route/route_text/route_composition。返回值仅可用于构造失焦意图，预编辑、指针取消及外部投递仍待接入。独占 Rust 借用只保证该同步调用，不代表 IPC 原子发布。验收依据为匹配源码的 input-retire-kernel 和 validation-chain。

### 宿主悬停退出（Rust）

`HoverTracker::retire_hover(&mut self, gate: &mut ResourceGate) -> Vec<HoverChange>` 先永久封闭传入 gate，再清理悬停并返回原目标的 Leave 意图；原来没有悬停时返回空列表。没有中间状态修改时重复调用为空。宿主负责同一会话的归属配对、所有输入入口准入检查，以及真实派发返回意图；此方法不清理 PointerRouter capture、PointerTracker 按压、手势或 IME。原 update/clear 的 Leave-before-Enter 和不变目标不重复发事件语义保持。验收以匹配源码的 hover-update-kernel/hover-retire-kernel 与 validation-chain 为准。

### 捕获目标查询（Rust）

`PointerRouter::captured_target(pointer_id) -> Option<NodeId>` 读取 capture 中保留的原目标，不按当前命中结果重定向。None 同时可能表示没有 capture、或按下起点在所有节点之外；用 has_capture 区分。查询不取消 capture，不修改跟踪表、路径或目标，也不校验会话身份。原 PointerId 的 u64 表示、相等、排序及 Hash 语义保持，相等/偏序/全序现采用显式实现以便对应数学规格。验收以当前 pointer-order-kernel/capture-query-kernel 与完整链证据为准。

该查询只保留捕获时保存的 NodeId；不保证节点在当前场景仍存活，也不能识别删除后复用同一 ID 的新实例。真实取消/投递仍须加入场景身份或版本的失效规则。

## UI 输入身份准入

`input_identity::{UiInputContext, UiInputTicket, accepts_ui_input}` 提供宿主无关的 UI 消费者准入谓词。宿主提供可信的当前目标 ID/目标代数、文档 ID/版本，以及独占生命周期的 ResourceGate；票据携带资源会话/槽位/代数与对应上下文。任何一维不匹配或 gate 已关闭都会拒绝。关闭焦点/会话时先 revoke/retire，再发布新绑定；不能仅更换 NodeId 后继续信任旧票据。

票据不是秘密凭证，调用方会话必须来自宿主认证上下文；当前上下文不能从请求中照抄。目标代数分配、身份不复用及当前绑定发布由宿主负责。此 API 只判断是否准入，不负责事件分发、原子编辑或取消已在运行的输入法任务。当前尚未把所有输入路由入口接到此 API，不能视为完整生命周期已实现。

`UiInputBinding` 持有持久 ResourceGate 与当前可选上下文。`new` 创建空闲绑定；`bind` 仅在所属会话且槽位空闲时签发票据并发布上下文，失败保持原状态；`accepts` 检查当前绑定；`close` 仅关闭精确票据，旧回调不能关闭新绑定；`retire` 由宿主永久停用。方法间的状态发布受 Rust 独占可变借用保护，不调用外部回调。关闭后可 bind 新文档版本，旧票据继续拒绝；宿主必须持久保留该对象，不得用同一会话/槽位重建以重置代数。该对象不代表实际异步任务已经取消，当前路由器还未自动接入。

`UiInputBinding::admit(caller_session, ticket, &payload)` 是统一的借用负载准入桥，可直接用于规范化 `InputEvent`。成功返回字段私有的 `AdmittedUiInput`，经 `payload()` 和 `target()` 读取原负载与准入目标；负载不复制、失败不调用消费者。返回值与其返回引用借用绑定，因此仍在消费时安全 Rust 不允许 close/rebind/retire。复制或脱离该借用后异步消费必须重新准入，不能把该桥当作任务取消。宿主关闭/退役等管理命令不强制经过已关闭的客户端票据门禁。桥只校验身份，不验证 IME 字符索引、指针数值或自动接管旧路由器。

`KeyboardRouter::route_admitted(&permit)` 接入真实键盘焦点消费者：仅当当前焦点 ID 与已准入目标一致时返回原负载借用；无焦点或不匹配拒绝。返回引用同时借用路由器与输入绑定，消费期间不能安全地变更焦点或退役绑定。此层仅过滤归属，不执行 KeyAction/文本编辑、角色策略或 IME 内容验证。原 route/route_text/route_composition 为兼容保留的可信底层入口，不会自动套用新门禁。宿主在焦点转移/节点重用时必须关闭/轮换绑定。

`KeyboardRouter::retire_input_binding(&mut binding)` 是宿主关闭路径：先永久退役绑定，后清空键盘焦点并返回原目标，保留焦点遍历顺序。此方法要求宿主提供归属匹配的对象，不认证全局身份，也不宣称外部任务或指针捕获已取消。

`UiInputBinding::replace(caller_session, old_ticket, next_context)` 在独占访问下原子轮换当前活动绑定，返回新的代数票据；成功后旧票据无权消费或关闭后继。错误所属方、过期上下文或代数耗尽时整份旧绑定保持不变，包括 MAX 最后一张有效票据：不会先 close 再因 issue 失败丢失旧状态。新上下文必须由可信宿主提供；此方法不移动 KeyboardRouter 焦点，跨对象焦点转移仍须使用后续协调入口，不是已完成的异步取消。

`KeyboardRouter::transfer_input_focus(binding, caller_session, old_ticket, next_context)` 协调普通焦点转移：旧票据目标必须仍为当前焦点，新目标必须在当前可聚焦顺序中，再轮换输入绑定并提交焦点。失败保持路由器焦点/顺序及完整绑定均不变，含代数耗尽；成功产生新票据，旧回调不能再提交或关闭后继。资格预检与提交在独占访问的未改变顺序上执行，无宿主回调，最终 focus 选择成功由精确规格保证。此入口用于已有活动绑定的转移，不创建初始绑定、不负责 Scene 节点代数分配或实际输入法/指针取消。

`KeyboardRouter::begin_input_focus(binding, caller, context)` 从无焦点/空闲绑定同步发布初始焦点和票据，拒绝不覆盖任何旧状态。`close_input_focus(binding, caller, ticket)` 正常关闭精确活动焦点与绑定，返回完整旧 `UiInputTicket` 作为取消意图，保留会话/槽位/代数、目标代数、文档/版本；不是只返回可能被复用的 NodeId。重复或旧票据关闭不会影响后继，关闭后可通过同一持久对象 begin 重开并增加代数，耗尽/永久退役则拒绝。宿主须把取消投向返回的旧归属，不将其解析为新场景节点；本 API 不投递、确认或终止编辑器/IME外部任务。

`composition_identity::{UiCompositionSlot, UiCompositionToken}` 只负责 UI 预编辑/候选视图的关联身份。`begin` 要求有效输入绑定、槽位空闲且序号未耗尽；每次组合独立递增序号，因此同一焦点票据的两次组合也不会混淆。`accepts` 同时检查当前绑定与完整原归属/组合序号。`cancel` 校验宿主认证会话及精确旧 token 后清除 UI 元数据，故意允许在焦点已关闭后清理旧视图；旧序号不能清除新组合。`retire` 永久退役并返还完整旧取消关联。

同一 UI 视图必须持久保留该槽位，旧回调仍可能存在时不得重建并重置序号。该类型不是通用 IME 引擎，不存储/清除外部组件的文本或像素，也不投递/确认原生任务取消；必须由编辑器/IME输出适配层在真正更新视图前使用它校验。

### UI composition replacement range admission

`UiCompositionSlot::admit_composition` combines current input ownership/composition
serial with the existing `CompositionEvent.start..end` UTF-8 **document replacement**
byte range. It returns the original borrowed event only when the document admits
that range at scalar boundaries. Host code must supply the current document for
that ticket's document/revision. The borrow ties consumption to the slot, binding,
and document; safe Rust cannot mutate those through the same owners while it lives.
This does not edit the document, validate preedit cursor/selection, or implement
native IME cancellation. `valid_composition_range` exposes the same range predicate.

**2026-10-24 15:42 UTC integration limitation:** current `DesktopInput::ime`
forwards preedit-local cursor offsets into legacy CompositionEvent, conflicting
with its declared document-offset semantics. Do not wire that raw event directly
to document-range admission. The mathematical range proof does not establish
producer/consumer coordinate agreement. Separate types/coordinates and preserve
absent cursor before completing native IME integration; see input-method-boundary.

### Explicit UI preedit-local payload

`PreeditEvent { text, cursor: Option<(usize,usize)> }` represents a cursor/selection
inside its own UTF-8 preedit string. `None` stays absent. It is a separate type from
legacy document-replacement `CompositionEvent`. `UiCompositionSlot::admit_preedit`
checks current owner/serial plus local scalar boundaries and returns the same
borrowed payload; it never applies an editor transaction. A nonempty preedit can
be valid when the editor document is empty. DesktopInput's legacy conversion still
needs migration; the new payload alone does not resolve that producer defect.

### Normalized host preedit admission

`desktop::normalize_ime` emits `InputEvent::Preedit { text, cursor }` for winit
preedit; `Composition` retains its legacy document-replacement meaning.
`UiCompositionSlot::admit_normalized_preedit` consumes that actual normalized
variant directly, validates local UTF-8 cursor bounds and the current owner/serial,
and returns the original borrowed event. Committed Text and all other variants
are rejected by this preedit-only entry point. An empty preedit with absent cursor
preserves clear intent; this does not itself clear a native editor or UI pixels.
The pure host conversion/admission path has independent scoped evidence. The
stateful DesktopInput wrapper's clock-related Kani failure remains pending.

### Owned UI preedit presentation state

`UiPreeditView` owns an optional `UiPreeditFrame { token, text, cursor }`.
`apply` moves an actual normalized Preedit payload into the view only after current
owner/serial and local UTF-8 checks; rejection preserves the old frame. `clear`
removes only the exact qualified owner/serial, including cleanup after focus closes.
Old cancellation cannot remove a later composition. `current` borrows presentation
data for the host; it is not an authorization check. Hosts must isolate views and
clear on closure/cancellation before rendering. This stores UI text only; it does
not alter committed editor text, native editor state or rendered pixels.

`UiCompositionSlot::cancel_preedit(view, caller, token)` pairs slot cancellation
with clearing the matching owned UI preedit frame. A rejected cancellation keeps
both objects unchanged. Success clears a matching frame and preserves a different
frame; cleanup works after the input binding closes. Use this paired entry point
instead of canceling the slot first and expecting a later repeated cancellation
to clear text. It does not deliver native cancellation or clear rendered pixels.

`KeyboardRouter::close_input_preedit(binding, slot, view, caller, token)` closes
an active preedit owner across focus, input binding, composition slot and owned UI
text. Rejection preserves all four; success returns the full old composition token
as native cancellation intent. Only a matching frame is cleared. A late old close
cannot close a reopened successor. For a focus with no active composition use
ordinary `close_input_focus`. This operation performs no external task delivery,
acknowledgement or rendered-pixel update.

`InputEvent::ImeEnabled(bool)` now preserves native IME availability events.
`normalize_ime` forwards Enabled/Disabled instead of dropping them.
For an active composition, `KeyboardRouter::handle_preedit_lifecycle` consumes a
qualified event/token: Focus(false) closes focus/binding/slot/matching UI text;
ImeEnabled(false) cancels only composition/UI text and preserves typing focus and
binding. Enabled, Focus(true), other events and rejected ownership leave all state
unchanged. Returned tokens are cancellation intent. No-composition focus loss still
uses ordinary focus closure; this API does not automatically start an IME engine.

`KeyboardRouter::handle_input_lifecycle` is the qualified entry point for idle and
composing input. It checks the supplied current ticket before reading composition
correlation, so an old focus event cannot acquire a new owner's token. Success
returns `(old_ticket, optional_composition_token)` as cancellation intent. Idle
focus loss closes ordinary input; active events retain the earlier disabled/lost
focus distinction. An idle slot with orphaned presentation rejects unchanged until
explicit qualified cleanup. `UiCompositionSlot::current_token` exposes read-only
correlation metadata and provides no independent authorization.
