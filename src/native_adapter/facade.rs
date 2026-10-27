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
pub use crate::frame_schedule::FrameDamage;
/// External-service display client; no authoritative editor document or input engine.
// BEGIN VERIFIED TEXT DISPLAY TYPE
pub struct TextDisplayView {inner:native_view::NativeTextView}
// END VERIFIED TEXT DISPLAY TYPE
impl TextDisplayView {
    pub fn new(owner:u64,context:UiInputContext,font:&[u8],text:&str,width:u32,height:u32)->Self {
        Self {inner:native_view::NativeTextView::new(owner,context,font,text,width,height)}
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
    pub fn semantics(&self,owner:u64,version:u64)->Option<SceneSemanticsSnapshot> {
        self.inner.semantics_snapshot_from_host(owner,version)
    }
    pub fn activate(&self,owner:u64,version:u64,node:NodeId,sink:&mut impl FnMut(SceneActivationEvent))->bool {
        self.inner.activate_from_host(owner,version,node,sink)
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
