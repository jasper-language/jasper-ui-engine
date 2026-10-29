//! Pending Jasper UI facade: native text presentation, no editor or IME engine.
use blitz_dom::{BaseDocument, DocumentConfig, QualName, Namespace, LocalName};
use blitz_traits::shell::Viewport;
use crate::view_slot::ViewSlot;
pub(crate) use crate::component_contract::{ComponentUpdate, ContentFlow, TextAppearance};
use crate::component_contract::supports_component_update;
use jasper_ui_engine::composition_identity::{valid_composition_range, UiCompositionSlot, UiCompositionToken};
use jasper_ui_engine::{input_identity::{next_ui_update_version, accepts_ui_display_snapshot, matches_ui_view_owner, UiInputBinding, UiInputContext, UiInputTicket}};

/// Crate-local candidate facade; no upstream types escape to its caller.
/// Text is supplied by the consumer, not owned as an authoritative edit document.
// BEGIN VERIFIED NATIVE VIEW TYPE
pub(crate) struct NativeTextView {
    document: ViewSlot<BaseDocument>,
    preedit: UiCompositionSlot,
    component_version: u64,
    session: u64,
    context: UiInputContext,
    content_node: blitz_dom::NodeId,
    text_node: blitz_dom::NodeId,
    text_element: blitz_dom::NodeId,
    preedit_node: blitz_dom::NodeId,
    preedit_element: blitz_dom::NodeId,
}
// END VERIFIED NATIVE VIEW TYPE

// BEGIN VERIFIED NATIVE HOST CONTEXT KERNEL
impl NativeTextView {
    /// Read-only owner/context admission for host lifecycle coordination.
    /// 宿主生命周期协调的只读归属检查，不把后端身份当作认证。
    pub(crate) fn host_context_matches(&self, caller: u64, expected: UiInputContext) -> bool {
        matches_ui_view_owner(self.session,self.context,caller,expected)
    }

}
// END VERIFIED NATIVE HOST CONTEXT KERNEL

impl NativeTextView {
    pub(crate) fn new(session: u64, context: UiInputContext, font_bytes: &[u8], text: &str, width: u32, height: u32) -> Self {
        Self::new_with_kind(session,context,font_bytes,text,width,height,NativeElementKind::Text)
    }

    /// Typed native button declaration; application supplies a caption, never markup.
    /// 类型化原生按钮声明；应用仅提供标题，不提供 HTML 或后端类型。
    pub(crate) fn new_button(session: u64, context: UiInputContext, font_bytes: &[u8], caption: &str, width: u32, height: u32) -> Self {
        Self::new_with_kind(session,context,font_bytes,caption,width,height,NativeElementKind::Button)
    }

    /// Host-ordered fallback stack (e.g. Latin file then CJK file); None for an empty,
    /// oversized (> MAX_INJECTED_FONTS) stack or a file that registers no font family.
    /// 宿主有序回退栈；空栈、超限或任一文件无字体族时返回 None，不静默跳过。
    pub(crate) fn new_with_fonts(session: u64, context: UiInputContext, fonts: &[&[u8]], text: &str, width: u32, height: u32) -> Option<Self> {
        let font_ctx=crate::native_fonts::injected_font_stack(fonts)?;
        Some(Self::new_with_context(session,context,font_ctx,text,width,height,NativeElementKind::Text))
    }

    fn new_with_kind(session: u64, context: UiInputContext, font_bytes: &[u8], text: &str,
        width: u32, height: u32, kind: NativeElementKind) -> Self {
        let font_ctx=crate::native_fonts::injected_font_context(font_bytes);
        Self::new_with_context(session,context,font_ctx,text,width,height,kind)
    }

    fn new_with_context(session: u64, context: UiInputContext, font_ctx: blitz_dom::FontContext, text: &str,
        width: u32, height: u32, kind: NativeElementKind) -> Self {
        let mut document = BaseDocument::new(DocumentConfig {
            font_ctx: Some(font_ctx),
            viewport: Some(Viewport { window_size: (width, height), ..Viewport::default() }),
            ..DocumentConfig::default()
        });
        let root = document.root_node().id;
        let content_node;
        let text_node;
        let text_element;
        let preedit_node;
        let preedit_element;
        {
            let mut tree = document.mutate();
            let name = |local: &str| QualName::new(None,
                Namespace::from("http://www.w3.org/1999/xhtml"), LocalName::from(local));
            let container = tree.create_element(name("html"), Vec::new());
            let body = tree.create_element(name("body"), Vec::new());
            content_node = body;
            text_element = tree.create_element(name(match kind {NativeElementKind::Text=>"div",NativeElementKind::Button=>"button"}), Vec::new());
            text_node = tree.create_text_node(text);
            tree.append_children(text_element, &[text_node]);
            preedit_element = tree.create_element(name("div"), Vec::new());
            preedit_node = tree.create_text_node("");
            tree.append_children(preedit_element, &[preedit_node]);
            tree.append_children(body, &[text_element, preedit_element]);
            tree.append_children(container, &[body]);
            tree.append_children(root, &[container]);
        }
        document.resolve(0.0);
        Self { document: ViewSlot::new(document), preedit: UiCompositionSlot::new(), component_version: 0, session, context, content_node, text_node, text_element, preedit_node, preedit_element }
    }

    /// Replaces a host-supplied display snapshot with compare-before-update semantics.
    /// Revision values are external opaque identities, not an edit algorithm or clock.
    /// Clears transient preedit when adopting the new display revision.
    pub(crate) fn apply_display_snapshot_from_host(
        &mut self, caller: u64, expected: UiInputContext, next: UiInputContext, text: &str,
    ) -> bool {
        if !accepts_ui_display_snapshot(self.session, self.context, caller, expected, next) { return false; }
        let Some(document) = self.document.as_mut() else { return false; };
        {
            let mut tree = document.mutate();
            tree.set_node_text(self.text_node, text);
            tree.set_node_text(self.preedit_node, "");
        }
        document.clear_text_selection();
        document.resolve(0.0);
        if let Some(token) = self.preedit.current_token() { let _ = self.preedit.cancel(caller, token); }
        self.context = next;
        true
    }

    /// Starts a UI presentation correlation, not an IME engine session.
    pub(crate) fn begin_preedit_from_host(
        &mut self, binding: &UiInputBinding, caller: u64, ticket: UiInputTicket,
    ) -> Option<UiCompositionToken> {
        if !self.document.is_open()
            || !matches_ui_view_owner(self.session, self.context, caller, ticket.context)
            || !binding.accepts(caller, ticket) { return None; }
        if let Some(token) = self.preedit.current_token() {
            if self.preedit.accepts(binding, caller, token) { return Some(token); }
            if !self.preedit.cancel(caller, token) { return None; }
            if !self.set_preedit_display(None) { return None; }
        }
        self.preedit.begin(binding, caller, ticket)
    }

    pub(crate) fn cancel_preedit_from_host(&mut self, caller: u64, token: UiCompositionToken) -> bool {
        if !self.document.is_open()
            || !matches_ui_view_owner(self.session, self.context, caller, token.owner.context)
            || !self.preedit.cancel(caller, token) { return false; }
        self.set_preedit_display(None)
    }

    /// Matches this view instance before using the existing admission lease.
    /// Caller identity and context originate in the trusted host, never client claims.
    pub(crate) fn set_preedit_from_host(
        &mut self, binding: &UiInputBinding, caller: u64,
        token: UiCompositionToken, text: Option<&str>,
    ) -> bool {
        if !self.preedit.accepts(binding, caller, token) { return false; }
        let ticket = token.owner;
        if !matches_ui_view_owner(self.session, self.context, caller, ticket.context) { return false; }
        let Some(input) = binding.admit(caller, ticket, &text) else { return false; };
        self.set_preedit_display(*input.payload())
    }

    /// Projects an admitted external UTF-8 byte selection into upstream text layout.
    /// The backend owns shaping/highlight geometry; this keeps no editor selection model.
    pub(crate) fn set_selection_from_host(
        &mut self, binding: &UiInputBinding, caller: u64,
        ticket: UiInputTicket, selection: Option<(usize, usize)>,
    ) -> bool {
        if !matches_ui_view_owner(self.session, self.context, caller, ticket.context)
            || binding.admit(caller, ticket, &selection).is_none() { return false; }
        let Some(document) = self.document.as_mut() else { return false; };
        if let Some((anchor, focus)) = selection {
            let Some(node) = document.get_node(self.text_node) else { return false; };
            let Some(root) = node.inline_root_ancestor() else { return false; };
            let Some(layout) = root.element_data().and_then(|data| data.inline_layout_data.as_ref()) else { return false; };
            if !valid_composition_range(&layout.text, anchor.min(focus), anchor.max(focus)) { return false; }
            let root_id = root.id;
            document.set_text_selection(root_id, anchor, root_id, focus);
        } else {
            document.clear_text_selection();
        }
        true
    }

    /// 消费宿主已授权的鼠标事件；命中、选择和默认交互交给 Blitz。
    /// Consume host-authorized mouse input; Blitz owns hit testing and default interaction.
    /// Window capture is a host obligation; this method does not claim to implement it.
    pub(crate) fn pointer_from_host(
        &mut self, caller: u64, expected: UiInputContext,
        phase: PointerAction, x: f32, y: f32, primary_pressed: bool,
    ) -> bool {
        self.pointer_with_events_from_host(caller,expected,phase,x,y,primary_pressed,
            &mut |_| crate::native_events::NativeEventReply::default())
    }

    /// 同步类型化回调；调用方不能获取 DOM、Document 或后端节点身份。
    /// Synchronous typed callback; no DOM, Document or backend identity escapes.
    pub(crate) fn pointer_with_events_from_host(
        &mut self, caller: u64, expected: UiInputContext,
        phase: PointerAction, x: f32, y: f32, primary_pressed: bool,
        sink: &mut impl FnMut(crate::native_events::NativePointerEvent) -> crate::native_events::NativeEventReply,
    ) -> bool {
        self.pointer_with_capture_from_host(caller,expected,phase,x,y,primary_pressed,None,sink)
    }

