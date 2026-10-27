use jasper_ui_blitz_host_probe::host::SceneHostView;
use jasper_ui_blitz_host_probe::{Scene,Node,NodeId,ScenePatch,Semantics,Role};
#[test]
fn embedding_host_uses_only_typed_jasper_contract() {
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
    let button=Node::text(NodeId(2),"Save").with_semantics(Semantics::default().role(Role::Button));
    let mut source=Scene::new(Node::container(NodeId(1)).child(button));
    let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();
    let old=source.version();let patches=[ScenePatch::UpdateText {id:NodeId(2),text:"Saved".into()}];
    source.apply_batch_at(old,&patches).unwrap();assert!(view.apply(10,old,source.version(),&patches));
    assert!(view.semantics(10,old).is_none());assert_eq!(view.semantics(10,source.version()).unwrap().root,NodeId(1));
    let mut action=None;assert!(view.activate(10,source.version(),NodeId(2),&mut |event|action=Some(event.node)));
    assert_eq!(action,Some(NodeId(2)));
    #[cfg(feature="cpu-pending")]
    {let mut surface=vec![0;160*80*4];assert!(view.render_to_surface(10,source.version(),&mut surface,160*4,1024*1024));}
    assert!(view.close(10));assert!(view.semantics(10,source.version()).is_none());
}

#[test]
fn embedding_host_observes_focus_retirement_through_public_values() {
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
    let make=|id|Node::text(NodeId(id),"Action").with_semantics(Semantics::default().role(Role::Button).focusable(true));
    let mut source=Scene::new(Node::container(NodeId(1)).child(make(2)));
    let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();let version=source.version();
    let mut events=Vec::new();assert!(view.focus(10,version,NodeId(2),&mut |e|events.push(e)));
    assert_eq!(events.len(),1);assert!(events[0].focused);events.clear();
    assert!(!view.clear_focus(11,version,&mut |e|events.push(e)));
    assert!(!view.clear_focus(10,version+1,&mut |e|events.push(e)));assert!(events.is_empty());
    let patches=[ScenePatch::Remove {id:NodeId(2)},ScenePatch::Insert {parent:NodeId(1),index:0,node:make(3)}];
    source.apply_batch_at(version,&patches).unwrap();
    assert!(view.apply_with_focus_events(10,version,source.version(),&patches,&mut |e|events.push(e)));
    assert_eq!(events.len(),1);assert_eq!(events[0].node,NodeId(2));assert!(!events[0].focused);
    assert_eq!(events[0].scene_version,source.version());events.clear();
    assert!(view.focus(10,source.version(),NodeId(3),&mut |e|events.push(e)));events.clear();
    assert!(view.close_with_focus_events(10,source.version(),&mut |e|events.push(e)));
    assert_eq!(events.len(),1);assert_eq!(events[0].node,NodeId(3));assert!(!events[0].focused);events.clear();
    assert!(!view.close_with_focus_events(10,source.version(),&mut |e|events.push(e)));assert!(events.is_empty());
}

#[test]
fn embedding_host_routes_pointer_events_without_backend_values() {
    use jasper_ui_blitz_host_probe::{PointerAction,SceneEventKind,EventReply};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
    let button=Node::text(NodeId(2),"Save").with_semantics(Semantics::default().role(Role::Button));
    let source=Scene::new(Node::container(NodeId(1)).child(button));let version=source.version();
    let mut view=SceneHostView::new(10,&source,&font,320,200).unwrap();
    for (owner,v,x) in [(11,version,12.0),(10,version+1,12.0),(10,version,f32::NAN)] {
        assert!(!view.pointer(owner,v,PointerAction::Down,x,12.0,true,&mut |_|panic!("rejected pointer delivered"),&mut |_|panic!("rejected focus delivered")));
    }
    let mut events=Vec::new();let mut focus=Vec::new();
    for (phase,pressed) in [(PointerAction::Down,true),(PointerAction::Up,false)] {
        assert!(view.pointer(10,version,phase,12.0,12.0,pressed,&mut |e| {events.push(e);EventReply::default()},&mut |e|focus.push(e)));
    }
    assert!(events.iter().any(|e|e.kind==SceneEventKind::Pointer(PointerAction::Down) && e.node==NodeId(2)));
    assert!(events.iter().any(|e|e.kind==SceneEventKind::Activate && e.node==NodeId(2)));
    assert!(events.iter().all(|e|e.scene_version==version));
    assert!(view.close(10));
    assert!(!view.pointer(10,version,PointerAction::Down,12.0,12.0,true,&mut |_|panic!("retired"),&mut |_|panic!("retired")));
}

#[test]
fn external_text_service_client_rejects_old_snapshot_and_preedit_identity() {
    use jasper_ui_blitz_host_probe::host::{TextDisplayView,UiInputBinding,UiInputContext};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
    let first=UiInputContext {target:NodeId(7),target_generation:1,document:20,revision:1};
    let next=UiInputContext {revision:2,..first};let mut binding=UiInputBinding::new(10,1);
    let ticket=binding.bind(10,first).unwrap();
    let mut display=TextDisplayView::new(10,first,&font,"External A😀B",160,80);
    let token=display.begin_preedit(&binding,10,ticket).unwrap();assert!(display.preedit(&binding,10,token,Some("暂存")));
    assert!(!display.snapshot(11,first,next,"Foreign"));
    assert!(display.snapshot(10,first,next,"Service successor 中文"));
    assert!(!display.preedit(&binding,10,token,Some("Stale")));
    assert!(!display.snapshot(10,first,next,"Old source"));
    assert!(binding.accepts(10,ticket)); // Display replacement does not acknowledge host lease cancellation.
    assert!(binding.close(10,ticket));
    let fresh=binding.bind(10,next).unwrap();let active=display.begin_preedit(&binding,10,fresh).unwrap();
    assert!(display.preedit(&binding,10,active,Some("新")));assert!(display.selection(&binding,10,fresh,Some((0,7))));
    assert!(display.cancel_preedit(10,active));assert!(!display.preedit(&binding,10,active,Some("Late")));
    #[cfg(feature="cpu-pending")]
    {assert!(display.render_cpu(11,next,1024*1024).is_none());assert!(display.render_cpu(10,first,1024*1024).is_none());assert_eq!(display.render_cpu(10,next,1024*1024).unwrap().len(),160*80*4);}
    assert!(!display.close(11,next));assert!(!display.close(10,first));assert!(display.close(10,next));
    assert!(!display.snapshot(10,next,next,"Retired"));
}

#[test]
fn external_text_client_routes_command_keys_and_closes_exact_focus_lease() {
    use jasper_ui_blitz_host_probe::{HostKeyInput,KeyLocation,Modifiers,EventReply};
    use jasper_ui_blitz_host_probe::host::{TextDisplayView,UiInputBinding,UiInputContext};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
    let context=UiInputContext {target:NodeId(7),target_generation:1,document:20,revision:1};
    let mut binding=UiInputBinding::new(10,1);let ticket=binding.bind(10,context).unwrap();
    let mut display=TextDisplayView::new(10,context,&font,"External document",160,80);
    let input=HostKeyInput {key:"ArrowLeft",code:"ArrowLeft",pressed:true,repeat:true,composing:true,
        modifiers:Modifiers {shift:true,control:true,alt:false,meta:false},location:KeyLocation::Left};
    assert!(!display.key(&binding,10,ticket,&input,&mut |_|panic!("unfocused")));
    let mut focus=Vec::new();assert!(display.focus(&binding,10,ticket,&mut |e|focus.push(e)));
    assert_eq!(focus.len(),1);assert!(focus[0].focused);assert!(binding.accepts(10,focus[0].owner));focus.clear();
    let mut keys=Vec::new();assert!(display.key(&binding,10,ticket,&input,&mut |e| {keys.push(e);EventReply {prevent_default:true,stop_propagation:true}}));
    assert_eq!(keys.len(),1);assert_eq!((keys[0].context.target.0,keys[0].context.target_generation,keys[0].context.document,keys[0].context.revision),(context.target.0,context.target_generation,context.document,context.revision));assert_eq!(keys[0].key,"ArrowLeft");
    assert!(keys[0].pressed && keys[0].repeat && keys[0].composing && keys[0].modifiers.control && keys[0].modifiers.shift);
    assert_eq!(keys[0].location,KeyLocation::Left);
    assert!(!display.close_focus(&mut binding,11,ticket,&mut |_|panic!("foreign")));assert!(binding.accepts(10,ticket));
    assert!(display.close_focus(&mut binding,10,ticket,&mut |e|focus.push(e)));
    assert!(!binding.accepts(10,ticket));assert_eq!(focus.len(),1);assert!(!focus[0].focused);
    assert!(!display.key(&binding,10,ticket,&input,&mut |_|panic!("closed lease")));
    assert!(!display.close_focus(&mut binding,10,ticket,&mut |_|panic!("duplicate close")));
}

