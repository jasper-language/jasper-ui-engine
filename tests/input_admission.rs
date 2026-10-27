use jasper_ui_engine::{
    geometry::Point,
    input::{CompositionEvent, FocusEvent, InputEvent, KeyEvent, KeyPhase, Modifiers, PointerEvent, ScrollEvent},
    input_identity::{UiInputBinding, UiInputContext},
    scene::NodeId,
};

#[test]
fn normalized_entrances_reject_closed_or_stale_binding() {
    let mut binding=UiInputBinding::new(10,20);
    let current=UiInputContext{target:NodeId(30),target_generation:1,document:40,revision:50};
    let ticket=binding.bind(10,current).unwrap();
    let events=[
        InputEvent::Pointer(PointerEvent::default()),
        InputEvent::Key(KeyEvent{key:"Enter".into(),phase:KeyPhase::Down,modifiers:Modifiers::default(),repeat:false}),
        InputEvent::Text("你好".into()),
        InputEvent::Preedit{text:"nihao".into(),cursor:None},
        InputEvent::Composition(CompositionEvent{text:"nihao".into(),start:0,end:5}),
        InputEvent::Scroll(ScrollEvent{delta:Point::ZERO}),
        InputEvent::Focus(FocusEvent{focused:true}),
        InputEvent::ImeEnabled(false),
        InputEvent::ImeEnabled(true),
    ];
    for event in &events {
        let admitted=binding.admit(10,ticket,event).unwrap();
        assert!(std::ptr::eq(admitted.payload(),event));
        assert_eq!(admitted.target(),NodeId(30));
        let mut stale=ticket;stale.context.revision^=1;
        assert!(binding.admit(10,stale,event).is_none());
    }
    assert!(binding.close(10,ticket));
    let successor=binding.bind(10,current).unwrap();
    for event in &events {
        assert!(binding.admit(10,ticket,event).is_none());
        assert!(binding.admit(10,successor,event).is_some());
    }
    binding.retire();
    for event in &events {assert!(binding.admit(10,successor,event).is_none());}
}

#[test]
fn real_keyboard_focus_filters_payload_then_retires_binding() {
    use jasper_ui_engine::{keyboard::KeyboardRouter,scene::{Node,Scene},semantics::{Role,Semantics,SemanticsTree}};
    let scene=Scene::new(Node::container(NodeId(1))
        .child(Node::rect(NodeId(30)).with_semantics(Semantics::default().role(Role::TextField).focusable(true)))
        .child(Node::rect(NodeId(31)).with_semantics(Semantics::default().role(Role::TextField).focusable(true))));
    let tree=SemanticsTree::from_scene(&scene);
    let mut router=KeyboardRouter::new(&tree);
    let mut binding=UiInputBinding::new(10,20);
    let context=UiInputContext{target:NodeId(30),target_generation:1,document:40,revision:50};
    let ticket=binding.bind(10,context).unwrap();
    let payload=InputEvent::Text("你好".into());
    assert!(router.route_admitted(&binding.admit(10,ticket,&payload).unwrap()).is_none());
    router.focus(NodeId(30)).unwrap();
    { let admitted=binding.admit(10,ticket,&payload).unwrap();
      assert!(std::ptr::eq(router.route_admitted(&admitted).unwrap(),&payload)); }
    router.focus(NodeId(31)).unwrap();
    assert!(router.route_admitted(&binding.admit(10,ticket,&payload).unwrap()).is_none());
    assert_eq!(router.retire_input_binding(&mut binding),Some(NodeId(31)));
    assert_eq!(router.focused(),None);
    assert!(binding.admit(10,ticket,&payload).is_none());
}

#[test]
fn binding_rotation_rejects_old_revision_and_keeps_old_binding_on_denial() {
    let mut binding=UiInputBinding::new(10,20);
    let current=UiInputContext{target:NodeId(30),target_generation:1,document:40,revision:50};
    let old=binding.bind(10,current).unwrap();
    let mut next=current;next.revision=51;
    assert!(binding.replace(11,old,next).is_none());
    assert!(binding.accepts(10,old));
    let fresh=binding.replace(10,old,next).unwrap();
    assert_eq!(fresh.key.generation,old.key.generation+1);
    assert!(!binding.accepts(10,old));
    assert!(!binding.close(10,old));
    assert!(binding.accepts(10,fresh));
    let payload=InputEvent::Composition(CompositionEvent{text:"nihao".into(),start:0,end:5});
    assert!(binding.admit(10,old,&payload).is_none());
    assert!(binding.admit(10,fresh,&payload).is_some());
}

