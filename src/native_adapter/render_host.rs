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
use crate::image_resource::{ImageError, ImageInfo, ImageLimits, ImageReport};
use crate::native::{SceneFocusEvent, SceneQuota, SceneUpdate, SceneUpdateError};
use crate::scene::{NodeId, Scene, ScenePatch};
#[cfg(feature = "cpu-pending")]
use crate::surface_lifecycle::RenderedPixels;
use crate::surface_lifecycle::{
    SurfaceCoordinator, SurfaceLifecycleError, SurfaceStatus, SurfaceStepError,
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

    /// E14 edit events; see `Engine::take_edit_events` (independent of the surface lifecycle).
    /// E14 编辑事件；见 `Engine::take_edit_events`（与 Surface 生命周期无关）。
    pub fn take_edit_events(&mut self, owner: u64) -> Option<Vec<crate::native::SceneEditEvent>> {
        self.inner.take_edit_events(owner)
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

    pub fn remove_image(&mut self, owner: u64, id: &str) -> Option<bool> {
        self.inner.remove_image(owner, id)
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
