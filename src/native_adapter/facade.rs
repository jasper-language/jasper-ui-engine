pub use update_contract::SceneUpdate;
pub use jasper_ui_engine::style;
pub use jasper_ui_engine::geometry::{Point,Rect};
pub use jasper_ui_engine::{scene::{Scene,ScenePatch,Node,NodeId},semantics::{Semantics,Role}};
pub use native_view::PointerAction;
pub use native_semantics::{SemanticRole as TextSemanticRole,ViewSemantics as TextSemantics};
pub use component_contract::{ComponentUpdate,ContentFlow,TextAppearance};
pub use native_keys::{HostKeyInput,KeyLocation,NativeKeyEvent as CommandKeyEvent};
pub use native_events::{NativeFocusEvent as TextFocusEvent,NativePointerEvent as TextPointerEvent,NativeActivationEvent as TextActivationEvent,ViewPart as TextPart};
pub use jasper_ui_engine::input::Modifiers;
pub use native_events::NativeEventReply as EventReply;
pub use scene_events::{ScenePointerInput,SceneKeyEvent,SceneEvent,SceneEventKind,SceneSemanticsNode,SceneSemanticsSnapshot,SceneActivationEvent,SceneFocusEvent};

/// Embedding-host adapter; application declarations use Scene/ScenePatch values above.
/// This module does not acquire windows, surfaces, devices or OS services.
pub mod host {
use super::*;
pub use jasper_ui_engine::input_identity::{UiInputBinding,UiInputContext,UiInputTicket};
pub use jasper_ui_engine::composition_identity::UiCompositionToken;
#[cfg(feature="cpu-pending")]
pub use crate::frame_schedule::{FrameDamage,FramePace};
pub use crate::access_tree::{AccessActions,AccessibilityNode,AccessibilityTree,AccessibilityUpdate};
/// The pinned AccessKit crate the bridge emits and accepts; hosts build their platform adapter on it.
/// 桥所输出与接受的固定 AccessKit crate；宿主在其上构建平台适配器。
#[cfg(feature="accesskit-pending")]
pub use ::accesskit;
#[cfg(feature="accesskit-pending")]
pub use crate::access_kit::{AccessRequest,access_request,accesskit_role};
#[cfg(feature="gpu-pending")]
pub use crate::gpu_render::{GpuRenderer,GpuFrameError};
/// External-service display client; no authoritative editor document or input engine.
// BEGIN VERIFIED TEXT DISPLAY TYPE
pub struct TextDisplayView {inner:native_view::NativeTextView}
// END VERIFIED TEXT DISPLAY TYPE
impl TextDisplayView {
    pub fn new(owner:u64,context:UiInputContext,font:&[u8],text:&str,width:u32,height:u32)->Self {
        Self {inner:native_view::NativeTextView::new(owner,context,font,text,width,height)}
    }
    /// Host-ordered fallback stack (e.g. Latin file then CJK file); None for an empty,
    /// oversized (> MAX_INJECTED_FONTS) stack or a file that registers no font family.
    /// Measurement, hit testing and painting share the one Parley layout.
    /// 宿主有序回退栈；空栈、超限或任一文件无字体族时返回 None；测量、命中与绘制共用同一布局。
    pub fn with_fonts(owner:u64,context:UiInputContext,fonts:&[&[u8]],text:&str,width:u32,height:u32)->Option<Self> {
        Some(Self {inner:native_view::NativeTextView::new_with_fonts(owner,context,fonts,text,width,height)?})
    }
    pub fn focus(&mut self,binding:&UiInputBinding,owner:u64,ticket:UiInputTicket,sink:&mut impl FnMut(TextFocusEvent))->bool {
        self.inner.focus_from_host_with_events(binding,owner,ticket,sink)
    }
    /// Revokes only the exact UI binding; no external input-service cancel acknowledgement.
    pub fn close_focus(&mut self,binding:&mut UiInputBinding,owner:u64,ticket:UiInputTicket,sink:&mut impl FnMut(TextFocusEvent))->bool {
        self.inner.close_focus_from_host_with_events(binding,owner,ticket,sink)
    }
    /// Command keys only; text commits arrive as external display snapshots.
    pub fn key(&mut self,binding:&UiInputBinding,owner:u64,ticket:UiInputTicket,input:&HostKeyInput<'_>,sink:&mut impl FnMut(CommandKeyEvent)->EventReply)->bool {
        self.inner.key_from_host(binding,owner,ticket,input,sink)
    }
    /// Consume a Framework update frame; no Signal scheduling or runtime is implemented.
    pub fn update(&mut self,owner:u64,context:UiInputContext,expected:u64,updates:&[ComponentUpdate<'_>])->bool {
        self.inner.apply_component_batch(owner,context,expected,updates)
    }

    /// Current display semantics for the embedding host; native focus excludes window focus.
    /// This read does not acknowledge an input lease or implement a platform accessibility API.
    pub fn semantics(&self,owner:u64,context:UiInputContext)->Option<TextSemantics> {
        self.inner.semantics_from_host(owner,context)
    }
    pub fn snapshot(&mut self,owner:u64,expected:UiInputContext,next:UiInputContext,text:&str)->bool {
        self.inner.apply_display_snapshot_from_host(owner,expected,next,text)
    }
    pub fn begin_preedit(&mut self,binding:&UiInputBinding,owner:u64,ticket:UiInputTicket)->Option<UiCompositionToken> {
        self.inner.begin_preedit_from_host(binding,owner,ticket)
    }
    pub fn preedit(&mut self,binding:&UiInputBinding,owner:u64,token:UiCompositionToken,text:Option<&str>)->bool {
        self.inner.set_preedit_from_host(binding,owner,token,text)
    }
    pub fn cancel_preedit(&mut self,owner:u64,token:UiCompositionToken)->bool {
        self.inner.cancel_preedit_from_host(owner,token)
    }
    pub fn selection(&mut self,binding:&UiInputBinding,owner:u64,ticket:UiInputTicket,range:Option<(usize,usize)>)->bool {
        self.inner.set_selection_from_host(binding,owner,ticket,range)
    }
    /// Returns display UTF-8 byte position only; caller sends any edit/caret request externally.
    pub fn hit(&self,binding:&UiInputBinding,owner:u64,ticket:UiInputTicket,x:f32,y:f32)->Option<usize> {
        self.inner.hit_display_text_from_host(binding,owner,ticket,x,y)
    }
    /// Read-only selected display bytes; does not access clipboard or apply edits.
    pub fn selected_text(&self,binding:&UiInputBinding,owner:u64,ticket:UiInputTicket)->Option<String> {
        self.inner.selected_display_text_from_host(binding,owner,ticket)
    }
    pub fn set_scroll(&mut self,binding:&UiInputBinding,owner:u64,ticket:UiInputTicket,x:f64,y:f64)->bool {
        self.inner.set_scroll_from_host(binding,owner,ticket,x,y)
    }
    pub fn resize(&mut self,owner:u64,context:UiInputContext,width:u32,height:u32)->bool {
        if !self.inner.host_context_matches(owner,context) {return false;}
        self.inner.resize(width,height)
    }
    pub fn set_scale(&mut self,owner:u64,context:UiInputContext,scale:f32)->bool {
        if !self.inner.host_context_matches(owner,context) {return false;}
        self.inner.set_host_scale(scale)
    }
    #[cfg(feature="cpu-pending")]
    pub fn render_cpu(&mut self,owner:u64,context:UiInputContext,budget:usize)->Option<Vec<u8>> {
        if !self.inner.host_context_matches(owner,context) {return None;}
        self.inner.render_cpu(budget)
    }

}
// BEGIN VERIFIED TEXT DISPLAY CLOSE KERNEL
impl TextDisplayView {
    pub fn close(&mut self,owner:u64,context:UiInputContext)->bool {
        if !self.inner.host_context_matches(owner,context) {return false;}
        self.inner.close()
    }
}
// END VERIFIED TEXT DISPLAY CLOSE KERNEL
// BEGIN VERIFIED TEXT DISPLAY VERSION KERNEL
impl TextDisplayView {
    pub fn update_version(&self,owner:u64,context:UiInputContext)->Option<u64> {
        if !self.inner.host_context_matches(owner,context) {return None;}
        self.inner.component_version()
    }
}
// END VERIFIED TEXT DISPLAY VERSION KERNEL
pub use crate::pointer_host::PointerCaptureHost;
/// Capture-client coordination only; the injected host authenticates and ACKs each grant.
pub struct TextPointerCapture<F,G> {inner:pointer_host::PointerHostBridge<F,G>}
impl<F:FnMut(TextPointerEvent)->EventReply,G:FnMut(TextActivationEvent)->EventReply> TextPointerCapture<F,G> {
    pub fn new(owner:u64,slot:u64,events:F,activation:G)->Self {
        Self {inner:pointer_host::PointerHostBridge::with_sinks(owner,slot,events,activation)}
    }
    pub fn down(&mut self,view:&mut TextDisplayView,host:&mut impl PointerCaptureHost,owner:u64,context:UiInputContext,x:f32,y:f32)->Option<UiInputTicket> {
        self.inner.down(&mut view.inner,host,owner,context,x,y)
    }
    pub fn move_pointer(&mut self,view:&mut TextDisplayView,owner:u64,ticket:UiInputTicket,x:f32,y:f32)->bool {
        self.inner.move_pointer(&mut view.inner,owner,ticket,x,y)
    }
    pub fn up(&mut self,view:&mut TextDisplayView,host:&mut impl PointerCaptureHost,owner:u64,ticket:UiInputTicket,x:f32,y:f32)->bool {
        self.inner.up_at(&mut view.inner,host,owner,ticket,x,y)
    }
    pub fn down_with_modifiers(&mut self,view:&mut TextDisplayView,host:&mut impl PointerCaptureHost,owner:u64,
        context:UiInputContext,x:f32,y:f32,modifiers:Modifiers)->Option<UiInputTicket> {
        self.inner.down_with_modifiers(&mut view.inner,host,owner,context,x,y,modifiers)
    }
    pub fn move_with_modifiers(&mut self,view:&mut TextDisplayView,owner:u64,ticket:UiInputTicket,
        x:f32,y:f32,modifiers:Modifiers)->bool {
        self.inner.move_with_modifiers(&mut view.inner,owner,ticket,x,y,modifiers)
    }
    pub fn up_with_modifiers(&mut self,view:&mut TextDisplayView,host:&mut impl PointerCaptureHost,owner:u64,
        ticket:UiInputTicket,x:f32,y:f32,modifiers:Modifiers)->bool {
        self.inner.up_with_modifiers(&mut view.inner,host,owner,ticket,x,y,modifiers)
    }
    pub fn cancel(&mut self,view:&mut TextDisplayView,host:&mut impl PointerCaptureHost,owner:u64,ticket:UiInputTicket)->bool {
        self.inner.cancel(&mut view.inner,host,owner,ticket)
    }
    pub fn focus_lost(&mut self,view:&mut TextDisplayView,host:&mut impl PointerCaptureHost,binding:&mut UiInputBinding,owner:u64,ticket:UiInputTicket,sink:&mut impl FnMut(TextFocusEvent))->bool {
        self.inner.focus_lost_with_events(&mut view.inner,host,binding,owner,ticket,sink)
    }
    pub fn retire(&mut self,view:&mut TextDisplayView,host:&mut impl PointerCaptureHost,owner:u64,context:UiInputContext)->bool {
        self.inner.retire_from_host(&mut view.inner,host,owner,context)
    }
    pub fn retry_release(&mut self,host:&mut impl PointerCaptureHost,owner:u64)->bool {
        self.inner.retry_release(host,owner)
    }
}
/// Host-authenticated owner is supplied by the embedding layer, not an application credential.
// BEGIN VERIFIED SCENE HOST TYPE
pub struct SceneHostView {inner:native_scene::NativeScene}
// END VERIFIED SCENE HOST TYPE
impl SceneHostView {
    pub fn new(owner:u64,scene:&Scene,font:&[u8],width:u32,height:u32)->Option<Self> {
        Some(Self {inner:native_scene::NativeScene::from_scene(owner,scene,font,width,height)?})
    }
    /// Host-ordered fallback stack (e.g. Latin file then CJK file); None for an empty,
    /// oversized (> MAX_INJECTED_FONTS) stack or a file that registers no font family.
    /// 宿主有序回退栈（如先拉丁后中日韩文件）；空栈、超限或任一文件无字体族时返回 None。
    pub fn with_fonts(owner:u64,scene:&Scene,fonts:&[&[u8]],width:u32,height:u32)->Option<Self> {
        Some(Self {inner:native_scene::NativeScene::from_scene_with_fonts(owner,scene,fonts,width,height)?})
    }
    pub fn apply(&mut self,owner:u64,expected:u64,next:u64,patches:&[ScenePatch])->bool {
        self.inner.apply_patches(owner,expected,next,patches)
    }
    /// Consume a Framework update value; authenticated owner stays in the embedding host.
    /// 消费 Framework 更新值；已认证的归属身份只由嵌入宿主提供。
    pub fn apply_update(&mut self,owner:u64,update:SceneUpdate<'_>)->bool {
        self.inner.apply_patches(owner,update.expected_version,update.next_version,update.patches)
    }
    /// Unclipped logical client border box for a source-visible node, not a paint/hit rectangle.
    /// Returns None for a node with its own nonzero scroll (pinned backend geometry limitation).
    /// Embedding host owns OS screen transforms and native accessibility API mapping.
    pub fn node_bounds(&self,owner:u64,version:u64,node:NodeId)->Option<Rect> {
        self.inner.node_bounds_from_host(owner,version,node)
    }
    /// UTF-8 offset in a text node's source text nearest a view-local logical point, from the
    /// same native layout that paints it; for caret placement, not an occlusion-aware hit test.
    /// None for a non-text/hidden node, a stale version, a node with its own scroll, or a layout
    /// whose text differs from the source (e.g. collapsed whitespace).
    /// 文字节点源文本中离视图局部逻辑点最近的 UTF-8 偏移，取自绘制所用的同一原生布局；用于放置光标，
    /// 不是考虑遮挡的命中测试。非文字/隐藏节点、过期版本、自身有滚动、或布局文本与源文本不同（如空白折叠）
    /// 时为 None。
    pub fn text_offset_at(&self,owner:u64,version:u64,node:NodeId,x:f32,y:f32)->Option<usize> {
        self.inner.text_offset_from_host(owner,version,node,x,y)
    }
    /// Caret rectangle `width` logical px wide before a source-text UTF-8 offset (end included),
    /// in node_bounds coordinates. None for a non-boundary or out-of-range offset and the
    /// text_offset_at rejections.
    /// 源文本 UTF-8 偏移（含末尾）之前、宽 `width` 逻辑像素的光标矩形，坐标同 node_bounds。
    /// 非字符边界或越界偏移，以及 text_offset_at 的各拒绝情形为 None。
    pub fn text_caret(&self,owner:u64,version:u64,node:NodeId,offset:usize,width:f32)->Option<Rect> {
        self.inner.text_caret_from_host(owner,version,node,offset,width)
    }
    /// Selection rectangles for the source-text UTF-8 range [start, end) (start ≤ end), per
    /// laid-out line in node_bounds coordinates, from the layout that paints the text; an empty
    /// range gives no rectangles. None for a reversed, non-boundary or out-of-range range and the
    /// text_offset_at rejections.
    /// 源文本 UTF-8 区间 [start, end)（start ≤ end）的选区矩形，逐行、坐标同 node_bounds，取自绘制所用
    /// 布局；空区间无矩形。反向、非字符边界或越界区间，以及 text_offset_at 的各拒绝情形为 None。
    pub fn text_selection(&self,owner:u64,version:u64,node:NodeId,start:usize,end:usize)->Option<Vec<Rect>> {
        self.inner.text_selection_from_host(owner,version,node,start,end)
    }
    pub fn semantics(&self,owner:u64,version:u64)->Option<SceneSemanticsSnapshot> {
        self.inner.semantics_snapshot_from_host(owner,version)
    }
    /// Accessibility tree for a platform bridge: visible nodes in depth-first source order with
    /// Framework semantics, node_bounds geometry, scene focus and the focus/activate actions this
    /// engine admits; hidden or undisplayed subtrees are left out. None for a non-owner, a stale
    /// version or a closed view. No platform accessibility API is called.
    /// 供平台无障碍桥消费的无障碍树：可见节点按源顺序深度优先，含 Framework 语义、node_bounds 几何、
    /// 场景焦点与本引擎准入的 focus/activate 动作；隐藏或未显示的子树剔除。非归属方、过期版本或已关闭
    /// 视图为 None。不调用任何平台无障碍 API。
    pub fn accessibility_tree(&self,owner:u64,version:u64)->Option<AccessibilityTree> {
        self.inner.accessibility_tree_from_host(owner,version)
    }
    pub fn activate(&self,owner:u64,version:u64,node:NodeId,sink:&mut impl FnMut(SceneActivationEvent))->bool {
        self.inner.activate_from_host(owner,version,node,sink)
    }
    /// AccessKit action request from the host's platform adapter: admitted only when the target
    /// is in this version's accessibility tree and offers the action (Focus → focus, Click →
    /// activate); anything else is refused without effect. Trees come from
    /// `AccessibilityTree::to_accesskit` / `AccessibilityUpdate::to_accesskit`.
    /// 宿主平台适配器送来的 AccessKit 动作请求：仅当目标在本版本无障碍树中且提供该动作时准入
    /// （Focus → focus，Click → activate）；其余一律拒绝且无副作用。树由 `AccessibilityTree::to_accesskit` /
    /// `AccessibilityUpdate::to_accesskit` 产生。
    #[cfg(feature="accesskit-pending")]
    pub fn accesskit_action(&mut self,owner:u64,version:u64,request:&accesskit::ActionRequest,
        focus:&mut impl FnMut(SceneFocusEvent),activation:&mut impl FnMut(SceneActivationEvent))->bool {
        crate::access_kit::route_action(self,owner,version,request,focus,activation)
    }
    /// Command key to native scene focus; text-service commits use the display client.
    pub fn key(&mut self,owner:u64,version:u64,input:&HostKeyInput<'_>,
        sink:&mut impl FnMut(SceneKeyEvent)->EventReply,focus:&mut impl FnMut(SceneFocusEvent))->bool {
        self.inner.key_from_host(owner,version,input,sink,focus)
    }
    /// Routed mouse sample; platform capture ownership/ACK remains outside this adapter.
    pub fn pointer(&mut self,owner:u64,version:u64,phase:PointerAction,x:f32,y:f32,pressed:bool,
        events:&mut impl FnMut(SceneEvent)->EventReply,focus:&mut impl FnMut(SceneFocusEvent))->bool {
        self.inner.pointer_with_focus_events_from_host(owner,version,phase,x,y,pressed,events,focus)
    }
    /// Typed mouse sample preserving common modifiers through native event routing.
    pub fn pointer_input(&mut self,owner:u64,version:u64,input:&ScenePointerInput,
        events:&mut impl FnMut(SceneEvent)->EventReply,focus:&mut impl FnMut(SceneFocusEvent))->bool {
        let before=self.inner.focused_node(owner,version);
        if !self.inner.pointer_input_from_host(owner,version,input,events) {return false;}
        let after=self.inner.focused_node(owner,version);
        scene_events::notify_focus_transition(version,before,after,focus);true
    }
    /// Current scene-local focus; embedding host combines OS window/application focus.
    pub fn focused_node(&self,owner:u64,version:u64)->Option<NodeId> {
        self.inner.focused_node(owner,version)
    }
    pub fn focus(&mut self,owner:u64,version:u64,node:NodeId,sink:&mut impl FnMut(SceneFocusEvent))->bool {
        self.inner.focus_from_host(owner,version,node,sink)
    }
    /// Clear scene-local hover/press/focus on host deactivation; no OS capture/service ACK.
    /// Host separately delivers its routed cancellation sample if Framework needs that event.
    pub fn deactivate(&mut self,owner:u64,version:u64,sink:&mut impl FnMut(SceneFocusEvent))->bool {
        self.inner.deactivate_from_host(owner,version,sink)
    }
    pub fn clear_focus(&mut self,owner:u64,version:u64,sink:&mut impl FnMut(SceneFocusEvent))->bool {
        self.inner.clear_focus_from_host(owner,version,sink)
    }
    pub fn apply_with_focus_events(&mut self,owner:u64,expected:u64,next:u64,patches:&[ScenePatch],sink:&mut impl FnMut(SceneFocusEvent))->bool {
        self.inner.apply_patches_with_focus_events(owner,expected,next,patches,sink)
    }
    pub fn close_with_focus_events(&mut self,owner:u64,version:u64,sink:&mut impl FnMut(SceneFocusEvent))->bool {
        self.inner.close_with_focus_events(owner,version,sink)
    }
    /// Host consumes this interface; application component declarations carry no window details.
    pub fn set_viewport(&mut self,owner:u64,width:u32,height:u32,scale:f32)->bool {
        self.inner.set_viewport_from_host(owner,width,height,scale)
    }
    pub fn set_viewport_scroll(&mut self,owner:u64,version:u64,x:f64,y:f64)->bool {
        self.inner.set_viewport_scroll_from_host(owner,version,x,y)
    }
    /// Native GPU frame (Vello hybrid on wgpu) read back as compact premultiplied RGBA8. Err(Rejected)
    /// for a foreign owner, stale version, closed view or over-budget extent; device/render/readback
    /// failures are reported, never replaced by a CPU frame. GPU and CPU frames share one frame
    /// cache: an unchanged view reuses the last produced frame (from either path) without GPU
    /// work — call `invalidate_frame` first to force a GPU repaint.
    /// 原生 GPU 帧（wgpu 上的 Vello hybrid），读回为紧凑预乘 RGBA8。非归属方、过期版本、已关闭视图或
    /// 超额尺寸为 Err(Rejected)；设备/渲染/读回失败如实报告，绝不以 CPU 帧顶替。GPU 与 CPU 帧共用
    /// 帧缓存：未变化视图不做 GPU 工作而复用最近产出的帧（任一路径）；需强制 GPU 重绘时先 `invalidate_frame`。
    #[cfg(feature="gpu-pending")]
    pub fn render_gpu(&mut self,owner:u64,version:u64,gpu:&mut GpuRenderer,budget:usize)->Result<Vec<u8>,GpuFrameError> {
        self.inner.render_gpu(owner,version,gpu,budget)
    }
    /// `render_gpu` copied into a host software surface (stride/padding preserved); surface
    /// admission precedes GPU work.
    /// `render_gpu` 写入宿主软件 Surface（保留 stride/padding）；Surface 准入先于 GPU 工作。
    #[cfg(feature="gpu-pending")]
    pub fn render_gpu_to_surface(&mut self,owner:u64,version:u64,gpu:&mut GpuRenderer,bytes:&mut [u8],stride:usize,budget:usize)
        ->Result<(),GpuFrameError> {
        self.inner.render_gpu_to_host_surface(owner,version,gpu,bytes,stride,budget)
    }
    /// `render_gpu_to_surface` that also reports damage relative to the last frame this view
    /// produced on either path (Full for the first frame, after an extent change or
    /// `invalidate_frame`). The whole frame is still painted and written.
    /// 同 `render_gpu_to_surface`，并报告相对本视图最近产出帧（任一路径）的损伤（首帧、尺寸变化或
    /// `invalidate_frame` 之后为 Full）。仍整帧绘制与写入。
    #[cfg(feature="gpu-pending")]
    pub fn render_gpu_to_surface_with_damage(&mut self,owner:u64,version:u64,gpu:&mut GpuRenderer,bytes:&mut [u8],stride:usize,budget:usize)
        ->Result<FrameDamage,GpuFrameError> {
        self.inner.render_gpu_to_host_surface_with_damage(owner,version,gpu,bytes,stride,budget)
    }
    /// Like `render_gpu_to_surface_with_damage`, but copies only the damaged pixels into
    /// `bytes`, which must still hold the frame this view produced last (after swapping or
    /// losing the buffer call `invalidate_frame` first). Rejected or failed calls write nothing.
    /// 同 `render_gpu_to_surface_with_damage`，但只把受损像素复制进 `bytes`；`bytes` 必须仍持有本视图
    /// 上一次产出的帧（换缓冲或丢失后先 `invalidate_frame`）。被拒或失败不写任何字节。
    #[cfg(feature="gpu-pending")]
    pub fn render_gpu_to_surface_damaged(&mut self,owner:u64,version:u64,gpu:&mut GpuRenderer,bytes:&mut [u8],stride:usize,budget:usize)
        ->Result<FrameDamage,GpuFrameError> {
        self.inner.render_gpu_to_host_surface_damaged(owner,version,gpu,bytes,stride,budget)
    }
    /// Native CPU frame to an embedding backend; unsupported backends return an explicit error.
    /// 原生 CPU 帧提交至嵌入后端；不支持的后端返回明确错误，呈现责任仍归宿主。
    #[cfg(feature="cpu-pending")]
    pub fn render_to_backend(&mut self,owner:u64,version:u64,
        backend:&mut impl jasper_ui_engine::backend::RenderBackend,budget:usize)
        ->Result<(),jasper_ui_engine::backend::BackendError> {
        self.inner.render_to_backend(owner,version,backend,budget)
    }
    /// On-demand scheduling: Some(true) while a possible visible change is unpainted; hosts
    /// skip rendering on Some(false). A repeated render of an unchanged view reuses the last
    /// painted frame. None for a foreign owner or a closed view.
    /// 按需调度：存在尚未绘制的可能可见变化时为 Some(true)，宿主见 Some(false) 可跳过渲染；
    /// 未变化视图的重复渲染复用上一帧。非归属方或已关闭视图返回 None。
    pub fn needs_frame(&self,owner:u64)->Option<bool> {self.inner.needs_frame(owner)}
    /// Force the next render to repaint, e.g. after the host lost its presentation surface.
    /// 强制下一次渲染重绘，如宿主呈现表面丢失之后。
    pub fn invalidate_frame(&mut self,owner:u64)->bool {self.inner.invalidate_frame_from_host(owner)}
    /// Refresh-clock pacing on top of `needs_frame`: RenderNow(tick) at most once per period
    /// (`origin + k·interval`), WaitUntil(t) when this period already presented a frame or the
    /// first tick is ahead, Idle when nothing is unpainted. `presented` is the tick the host
    /// last presented for this view. None for a foreign owner, a closed view, a zero interval
    /// or clock overflow. Does not paint or wait; the host owns the clock and vsync source.
    /// 在 `needs_frame` 之上按刷新时钟定节拍：每周期（`origin + k·interval`）至多一次
    /// RenderNow(tick)；本周期已呈现或首个节拍未到时 WaitUntil(t)；无待绘变化为 Idle。
    /// `presented` 为宿主上次为本视图呈现的节拍。非归属方、已关闭视图、周期为零或时钟溢出时为
    /// None。本方法不绘制也不等待；时钟与垂直同步来源归宿主。
    #[cfg(feature="cpu-pending")]
    pub fn frame_pace(&self,owner:u64,presented:Option<u64>,origin:u64,interval:u64,now:u64)->Option<FramePace> {
        crate::frame_schedule::frame_pace(self.inner.needs_frame(owner)?,presented,origin,interval,now)
    }
    #[cfg(feature="cpu-pending")]
    pub fn render_to_surface(&mut self,owner:u64,version:u64,bytes:&mut [u8],stride:usize,budget:usize)->bool {
        self.inner.render_to_host_surface(owner,version,bytes,stride,budget)
    }
    /// `render_to_surface` that also reports damage relative to the previous frame this view
    /// produced: Full for the first frame, after an extent change or `invalidate_frame`; None
    /// when rejected. The whole frame is still painted and written.
    /// 同 `render_to_surface`，并报告相对本视图上一帧的损伤：首帧、尺寸变化或 `invalidate_frame`
    /// 之后为 Full；拒绝时为 None。仍整帧绘制与写入。
    #[cfg(feature="cpu-pending")]
    pub fn render_to_surface_with_damage(&mut self,owner:u64,version:u64,bytes:&mut [u8],stride:usize,budget:usize)
        ->Option<FrameDamage> {
        self.inner.render_to_host_surface_with_damage(owner,version,bytes,stride,budget)
    }
    /// Like `render_to_surface_with_damage`, but copies only the damaged pixels into `bytes`
    /// (Full: every row; Rect: the rectangle; Unchanged: nothing). `bytes` must still hold the
    /// frame this view produced last — after swapping or losing the buffer call
    /// `invalidate_frame` first. None when rejected; nothing is written then. Painting is still
    /// whole-frame.
    /// 同 `render_to_surface_with_damage`，但只把受损像素复制进 `bytes`（Full 全部行、Rect 矩形、
    /// Unchanged 不写）。`bytes` 必须仍持有本视图上一次产出的帧——换缓冲或丢失后先调用
    /// `invalidate_frame`。拒绝时为 None 且不写任何字节。绘制仍为整帧。
    #[cfg(feature="cpu-pending")]
    pub fn render_to_surface_damaged(&mut self,owner:u64,version:u64,bytes:&mut [u8],stride:usize,budget:usize)
        ->Option<FrameDamage> {
        self.inner.render_to_host_surface_damaged(owner,version,bytes,stride,budget)
    }

}
// BEGIN VERIFIED SCENE HOST CLOSE KERNEL
impl SceneHostView {
    pub fn close(&mut self,owner:u64)->bool {self.inner.close(owner)}
}
// END VERIFIED SCENE HOST CLOSE KERNEL

/// Stored source version/node count; not native layout statistics.
// BEGIN VERIFIED SCENE HOST IDENTITY KERNEL
impl SceneHostView {
    pub fn snapshot_identity(&self,owner:u64)->Option<(u64,usize)> {
        self.inner.snapshot_identity(owner)
    }
}
// END VERIFIED SCENE HOST IDENTITY KERNEL

}
