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
pub mod engine;
pub mod geometry;
pub mod input;
pub mod layout;
pub mod paint;
pub mod platform;
pub mod raster;
pub mod scene;
pub mod style;
pub mod text;

pub use backend::{BackendError, RenderBackend};
pub use batch::{BatchError, UiBatch};
pub use engine::{Engine, EngineError, FrameStats};
pub use geometry::{Edges, Point, Rect, Size};
pub use input::{InputEvent, PointerEvent, PointerKind, PointerPhase, PointerTracker};
pub use layout::{LayoutBox, LayoutEngine, LayoutError, LayoutTree, TaffyLayoutEngine};
pub use paint::{paint, DisplayList, PaintOp};
pub use scene::{Node, NodeId, NodeKind, Scene, SceneError, ScenePatch};
pub use style::{Align, Color, Display, FlexDirection, Justify, Length, Style};
pub use text::{SimpleTextEngine, TextEngine, TextMetrics};
