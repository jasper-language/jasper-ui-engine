//! Rendering-host coordinator around the canonical native engine.
//! 正式原生引擎之上的渲染宿主协调器。
//!
//! It owns no window, native Surface, GPU device or swapchain. It binds host-owned resources to
//! authenticated owner, version and presentation generations, and coordinates pause/resume/retire.
//! 它不持有窗口、原生 Surface、GPU 设备或交换链；只把宿主持有资源绑定到已认证归属、版本与
//! 呈现代数，并协调暂停、恢复和退休。
use crate::native::host::SceneHostView;
#[cfg(feature = "gpu-pending")]
use crate::native::host::{GpuFrameError, GpuRenderer};
use crate::native::{SceneFocusEvent, SceneUpdate, SceneUpdateError};
use crate::scene::{NodeId, Scene, ScenePatch};
use crate::surface_lifecycle::{
    SurfaceLifecycle, SurfaceLifecycleError, SurfaceState, SurfaceStatus,
};

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

impl From<SurfaceLifecycleError> for HostRenderError {
    fn from(error: SurfaceLifecycleError) -> Self {
        Self::Lifecycle(error)
    }
}

/// One native view plus its host-owned presentation lifecycle.
/// 一个原生视图及其宿主持有的呈现生命周期。
pub struct HostRenderer {
    owner: u64,
    inner: SceneHostView,
    surface: SurfaceLifecycle,
    gpu_present_pending: bool,
}