#[test]
fn coordinated_focus_transfer_preserves_both_objects_on_rejection() {
    use jasper_ui_engine::{keyboard::KeyboardRouter,scene::{Node,Scene},semantics::{Role,Semantics,SemanticsTree}};
    let scene=Scene::new(Node::container(NodeId(1))
        .child(Node::rect(NodeId(30)).with_semantics(Semantics::default().role(Role::TextField).focusable(true)))
        .child(Node::rect(NodeId(31)).with_semantics(Semantics::default().role(Role::TextField).focusable(true))));
    let tree=SemanticsTree::from_scene(&scene);
    let mut router=KeyboardRouter::new(&tree);router.focus(NodeId(30)).unwrap();
    let mut binding=UiInputBinding::new(10,20);
    let current=UiInputContext{target:NodeId(30),target_generation:1,document:40,revision:50};
    let old=binding.bind(10,current).unwrap();
    let mut next=current;next.target=NodeId(999);
    assert!(router.transfer_input_focus(&mut binding,10,old,next).is_none());
    assert_eq!(router.focused(),Some(NodeId(30)));assert!(binding.accepts(10,old));
    next.target=NodeId(31);next.document=41;next.revision=1;
    assert!(router.transfer_input_focus(&mut binding,11,old,next).is_none());
    assert_eq!(router.focused(),Some(NodeId(30)));assert!(binding.accepts(10,old));
    let fresh=router.transfer_input_focus(&mut binding,10,old,next).unwrap();
    assert_eq!(router.focused(),Some(NodeId(31)));assert!(!binding.accepts(10,old));
    let text=InputEvent::Text("新文档".into());
    {let admitted=binding.admit(10,fresh,&text).unwrap();assert!(std::ptr::eq(router.route_admitted(&admitted).unwrap(),&text));}
    assert!(router.transfer_input_focus(&mut binding,10,old,current).is_none());
    assert_eq!(router.focused(),Some(NodeId(31)));assert!(binding.accepts(10,fresh));
}

#[test]
fn initial_focus_transfer_close_reopen_keeps_old_owner_qualified() {
    use jasper_ui_engine::{keyboard::KeyboardRouter,scene::{Node,Scene},semantics::{Role,Semantics,SemanticsTree}};
    let scene=Scene::new(Node::container(NodeId(1))
        .child(Node::rect(NodeId(30)).with_semantics(Semantics::default().role(Role::TextField).focusable(true)))
        .child(Node::rect(NodeId(31)).with_semantics(Semantics::default().role(Role::TextField).focusable(true))));
    let tree=SemanticsTree::from_scene(&scene);let mut router=KeyboardRouter::new(&tree);
    let mut binding=UiInputBinding::new(10,20);
    let context=UiInputContext{target:NodeId(30),target_generation:1,document:40,revision:50};
    let mut invalid=context;invalid.target=NodeId(999);
    assert!(router.begin_input_focus(&mut binding,10,invalid).is_none());assert_eq!(router.focused(),None);
    assert!(router.begin_input_focus(&mut binding,11,context).is_none());assert_eq!(router.focused(),None);
    let first=router.begin_input_focus(&mut binding,10,context).unwrap();
    assert!(router.begin_input_focus(&mut binding,10,context).is_none());assert!(binding.accepts(10,first));
    let mut next=context;next.target=NodeId(31);next.document=41;next.revision=1;
    let second=router.transfer_input_focus(&mut binding,10,first,next).unwrap();
    let owner=router.close_input_focus(&mut binding,10,second).unwrap();
    assert_eq!(owner.key,second.key);assert_eq!(owner.context.target,NodeId(31));assert_eq!(owner.context.document,41);
    assert_eq!(router.focused(),None);
    let mut reused=context;reused.target_generation=2;
    let third=router.begin_input_focus(&mut binding,10,reused).unwrap();
    assert!(router.close_input_focus(&mut binding,10,owner).is_none());assert_eq!(router.focused(),Some(NodeId(30)));
    let payload=InputEvent::Composition(CompositionEvent{text:"nihao".into(),start:0,end:5});
    assert!(binding.admit(10,owner,&payload).is_none());
    let admitted=binding.admit(10,third,&payload).unwrap();
    assert!(std::ptr::eq(router.route_admitted(&admitted).unwrap(),&payload));
}