#[test]
fn framework_typed_update_frame_has_no_signal_runtime_or_editor_mutation() {
    use jasper_ui_blitz_host_probe::{ComponentUpdate,ContentFlow,TextAppearance};
    use jasper_ui_blitz_host_probe::host::{TextDisplayView,UiInputContext};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
    let context=UiInputContext {target:NodeId(7),target_generation:1,document:20,revision:1};
    let mut display=TextDisplayView::new(10,context,&font,"Service document",160,80);
    let appearance=TextAppearance {font_size_px:24,padding_px:8,foreground:[20,40,180],background:[240,245,250]};
    let updates=[ComponentUpdate::Appearance(appearance),ComponentUpdate::Flow {direction:ContentFlow::Row,gap_px:4},ComponentUpdate::AccessibleLabel(Some("Service display"))];
    assert_eq!(display.update_version(10,context),Some(0));assert_eq!(display.update_version(11,context),None);
    #[cfg(feature="cpu-pending")]
    let before=display.render_cpu(10,context,1024*1024).unwrap();
    assert!(!display.update(11,context,0,&updates));assert!(display.update(10,context,0,&updates));
    assert_eq!(display.update_version(10,context),Some(1));assert!(!display.update(10,context,0,&updates));
    #[cfg(feature="cpu-pending")]
    let after=display.render_cpu(10,context,1024*1024).unwrap();
    #[cfg(feature="cpu-pending")]
    assert!(before!=after);
    assert!(!display.update(10,context,1,&[ComponentUpdate::ButtonCaption("Not an editor update")]));
    let invalid=[ComponentUpdate::AccessibleLabel(Some("Must not apply")),ComponentUpdate::Appearance(TextAppearance {font_size_px:0,..appearance})];
    assert!(!display.update(10,context,1,&invalid));assert_eq!(display.update_version(10,context),Some(1));
    #[cfg(feature="cpu-pending")]
    assert!(after==display.render_cpu(10,context,1024*1024).unwrap());
    assert!(display.close(10,context));assert_eq!(display.update_version(10,context),None);
}

#[test]
fn external_host_capture_ack_failure_retries_without_redelivering_up() {
    use jasper_ui_blitz_host_probe::{PointerAction,EventReply};
    use jasper_ui_blitz_host_probe::host::{TextDisplayView,TextPointerCapture,PointerCaptureHost,UiInputContext,UiInputTicket};
    #[derive(Default)] struct Host {captures:usize,releases:usize,fail:bool}
    impl PointerCaptureHost for Host {
        fn capture(&mut self,_:UiInputTicket)->bool {self.captures+=1;true}
        fn release(&mut self,_:UiInputTicket)->bool {self.releases+=1;!self.fail}
    }
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
    let context=UiInputContext {target:NodeId(7),target_generation:1,document:20,revision:1};
    let mut display=TextDisplayView::new(10,context,&font,"External document",320,200);
    let phases=std::cell::RefCell::new(Vec::new());let mut host=Host::default();
    let mut capture=TextPointerCapture::new(10,4,|event| {phases.borrow_mut().push(event.phase);EventReply::default()},|_|EventReply::default());
    assert!(capture.down(&mut display,&mut host,11,context,12.0,12.0).is_none());assert_eq!(host.captures,0);
    let ticket=capture.down(&mut display,&mut host,10,context,12.0,12.0).unwrap();assert_eq!(host.captures,1);
    assert!(capture.move_pointer(&mut display,10,ticket,-20.0,-20.0));host.fail=true;
    assert!(!capture.up(&mut display,&mut host,10,ticket,-20.0,-20.0));
    assert_eq!(phases.borrow().iter().filter(|phase|**phase==PointerAction::Up).count(),1);
    assert!(!capture.retry_release(&mut host,11));host.fail=false;assert!(capture.retry_release(&mut host,10));
    assert_eq!(host.releases,2);assert_eq!(phases.borrow().iter().filter(|phase|**phase==PointerAction::Up).count(),1);
    assert!(capture.retire(&mut display,&mut host,10,context));
    assert!(capture.down(&mut display,&mut host,10,context,12.0,12.0).is_none());assert_eq!(host.captures,1);
}

#[test]
fn external_text_host_resizes_without_recreating_input_binding() {
    use jasper_ui_blitz_host_probe::host::{TextDisplayView,UiInputBinding,UiInputContext};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
    let context=UiInputContext {target:NodeId(7),target_generation:1,document:20,revision:1};
    let stale=UiInputContext {revision:2,..context};let mut binding=UiInputBinding::new(10,1);let ticket=binding.bind(10,context).unwrap();
    let mut display=TextDisplayView::new(10,context,&font,"Injected host DPI 中文",160,80);
    assert!(!display.resize(11,context,320,160));assert!(!display.resize(10,stale,320,160));
    assert!(!display.set_scale(11,context,1.5));assert!(!display.set_scale(10,stale,1.5));
    assert!(!display.set_scale(10,context,f32::NAN));
    assert!(display.set_scale(10,context,1.5));assert!(display.resize(10,context,320,160));
    assert!(binding.accepts(10,ticket));let token=display.begin_preedit(&binding,10,ticket).unwrap();assert!(display.preedit(&binding,10,token,Some("新")));
    #[cfg(feature="cpu-pending")]
    assert_eq!(display.render_cpu(10,context,1024*1024).unwrap().len(),320*160*4);
    assert!(display.resize(10,context,0,0));
    #[cfg(feature="cpu-pending")]
    assert!(display.render_cpu(10,context,1024*1024).is_none());
    assert!(display.resize(10,context,160,80));assert!(display.set_scale(10,context,1.0));
    assert!(display.close(10,context));assert!(!display.resize(10,context,320,160));assert!(!display.set_scale(10,context,2.0));
}

#[test]
fn external_text_client_projects_utf8_hits_and_selection_after_scroll() {
    use jasper_ui_blitz_host_probe::host::{TextDisplayView,UiInputBinding,UiInputContext};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
    let context=UiInputContext {target:NodeId(7),target_generation:1,document:20,revision:1};
    let text=(0..80).map(|n|format!("段落{n} unique display words ")).collect::<String>();
    let mut binding=UiInputBinding::new(10,1);let ticket=binding.bind(10,context).unwrap();
    let mut display=TextDisplayView::new(10,context,&font,&text,320,200);assert!(display.set_scale(10,context,1.5));
    assert!(display.hit(&binding,11,ticket,30.0,20.0).is_none());assert!(display.hit(&binding,10,ticket,-1.0,20.0).is_none());
    let before=display.hit(&binding,10,ticket,30.0,20.0).unwrap();assert!(text.is_char_boundary(before));
    assert!(display.selection(&binding,10,ticket,Some((0,"段落".len()))));
    assert_eq!(display.selected_text(&binding,10,ticket).as_deref(),Some("段落"));assert!(display.selected_text(&binding,11,ticket).is_none());
    assert!(display.set_scroll(&binding,10,ticket,0.0,32.0));let after=display.hit(&binding,10,ticket,30.0,20.0).unwrap();
    assert!(after>before && text.is_char_boundary(after));assert!(!display.set_scroll(&binding,11,ticket,0.0,0.0));
    assert!(!display.set_scroll(&binding,10,ticket,0.0,f64::NAN));assert_eq!(display.hit(&binding,10,ticket,30.0,20.0),Some(after));
    assert!(display.set_scroll(&binding,10,ticket,0.0,0.0));assert_eq!(display.hit(&binding,10,ticket,30.0,20.0),Some(before));
    assert!(binding.close(10,ticket));assert!(display.hit(&binding,10,ticket,30.0,20.0).is_none());assert!(display.selected_text(&binding,10,ticket).is_none());
}