    /// Host bridge supplies only the original Jasper part, never a backend node identity.
    /// 捕获桥仅传入原 Jasper 部件；Blitz 命中与默认动作仍在后端内部处理。
    pub(crate) fn pointer_with_capture_from_host(
        &mut self, caller: u64, expected: UiInputContext, phase: PointerAction,
        x: f32, y: f32, primary_pressed: bool, captured_part: Option<crate::native_events::ViewPart>,
        sink: &mut impl FnMut(crate::native_events::NativePointerEvent) -> crate::native_events::NativeEventReply,
    ) -> bool {
        self.dispatch_pointer_from_host(caller,expected,phase,x,y,primary_pressed,captured_part,sink,None)
    }

    /// Type-safe native button activation; click detection remains in Blitz.
    /// 原生按钮激活回调；点击判定沿用 Blitz，不在 Jasper 中另写一套。
    pub(crate) fn button_pointer_from_host(&mut self, caller: u64, expected: UiInputContext,
        phase: PointerAction, x: f32, y: f32, primary_pressed: bool,
        sink: &mut impl FnMut(crate::native_events::NativeActivationEvent) -> crate::native_events::NativeEventReply) -> bool {
        if !self.host_context_matches(caller,expected) { return false; }
        let Some(document)=self.document.as_mut() else { return false; };
        {
            let tree=document.mutate();
            if !tree.element_name(self.text_element).is_some_and(|name| name.local==LocalName::from("button")) { return false; }
        }
        self.dispatch_pointer_from_host(caller,expected,phase,x,y,primary_pressed,None,
            &mut |_| crate::native_events::NativeEventReply::default(),Some(sink))
    }

    pub(crate) fn dispatch_pointer_from_host(&mut self, caller: u64, expected: UiInputContext, phase: PointerAction,
        x: f32, y: f32, primary_pressed: bool, captured_part: Option<crate::native_events::ViewPart>,
        sink: &mut impl FnMut(crate::native_events::NativePointerEvent) -> crate::native_events::NativeEventReply,
        activation_sink: Option<&mut dyn FnMut(crate::native_events::NativeActivationEvent) -> crate::native_events::NativeEventReply>,
    ) -> bool {
        self.dispatch_pointer_modifiers_from_host(caller,expected,phase,x,y,primary_pressed,Default::default(),captured_part,sink,activation_sink)
    }
    pub(crate) fn dispatch_pointer_modifiers_from_host(&mut self,caller:u64,expected:UiInputContext,phase:PointerAction,
        x:f32,y:f32,primary_pressed:bool,modifiers:jasper_ui_engine::input::Modifiers,captured_part:Option<crate::native_events::ViewPart>,
        sink:&mut impl FnMut(crate::native_events::NativePointerEvent)->crate::native_events::NativeEventReply,
        activation_sink:Option<&mut dyn FnMut(crate::native_events::NativeActivationEvent)->crate::native_events::NativeEventReply>)->bool {
        use blitz_traits::events::{BlitzPointerEvent, BlitzPointerId, MouseEventButton,
            MouseEventButtons, PointerCoords, PointerDetails, Point, UiEvent};
        if !matches_ui_view_owner(self.session,self.context,caller,expected)
            || !x.is_finite() || !y.is_finite() { return false; }
        let Some(document) = self.document.as_mut() else { return false; };
        let scroll = document.viewport_scroll();
        let page_x = x as f64 + scroll.x;
        let page_y = y as f64 + scroll.y;
        if !page_x.is_finite() || !page_y.is_finite()
            || page_x.abs() > f32::MAX as f64 || page_y.abs() > f32::MAX as f64 { return false; }
        let mut native_modifiers=keyboard_types::Modifiers::empty();
        if modifiers.shift {native_modifiers.insert(keyboard_types::Modifiers::SHIFT);}
        if modifiers.control {native_modifiers.insert(keyboard_types::Modifiers::CONTROL);}
        if modifiers.alt {native_modifiers.insert(keyboard_types::Modifiers::ALT);}
        if modifiers.meta {native_modifiers.insert(keyboard_types::Modifiers::META);}
        let pointer = BlitzPointerEvent {
            id: BlitzPointerId::Mouse, is_primary: true,
            coords: PointerCoords { page_x: page_x as f32, page_y: page_y as f32,
                client_x: x, client_y: y, screen_x: x, screen_y: y },
            button: MouseEventButton::Main,
            buttons: if primary_pressed { MouseEventButtons::Primary } else { MouseEventButtons::None },
            mods: native_modifiers, details: PointerDetails::default(),
            element: Point::default(), active_pointers: Default::default(),
        };
        let event = match phase {
            PointerAction::Move => UiEvent::PointerMove(pointer),
            PointerAction::Down => UiEvent::PointerDown(pointer),
            PointerAction::Up => UiEvent::PointerUp(pointer),
            PointerAction::Cancel => UiEvent::PointerCancel(pointer),
            PointerAction::Leave => UiEvent::PointerLeave(pointer),
        };
        let suppress_unpaired_up=phase==PointerAction::Up && document.get_mousedown_node_id().is_none();
        let handler = crate::native_events::FrameworkEventHandler {
            context: self.context, suppress_unpaired_up, text: self.text_element, preedit: self.preedit_element,
            content: self.content_node, captured_part, sink, activation_sink,
        };
        blitz_dom::EventDriver::new(document,handler).handle_ui_event(event);
        if matches!(phase,PointerAction::Up|PointerAction::Cancel) {document.set_mousedown_node_id(None);}
        document.resolve(0.0);
        true
    }

    /// 把已授权外部焦点投影至原生树；不向系统请求窗口焦点。
    /// Project admitted focus into the native tree, without requesting OS window focus.
    /// Cancel only this component lineage; retired payloads need only host release.
    /// Revision successors receive backend cleanup without stale Framework callbacks.
    pub(crate) fn cancel_capture_from_host(&mut self,caller:u64,origin:UiInputContext,deliver:bool,
        x:f32,y:f32,modifiers:jasper_ui_engine::input::Modifiers,captured_part:Option<crate::native_events::ViewPart>,
        sink:&mut impl FnMut(crate::native_events::NativePointerEvent)->crate::native_events::NativeEventReply,
        activation_sink:&mut dyn FnMut(crate::native_events::NativeActivationEvent)->crate::native_events::NativeEventReply)->bool {
        use crate::capture_policy::{capture_cancel_route,CaptureCancelRoute};
        match capture_cancel_route(self.session,self.context,caller,origin,deliver,self.document.is_open()) {
            CaptureCancelRoute::Reject=>false,
            CaptureCancelRoute::HostOnly=>true,
            CaptureCancelRoute::Current=>self.dispatch_pointer_modifiers_from_host(caller,origin,PointerAction::Cancel,x,y,false,
                modifiers,captured_part,sink,Some(activation_sink)),
            CaptureCancelRoute::Successor=>self.dispatch_pointer_modifiers_from_host(caller,self.context,PointerAction::Cancel,x,y,false,
                modifiers,captured_part,&mut |_|crate::native_events::NativeEventReply::default(),None),
        }
    }

    pub(crate) fn focus_from_host(&mut self, binding: &UiInputBinding, caller: u64, ticket: UiInputTicket) -> bool {
        self.focus_from_host_with_events(binding,caller,ticket,&mut |_| {})
    }

    pub(crate) fn focus_from_host_with_events(&mut self, binding: &UiInputBinding, caller: u64,
        ticket: UiInputTicket, sink: &mut impl FnMut(crate::native_events::NativeFocusEvent)) -> bool {
        if !matches_ui_view_owner(self.session,self.context,caller,ticket.context)
            || !binding.accepts(caller,ticket) { return false; }
        let Some(document)=self.document.as_mut() else { return false; };
        if document.get_node(self.text_element).and_then(|n| n.element_data())
            .and_then(|e| e.attr(LocalName::from("disabled"))).is_some() { return false; }
        let was_focused=document.get_focussed_node_id()==Some(self.text_element);
        document.set_focus_to(self.text_element);
        document.resolve(0.0);
        let focused=document.get_focussed_node_id()==Some(self.text_element);
        if focused && !was_focused { sink(crate::native_events::NativeFocusEvent {owner:ticket,focused:true}); }
        focused
    }

    /// 命令键交给 Blitz，输入文字仍由外部服务提交为显示快照。
    /// Route command keys through Blitz; committed text remains an external-service snapshot.
    pub(crate) fn key_from_host(&mut self, binding: &UiInputBinding, caller: u64, ticket: UiInputTicket,
        input: &crate::native_keys::HostKeyInput<'_>,
        sink: &mut impl FnMut(crate::native_keys::NativeKeyEvent) -> crate::native_events::NativeEventReply) -> bool {
        use blitz_traits::events::UiEvent;
        if !matches_ui_view_owner(self.session,self.context,caller,ticket.context)
            || !binding.accepts(caller,ticket) { return false; }
        let Some(key)=crate::native_keys::native_command_key(input) else {return false;};
        let Some(document)=self.document.as_mut() else { return false; };
        if document.get_focussed_node_id()!=Some(self.text_element)
            || document.get_node(self.text_element).and_then(|n| n.element_data())
                .and_then(|e| e.attr(LocalName::from("disabled"))).is_some() { return false; }
        let event=if input.pressed {UiEvent::KeyDown(key)} else {UiEvent::KeyUp(key)};
        let handler=crate::native_keys::FrameworkKeyHandler {context:self.context,text:self.text_element,sink};
        blitz_dom::EventDriver::new(document,handler).handle_ui_event(event);
        document.resolve(0.0);
        true
    }