#[test]
fn composition_view_rejects_late_updates_and_old_cancellation_with_same_focus_owner() {
    use jasper_ui_engine::{composition_identity::UiCompositionSlot,keyboard::KeyboardRouter,scene::{Node,Scene},semantics::{Role,Semantics,SemanticsTree}};
    let scene=Scene::new(Node::rect(NodeId(30)).with_semantics(Semantics::default().role(Role::TextField).focusable(true)));
    let tree=SemanticsTree::from_scene(&scene);let mut router=KeyboardRouter::new(&tree);
    let mut binding=UiInputBinding::new(10,20);
    let context=UiInputContext{target:NodeId(30),target_generation:1,document:40,revision:50};
    let owner=router.begin_input_focus(&mut binding,10,context).unwrap();
    let mut slot=UiCompositionSlot::new();let first=slot.begin(&binding,10,owner).unwrap();
    let payload=InputEvent::Composition(CompositionEvent{text:"nihao".into(),start:0,end:5});
    assert!(slot.accepts(&binding,10,first));
    {let admitted=binding.admit(10,first.owner,&payload).unwrap();assert!(router.route_admitted(&admitted).is_some());}
    assert!(slot.cancel(10,first));assert!(!slot.accepts(&binding,10,first));
    let second=slot.begin(&binding,10,owner).unwrap();assert!(second.serial>first.serial);
    assert!(!slot.cancel(10,first));assert!(slot.accepts(&binding,10,second));
    let old_owner=router.close_input_focus(&mut binding,10,owner).unwrap();assert_eq!(old_owner.key,second.owner.key);
    assert!(!slot.accepts(&binding,10,second));
    assert!(slot.cancel(10,second));
    let new_owner=router.begin_input_focus(&mut binding,10,context).unwrap();
    let third=slot.begin(&binding,10,new_owner).unwrap();
    assert!(!slot.accepts(&binding,10,second));assert!(!slot.cancel(10,second));assert!(slot.accepts(&binding,10,third));
    let cancellation=slot.retire().unwrap();assert_eq!(cancellation.serial,third.serial);assert_eq!(cancellation.owner.key,new_owner.key);
    assert!(!slot.accepts(&binding,10,third));assert!(slot.begin(&binding,10,new_owner).is_none());
}

#[test]
fn composition_replacement_range_is_current_document_utf8_boundary() {
    use jasper_ui_engine::{composition_identity::UiCompositionSlot,keyboard::KeyboardRouter,scene::{Node,Scene},semantics::{Role,Semantics,SemanticsTree}};
    let scene=Scene::new(Node::rect(NodeId(30)).with_semantics(Semantics::default().role(Role::TextField).focusable(true)));
    let tree=SemanticsTree::from_scene(&scene);let mut router=KeyboardRouter::new(&tree);
    let mut binding=UiInputBinding::new(10,20);
    let owner=router.begin_input_focus(&mut binding,10,UiInputContext{target:NodeId(30),target_generation:1,document:40,revision:50}).unwrap();
    let mut slot=UiCompositionSlot::new();let token=slot.begin(&binding,10,owner).unwrap();
    let document="你a🦀";
    let good=CompositionEvent{text:"新".into(),start:3,end:4};
    let bad=CompositionEvent{text:"新".into(),start:1,end:3};
    assert!(slot.admit_composition(&binding,10,token,document,&bad).is_none());
    {
        let checked=slot.admit_composition(&binding,10,token,document,&good).unwrap();
        assert!(std::ptr::eq(checked,&good));
        let admitted=binding.admit(10,owner,checked).unwrap();
        assert!(std::ptr::eq(router.route_admitted(&admitted).unwrap(),&good));
    }
    assert!(slot.cancel(10,token));let next=slot.begin(&binding,10,owner).unwrap();
    assert!(slot.admit_composition(&binding,10,token,document,&good).is_none());
    assert!(slot.admit_composition(&binding,10,next,document,&good).is_some());
    router.close_input_focus(&mut binding,10,owner).unwrap();
    assert!(slot.admit_composition(&binding,10,next,document,&good).is_none());
    assert_eq!(document,"你a🦀");
}

