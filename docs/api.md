# Rust API（v0.1 原型）

> 这是当前 crate 的公开 API 说明；**不是稳定 ABI**。原型阶段允许破坏性修改。

## 快速开始

```rust
use jasper_ui_engine::backend::HeadlessBackend;
use jasper_ui_engine::engine::Engine;
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
- `ScenePatch` 当前不是事务；失败可能已部分应用，批量/原子 API 后续补。

## Style / Layout

- 尺寸：`Length::Auto/Px/Percent`；`min_*`/`max_*`。
- 布局：`Display::{Block,Flex,None}`、`FlexDirection::{Row,Column}`、`Align`、`Justify`、`gap`、`flex_grow`。
- 间距：`padding`、`margin`、`border`。
- 文字：`font_size`、`color`（测量参与布局；光栅未实现）。
- 当前不支持阴影/滤镜/变换/滚动/虚拟化。

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
- 手势、命中测试、IME 组合尚未实现。

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

## Engine

- `Engine::new(size, backend, text)`：构造。
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