#[test]
fn external_text_host_reads_current_accessibility_semantics() {
    use jasper_ui_blitz_host_probe::{ComponentUpdate,TextSemanticRole};
    use jasper_ui_blitz_host_probe::host::{TextDisplayView,UiInputBinding,UiInputContext};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
    let context=UiInputContext {target:NodeId(7),target_generation:1,document:20,revision:1};
    let mut view=TextDisplayView::new(10,context,&font,"External 😀",160,80);
    assert!(view.semantics(11,context).is_none());
    assert!(view.semantics(10,UiInputContext {target_generation:2,..context}).is_none());
    let initial=view.semantics(10,context).unwrap();
    assert_eq!(initial.role,TextSemanticRole::Text);assert_eq!(initial.name,"External 😀");
    assert!(!initial.disabled && !initial.native_focused && !initial.can_activate);
    assert!(view.update(10,context,initial.component_version,&[ComponentUpdate::AccessibleLabel(Some("Document"))]));
    let named=view.semantics(10,context).unwrap();assert_eq!(named.name,"Document");
    assert_eq!(named.component_version,initial.component_version+1);
    let mut binding=UiInputBinding::new(10,1);let ticket=binding.bind(10,context).unwrap();
    assert!(view.focus(&binding,10,ticket,&mut |_|{}));
    assert!(view.semantics(10,context).unwrap().native_focused);
    let next=UiInputContext {revision:2,..context};
    assert!(view.snapshot(10,context,next,"Successor"));
    assert!(view.semantics(10,context).is_none());
    assert_eq!(view.semantics(10,next).unwrap().context.revision,2);
    assert!(binding.accepts(10,ticket));
    assert!(view.close(10,next));assert!(view.semantics(10,next).is_none());
}

#[test]
fn scene_command_keys_follow_native_focus_and_source_version() {
    use jasper_ui_blitz_host_probe::{HostKeyInput,KeyLocation,Modifiers,EventReply};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
    let mut source=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Action")
        .with_semantics(Semantics::default().role(Role::Button).focusable(true))));
    let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();let version=source.version();
    let mut input=HostKeyInput {key:"ArrowLeft",code:"ArrowLeft",pressed:true,repeat:true,composing:true,
        modifiers:Modifiers {shift:true,control:true,alt:true,meta:true},location:KeyLocation::Right};
    assert!(!view.key(10,version,&input,&mut |_|panic!("unfocused"),&mut |_|panic!("focus")));
    assert!(view.focus(10,version,NodeId(2),&mut |_|{}));
    assert!(!view.key(11,version,&input,&mut |_|panic!("foreign"),&mut |_|panic!("focus")));
    assert!(!view.key(10,version+1,&input,&mut |_|panic!("stale"),&mut |_|panic!("focus")));
    for pressed in [true,false] {
        input.pressed=pressed;let mut seen=0;
        assert!(view.key(10,version,&input,&mut |event| {
            seen+=1;assert_eq!(event.node,NodeId(2));assert_eq!(event.scene_version,version);
            assert_eq!(event.key,"ArrowLeft");assert_eq!(event.code,"ArrowLeft");
            assert_eq!(event.pressed,pressed);assert!(event.repeat && event.composing);
            assert!(event.modifiers.shift && event.modifiers.control && event.modifiers.alt && event.modifiers.meta);
            assert_eq!(event.location,KeyLocation::Right);
            EventReply {prevent_default:true,stop_propagation:true}
        },&mut |_|panic!("prevented command changed focus")));assert_eq!(seen,1);
    }
    input.code="InvalidPhysicalCode";
    assert!(!view.key(10,version,&input,&mut |_|panic!("invalid"),&mut |_|panic!("focus")));
    input.code="ArrowLeft";
    let patches=[ScenePatch::Remove {id:NodeId(2)}];source.apply_batch_at(version,&patches).unwrap();
    assert!(view.apply_with_focus_events(10,version,source.version(),&patches,&mut |_|{}));
    assert!(!view.key(10,source.version(),&input,&mut |_|panic!("removed"),&mut |_|panic!("focus")));
    assert!(view.close(10));
    assert!(!view.key(10,source.version(),&input,&mut |_|panic!("retired"),&mut |_|panic!("focus")));
}

#[test]
fn scene_tab_default_reports_native_focus_and_can_be_prevented() {
    use jasper_ui_blitz_host_probe::{HostKeyInput,KeyLocation,Modifiers,EventReply};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
    let button=|id|Node::text(NodeId(id),"Focus").with_semantics(Semantics::default().role(Role::Button).focusable(true));
    let source=Scene::new(Node::container(NodeId(1)).child(button(2)).child(button(3)));
    let version=source.version();let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();
    assert!(view.focus(10,version,NodeId(2),&mut |_|{}));
    let mut input=HostKeyInput {key:"Tab",code:"Tab",pressed:true,repeat:false,composing:false,
        modifiers:Modifiers::default(),location:KeyLocation::Standard};
    let mut transitions=Vec::new();let mut routed=Vec::new();
    assert!(view.key(10,version,&input,&mut |event| {routed.push(event.node);EventReply::default()},
        &mut |event|transitions.push((event.node,event.focused))));
    assert_eq!(routed,vec![NodeId(2)]);
    assert_eq!(transitions,vec![(NodeId(2),false),(NodeId(3),true)]);
    transitions.clear();
    assert!(view.key(10,version,&input,&mut |event| {assert_eq!(event.node,NodeId(3));
        EventReply {prevent_default:true,stop_propagation:true}},&mut |event|transitions.push((event.node,event.focused))));
    assert!(transitions.is_empty());input.modifiers.shift=true;
    assert!(view.key(10,version,&input,&mut |_|EventReply::default(),&mut |event|transitions.push((event.node,event.focused))));
    assert_eq!(transitions,vec![(NodeId(3),false),(NodeId(2),true)]);
}

#[test]
fn scene_tab_skips_framework_hidden_ancestor() {
    use jasper_ui_blitz_host_probe::{HostKeyInput,KeyLocation,Modifiers,EventReply};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
    let button=|id|Node::text(NodeId(id),"Focus").with_semantics(Semantics::default().role(Role::Button).focusable(true));
    let hidden=Node::container(NodeId(4)).with_semantics(Semantics::default().hidden(true)).child(button(5));
    let mut source=Scene::new(Node::container(NodeId(1)).child(button(2)).child(hidden).child(button(3)));
    let version=source.version();let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();
    assert!(view.focus(10,version,NodeId(2),&mut |_|{}));
    let input=HostKeyInput {key:"Tab",code:"Tab",pressed:true,repeat:false,composing:false,
        modifiers:Modifiers::default(),location:KeyLocation::Standard};
    let mut transitions=Vec::new();
    assert!(view.key(10,version,&input,&mut |_|EventReply::default(),&mut |event|transitions.push((event.node,event.focused))));
    assert_eq!(transitions,vec![(NodeId(2),false),(NodeId(3),true)]);
    let patches=[ScenePatch::Insert {parent:NodeId(4),index:0,node:button(6)}];
    source.apply_batch_at(version,&patches).unwrap();
    assert!(view.apply(10,version,source.version(),&patches));
    assert!(view.focus(10,source.version(),NodeId(2),&mut |_|{}));transitions.clear();
    assert!(view.key(10,source.version(),&input,&mut |_|EventReply::default(),
        &mut |event|transitions.push((event.node,event.focused))));
    assert_eq!(transitions,vec![(NodeId(2),false),(NodeId(3),true)]);
    let metadata=view.semantics(10,source.version()).unwrap();
    assert!(metadata.nodes.iter().find(|node|node.node==NodeId(6)).unwrap().semantics.focusable);
    assert!(metadata.nodes.iter().find(|node|node.node==NodeId(4)).unwrap().semantics.hidden);

}

#[test]
fn scene_tab_tracks_framework_display_changes() {
    use jasper_ui_blitz_host_probe::{HostKeyInput,KeyLocation,Modifiers,EventReply,style::{Style,Display}};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
    let button=|id|Node::text(NodeId(id),"Focus").with_semantics(Semantics::default().role(Role::Button).focusable(true));
    let mut source=Scene::new(Node::container(NodeId(1)).child(button(2)).child(button(3)).child(button(4)));
    let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();
    let input=HostKeyInput {key:"Tab",code:"Tab",pressed:true,repeat:false,composing:false,
        modifiers:Modifiers::default(),location:KeyLocation::Standard};
    for (display,next_focus) in [(Display::None,NodeId(4)),(Display::Flex,NodeId(3))] {
        let old=source.version();let mut style=Style::default();style.display=display;
        let patches=[ScenePatch::UpdateStyle {id:NodeId(3),style}];source.apply_batch_at(old,&patches).unwrap();
        assert!(view.apply(10,old,source.version(),&patches));
        assert!(view.focus(10,source.version(),NodeId(2),&mut |_|{}));let mut transitions=Vec::new();
        assert!(view.key(10,source.version(),&input,&mut |_|EventReply::default(),
            &mut |event|transitions.push((event.node,event.focused))));
        assert_eq!(transitions,vec![(NodeId(2),false),(next_focus,true)]);
    }
}