    /// Provisional Framework Enter activation policy; not text input or an IME commit.
    /// Framework Enter 激活候选：只接收已聚焦按钮的非重复、非组合、无修饰按下。
    pub(crate) fn button_enter_from_host(&mut self, binding: &UiInputBinding, caller: u64,
        ticket: UiInputTicket, input: &crate::native_keys::HostKeyInput<'_>,
        sink: &mut impl FnMut(crate::native_events::NativeActivationEvent)) -> bool {
        if !self.host_context_matches(caller,ticket.context) || !binding.accepts(caller,ticket)
            || input.key!="Enter" || !input.pressed || input.repeat || input.composing
            || input.modifiers!=jasper_ui_engine::input::Modifiers::default() { return false; }
        let Some(document)=self.document.as_mut() else { return false; };
        {
            let tree=document.mutate();
            if !tree.element_name(self.text_element).is_some_and(|name| name.local==LocalName::from("button")) { return false; }
        }
        if !self.key_from_host(binding,caller,ticket,input,&mut |_| crate::native_events::NativeEventReply {
            prevent_default:true,stop_propagation:true,
        }) { return false; }
        sink(crate::native_events::NativeActivationEvent {context:self.context});
        true
    }

    /// Consume a trusted host's own semantic action, without synthesizing keyboard or pointer input.
    /// 消费可信宿主语义动作，不伪造键盘/鼠标，不实现平台无障碍服务。
    pub(crate) fn activate_from_host(&self, caller: u64, expected: UiInputContext, expected_version: u64,
        sink: &mut impl FnMut(crate::native_events::NativeActivationEvent)) -> bool {
        let Some(semantics)=self.semantics_from_host(caller,expected) else { return false; };
        if !crate::native_semantics::accepts_activation(semantics.component_version,expected_version,semantics.can_activate) { return false; }
        sink(crate::native_events::NativeActivationEvent {context:semantics.context});
        true
    }

    /// Reads a display-text hit from an admitted view-local logical point.
    /// Returns only the external display's UTF-8 byte offset; no backend node escapes.
    /// Host routing/physical-to-logical conversion remains the host's responsibility.
    pub(crate) fn hit_display_text_from_host(
        &self, binding: &UiInputBinding, caller: u64, ticket: UiInputTicket, x: f32, y: f32,
    ) -> Option<usize> {
        if !x.is_finite() || !y.is_finite()
            || !matches_ui_view_owner(self.session, self.context, caller, ticket.context)
            || binding.admit(caller, ticket, &(x, y)).is_none() { return None; }
        let document = self.document.as_ref()?;
        let root = document.get_node(self.text_node)?.inline_root_ancestor()?;
        let layout = root.element_data()?.inline_layout_data.as_ref()?;
        let viewport = document.viewport();
        let scale = viewport.scale_f64();
        if x < 0.0 || y < 0.0 || x as f64 > viewport.window_size.0 as f64 / scale
            || y as f64 > viewport.window_size.1 as f64 / scale { return None; }
        // Pinned Blitz find_text_position consumes document coordinates, as used by hover refresh.
        let scroll = document.viewport_scroll();
        let document_x = x as f64 + scroll.x;
        let document_y = y as f64 + scroll.y;
        if !document_x.is_finite() || !document_y.is_finite()
            || document_x.abs() > f32::MAX as f64 || document_y.abs() > f32::MAX as f64 { return None; }
        let (hit_root, offset) = document.find_text_position(document_x as f32, document_y as f32)?;
        if hit_root != root.id || !layout.text.is_char_boundary(offset) { return None; }
        Some(offset)
    }

    /// Reads current selected display text for an authorized host consumer.
    /// Does not access a clipboard or send an edit/service request.
    pub(crate) fn selected_display_text_from_host(
        &self, binding: &UiInputBinding, caller: u64, ticket: UiInputTicket,
    ) -> Option<String> {
        if !matches_ui_view_owner(self.session, self.context, caller, ticket.context)
            || binding.admit(caller, ticket, &()).is_none() { return None; }
        self.document.as_ref()?.get_selected_text()
    }

    /// Consumes a routed logical content-scroll position; host supplies valid extent.
    /// Does not own system window/display scrolling or implement a scroll algorithm.
    pub(crate) fn set_scroll_from_host(
        &mut self, binding: &UiInputBinding, caller: u64, ticket: UiInputTicket, x: f64, y: f64,
    ) -> bool {
        if !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0
            || x > u32::MAX as f64 || y > u32::MAX as f64
            || !matches_ui_view_owner(self.session, self.context, caller, ticket.context)
            || binding.admit(caller, ticket, &(x, y)).is_none() { return false; }
        let Some(document) = self.document.as_mut() else { return false; };
        document.set_viewport_scroll(blitz_dom::Point { x, y });
        true
    }

    pub(crate) fn scroll_position(&self) -> Option<(f64, f64)> {
        let position = self.document.as_ref()?.viewport_scroll();
        Some((position.x, position.y))
    }

    /// Synchronous host focus closure: revoke the exact binding, then clear this view.
    /// Stale closures cannot clear a successor; this is not an external cancel ACK.
    pub(crate) fn close_focus_from_host(
        &mut self, binding: &mut UiInputBinding, caller: u64, ticket: UiInputTicket,
    ) -> bool {
        self.close_focus_from_host_with_events(binding,caller,ticket,&mut |_| {})
    }

    pub(crate) fn close_focus_from_host_with_events(
        &mut self, binding: &mut UiInputBinding, caller: u64, ticket: UiInputTicket,
        sink: &mut impl FnMut(crate::native_events::NativeFocusEvent),
    ) -> bool {
        if !self.document.is_open()
            || !matches_ui_view_owner(self.session, self.context, caller, ticket.context) {
            return false;
        }
        if !binding.close(caller, ticket) { return false; }
        let was_focused=self.document.as_ref().is_some_and(|doc| doc.get_focussed_node_id()==Some(self.text_element));
        if let Some(document)=self.document.as_mut() { document.clear_focus(); }
        if let Some(token) = self.preedit.current_token() { let _ = self.preedit.cancel(caller, token); }
        let cleared=self.set_preedit_display(None);
        if cleared && was_focused { sink(crate::native_events::NativeFocusEvent {owner:ticket,focused:false}); }
        cleared
    }

    /// Presents an already-authorized input-service snapshot separately from text.
    /// None clears only this transient display; it does not send a service cancel ACK.
    /// No candidate window, composition engine or editable document is created here.
    fn set_preedit_display(&mut self, text: Option<&str>) -> bool {
        let Some(document) = self.document.as_mut() else { return false; };
        {
            let mut tree = document.mutate();
            tree.set_node_text(self.preedit_node, text.unwrap_or(""));
        }
        document.resolve(0.0);
        true
    }

    /// Consumes a Framework component update without implementing a Signal runtime.
    /// Host supplies authenticated caller and the update's expected view context.
    pub(crate) fn apply_component_update(
        &mut self, caller: u64, expected: UiInputContext, expected_update: u64, update: ComponentUpdate<'_>,
    ) -> bool {
        self.apply_component_batch(caller,expected,expected_update,&[update])
    }


