//! jasper-ui-engine: host-agnostic foundation for the Jasper UI native engine.
//!
//! 这个 crate 只负责底层执行：场景、布局、绘制、UiBatch、软件光栅、输入模型与后端接口。
//! Jasper UI Framework、UI IR、Jasper ABI 与 Web 后端不在这里。
//!
//! ```no_run
//! // Explicit compatibility example; native callers use engine::Engine with native-blitz-pending.
//! // 显式兼容示例；原生调用方启用 native-blitz-pending 后使用 engine::Engine。
//! use jasper_ui_engine::engine::LegacyEngine as Engine;
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
pub mod composition_identity;
#[cfg(feature = "canvas-tiny-skia")]
pub mod canvas_tiny_skia;
#[cfg(feature = "desktop")]
pub mod desktop;
pub mod engine;
mod legacy_engine;
pub mod geometry;
pub mod gesture;
#[cfg(feature = "gpu")]
pub mod gpu_wgpu;
pub mod graphics;
pub mod hit_test;
pub mod input;
pub mod input_identity;
pub mod keyboard;
pub mod layout;
pub mod paint;
pub mod platform;
pub mod pointer;
pub mod raster;
pub mod resource;
pub mod scene;
pub mod scroll;
pub mod scrollbar;
pub mod semantics;
pub mod style;
pub mod surface;
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
pub use engine::{LegacyEngine, EngineError, FrameStats};
#[cfg(feature="native-blitz-pending")]
pub use engine::Engine;
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


// Pending third-party text adapter; full ordered verification remains required.
#[cfg(feature = "text-parley-pending")]
pub mod text_parley;

// Root-owned native implementation; the diagnostic crate only consumes its facade.
// 原生实现归根 crate 所有，诊断 crate 仅消费公开封装。
#[cfg(feature="native-blitz-pending")]
extern crate self as jasper_ui_engine;
#[cfg(feature="native-blitz-pending")]
#[path="native_adapter/native_view.rs"]
mod native_view;
#[cfg(feature="native-blitz-pending")]
#[path="native_adapter/component_contract.rs"]
mod component_contract;
#[cfg(feature="native-blitz-pending")]
#[path="native_adapter/update_contract.rs"]
mod update_contract;
#[cfg(feature="native-blitz-pending")]
#[path="native_adapter/native_fonts.rs"]
mod native_fonts;
#[cfg(feature="native-blitz-pending")]
#[path="native_adapter/cpu_budget.rs"]
mod cpu_budget;
#[cfg(feature="cpu-pending")]
#[path="native_adapter/frame_schedule.rs"]
mod frame_schedule;
#[cfg(feature="gpu-pending")]
#[path="native_adapter/gpu_render.rs"]
mod gpu_render;
#[cfg(feature="native-blitz-pending")]
#[path="native_adapter/text_geometry.rs"]
mod text_geometry;
#[cfg(feature="native-blitz-pending")]
#[path="native_adapter/access_tree.rs"]
mod access_tree;
#[cfg(feature="accesskit-pending")]
#[path="native_adapter/access_kit.rs"]
mod access_kit;
#[cfg(feature="native-blitz-pending")]
#[path="native_adapter/capture_policy.rs"]
mod capture_policy;
#[cfg(feature="native-blitz-pending")]
#[path="native_adapter/native_scene.rs"]
mod native_scene;
#[cfg(feature="native-blitz-pending")]
#[path="native_adapter/scene_events.rs"]
mod scene_events;
#[cfg(feature="native-blitz-pending")]
#[path="native_adapter/pointer_host.rs"]
mod pointer_host;
#[cfg(feature="native-blitz-pending")]
#[path="native_adapter/native_events.rs"]
mod native_events;
#[cfg(feature="native-blitz-pending")]
#[path="native_adapter/native_keys.rs"]
mod native_keys;
#[cfg(feature="native-blitz-pending")]
#[path="native_adapter/native_semantics.rs"]
mod native_semantics;
#[cfg(feature="native-blitz-pending")]
#[path="native_adapter/view_slot.rs"]
mod view_slot;
#[cfg(feature="native-blitz-pending")]
pub mod native {
    use crate::{native_view,component_contract,update_contract,
        native_scene,scene_events,pointer_host,native_events,native_keys,native_semantics};
    /// Per-view limit for `engine::Engine::with_fonts`.
    /// `engine::Engine::with_fonts` 每视图注入字体文件上限。
    pub use crate::native_fonts::MAX_INJECTED_FONTS;
    include!("native_adapter/facade.rs");
}