#[test]
fn scene_tab_tracks_ancestor_display_changes() {
    use jasper_ui_blitz_host_probe::{HostKeyInput,KeyLocation,Modifiers,EventReply,style::{Style,Display}};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
    let button=|id|Node::text(NodeId(id),"Focus").with_semantics(Semantics::default().role(Role::Button).focusable(true));
    let mut source=Scene::new(Node::container(NodeId(1)).child(button(2)).child(Node::container(NodeId(3)).child(button(5))).child(button(4)));
    let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();
    let input=HostKeyInput {key:"Tab",code:"Tab",pressed:true,repeat:false,composing:false,
        modifiers:Modifiers::default(),location:KeyLocation::Standard};
    for (display,next_focus) in [(Display::None,NodeId(4)),(Display::Flex,NodeId(5))] {
        let old=source.version();let mut style=Style::default();style.display=display;
        let patches=[ScenePatch::UpdateStyle {id:NodeId(3),style}];source.apply_batch_at(old,&patches).unwrap();
        assert!(view.apply(10,old,source.version(),&patches));
        assert!(view.focus(10,source.version(),NodeId(2),&mut |_|{}));let mut transitions=Vec::new();
        assert!(view.key(10,source.version(),&input,&mut |_|EventReply::default(),
            &mut |event|transitions.push((event.node,event.focused))));
        assert_eq!(transitions,vec![(NodeId(2),false),(next_focus,true)]);
    }
}

#[test]
fn typed_scene_pointer_preserves_modifiers_and_rejects_invalid_sample() {
    use jasper_ui_blitz_host_probe::{ScenePointerInput,PointerAction,Modifiers,EventReply};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
    let source=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Action")
        .with_semantics(Semantics::default().role(Role::Button))));
    let version=source.version();let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();
    for flags in 0..16 {
        let modifiers=Modifiers {shift:flags&1!=0,control:flags&2!=0,alt:flags&4!=0,meta:flags&8!=0};
        let input=ScenePointerInput {phase:PointerAction::Move,position:(12.0,12.0),primary_pressed:false,modifiers};
        let mut observed=0;
        assert!(view.pointer_input(10,version,&input,&mut |event| {
            assert_eq!(event.node,NodeId(2));assert_eq!(event.position,(12.0,12.0));
            assert_eq!((event.modifiers.shift,event.modifiers.control,event.modifiers.alt,event.modifiers.meta),
                (modifiers.shift,modifiers.control,modifiers.alt,modifiers.meta));observed+=1;
            EventReply {prevent_default:true,stop_propagation:true}
        },&mut |_|panic!("prevented move changed focus")));assert_eq!(observed,1);
    }
    for (owner,v,x) in [(11,version,12.0),(10,version+1,12.0),(10,version,f32::NAN)] {
        let input=ScenePointerInput {phase:PointerAction::Down,position:(x,12.0),primary_pressed:true,modifiers:Modifiers::default()};
        assert!(!view.pointer_input(owner,v,&input,&mut |_|panic!("rejected pointer"),&mut |_|panic!("rejected focus")));
    }
}

#[test]
fn text_capture_retains_latest_modifiers_through_cancel_and_release_retry() {
    use jasper_ui_blitz_host_probe::{Modifiers,PointerAction,EventReply};
    use jasper_ui_blitz_host_probe::host::{TextDisplayView,TextPointerCapture,PointerCaptureHost,UiInputContext,UiInputTicket};
    use std::{rc::Rc,cell::RefCell};
    #[derive(Default)]struct Host {captures:usize,releases:usize,fail:bool}
    impl PointerCaptureHost for Host {
        fn capture(&mut self,_:UiInputTicket)->bool {self.captures+=1;true}
        fn release(&mut self,_:UiInputTicket)->bool {self.releases+=1;!self.fail}
    }
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
    let context=UiInputContext {target:NodeId(7),target_generation:1,document:20,revision:1};
    let mut view=TextDisplayView::new(10,context,&font,"External display",160,80);
    let events=Rc::new(RefCell::new(Vec::new()));let output=events.clone();let mut host=Host::default();
    let mut capture=TextPointerCapture::new(10,1,move |event| {
        output.borrow_mut().push((event.phase,(event.modifiers.shift,event.modifiers.control,event.modifiers.alt,event.modifiers.meta)));
        EventReply {prevent_default:true,stop_propagation:true}
    },|_|EventReply::default());
    let flags=|x|Modifiers {shift:x&1!=0,control:x&2!=0,alt:x&4!=0,meta:x&8!=0};
    let tuple=|m:Modifiers|(m.shift,m.control,m.alt,m.meta);
    for mask in 0..16 {
        events.borrow_mut().clear();let first=flags(mask);let latest=flags(mask^15);
        let ticket=capture.down_with_modifiers(&mut view,&mut host,10,context,12.0,12.0,first).unwrap();
        assert!(capture.move_with_modifiers(&mut view,10,ticket,-20.0,-20.0,latest));
        assert!(!capture.move_with_modifiers(&mut view,10,ticket,f32::NAN,0.0,Modifiers::default()));
        host.fail=true;assert!(!capture.cancel(&mut view,&mut host,10,ticket));
        assert_eq!(*events.borrow(),vec![(PointerAction::Down,tuple(first)),(PointerAction::Move,tuple(latest)),(PointerAction::Cancel,tuple(latest))]);
        host.fail=false;assert!(capture.retry_release(&mut host,10));assert_eq!(events.borrow().len(),3);
        let ticket=capture.down_with_modifiers(&mut view,&mut host,10,context,12.0,12.0,first).unwrap();
        assert!(capture.up_with_modifiers(&mut view,&mut host,10,ticket,12.0,12.0,latest));
        assert_eq!(events.borrow().last(),Some(&(PointerAction::Up,tuple(latest))));
    }
    let count=host.captures;
    assert!(capture.down_with_modifiers(&mut view,&mut host,11,context,12.0,12.0,flags(15)).is_none());
    assert_eq!(host.captures,count);
}

#[test]
fn capture_cancel_rejects_another_view_without_losing_original_grant() {
    use jasper_ui_blitz_host_probe::{EventReply,PointerAction};
    use jasper_ui_blitz_host_probe::host::{TextDisplayView,TextPointerCapture,PointerCaptureHost,UiInputContext,UiInputTicket};
    use std::{rc::Rc,cell::RefCell};
    #[derive(Default)]struct Host {releases:usize,fail:bool}
    impl PointerCaptureHost for Host {
        fn capture(&mut self,_:UiInputTicket)->bool {true}
        fn release(&mut self,_:UiInputTicket)->bool {self.releases+=1;!self.fail}
    }
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
    let context=UiInputContext {target:NodeId(7),target_generation:1,document:20,revision:1};
    let mut original=TextDisplayView::new(10,context,&font,"Original",160,80);
    let mut other=TextDisplayView::new(10,UiInputContext {target:NodeId(8),..context},&font,"Other",160,80);
    let events=Rc::new(RefCell::new(Vec::new()));let output=events.clone();let mut host=Host::default();
    let mut capture=TextPointerCapture::new(10,1,move |event| {output.borrow_mut().push(event.phase);EventReply::default()},|_|EventReply::default());
    let ticket=capture.down(&mut original,&mut host,10,context,12.0,12.0).unwrap();
    assert!(!capture.cancel(&mut other,&mut host,10,ticket));assert_eq!(host.releases,0);
    assert!(capture.move_pointer(&mut original,10,ticket,24.0,12.0));
    let next=UiInputContext {revision:2,..context};assert!(original.snapshot(10,context,next,"Successor"));
    assert!(capture.cancel(&mut original,&mut host,10,ticket));assert_eq!(host.releases,1);
    assert_eq!(events.borrow().iter().filter(|phase|**phase==PointerAction::Cancel).count(),0);
    let fresh=capture.down(&mut original,&mut host,10,next,12.0,12.0).unwrap();
    host.fail=true;assert!(!capture.cancel(&mut original,&mut host,10,fresh));let pending=host.releases;
    assert!(!capture.up(&mut other,&mut host,10,fresh,12.0,12.0));assert_eq!(host.releases,pending);
    host.fail=false;assert!(capture.retry_release(&mut host,10));
    assert_eq!(events.borrow().iter().filter(|phase|**phase==PointerAction::Cancel).count(),1);

}