    /// Synchronously consume a Framework update frame, with one UI version successor.
    /// Validate the entire borrowed batch before native mutation; no Signal runtime is owned here.
    /// Backend/callback panic rollback is not provided by this candidate.
    pub(crate) fn apply_component_batch(&mut self, caller: u64, expected: UiInputContext,
        expected_update: u64, updates: &[ComponentUpdate<'_>]) -> bool {
        if updates.is_empty() || !self.host_context_matches(caller,expected) { return false; }
        let Some(next)=next_ui_update_version(self.component_version,expected_update) else { return false; };
        let Some(document)=self.document.as_ref() else { return false; };
        let Some(element)=document.get_node(self.text_element).and_then(|node| node.element_data()) else { return false; };
        let button=element.name.local==LocalName::from("button");
        for update in updates {
            if !supports_component_update(button,*update) { return false; }
        }
        for update in updates {
            if !self.apply_component_value(*update) { return false; }
        }
        self.component_version=next;
        true
    }

    /// Internal mapping shared by single updates and preflighted frames.
    fn apply_component_value(&mut self, update: ComponentUpdate<'_>) -> bool {
        match update {
            ComponentUpdate::AccessibleLabel(label) => self.set_accessible_label(label),
            ComponentUpdate::ButtonCaption(caption) => self.set_button_caption(caption),
            ComponentUpdate::ButtonDisabled(disabled) => self.set_button_disabled(disabled),
            ComponentUpdate::Appearance(appearance) => self.set_appearance(appearance),
            ComponentUpdate::Flow { direction, gap_px } => self.set_content_flow(direction, gap_px),
        }
    }

    /// Apply a component update and report a native focus transition to the current owner.
    /// 外部焦点租约仍归输入服务；这里只通知实际原生焦点变化。
    pub(crate) fn apply_component_update_with_focus_events(
        &mut self, binding: &UiInputBinding, caller: u64, ticket: UiInputTicket,
        expected_update: u64, update: ComponentUpdate<'_>,
        sink: &mut impl FnMut(crate::native_events::NativeFocusEvent),
    ) -> bool {
        self.apply_component_batch_with_focus_events(binding,caller,ticket,expected_update,&[update],sink)
    }

    /// A frame publishes its final native focus transition once, after all admitted updates.
    /// 整帧只报告最终原生焦点变化；中途禁用后重新启用不会擅自恢复焦点。
    pub(crate) fn apply_component_batch_with_focus_events(
        &mut self, binding: &UiInputBinding, caller: u64, ticket: UiInputTicket,
        expected_update: u64, updates: &[ComponentUpdate<'_>],
        sink: &mut impl FnMut(crate::native_events::NativeFocusEvent),
    ) -> bool {
        if !self.host_context_matches(caller,ticket.context) || !binding.accepts(caller,ticket) { return false; }
        let Some(document)=self.document.as_ref() else { return false; };
        let was_focused=document.get_focussed_node_id()==Some(self.text_element);
        if !self.apply_component_batch(caller,ticket.context,expected_update,updates) { return false; }
        let focused=self.document.as_ref().is_some_and(|document|
            document.get_focussed_node_id()==Some(self.text_element));
        if was_focused!=focused {
            sink(crate::native_events::NativeFocusEvent {owner:ticket,focused});
        }
        true
    }

    /// Updates Framework-owned button caption only; never writes an external text document.
    /// 只更新 Framework 按钮标题，不修改外部文本服务文档。
    fn set_button_caption(&mut self, caption: &str) -> bool {
        let Some(document)=self.document.as_mut() else { return false; };
        {
            let mut tree=document.mutate();
            if !tree.element_name(self.text_element).is_some_and(|name| name.local==LocalName::from("button")) { return false; }
            tree.set_node_text(self.text_node,caption);
        }
        document.resolve(0.0);
        true
    }

    /// Native disabled semantics belong to Blitz; external focus lease closure remains host-owned.
    /// 禁用状态映射原生按钮；外部焦点租约仍由宿主关闭，不在此伪造服务确认。
    fn set_button_disabled(&mut self, disabled: bool) -> bool {
        let Some(document)=self.document.as_mut() else { return false; };
        {
            let mut tree=document.mutate();
            if !tree.element_name(self.text_element).is_some_and(|name| name.local==LocalName::from("button")) { return false; }
            let name=QualName::new(None,Namespace::from(""),LocalName::from("disabled"));
            if disabled { tree.set_attribute(self.text_element,name,""); }
            else { tree.clear_attribute(self.text_element,name); }
        }
        if disabled && document.get_focussed_node_id()==Some(self.text_element) { document.clear_focus(); }
        document.resolve(0.0);
        true
    }

    /// Maps supplied UI accessibility metadata; no OS accessibility bridge is owned here.
    fn set_accessible_label(&mut self, label: Option<&str>) -> bool {
        let Some(document) = self.document.as_mut() else { return false; };
        {
            let mut tree = document.mutate();
            let name = QualName::new(None, Namespace::from(""), LocalName::from("aria-label"));
            match label {
                Some(value) => tree.set_attribute(self.text_element, name, value),
                None => tree.clear_attribute(self.text_element, name),
            }
        }
        document.resolve(0.0);
        true
    }

    pub(crate) fn accessible_label(&self) -> Option<&str> {
        self.document.as_ref()?.get_node(self.text_element)?.element_data()?.attr(LocalName::from("aria-label"))
    }

    /// Export own accessibility metadata without leaking backend objects or claiming OS support.
    /// 输出自有无障碍语义；不外露后端对象，也不声称已接通系统无障碍服务。
    pub(crate) fn semantics_from_host(&self, caller: u64, expected: UiInputContext)
        -> Option<crate::native_semantics::ViewSemantics> {
        use crate::native_semantics::{SemanticRole,ViewSemantics};
        if !self.host_context_matches(caller,expected) { return None; }
        let document=self.document.as_ref()?;
        let element=document.get_node(self.text_element)?.element_data()?;
        let role=if element.name.local==LocalName::from("button") {SemanticRole::Button} else {SemanticRole::Text};
        let name=match element.attr(LocalName::from("aria-label")) {
            Some(name)=>name.to_owned(),
            None=>document.get_node(self.text_node)?.text_content(),
        };
        let disabled=element.attr(LocalName::from("disabled")).is_some();
        Some(ViewSemantics {context:self.context,component_version:self.component_version,role,name,disabled,
            native_focused:document.get_focussed_node_id()==Some(self.text_element),
            can_activate:role==SemanticRole::Button && !disabled})
    }

    /// Maps typed component appearance into private backend properties.
    /// No stylesheet, selector or backend type is accepted from the caller.
    fn set_appearance(&mut self, appearance: TextAppearance) -> bool {
        if appearance.font_size_px == 0 { return false; }
        let Some(document) = self.document.as_mut() else { return false; };
        {
            let mut tree = document.mutate();
            for element in [self.text_element, self.preedit_element] {
            tree.set_style_property(element, "font-size", &format!("{}px", appearance.font_size_px));
            tree.set_style_property(element, "padding", &format!("{}px", appearance.padding_px));
            let [r, g, b] = appearance.foreground;
            tree.set_style_property(element, "color", &format!("#{r:02x}{g:02x}{b:02x}"));
            let [r, g, b] = appearance.background;
            tree.set_style_property(element, "background-color", &format!("#{r:02x}{g:02x}{b:02x}"));
            }
        }
        document.resolve(0.0);
        true
    }

    /// Maps a typed layout declaration; upstream Blitz owns all flex layout work.
    fn set_content_flow(&mut self, flow: ContentFlow, gap_px: u16) -> bool {
        let Some(document) = self.document.as_mut() else { return false; };
        {
            let mut tree = document.mutate();
            tree.set_style_property(self.content_node, "display", "flex");
            tree.set_style_property(self.content_node, "flex-direction", match flow {
                ContentFlow::Row => "row", ContentFlow::Column => "column",
            });
            tree.set_style_property(self.content_node, "gap", &format!("{gap_px}px"));
        }
        document.resolve(0.0);
        true
    }

    /// Consumes host window metrics without owning or changing the system window.
    pub(crate) fn resize(&mut self, width: u32, height: u32) -> bool {
        let Some(document) = self.document.as_mut() else { return false; };
        let mut viewport = document.viewport().clone();
        viewport.window_size = (width, height);
        document.set_viewport(viewport);
        document.resolve(0.0);
        true
    }

    /// Consumes host DPI without requesting any OS display or window change.
    /// Candidate host policy bounds scale to [1/64, 64] before backend mutation.
    /// This internal limit is not a frozen public platform contract.
    pub(crate) fn set_host_scale(&mut self, scale: f32) -> bool {
        if !scale.is_finite() || !(0.015625..=64.0).contains(&scale) { return false; }
        let Some(document) = self.document.as_mut() else { return false; };
        let mut viewport = document.viewport().clone();
        viewport.hidpi_scale = scale;
        document.set_viewport(viewport);
        document.resolve(0.0);
        true
    }

    pub(crate) fn host_scale(&self) -> Option<f32> {
        self.document.as_ref().map(|document| document.viewport().hidpi_scale)
    }

    /// Paints view content via upstream Blitz/AnyRender/Vello CPU only.
    /// Returned bytes belong to this UI frame, not to a host window or compositor.
    /// Byte/dimension guard and whole adapter remain unverified pending candidates.
    #[cfg(feature = "cpu-pending")]
    pub(crate) fn render_cpu(&mut self, max_bytes: usize) -> Option<Vec<u8>> {
        let (width, height) = self.viewport_size()?;
        let bytes=crate::cpu_budget::cpu_frame_bytes(width,height,max_bytes)?;
        let document = self.document.as_mut()?;
        let scale = document.viewport().scale_f64();
        let pixels = anyrender::render_to_buffer::<anyrender_vello_cpu::VelloCpuImageRenderer, _>(
            |scene| blitz_paint::paint_scene(&mut crate::glyph_skew::UprightSyntheticItalic(scene), document, scale, width, height, 0, 0),
            width,
            height,
        );
        // Post-check is a TCB contract check, not an allocator bound for upstream scratch data.
        if pixels.len() != bytes { return None; }
        Some(pixels)
    }

    pub(crate) fn viewport_size(&self) -> Option<(u32, u32)> {
        self.document.as_ref().map(|document| document.viewport().window_size)
    }

}
// BEGIN VERIFIED NATIVE VIEW CLOSE KERNEL
impl NativeTextView {
    /// Closes this view permanently; later calls cannot access its upstream tree.
    /// This does not acknowledge cancellation of external services or host tasks.
    pub(crate) fn close(&mut self) -> bool {
        let _ = self.preedit.retire();
        self.document.close()
    }
}

// END VERIFIED NATIVE VIEW CLOSE KERNEL

#[derive(Clone, Copy)]
enum NativeElementKind { Text, Button }

/// Candidate Rust update value, not a frozen Jasper-language style/namespace API.
// BEGIN VERIFIED NATIVE COMPONENT VERSION KERNEL
impl NativeTextView {
    /// Independent UI update sequence, not the external text document revision.
    pub(crate) fn component_version(&self) -> Option<u64> {
        if self.document.is_open() { Some(self.component_version) } else { None }
    }
}
// END VERIFIED NATIVE COMPONENT VERSION KERNEL



/// Internal host-routed mouse phases; no public namespace or device protocol is frozen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerAction { Move, Down, Up, Cancel, Leave }

#[cfg(test)]
mod tests {
    use super::*;
    use jasper_ui_engine::scene::NodeId;

    #[test]
    #[cfg(feature="cpu-pending")]
    fn native_update_frame_matches_sequential_final_pixels() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required")).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut batched=NativeTextView::new_button(10,context,&bytes,"Old",320,200);
        let mut sequential=NativeTextView::new_button(10,context,&bytes,"Old",320,200);
        let updates=[ComponentUpdate::ButtonCaption("Saved 中文"),
            ComponentUpdate::Appearance(TextAppearance {font_size_px:24,padding_px:12,foreground:[40,80,120],background:[235,240,250]}),
            ComponentUpdate::Flow {direction:ContentFlow::Row,gap_px:8},
            ComponentUpdate::AccessibleLabel(Some("Saved file")),ComponentUpdate::ButtonDisabled(true)];
        let baseline=batched.render_cpu(1024*1024).unwrap();
        let initial_reference=sequential.render_cpu(1024*1024).unwrap();
        assert!(baseline==initial_reference,"independent initial injected-font views must match");
        assert!(batched.apply_component_batch(10,context,0,&updates));
        for (version,update) in updates.iter().enumerate() {
            assert!(sequential.apply_component_update(10,context,version as u64,*update));
        }
        let frame=batched.render_cpu(1024*1024).unwrap();
        assert!(frame!=baseline,"frame must change pixels");
        let reference=sequential.render_cpu(1024*1024).unwrap();
        let differing=frame.iter().zip(&reference).filter(|(a,b)|a!=b).count();
        assert!(frame==reference,"batch/sequential differing bytes={differing}");
        assert_eq!(batched.component_version(),Some(1));assert_eq!(sequential.component_version(),Some(5));
        assert_eq!(batched.semantics_from_host(10,context).unwrap().name,"Saved file");
    }

    #[test]
    fn update_frame_reports_focus_only_after_successful_whole_frame() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required")).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut view=NativeTextView::new_button(10,context,&bytes,"Save",320,200);
        let mut binding=UiInputBinding::new(10,5);let owner=binding.bind(10,context).unwrap();
        assert!(view.focus_from_host(&binding,10,owner));
        let mut events=Vec::new();
        let invalid=[ComponentUpdate::ButtonDisabled(true),ComponentUpdate::Appearance(
            TextAppearance {font_size_px:0,padding_px:0,foreground:[0;3],background:[255;3]})];
        assert!(!view.apply_component_batch_with_focus_events(&binding,10,owner,0,&invalid,&mut |event| events.push(event)));
        assert!(view.semantics_from_host(10,context).unwrap().native_focused);assert!(events.is_empty());
        let updates=[ComponentUpdate::ButtonDisabled(true),ComponentUpdate::ButtonCaption("Saved"),ComponentUpdate::ButtonDisabled(false)];
        assert!(view.apply_component_batch_with_focus_events(&binding,10,owner,0,&updates,&mut |event| events.push(event)));
        assert_eq!(events.len(),1);assert!(!events[0].focused);assert!(binding.accepts(10,events[0].owner));
        let semantics=view.semantics_from_host(10,context).unwrap();
        assert!(!semantics.disabled);assert!(!semantics.native_focused);assert_eq!(semantics.component_version,1);
        assert_eq!(semantics.name,"Saved");
        assert!(!view.apply_component_batch_with_focus_events(&binding,10,owner,0,&updates,&mut |event| events.push(event)));
        assert_eq!(events.len(),1);
    }

