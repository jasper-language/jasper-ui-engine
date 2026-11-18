//! Rendering-host coordinator around the canonical native engine.
//! 正式原生引擎之上的渲染宿主协调器。
//!
//! It owns no window, native Surface, GPU device or swapchain. It binds host-owned resources to
//! authenticated owner, version and presentation generations, and coordinates pause/resume/retire.
//! 它不持有窗口、原生 Surface、GPU 设备或交换链；只把宿主持有资源绑定到已认证归属、版本与
//! 呈现代数，并协调暂停、恢复和退休。
use crate::image_resource::{ImageError, ImageInfo, ImageLimits, ImageReport};
use crate::native::host::SceneHostView;
#[cfg(feature = "gpu-pending")]
use crate::native::host::{GpuFrameError, GpuRenderer};
use crate::native::{
    EventReply, PointerAction, SceneEventKind, SceneFocusEvent, SceneKeyEvent, SceneQuota,
    SceneUpdate, SceneUpdateError,
};
use crate::scene::{NodeId, Scene, ScenePatch};
#[cfg(feature = "cpu-pending")]
use crate::surface_lifecycle::RenderedPixels;
use crate::surface_lifecycle::{
    SurfaceCoordinator, SurfaceLifecycleError, SurfaceStatus, SurfaceStepError,
};
use crate::typed_accessibility::{
    typed_accessibility_tree as project_typed_accessibility, TypedAccessibilityError,
    TypedAccessibilityTree, TypedAccessibilityUpdate,
};
use crate::ui_ir::{
    AdmittedEvent, NativeSceneUpdate, NativeUiDocument, NativeUiFrameError, ResourceKind,
    ResourceRef, UiEventAdmissionError, UiEventContext, UiEventKind, UiFrame, UiKey, UiPermission,
};
use std::collections::BTreeMap;

/// Host coordinator failure. A lifecycle rejection precedes all native engine work.
/// 宿主协调失败；生命周期拒绝先于任何原生引擎工作。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostRenderError {
    Lifecycle(SurfaceLifecycleError),
    ViewportRejected,
    ViewRetired,
}

impl std::fmt::Display for HostRenderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lifecycle(error) => error.fmt(formatter),
            Self::ViewportRejected => write!(formatter, "native viewport or DPI was rejected"),
            Self::ViewRetired => write!(formatter, "native view is no longer live"),
        }
    }
}

impl std::error::Error for HostRenderError {}

/// Cross-layer Typed UI IR rejection: mapping happens before the host gate and neither side
/// commits unless both accept the frame.
#[derive(Clone, Debug, PartialEq)]
pub enum TypedUiApplyError {
    Frame(NativeUiFrameError),
    Host(SceneUpdateError),
    SurfaceRejected,
}

impl std::fmt::Display for TypedUiApplyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Frame(error) => write!(formatter, "Typed UI IR frame rejected: {error}"),
            Self::Host(error) => {
                write!(formatter, "Native host rejected Typed UI IR frame: {error}")
            }
            Self::SurfaceRejected => {
                write!(formatter, "host Surface generation rejected Typed UI IR frame")
            }
        }
    }
}

impl std::error::Error for TypedUiApplyError {}

/// Rejection of a host resource upload that is not the current committed Typed IR resource.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypedUiResourceError {
    HostRevision { document: u64, current: Option<u64> },
    ResourceNotCommitted { id: String, generation: u64 },
    ResourceStillActive { id: String },
    UnsupportedKind(ResourceKind),
    Image(ImageError),
}

impl std::fmt::Display for TypedUiResourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HostRevision { document, current } => write!(
                formatter,
                "Typed IR document revision {document} does not match host revision {current:?}"
            ),
            Self::ResourceNotCommitted { id, generation } => write!(
                formatter,
                "resource {id:?} generation {generation} is not the committed Typed IR resource"
            ),
            Self::ResourceStillActive { id } => {
                write!(
                    formatter,
                    "resource {id:?} is still active in the Typed IR document"
                )
            }
            Self::UnsupportedKind(kind) => {
                write!(
                    formatter,
                    "resource kind {kind:?} cannot be uploaded as an image"
                )
            }
            Self::Image(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for TypedUiResourceError {}

/// Structured failure while projecting host-gated accessibility data to public UiKeys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypedUiAccessibilityError {
    HostRejected,
    RevisionMismatch { host: u64, document: u64 },
    InvalidUpdateSequence,
    Projection(TypedAccessibilityError),
}

impl std::fmt::Display for TypedUiAccessibilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HostRejected => write!(f, "host surface generation rejected accessibility read"),
            Self::RevisionMismatch { host, document } => write!(
                f,
                "host accessibility revision {host} does not match document revision {document}"
            ),
            Self::InvalidUpdateSequence => {
                write!(
                    f,
                    "accessibility snapshots do not form one valid ordered update"
                )
            }
            Self::Projection(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for TypedUiAccessibilityError {}

/// Typed target rejection for focus, text input and IME operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypedUiTargetError {
    HostRejected,
    RevisionMismatch { host: u64, document: u64 },
    UnmappedTarget,
    Admission(UiEventAdmissionError),
}

impl std::fmt::Display for TypedUiTargetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HostRejected => write!(f, "Native host rejected Typed UI target operation"),
            Self::RevisionMismatch { host, document } => write!(
                f,
                "host revision {host} does not match document revision {document}"
            ),
            Self::UnmappedTarget => {
                write!(f, "Native event target has no active Typed UI key")
            }
            Self::Admission(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for TypedUiTargetError {}

/// Typed geometry-query rejection before exposing a backend NodeId.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypedUiGeometryError {
    HostRejected,
    RevisionMismatch { host: u64, document: u64 },
    UnknownKey(UiKey),
    UnmappedTarget,
}

impl std::fmt::Display for TypedUiGeometryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HostRejected => write!(f, "host surface generation rejected geometry query"),
            Self::RevisionMismatch { host, document } => write!(
                f,
                "host revision {host} does not match document revision {document}"
            ),
            Self::UnknownKey(key) => write!(
                f,
                "Typed UI key {}:{} is not active",
                key.semantic_id, key.generation
            ),
            Self::UnmappedTarget => {
                write!(f, "Native geometry target has no active Typed UI key")
            }
        }
    }
}

impl std::error::Error for TypedUiGeometryError {}

/// Pointer hit/routing rejection before a backend NodeId can cross the host boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypedUiPointerError {
    HostRejected,
    RevisionMismatch { host: u64, document: u64 },
    InvalidPoint,
    UnmappedTarget,
    Admission(UiEventAdmissionError),
}

impl std::fmt::Display for TypedUiPointerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HostRejected => write!(formatter, "host surface generation rejected pointer routing"),
            Self::RevisionMismatch { host, document } => write!(
                formatter,
                "host revision {host} does not match document revision {document}"
            ),
            Self::InvalidPoint => write!(formatter, "pointer sample is not finite"),
            Self::UnmappedTarget => {
                write!(formatter, "Native pointer target has no active Typed UI key")
            }
            Self::Admission(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for TypedUiPointerError {}

/// One backend-routed pointer event projected to public identity. `admitted` is present only
/// when the Typed frame binds the corresponding event and the host context authorizes it.
#[derive(Clone, Debug, PartialEq)]
pub struct TypedUiPointerEvent {
    pub target: UiKey,
    pub kind: SceneEventKind,
    pub admitted: Option<AdmittedEvent>,
    pub position: (f32, f32),
    pub primary_pressed: bool,
    pub modifiers: crate::input::Modifiers,
}

/// A focus transition projected to public identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypedUiFocusEvent {
    pub target: UiKey,
    pub focused: bool,
    pub admitted: Option<AdmittedEvent>,
}

/// A command-key event projected to public identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypedUiKeyEvent {
    pub target: UiKey,
    pub key: String,
    pub code: String,
    pub pressed: bool,
    pub repeat: bool,
    pub composing: bool,
    pub modifiers: crate::input::Modifiers,
    pub location: crate::native::KeyLocation,
    pub admitted: Option<AdmittedEvent>,
}

/// A host-admitted activation projected to public identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypedUiActivationEvent {
    pub target: UiKey,
}

/// An E14 edit event projected to public identity. `admitted` is present only when the event
/// maps to a bound `text-commit` or `ime-cancel` and the host context authorizes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypedUiEditEvent {
    pub target: UiKey,
    pub kind: crate::native::SceneEditEventKind,
    pub admitted: Option<AdmittedEvent>,
}

/// Current E14 edit state projected to public identity.
#[derive(Clone, Debug, PartialEq)]
pub struct TypedUiEditState {
    pub target: UiKey,
    pub text: String,
    pub anchor: usize,
    pub caret: usize,
    pub preedit: Option<String>,
    pub caret_rect: Option<crate::geometry::Rect>,
}

/// One Native overlay description projected to public identity. Overlay behavior remains a
/// candidate capability even though its identities are Typed.
#[derive(Clone, Debug, PartialEq)]
pub struct TypedUiOverlayInfo {
    pub target: UiKey,
    pub rank: usize,
    pub spec: crate::native::OverlaySpec,
    pub placement: Option<crate::native::OverlayPlacement>,
    pub suppressed: Option<crate::native::OverlaySuppression>,
    pub return_focus: Option<UiKey>,
}