#[test]
fn source_scroll_patch_matches_fresh_native_snapshot_and_rejects_bad_frame() {
    use jasper_ui_blitz_host_probe::{style::{Style,Display,Overflow,Length,Color},ScenePointerInput,PointerAction,Modifiers,EventReply};
    use jasper_ui_blitz_host_probe::Point;
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
    let mut clip=Style::default().size(Length::Px(128.0),Length::Px(48.0));clip.display=Display::Block;clip.overflow=Overflow::Hidden;
    let block=|id,color|Node::rect(NodeId(id)).with_style(Style::default().size(Length::Px(128.0),Length::Px(32.0)).background(color));
    let mut source=Scene::new(Node::container(NodeId(1)).child(Node::container(NodeId(2)).with_style(clip)
        .child(block(3,Color::RED)).child(block(4,Color::GREEN)).child(block(5,Color::BLUE))));
    let mut view=SceneHostView::new(10,&source,&font,160,100).unwrap();
    #[cfg(feature="cpu-pending")]
    let render=|v:&mut SceneHostView,version| {let mut bytes=vec![0;160*100*4];assert!(v.render_to_surface(10,version,&mut bytes,160*4,1024*1024));bytes};
    #[cfg(feature="cpu-pending")]
    let original=render(&mut view,source.version());
    let old=source.version();let patches=[ScenePatch::SetScroll {id:NodeId(2),offset:Point::new(0.0,32.0)}];
    source.apply_batch_at(old,&patches).unwrap();assert!(view.apply(10,old,source.version(),&patches));
    #[cfg(feature="cpu-pending")]
    {
        let changed=render(&mut view,source.version());assert_ne!(changed,original);
        let mut reference=SceneHostView::new(10,&source,&font,160,100).unwrap();assert_eq!(changed,render(&mut reference,source.version()));
    }
    let pointer=ScenePointerInput {phase:PointerAction::Move,position:(12.0,12.0),primary_pressed:false,modifiers:Modifiers::default()};
    let mut hit=None;assert!(view.pointer_input(10,source.version(),&pointer,&mut |event| {hit=Some(event.node);EventReply::default()},&mut |_|{}));
    assert_eq!(hit,Some(NodeId(4)));
    for invalid in [Point::new(-1.0,0.0),Point::new(f32::NAN,0.0),Point::new(0.0,f32::INFINITY)] {
        let bad=[ScenePatch::SetScroll {id:NodeId(2),offset:Point::ZERO},ScenePatch::SetScroll {id:NodeId(2),offset:invalid}];
        assert!(!view.apply(10,source.version(),source.version()+1,&bad));
        let mut hit=None;assert!(view.pointer_input(10,source.version(),&pointer,&mut |event| {hit=Some(event.node);EventReply::default()},&mut |_|{}));
        assert_eq!(hit,Some(NodeId(4)));
    }
    let old=source.version();let patches=[ScenePatch::Insert {parent:NodeId(2),index:3,node:block(6,Color::RED)},
        ScenePatch::SetScroll {id:NodeId(2),offset:Point::new(0.0,64.0)},
        ScenePatch::Insert {parent:NodeId(1),index:1,node:{
            let mut clip=Style::default().size(Length::Px(128.0),Length::Px(48.0));clip.display=Display::Block;clip.overflow=Overflow::Hidden;
            Node::container(NodeId(7)).with_style(clip).scroll(Point::new(0.0,32.0))
                .child(block(8,Color::RED)).child(block(9,Color::GREEN))
        }}];
    source.apply_batch_at(old,&patches).unwrap();assert!(view.apply(10,old,source.version(),&patches));
    #[cfg(feature="cpu-pending")]
    {let mut reference=SceneHostView::new(10,&source,&font,160,100).unwrap();assert_eq!(render(&mut view,source.version()),render(&mut reference,source.version()));}
    let pointer=ScenePointerInput {position:(12.0,60.0),..pointer};let mut hit=None;
    assert!(view.pointer_input(10,source.version(),&pointer,&mut |event| {hit=Some(event.node);EventReply::default()},&mut |_|{}));
    assert_eq!(hit,Some(NodeId(9)));
    for (display,expected_hit) in [(Display::None,NodeId(9)),(Display::Block,NodeId(5))] {
        let mut clip=Style::default().size(Length::Px(128.0),Length::Px(48.0));clip.display=display;clip.overflow=Overflow::Hidden;
        let old=source.version();let patches=[ScenePatch::UpdateStyle {id:NodeId(2),style:clip}];
        source.apply_batch_at(old,&patches).unwrap();assert!(view.apply(10,old,source.version(),&patches));
        let pointer=ScenePointerInput {position:(12.0,12.0),..pointer};let mut hit=None;
        assert!(view.pointer_input(10,source.version(),&pointer,&mut |event| {hit=Some(event.node);EventReply::default()},&mut |_|{}));
        assert_eq!(hit,Some(expected_hit));
        #[cfg(feature="cpu-pending")]
        {let mut reference=SceneHostView::new(10,&source,&font,160,100).unwrap();assert_eq!(render(&mut view,source.version()),render(&mut reference,source.version()));}
    }
}


#[test]
fn semantic_source_frames_update_disabled_focus_and_public_metadata() {
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
    let button=Semantics::default().role(Role::Button).focusable(true);
    let mut source=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Caption").with_semantics(button.clone())));
    let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();
    assert!(view.focus(10,source.version(),NodeId(2),&mut |_|{}));
    let disabled=Semantics {label:"Save file".into(),description:"Current action".into(),value:"busy".into(),disabled:true,..button.clone()};
    let old=source.version();let patches=[ScenePatch::UpdateSemantics {id:NodeId(2),semantics:disabled.clone()}];
    source.apply_batch_at(old,&patches).unwrap();let mut focus=Vec::new();
    assert!(view.apply_with_focus_events(10,old,source.version(),&patches,&mut |event|focus.push((event.node,event.focused,event.scene_version))));
    assert_eq!(focus,vec![(NodeId(2),false,source.version())]);
    let snapshot=view.semantics(10,source.version()).unwrap();assert_eq!(snapshot.nodes.iter().find(|node|node.node==NodeId(2)).unwrap().semantics,disabled);
    assert!(!view.focus(10,source.version(),NodeId(2),&mut |_|panic!("disabled focus")));
    assert!(!view.activate(10,source.version(),NodeId(2),&mut |_|panic!("disabled action")));
    let old=source.version();let patches=[ScenePatch::UpdateSemantics {id:NodeId(2),semantics:button.clone()},
        ScenePatch::UpdateSemantics {id:NodeId(1),semantics:Semantics::default().hidden(true)}];
    source.apply_batch_at(old,&patches).unwrap();assert!(view.apply(10,old,source.version(),&patches));
    assert!(!view.focus(10,source.version(),NodeId(2),&mut |_|panic!("hidden ancestor focus")));
    assert!(!view.activate(10,source.version(),NodeId(2),&mut |_|panic!("hidden ancestor action")));
    assert!(view.semantics(10,source.version()).unwrap().nodes.iter().find(|node|node.node==NodeId(2)).unwrap().semantics.label.is_empty());
    let old=source.version();let patches=[ScenePatch::UpdateSemantics {id:NodeId(1),semantics:Semantics::default()}];
    source.apply_batch_at(old,&patches).unwrap();assert!(view.apply(10,old,source.version(),&patches));
    assert!(view.focus(10,source.version(),NodeId(2),&mut |_|{}));
    let rejected=[ScenePatch::UpdateSemantics {id:NodeId(2),semantics:button.clone().label("Do not apply")},
        ScenePatch::UpdateSemantics {id:NodeId(2),semantics:Semantics::default().role(Role::Text)}];
    assert!(!view.apply(10,source.version(),source.version()+1,&rejected));
    let snapshot=view.semantics(10,source.version()).unwrap();assert_eq!(snapshot.nodes.iter().find(|node|node.node==NodeId(2)).unwrap().semantics,button);
}