#[test]
fn preedit_cursor_is_local_and_none_remains_absent() {
    use jasper_ui_engine::{input::PreeditEvent,composition_identity::UiCompositionSlot,keyboard::KeyboardRouter,scene::{Node,Scene},semantics::{Role,Semantics,SemanticsTree}};
    let scene=Scene::new(Node::rect(NodeId(30)).with_semantics(Semantics::default().role(Role::TextField).focusable(true)));
    let tree=SemanticsTree::from_scene(&scene);let mut router=KeyboardRouter::new(&tree);
    let mut binding=UiInputBinding::new(10,20);
    let owner=router.begin_input_focus(&mut binding,10,UiInputContext{target:NodeId(30),target_generation:1,document:40,revision:50}).unwrap();
    let mut slot=UiCompositionSlot::new();let token=slot.begin(&binding,10,owner).unwrap();
    // The editor may be empty while a nonempty preedit-local selection is valid.
    // 编辑器可为空，预编辑局部选区仍可有效；不把预编辑位置写作文档偏移。
    let document="";let event=PreeditEvent{text:"你好".into(),cursor:Some((0,6))};
    {let checked=slot.admit_preedit(&binding,10,token,&event).unwrap();
     let admitted=binding.admit(10,owner,checked).unwrap();
     assert!(std::ptr::eq(router.route_admitted(&admitted).unwrap(),&event));}
    let absent=PreeditEvent{text:"你好".into(),cursor:None};
    assert_eq!(slot.admit_preedit(&binding,10,token,&absent).unwrap().cursor,None);
    let invalid=PreeditEvent{text:"你好".into(),cursor:Some((1,3))};
    assert!(slot.admit_preedit(&binding,10,token,&invalid).is_none());
    router.close_input_focus(&mut binding,10,owner).unwrap();
    assert!(slot.admit_preedit(&binding,10,token,&event).is_none());
    assert_eq!(document,"");
}

#[cfg(feature="desktop")]
#[test]
fn actual_winit_preedit_normalizes_into_current_ui_composition() {
    use jasper_ui_engine::{desktop::normalize_ime,composition_identity::UiCompositionSlot,keyboard::KeyboardRouter,scene::{Node,Scene},semantics::{Role,Semantics,SemanticsTree}};
    use winit::event::Ime;
    let scene=Scene::new(Node::rect(NodeId(30)).with_semantics(Semantics::default().role(Role::TextField).focusable(true)));
    let tree=SemanticsTree::from_scene(&scene);let mut router=KeyboardRouter::new(&tree);
    let mut binding=UiInputBinding::new(10,20);
    let owner=router.begin_input_focus(&mut binding,10,UiInputContext{target:NodeId(30),target_generation:1,document:40,revision:50}).unwrap();
    let mut slot=UiCompositionSlot::new();let first=slot.begin(&binding,10,owner).unwrap();
    for cursor in [None,Some((0,6))] {
        let event=normalize_ime(&Ime::Preedit("你好".into(),cursor)).unwrap();
        let checked=slot.admit_normalized_preedit(&binding,10,first,&event).unwrap();
        let admitted=binding.admit(10,owner,checked).unwrap();
        assert!(std::ptr::eq(router.route_admitted(&admitted).unwrap(),&event));
        assert!(matches!(event,InputEvent::Preedit{cursor:position,..} if position==cursor));
    }
    let invalid=normalize_ime(&Ime::Preedit("你好".into(),Some((1,3)))).unwrap();
    assert!(slot.admit_normalized_preedit(&binding,10,first,&invalid).is_none());
    let commit=normalize_ime(&Ime::Commit("你好".into())).unwrap();
    assert!(slot.admit_normalized_preedit(&binding,10,first,&commit).is_none());
    let clear=normalize_ime(&Ime::Preedit("".into(),None)).unwrap();
    assert!(slot.admit_normalized_preedit(&binding,10,first,&clear).is_some());
    assert!(slot.cancel(10,first));let second=slot.begin(&binding,10,owner).unwrap();
    assert!(slot.admit_normalized_preedit(&binding,10,first,&clear).is_none());
    assert!(slot.admit_normalized_preedit(&binding,10,second,&clear).is_some());
    router.close_input_focus(&mut binding,10,owner).unwrap();
    assert!(slot.admit_normalized_preedit(&binding,10,second,&clear).is_none());
}

