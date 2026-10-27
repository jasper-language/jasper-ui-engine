//! Crate-private Framework event projection; upstream objects never reach consumers.
//! Framework 事件投影候选；回调仅接收自有类型，不暴露后端对象。
use crate::native_view::PointerAction;
use jasper_ui_engine::input_identity::UiInputContext;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewPart { Text, Preedit, Container }
#[derive(Clone, Copy)]
pub struct NativePointerEvent {
    pub context: UiInputContext,
    pub phase: PointerAction,
    pub part: ViewPart,
    /// View-local logical coordinates, not OS screen coordinates.
    pub position: (f32,f32),
    pub primary_pressed: bool,
    pub modifiers: jasper_ui_engine::input::Modifiers,
}
/// Native view focus notification, not an OS focus request or input-service ACK.
/// 原生视图焦点通知，不是窗口焦点请求或输入服务确认。
#[derive(Clone, Copy)]
pub struct NativeFocusEvent {
    pub owner: jasper_ui_engine::input_identity::UiInputTicket,
    pub focused: bool,
}
/// Component activation emitted by the upstream event driver, not a business command.
/// 后端事件驱动产生的控件激活，不执行任何业务命令。
#[derive(Clone, Copy)]
pub struct NativeActivationEvent { pub context: UiInputContext }
#[derive(Default)]
pub struct NativeEventReply {
    pub prevent_default: bool,
    pub stop_propagation: bool,
}

pub(crate) struct FrameworkEventHandler<'a, 'b, F> {
    pub context: UiInputContext,
    pub suppress_unpaired_up: bool,
    pub text: blitz_dom::NodeId,
    pub preedit: blitz_dom::NodeId,
    pub content: blitz_dom::NodeId,
    pub captured_part: Option<ViewPart>,
    pub sink: &'a mut F,
    pub activation_sink: Option<&'b mut dyn FnMut(NativeActivationEvent) -> NativeEventReply>,
}
impl<F: FnMut(NativePointerEvent) -> NativeEventReply> blitz_dom::EventHandler for FrameworkEventHandler<'_, '_, F> {
    fn handle_event(&mut self, chain: &[blitz_dom::NodeId], event: &mut blitz_traits::events::DomEvent,
        doc: &mut dyn blitz_dom::Document, state: &mut blitz_traits::events::EventState) {
        use blitz_traits::events::DomEventData;
        // Native press identity owns gesture lifetime; no second click recognizer.
        // Clear default activation before any unmapped-target early return.
        if self.suppress_unpaired_up && matches!(&event.data,DomEventData::PointerUp(_)) {
            state.prevent_default();
        }
        if matches!(&event.data,DomEventData::Click(_)) {
            if chain.contains(&self.text) {
                let enabled={ let document=doc.inner();
                    document.get_node(self.text).and_then(|node| node.element_data())
                        .is_some_and(|element| element.name.local==blitz_dom::LocalName::from("button")
                            && element.attr(blitz_dom::LocalName::from("disabled")).is_none()) };
                if enabled {
                    if let Some(sink)=self.activation_sink.as_mut() {
                        let reply=sink(NativeActivationEvent {context:self.context});
                        if reply.prevent_default { state.prevent_default(); }
                        if reply.stop_propagation { state.stop_propagation(); }
                    }
                }
            }
            return;
        }
        let (phase,pointer) = match &event.data {
            DomEventData::PointerDown(pointer) => (PointerAction::Down,pointer),
            DomEventData::PointerMove(pointer) => (PointerAction::Move,pointer),
            DomEventData::PointerUp(pointer) => (PointerAction::Up,pointer),
            DomEventData::PointerCancel(pointer) => (PointerAction::Cancel,pointer),
            DomEventData::PointerLeave(pointer) => (PointerAction::Leave,pointer),
            _ => return,
        };
        // Captured component delivery is Jasper semantics; Blitz still owns default actions.
        if self.captured_part.is_some() && phase==PointerAction::Leave { return; }
        let part = if let Some(part)=self.captured_part { part }
            else if chain.contains(&self.text) { ViewPart::Text }
            else if chain.contains(&self.preedit) { ViewPart::Preedit }
            else if chain.contains(&self.content) { ViewPart::Container }
            else { return; };
        let reply = (self.sink)(NativePointerEvent { context: self.context, phase, part,
            position:(pointer.client_x(),pointer.client_y()),
            primary_pressed:pointer.buttons.contains(blitz_traits::events::MouseEventButtons::Primary),
            modifiers:jasper_ui_engine::input::Modifiers {
                shift:pointer.mods.contains(keyboard_types::Modifiers::SHIFT),
                control:pointer.mods.contains(keyboard_types::Modifiers::CONTROL),
                alt:pointer.mods.contains(keyboard_types::Modifiers::ALT),
                meta:pointer.mods.contains(keyboard_types::Modifiers::META)} });
        if reply.prevent_default { state.prevent_default(); }
        if reply.stop_propagation { state.stop_propagation(); }
    }
}