#[test]
fn semantic_frames_follow_insert_remove_and_reused_identity_roles() {
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
    let mut source=Scene::new(Node::container(NodeId(1)));
    let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();
    let button=Semantics::default().role(Role::Button).focusable(true);
    let patches=[
        ScenePatch::Insert {parent:NodeId(1),index:0,node:Node::text(NodeId(2),"Save").with_semantics(button.clone())},
        ScenePatch::UpdateSemantics {id:NodeId(2),semantics:button.label("Inserted action")},
        ScenePatch::Remove {id:NodeId(2)},
        ScenePatch::Insert {parent:NodeId(1),index:0,node:Node::text(NodeId(2),"Result").with_semantics(Semantics::default().role(Role::Text))},
        ScenePatch::UpdateSemantics {id:NodeId(2),semantics:Semantics::default().role(Role::Text).label("Reused text identity")},
    ];
    let old=source.version();source.apply_batch_at(old,&patches).unwrap();
    assert!(view.apply(10,old,source.version(),&patches));
    let snapshot=view.semantics(10,source.version()).unwrap();
    let node=snapshot.nodes.iter().find(|node|node.node==NodeId(2)).unwrap();
    assert_eq!(node.semantics.role,Role::Text);assert_eq!(node.semantics.label,"Reused text identity");
    assert!(!view.activate(10,source.version(),NodeId(2),&mut |_|panic!("old button role survived")));
    let rejected=[ScenePatch::Remove {id:NodeId(2)},ScenePatch::UpdateSemantics {id:NodeId(2),semantics:Semantics::default().role(Role::Text)}];
    assert!(!view.apply(10,source.version(),source.version()+1,&rejected));
    assert_eq!(view.semantics(10,source.version()).unwrap(),snapshot);
}

#[test]
fn pointer_activation_respects_hidden_source_ancestor() {
    use jasper_ui_blitz_host_probe::{PointerAction,SceneEventKind,EventReply};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
    let mut source=Scene::new(Node::container(NodeId(1)).with_semantics(Semantics::default().hidden(true))
        .child(Node::text(NodeId(2),"Save").with_semantics(Semantics::default().role(Role::Button).focusable(true))));
    let mut view=SceneHostView::new(10,&source,&font,320,200).unwrap();let mut activated=false;let mut focused=false;
    for (phase,pressed) in [(PointerAction::Down,true),(PointerAction::Up,false)] {
        assert!(view.pointer(10,source.version(),phase,12.0,12.0,pressed,&mut |event| {
            activated|=event.kind==SceneEventKind::Activate;EventReply::default()
        },&mut |event|focused|=event.focused));
    }
    assert!(!activated,"hidden source ancestor must suppress semantic activation");
    assert!(!focused,"hidden source ancestor must suppress native pointer focus");
    let old=source.version();let patches=[ScenePatch::UpdateSemantics {id:NodeId(1),semantics:Semantics::default()}];
    source.apply_batch_at(old,&patches).unwrap();assert!(view.apply(10,old,source.version(),&patches));
    for (phase,pressed) in [(PointerAction::Down,true),(PointerAction::Up,false)] {
        assert!(view.pointer(10,source.version(),phase,12.0,12.0,pressed,&mut |event| {
            activated|=event.kind==SceneEventKind::Activate;EventReply::default()
        },&mut |event|focused|=event.focused));
    }
    assert!(activated,"restored source ancestor must allow native activation");
    assert!(view.focus(10,source.version(),NodeId(2),&mut |_|{}),"restored source ancestor must allow explicit Framework focus");
}

#[test]
fn semantic_attribute_roles_change_without_replacing_native_tree() {
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
    let mut source=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Resource")
        .with_semantics(Semantics::default().role(Role::Text))));
    let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();
    for semantics in [Semantics::default().role(Role::Link).label("Open resource").focusable(true),
        Semantics::default().role(Role::Text).label("Resource label")] {
        let old=source.version();let patches=[ScenePatch::UpdateSemantics {id:NodeId(2),semantics:semantics.clone()}];
        source.apply_batch_at(old,&patches).unwrap();assert!(view.apply(10,old,source.version(),&patches));
        let snapshot=view.semantics(10,source.version()).unwrap();
        assert_eq!(snapshot.nodes.iter().find(|node|node.node==NodeId(2)).unwrap().semantics,semantics);
        assert_eq!(view.focus(10,source.version(),NodeId(2),&mut |_|{}),semantics.focusable);
    }
    assert!(!view.apply(10,source.version(),source.version()+1,&[ScenePatch::UpdateSemantics {
        id:NodeId(2),semantics:Semantics::default().role(Role::Button)}]));
}

#[test]
fn public_scene_identity_tracks_committed_frames_and_retirement() {
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
    let mut source=Scene::new(Node::container(NodeId(1)));
    let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();
    assert_eq!(view.snapshot_identity(10),Some((source.version(),1)));
    assert_eq!(view.snapshot_identity(11),None);
    let old=source.version();let patches=[ScenePatch::Insert {parent:NodeId(1),index:0,node:Node::text(NodeId(2),"Item")}];
    source.apply_batch_at(old,&patches).unwrap();assert!(view.apply(10,old,source.version(),&patches));
    assert_eq!(view.snapshot_identity(10),Some((source.version(),2)));
    assert!(!view.apply(10,old,source.version(),&patches));
    assert_eq!(view.snapshot_identity(10),Some((source.version(),2)));
    assert!(view.close(10));assert_eq!(view.snapshot_identity(10),None);
}

#[test]
fn host_focus_query_is_owner_version_and_lifetime_scoped() {
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
    let source=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Save")
        .with_semantics(Semantics::default().role(Role::Button).focusable(true))));
    let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();let version=source.version();
    assert_eq!(view.focused_node(10,version),None);
    assert!(view.focus(10,version,NodeId(2),&mut |_|{}));
    assert_eq!(view.focused_node(10,version),Some(NodeId(2)));
    assert_eq!(view.focused_node(11,version),None);assert_eq!(view.focused_node(10,version+1),None);
    assert!(view.clear_focus(10,version,&mut |_|{}));assert_eq!(view.focused_node(10,version),None);
    assert!(view.focus(10,version,NodeId(2),&mut |_|{}));assert!(view.close(10));
    assert_eq!(view.focused_node(10,version),None);
}

#[test]
fn initial_tab_enters_native_scene_focus_order() {
    use jasper_ui_blitz_host_probe::{HostKeyInput,KeyLocation,Modifiers,EventReply};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
    let source=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Save")
        .with_semantics(Semantics::default().role(Role::Button).focusable(true))));
    let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();let mut transitions=Vec::new();
    let input=HostKeyInput {key:"Tab",code:"Tab",pressed:true,repeat:false,composing:false,modifiers:Modifiers::default(),location:KeyLocation::Standard};
    assert!(view.key(10,source.version(),&input,&mut |_|EventReply::default(),&mut |event|transitions.push((event.node,event.focused))));
    assert_eq!(view.focused_node(10,source.version()),Some(NodeId(2)));assert_eq!(transitions,vec![(NodeId(2),true)]);
}

#[test]
fn initial_tab_admission_and_reverse_order_stay_native() {
    use jasper_ui_blitz_host_probe::{HostKeyInput,KeyLocation,Modifiers,EventReply};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
    let button=|id|Node::text(NodeId(id),"Focus").with_semantics(Semantics::default().role(Role::Button).focusable(true));
    let source=Scene::new(Node::container(NodeId(1)).child(button(2)).child(button(3)));
    let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();let version=source.version();
    let mut key=HostKeyInput {key:"Tab",code:"Tab",pressed:true,repeat:false,composing:false,
        modifiers:Modifiers {shift:true,..Default::default()},location:KeyLocation::Standard};
    for (owner,v) in [(11,version),(10,version+1)] {
        assert!(!view.key(owner,v,&key,&mut |_|panic!("rejected key"),&mut |_|panic!("rejected focus")));
        assert_eq!(view.focused_node(10,version),None);
    }
    key.composing=true;
    assert!(!view.key(10,version,&key,&mut |_|panic!("composition command"),&mut |_|panic!("composition focus")));
    assert_eq!(view.focused_node(10,version),None);key.composing=false;
    key.pressed=false;
    assert!(!view.key(10,version,&key,&mut |_|panic!("unfocused keyup"),&mut |_|panic!("unfocused keyup")));
    key.pressed=true;key.code="InvalidKeyCode";
    assert!(!view.key(10,version,&key,&mut |_|panic!("invalid code"),&mut |_|panic!("invalid code")));
    key.code="Tab";let mut transitions=Vec::new();
    assert!(view.key(10,version,&key,&mut |_|EventReply::default(),&mut |event|transitions.push((event.node,event.focused))));
    assert_eq!(transitions,vec![(NodeId(3),true)]);assert_eq!(view.focused_node(10,version),Some(NodeId(3)));
    assert!(view.close(10));
    assert!(!view.key(10,version,&key,&mut |_|panic!("retired key"),&mut |_|panic!("retired focus")));
}

