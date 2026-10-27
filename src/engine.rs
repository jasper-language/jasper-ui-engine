//! Canonical typed native engine entry; old execution is explicit compatibility only.
//! 正式类型化原生引擎入口；旧执行链仅通过显式兼容名称提供。

/// Compatibility-only Taffy/PaintOp engine and its old diagnostics.
/// 仅供历史兼容的 Taffy/PaintOp 引擎及旧诊断，不是原生执行路径。
pub use crate::legacy_engine::{Engine as LegacyEngine, EngineError, FrameStats};

/// Pinned native engine consuming typed scenes/updates and injected host resources.
/// 固定原生引擎，消费类型化场景/更新与宿主注入资源；完整验收尚未完成。
#[cfg(feature="native-blitz-pending")]
pub use crate::native::host::SceneHostView as Engine;

/// Compatibility name for callers that used the first native candidate entry.
/// 兼容首版原生候选调用方的名称，与 Engine 是同一实现。
#[cfg(feature="native-blitz-pending")]
pub use Engine as BlitzEngine;