#[cfg(feature="desktop")]
#[test]
fn owned_preedit_presentation_changes_and_clears_only_for_exact_owner() {
    use jasper_ui_engine::{desktop::normalize_ime,composition_identity::{UiCompositionSlot,UiPreeditView},keyboard::KeyboardRouter,scene::{Node,Scene},semantics::{Role,Semantics,SemanticsTree}};
    use winit::event::Ime;
    let scene=Scene::new(Node::rect(NodeId(30)).with_semantics(Semantics::default().role(Role::TextField).focusable(true)));
    let tree=SemanticsTree::from_scene(&scene);let mut router=KeyboardRouter::new(&tree);
    let mut binding=UiInputBinding::new(10,20);
    let owner=router.begin_input_focus(&mut binding,10,UiInputContext{target:NodeId(30),target_generation:1,document:40,revision:50}).unwrap();
    let mut slot=UiCompositionSlot::new();let first=slot.begin(&binding,10,owner).unwrap();
    let mut view=UiPreeditView::new();
    let event=normalize_ime(&Ime::Preedit("你好".into(),Some((0,6)))).unwrap();
    assert!(view.apply(&slot,&binding,10,first,event));assert_eq!(view.current().unwrap().text,"你好");
    let invalid=normalize_ime(&Ime::Preedit("破坏".into(),Some((1,3)))).unwrap();
    assert!(!view.apply(&slot,&binding,10,first,invalid));assert_eq!(view.current().unwrap().text,"你好");
    assert!(slot.cancel(10,first));let second=slot.begin(&binding,10,owner).unwrap();
    let event=normalize_ime(&Ime::Preedit("后继".into(),None)).unwrap();
    assert!(view.apply(&slot,&binding,10,second,event));
    assert!(!view.clear(10,first));assert_eq!(view.current().unwrap().text,"后继");
    let mut foreign=second;foreign.owner.context.document^=1;
    assert!(!view.clear(10,foreign));assert!(!view.clear(11,second));
    router.close_input_focus(&mut binding,10,owner).unwrap();
    assert!(slot.cancel(10,second));assert!(view.clear(10,second));assert!(view.current().is_none());
    assert!(!view.clear(10,second));
}

#[cfg(feature="desktop")]
#[test]
fn cancellation_pairs_slot_and_owned_text_without_clearing_a_successor() {
    use jasper_ui_engine::{desktop::normalize_ime,composition_identity::{UiCompositionSlot,UiPreeditView},keyboard::KeyboardRouter,scene::{Node,Scene},semantics::{Role,Semantics,SemanticsTree}};
    use winit::event::Ime;
    let scene=Scene::new(Node::rect(NodeId(30)).with_semantics(Semantics::default().role(Role::TextField).focusable(true)));
    let tree=SemanticsTree::from_scene(&scene);let mut router=KeyboardRouter::new(&tree);
    let mut binding=UiInputBinding::new(10,20);
    let owner=router.begin_input_focus(&mut binding,10,UiInputContext{target:NodeId(30),target_generation:1,document:40,revision:50}).unwrap();
    let mut slot=UiCompositionSlot::new();let first=slot.begin(&binding,10,owner).unwrap();
    let mut view=UiPreeditView::new();
    assert!(view.apply(&slot,&binding,10,first,normalize_ime(&Ime::Preedit("旧组合".into(),None)).unwrap()));
    assert!(!slot.cancel_preedit(&mut view,11,first));
    assert!(slot.accepts(&binding,10,first));assert_eq!(view.current().unwrap().text,"旧组合");
    assert!(slot.cancel_preedit(&mut view,10,first));assert!(view.current().is_none());
    assert!(!slot.accepts(&binding,10,first));
    let second=slot.begin(&binding,10,owner).unwrap();
    assert!(view.apply(&slot,&binding,10,second,normalize_ime(&Ime::Preedit("后继组合".into(),None)).unwrap()));
    assert!(!slot.cancel_preedit(&mut view,10,first));
    assert!(slot.accepts(&binding,10,second));assert_eq!(view.current().unwrap().text,"后继组合");
    router.close_input_focus(&mut binding,10,owner).unwrap();
    assert!(slot.cancel_preedit(&mut view,10,second));assert!(view.current().is_none());
    assert!(!slot.cancel_preedit(&mut view,10,second));
}