#[test]
fn scene_pointer_projection_keeps_native_primary_button_state() {
    use jasper_ui_blitz_host_probe::{PointerAction,ScenePointerInput,SceneEventKind,Modifiers,EventReply};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
    let source=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Sample")));
    let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();
    for pressed in [false,true,false] {
        let input=ScenePointerInput {phase:PointerAction::Move,position:(12.0,12.0),primary_pressed:pressed,modifiers:Modifiers::default()};
        let mut samples=Vec::new();
        assert!(view.pointer_input(10,source.version(),&input,&mut |event| {
            if event.kind==SceneEventKind::Pointer(PointerAction::Move) {samples.push(event.primary_pressed);}
            EventReply::default()
        },&mut |_|{}));
        assert_eq!(samples,vec![pressed]);
    }
}

#[test]
fn removed_pressed_button_does_not_activate_reused_source_identity() {
    use jasper_ui_blitz_host_probe::{PointerAction,SceneEventKind,EventReply};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
    let button=||Node::text(NodeId(2),"Save").with_semantics(Semantics::default().role(Role::Button));
    let mut source=Scene::new(Node::container(NodeId(1)).child(button()));
    let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();
    assert!(view.pointer(10,source.version(),PointerAction::Down,12.0,12.0,true,&mut |_|EventReply::default(),&mut |_|{}));
    let old=source.version();let patches=[ScenePatch::Remove {id:NodeId(2)},ScenePatch::Insert {parent:NodeId(1),index:0,node:button()}];
    source.apply_batch_at(old,&patches).unwrap();assert!(view.apply(10,old,source.version(),&patches));
    let mut activated=false;
    assert!(view.pointer(10,source.version(),PointerAction::Up,12.0,12.0,false,&mut |event| {
        activated|=event.kind==SceneEventKind::Activate;EventReply::default()
    },&mut |_|{}));
    assert!(!activated,"retired pressed node activated its replacement");
}

#[test]
fn cancelled_scene_press_cannot_activate_on_later_up() {
    use jasper_ui_blitz_host_probe::{PointerAction,SceneEventKind,EventReply};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
    let source=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Save")
        .with_semantics(Semantics::default().role(Role::Button))));
    let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();let mut actions=0;
    for (phase,pressed) in [(PointerAction::Down,true),(PointerAction::Cancel,false),(PointerAction::Up,false)] {
        assert!(view.pointer(10,source.version(),phase,12.0,12.0,pressed,&mut |event| {
            if event.kind==SceneEventKind::Activate {actions+=1;}EventReply::default()
        },&mut |_|{}));
    }
    assert_eq!(actions,0,"cancelled native press retained click eligibility");
    for (phase,pressed) in [(PointerAction::Down,true),(PointerAction::Up,false)] {
        assert!(view.pointer(10,source.version(),phase,12.0,12.0,pressed,&mut |event| {
            if event.kind==SceneEventKind::Activate {actions+=1;}EventReply::default()
        },&mut |_|{}));
    }
    assert_eq!(actions,1);
}

#[test]
fn live_pressed_node_survives_unrelated_frame_and_rejected_cancellation() {
    use jasper_ui_blitz_host_probe::{PointerAction,SceneEventKind,EventReply};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
    let mut source=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Save")
        .with_semantics(Semantics::default().role(Role::Button))).child(Node::text(NodeId(3),"Sibling")));
    let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();
    assert!(view.pointer(10,source.version(),PointerAction::Down,12.0,12.0,true,&mut |_|EventReply::default(),&mut |_|{}));
    let old=source.version();let patches=[ScenePatch::Remove {id:NodeId(3)},ScenePatch::UpdateText {id:NodeId(2),text:"Saved".into()}];
    source.apply_batch_at(old,&patches).unwrap();assert!(view.apply(10,old,source.version(),&patches));
    assert!(!view.pointer(10,old,PointerAction::Up,12.0,12.0,false,&mut |_|panic!("stale Up delivered"),&mut |_|{}));
    assert!(!view.pointer(11,source.version(),PointerAction::Cancel,12.0,12.0,false,&mut |_|panic!("foreign cancel delivered"),&mut |_|{}));
    let mut actions=0;
    assert!(view.pointer(10,source.version(),PointerAction::Up,12.0,12.0,false,&mut |event| {
        if event.kind==SceneEventKind::Activate {actions+=1;}EventReply::default()
    },&mut |_|{}));
    assert_eq!(actions,1,"unrelated frame or rejected input cancelled live native press");
}

#[test]
fn duplicate_scene_up_does_not_repeat_native_activation() {
    use jasper_ui_blitz_host_probe::{PointerAction,SceneEventKind,EventReply};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
    let source=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Save")
        .with_semantics(Semantics::default().role(Role::Button))));
    let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();let mut actions=0;
    for (phase,pressed) in [(PointerAction::Down,true),(PointerAction::Up,false),(PointerAction::Up,false)] {
        assert!(view.pointer(10,source.version(),phase,12.0,12.0,pressed,&mut |event| {
            if event.kind==SceneEventKind::Activate {actions+=1;}EventReply::default()
        },&mut |_|{}));
    }
    assert_eq!(actions,1,"duplicate Up repeated completed native activation");
}

#[test]
fn host_deactivation_retires_press_and_focus_without_closing_scene() {
    use jasper_ui_blitz_host_probe::{PointerAction,SceneEventKind,EventReply};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
    let source=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Save")
        .with_semantics(Semantics::default().role(Role::Button).focusable(true))));
    let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();let v=source.version();
    assert!(view.focus(10,v,NodeId(2),&mut |_|{}));
    assert!(view.pointer(10,v,PointerAction::Down,12.0,12.0,true,&mut |_|EventReply::default(),&mut |_|{}));
    assert!(!view.deactivate(11,v,&mut |_|panic!("foreign deactivation")));
    assert_eq!(view.focused_node(10,v),Some(NodeId(2)));
    let mut focus=Vec::new();assert!(view.deactivate(10,v,&mut |event|focus.push((event.node,event.focused))));
    assert_eq!(focus,vec![(NodeId(2),false)]);assert_eq!(view.focused_node(10,v),None);
    assert!(view.pointer(10,v,PointerAction::Up,12.0,12.0,false,&mut |event| {
        assert_ne!(event.kind,SceneEventKind::Activate);EventReply::default()
    },&mut |_|{}));
    assert!(view.snapshot_identity(10).is_some());assert!(view.focus(10,v,NodeId(2),&mut |_|{}));
    assert!(view.close(10));assert!(!view.deactivate(10,v,&mut |_|panic!("retired deactivation")));
}

#[test]
fn scene_deactivation_is_local_to_one_host_view() {
    use jasper_ui_blitz_host_probe::{PointerAction,SceneEventKind,EventReply};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
    let source=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Save")
        .with_semantics(Semantics::default().role(Role::Button).focusable(true))));
    let v=source.version();let mut a=SceneHostView::new(10,&source,&font,160,80).unwrap();
    let mut b=SceneHostView::new(11,&source,&font,160,80).unwrap();
    assert!(a.focus(10,v,NodeId(2),&mut |_|{}));assert!(b.focus(11,v,NodeId(2),&mut |_|{}));
    for (view,owner) in [(&mut a,10),(&mut b,11)] {
        assert!(view.pointer(owner,v,PointerAction::Down,12.0,12.0,true,&mut |_|EventReply::default(),&mut |_|{}));
    }
    let identity=a.snapshot_identity(10);
    assert!(!a.deactivate(10,v+1,&mut |_|panic!("stale deactivation")));
    assert!(a.deactivate(10,v,&mut |_|{}));assert_eq!(a.snapshot_identity(10),identity);
    assert!(a.deactivate(10,v,&mut |_|panic!("duplicate loss notification")));
    assert_eq!(b.focused_node(11,v),Some(NodeId(2)));let mut actions=0;
    assert!(b.pointer(11,v,PointerAction::Up,12.0,12.0,false,&mut |event| {
        if event.kind==SceneEventKind::Activate {actions+=1;}EventReply::default()
    },&mut |_|{}));assert_eq!(actions,1);
}

