//! Host-routed command keys, not text commits or an input engine.
//! 宿主路由命令键；文本提交走外部文本/输入服务，不在此实现编辑引擎。
use crate::native_events::NativeEventReply;
use jasper_ui_engine::{input_identity::UiInputContext,input::Modifiers};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeyLocation { #[default] Standard, Left, Right, Numpad }
pub struct HostKeyInput<'a> {
    pub key: &'a str,
    pub code: &'a str,
    pub pressed: bool,
    pub repeat: bool,
    pub composing: bool,
    pub modifiers: Modifiers,
    pub location: KeyLocation,
}
pub struct NativeKeyEvent {
    pub context: UiInputContext,
    pub key: String,
    pub code: String,
    pub pressed: bool,
    pub repeat: bool,
    pub composing: bool,
    pub modifiers: Modifiers,
    pub location: KeyLocation,
}
pub(crate) struct FrameworkKeyHandler<'a, F> {
    pub context: UiInputContext,
    pub text: blitz_dom::NodeId,
    pub sink: &'a mut F,
}
impl<F: FnMut(NativeKeyEvent) -> NativeEventReply> blitz_dom::EventHandler for FrameworkKeyHandler<'_, F> {
    fn handle_event(&mut self, chain: &[blitz_dom::NodeId], event: &mut blitz_traits::events::DomEvent,
        _: &mut dyn blitz_dom::Document, state: &mut blitz_traits::events::EventState) {
        use blitz_traits::events::DomEventData;
        let (key,pressed) = match &event.data {
            DomEventData::KeyDown(key) => (key,true),
            DomEventData::KeyUp(key) => (key,false),
            _ => return,
        };
        if !chain.contains(&self.text) { return; }
        let reply=(self.sink)(NativeKeyEvent {context:self.context,key:key.key.to_string(),
            code:key.code.to_string(),pressed,repeat:key.is_auto_repeating,composing:key.is_composing,
            modifiers:Modifiers {shift:key.modifiers.contains(keyboard_types::Modifiers::SHIFT),
                control:key.modifiers.contains(keyboard_types::Modifiers::CONTROL),
                alt:key.modifiers.contains(keyboard_types::Modifiers::ALT),
                meta:key.modifiers.contains(keyboard_types::Modifiers::META)},
            location:match key.location {keyboard_types::Location::Standard=>KeyLocation::Standard,
                keyboard_types::Location::Left=>KeyLocation::Left,keyboard_types::Location::Right=>KeyLocation::Right,
                keyboard_types::Location::Numpad=>KeyLocation::Numpad},
        });
        if reply.prevent_default { state.prevent_default(); }
        if reply.stop_propagation { state.stop_propagation(); }
    }
}

/// Single private conversion for scene and external-text command clients.
/// Text commits remain external snapshots; no native text payload is synthesized.
pub(crate) fn native_command_key(input:&HostKeyInput<'_>)->Option<blitz_traits::events::BlitzKeyEvent> {
    use blitz_traits::events::{BlitzKeyEvent,KeyState};
    let key=input.key.parse().ok()?;let code=input.code.parse().ok()?;
    let mut modifiers=keyboard_types::Modifiers::empty();
    if input.modifiers.shift { modifiers.insert(keyboard_types::Modifiers::SHIFT); }
    if input.modifiers.control { modifiers.insert(keyboard_types::Modifiers::CONTROL); }
    if input.modifiers.alt { modifiers.insert(keyboard_types::Modifiers::ALT); }
    if input.modifiers.meta { modifiers.insert(keyboard_types::Modifiers::META); }
    let location=match input.location {
        crate::native_keys::KeyLocation::Standard=>keyboard_types::Location::Standard,
        crate::native_keys::KeyLocation::Left=>keyboard_types::Location::Left,
        crate::native_keys::KeyLocation::Right=>keyboard_types::Location::Right,
        crate::native_keys::KeyLocation::Numpad=>keyboard_types::Location::Numpad,
    };
    let key=BlitzKeyEvent {key,code,modifiers,location,
        is_auto_repeating:input.repeat,is_composing:input.composing,
        state:if input.pressed {KeyState::Pressed} else {KeyState::Released},text:None};
    Some(key)
}

#[cfg(test)]
mod conversion_tests {
    use super::*;
    #[test]
    fn command_conversion_preserves_all_metadata_without_text_commit() {
        let locations=[KeyLocation::Standard,KeyLocation::Left,KeyLocation::Right,KeyLocation::Numpad];
        let native_locations=[keyboard_types::Location::Standard,keyboard_types::Location::Left,
            keyboard_types::Location::Right,keyboard_types::Location::Numpad];
        for mask in 0..16 {for (location,expected_location) in locations.into_iter().zip(native_locations) {for flags in 0..8 {
            let input=HostKeyInput {key:"ArrowLeft",code:"ArrowLeft",pressed:flags&1!=0,repeat:flags&2!=0,composing:flags&4!=0,
                modifiers:Modifiers {shift:mask&1!=0,control:mask&2!=0,alt:mask&4!=0,meta:mask&8!=0},location};
            let converted=native_command_key(&input).unwrap();
            assert_eq!(converted.key.to_string(),input.key);assert_eq!(converted.code.to_string(),input.code);
            assert_eq!(converted.location,expected_location);assert_eq!(converted.state.is_pressed(),input.pressed);
            assert_eq!(converted.is_auto_repeating,input.repeat);assert_eq!(converted.is_composing,input.composing);
            assert!(converted.text.is_none());
            for (bit,present) in [(keyboard_types::Modifiers::SHIFT,input.modifiers.shift),
                (keyboard_types::Modifiers::CONTROL,input.modifiers.control),(keyboard_types::Modifiers::ALT,input.modifiers.alt),
                (keyboard_types::Modifiers::META,input.modifiers.meta)] {assert_eq!(converted.modifiers.contains(bit),present);}
        }}}
        let invalid=HostKeyInput {key:"ArrowLeft",code:"InvalidKeyCode",pressed:true,repeat:false,composing:false,
            modifiers:Modifiers::default(),location:KeyLocation::Standard};
        assert!(native_command_key(&invalid).is_none());
    }
}