/// One measured table cell projected to public identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypedUiTableCellMetrics {
    pub target: UiKey,
    pub row: usize,
    pub column: usize,
    pub rect: crate::geometry::Rect,
}

/// Measured table geometry with public table/cell identities.
#[derive(Clone, Debug, PartialEq)]
pub struct TypedUiTableMetrics {
    pub target: UiKey,
    pub rect: crate::geometry::Rect,
    pub column_widths: Vec<f32>,
    pub row_heights: Vec<f32>,
    pub cells: Vec<TypedUiTableCellMetrics>,
}

/// One overlay dismiss request projected to public identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypedUiOverlayEvent {
    pub target: UiKey,
    pub kind: crate::native::OverlayEventKind,
    pub admitted: Option<AdmittedEvent>,
}

/// A host-detected scroll delta projected to public identity and an admitted binding.
#[derive(Clone, Debug, PartialEq)]
pub struct TypedUiScrollEvent {
    pub target: UiKey,
    pub delta: crate::geometry::Point,
    pub admitted: AdmittedEvent,
}

/// Failure while admitting a scroll event before Framework code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypedUiScrollError {
    HostRejected,
    RevisionMismatch { host: u64, document: u64 },
    InvalidDelta,
    Admission(UiEventAdmissionError),
}

impl std::fmt::Display for TypedUiScrollError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HostRejected => write!(formatter, "host rejected Typed scroll event"),
            Self::RevisionMismatch { host, document } => write!(
                formatter,
                "host revision {host} does not match document revision {document}"
            ),
            Self::InvalidDelta => write!(formatter, "scroll delta is zero or non-finite"),
            Self::Admission(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for TypedUiScrollError {}

/// Failure while querying or draining candidate overlay data for Typed UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypedUiOverlayError {
    HostRejected,
    RevisionMismatch { host: u64, document: u64 },
    FutureRevision { event: u64, document: u64 },
    UnmappedTarget,
    Admission(UiEventAdmissionError),
}

impl std::fmt::Display for TypedUiOverlayError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HostRejected => write!(formatter, "host rejected Typed overlay operation"),
            Self::RevisionMismatch { host, document } => write!(
                formatter,
                "host revision {host} does not match document revision {document}"
            ),
            Self::FutureRevision { event, document } => write!(
                formatter,
                "overlay event revision {event} is newer than document revision {document}"
            ),
            Self::UnmappedTarget => {
                write!(formatter, "Native overlay identity has no active Typed UI key")
            }
            Self::Admission(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for TypedUiOverlayError {}

/// Failure while draining Native edit events into Typed identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypedUiEditError {
    HostRejected,
    FutureRevision { event: u64, document: u64 },
    UnmappedTarget,
    Admission(UiEventAdmissionError),
}

impl std::fmt::Display for TypedUiEditError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HostRejected => write!(formatter, "Native host rejected Typed edit event drain"),
            Self::FutureRevision { event, document } => write!(
                formatter,
                "edit event revision {event} is newer than document revision {document}"
            ),
            Self::UnmappedTarget => {
                write!(formatter, "Native edit target has no active Typed UI key")
            }
            Self::Admission(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for TypedUiEditError {}

/// Typed AccessKit action result after UiKey and permission admission.
#[cfg(feature = "accesskit-pending")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypedAccessibilityAction {
    Focused(UiKey),
    Activated(AdmittedEvent),
}

#[cfg(feature = "accesskit-pending")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypedUiAccessibilityActionError {
    HostRejected,
    ActionNotSupported,
    UnmappedTarget,
    Admission(UiEventAdmissionError),
}

#[cfg(feature = "accesskit-pending")]
impl std::fmt::Display for TypedUiAccessibilityActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HostRejected => write!(f, "host rejected accessibility action"),
            Self::ActionNotSupported => {
                write!(f, "accessibility action is not supported by Typed UI")
            }
            Self::UnmappedTarget => {
                write!(f, "Native accessibility target has no active Typed UI key")
            }
            Self::Admission(error) => error.fmt(f),
        }
    }
}

#[cfg(feature = "accesskit-pending")]
impl std::error::Error for TypedUiAccessibilityActionError {}

impl From<SurfaceLifecycleError> for HostRenderError {
    fn from(error: SurfaceLifecycleError) -> Self {
        Self::Lifecycle(error)
    }
}

/// Name what the engine's refusal means for this step.
/// 说明引擎拒绝对这一步意味着什么。
fn step_error(error: SurfaceStepError, engine_rejected: HostRenderError) -> HostRenderError {
    match error {
        SurfaceStepError::Lifecycle(error) => HostRenderError::Lifecycle(error),
        SurfaceStepError::EngineRejected => engine_rejected,
    }
}

/// One native view plus its host-owned presentation lifecycle.
/// 一个原生视图及其宿主持有的呈现生命周期。
pub struct HostRenderer {
    owner: u64,
    inner: SceneHostView,
    surface: SurfaceCoordinator,
}

