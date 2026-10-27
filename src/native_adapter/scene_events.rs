//! Own per-node scene event projection; native targeting/default actions stay in Blitz.
use jasper_ui_engine::{scene::NodeId,semantics::{Role,Semantics}};
use crate::{native_view::PointerAction,native_events::NativeEventReply};
use std::collections::BTreeMap;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum SceneEventKind {Pointer(PointerAction),Activate}
/// Host-routed logical mouse sample; platform identity/capture remain host-owned.
pub struct ScenePointerInput {
    pub phase:PointerAction,
    pub position:(f32,f32),
    pub primary_pressed:bool,
    pub modifiers:jasper_ui_engine::input::Modifiers,
}
pub struct SceneEvent {
    pub scene_version:u64,
    pub node:NodeId,
    pub kind:SceneEventKind,
    /// View-local logical coordinates; not OS screen coordinates.
    pub position:(f32,f32),
    /// State from the native sample, including Move/Up/Cancel.
    pub primary_pressed:bool,
    pub modifiers:jasper_ui_engine::input::Modifiers,
}
/// Raw Framework semantics and ordered source children; host builds its native API tree.
/// Names/roles are preserved, not recomputed from backend DOM or copied display text.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct SceneSemanticsNode {
    pub node:NodeId,
    pub semantics:Semantics,
    pub children:Vec<NodeId>,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct SceneSemanticsSnapshot {
    pub scene_version:u64,
    pub root:NodeId,
    pub nodes:Vec<SceneSemanticsNode>,
}
/// Semantic action identity has no pointer coordinates or backend target.
pub struct SceneActivationEvent {
    pub scene_version:u64,
    pub node:NodeId,
}
/// Native scene focus only; no window focus or external text-service lease ACK.
pub struct SceneFocusEvent {
    pub scene_version:u64,
    pub node:NodeId,
    pub focused:bool,
}
pub(crate) struct SceneEventHandler<'a,F> {
    pub version:u64,
    pub suppress_unpaired_up:bool,
    pub nodes:&'a BTreeMap<NodeId,(blitz_dom::NodeId,Option<blitz_dom::NodeId>)>,
    pub semantics:&'a BTreeMap<NodeId,Semantics>,
    pub sink:&'a mut F,
}
impl<F:FnMut(SceneEvent)->NativeEventReply> blitz_dom::EventHandler for SceneEventHandler<'_,F> {
    fn handle_event(&mut self,chain:&[blitz_dom::NodeId],event:&mut blitz_traits::events::DomEvent,
        _: &mut dyn blitz_dom::Document,state:&mut blitz_traits::events::EventState) {
        use blitz_traits::events::DomEventData;
        // Pinned chain remains authoritative. Pointer targets are the nearest source node;
        // activation belongs to the nearest Button component, including nested caption nodes.
        // Apply lifecycle default suppression even when no source node maps this native target.
        if self.suppress_unpaired_up && matches!(&event.data,DomEventData::PointerUp(_)) {state.prevent_default();}
        let activation=matches!(&event.data,DomEventData::Click(_));
        // Preserve Framework ancestor hiding for semantic actions; native targeting stays upstream.
        if activation && chain.iter().any(|target|self.nodes.iter().any(|(id,(element,text))|
            (*element==*target || *text==Some(*target)) && self.semantics.get(id).is_some_and(|value|value.hidden))) {return;}
        let Some(node)=chain.iter().find_map(|target|self.nodes.iter().find_map(|(id,(element,text))|
            ((*element==*target || *text==Some(*target))
                && (!activation || self.semantics.get(id).is_some_and(|s|s.role==Role::Button)))
                .then_some(*id))) else {return;};
        let (kind,pointer)=match &event.data {
            DomEventData::PointerDown(p)=>(SceneEventKind::Pointer(PointerAction::Down),p),
            DomEventData::PointerMove(p)=>(SceneEventKind::Pointer(PointerAction::Move),p),
            DomEventData::PointerUp(p)=>(SceneEventKind::Pointer(PointerAction::Up),p),
            DomEventData::PointerCancel(p)=>(SceneEventKind::Pointer(PointerAction::Cancel),p),
            DomEventData::PointerLeave(p)=>(SceneEventKind::Pointer(PointerAction::Leave),p),
            DomEventData::Click(p)=>{
                if !self.semantics.get(&node).is_some_and(|s|s.role==Role::Button && !s.disabled) {return;}
                (SceneEventKind::Activate,p)
            },
            _=>return,
        };
        let reply=(self.sink)(SceneEvent {scene_version:self.version,node,kind,
            position:(pointer.coords.client_x,pointer.coords.client_y),
            primary_pressed:pointer.buttons.contains(blitz_traits::events::MouseEventButtons::Primary),
            modifiers:jasper_ui_engine::input::Modifiers {
                shift:pointer.mods.contains(keyboard_types::Modifiers::SHIFT),
                control:pointer.mods.contains(keyboard_types::Modifiers::CONTROL),
                alt:pointer.mods.contains(keyboard_types::Modifiers::ALT),
                meta:pointer.mods.contains(keyboard_types::Modifiers::META)}});
        if reply.prevent_default {state.prevent_default();}
        if reply.stop_propagation {state.stop_propagation();}
    }
}