impl HostRenderer {
    pub fn new(owner: u64, scene: &Scene, font: &[u8], width: u32, height: u32) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }
        Some(Self {
            owner,
            inner: SceneHostView::new(owner, scene, font, width, height)?,
            surface: SurfaceLifecycle::new(owner, width, height),
            gpu_present_pending: false,
        })
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
            surface: SurfaceLifecycle::new(owner, width, height),
            gpu_present_pending: false,
        })
    }

    pub fn surface_status(&self, caller: u64) -> Option<SurfaceStatus> {
        self.surface.status(caller)
    }

    pub fn needs_frame(&self, owner: u64, generation: u64) -> Option<bool> {
        if !self.surface.render_allowed(owner, generation) {
            return None;
        }
        Some(self.gpu_present_pending || self.inner.needs_frame(self.owner)?)
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
        let status = self.preflight_advance(owner, generation)?;
        if status.state != SurfaceState::Active {
            return Err(HostRenderError::Lifecycle(
                SurfaceLifecycleError::WrongState,
            ));
        }
        if width == 0 || height == 0 {
            return Err(HostRenderError::ViewportRejected);
        }
        if !self.inner.set_viewport(owner, width, height, scale) {
            return Err(HostRenderError::ViewportRejected);
        }
        // A host Surface replacement invalidates its old pixel copy, not only layout.
        // 宿主替换 Surface 时也必须废弃旧像素副本，而不仅是重新布局。
        self.inner.invalidate_frame(owner);
        self.gpu_present_pending = false;
        Ok(self.surface.reconfigure(owner, generation, width, height)?)
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
        let status = self.preflight_advance(owner, generation)?;
        if status.state != SurfaceState::Active {
            return Err(HostRenderError::Lifecycle(
                SurfaceLifecycleError::WrongState,
            ));
        }
        if !self.inner.deactivate(owner, version, focus) {
            return Err(HostRenderError::ViewRetired);
        }
        self.gpu_present_pending = false;
        Ok(self.surface.suspend(owner, generation)?)
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
        let status = self
            .surface
            .status(owner)
            .ok_or(HostRenderError::Lifecycle(
                SurfaceLifecycleError::WrongOwner,
            ))?;
        if status.state != SurfaceState::Suspended {
            return Err(HostRenderError::Lifecycle(
                SurfaceLifecycleError::WrongState,
            ));
        }
        if status.generation != generation {
            return Err(HostRenderError::Lifecycle(
                SurfaceLifecycleError::StaleGeneration,
            ));
        }
        if generation == u64::MAX {
            return Err(HostRenderError::Lifecycle(
                SurfaceLifecycleError::GenerationExhausted,
            ));
        }
        if width == 0 || height == 0 {
            return Err(HostRenderError::ViewportRejected);
        }
        if !self.inner.set_viewport(owner, width, height, scale) {
            return Err(HostRenderError::ViewportRejected);
        }
        self.inner.invalidate_frame(owner);
        Ok(self.surface.resume(owner, generation, width, height)?)
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
        self.preflight(owner, generation)?;
        if !self.inner.close_with_focus_events(owner, version, focus) {
            return Err(HostRenderError::ViewRetired);
        }
        self.gpu_present_pending = false;
        self.surface.retire(owner, generation)?;
        Ok(())
    }

    /// Retire without an epoch. Preferred only for error paths that do not own a current surface.
    /// 不带代数的兼容退休；仅建议用于不持有当前 Surface 的错误清理路径。
    pub fn close(
        &mut self,
        owner: u64,
        version: u64,
        focus: &mut impl FnMut(SceneFocusEvent),
    ) -> bool {
        if !self.inner.close_with_focus_events(owner, version, focus) {
            return false;
        }
        self.gpu_present_pending = false;
        self.surface.retire_current(owner)
    }

    pub fn apply(&mut self, owner: u64, expected: u64, next: u64, patches: &[ScenePatch]) -> bool {
        if !self.surface.active_for_owner(owner) {
            return false;
        }
        let applied = self.inner.apply(owner, expected, next, patches);
        if applied {
            self.gpu_present_pending = false;
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
            self.gpu_present_pending = false;
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

    pub fn focus(
        &mut self,
        owner: u64,
        version: u64,
        generation: u64,
        node: NodeId,
        sink: &mut impl FnMut(SceneFocusEvent),
    ) -> bool {
        self.preflight_active(owner, generation).is_ok()
            && self.inner.focus(owner, version, node, sink)
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
        self.preflight_active(owner, generation).is_ok()
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
        self.preflight_active(owner, generation).is_ok()
            && self.inner.key(owner, version, input, key, focus)
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
            self.gpu_present_pending = false;
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
        self.preflight_active(owner, generation).is_ok()
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
        self.preflight_active(owner, generation).is_ok()
            && self.inner.deactivate(owner, version, focus)
    }

    pub fn invalidate_frame(&mut self, owner: u64, generation: u64) -> bool {
        if !self.surface.render_allowed(owner, generation) {
            return false;
        }
        self.gpu_present_pending = false;
        self.inner.invalidate_frame(owner)
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
        if !self.surface.render_allowed(owner, generation) {
            return false;
        }
        let rendered = self
            .inner
            .render_to_surface(owner, version, bytes, stride, budget);
        if rendered {
            self.gpu_present_pending = false;
        }
        rendered
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
        if !self.surface.render_allowed(owner, generation) {
            return None;
        }
        let damage = self
            .inner
            .render_to_surface_with_damage(owner, version, bytes, stride, budget);
        if damage.is_some() {
            self.gpu_present_pending = false;
        }
        damage
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
        if !self.surface.render_allowed(owner, generation) {
            return Err(crate::backend::BackendError::Invalid(
                "stale or inactive host surface generation".into(),
            ));
        }
        let result = self
            .inner
            .render_to_backend(owner, version, backend, budget);
        if result.is_ok() {
            self.gpu_present_pending = false;
        } else {
            self.inner.invalidate_frame(owner);
        }
        result
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
        if !self.surface.render_allowed(owner, generation) {
            return Err(GpuFrameError::Rejected);
        }
        let result = self.inner.render_gpu(owner, version, gpu, budget);
        if result.is_ok() {
            self.gpu_present_pending = false;
        } else {
            self.inner.invalidate_frame(owner);
        }
        result
    }

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
        if !self.surface.render_allowed(owner, generation) {
            return Err(GpuFrameError::Rejected);
        }
        let result = self
            .inner
            .render_gpu_to_host_view(owner, version, gpu, format, view, budget);
        if result.is_ok() {
            self.gpu_present_pending = true;
        }
        result
    }

    /// Acknowledge that the host's `Queue::present` succeeded for the exact live generation.
    /// Until this succeeds, `needs_frame` keeps the texture frame pending.
    /// 确认宿主对精确当前代数成功执行 `Queue::present`；确认前 `needs_frame` 保持该纹理帧待呈现。
    #[cfg(feature = "gpu-pending")]
    pub fn gpu_presented(&mut self, owner: u64, generation: u64) -> bool {
        if !self.surface.render_allowed(owner, generation) || !self.gpu_present_pending {
            return false;
        }
        self.gpu_present_pending = false;
        true
    }

    fn preflight_active(&self, owner: u64, generation: u64) -> Result<(), HostRenderError> {
        self.preflight(owner, generation)?;
        let status = self
            .surface
            .status(owner)
            .ok_or(HostRenderError::Lifecycle(
                SurfaceLifecycleError::WrongOwner,
            ))?;
        if status.state != SurfaceState::Active {
            return Err(HostRenderError::Lifecycle(
                SurfaceLifecycleError::WrongState,
            ));
        }
        Ok(())
    }

    fn preflight_advance(
        &self,
        owner: u64,
        generation: u64,
    ) -> Result<SurfaceStatus, HostRenderError> {
        let status = self
            .surface
            .status(owner)
            .ok_or(HostRenderError::Lifecycle(
                SurfaceLifecycleError::WrongOwner,
            ))?;
        if status.generation != generation {
            return Err(HostRenderError::Lifecycle(
                SurfaceLifecycleError::StaleGeneration,
            ));
        }
        if generation == u64::MAX {
            return Err(HostRenderError::Lifecycle(
                SurfaceLifecycleError::GenerationExhausted,
            ));
        }
        Ok(status)
    }

    fn preflight(&self, owner: u64, generation: u64) -> Result<(), HostRenderError> {
        let status = self
            .surface
            .status(owner)
            .ok_or(HostRenderError::Lifecycle(
                SurfaceLifecycleError::WrongOwner,
            ))?;
        if status.generation != generation {
            return Err(HostRenderError::Lifecycle(
                SurfaceLifecycleError::StaleGeneration,
            ));
        }
        Ok(())
    }
}
