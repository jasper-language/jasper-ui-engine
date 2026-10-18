//! jasper-ui-engine: host-agnostic foundation for the Jasper UI native engine.
//!
//! 这个 crate 只负责底层执行：场景、布局、绘制、UiBatch、软件光栅、输入模型与后端接口。
//! Jasper UI Framework、UI IR、Jasper ABI 与 Web 后端不在这里。
//!
//! ```no_run
//! use jasper_ui_engine::engine::Engine;
//! use jasper_ui_engine::backend::HeadlessBackend;
//! use jasper_ui_engine::geometry::Size;
//! use jasper_ui_engine::scene::{Node, NodeId, Scene};
//! use jasper_ui_engine::style::{Color, Length, Style};
//! use jasper_ui_engine::text::SimpleTextEngine;
//!
//! let scene = Scene::new(
//!     Node::container(NodeId(1))
//!         .with_style(Style::default().size(Length::Px(64.0), Length::Px(64.0)).background(Color::RED)),
//! );
//! let mut engine = Engine::new(
//!     Size::new(64.0, 64.0),
//!     HeadlessBackend::new(64, 64),
//!     Box::new(SimpleTextEngine),
//! );
//! engine.render(&scene).unwrap();
//! ```

pub mod backend;
pub mod batch;
pub mod canvas;
#[cfg(feature = "canvas-tiny-skia")]
pub mod canvas_tiny_skia;
#[cfg(feature = "desktop")]
pub mod desktop;
pub mod engine;
pub mod geometry;
pub mod gesture;
#[cfg(feature = "gpu")]
pub mod gpu_wgpu;
pub mod graphics;
pub mod hit_test;
pub mod input;
pub mod keyboard;
pub mod layout;
pub mod paint;
pub mod platform;
pub mod pointer;
pub mod raster;
pub mod scene;
pub mod scroll;
pub mod scrollbar;
pub mod semantics;
pub mod style;
pub mod text;
#[cfg(feature = "real-text")]
pub mod text_cosmic;

pub use backend::{BackendError, RenderBackend};
pub use batch::{BatchError, UiBatch};
pub use canvas::{Canvas, CanvasHitRegion, CanvasOp};
#[cfg(feature = "canvas-tiny-skia")]
pub use canvas_tiny_skia::{render_canvas, CanvasRenderReport};
#[cfg(feature = "desktop")]
pub use desktop::{rgba_to_u32, DesktopInput, PixelError};
pub use engine::{Engine, EngineError, FrameStats};
pub use geometry::{Edges, Point, Rect, Size};
pub use gesture::{Gesture, GestureConfig, GestureRecognizer};
#[cfg(feature = "gpu")]
pub use gpu_wgpu::{
    adapter_kind, adapters as wgpu_adapters, describe as describe_wgpu_adapter,
    instance as wgpu_instance, select as select_wgpu_adapter,
};
pub use graphics::{
    select_adapter, AdapterInfo, AdapterKind, AdapterSelection, GpuPreference, GraphicsSettings,
    SelectionFallback,
};
pub use hit_test::{hit_test as hit_test_at, hit_test_path};
pub use input::{InputEvent, PointerEvent, PointerKind, PointerPhase, PointerTracker};
pub use keyboard::{CompositionRoute, KeyAction, KeyRoute, KeyboardRouter, TextRoute};
pub use layout::{LayoutBox, LayoutEngine, LayoutError, LayoutTree, TaffyLayoutEngine};
pub use paint::{paint, DisplayList, PaintOp};
pub use pointer::{HoverChange, HoverTracker, PointerRoute, PointerRouter};
pub use scene::{Node, NodeId, NodeKind, Scene, SceneError, ScenePatch};
pub use scroll::{
    scroll_bounds, scroll_chain, ScrollBounds, ScrollConfig, ScrollController, ScrollUpdate,
};
pub use scrollbar::{
    paint_scrollbars, scrollbars, ScrollAxis, Scrollbar, ScrollbarConfig, ScrollbarDrag, Scrollbars,
};
pub use semantics::{FocusError, FocusManager, Role, Semantics, SemanticsNode, SemanticsTree};
pub use style::{Align, Color, Display, FlexDirection, Justify, Length, Style};
pub use text::{SimpleTextEngine, TextEngine, TextMetrics};
#[cfg(feature = "real-text")]
pub use text_cosmic::CosmicTextEngine;
