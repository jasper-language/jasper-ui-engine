//! Candidate window-capture consumer; no window system or OS input service lives here.
//! 窗口捕获客户端候选；实际授权/捕获/释放由宿主实现，失败保持可重试关联。
use crate::native_view::{NativeTextView, PointerAction};
use crate::native_events::{NativePointerEvent, NativeEventReply, NativeActivationEvent, ViewPart};
use jasper_ui_engine::input_identity::{matches_ui_view_owner, UiInputBinding, UiInputContext, UiInputTicket};

/// Host must authenticate the session/window and acknowledge this exact generation.
/// 宿主必须认证会话/窗口，并确认对应代际；关联票据自身不是认证凭证。
pub trait PointerCaptureHost {
    fn capture(&mut self, ticket: UiInputTicket) -> bool;
    fn release(&mut self, ticket: UiInputTicket) -> bool;
}

type DefaultSink = fn(NativePointerEvent) -> NativeEventReply;

type DefaultActivationSink = fn(NativeActivationEvent) -> NativeEventReply;

pub(crate) struct PointerHostBridge<F = DefaultSink, G = DefaultActivationSink> {
    session: u64,
    binding: UiInputBinding,
    current: Option<UiInputTicket>,
    dragging: bool,
    retiring: bool,
    last: (f32, f32),
    last_modifiers: jasper_ui_engine::input::Modifiers,
    captured_part: Option<ViewPart>,
    sink: F,
    activation_sink: G,
}
impl PointerHostBridge<DefaultSink> {
    pub(crate) fn new(session: u64, slot: u64) -> Self {
        Self::with_sink(session,slot,|_| NativeEventReply::default())
    }
}
impl<F: FnMut(NativePointerEvent) -> NativeEventReply> PointerHostBridge<F> {
    /// 独立注入 Framework 回调，不把应用事件接收职责塞入平台捕获宿主。
    /// Inject Framework callbacks separately from the platform capture host.
    pub(crate) fn with_sink(session: u64, slot: u64, sink: F) -> Self {
        Self::with_sinks(session,slot,sink,|_| NativeEventReply::default())
    }
}
impl<F: FnMut(NativePointerEvent) -> NativeEventReply,
    G: FnMut(NativeActivationEvent) -> NativeEventReply> PointerHostBridge<F,G> {
    /// Both Framework sinks share admitted capture delivery; host only owns capture ACKs.
    pub(crate) fn with_sinks(session: u64, slot: u64, sink: F, activation_sink: G) -> Self {
        Self { session, binding: UiInputBinding::new(session,slot), current: None,
            dragging: false, retiring: false, last: (0.0,0.0), last_modifiers:Default::default(), captured_part: None, sink, activation_sink }
    }
    /// Capture acknowledgement precedes delivery; failed delivery compensates with release.
    /// 先确认捕获再投递；投递失败补偿释放，释放失败则保留票据供宿主重试。
    pub(crate) fn down(&mut self, view: &mut NativeTextView, host: &mut impl PointerCaptureHost,
        caller: u64, context: UiInputContext, x: f32, y: f32) -> Option<UiInputTicket> {
        self.down_with_modifiers(view,host,caller,context,x,y,Default::default())
    }
    pub(crate) fn down_with_modifiers(&mut self,view:&mut NativeTextView,host:&mut impl PointerCaptureHost,
        caller:u64,context:UiInputContext,x:f32,y:f32,modifiers:jasper_ui_engine::input::Modifiers)->Option<UiInputTicket> {
        if caller!=self.session || !view.host_context_matches(caller,context)
            || self.retiring || self.current.is_some() || !x.is_finite() || !y.is_finite()
            || view.component_version().is_none() { return None; }
        let ticket = self.binding.bind(caller,context)?;
        if !host.capture(ticket) { self.binding.close(caller,ticket); return None; }
        self.current = Some(ticket);
        self.last = (x,y);
        self.last_modifiers=modifiers;
        let mut captured_part=None;
        let sink=&mut self.sink;
        let delivered=view.dispatch_pointer_modifiers_from_host(caller,context,PointerAction::Down,x,y,true,modifiers,None,&mut |event| {
            if event.phase==PointerAction::Down { captured_part=Some(event.part); }
            sink(event)
        },Some(&mut self.activation_sink));
        if !delivered {
            let _ = self.cancel(view,host,caller,ticket);
            return None;
        }
        self.captured_part=captured_part;
        self.dragging = true;
        if self.captured_part.is_none() {
            // Down reached Blitz but no Framework component owns this press.
            // 已投递后端但无 Framework 目标：取消后端按压并补偿释放，不能后来改绑命中目标。
            let _ = self.cancel(view,host,caller,ticket);
            return None;
        }
        Some(ticket)
    }
    pub(crate) fn move_pointer(&mut self, view: &mut NativeTextView, caller: u64,
        ticket: UiInputTicket, x: f32, y: f32) -> bool {
        self.move_with_modifiers(view,caller,ticket,x,y,Default::default())
    }
    pub(crate) fn move_with_modifiers(&mut self,view:&mut NativeTextView,caller:u64,ticket:UiInputTicket,
        x:f32,y:f32,modifiers:jasper_ui_engine::input::Modifiers)->bool {
        if !self.dragging || !self.binding.accepts(caller,ticket) { return false; }
        if !view.dispatch_pointer_modifiers_from_host(caller,ticket.context,PointerAction::Move,x,y,true,modifiers,self.captured_part,&mut self.sink,Some(&mut self.activation_sink)) { return false; }
        self.last = (x,y);self.last_modifiers=modifiers;
        true
    }
    pub(crate) fn up(&mut self, view: &mut NativeTextView, host: &mut impl PointerCaptureHost,
        caller: u64, ticket: UiInputTicket) -> bool {
        self.up_with_modifiers(view,host,caller,ticket,self.last.0,self.last.1,self.last_modifiers)
    }
    /// Consume the actual final release sample rather than a cached move position.
    /// 释放使用宿主最终坐标；无效坐标拒绝且保持现有捕获，等待有效释放/取消。
    pub(crate) fn up_at(&mut self, view: &mut NativeTextView, host: &mut impl PointerCaptureHost,
        caller: u64, ticket: UiInputTicket, x: f32, y: f32) -> bool {
        self.up_with_modifiers(view,host,caller,ticket,x,y,Default::default())
    }
    pub(crate) fn up_with_modifiers(&mut self,view:&mut NativeTextView,host:&mut impl PointerCaptureHost,
        caller:u64,ticket:UiInputTicket,x:f32,y:f32,modifiers:jasper_ui_engine::input::Modifiers)->bool {
        if !self.binding.accepts(caller,ticket) || !view.host_context_matches(caller,ticket.context)
            || !x.is_finite() || !y.is_finite() { return false; }
        if self.dragging {
            if !view.dispatch_pointer_modifiers_from_host(caller,ticket.context,PointerAction::Up,x,y,false,modifiers,self.captured_part,&mut self.sink,Some(&mut self.activation_sink)) {
                // A released host grant cannot make a rejected UI delivery successful.
                // 未投递释放事件时保留关联，交由宿主显式取消/退役，不误报完成。
                return false;
            }
        }
        self.last=(x,y);self.last_modifiers=modifiers;
        self.dragging=false;
        self.release(host,caller,ticket)
    }
    /// Focus loss/host cancellation uses the original grant, including after view retirement.
    /// 焦点丢失/宿主取消沿原关联释放；视图已关闭也允许回收宿主捕获。
    pub(crate) fn cancel(&mut self, view: &mut NativeTextView, host: &mut impl PointerCaptureHost,
        caller: u64, ticket: UiInputTicket) -> bool {
        if !self.binding.accepts(caller,ticket) { return false; }
        if !view.cancel_capture_from_host(caller,ticket.context,self.dragging,self.last.0,self.last.1,
            self.last_modifiers,self.captured_part,&mut self.sink,&mut self.activation_sink) {return false;}
        self.dragging = false;
        self.release(host,caller,ticket)
    }
    /// Close an admitted text focus and cancel its view's pointer grant together.
    /// 关闭已准入焦点并取消同视图捕获；释放失败仍清理焦点，保留待重试释放。
    pub(crate) fn focus_lost(&mut self, view: &mut NativeTextView, host: &mut impl PointerCaptureHost,
        input: &mut UiInputBinding, caller: u64, owner: UiInputTicket) -> bool {
        self.focus_lost_with_events(view,host,input,caller,owner,&mut |_| {})
    }

    pub(crate) fn focus_lost_with_events(&mut self, view: &mut NativeTextView, host: &mut impl PointerCaptureHost,
        input: &mut UiInputBinding, caller: u64, owner: UiInputTicket,
        sink: &mut impl FnMut(crate::native_events::NativeFocusEvent)) -> bool {
        if caller!=self.session || !view.host_context_matches(caller,owner.context)
            || !input.accepts(caller,owner) { return false; }
        if let Some(pointer) = self.current {
            if !matches_ui_view_owner(caller,pointer.context,caller,owner.context) { return false; }
        }
        let released = match self.current {
            Some(pointer) => self.cancel(view,host,caller,pointer),
            None => true,
        };
        let closed = view.close_focus_from_host_with_events(input,caller,owner,sink);
        released && closed
    }
    /// Trusted host teardown: block new grants before cancellation and view destruction.
    /// 仅持有此桥的可信宿主调用：先阻止新关联，再取消/销毁视图；失败释放保留重试。
    /// The external text-focus service remains caller-owned and needs its own closure.
    pub(crate) fn retire_from_host(&mut self, view: &mut NativeTextView,
        host: &mut impl PointerCaptureHost, caller: u64, expected: UiInputContext) -> bool {
        if caller!=self.session || !view.host_context_matches(caller,expected) { return false; }
        if let Some(ticket)=self.current {
            if !jasper_ui_engine::input_identity::accepts_ui_display_snapshot(caller,ticket.context,caller,ticket.context,expected) { return false; }
        }
        self.retiring = true;
        let released = match self.current {
            Some(ticket) => self.cancel(view,host,ticket.key.session,ticket),
            None => true,
        };
        let _ = view.close();
        if released { self.binding.retire(); }
        released
    }
    /// Recover a failed release even when a failed Down returned no ticket.
    /// Down 失败且未返回票据时也可回收；只允许重试已停止投递的原宿主关联。
    pub(crate) fn retry_release(&mut self, host: &mut impl PointerCaptureHost, caller: u64) -> bool {
        if self.dragging { return false; }
        let Some(ticket) = self.current else { return false; };
        if !self.binding.accepts(caller,ticket) { return false; }
        self.release(host,caller,ticket)
    }
    fn release(&mut self, host: &mut impl PointerCaptureHost, caller: u64, ticket: UiInputTicket) -> bool {
        if !host.release(ticket) { return false; }
        if !self.binding.close(caller,ticket) { return false; }
        self.current = None;
        self.captured_part = None;
        if self.retiring { self.binding.retire(); }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jasper_ui_engine::scene::NodeId;
    #[derive(Default)]
    struct Host { captures: usize, releases: usize, deny_capture: bool, deny_release: bool }
    impl PointerCaptureHost for Host {
        fn capture(&mut self, _: UiInputTicket) -> bool { self.captures+=1; !self.deny_capture }
        fn release(&mut self, _: UiInputTicket) -> bool { self.releases+=1; !self.deny_release }
    }
    #[test]
    fn idle_bridge_cannot_close_another_applications_valid_focus() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required")).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut foreign_view=NativeTextView::new(11,context,&bytes,"Other app",320,200);
        let mut foreign_input=UiInputBinding::new(11,5);
        let owner=foreign_input.bind(11,context).unwrap();
        assert!(foreign_view.focus_from_host(&foreign_input,11,owner));
        let mut bridge=PointerHostBridge::new(10,4);
        let mut host=Host::default();
        assert!(!bridge.focus_lost_with_events(&mut foreign_view,&mut host,&mut foreign_input,
            11,owner,&mut |_| panic!("foreign focus notification")));
        assert!(foreign_input.accepts(11,owner));
        assert!(foreign_view.semantics_from_host(11,context).unwrap().native_focused);
        assert_eq!(host.captures,0);assert_eq!(host.releases,0);
    }

    #[test]
    fn rejected_release_delivery_retains_capture_until_explicit_cancel() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required")).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut view=NativeTextView::new_button(10,context,&bytes,"Save",320,200);
        let mut bridge=PointerHostBridge::new(10,4);
        let mut host=Host::default();
        let ticket=bridge.down(&mut view,&mut host,10,context,12.0,12.0).unwrap();
        assert!(view.close());
        assert!(!bridge.up_at(&mut view,&mut host,10,ticket,12.0,12.0));
        assert_eq!(host.releases,0);
        assert!(bridge.current.is_some());assert!(bridge.dragging);
        assert!(bridge.cancel(&mut view,&mut host,10,ticket));
        assert_eq!(host.releases,1);assert!(bridge.current.is_none());
    }

    #[test]
    fn capture_release_delivers_button_activation_once_even_when_release_ack_fails() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required")).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut view=NativeTextView::new_button(10,context,&bytes,"Save",320,200);
        let mut activations=0;
        let mut bridge=PointerHostBridge::with_sinks(10,4,|_| NativeEventReply::default(),
            |_| {activations+=1;NativeEventReply::default()});
        let mut host=Host::default();
        let ticket=bridge.down(&mut view,&mut host,10,context,12.0,12.0).unwrap();
        host.deny_release=true;
        assert!(!bridge.up_at(&mut view,&mut host,10,ticket,12.0,12.0));
        host.deny_release=false;
        assert!(bridge.retry_release(&mut host,10));
        let ticket=bridge.down(&mut view,&mut host,10,context,12.0,12.0).unwrap();
        assert!(bridge.cancel(&mut view,&mut host,10,ticket));
        drop(bridge);
        assert_eq!(activations,1);
        let mut text=NativeTextView::new(10,context,&bytes,"Text",320,200);
        let mut bridge=PointerHostBridge::with_sinks(10,5,|_| NativeEventReply::default(),
            |_| panic!("text is not an activatable button"));
        let ticket=bridge.down(&mut text,&mut host,10,context,12.0,12.0).unwrap();
        assert!(bridge.up(&mut text,&mut host,10,ticket));
    }

    #[test]
    fn capture_cancel_retry_and_stale_release_use_real_native_view() {
        let path=std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes=std::fs::read(path).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut view=NativeTextView::new(10,context,&bytes,"drag text",320,200);
        let mut bridge=PointerHostBridge::new(10,4);
        let mut host=Host::default();
        assert!(bridge.down(&mut view,&mut host,11,context,12.0,12.0).is_none());
        assert_eq!(host.captures,0);
        host.deny_capture=true;
        assert!(bridge.down(&mut view,&mut host,10,context,12.0,12.0).is_none());
        host.deny_capture=false;
        host.deny_release=true;
        let stale_context=UiInputContext {revision:4,..context};
        assert!(bridge.down(&mut view,&mut host,10,stale_context,12.0,12.0).is_none());
        assert!(bridge.current.is_none()); // Rejected context must never reach the host now.
        assert!(!bridge.retry_release(&mut host,11));
        host.deny_release=false;
        assert!(!bridge.retry_release(&mut host,10));
        let old=bridge.down(&mut view,&mut host,10,context,12.0,12.0).unwrap();
        assert!(bridge.move_pointer(&mut view,10,old,400.0,12.0));
        host.deny_release=true;
        assert!(!bridge.cancel(&mut view,&mut host,10,old));
        assert!(!bridge.move_pointer(&mut view,10,old,20.0,12.0));
        assert!(bridge.down(&mut view,&mut host,10,context,12.0,12.0).is_none());
        host.deny_release=false;
        assert!(bridge.cancel(&mut view,&mut host,10,old));
        let next=bridge.down(&mut view,&mut host,10,context,12.0,12.0).unwrap();
        let releases=host.releases;
        assert!(!bridge.cancel(&mut view,&mut host,10,old));
        assert!(!bridge.cancel(&mut view,&mut host,11,next));
        assert_eq!(host.releases,releases);
        assert!(bridge.up(&mut view,&mut host,10,next));
        let mut text_focus=UiInputBinding::new(10,5);
        let focus=text_focus.bind(10,context).unwrap();
        let stale_focus=focus;
        let focus=text_focus.replace(10,focus,context).unwrap();
        let captured=bridge.down(&mut view,&mut host,10,context,12.0,12.0).unwrap();
        let releases=host.releases;
        assert!(!bridge.focus_lost(&mut view,&mut host,&mut text_focus,10,stale_focus));
        assert_eq!(host.releases,releases);
        assert!(bridge.binding.accepts(10,captured));
        assert!(bridge.focus_lost(&mut view,&mut host,&mut text_focus,10,focus));
        assert!(!text_focus.accepts(10,focus));
        assert!(bridge.current.is_none());
        let final_ticket=bridge.down(&mut view,&mut host,10,context,12.0,12.0).unwrap();
        assert!(view.close());
        assert!(bridge.cancel(&mut view,&mut host,10,final_ticket));
        assert!(bridge.current.is_none());
    }
    #[test]
    fn capture_to_framework_callback_and_focus_cancel_are_one_path() {
        let path=std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes=std::fs::read(path).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut view=NativeTextView::new(10,context,&bytes,"captured callback text",320,200);
        let mut phases=Vec::new();
        let mut bridge=PointerHostBridge::with_sink(10,4,|event:NativePointerEvent| {
            assert!(matches_ui_view_owner(10,context,10,event.context));
            phases.push(event.phase);
            NativeEventReply::default()
        });
        let mut host=Host::default();
        let mut focus=UiInputBinding::new(10,5);
        let owner=focus.bind(10,context).unwrap();
        let pointer=bridge.down(&mut view,&mut host,10,context,12.0,12.0).unwrap();
        assert!(bridge.move_pointer(&mut view,10,pointer,24.0,12.0));
        host.deny_release=true;
        assert!(!bridge.focus_lost(&mut view,&mut host,&mut focus,10,owner));
        assert!(!focus.accepts(10,owner));
        assert!(!bridge.cancel(&mut view,&mut host,10,pointer)); // Retry must not duplicate Framework cancellation.
        assert!(!bridge.move_pointer(&mut view,10,pointer,36.0,12.0));
        host.deny_release=false;
        assert!(bridge.retry_release(&mut host,10));
        assert!(!bridge.cancel(&mut view,&mut host,10,pointer));
        let next=bridge.down(&mut view,&mut host,10,context,12.0,12.0).unwrap();
        assert!(bridge.up(&mut view,&mut host,10,next));
        drop(bridge);
        assert_eq!(phases,vec![PointerAction::Down,PointerAction::Move,PointerAction::Cancel,
            PointerAction::Down,PointerAction::Up]);
    }

    #[test]
    fn framework_focus_notifications_survive_capture_release_failure_once() {
        let path=std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes=std::fs::read(path).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut view=NativeTextView::new(10,context,&bytes,"focused callback text",320,200);
        let mut input=UiInputBinding::new(10,5);
        let owner=input.bind(10,context).unwrap();
        let mut notifications=Vec::new();
        let mut sink=|event:crate::native_events::NativeFocusEvent| {
            assert!(matches_ui_view_owner(10,context,10,event.owner.context));
            assert_eq!(event.owner.key.generation,owner.key.generation);
            notifications.push(event.focused);
        };
        assert!(!view.focus_from_host_with_events(&input,11,owner,&mut sink));
        assert!(view.focus_from_host_with_events(&input,10,owner,&mut sink));
        assert!(view.focus_from_host_with_events(&input,10,owner,&mut sink));
        let mut bridge=PointerHostBridge::new(10,4);
        let mut host=Host::default();
        let pointer=bridge.down(&mut view,&mut host,10,context,12.0,12.0).unwrap();
        host.deny_release=true;
        assert!(!bridge.focus_lost_with_events(&mut view,&mut host,&mut input,10,owner,&mut sink));
        assert!(!bridge.focus_lost_with_events(&mut view,&mut host,&mut input,10,owner,&mut sink));
        assert!(!bridge.move_pointer(&mut view,10,pointer,24.0,12.0));
        host.deny_release=false;
        assert!(bridge.retry_release(&mut host,10));
        assert_eq!(notifications,vec![true,false]);
    }

    #[test]
    fn host_retirement_blocks_reopen_and_retries_release_without_touching_other_app() {
        let path=std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes=std::fs::read(path).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut a=NativeTextView::new(10,context,&bytes,"app A",320,200);
        let mut b=NativeTextView::new(11,context,&bytes,"app B",320,200);
        let mut bridge=PointerHostBridge::new(10,4);
        let mut host=Host::default();
        let ticket=bridge.down(&mut a,&mut host,10,context,12.0,12.0).unwrap();
        #[cfg(feature="cpu-pending")]
        let b_before=b.render_cpu(1024*1024).unwrap();
        let releases=host.releases;
        assert!(!bridge.retire_from_host(&mut b,&mut host,10,context));
        assert!(!bridge.retire_from_host(&mut a,&mut host,11,context));
        assert_eq!(host.releases,releases);
        assert!(!bridge.retiring);
        host.deny_release=true;
        assert!(!bridge.retire_from_host(&mut a,&mut host,10,context));
        assert_eq!(a.component_version(),None);
        assert!(!bridge.move_pointer(&mut a,10,ticket,24.0,12.0));
        let mut reopened=NativeTextView::new(10,context,&bytes,"new view",320,200);
        let captures=host.captures;
        assert!(bridge.down(&mut reopened,&mut host,10,context,12.0,12.0).is_none());
        assert_eq!(host.captures,captures);
        host.deny_release=false;
        assert!(bridge.retry_release(&mut host,10));
        assert!(bridge.current.is_none());
        assert!(bridge.down(&mut reopened,&mut host,10,context,12.0,12.0).is_none());
        assert!(bridge.retire_from_host(&mut a,&mut host,10,context));
        assert!(b.component_version().is_some());
        #[cfg(feature="cpu-pending")]
        assert!(b.render_cpu(1024*1024).unwrap()==b_before,"other application frame changed");
    }

    #[test]
    fn rejected_view_context_never_requests_host_capture() {
        let path=std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes=std::fs::read(path).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut view=NativeTextView::new(10,context,&bytes,"capture admission",320,200);
        let mut bridge=PointerHostBridge::new(10,4);
        let mut host=Host::default();
        for stale in [UiInputContext {target:NodeId(8),..context},
            UiInputContext {target_generation:0,..context},UiInputContext {document:9,..context},
            UiInputContext {revision:4,..context}] {
            assert!(bridge.down(&mut view,&mut host,10,stale,12.0,12.0).is_none());
        }
        assert!(bridge.down(&mut view,&mut host,11,context,12.0,12.0).is_none());
        assert_eq!(host.captures,0,"rejected view identity requested a host side effect");
        assert_eq!(host.releases,0);
        assert!(bridge.current.is_none());
    }



    #[test]
    fn captured_drag_outside_content_keeps_framework_target() {
        use crate::native_events::ViewPart;
        let path=std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes=std::fs::read(path).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut view=NativeTextView::new(10,context,&bytes,"captured text",320,200);
        let mut events=Vec::new();
        let mut bridge=PointerHostBridge::with_sink(10,4,|event:NativePointerEvent| {
            if event.phase!=PointerAction::Leave { events.push((event.phase,event.part,event.position)); }
            NativeEventReply::default()
        });
        let mut host=Host::default();
        let ticket=bridge.down(&mut view,&mut host,10,context,12.0,12.0).unwrap();
        assert!(bridge.move_pointer(&mut view,10,ticket,400.0,12.0));
        assert!(!bridge.up_at(&mut view,&mut host,10,ticket,f32::NAN,12.0));
        assert!(bridge.up_at(&mut view,&mut host,10,ticket,500.0,14.0));
        drop(bridge);
        assert_eq!(events,vec![(PointerAction::Down,ViewPart::Text,(12.0,12.0)),
            (PointerAction::Move,ViewPart::Text,(400.0,12.0)),
            (PointerAction::Up,ViewPart::Text,(500.0,14.0))]);
    }

    #[test]
    fn down_without_framework_target_cannot_start_a_later_component_drag() {
        let path=std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes=std::fs::read(path).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut view=NativeTextView::new(10,context,&bytes,"target text",320,200);
        let mut events=Vec::new();
        let mut bridge=PointerHostBridge::with_sink(10,4,|event:NativePointerEvent| {
            events.push(event.phase); NativeEventReply::default()
        });
        let mut host=Host::default();
        assert!(bridge.down(&mut view,&mut host,10,context,400.0,250.0).is_none());
        assert!(bridge.current.is_none());
        assert_eq!(host.captures,host.releases);
        host.deny_release=true;
        assert!(bridge.down(&mut view,&mut host,10,context,400.0,250.0).is_none());
        let pending=bridge.current.expect("failed release remains retryable");
        assert!(!bridge.move_pointer(&mut view,10,pending,12.0,12.0));
        assert!(!bridge.retry_release(&mut host,11));
        host.deny_release=false;
        assert!(bridge.retry_release(&mut host,10));
        assert!(bridge.current.is_none());
        let valid=bridge.down(&mut view,&mut host,10,context,12.0,12.0).unwrap();
        assert!(bridge.up(&mut view,&mut host,10,valid));
        drop(bridge);
        assert_eq!(events,vec![PointerAction::Down,PointerAction::Up]);
    }

}