impl HostRenderer {
    pub fn new(owner: u64, scene: &Scene, font: &[u8], width: u32, height: u32) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }
        Some(Self {
            owner,
            inner: SceneHostView::new(owner, scene, font, width, height)?,
            surface: SurfaceCoordinator::new(owner, width, height),
        })
    }

    /// Construct from an admitted Typed UI IR document and apply its initial reduce-motion state.
    pub fn new_ui_document(
        owner: u64,
        document: &NativeUiDocument,
        font: &[u8],
        width: u32,
        height: u32,
    ) -> Option<Self> {
        let mut host = Self::new(owner, document.scene(), font, width, height)?;
        if let Some(on) = document.environment().reduce_motion {
            if !host.set_reduce_motion(owner, on) {
                return None;
            }
        }
        Some(host)
    }

    pub fn with_fonts(
        owner: u64,
        scene: &Scene,
        fonts: &[&[u8]],
        width: u32,
        height: u32,
    ) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }
        Some(Self {
            owner,
            inner: SceneHostView::with_fonts(owner, scene, fonts, width, height)?,
            surface: SurfaceCoordinator::new(owner, width, height),
        })
    }

    pub fn with_fonts_ui_document(
        owner: u64,
        document: &NativeUiDocument,
        fonts: &[&[u8]],
        width: u32,
        height: u32,
    ) -> Option<Self> {
        let mut host = Self::with_fonts(owner, document.scene(), fonts, width, height)?;
        if let Some(on) = document.environment().reduce_motion {
            if !host.set_reduce_motion(owner, on) {
                return None;
            }
        }
        Some(host)
    }

    pub fn surface_status(&self, caller: u64) -> Option<SurfaceStatus> {
        self.surface.status(caller)
    }

    pub fn needs_frame(&self, owner: u64, generation: u64) -> Option<bool> {
        if !self.surface.render_allowed(owner, generation) {
            return None;
        }
        Some(self.surface.present_pending() || self.inner.needs_frame(self.owner)?)
    }

    /// Reconfigure the engine viewport and return the new surface generation. Validation failure
    /// leaves both generations unchanged. The host must replace its own surface first.
    /// 重配引擎 viewport 并返回后继 Surface 代数；校验失败时两代均保持不变。宿主须先替换自身 Surface。
    pub fn reconfigure(
        &mut self,
        owner: u64,
        generation: u64,
        width: u32,
        height: u32,
        scale: f32,
    ) -> Result<u64, HostRenderError> {
        let inner = &mut self.inner;
        self.surface
            .reconfigure(owner, generation, width, height, || {
                if width == 0 || height == 0 || !inner.set_viewport(owner, width, height, scale) {
                    return false;
                }
                // A host Surface replacement invalidates its old pixel copy, not only layout.
                // 宿主替换 Surface 时也必须废弃旧像素副本，而不仅是重新布局。
                inner.invalidate_frame(owner);
                true
            })
            .map_err(|error| step_error(error, HostRenderError::ViewportRejected))
    }

    /// Host releases window/Surface/device resources. Scene and semantic state survive.
    /// 宿主释放窗口、Surface 与设备资源；场景与语义状态保留。
    pub fn suspend(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        focus: &mut impl FnMut(SceneFocusEvent),
    ) -> Result<u64, HostRenderError> {
        let inner = &mut self.inner;
        self.surface
            .suspend(owner, generation, || {
                inner.deactivate(owner, version, focus)
            })
            .map_err(|error| step_error(error, HostRenderError::ViewRetired))
    }

    /// Host recreated resources. The viewport metadata is updated before activation becomes
    /// visible in the successor generation.
    /// 宿主已重建资源；在后继代数可见前更新 viewport 元数据。
    pub fn resume(
        &mut self,
        owner: u64,
        generation: u64,
        width: u32,
        height: u32,
        scale: f32,
    ) -> Result<u64, HostRenderError> {
        let inner = &mut self.inner;
        self.surface
            .resume(owner, generation, width, height, || {
                if width == 0 || height == 0 || !inner.set_viewport(owner, width, height, scale) {
                    return false;
                }
                inner.invalidate_frame(owner);
                true
            })
            .map_err(|error| step_error(error, HostRenderError::ViewportRejected))
    }

    /// Retire native ownership after the host observed its final lifecycle generation.
    /// 在宿主观察到最终生命周期代数后退休原生所有权。
    pub fn retire(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        focus: &mut impl FnMut(SceneFocusEvent),
    ) -> Result<(), HostRenderError> {
        let inner = &mut self.inner;
        self.surface
            .retire(owner, generation, || {
                inner.close_with_focus_events(owner, version, focus)
            })
            .map_err(|error| step_error(error, HostRenderError::ViewRetired))
    }

    /// Retire without an epoch. Preferred only for error paths that do not own a current surface.
    /// 不带代数的兼容退休；仅建议用于不持有当前 Surface 的错误清理路径。
    pub fn close(
        &mut self,
        owner: u64,
        version: u64,
        focus: &mut impl FnMut(SceneFocusEvent),
    ) -> bool {
        let inner = &mut self.inner;
        self.surface.close(owner, || {
            inner.close_with_focus_events(owner, version, focus)
        })
    }

    pub fn apply(&mut self, owner: u64, expected: u64, next: u64, patches: &[ScenePatch]) -> bool {
        if !self.surface.active_for_owner(owner) {
            return false;
        }
        let applied = self.inner.apply(owner, expected, next, patches);
        if applied {
            self.surface.frame_superseded();
        }
        applied
    }

    pub fn try_apply(
        &mut self,
        owner: u64,
        expected: u64,
        next: u64,
        patches: &[ScenePatch],
    ) -> Result<(), SceneUpdateError> {
        if !self.surface.active_for_owner(owner) {
            return Err(SceneUpdateError::Closed);
        }
        let result = self.inner.try_apply(owner, expected, next, patches);
        if result.is_ok() {
            self.surface.frame_superseded();
        }
        result
    }

    /// Apply a Typed UI IR successor through the same surface-activity gate and native
    /// transaction as a raw ScenePatch frame.
    pub fn try_apply_ui_ir(
        &mut self,
        owner: u64,
        update: &NativeSceneUpdate,
    ) -> Result<(), SceneUpdateError> {
        self.try_apply(
            owner,
            update.expected_revision,
            update.next_revision,
            &update.patches,
        )
    }

    pub fn try_apply_ui_ir_at(
        &mut self,
        owner: u64,
        generation: u64,
        update: &NativeSceneUpdate,
    ) -> Result<(), TypedUiApplyError> {
        if !self.surface.render_allowed(owner, generation) {
            return Err(TypedUiApplyError::SurfaceRejected);
        }
        let result = self.inner.try_apply(
            owner,
            update.expected_revision,
            update.next_revision,
            &update.patches,
        );
        if result.is_ok() {
            self.surface.frame_superseded();
        }
        result.map_err(TypedUiApplyError::Host)
    }

    /// Atomically map and apply a Framework frame. The document is advanced only after the
    /// current Surface generation accepts the projected Native transaction.
    pub fn try_apply_ui_frame(
        &mut self,
        owner: u64,
        document: &mut NativeUiDocument,
        frame: &UiFrame,
    ) -> Result<NativeSceneUpdate, TypedUiApplyError> {
        let generation = self
            .surface
            .status(owner)
            .ok_or(TypedUiApplyError::SurfaceRejected)?
            .generation;
        self.try_apply_ui_frame_at(owner, generation, document, frame)
    }

    pub fn try_apply_ui_frame_at(
        &mut self,
        owner: u64,
        generation: u64,
        document: &mut NativeUiDocument,
        frame: &UiFrame,
    ) -> Result<NativeSceneUpdate, TypedUiApplyError> {
        if !self.surface.render_allowed(owner, generation) {
            return Err(TypedUiApplyError::SurfaceRejected);
        }
        let mut candidate = document.clone();
        let update = candidate
            .apply_frame_with_patches(frame)
            .map_err(TypedUiApplyError::Frame)?;
        let reduce_motion = candidate.environment().reduce_motion;
        if reduce_motion.is_some() && self.reduce_motion(owner).is_none() {
            return Err(TypedUiApplyError::Host(SceneUpdateError::OwnerMismatch));
        }
        self.try_apply_ui_ir_at(owner, generation, &update)?;
        if let Some(on) = reduce_motion {
            if !self.set_reduce_motion(owner, on) {
                return Err(TypedUiApplyError::Host(SceneUpdateError::Closed));
            }
        }
        *document = candidate;
        Ok(update)
    }

    /// `try_apply` under a host-configured quota; see `Engine::try_apply_within`.
    /// 宿主额度下的 `try_apply`；见 `Engine::try_apply_within`。
    pub fn try_apply_within(
        &mut self,
        owner: u64,
        expected: u64,
        next: u64,
        patches: &[ScenePatch],
        quota: SceneQuota,
    ) -> Result<(), SceneUpdateError> {
        if !self.surface.active_for_owner(owner) {
            return Err(SceneUpdateError::Closed);
        }
        let result = self
            .inner
            .try_apply_within(owner, expected, next, patches, quota);
        if result.is_ok() {
            self.surface.frame_superseded();
        }
        result
    }

    pub fn apply_update(&mut self, owner: u64, update: SceneUpdate<'_>) -> bool {
        self.apply(
            owner,
            update.expected_version,
            update.next_version,
            update.patches,
        )
    }

    pub fn try_apply_update(
        &mut self,
        owner: u64,
        update: SceneUpdate<'_>,
    ) -> Result<(), SceneUpdateError> {
        self.try_apply(
            owner,
            update.expected_version,
            update.next_version,
            update.patches,
        )
    }

    pub fn snapshot_identity(&self, owner: u64) -> Option<(u64, usize)> {
        self.inner.snapshot_identity(owner)
    }

    pub fn focused_node(&self, owner: u64, version: u64) -> Option<NodeId> {
        if !self.surface.active_for_owner(owner) {
            return None;
        }
        self.inner.focused_node(owner, version)
    }

    pub fn typed_focused_node(
        &self,
        owner: u64,
        version: u64,
        document: &NativeUiDocument,
    ) -> Result<Option<UiKey>, TypedUiTargetError> {
        if version != document.revision() {
            return Err(TypedUiTargetError::RevisionMismatch {
                host: version,
                document: document.revision(),
            });
        }
        self.focused_node(owner, version)
            .map(|node| {
                document
                    .key_for_node(node)
                    .ok_or(TypedUiTargetError::UnmappedTarget)
            })
            .transpose()
    }

    pub fn accessibility_tree(
        &self,
        owner: u64,
        version: u64,
        generation: u64,
    ) -> Option<crate::native::host::AccessibilityTree> {
        if !self.surface.render_allowed(owner, generation) {
            return None;
        }
        self.inner.accessibility_tree(owner, version)
    }

    /// Surface-gated accessibility tree projected losslessly onto Typed UI `UiKey`s.
    pub fn typed_accessibility_tree(
        &self,
        owner: u64,
        version: u64,
        generation: u64,
        document: &NativeUiDocument,
    ) -> Result<TypedAccessibilityTree, TypedUiAccessibilityError> {
        if version != document.revision() {
            return Err(TypedUiAccessibilityError::RevisionMismatch {
                host: version,
                document: document.revision(),
            });
        }
        let tree = self
            .accessibility_tree(owner, version, generation)
            .ok_or(TypedUiAccessibilityError::HostRejected)?;
        project_typed_accessibility(&tree, document).map_err(TypedUiAccessibilityError::Projection)
    }

    /// Surface-gated incremental accessibility projection using previous/next Typed IR documents.
    pub fn typed_accessibility_update(
        &self,
        owner: u64,
        next_version: u64,
        generation: u64,
        previous: &crate::native::host::AccessibilityTree,
        next: &crate::native::host::AccessibilityTree,
        previous_document: &NativeUiDocument,
        next_document: &NativeUiDocument,
    ) -> Result<TypedAccessibilityUpdate, TypedUiAccessibilityError> {
        if !self.surface.render_allowed(owner, generation) {
            return Err(TypedUiAccessibilityError::HostRejected);
        }
        if previous.scene_version != previous_document.revision()
            || next_version != next_document.revision()
            || next.scene_version != next_version
        {
            return Err(TypedUiAccessibilityError::RevisionMismatch {
                host: next_version,
                document: next_document.revision(),
            });
        }
        let update = next
            .update_since(previous)
            .ok_or(TypedUiAccessibilityError::InvalidUpdateSequence)?;
        crate::typed_accessibility::typed_accessibility_update(
            &update,
            previous_document,
            next_document,
        )
        .map_err(TypedUiAccessibilityError::Projection)
    }

    /// Route one native accessibility action through the current live surface generation.
    /// 通过当前活动 Surface 代数路由一个原生无障碍动作。
    #[cfg(feature = "accesskit-pending")]
    pub fn accesskit_action(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        request: &crate::native::host::accesskit::ActionRequest,
        focus: &mut impl FnMut(SceneFocusEvent),
        activation: &mut impl FnMut(crate::native::SceneActivationEvent),
    ) -> bool {
        self.surface.render_allowed(owner, generation)
            && self
                .inner
                .accesskit_action(owner, version, request, focus, activation)
    }

    #[cfg(feature = "accesskit-pending")]
    pub fn typed_accesskit_action(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        document: &NativeUiDocument,
        context: &UiEventContext,
        request: &crate::native::host::accesskit::ActionRequest,
        focus: &mut impl FnMut(TypedUiFocusEvent),
        activation: &mut impl FnMut(TypedUiActivationEvent),
    ) -> Result<TypedAccessibilityAction, TypedUiAccessibilityActionError> {
        let node = NodeId(request.target_node.0);
        let result = match request.action {
            crate::native::host::accesskit::Action::Focus => {
                let key = document
                    .admit_native_target(context, node, UiPermission::Focus)
                    .map_err(TypedUiAccessibilityActionError::Admission)?;
                TypedAccessibilityAction::Focused(key)
            }
            crate::native::host::accesskit::Action::Click => {
                let event = document
                    .admit_native_event(context, node, UiEventKind::AccessibilityAction)
                    .map_err(TypedUiAccessibilityActionError::Admission)?;
                TypedAccessibilityAction::Activated(event)
            }
            _ => return Err(TypedUiAccessibilityActionError::ActionNotSupported),
        };
        let mut focus_admissions = Vec::new();
        if request.action == crate::native::host::accesskit::Action::Focus {
            for focus_node in [self.focused_node(owner, version), Some(node)]
                .into_iter()
                .flatten()
            {
                let key = document
                    .key_for_node(focus_node)
                    .ok_or(TypedUiAccessibilityActionError::UnmappedTarget)?;
                if focus_admissions
                    .iter()
                    .any(|(existing, _)| *existing == key)
                {
                    continue;
                }
                if let Some(admitted) = admitted_optional_event(
                    document,
                    context,
                    key,
                    focus_node,
                    UiEventKind::Focus,
                )
                .map_err(TypedUiAccessibilityActionError::Admission)?
                {
                    focus_admissions.push((key, admitted));
                }
            }
        }
        let mut focus_mapping_error = false;
        let mut activation_mapping_error = false;
        let accepted = self.accesskit_action(
            owner,
            version,
            generation,
            request,
            &mut |event| {
                let Some(target) = document.key_for_node(event.node) else {
                    focus_mapping_error = true;
                    return;
                };
                focus(TypedUiFocusEvent {
                    target,
                    focused: event.focused,
                    admitted: focus_admissions
                        .iter()
                        .find(|(key, _)| *key == target)
                        .map(|(_, admitted)| admitted.clone()),
                });
            },
            &mut |event| {
                if let Some(target) = document.key_for_node(event.node) {
                    activation(TypedUiActivationEvent { target });
                } else {
                    activation_mapping_error = true;
                }
            },
        );
        if focus_mapping_error {
            return Err(TypedUiAccessibilityActionError::UnmappedTarget);
        }
        if activation_mapping_error {
            return Err(TypedUiAccessibilityActionError::UnmappedTarget);
        }
        if !accepted {
            return Err(TypedUiAccessibilityActionError::HostRejected);
        }
        Ok(result)
    }

    pub fn focus(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        node: NodeId,
        sink: &mut impl FnMut(SceneFocusEvent),
    ) -> bool {
        self.surface.render_allowed(owner, generation)
            && self.inner.focus(owner, version, node, sink)
    }

    pub fn typed_focus(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        document: &NativeUiDocument,
        context: &UiEventContext,
        node: NodeId,
        sink: &mut impl FnMut(TypedUiFocusEvent),
    ) -> Result<UiKey, TypedUiTargetError> {
        if version != document.revision() {
            return Err(TypedUiTargetError::RevisionMismatch {
                host: version,
                document: document.revision(),
            });
        }
        let target = document
            .admit_native_target(context, node, UiPermission::Focus)
            .map_err(TypedUiTargetError::Admission)?;
        let mut focus_admissions = Vec::new();
        for focus_node in [self.focused_node(owner, version), Some(node)]
            .into_iter()
            .flatten()
        {
            let key = document
                .key_for_node(focus_node)
                .ok_or(TypedUiTargetError::UnmappedTarget)?;
            if focus_admissions
                .iter()
                .any(|(existing, _)| *existing == key)
            {
                continue;
            }
            if let Some(admitted) =
                admitted_optional_event(document, context, key, focus_node, UiEventKind::Focus)
                    .map_err(TypedUiTargetError::Admission)?
            {
                focus_admissions.push((key, admitted));
            }
        }
        let mut mapping_error = false;
        let accepted = self.focus(owner, version, generation, node, &mut |event| {
            if let Some(target) = document.key_for_node(event.node) {
                sink(TypedUiFocusEvent {
                    target,
                    focused: event.focused,
                    admitted: focus_admissions
                        .iter()
                        .find(|(key, _)| *key == target)
                        .map(|(_, admitted)| admitted.clone()),
                });
            } else {
                mapping_error = true;
            }
        });
        if mapping_error {
            return Err(TypedUiTargetError::UnmappedTarget);
        }
        if !accepted {
            return Err(TypedUiTargetError::HostRejected);
        }
        Ok(target)
    }

    /// Read-only pointer hit query projected to public node identity. The result follows the
    /// same presented geometry, overlay blocking, corner exclusion and hit slop as pointer
    /// routing. `Ok(None)` means the sample has no interactive Typed target.
    pub fn typed_pointer_target(
        &self,
        owner: u64,
        version: u64,
        generation: u64,
        document: &NativeUiDocument,
        x: f32,
        y: f32,
    ) -> Result<Option<UiKey>, TypedUiPointerError> {
        self.pointer_route_target(owner, version, generation, document, x, y)
            .map(|target| target.map(|(key, _)| key))
    }

    /// Route a pointer sample only after the current Surface and Typed authority admit it.
    /// Pointer callbacks receive `UiKey`s, never backend NodeIds. A bound pointer or activation
    /// event is admitted separately against the node that actually received it, so activation
    /// may still belong to a source ancestor of the hit target.
    pub fn typed_pointer(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        document: &NativeUiDocument,
        context: &UiEventContext,
        phase: PointerAction,
        x: f32,
        y: f32,
        pressed: bool,
        pointer: &mut impl FnMut(TypedUiPointerEvent) -> EventReply,
        focus: &mut impl FnMut(TypedUiFocusEvent),
    ) -> Result<Option<UiKey>, TypedUiPointerError> {
        document
            .admit_permission(context, UiPermission::Pointer)
            .map_err(TypedUiPointerError::Admission)?;
        let target = self.pointer_route_target(owner, version, generation, document, x, y)?;
        if let Some((_, node)) = target {
            document
                .admit_native_target(context, node, UiPermission::Pointer)
                .map_err(TypedUiPointerError::Admission)?;
        }

        let mut focus_admissions = Vec::new();
        if phase == PointerAction::Down {
            let mut focus_candidates = Vec::new();
            if let Some((_, node)) = target {
                focus_candidates.push(node);
                if let Some(current) = self.focused_node(owner, version) {
                    focus_candidates.push(current);
                }
                focus_candidates.extend(source_ancestors(document.scene(), node));
            }
            focus_candidates.sort_unstable();
            focus_candidates.dedup();
            for focus_node in focus_candidates {
                let key = document
                    .key_for_node(focus_node)
                    .ok_or(TypedUiPointerError::UnmappedTarget)?;
                if let Some(admitted) =
                    admitted_optional_event(document, context, key, focus_node, UiEventKind::Focus)
                        .map_err(TypedUiPointerError::Admission)?
                {
                    focus_admissions.push((key, admitted));
                }
            }
        }

        let mut routing_error = None;
        let mut focus_mapping_error = false;
        let accepted = self.pointer(
            owner,
            version,
            generation,
            phase,
            x,
            y,
            pressed,
            &mut |event| {
                if routing_error.is_some() {
                    return EventReply::default();
                }
                let Some(target) = document.key_for_node(event.node) else {
                    routing_error = Some(TypedUiPointerError::UnmappedTarget);
                    return EventReply::default();
                };
                let mut event_context = context.clone();
                event_context.target = target;
                let admitted = match typed_pointer_event_kind(event.kind) {
                    Some(kind) if document.event_binding(target, kind).is_some() => {
                        match document.admit_native_event(&event_context, event.node, kind) {
                            Ok(event) => Some(event),
                            Err(error) => {
                                routing_error = Some(TypedUiPointerError::Admission(error));
                                return EventReply::default();
                            }
                        }
                    }
                    None if event.kind == SceneEventKind::Activate => {
                        let kind = if document
                            .event_binding(target, UiEventKind::Click)
                            .is_some()
                        {
                            UiEventKind::Click
                        } else {
                            UiEventKind::Activate
                        };
                        if document.event_binding(target, kind).is_some() {
                            match document.admit_native_event(&event_context, event.node, kind) {
                                Ok(event) => Some(event),
                                Err(error) => {
                                    routing_error = Some(TypedUiPointerError::Admission(error));
                                    return EventReply::default();
                                }
                            }
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                pointer(TypedUiPointerEvent {
                    target,
                    kind: event.kind,
                    admitted,
                    position: event.position,
                    primary_pressed: event.primary_pressed,
                    modifiers: event.modifiers,
                })
            },
            &mut |event| {
                if let Some(target) = document.key_for_node(event.node) {
                    focus(TypedUiFocusEvent {
                        target,
                        focused: event.focused,
                        admitted: focus_admissions
                            .iter()
                            .find(|(key, _)| *key == target)
                            .map(|(_, admitted)| admitted.clone()),
                    });
                } else {
                    focus_mapping_error = true;
                }
            },
        );
        if focus_mapping_error {
            return Err(TypedUiPointerError::UnmappedTarget);
        }
        if let Some(error) = routing_error {
            return Err(error);
        }
        if !accepted {
            return Err(TypedUiPointerError::HostRejected);
        }
        Ok(target.map(|(key, _)| key))
    }

    fn pointer_route_target(
        &self,
        owner: u64,
        version: u64,
        generation: u64,
        document: &NativeUiDocument,
        x: f32,
        y: f32,
    ) -> Result<Option<(UiKey, NodeId)>, TypedUiPointerError> {
        if version != document.revision() {
            return Err(TypedUiPointerError::RevisionMismatch {
                host: version,
                document: document.revision(),
            });
        }
        if !self.surface.render_allowed(owner, generation) {
            return Err(TypedUiPointerError::HostRejected);
        }
        if !x.is_finite() || !y.is_finite() {
            return Err(TypedUiPointerError::InvalidPoint);
        }
        let Some(node) = self
            .inner
            .pointer_target_from_host(owner, version, x, y)
        else {
            return Ok(None);
        };
        let key = document
            .key_for_node(node)
            .ok_or(TypedUiPointerError::UnmappedTarget)?;
        Ok(Some((key, node)))
    }

    pub fn pointer(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        phase: crate::native::PointerAction,
        x: f32,
        y: f32,
        pressed: bool,
        pointer: &mut impl FnMut(crate::native::SceneEvent) -> crate::native::EventReply,
        focus: &mut impl FnMut(SceneFocusEvent),
    ) -> bool {
        self.surface.render_allowed(owner, generation)
            && self
                .inner
                .pointer(owner, version, phase, x, y, pressed, pointer, focus)
    }

    pub fn key(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        input: &crate::native::HostKeyInput<'_>,
        key: &mut impl FnMut(crate::native::SceneKeyEvent) -> crate::native::EventReply,
        focus: &mut impl FnMut(SceneFocusEvent),
    ) -> bool {
        self.surface.render_allowed(owner, generation)
            && self.inner.key(owner, version, input, key, focus)
    }

    pub fn typed_key(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        document: &NativeUiDocument,
        context: &UiEventContext,
        node: NodeId,
        input: &crate::native::HostKeyInput<'_>,
        key: &mut impl FnMut(TypedUiKeyEvent) -> crate::native::EventReply,
        focus: &mut impl FnMut(TypedUiFocusEvent),
    ) -> Result<UiKey, TypedUiTargetError> {
        if version != document.revision() {
            return Err(TypedUiTargetError::RevisionMismatch {
                host: version,
                document: document.revision(),
            });
        }
        let target = document
            .admit_native_target(context, node, UiPermission::Keyboard)
            .map_err(TypedUiTargetError::Admission)?;
        if self.focused_node(owner, version) != Some(node) {
            return Err(TypedUiTargetError::HostRejected);
        }
        let key_admission =
            admitted_optional_event(document, context, target, node, UiEventKind::Key)
                .map_err(TypedUiTargetError::Admission)?;
        let mut key_mapping_error = false;
        let mut focus_mapping_error = false;
        let mut focus_admission_error = None;
        let accepted = self.key(
            owner,
            version,
            generation,
            input,
            &mut |event: SceneKeyEvent| {
                if let Some(target) = document.key_for_node(event.node) {
                    key(TypedUiKeyEvent {
                        target,
                        key: event.key,
                        code: event.code,
                        pressed: event.pressed,
                        repeat: event.repeat,
                        composing: event.composing,
                        modifiers: event.modifiers,
                        location: event.location,
                        admitted: key_admission.clone(),
                    })
                } else {
                    key_mapping_error = true;
                    EventReply::default()
                }
            },
            &mut |event| {
                if let Some(target) = document.key_for_node(event.node) {
                    match admitted_optional_event(
                        document,
                        context,
                        target,
                        event.node,
                        UiEventKind::Focus,
                    ) {
                        Ok(admitted) => focus(TypedUiFocusEvent {
                            target,
                            focused: event.focused,
                            admitted,
                        }),
                        Err(error) => focus_admission_error = Some(error),
                    }
                } else {
                    focus_mapping_error = true;
                }
            },
        );
        if let Some(error) = focus_admission_error {
            return Err(TypedUiTargetError::Admission(error));
        }
        if key_mapping_error || focus_mapping_error {
            return Err(TypedUiTargetError::UnmappedTarget);
        }
        if !accepted {
            return Err(TypedUiTargetError::HostRejected);
        }
        Ok(target)
    }

    pub fn typed_key_with_activation(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        document: &NativeUiDocument,
        context: &UiEventContext,
        node: NodeId,
        input: &crate::native::HostKeyInput<'_>,
        key: &mut impl FnMut(TypedUiKeyEvent) -> crate::native::EventReply,
        focus: &mut impl FnMut(TypedUiFocusEvent),
        activation: &mut impl FnMut(TypedUiActivationEvent),
    ) -> Result<Option<AdmittedEvent>, TypedUiTargetError> {
        if version != document.revision() {
            return Err(TypedUiTargetError::RevisionMismatch {
                host: version,
                document: document.revision(),
            });
        }
        let target = document
            .admit_native_target(context, node, UiPermission::Keyboard)
            .map_err(TypedUiTargetError::Admission)?;
        if self.focused_node(owner, version) != Some(node) {
            return Err(TypedUiTargetError::HostRejected);
        }
        let key_admission =
            admitted_optional_event(document, context, target, node, UiEventKind::Key)
                .map_err(TypedUiTargetError::Admission)?;
        let mut admitted = None;
        let mut admission_error = None;
        let mut key_mapping_error = false;
        let mut focus_mapping_error = false;
        let mut activation_mapping_error = false;
        let mut focus_admission_error = None;
        let accepted = self.key_with_activation(
            owner,
            version,
            generation,
            input,
            &mut |event: SceneKeyEvent| {
                if let Some(target) = document.key_for_node(event.node) {
                    key(TypedUiKeyEvent {
                        target,
                        key: event.key,
                        code: event.code,
                        pressed: event.pressed,
                        repeat: event.repeat,
                        composing: event.composing,
                        modifiers: event.modifiers,
                        location: event.location,
                        admitted: key_admission.clone(),
                    })
                } else {
                    key_mapping_error = true;
                    EventReply::default()
                }
            },
            &mut |event| {
                if let Some(target) = document.key_for_node(event.node) {
                    match admitted_optional_event(
                        document,
                        context,
                        target,
                        event.node,
                        UiEventKind::Focus,
                    ) {
                        Ok(admitted) => focus(TypedUiFocusEvent {
                            target,
                            focused: event.focused,
                            admitted,
                        }),
                        Err(error) => focus_admission_error = Some(error),
                    }
                } else {
                    focus_mapping_error = true;
                }
            },
            &mut |event| match document.admit_activation(
                context,
                event.node,
                crate::ui_ir::NativeActivationSource::Keyboard,
            ) {
                Ok(result) => {
                    admitted = Some(result);
                    if let Some(target) = document.key_for_node(event.node) {
                        activation(TypedUiActivationEvent { target });
                    } else {
                        activation_mapping_error = true;
                    }
                }
                Err(error) => admission_error = Some(error),
            },
        );
        if let Some(error) = admission_error {
            return Err(TypedUiTargetError::Admission(error));
        }
        if let Some(error) = focus_admission_error {
            return Err(TypedUiTargetError::Admission(error));
        }
        if key_mapping_error || focus_mapping_error || activation_mapping_error {
            return Err(TypedUiTargetError::UnmappedTarget);
        }
        if !accepted {
            return Err(TypedUiTargetError::HostRejected);
        }
        Ok(admitted)
    }

    /// `key` plus engine keyboard activation; see `Engine::key_with_activation`.
    /// `key` 加引擎键盘激活；见 `Engine::key_with_activation`。
    pub fn key_with_activation(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        input: &crate::native::HostKeyInput<'_>,
        key: &mut impl FnMut(crate::native::SceneKeyEvent) -> crate::native::EventReply,
        focus: &mut impl FnMut(SceneFocusEvent),
        activation: &mut impl FnMut(crate::native::SceneActivationEvent),
    ) -> bool {
        self.surface.render_allowed(owner, generation)
            && self
                .inner
                .key_with_activation(owner, version, input, key, focus, activation)
    }

    /// E14 text input through the live surface generation; see `Engine::text_input`.
    /// 经当前活动 Surface 代数的 E14 文字输入；见 `Engine::text_input`。
    pub fn text_input(&mut self, owner: u64, version: u64, generation: u64, text: &str) -> bool {
        self.surface.render_allowed(owner, generation)
            && self.inner.text_input(owner, version, text)
    }

    pub fn typed_text_input(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        document: &NativeUiDocument,
        context: &UiEventContext,
        node: NodeId,
        text: &str,
    ) -> Result<UiKey, TypedUiTargetError> {
        if version != document.revision() {
            return Err(TypedUiTargetError::RevisionMismatch {
                host: version,
                document: document.revision(),
            });
        }
        let target = document
            .admit_native_target(context, node, UiPermission::TextEdit)
            .map_err(TypedUiTargetError::Admission)?;
        if self
            .edit_state(owner, version)
            .and_then(|state| state.map(|state| state.node))
            != Some(node)
        {
            return Err(TypedUiTargetError::HostRejected);
        }
        if !self.text_input(owner, version, generation, text) {
            return Err(TypedUiTargetError::HostRejected);
        }
        Ok(target)
    }

    /// E14 IME preedit through the live surface generation; see `Engine::set_preedit`.
    /// 经当前活动 Surface 代数的 E14 输入法预编辑；见 `Engine::set_preedit`。
    pub fn set_preedit(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        text: Option<&str>,
        cursor: Option<(usize, usize)>,
    ) -> bool {
        self.surface.render_allowed(owner, generation)
            && self.inner.set_preedit(owner, version, text, cursor)
    }

    pub fn typed_set_preedit(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        document: &NativeUiDocument,
        context: &UiEventContext,
        node: NodeId,
        text: Option<&str>,
        cursor: Option<(usize, usize)>,
    ) -> Result<UiKey, TypedUiTargetError> {
        if version != document.revision() {
            return Err(TypedUiTargetError::RevisionMismatch {
                host: version,
                document: document.revision(),
            });
        }
        let target = document
            .admit_native_target(context, node, UiPermission::Ime)
            .map_err(TypedUiTargetError::Admission)?;
        if self
            .edit_state(owner, version)
            .and_then(|state| state.map(|state| state.node))
            != Some(node)
        {
            return Err(TypedUiTargetError::HostRejected);
        }
        if !self.set_preedit(owner, version, generation, text, cursor) {
            return Err(TypedUiTargetError::HostRejected);
        }
        Ok(target)
    }

    /// Commit input-method text into the current edit session. The caller supplies the exact
    /// UiKey; an optional `ime-commit` binding is admitted before Native text insertion.
    pub fn typed_ime_commit(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        document: &NativeUiDocument,
        context: &UiEventContext,
        node: NodeId,
        text: &str,
    ) -> Result<Option<AdmittedEvent>, TypedUiTargetError> {
        if version != document.revision() {
            return Err(TypedUiTargetError::RevisionMismatch {
                host: version,
                document: document.revision(),
            });
        }
        let target = document
            .admit_native_target(context, node, UiPermission::Ime)
            .map_err(TypedUiTargetError::Admission)?;
        if self
            .edit_state(owner, version)
            .and_then(|state| state.map(|state| state.node))
            != Some(node)
        {
            return Err(TypedUiTargetError::HostRejected);
        }
        let admitted = admitted_optional_event(
            document,
            context,
            target,
            node,
            UiEventKind::ImeCommit,
        )
        .map_err(TypedUiTargetError::Admission)?;
        if !self.text_input(owner, version, generation, text) {
            return Err(TypedUiTargetError::HostRejected);
        }
        Ok(admitted)
    }

    /// E14 edit events; see `Engine::take_edit_events` (independent of the surface lifecycle).
    /// E14 编辑事件；见 `Engine::take_edit_events`（与 Surface 生命周期无关）。
    pub fn take_edit_events(&mut self, owner: u64) -> Option<Vec<crate::native::SceneEditEvent>> {
        self.inner.take_edit_events(owner)
    }

    /// Drain E14 edit events and project their targets to `UiKey`. `changed_as_commit` forwards
    /// live-text `Changed` events to a bound `text-commit`, matching the documented host mode.
    /// The whole drained batch is validated before it is returned; no backend NodeId is exposed.
    pub fn typed_take_edit_events(
        &mut self,
        owner: u64,
        document: &NativeUiDocument,
        context: &UiEventContext,
        changed_as_commit: bool,
    ) -> Result<Vec<TypedUiEditEvent>, TypedUiEditError> {
        let events = self
            .take_edit_events(owner)
            .ok_or(TypedUiEditError::HostRejected)?;
        let mut typed = Vec::with_capacity(events.len());
        for event in events {
            // A Native frame transaction carries already-queued edit events into its
            // successor, so an event may legitimately name an earlier revision. The current
            // UiKey mapping and authority context still gate delivery.
            if event.scene_version > document.revision() {
                return Err(TypedUiEditError::FutureRevision {
                    event: event.scene_version,
                    document: document.revision(),
                });
            }
            let target = document
                .key_for_node(event.node)
                .ok_or(TypedUiEditError::UnmappedTarget)?;
            let binding_kind = match event.kind {
                crate::native::SceneEditEventKind::Changed(_) if changed_as_commit => {
                    Some(UiEventKind::TextCommit)
                }
                crate::native::SceneEditEventKind::Commit(_) => Some(UiEventKind::TextCommit),
                crate::native::SceneEditEventKind::CompositionCancelled => {
                    Some(UiEventKind::ImeCancel)
                }
                _ => None,
            };
            let admitted = match binding_kind {
                Some(kind) if document.event_binding(target, kind).is_some() => {
                    let mut event_context = context.clone();
                    event_context.target = target;
                    Some(
                        document
                            .admit_native_event(&event_context, event.node, kind)
                            .map_err(TypedUiEditError::Admission)?,
                    )
                }
                _ => None,
            };
            typed.push(TypedUiEditEvent {
                target,
                kind: event.kind,
                admitted,
            });
        }
        Ok(typed)
    }

    /// E14 active edit session; see `Engine::edit_state`.
    /// E14 当前编辑会话；见 `Engine::edit_state`。
    pub fn edit_state(
        &self,
        owner: u64,
        version: u64,
    ) -> Option<Option<crate::native::SceneEditState>> {
        self.inner.edit_state(owner, version)
    }

    pub fn typed_edit_state(
        &self,
        owner: u64,
        version: u64,
        document: &NativeUiDocument,
    ) -> Result<Option<TypedUiEditState>, TypedUiTargetError> {
        if version != document.revision() {
            return Err(TypedUiTargetError::RevisionMismatch {
                host: version,
                document: document.revision(),
            });
        }
        let state = self
            .edit_state(owner, version)
            .ok_or(TypedUiTargetError::HostRejected)?;
        state
            .map(|state| {
                let target = document
                    .key_for_node(state.node)
                    .ok_or(TypedUiTargetError::UnmappedTarget)?;
                Ok(TypedUiEditState {
                    target,
                    text: state.text,
                    anchor: state.anchor,
                    caret: state.caret,
                    preedit: state.preedit,
                    caret_rect: state.caret_rect,
                })
            })
            .transpose()
    }

    /// E14 caret blink through the live surface generation; see `Engine::caret_blink`.
    /// 经当前活动 Surface 代数的 E14 光标闪烁；见 `Engine::caret_blink`。
    pub fn caret_blink(
        &mut self,
        owner: u64,
        generation: u64,
        now_ms: f64,
    ) -> Option<crate::native::CaretBlink> {
        if !self.surface.render_allowed(owner, generation) {
            return None;
        }
        self.inner.caret_blink(owner, now_ms)
    }

    /// View-local logical border box of a source-visible node through the live surface
    /// generation; see `Engine::node_bounds`.
    /// 经当前活动 Surface 代数取源可见节点的视图局部逻辑边框盒；见 `Engine::node_bounds`。
    pub fn node_bounds(
        &self,
        owner: u64,
        version: u64,
        generation: u64,
        node: NodeId,
    ) -> Option<crate::geometry::Rect> {
        if !self.surface.render_allowed(owner, generation) {
            return None;
        }
        self.inner.node_bounds(owner, version, node)
    }

    pub fn typed_node_bounds(
        &self,
        owner: u64,
        version: u64,
        generation: u64,
        document: &NativeUiDocument,
        key: UiKey,
    ) -> Result<Option<crate::geometry::Rect>, TypedUiGeometryError> {
        self.geometry_target(owner, version, generation, document, key)?;
        let node = document
            .node_for_key(key)
            .ok_or(TypedUiGeometryError::UnknownKey(key))?;
        Ok(self.node_bounds(owner, version, generation, node))
    }

    pub fn scroll_geometry(
        &self,
        owner: u64,
        version: u64,
        node: NodeId,
    ) -> Option<crate::ScrollGeometry> {
        self.inner.scroll_geometry(owner, version, node)
    }

    pub fn typed_scroll_geometry(
        &self,
        owner: u64,
        version: u64,
        generation: u64,
        document: &NativeUiDocument,
        key: UiKey,
    ) -> Result<Option<crate::ScrollGeometry>, TypedUiGeometryError> {
        self.geometry_target(owner, version, generation, document, key)?;
        let node = document
            .node_for_key(key)
            .ok_or(TypedUiGeometryError::UnknownKey(key))?;
        Ok(self.scroll_geometry(owner, version, node))
    }

    pub fn table_metrics(
        &self,
        owner: u64,
        version: u64,
        node: NodeId,
    ) -> Option<crate::TableMetrics> {
        self.inner.table_metrics(owner, version, node)
    }

    pub fn typed_table_metrics(
        &self,
        owner: u64,
        version: u64,
        generation: u64,
        document: &NativeUiDocument,
        key: UiKey,
    ) -> Result<Option<TypedUiTableMetrics>, TypedUiGeometryError> {
        self.geometry_target(owner, version, generation, document, key)?;
        let node = document
            .node_for_key(key)
            .ok_or(TypedUiGeometryError::UnknownKey(key))?;
        self.table_metrics(owner, version, node)
            .map(|metrics| {
                Ok(TypedUiTableMetrics {
                    target: key,
                    rect: metrics.rect,
                    column_widths: metrics.column_widths,
                    row_heights: metrics.row_heights,
                    cells: metrics
                        .cells
                        .into_iter()
                        .map(|cell| {
                            Ok(TypedUiTableCellMetrics {
                                target: document
                                    .key_for_node(cell.node)
                                    .ok_or(TypedUiGeometryError::UnmappedTarget)?,
                                row: cell.row,
                                column: cell.column,
                                rect: cell.rect,
                            })
                        })
                        .collect::<Result<_, _>>()?,
                })
            })
            .transpose()
    }

    /// Admit a host-detected wheel/trackpad delta against one Typed target. This does not mutate
    /// the Native scroll offset; Framework returns the resulting versioned scene update.
    pub fn typed_scroll_event(
        &self,
        owner: u64,
        version: u64,
        generation: u64,
        document: &NativeUiDocument,
        context: &UiEventContext,
        node: NodeId,
        delta: crate::geometry::Point,
    ) -> Result<TypedUiScrollEvent, TypedUiScrollError> {
        if version != document.revision() {
            return Err(TypedUiScrollError::RevisionMismatch {
                host: version,
                document: document.revision(),
            });
        }
        if !self.surface.render_allowed(owner, generation) {
            return Err(TypedUiScrollError::HostRejected);
        }
        if !delta.x.is_finite()
            || !delta.y.is_finite()
            || (delta.x == 0.0 && delta.y == 0.0)
        {
            return Err(TypedUiScrollError::InvalidDelta);
        }
        let admitted = document
            .admit_native_event(context, node, UiEventKind::Scroll)
            .map_err(TypedUiScrollError::Admission)?;
        Ok(TypedUiScrollEvent {
            target: admitted.target,
            delta,
            admitted,
        })
    }

    pub(crate) fn overlays(
        &self,
        owner: u64,
        version: u64,
        generation: u64,
    ) -> Option<Vec<crate::native::OverlayInfo>> {
        if !self.surface.render_allowed(owner, generation) {
            return None;
        }
        self.inner.overlays(owner, version)
    }

    pub fn typed_overlays(
        &self,
        owner: u64,
        version: u64,
        generation: u64,
        document: &NativeUiDocument,
    ) -> Result<Vec<TypedUiOverlayInfo>, TypedUiOverlayError> {
        if version != document.revision() {
            return Err(TypedUiOverlayError::RevisionMismatch {
                host: version,
                document: document.revision(),
            });
        }
        let overlays = self
            .overlays(owner, version, generation)
            .ok_or(TypedUiOverlayError::HostRejected)?;
        overlays
            .into_iter()
            .map(|overlay| {
                Ok(TypedUiOverlayInfo {
                    target: document
                        .key_for_node(overlay.overlay)
                        .ok_or(TypedUiOverlayError::UnmappedTarget)?,
                    rank: overlay.rank,
                    spec: overlay.spec,
                    placement: overlay.placement,
                    suppressed: overlay.suppressed,
                    return_focus: overlay
                        .return_focus
                        .map(|node| {
                            document
                                .key_for_node(node)
                                .ok_or(TypedUiOverlayError::UnmappedTarget)
                        })
                        .transpose()?,
                })
            })
            .collect()
    }

    pub(crate) fn take_overlay_events(
        &mut self,
        owner: u64,
    ) -> Option<Vec<crate::native::OverlayEvent>> {
        self.inner.take_overlay_events(owner)
    }

    /// Drain overlay dismiss requests into Typed identity. A queued request may name an earlier
    /// revision carried by a frame transaction, but never a future document revision.
    pub fn typed_take_overlay_events(
        &mut self,
        owner: u64,
        document: &NativeUiDocument,
        context: &UiEventContext,
    ) -> Result<Vec<TypedUiOverlayEvent>, TypedUiOverlayError> {
        let events = self
            .take_overlay_events(owner)
            .ok_or(TypedUiOverlayError::HostRejected)?;
        let mut typed = Vec::with_capacity(events.len());
        for event in events {
            if event.scene_version > document.revision() {
                return Err(TypedUiOverlayError::FutureRevision {
                    event: event.scene_version,
                    document: document.revision(),
                });
            }
            let target = document
                .key_for_node(event.overlay)
                .ok_or(TypedUiOverlayError::UnmappedTarget)?;
            let admitted = if document
                .event_binding(target, UiEventKind::Dismiss)
                .is_some()
            {
                let mut event_context = context.clone();
                event_context.target = target;
                Some(
                    document
                        .admit_native_event(&event_context, event.overlay, UiEventKind::Dismiss)
                        .map_err(TypedUiOverlayError::Admission)?,
                )
            } else {
                None
            };
            typed.push(TypedUiOverlayEvent {
                target,
                kind: event.kind,
                admitted,
            });
        }
        Ok(typed)
    }

    fn geometry_target(
        &self,
        owner: u64,
        version: u64,
        generation: u64,
        document: &NativeUiDocument,
        key: UiKey,
    ) -> Result<(), TypedUiGeometryError> {
        if version != document.revision() {
            return Err(TypedUiGeometryError::RevisionMismatch {
                host: version,
                document: document.revision(),
            });
        }
        if !self.surface.render_allowed(owner, generation) {
            return Err(TypedUiGeometryError::HostRejected);
        }
        if document.node_for_key(key).is_none() {
            return Err(TypedUiGeometryError::UnknownKey(key));
        }
        Ok(())
    }

    /// E7 transition clock through the live surface generation; see `Engine::animate`.
    /// 经当前活动 Surface 代数推进 E7 过渡时钟；见 `Engine::animate`。
    pub fn animate(
        &mut self,
        owner: u64,
        generation: u64,
        now_ms: f64,
    ) -> Option<crate::motion::AnimationTick> {
        if !self.surface.render_allowed(owner, generation) {
            return None;
        }
        self.inner.animate(owner, now_ms)
    }

    /// E7: some transition is running; see `Engine::animating`.
    /// E7：有过渡在运行；见 `Engine::animating`。
    pub fn animating(&self, owner: u64) -> Option<bool> {
        self.inner.animating(owner)
    }

    /// E7 host reduce-motion environment; see `Engine::set_reduce_motion`. Independent of the
    /// surface lifecycle (it is a scene setting).
    /// E7 宿主「减少动态」环境；见 `Engine::set_reduce_motion`。与 Surface 生命周期无关（属场景设置）。
    pub fn set_reduce_motion(&mut self, owner: u64, on: bool) -> bool {
        self.inner.set_reduce_motion(owner, on)
    }
    pub fn reduce_motion(&self, owner: u64) -> Option<bool> {
        self.inner.reduce_motion(owner)
    }

    /// E5 material budget; see `Engine::set_backdrop_budget` (a scene setting, like reduce-motion).
    /// E5 材质预算；见 `Engine::set_backdrop_budget`（与减少动态一样属场景设置）。
    pub fn set_backdrop_budget(
        &mut self,
        owner: u64,
        budget: crate::native::BackdropBudget,
    ) -> bool {
        self.inner.set_backdrop_budget(owner, budget)
    }

    /// E5 report of the last painted frame; see `Engine::backdrop_report`.
    /// E5 上一绘制帧的材质报告；见 `Engine::backdrop_report`。
    pub fn backdrop_report(&self, owner: u64) -> Option<crate::native::BackdropReport> {
        self.inner.backdrop_report(owner)
    }

    /// Host-owned image resources; see `Engine::insert_image`.
    /// 宿主持有的图片资源；见 `Engine::insert_image`。
    pub fn insert_image(
        &mut self,
        owner: u64,
        id: &str,
        bytes: &[u8],
        density: f32,
    ) -> Result<ImageInfo, ImageError> {
        self.inner.insert_image(owner, id, bytes, density)
    }

    /// Upload bytes only for the exact committed resource named by the current Typed IR document.
    pub fn insert_ui_image(
        &mut self,
        owner: u64,
        document: &NativeUiDocument,
        resource: &ResourceRef,
        bytes: &[u8],
        density: f32,
    ) -> Result<ImageInfo, TypedUiResourceError> {
        let current = self
            .inner
            .snapshot_identity(owner)
            .map(|(version, _)| version);
        if current != Some(document.revision()) {
            return Err(TypedUiResourceError::HostRevision {
                document: document.revision(),
                current,
            });
        }
        if document.committed_resource(&resource.id) != Some(resource) {
            return Err(TypedUiResourceError::ResourceNotCommitted {
                id: resource.id.clone(),
                generation: resource.generation,
            });
        }
        if !matches!(
            resource.kind,
            ResourceKind::Image | ResourceKind::Svg | ResourceKind::Icon
        ) {
            return Err(TypedUiResourceError::UnsupportedKind(resource.kind));
        }
        self.inner
            .insert_image(owner, &resource.id, bytes, density)
            .map_err(TypedUiResourceError::Image)
    }

    pub fn remove_image(&mut self, owner: u64, id: &str) -> Option<bool> {
        self.inner.remove_image(owner, id)
    }

    /// Remove host bytes only after the Typed IR document has released the resource.
    pub fn remove_ui_image(
        &mut self,
        owner: u64,
        document: &NativeUiDocument,
        id: &str,
    ) -> Result<Option<bool>, TypedUiResourceError> {
        let current = self
            .inner
            .snapshot_identity(owner)
            .map(|(version, _)| version);
        if current != Some(document.revision()) {
            return Err(TypedUiResourceError::HostRevision {
                document: document.revision(),
                current,
            });
        }
        if document.active_resource(id).is_some() {
            return Err(TypedUiResourceError::ResourceStillActive { id: id.into() });
        }
        Ok(self.inner.remove_image(owner, id))
    }

    pub fn set_image_limits(
        &mut self,
        owner: u64,
        limits: ImageLimits,
    ) -> Option<Result<(), ImageError>> {
        self.inner.set_image_limits(owner, limits)
    }

    pub fn image_report(&self, owner: u64) -> Option<ImageReport> {
        self.inner.image_report(owner)
    }

    /// Semantic activation through the live surface generation; see `Engine::activate`.
    /// 经当前活动 Surface 代数做语义激活；见 `Engine::activate`。
    pub fn activate(
        &self,
        owner: u64,
        version: u64,
        generation: u64,
        node: NodeId,
        sink: &mut impl FnMut(crate::native::SceneActivationEvent),
    ) -> bool {
        self.surface.render_allowed(owner, generation)
            && self.inner.activate(owner, version, node, sink)
    }

    pub fn typed_activate(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        document: &NativeUiDocument,
        context: &UiEventContext,
        node: NodeId,
        activation: &mut impl FnMut(TypedUiActivationEvent),
    ) -> Result<AdmittedEvent, TypedUiTargetError> {
        if version != document.revision() {
            return Err(TypedUiTargetError::RevisionMismatch {
                host: version,
                document: document.revision(),
            });
        }
        let admitted = document
            .admit_activation(context, node, crate::ui_ir::NativeActivationSource::Accessibility)
            .map_err(TypedUiTargetError::Admission)?;
        let target = admitted.target;
        if !self.activate(owner, version, generation, node, &mut |_| {
            activation(TypedUiActivationEvent { target })
        }) {
            return Err(TypedUiTargetError::HostRejected);
        }
        Ok(admitted)
    }

    pub fn apply_with_focus_events(
        &mut self,
        owner: u64,
        expected: u64,
        next: u64,
        generation: u64,
        patches: &[ScenePatch],
        focus: &mut impl FnMut(SceneFocusEvent),
    ) -> bool {
        if !self.surface.render_allowed(owner, generation) {
            return false;
        }
        let applied = self
            .inner
            .apply_with_focus_events(owner, expected, next, patches, focus);
        if applied {
            self.surface.frame_superseded();
        }
        applied
    }

    pub fn set_viewport_scroll(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        x: f64,
        y: f64,
    ) -> bool {
        self.surface.render_allowed(owner, generation)
            && self.inner.set_viewport_scroll(owner, version, x, y)
    }

    /// Clear native hover/press/focus on window deactivation without destroying the surface.
    /// 窗口失活时清除原生 hover/press/focus；不销毁 Surface。
    pub fn deactivate(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        focus: &mut impl FnMut(SceneFocusEvent),
    ) -> bool {
        self.surface.render_allowed(owner, generation)
            && self.inner.deactivate(owner, version, focus)
    }

    pub fn invalidate_frame(&mut self, owner: u64, generation: u64) -> bool {
        if !self.surface.render_allowed(owner, generation) {
            return false;
        }
        self.surface.frame_superseded();
        self.inner.invalidate_frame(owner)
    }

    /// Every render entry goes through here: the lifecycle admits it, a failure leaves the view
    /// dirty, and the coordinator records what the outcome means for a pending present.
    /// 所有渲染入口都经过这里：由生命周期准入，失败后视图保持待重绘，协调器记录结果对待呈现帧的影响。
    #[cfg(feature = "cpu-pending")]
    fn gated_render<T, E>(
        &mut self,
        owner: u64,
        generation: u64,
        pixels: RenderedPixels,
        render: impl FnOnce(&mut SceneHostView) -> Result<T, E>,
    ) -> Option<Result<T, E>> {
        let inner = &mut self.inner;
        self.surface.render(owner, generation, pixels, || {
            let result = render(inner);
            if result.is_err() {
                inner.invalidate_frame(owner);
            }
            result
        })
    }

    #[cfg(feature = "cpu-pending")]
    pub fn render_to_surface(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        bytes: &mut [u8],
        stride: usize,
        budget: usize,
    ) -> bool {
        let rendered = self.gated_render(owner, generation, RenderedPixels::Delivered, |inner| {
            inner
                .render_to_surface(owner, version, bytes, stride, budget)
                .then_some(())
                .ok_or(())
        });
        rendered == Some(Ok(()))
    }

    #[cfg(feature = "cpu-pending")]
    pub fn render_to_surface_with_damage(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        bytes: &mut [u8],
        stride: usize,
        budget: usize,
    ) -> Option<crate::native::host::FrameDamage> {
        self.gated_render(owner, generation, RenderedPixels::Delivered, |inner| {
            inner
                .render_to_surface_with_damage(owner, version, bytes, stride, budget)
                .ok_or(())
        })?
        .ok()
    }

    #[cfg(feature = "cpu-pending")]
    pub fn render_to_backend(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        backend: &mut impl crate::backend::RenderBackend,
        budget: usize,
    ) -> Result<(), crate::backend::BackendError> {
        self.gated_render(owner, generation, RenderedPixels::Delivered, |inner| {
            inner.render_to_backend(owner, version, backend, budget)
        })
        .unwrap_or_else(|| {
            Err(crate::backend::BackendError::Invalid(
                "stale or inactive host surface generation".into(),
            ))
        })
    }

    #[cfg(feature = "gpu-pending")]
    pub fn render_gpu(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        gpu: &mut GpuRenderer,
        budget: usize,
    ) -> Result<Vec<u8>, GpuFrameError> {
        self.gated_render(owner, generation, RenderedPixels::Delivered, |inner| {
            inner.render_gpu(owner, version, gpu, budget)
        })
        .unwrap_or(Err(GpuFrameError::Rejected))
    }

    /// A failure here withdraws any frame still waiting for a present: it may have written to the
    /// same texture. `gpu_presented` then refuses and `needs_frame` asks for a new render.
    /// 此处失败会撤销仍在等待呈现的帧，因为它可能已写入同一纹理；之后 `gpu_presented` 拒绝，`needs_frame` 要求重新渲染。
    #[cfg(feature = "gpu-pending")]
    pub fn render_gpu_to_host_view(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        gpu: &mut GpuRenderer,
        format: wgpu::TextureFormat,
        view: &wgpu::TextureView,
        budget: usize,
    ) -> Result<(), GpuFrameError> {
        self.gated_render(
            owner,
            generation,
            RenderedPixels::AwaitingPresent,
            |inner| inner.render_gpu_to_host_view(owner, version, gpu, format, view, budget),
        )
        .unwrap_or(Err(GpuFrameError::Rejected))
    }

    /// Acknowledge that the host's `Queue::present` succeeded for the exact live generation.
    /// Until this succeeds, `needs_frame` keeps the texture frame pending.
    /// 确认宿主对精确当前代数成功执行 `Queue::present`；确认前 `needs_frame` 保持该纹理帧待呈现。
    #[cfg(feature = "gpu-pending")]
    pub fn gpu_presented(&mut self, owner: u64, generation: u64) -> bool {
        self.surface.presented(owner, generation)
    }
}

fn typed_pointer_event_kind(kind: SceneEventKind) -> Option<UiEventKind> {
    match kind {
        SceneEventKind::Pointer(PointerAction::Down) => Some(UiEventKind::PointerDown),
        SceneEventKind::Pointer(PointerAction::Move) => Some(UiEventKind::Hover),
        SceneEventKind::Pointer(PointerAction::Up) => Some(UiEventKind::PointerUp),
        SceneEventKind::Pointer(PointerAction::Cancel | PointerAction::Leave)
        | SceneEventKind::Activate => None,
    }
}

fn admitted_optional_event(
    document: &NativeUiDocument,
    context: &UiEventContext,
    target: UiKey,
    node: NodeId,
    kind: UiEventKind,
) -> Result<Option<AdmittedEvent>, UiEventAdmissionError> {
    if document.event_binding(target, kind).is_none() {
        return Ok(None);
    }
    let mut event_context = context.clone();
    event_context.target = target;
    document
        .admit_native_event(&event_context, node, kind)
        .map(Some)
}

fn source_ancestors(scene: &Scene, node: NodeId) -> Vec<NodeId> {
    let mut parents = BTreeMap::new();
    let mut stack = vec![&scene.root];
    while let Some(parent) = stack.pop() {
        for child in &parent.children {
            parents.insert(child.id, parent.id);
            stack.push(child);
        }
    }
    let mut ancestors = Vec::new();
    let mut current = parents.get(&node).copied();
    for _ in 0..=parents.len() {
        let Some(ancestor) = current else {
            break;
        };
        ancestors.push(ancestor);
        current = parents.get(&ancestor).copied();
    }
    ancestors
}