    #[test]
    fn component_frame_preflights_all_updates_and_advances_one_version() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required")).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut button=NativeTextView::new_button(10,context,&bytes,"Old",320,200);
        let invalid=[ComponentUpdate::ButtonCaption("Must not appear"),ComponentUpdate::Appearance(
            TextAppearance {font_size_px:0,padding_px:0,foreground:[0;3],background:[255;3]})];
        assert!(!button.apply_component_batch(10,context,0,&invalid));
        assert_eq!(button.component_version(),Some(0));
        assert_eq!(button.semantics_from_host(10,context).unwrap().name,"Old");
        let updates=[ComponentUpdate::ButtonCaption("New"),ComponentUpdate::AccessibleLabel(Some("Save file")),
            ComponentUpdate::ButtonDisabled(true)];
        assert!(!button.apply_component_batch(11,context,0,&updates));
        assert!(button.apply_component_batch(10,context,0,&updates));
        let semantics=button.semantics_from_host(10,context).unwrap();
        assert_eq!(semantics.component_version,1);assert_eq!(semantics.name,"Save file");assert!(semantics.disabled);
        assert!(!button.apply_component_batch(10,context,0,&updates));
        assert!(!button.apply_component_batch(10,context,1,&[]));
        let mut text=NativeTextView::new(10,context,&bytes,"External",320,200);
        assert!(!text.apply_component_batch(10,context,0,&[
            ComponentUpdate::AccessibleLabel(Some("Must not appear")),ComponentUpdate::ButtonCaption("No")]));
        assert_eq!(text.accessible_label(),None);assert_eq!(text.component_version(),Some(0));
        button.component_version=u64::MAX;
        assert!(!button.apply_component_batch(10,context,u64::MAX,&updates));
        assert!(button.close());assert!(!button.apply_component_batch(10,context,u64::MAX,&updates));
    }

    #[test]
    fn actual_dispatch_rejects_sequence_exhaustion() {
        let path = std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes = std::fs::read(path).expect("injected test font must be readable");
        let context = UiInputContext { target: NodeId(9), target_generation: 1, document: 2, revision: 3 };
        let mut view = NativeTextView::new(10, context, &bytes, "序号边界", 320, 200);
        // Reachable production state injected directly to avoid2^64 fixture updates.
        view.component_version = u64::MAX - 1;
        assert!(view.apply_component_update(10, context, u64::MAX - 1,
            ComponentUpdate::AccessibleLabel(Some("last accepted"))));
        assert_eq!(view.component_version(), Some(u64::MAX));
        #[cfg(feature = "cpu-pending")]
        let before = view.render_cpu(1024 * 1024).expect("boundary frame");
        for expected in [u64::MAX - 1, u64::MAX] {
            assert!(!view.apply_component_update(10, context, expected,
                ComponentUpdate::AccessibleLabel(Some("must not replace"))));
            assert_eq!(view.component_version(), Some(u64::MAX));
            assert_eq!(view.accessible_label(), Some("last accepted"));
        }
        #[cfg(feature = "cpu-pending")]
        assert_eq!(view.render_cpu(1024 * 1024).expect("rejected boundary frame"), before);
        assert!(view.close());
        assert_eq!(view.component_version(), None);
    }
    #[test]
    fn two_native_views_isolate_application_and_close() {
        let path = std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes = std::fs::read(path).expect("injected test font must be readable");
        let context = UiInputContext { target: NodeId(9), target_generation: 1, document: 2, revision: 3 };
        let mut a = NativeTextView::new(10, context, &bytes, "alpha", 320, 200);
        let mut b = NativeTextView::new(11, context, &bytes, "beta", 320, 200);
        let mut ba = UiInputBinding::new(10, 1);
        let mut bb = UiInputBinding::new(11, 1);
        let ta = ba.bind(10, context).expect("app A binding");
        let tb = bb.bind(11, context).expect("app B binding");
        assert!(a.apply_component_update(10, context, 0, ComponentUpdate::AccessibleLabel(Some("A"))));
        assert!(!b.apply_component_update(10, context, 0, ComponentUpdate::AccessibleLabel(Some("wrong"))));
        assert_eq!(b.component_version(), Some(0));
        assert!(b.apply_component_update(11, context, 0, ComponentUpdate::AccessibleLabel(Some("B"))));
        assert!(a.set_selection_from_host(&ba, 10, ta, Some((0, 1))));
        assert!(b.set_selection_from_host(&bb, 11, tb, Some((0, 1))));
        assert_eq!(a.selected_display_text_from_host(&ba, 10, ta).as_deref(), Some("a"));
        assert_eq!(b.selected_display_text_from_host(&bb, 11, tb).as_deref(), Some("b"));
        assert_eq!(b.selected_display_text_from_host(&ba, 10, ta), None);
        assert!(b.begin_preedit_from_host(&ba,10,ta).is_none());
        #[cfg(feature = "cpu-pending")]
        let before_b = b.render_cpu(1024 * 1024).expect("B frame");
        assert!(a.close());
        ba.retire();
        assert!(bb.accepts(11, tb));
        assert_eq!(b.accessible_label(), Some("B"));
        assert_eq!(b.component_version(), Some(1));
        assert_eq!(b.selected_display_text_from_host(&bb, 11, tb).as_deref(), Some("b"));
        #[cfg(feature = "cpu-pending")]
        assert_eq!(b.render_cpu(1024 * 1024).expect("B surviving frame"), before_b);
        assert!(b.close_focus_from_host(&mut bb, 11, tb));
    }

    #[test]
    fn actual_viewport_scroll_maps_and_restores() {
        let path = std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes = std::fs::read(path).expect("injected test font must be readable");
        let context = UiInputContext { target: NodeId(9), target_generation: 1, document: 2, revision: 3 };
        let text = (0..80).map(|n| format!("段落{n} unique display words ")).collect::<String>();
        for scale in [1.0f32, 1.5, 2.0] {
        let mut view = NativeTextView::new(10, context, &bytes, &text, 320, 200);
        assert!(view.set_host_scale(scale));
        let mut binding = UiInputBinding::new(10, 1);
        let ticket = binding.bind(10, context).expect("scroll binding");
        #[cfg(feature = "cpu-pending")]
        let before = view.render_cpu(1024 * 1024).expect("scroll baseline");
        assert_eq!(view.hit_display_text_from_host(&binding, 10, ticket, -1.0, 20.0), None);
        assert_eq!(view.hit_display_text_from_host(&binding, 10, ticket, 320.0 / scale + 1.0, 20.0), None);
        let hit_before = view.hit_display_text_from_host(&binding, 10, ticket, 30.0, 20.0).expect("baseline text hit");
        assert!(view.set_scroll_from_host(&binding, 10, ticket, 0.0, 32.0));
        let hit_scrolled = view.hit_display_text_from_host(&binding, 10, ticket, 30.0, 20.0).expect("scrolled text hit");
        assert!(hit_scrolled > hit_before, "same viewport point must hit later display text after scroll");
        assert_eq!(view.scroll_position(), Some((0.0,32.0)));
        #[cfg(feature = "cpu-pending")]
        assert_ne!(view.render_cpu(1024 * 1024).expect("scrolled frame"), before);
        for (caller,x,y) in [(11,0.0,0.0),(10,-1.0,0.0),(10,f64::NAN,0.0),(10,0.0,f64::INFINITY)] {
            assert!(!view.set_scroll_from_host(&binding, caller, ticket, x, y));
            assert_eq!(view.scroll_position(), Some((0.0,32.0)));
        }
        assert!(view.set_scroll_from_host(&binding, 10, ticket, 0.0, 0.0));
        assert_eq!(view.hit_display_text_from_host(&binding, 10, ticket, 30.0, 20.0), Some(hit_before));
        #[cfg(feature = "cpu-pending")]
        assert_eq!(view.render_cpu(1024 * 1024).expect("restored scroll frame"), before);
        assert!(view.close());
        assert!(!view.set_scroll_from_host(&binding, 10, ticket, 0.0,0.0));
        assert_eq!(view.scroll_position(), None);
        }
    }

    #[test]
    fn cancelled_preedit_cannot_resurrect_or_clear_successor() {
        let path = std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes = std::fs::read(path).expect("injected test font must be readable");
        let context = UiInputContext { target: NodeId(9), target_generation: 1, document: 2, revision: 3 };
        let mut view = NativeTextView::new(10, context, &bytes, "document", 320, 200);
        let mut binding = UiInputBinding::new(10, 1);
        let ticket = binding.bind(10, context).expect("preedit binding");
        let old = view.begin_preedit_from_host(&binding,10,ticket).expect("old token");
        assert!(view.set_preedit_from_host(&binding,10,old,Some("old")));
        assert!(view.cancel_preedit_from_host(10,old));
        assert!(binding.accepts(10,ticket));
        assert!(!view.set_preedit_from_host(&binding,10,old,Some("late")));
        let fresh = view.begin_preedit_from_host(&binding,10,ticket).expect("fresh token");
        assert!(fresh.serial > old.serial);
        assert!(view.set_preedit_from_host(&binding,10,fresh,Some("successor")));
        #[cfg(feature = "cpu-pending")]
        let before = view.render_cpu(1024 * 1024).expect("successor frame");
        assert!(!view.cancel_preedit_from_host(10,old));
        assert!(!view.cancel_preedit_from_host(11,fresh));
        assert!(!view.set_preedit_from_host(&binding,10,old,Some("late")));
        #[cfg(feature = "cpu-pending")]
        assert_eq!(view.render_cpu(1024 * 1024).expect("late rejection frame"),before);
        assert!(view.cancel_preedit_from_host(10,fresh));
        assert!(view.preedit.current_token().is_none());
    }

    #[test]
    fn snapshot_change_invalidates_old_preedit_and_selection() {
        let path = std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes = std::fs::read(path).expect("injected test font must be readable");
        let old_context = UiInputContext { target: NodeId(9), target_generation: 1, document: 2, revision: 3 };
        let next_context = UiInputContext { revision: 4, ..old_context };
        let mut view = NativeTextView::new(10, old_context, &bytes, "old document", 320, 200);
        let mut binding = UiInputBinding::new(10,1);
        let old_ticket = binding.bind(10,old_context).expect("old focus");
        let old = view.begin_preedit_from_host(&binding,10,old_ticket).expect("old token");
        assert!(view.set_preedit_from_host(&binding,10,old,Some("old preedit")));
        assert!(view.set_selection_from_host(&binding,10,old_ticket,Some((0,3))));
        assert!(view.apply_display_snapshot_from_host(10,old_context,next_context,"new snapshot"));
        assert!(view.preedit.current_token().is_none());
        assert!(binding.accepts(10,old_ticket)); // Host owns the separate focus binding.
        assert!(!view.set_preedit_from_host(&binding,10,old,Some("late")));
        assert!(!view.cancel_preedit_from_host(10,old));
        let ticket = binding.replace(10,old_ticket,next_context).expect("new focus context");
        assert!(view.selected_display_text_from_host(&binding,10,ticket).is_none());
        let fresh = view.begin_preedit_from_host(&binding,10,ticket).expect("new token");
        assert!(fresh.serial > old.serial);
        assert!(view.set_preedit_from_host(&binding,10,fresh,Some("new preedit")));
        assert!(view.set_selection_from_host(&binding,10,ticket,Some((0,3))));
        assert_eq!(view.selected_display_text_from_host(&binding,10,ticket).as_deref(),Some("new"));
        #[cfg(feature = "cpu-pending")]
        let before = view.render_cpu(1024*1024).expect("new snapshot frame");
        assert!(!view.set_preedit_from_host(&binding,10,old,Some("late")));
        assert!(!view.cancel_preedit_from_host(10,old));
        #[cfg(feature = "cpu-pending")]
        assert_eq!(view.render_cpu(1024*1024).expect("late rejection frame"),before);
    }

    #[test]
    fn replacing_focus_binding_clears_previous_preedit_before_new_payload() {
        let path = std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes = std::fs::read(path).expect("injected test font must be readable");
        let context = UiInputContext { target: NodeId(9), target_generation: 1, document: 2, revision: 3 };
        let mut view = NativeTextView::new(10,context,&bytes,"document",320,200);
        let mut binding = UiInputBinding::new(10,1);
        let ticket = binding.bind(10,context).expect("old binding");
        let old = view.begin_preedit_from_host(&binding,10,ticket).expect("old composition");
        #[cfg(feature = "cpu-pending")]
        let empty = view.render_cpu(1024*1024).expect("empty preedit frame");
        assert!(view.set_preedit_from_host(&binding,10,old,Some("old composition text")));
        let successor = binding.replace(10,ticket,context).expect("replacement binding");
        let fresh = view.begin_preedit_from_host(&binding,10,successor).expect("successor composition");
        assert!(fresh.serial > old.serial);
        #[cfg(feature = "cpu-pending")]
        assert!(view.render_cpu(1024*1024).expect("successor before first payload") == empty, "old preedit pixels retained");
        assert!(!view.set_preedit_from_host(&binding,10,old,Some("late")));
        assert!(!view.cancel_preedit_from_host(10,old));
    }

    // 外观更新只投影样式，不能重建输入关联或丢失外部选区/滚动。
    // Appearance changes project styles without rebuilding input identity or external selection/scroll.
    #[test]
    fn appearance_updates_preserve_active_input_and_display_state() {
        let path = std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes = std::fs::read(path).expect("injected test font must be readable");
        let context = UiInputContext { target: NodeId(9), target_generation: 1, document: 2, revision: 3 };
        let mut view = NativeTextView::new(10,context,&bytes,"外部文档快照与受控选区",320,200);
        let mut binding = UiInputBinding::new(10,1);
        let ticket = binding.bind(10,context).expect("active focus");
        let token = view.begin_preedit_from_host(&binding,10,ticket).expect("active composition");
        assert!(view.set_preedit_from_host(&binding,10,token,Some("预编辑")));
        assert!(view.set_selection_from_host(&binding,10,ticket,Some((0,6))));
        assert!(view.set_scroll_from_host(&binding,10,ticket,0.0,8.0));
        for (version, foreground, background) in [(0,[20,30,40],[240,240,240]), (1,[240,240,240],[20,30,40])] {
            assert!(view.apply_component_update(10,context,version,ComponentUpdate::Appearance(TextAppearance {
                font_size_px: 24, padding_px: 4, foreground, background,
            })));
            assert!(binding.accepts(10,ticket));
            let current = view.preedit.current_token().expect("composition preserved");
            assert_eq!(current.serial,token.serial);
            assert!(view.preedit.accepts(&binding,10,token));
            assert_eq!(view.selected_display_text_from_host(&binding,10,ticket).as_deref(),Some("外部"));
            assert_eq!(view.scroll_position(),Some((0.0,8.0)));
            assert!(matches_ui_view_owner(10,view.context,10,context));
            #[cfg(feature = "cpu-pending")]
            assert!(view.render_cpu(1024*1024).is_some());
        }
        assert!(view.set_preedit_from_host(&binding,10,token,Some("更新预编辑")));
        assert!(view.close_focus_from_host(&mut binding,10,ticket));
        assert!(!view.set_preedit_from_host(&binding,10,token,Some("迟到")));
    }

    #[test]
    fn routed_mouse_uses_blitz_and_rejects_foreign_or_closed_view() {
        let path = std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes = std::fs::read(path).expect("injected test font must be readable");
        let context = UiInputContext { target: NodeId(9), target_generation: 1, document: 2, revision: 3 };
        let mut view = NativeTextView::new(10,context,&bytes,"pointer text",320,200);
        assert!(view.pointer_from_host(10,context,PointerAction::Move,12.0,12.0,false));
        assert!(view.document.as_ref().unwrap().get_hover_node_id().is_some());
        #[cfg(feature = "cpu-pending")]
        let before = view.render_cpu(1024*1024).unwrap();
        assert!(!view.pointer_from_host(11,context,PointerAction::Down,12.0,12.0,true));
        let stale = UiInputContext { target_generation: 0, ..context };
        assert!(!view.pointer_from_host(10,stale,PointerAction::Down,12.0,12.0,true));
        assert!(!view.pointer_from_host(10,context,PointerAction::Down,f32::NAN,12.0,true));
        #[cfg(feature = "cpu-pending")]
        assert!(view.render_cpu(1024*1024).unwrap() == before, "rejected input changed pixels");
        assert!(view.pointer_from_host(10,context,PointerAction::Down,12.0,12.0,true));
        assert!(view.pointer_from_host(10,context,PointerAction::Move,60.0,12.0,true));
        assert!(view.pointer_from_host(10,context,PointerAction::Cancel,60.0,12.0,false));
        assert!(!view.document.as_mut().unwrap().unactive_node(), "cancel retained active backend state");
        assert!(view.document.as_ref().unwrap().get_mousedown_node_id().is_none(),"cancel retained native press identity");
        assert!(view.pointer_from_host(10,context,PointerAction::Down,12.0,12.0,true));
        assert!(view.pointer_from_host(10,context,PointerAction::Up,60.0,12.0,false));
        assert!(!view.document.as_mut().unwrap().unactive_node(), "release retained active backend state");
        assert!(view.pointer_from_host(10,context,PointerAction::Leave,400.0,12.0,false));
        assert!(view.document.as_ref().unwrap().get_hover_node_id().is_none());
        assert!(view.close());
        assert!(!view.pointer_from_host(10,context,PointerAction::Move,12.0,12.0,false));
    }

    #[test]
    fn framework_pointer_callbacks_are_typed_and_admission_gated() {
        use crate::native_events::{NativeEventReply,ViewPart};
        let path=std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes=std::fs::read(path).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut view=NativeTextView::new(10,context,&bytes,"callback text",320,200);
        let mut seen=Vec::new();
        let mut sink=|event: crate::native_events::NativePointerEvent| {
            assert!(matches_ui_view_owner(10,context,10,event.context));
            seen.push((event.phase,event.part));
            NativeEventReply {prevent_default:true,stop_propagation:true}
        };
        assert!(!view.pointer_with_events_from_host(11,context,PointerAction::Down,12.0,12.0,true,&mut sink));
        for phase in [PointerAction::Down,PointerAction::Move,PointerAction::Cancel] {
            assert!(view.pointer_with_events_from_host(10,context,phase,12.0,12.0,phase!=PointerAction::Cancel,&mut sink));
        }
        assert_eq!(seen,vec![(PointerAction::Down,ViewPart::Text),(PointerAction::Move,ViewPart::Text),(PointerAction::Cancel,ViewPart::Text)]);
        assert!(view.document.as_ref().unwrap().get_selected_text().is_none());
        assert!(view.close());
        assert!(!view.pointer_with_events_from_host(10,context,PointerAction::Move,12.0,12.0,false,
            &mut |_| panic!("closed view callback")));
    }

    #[test]
    fn focused_command_keys_reach_framework_without_editing_document() {
        use crate::{native_keys::{HostKeyInput,NativeKeyEvent},native_events::NativeEventReply};
        let path=std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes=std::fs::read(path).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut view=NativeTextView::new(10,context,&bytes,"external text",320,200);
        let mut binding=UiInputBinding::new(10,1);
        let ticket=binding.bind(10,context).unwrap();
        let mut seen=Vec::new();
        let mut sink=|event:NativeKeyEvent| {
            assert!(matches_ui_view_owner(10,context,10,event.context));
            seen.push((event.key,event.code,event.pressed,event.repeat,event.composing));
            NativeEventReply {prevent_default:true,stop_propagation:true}
        };
        let mut key=HostKeyInput {key:"ArrowLeft",code:"ArrowLeft",pressed:true,repeat:true,composing:false,modifiers:Default::default(),location:Default::default()};
        assert!(!view.key_from_host(&binding,10,ticket,&key,&mut sink));
        assert!(!view.focus_from_host(&binding,11,ticket));
        assert!(view.focus_from_host(&binding,10,ticket));
        assert!(view.focus_from_host(&binding,10,ticket));
        assert!(!view.key_from_host(&binding,11,ticket,&key,&mut sink));
        assert!(view.key_from_host(&binding,10,ticket,&key,&mut sink));
        key.pressed=false;
        assert!(view.key_from_host(&binding,10,ticket,&key,&mut sink));
        assert_eq!(seen,vec![("ArrowLeft".into(),"ArrowLeft".into(),true,true,false),
            ("ArrowLeft".into(),"ArrowLeft".into(),false,true,false)]);
        assert!(view.close_focus_from_host(&mut binding,10,ticket));
        // Blitz returns the document root as fallback after clear_focus.
        assert_ne!(view.document.as_ref().unwrap().get_focussed_node_id(),Some(view.text_element));
        assert!(!view.key_from_host(&binding,10,ticket,&key,&mut |_| panic!("late key callback")));
        let next=binding.bind(10,context).unwrap();
        assert!(view.focus_from_host(&binding,10,next));
        assert!(view.set_selection_from_host(&binding,10,next,Some((0,13))));
        assert_eq!(view.selected_display_text_from_host(&binding,10,next).as_deref(),Some("external text"));
    }

    #[test]
    fn command_keys_preserve_modifier_combinations_and_locations() {
        use crate::{native_keys::{HostKeyInput,KeyLocation},native_events::NativeEventReply};
        use jasper_ui_engine::input::Modifiers;
        let path=std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes=std::fs::read(path).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut view=NativeTextView::new(10,context,&bytes,"external text",320,200);
        let mut binding=UiInputBinding::new(10,1);
        let ticket=binding.bind(10,context).unwrap();
        assert!(view.focus_from_host(&binding,10,ticket));
        for mask in 0..16 {
            let modifiers=Modifiers {shift:mask&1!=0,control:mask&2!=0,alt:mask&4!=0,meta:mask&8!=0};
            for location in [KeyLocation::Standard,KeyLocation::Left,KeyLocation::Right,KeyLocation::Numpad] {
                let input=HostKeyInput {key:"ArrowLeft",code:"ArrowLeft",pressed:true,repeat:false,
                    composing:true,modifiers,location};
                let mut calls=0;
                assert!(view.key_from_host(&binding,10,ticket,&input,&mut |event| {
                    calls+=1;
                    assert_eq!(event.modifiers,modifiers);
                    assert_eq!(event.location,location);
                    assert!(event.composing);
                    NativeEventReply {prevent_default:true,stop_propagation:true}
                }));
                assert_eq!(calls,1);
            }
        }
    }

    #[test]
    fn framework_pointer_positions_are_view_local_across_scale_and_scroll() {
        use crate::native_events::NativeEventReply;
        let path=std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes=std::fs::read(path).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut view=NativeTextView::new(10,context,&bytes,"line one text line two text",320,200);
        let mut binding=UiInputBinding::new(10,1);
        let ticket=binding.bind(10,context).unwrap();
        assert!(view.set_host_scale(2.0));
        assert!(view.set_scroll_from_host(&binding,10,ticket,0.0,4.0));
        let mut events=Vec::new();
        for (phase,pressed) in [(PointerAction::Down,true),(PointerAction::Move,true),(PointerAction::Up,false)] {
            assert!(view.pointer_with_events_from_host(10,context,phase,12.0,12.0,pressed,&mut |event| {
                events.push((event.phase,event.position,event.primary_pressed));
                NativeEventReply {prevent_default:true,stop_propagation:false}
            }));
        }
        assert_eq!(events,vec![(PointerAction::Down,(12.0,12.0),true),
            (PointerAction::Move,(12.0,12.0),true),(PointerAction::Up,(12.0,12.0),false)]);
    }

    #[test]
    fn typed_button_uses_native_element_and_versioned_caption_updates() {
        let path=std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes=std::fs::read(path).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut button=NativeTextView::new_button(10,context,&bytes,"确认",320,200);
        {
            let tree=button.document.as_mut().unwrap().mutate();
            assert_eq!(tree.element_name(button.text_element).unwrap().local,LocalName::from("button"));
        }
        #[cfg(feature="cpu-pending")]
        let before=button.render_cpu(1024*1024).unwrap();
        assert!(button.apply_component_update(10,context,0,ComponentUpdate::ButtonCaption("保存并继续")));
        assert_eq!(button.component_version(),Some(1));
        #[cfg(feature="cpu-pending")]
        assert!(button.render_cpu(1024*1024).unwrap()!=before,"button caption did not update pixels");
        assert!(!button.apply_component_update(10,context,0,ComponentUpdate::ButtonCaption("stale")));
        let mut binding=UiInputBinding::new(10,1);
        let ticket=binding.bind(10,context).unwrap();
        assert!(button.focus_from_host(&binding,10,ticket));
        let mut text=NativeTextView::new(10,context,&bytes,"external document",320,200);
        assert!(!text.apply_component_update(10,context,0,ComponentUpdate::ButtonCaption("must not edit document")));
        assert_eq!(text.component_version(),Some(0));
    }

    #[test]
    fn versioned_disabled_button_rejects_focus_and_command_keys() {
        use crate::native_keys::HostKeyInput;
        let path=std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes=std::fs::read(path).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut button=NativeTextView::new_button(10,context,&bytes,"保存",320,200);
        let mut binding=UiInputBinding::new(10,1);
        let ticket=binding.bind(10,context).unwrap();
        assert!(button.focus_from_host(&binding,10,ticket));
        assert!(button.apply_component_update(10,context,0,ComponentUpdate::ButtonDisabled(true)));
        assert!(binding.accepts(10,ticket)); // External service focus is not owned by the component setter.
        assert!(!button.focus_from_host(&binding,10,ticket));
        assert!(button.pointer_from_host(10,context,PointerAction::Down,12.0,12.0,true));
        let key=HostKeyInput {key:"Enter",code:"Enter",pressed:true,repeat:false,composing:false,
            modifiers:Default::default(),location:Default::default()};
        assert!(!button.key_from_host(&binding,10,ticket,&key,&mut |_| panic!("disabled button key callback")));
        assert!(!button.apply_component_update(10,context,0,ComponentUpdate::ButtonDisabled(false)));
        assert!(!button.focus_from_host(&binding,10,ticket));
        assert!(button.apply_component_update(10,context,1,ComponentUpdate::ButtonDisabled(false)));
        assert!(button.focus_from_host(&binding,10,ticket));
        let mut text=NativeTextView::new(10,context,&bytes,"external document",320,200);
        assert!(!text.apply_component_update(10,context,0,ComponentUpdate::ButtonDisabled(true)));
        assert_eq!(text.component_version(),Some(0));
    }

    #[test]
    fn typed_button_terminal_up_cannot_reactivate_a_finished_or_cancelled_press() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut button=NativeTextView::new_button(10,context,&bytes,"Save",320,200);
        let mut clicks=0;
        for phase in [PointerAction::Down,PointerAction::Up,PointerAction::Up] {
            assert!(button.button_pointer_from_host(10,context,phase,16.0,16.0,phase==PointerAction::Down,
                &mut |_| {clicks+=1;crate::native_events::NativeEventReply::default()}));
        }
        assert_eq!(clicks,1,"duplicate Up must not create another native activation");
        for phase in [PointerAction::Down,PointerAction::Cancel,PointerAction::Up] {
            assert!(button.button_pointer_from_host(10,context,phase,16.0,16.0,phase==PointerAction::Down,
                &mut |_| {clicks+=1;crate::native_events::NativeEventReply::default()}));
        }
        assert_eq!(clicks,1,"cancelled press must not activate on a later Up");
        assert!(button.button_pointer_from_host(10,context,PointerAction::Down,16.0,16.0,true,
            &mut |_|panic!("Down activation")));
        assert!(!button.button_pointer_from_host(11,context,PointerAction::Up,16.0,16.0,false,
            &mut |_|panic!("foreign activation")));
        assert!(button.button_pointer_from_host(10,context,PointerAction::Up,16.0,16.0,false,
            &mut |_| {clicks+=1;crate::native_events::NativeEventReply::default()}));
        assert_eq!(clicks,2,"rejected foreign Up must preserve the live native press");
        let mut ups=0;
        assert!(button.dispatch_pointer_from_host(10,context,PointerAction::Up,16.0,16.0,false,None,
            &mut |event| {if event.phase==PointerAction::Up {ups+=1;}crate::native_events::NativeEventReply::default()},
            Some(&mut |_|panic!("unpaired Up activation"))));
        assert_eq!(ups,1,"default suppression must preserve typed pointer delivery");
    }

    #[test]
    fn typed_button_activation_comes_from_blitz_click_and_respects_disabled() {
        use crate::native_events::{NativeActivationEvent,NativeEventReply};
        let path=std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes=std::fs::read(path).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut button=NativeTextView::new_button(10,context,&bytes,"确认",320,200);
        let mut clicks=0;
        let mut sink=|event:NativeActivationEvent| {
            assert!(matches_ui_view_owner(10,context,10,event.context));
            clicks+=1; NativeEventReply::default()
        };
        assert!(!button.button_pointer_from_host(11,context,PointerAction::Down,16.0,16.0,true,&mut sink));
        for phase in [PointerAction::Down,PointerAction::Up] {
            assert!(button.button_pointer_from_host(10,context,phase,16.0,16.0,phase==PointerAction::Down,&mut sink));
        }
        assert_eq!(clicks,1);
        assert!(button.apply_component_update(10,context,0,ComponentUpdate::ButtonDisabled(true)));
        for phase in [PointerAction::Down,PointerAction::Up] {
            assert!(button.button_pointer_from_host(10,context,phase,16.0,16.0,phase==PointerAction::Down,
                &mut |_| panic!("disabled button activation")));
        }
        assert!(button.apply_component_update(10,context,1,ComponentUpdate::ButtonDisabled(false)));
        let mut cancelled_clicks=0;
        for phase in [PointerAction::Down,PointerAction::Cancel] {
            assert!(button.button_pointer_from_host(10,context,phase,16.0,16.0,phase==PointerAction::Down,
                &mut |_| {cancelled_clicks+=1;NativeEventReply::default()}));
        }
        assert_eq!(cancelled_clicks,0);
    }

    #[test]
    fn button_enter_activation_requires_admitted_focus_without_ime_or_shortcut() {
        use crate::native_keys::HostKeyInput;
        let path=std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes=std::fs::read(path).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut button=NativeTextView::new_button(10,context,&bytes,"确认",320,200);
        let mut binding=UiInputBinding::new(10,1);
        let ticket=binding.bind(10,context).unwrap();
        let mut key=HostKeyInput {key:"Enter",code:"Enter",pressed:true,repeat:false,composing:false,
            modifiers:Default::default(),location:Default::default()};
        assert!(!button.button_enter_from_host(&binding,10,ticket,&key,&mut |_| panic!("unfocused activation")));
        assert!(button.focus_from_host(&binding,10,ticket));
        let mut calls=0;
        assert!(button.button_enter_from_host(&binding,10,ticket,&key,&mut |event| {
            assert!(matches_ui_view_owner(10,context,10,event.context)); calls+=1;
        }));
        assert_eq!(calls,1);
        key.composing=true;
        assert!(!button.button_enter_from_host(&binding,10,ticket,&key,&mut |_| panic!("IME activation")));
        key.composing=false;key.repeat=true;
        assert!(!button.button_enter_from_host(&binding,10,ticket,&key,&mut |_| panic!("repeat activation")));
        key.repeat=false;key.modifiers.control=true;
        assert!(!button.button_enter_from_host(&binding,10,ticket,&key,&mut |_| panic!("shortcut activation")));
        key.modifiers.control=false;
        assert!(button.apply_component_update(10,context,0,ComponentUpdate::ButtonDisabled(true)));
        assert!(!button.button_enter_from_host(&binding,10,ticket,&key,&mut |_| panic!("disabled activation")));
        assert!(button.close_focus_from_host(&mut binding,10,ticket));
        assert!(!button.button_enter_from_host(&binding,10,ticket,&key,&mut |_| panic!("closed focus activation")));
    }

    #[test]
    fn own_semantics_export_role_name_disabled_focus_and_authorized_context() {
        use crate::native_semantics::SemanticRole;
        let path=std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes=std::fs::read(path).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut button=NativeTextView::new_button(10,context,&bytes,"保存",320,200);
        assert!(button.semantics_from_host(11,context).is_none());
        let initial=button.semantics_from_host(10,context).unwrap();
        assert_eq!(initial.role,SemanticRole::Button);
        assert_eq!(initial.name,"保存");assert!(initial.can_activate);assert!(!initial.disabled);
        assert_eq!(initial.component_version,0);
        assert!(matches_ui_view_owner(10,context,10,initial.context));
        let mut binding=UiInputBinding::new(10,1);
        let ticket=binding.bind(10,context).unwrap();
        assert!(button.focus_from_host(&binding,10,ticket));
        assert!(button.semantics_from_host(10,context).unwrap().native_focused);
        assert!(button.apply_component_update(10,context,0,ComponentUpdate::AccessibleLabel(Some("保存文件"))));
        assert_eq!(button.semantics_from_host(10,context).unwrap().name,"保存文件");
        assert!(button.apply_component_update(10,context,1,ComponentUpdate::AccessibleLabel(Some(""))));
        assert_eq!(button.semantics_from_host(10,context).unwrap().name,"");
        assert!(button.apply_component_update(10,context,2,ComponentUpdate::ButtonDisabled(true)));
        let disabled=button.semantics_from_host(10,context).unwrap();
        assert!(disabled.disabled);assert!(!disabled.can_activate);assert!(!disabled.native_focused);
        assert_eq!(disabled.component_version,3);
        assert!(button.close());assert!(button.semantics_from_host(10,context).is_none());
        let text=NativeTextView::new(10,context,&bytes,"外部文档",320,200);
        let plain=text.semantics_from_host(10,context).unwrap();
        assert_eq!(plain.role,SemanticRole::Text);assert!(!plain.can_activate);assert_eq!(plain.name,"外部文档");
    }

    #[test]
    fn disabling_component_notifies_current_focus_owner_once() {
        let path=std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes=std::fs::read(path).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut button=NativeTextView::new_button(10,context,&bytes,"Save",320,200);
        let mut binding=UiInputBinding::new(10,1);
        let ticket=binding.bind(10,context).unwrap();
        assert!(button.focus_from_host(&binding,10,ticket));
        let mut events=Vec::new();
        assert!(!button.apply_component_update_with_focus_events(&binding,11,ticket,0,
            ComponentUpdate::ButtonDisabled(true),&mut |event| events.push(event)));
        assert!(button.semantics_from_host(10,context).unwrap().native_focused);
        assert!(button.apply_component_update_with_focus_events(&binding,10,ticket,0,
            ComponentUpdate::ButtonDisabled(true),&mut |event| events.push(event)));
        assert_eq!(events.len(),1);assert!(!events[0].focused);assert!(binding.accepts(10,events[0].owner));
        assert!(binding.accepts(10,ticket));
        assert!(!button.apply_component_update_with_focus_events(&binding,10,ticket,0,
            ComponentUpdate::ButtonDisabled(false),&mut |event| events.push(event)));
        assert!(button.apply_component_update_with_focus_events(&binding,10,ticket,1,
            ComponentUpdate::ButtonDisabled(true),&mut |event| events.push(event)));
        assert!(button.apply_component_update_with_focus_events(&binding,10,ticket,2,
            ComponentUpdate::ButtonDisabled(false),&mut |event| events.push(event)));
        assert_eq!(events.len(),1);
        assert!(binding.close(10,ticket));
        assert!(!button.apply_component_update_with_focus_events(&binding,10,ticket,3,
            ComponentUpdate::ButtonDisabled(true),&mut |event| events.push(event)));
        assert_eq!(button.component_version(),Some(3));
    }

    #[test]
    fn semantic_activation_requires_current_enabled_button_snapshot() {
        let path=std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required");
        let bytes=std::fs::read(path).unwrap();
        let context=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};
        let mut button=NativeTextView::new_button(10,context,&bytes,"确认",320,200);
        let snapshot=button.semantics_from_host(10,context).unwrap();
        assert!(!snapshot.native_focused);
        let mut calls=0;
        assert!(button.activate_from_host(10,context,snapshot.component_version,&mut |event| {
            assert!(matches_ui_view_owner(10,context,10,event.context)); calls+=1;
        }));
        assert_eq!(calls,1);
        assert!(!button.activate_from_host(11,context,0,&mut |_| panic!("foreign action")));
        assert!(button.apply_component_update(10,context,0,ComponentUpdate::ButtonCaption("已更新")));
        assert!(!button.activate_from_host(10,context,snapshot.component_version,&mut |_| panic!("stale action")));
        assert!(button.activate_from_host(10,context,1,&mut |_| {calls+=1;}));
        assert_eq!(calls,2);
        assert!(button.apply_component_update(10,context,1,ComponentUpdate::ButtonDisabled(true)));
        assert!(!button.activate_from_host(10,context,2,&mut |_| panic!("disabled action")));
        assert!(button.close());
        assert!(!button.activate_from_host(10,context,2,&mut |_| panic!("closed action")));
        let text=NativeTextView::new(10,context,&bytes,"text",320,200);
        assert!(!text.activate_from_host(10,context,0,&mut |_| panic!("text action")));
    }

}

#[cfg(test)]
mod capture_successor_tests {
    use super::*;
    #[test]
    fn successor_cancel_clears_native_press_without_stale_callback() {
        let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
        let first=UiInputContext {target:jasper_ui_engine::scene::NodeId(7),target_generation:1,document:20,revision:1};
        let next=UiInputContext {revision:2,..first};let mut view=NativeTextView::new(10,first,&font,"Original",160,80);
        assert!(view.pointer_from_host(10,first,PointerAction::Down,12.0,12.0,true));
        assert!(view.apply_display_snapshot_from_host(10,first,next,"Successor"));
        assert!(view.cancel_capture_from_host(10,first,true,12.0,12.0,Default::default(),None,
            &mut |_|panic!("stale callback"),&mut |_|panic!("stale activation")));
        assert!(!view.document.as_mut().unwrap().unactive_node());
        assert!(view.document.as_ref().unwrap().get_mousedown_node_id().is_none());
    }
}