#[cfg(feature="desktop")]
#[test]
fn focus_close_clears_matching_preedit_and_rejects_old_close_after_reopen() {
    use jasper_ui_engine::{desktop::normalize_ime,composition_identity::{UiCompositionSlot,UiPreeditView},keyboard::KeyboardRouter,scene::{Node,Scene},semantics::{Role,Semantics,SemanticsTree}};
    use winit::event::Ime;
    let scene=Scene::new(Node::container(NodeId(1))
        .child(Node::rect(NodeId(30)).with_semantics(Semantics::default().role(Role::TextField).focusable(true)))
        .child(Node::rect(NodeId(31)).with_semantics(Semantics::default().role(Role::TextField).focusable(true))));
    let tree=SemanticsTree::from_scene(&scene);let mut router=KeyboardRouter::new(&tree);
    let mut binding=UiInputBinding::new(10,20);
    let context=UiInputContext{target:NodeId(30),target_generation:1,document:40,revision:50};
    let owner=router.begin_input_focus(&mut binding,10,context).unwrap();
    let mut slot=UiCompositionSlot::new();let first=slot.begin(&binding,10,owner).unwrap();let mut view=UiPreeditView::new();
    assert!(view.apply(&slot,&binding,10,first,normalize_ime(&Ime::Preedit("你好".into(),None)).unwrap()));
    router.focus(NodeId(31)).unwrap();
    assert!(router.close_input_preedit(&mut binding,&mut slot,&mut view,10,first).is_none());
    assert_eq!(router.focused(),Some(NodeId(31)));assert!(binding.accepts(10,owner));assert!(slot.accepts(&binding,10,first));assert_eq!(view.current().unwrap().text,"你好");
    router.focus(NodeId(30)).unwrap();
    let cancellation=router.close_input_preedit(&mut binding,&mut slot,&mut view,10,first).unwrap();
    assert_eq!(cancellation.serial,first.serial);assert_eq!(cancellation.owner.key,owner.key);
    assert_eq!(router.focused(),None);assert!(!binding.accepts(10,owner));assert!(!slot.accepts(&binding,10,first));assert!(view.current().is_none());
    let next=router.begin_input_focus(&mut binding,10,context).unwrap();let second=slot.begin(&binding,10,next).unwrap();
    assert!(view.apply(&slot,&binding,10,second,normalize_ime(&Ime::Preedit("后继".into(),None)).unwrap()));
    assert!(router.close_input_preedit(&mut binding,&mut slot,&mut view,10,first).is_none());
    assert_eq!(router.focused(),Some(NodeId(30)));assert!(binding.accepts(10,next));assert!(slot.accepts(&binding,10,second));assert_eq!(view.current().unwrap().text,"后继");
}