#[test]
fn host_node_bounds_project_native_layout_scroll_and_version() {
    use jasper_ui_blitz_host_probe::{Rect,style::{Style,Length}};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
    let mut source=Scene::new(Node::container(NodeId(1)).child(Node::rect(NodeId(2))
        .with_style(Style::default().size(Length::Px(80.0),Length::Px(40.0)))));
    let mut view=SceneHostView::new(10,&source,&font,160,80).unwrap();let v=source.version();
    assert_eq!(view.node_bounds(10,v,NodeId(2)),Some(Rect::new(0.0,0.0,80.0,40.0)));
    assert_eq!(view.node_bounds(11,v,NodeId(2)),None);assert_eq!(view.node_bounds(10,v+1,NodeId(2)),None);
    assert_eq!(view.node_bounds(10,v,NodeId(99)),None);
    assert!(view.set_viewport(10,320,160,2.0));assert!(view.set_viewport_scroll(10,v,0.0,10.0));
    assert_eq!(view.node_bounds(10,v,NodeId(2)),Some(Rect::new(0.0,-10.0,80.0,40.0)));
    let patches=[ScenePatch::UpdateStyle {id:NodeId(2),style:Style::default().size(Length::Px(120.0),Length::Px(50.0))}];
    source.apply_batch_at(v,&patches).unwrap();assert!(view.apply(10,v,source.version(),&patches));
    assert_eq!(view.node_bounds(10,source.version(),NodeId(2)),Some(Rect::new(0.0,-10.0,120.0,50.0)));
    assert!(view.close(10));assert_eq!(view.node_bounds(10,source.version(),NodeId(2)),None);
}

#[test]
fn host_node_bounds_follow_nested_source_scroll_without_clipping() {
    use jasper_ui_blitz_host_probe::{Rect,Point,style::{Style,Display,Overflow,Length}};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
    let mut clip=Style::default().size(Length::Px(100.0),Length::Px(40.0));
    clip.display=Display::Block;clip.overflow=Overflow::Hidden;
    let block=|id|Node::rect(NodeId(id)).with_style(Style::default().size(Length::Px(100.0),Length::Px(30.0)));
    let mut source=Scene::new(Node::container(NodeId(1)).child(Node::container(NodeId(2)).with_style(clip)
        .child(block(3)).child(block(4)).child(block(5))));
    let mut view=SceneHostView::new(10,&source,&font,160,100).unwrap();
    let old=source.version();let patches=[ScenePatch::SetScroll {id:NodeId(2),offset:Point::new(0.0,30.0)}];
    source.apply_batch_at(old,&patches).unwrap();assert!(view.apply(10,old,source.version(),&patches));
    assert_eq!(view.node_bounds(10,source.version(),NodeId(3)),Some(Rect::new(0.0,-30.0,100.0,30.0)));
    assert_eq!(view.node_bounds(10,source.version(),NodeId(4)),Some(Rect::new(0.0,0.0,100.0,30.0)));
    assert_eq!(view.node_bounds(10,source.version(),NodeId(5)),Some(Rect::new(0.0,30.0,100.0,30.0)));
    assert_eq!(view.node_bounds(10,source.version(),NodeId(2)),None);
    let old=source.version();let patches=[ScenePatch::UpdateSemantics {id:NodeId(2),semantics:Semantics::default().hidden(true)}];
    source.apply_batch_at(old,&patches).unwrap();assert!(view.apply(10,old,source.version(),&patches));
    assert_eq!(view.node_bounds(10,source.version(),NodeId(4)),None);
    let old=source.version();let patches=[
        ScenePatch::UpdateSemantics {id:NodeId(2),semantics:Semantics::default()},
        ScenePatch::SetScroll {id:NodeId(2),offset:Point::new(0.0,0.0)},
    ];
    source.apply_batch_at(old,&patches).unwrap();assert!(view.apply(10,old,source.version(),&patches));
    assert_eq!(view.node_bounds(10,source.version(),NodeId(2)),Some(Rect::new(0.0,0.0,100.0,40.0)));
    assert_eq!(view.node_bounds(10,source.version(),NodeId(4)),Some(Rect::new(0.0,30.0,100.0,30.0)));
}

// Theme recipes stay in Framework; this checks consumption of a typed visual-only frame.
#[test]
fn visual_style_frame_preserves_native_focus_and_scroll() {
    use jasper_ui_blitz_host_probe::{Point,style::{Style,Display,Overflow,Length,Color}};
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
    let mut clip=Style::default().size(Length::Px(100.0),Length::Px(40.0));
    clip.display=Display::Block;clip.overflow=Overflow::Hidden;
    let button_style=Style::default().size(Length::Px(100.0),Length::Px(30.0));
    let mut source=Scene::new(Node::container(NodeId(1)).child(Node::container(NodeId(2)).with_style(clip.clone())
        .scroll(Point::new(0.0,30.0)).child(Node::rect(NodeId(3)).with_style(button_style.clone()))
        .child(Node::text(NodeId(4),"Save").with_style(button_style.clone())
            .with_semantics(Semantics::default().role(Role::Button).focusable(true).label("Save")))));
    let mut view=SceneHostView::new(10,&source,&font,160,100).unwrap();let old=source.version();
    assert!(view.focus(10,old,NodeId(4),&mut |_|{}));
    assert!(view.set_viewport_scroll(10,old,0.0,5.0));
    let bounds=view.node_bounds(10,old,NodeId(4));assert!(bounds.is_some());
    let identity=view.snapshot_identity(10).unwrap();
    let patches=[ScenePatch::UpdateStyle {id:NodeId(2),style:clip.background(Color::BLUE)},
        ScenePatch::UpdateStyle {id:NodeId(4),style:button_style.background(Color::RED)}];
    source.apply_batch_at(old,&patches).unwrap();
    assert!(view.apply_with_focus_events(10,old,source.version(),&patches,&mut |_|panic!("visual frame changed focus")));
    assert_eq!(view.focused_node(10,source.version()),Some(NodeId(4)));
    assert_eq!(view.node_bounds(10,source.version(),NodeId(4)),bounds);
    assert_eq!(view.snapshot_identity(10).unwrap().1,identity.1);
    let semantic=view.semantics(10,source.version()).unwrap();
    assert_eq!(semantic.nodes.iter().find(|n|n.node==NodeId(4)).unwrap().semantics.label,"Save");
}

#[test]
fn framework_scene_update_value_keeps_host_identity_outside_payload() {
    use jasper_ui_blitz_host_probe::SceneUpdate;
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
    let mut source=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Before")));
    let mut host=SceneHostView::new(10,&source,&font,160,100).unwrap();let old=source.version();
    let identity=host.snapshot_identity(10);
    let patches=[ScenePatch::UpdateText {id:NodeId(2),text:"After".into()}];
    let frame=SceneUpdate {expected_version:old,next_version:old+1,patches:&patches};
    assert!(!host.apply_update(11,frame));
    assert!(!host.apply_update(10,SceneUpdate {next_version:old,..frame}));
    assert!(!host.apply_update(10,SceneUpdate {expected_version:old+1,..frame}));
    assert!(!host.apply_update(10,SceneUpdate {patches:&[],..frame}));
    let invalid=[ScenePatch::UpdateText {id:NodeId(99),text:"Missing".into()}];
    assert!(!host.apply_update(10,SceneUpdate {patches:&invalid,..frame}));
    assert_eq!(host.snapshot_identity(10),identity);
    source.apply_batch_at(old,frame.patches).unwrap();assert!(host.apply_update(10,frame));
    assert_eq!(host.snapshot_identity(10),Some((source.version(),2)));
    assert!(!host.apply_update(10,frame));
    assert!(host.close(10));assert!(!host.apply_update(10,SceneUpdate {expected_version:old+1,next_version:old+2,..frame}));
}