/// Final-state notifications only; no external input-service or OS acknowledgement.
pub(crate) fn notify_focus_transition(version:u64,before:Option<NodeId>,after:Option<NodeId>,
    sink:&mut impl FnMut(SceneFocusEvent)) {
    if before==after {return;}
    if let Some(node)=before {sink(SceneFocusEvent {scene_version:version,node,focused:false});}
    if let Some(node)=after {sink(SceneFocusEvent {scene_version:version,node,focused:true});}
}

/// Native command key projected onto a source node, never an external text-service commit.
pub struct SceneKeyEvent {
    pub scene_version:u64,
    pub node:NodeId,
    pub key:String,
    pub code:String,
    pub pressed:bool,
    pub repeat:bool,
    pub composing:bool,
    pub modifiers:jasper_ui_engine::input::Modifiers,
    pub location:crate::native_keys::KeyLocation,
}
pub(crate) struct SceneKeyHandler<'a,F> {
    pub version:u64,
    pub node:NodeId,
    pub element:blitz_dom::NodeId,
    pub sink:&'a mut F,
}
impl<F:FnMut(SceneKeyEvent)->NativeEventReply> blitz_dom::EventHandler for SceneKeyHandler<'_,F> {
    fn handle_event(&mut self,chain:&[blitz_dom::NodeId],event:&mut blitz_traits::events::DomEvent,
        _: &mut dyn blitz_dom::Document,state:&mut blitz_traits::events::EventState) {
        use blitz_traits::events::DomEventData;
        let (key,pressed)=match &event.data {
            DomEventData::KeyDown(key)=>(key,true),DomEventData::KeyUp(key)=>(key,false),_=>return,
        };
        if !chain.contains(&self.element) {return;}
        let reply=(self.sink)(SceneKeyEvent {scene_version:self.version,node:self.node,
            key:key.key.to_string(),code:key.code.to_string(),pressed,repeat:key.is_auto_repeating,
            composing:key.is_composing,modifiers:jasper_ui_engine::input::Modifiers {
                shift:key.modifiers.contains(keyboard_types::Modifiers::SHIFT),
                control:key.modifiers.contains(keyboard_types::Modifiers::CONTROL),
                alt:key.modifiers.contains(keyboard_types::Modifiers::ALT),
                meta:key.modifiers.contains(keyboard_types::Modifiers::META)},
            location:match key.location {
                keyboard_types::Location::Standard=>crate::native_keys::KeyLocation::Standard,
                keyboard_types::Location::Left=>crate::native_keys::KeyLocation::Left,
                keyboard_types::Location::Right=>crate::native_keys::KeyLocation::Right,
                keyboard_types::Location::Numpad=>crate::native_keys::KeyLocation::Numpad},
        });
        if reply.prevent_default {state.prevent_default();}
        if reply.stop_propagation {state.stop_propagation();}
    }
}