#[cfg(feature="desktop")]
#[test]
fn real_host_ime_disabled_preserves_typing_focus_but_focus_loss_closes_it() {
    use jasper_ui_engine::{desktop::{DesktopInput,normalize_ime},composition_identity::{UiCompositionSlot,UiPreeditView},keyboard::KeyboardRouter,scene::{Node,Scene},semantics::{Role,Semantics,SemanticsTree}};
    use winit::event::{Ime,WindowEvent};
    let scene=Scene::new(Node::rect(NodeId(30)).with_semantics(Semantics::default().role(Role::TextField).focusable(true)));
    let tree=SemanticsTree::from_scene(&scene);let mut router=KeyboardRouter::new(&tree);let mut host=DesktopInput::new();
    let mut binding=UiInputBinding::new(10,20);
    let owner=router.begin_input_focus(&mut binding,10,UiInputContext{target:NodeId(30),target_generation:1,document:40,revision:50}).unwrap();
    let mut slot=UiCompositionSlot::new();let first=slot.begin(&binding,10,owner).unwrap();let mut view=UiPreeditView::new();
    assert!(view.apply(&slot,&binding,10,first,normalize_ime(&Ime::Preedit("你好".into(),None)).unwrap()));
    let disabled=host.translate(&WindowEvent::Ime(Ime::Disabled)).pop().unwrap();
    assert!(router.handle_preedit_lifecycle(&mut binding,&mut slot,&mut view,11,first,&disabled).is_none());assert!(slot.accepts(&binding,10,first));
    assert!(router.handle_preedit_lifecycle(&mut binding,&mut slot,&mut view,10,first,&disabled).is_some());
    assert_eq!(router.focused(),Some(NodeId(30)));assert!(binding.accepts(10,owner));assert!(view.current().is_none());
    let second=slot.begin(&binding,10,owner).unwrap();
    assert!(view.apply(&slot,&binding,10,second,normalize_ime(&Ime::Preedit("后继".into(),None)).unwrap()));
    assert!(router.handle_preedit_lifecycle(&mut binding,&mut slot,&mut view,10,first,&disabled).is_none());assert_eq!(view.current().unwrap().text,"后继");
    let enabled=host.translate(&WindowEvent::Ime(Ime::Enabled)).pop().unwrap();
    assert!(router.handle_preedit_lifecycle(&mut binding,&mut slot,&mut view,10,second,&enabled).is_none());assert!(slot.accepts(&binding,10,second));
    let lost=host.translate(&WindowEvent::Focused(false)).pop().unwrap();
    assert!(router.handle_preedit_lifecycle(&mut binding,&mut slot,&mut view,10,second,&lost).is_some());
    assert_eq!(router.focused(),None);assert!(!binding.accepts(10,owner));assert!(!slot.accepts(&binding,10,second));assert!(view.current().is_none());
}

#[cfg(feature="desktop")]
#[test]
fn unified_host_lifecycle_handles_idle_focus_and_composition_with_stale_owner_rejection() {
    use jasper_ui_engine::{desktop::{DesktopInput,normalize_ime},composition_identity::{UiCompositionSlot,UiPreeditView},keyboard::KeyboardRouter,scene::{Node,Scene},semantics::{Role,Semantics,SemanticsTree}};
    use winit::event::{Ime,WindowEvent};
    let scene=Scene::new(Node::rect(NodeId(30)).with_semantics(Semantics::default().role(Role::TextField).focusable(true)));
    let tree=SemanticsTree::from_scene(&scene);let mut router=KeyboardRouter::new(&tree);let mut host=DesktopInput::new();
    let mut binding=UiInputBinding::new(10,20);let mut slot=UiCompositionSlot::new();let mut view=UiPreeditView::new();
    let context=UiInputContext{target:NodeId(30),target_generation:1,document:40,revision:50};
    let old=router.begin_input_focus(&mut binding,10,context).unwrap();
    let lost=host.translate(&WindowEvent::Focused(false)).pop().unwrap();
    let closed=router.handle_input_lifecycle(&mut binding,&mut slot,&mut view,10,old,&lost).unwrap();
    assert_eq!(closed.0.key,old.key);assert!(closed.1.is_none());assert_eq!(router.focused(),None);assert!(!binding.accepts(10,old));
    let owner=router.begin_input_focus(&mut binding,10,context).unwrap();let token=slot.begin(&binding,10,owner).unwrap();
    assert!(view.apply(&slot,&binding,10,token,normalize_ime(&Ime::Preedit("你好".into(),None)).unwrap()));
    assert!(router.handle_input_lifecycle(&mut binding,&mut slot,&mut view,10,old,&lost).is_none());
    assert!(slot.accepts(&binding,10,token));assert_eq!(view.current().unwrap().text,"你好");
    let disabled=host.translate(&WindowEvent::Ime(Ime::Disabled)).pop().unwrap();
    let cancelled=router.handle_input_lifecycle(&mut binding,&mut slot,&mut view,10,owner,&disabled).unwrap();
    assert_eq!(cancelled.1.unwrap().serial,token.serial);assert!(binding.accepts(10,owner));assert_eq!(router.focused(),Some(NodeId(30)));assert!(view.current().is_none());
    let closed=router.handle_input_lifecycle(&mut binding,&mut slot,&mut view,10,owner,&lost).unwrap();
    assert!(closed.1.is_none());assert_eq!(router.focused(),None);assert!(!binding.accepts(10,owner));
}
