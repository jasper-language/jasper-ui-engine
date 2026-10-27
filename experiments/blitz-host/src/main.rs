//! Typed embedding prototype candidate; native internals remain in the library.
use jasper_ui_blitz_host_probe::{NodeId,ComponentUpdate,ContentFlow,TextAppearance,HostKeyInput,KeyLocation,Modifiers,EventReply};
use jasper_ui_blitz_host_probe::host::{TextDisplayView,TextPointerCapture,PointerCaptureHost,UiInputBinding,UiInputContext,UiInputTicket};

// Synchronous injected-host fixture only; no OS capture or window implementation.
#[derive(Default)]
struct CaptureFixture {captures:usize,releases:usize}
impl PointerCaptureHost for CaptureFixture {
    fn capture(&mut self,_:UiInputTicket)->bool {self.captures+=1;true}
    fn release(&mut self,_:UiInputTicket)->bool {self.releases+=1;true}
}
fn main()->Result<(),Box<dyn std::error::Error>> {
    let path=std::env::args().nth(1).ok_or("supply an injected font file")?;
    let bytes=std::fs::read(path)?;
    {
        use jasper_ui_blitz_host_probe::{Scene,SceneUpdate,Node,ScenePatch,Semantics,Role,style::{Style,Length,Color,Display,Overflow},Point,Rect};
        use jasper_ui_engine::engine::Engine as SceneHostView;
        #[cfg(feature="cpu-pending")]
        let render=|view:&mut SceneHostView,version:u64|->Option<Vec<u8>> {
            let mut pixels=vec![0;640*480*4];
            view.render_to_surface(10,version,&mut pixels,640*4,4*1024*1024).then_some(pixels)
        };
        let mut title=Node::text(NodeId(2),"Jasper native scene 中文");
        title.semantics=Semantics::default().role(Role::Text).label("Scene title");
        let mut button=Node::text(NodeId(4),"Native focus target");
        button.semantics=Semantics {role:Role::Button,focusable:true,..Default::default()};
        let mut scroll_style=Style::default().size(Length::Px(128.0),Length::Px(40.0));
        scroll_style.display=Display::Block;scroll_style.overflow=Overflow::Hidden;
        let scroll=Node::container(NodeId(6)).with_style(scroll_style).scroll(Point::new(0.0,16.0))
            .with_semantics(Semantics::default().role(Role::ScrollView).label("Source scroll area"))
            .child(Node::rect(NodeId(7)).with_style(Style::default().size(Length::Px(128.0),Length::Px(64.0)).background(Color::RED)))
            .child(Node::rect(NodeId(8)).with_style(Style::default().size(Length::Px(128.0),Length::Px(64.0)).background(Color::BLUE)));
        let mut source=Scene::new(Node::container(NodeId(1)).child(title)
            .child(Node::rect(NodeId(3)).with_style(Style::default().size(Length::Px(120.0),Length::Px(32.0)).background(Color::BLUE))).child(button).child(scroll));
        let mut native=SceneHostView::new(10,&source,&bytes,640,480)
            .ok_or("existing scene native import failed")?;
        if native.snapshot_identity(10)!=Some((source.version(),7)) || native.snapshot_identity(11).is_some() {
            return Err("source scene identity admission failed".into());
        }
        if native.node_bounds(10,source.version(),NodeId(1))!=Some(Rect::new(0.0,0.0,640.0,480.0)) {
            return Err("native root geometry projection failed".into());
        }
        let semantics=native.semantics(10,source.version()).ok_or("source semantics unavailable")?;
        if semantics.nodes.len()!=7 || semantics.root!=NodeId(1)
            || semantics.nodes.iter().find(|node|node.node==NodeId(2)).map(|node|node.semantics.label.as_str())!=Some("Scene title") {
            return Err("existing scene identity/semantics projection failed".into());
        }
        #[cfg(feature="cpu-pending")]
        let before=render(&mut native,source.version()).ok_or("source scene render failed")?;
        let expected=source.version();
        let patches=[ScenePatch::UpdateText {id:NodeId(2),text:"Updated native scene 中文".into()},
            ScenePatch::UpdateStyle {id:NodeId(3),style:Style::default().size(Length::Px(160.0),Length::Px(40.0)).background(Color::GREEN)},
            ScenePatch::Insert {parent:NodeId(1),index:1,node:Node::text(NodeId(5),"Inserted through source frame")},
            ScenePatch::Remove {id:NodeId(3)},
            ScenePatch::SetScroll {id:NodeId(6),offset:Point::new(0.0,64.0)},
            ScenePatch::UpdateSemantics {id:NodeId(4),semantics:Semantics::default().role(Role::Button).focusable(true).label("Updated action")},
            ScenePatch::UpdateSemantics {id:NodeId(2),semantics:Semantics::default().role(Role::Link).label("Updated scene title")}];
        source.apply_batch_at(expected,&patches).map_err(|_|"source scene batch failed")?;
        let update=SceneUpdate {expected_version:expected,next_version:source.version(),patches:&patches};
        if !native.apply_update(10,update)
            || native.apply_update(10,update) {
            return Err("native scene incremental/stale update failed".into());
        }
        if native.node_bounds(10,source.version(),NodeId(3)).is_some()
            || native.node_bounds(10,source.version(),NodeId(5)).is_none() {
            return Err("native geometry source identity update failed".into());
        }
        let updated=native.semantics(10,source.version()).ok_or("updated semantics unavailable")?;
        if updated.nodes.iter().find(|node|node.node==NodeId(4)).map(|node|node.semantics.label.as_str())!=Some("Updated action") {
            return Err("typed semantic frame was not projected".into());
        }
        if updated.nodes.iter().find(|node|node.node==NodeId(2)).map(|node|node.semantics.role)!=Some(Role::Link) {
            return Err("typed semantic role update was not projected".into());
        }
        let mut activated=None;
        if native.activate(10,expected,NodeId(4),&mut |_| {})
            || !native.activate(10,source.version(),NodeId(4),&mut |event| {
                activated=Some((event.scene_version,event.node));
            }) || activated!=Some((source.version(),NodeId(4))) {
            return Err("scene prototype semantic action routing failed".into());
        }
        #[cfg(feature="cpu-pending")]
        {
            let after=render(&mut native,source.version()).ok_or("updated source scene render failed")?;
            if after==before {return Err("source scene patch did not change pixels".into());}
            let mut backend=jasper_ui_engine::backend::HeadlessBackend::new(1,1);
            native.render_to_backend(10,source.version(),&mut backend,4*1024*1024)
                .map_err(|_|"native frame backend submission failed")?;
            if backend.canvas().pixels()!=after.as_slice() {
                return Err("native backend frame differs from injected surface output".into());
            }
            let stride=640*4+16;let mut host_surface=vec![0xa5;479*stride+640*4];
            if !native.render_to_surface(10,source.version(),&mut host_surface,stride,4*1024*1024) {
                return Err("injected software surface adaptation failed".into());
            }
            let mut reference=SceneHostView::new(10,&source,&bytes,640,480)
                .ok_or("updated source reference import failed")?;
            if render(&mut reference,source.version()).as_deref()!=Some(after.as_slice()) {
                return Err("incremental source frame differs from fresh native import".into());
            }
            if !native.set_viewport_scroll(10,source.version(),0.0,24.0)
                || !native.set_viewport_scroll(10,source.version(),0.0,0.0)
                || render(&mut native,source.version()).as_deref()!=Some(after.as_slice()) {
                return Err("scene prototype viewport restoration failed".into());
            }
            println!("existing Scene native CPU frame bytes={}; acceptance pending",after.len());
        }
        let pointer=jasper_ui_blitz_host_probe::ScenePointerInput {
            phase:jasper_ui_blitz_host_probe::PointerAction::Move,position:(12.0,12.0),primary_pressed:true,
            modifiers:Modifiers {shift:true,control:true,alt:false,meta:false}};
        let mut pointer_routed=false;
        if !native.pointer_input(10,source.version(),&pointer,&mut |event| {
            pointer_routed=event.scene_version==source.version() && event.primary_pressed
                && event.modifiers.shift && event.modifiers.control;
            EventReply {prevent_default:true,stop_propagation:true}
        },&mut |_| {}) || !pointer_routed {return Err("scene prototype pointer metadata lost".into());}
        let mut focused=false;
        let entry=HostKeyInput {key:"Tab",code:"Tab",pressed:true,repeat:false,composing:false,
            modifiers:Modifiers::default(),location:KeyLocation::Standard};
        if native.focused_node(10,source.version()).is_some()
            || !native.key(10,source.version(),&entry,&mut |_|EventReply::default(),
                &mut |event| {focused=event.node==NodeId(4) && event.focused;})
            || !focused || native.focused_node(10,source.version())!=Some(NodeId(4)) {return Err("scene prototype initial Tab failed".into());}
        let key=HostKeyInput {key:"ArrowLeft",code:"ArrowLeft",pressed:true,repeat:false,composing:false,
            modifiers:Modifiers::default(),location:KeyLocation::Standard};
        let mut routed=false;
        if !native.key(10,source.version(),&key,&mut |event| {
            routed=event.node==NodeId(4) && event.scene_version==source.version();
            EventReply {prevent_default:true,stop_propagation:true}
        },&mut |_| {}) || !routed {return Err("scene prototype command routing failed".into());}
        let mut deactivated_focus=false;
        if !native.deactivate(10,source.version(),&mut |event|deactivated_focus=event.node==NodeId(4) && !event.focused)
            || !deactivated_focus || native.focused_node(10,source.version()).is_some()
            || native.snapshot_identity(10).is_none()
            || !native.focus(10,source.version(),NodeId(4),&mut |_|{}) {
            return Err("scene prototype deactivation/reentry failed".into());
        }
        let mut retired_focus=false;
        if !native.set_viewport(10,0,0,1.0)
            || !native.set_viewport(10,640,480,1.0)
            || !native.close_with_focus_events(10,source.version(),&mut |event| {retired_focus=event.node==NodeId(4) && !event.focused;})
            || !retired_focus || native.semantics(10,source.version()).is_some() {
            return Err("native scene surface/retirement fixture failed".into());
        }
        // Separate source fixture consumes native cancellation; no OS capture is acquired here.
        let lifecycle_source=Scene::new(Node::container(NodeId(20)).child(Node::text(NodeId(21),"Action")
            .with_semantics(Semantics::default().role(Role::Button))));
        let mut lifecycle=SceneHostView::new(10,&lifecycle_source,&bytes,160,80).ok_or("lifecycle fixture import failed")?;
        let mut actions=0;
        for (phase,pressed) in [(PointerAction::Down,true),(PointerAction::Cancel,false),(PointerAction::Up,false)] {
            if !lifecycle.pointer(10,lifecycle_source.version(),phase,12.0,12.0,pressed,&mut |event| {
                if event.kind==jasper_ui_blitz_host_probe::SceneEventKind::Activate {actions+=1;}EventReply::default()
            },&mut |_|{}) {return Err("scene cancellation fixture dispatch failed".into());}
        }
        if actions!=0 {return Err("cancelled scene gesture activated".into());}
        for (phase,pressed) in [(PointerAction::Down,true),(PointerAction::Up,false),(PointerAction::Up,false)] {
            if !lifecycle.pointer(10,lifecycle_source.version(),phase,12.0,12.0,pressed,&mut |event| {
                if event.kind==jasper_ui_blitz_host_probe::SceneEventKind::Activate {actions+=1;}EventReply::default()
            },&mut |_|{}) {return Err("fresh scene gesture dispatch failed".into());}
        }
        if actions!=1 || !lifecycle.close(10) {return Err("scene cancellation recovery fixture failed".into());}
    }
    // External producer fixtures: supplied snapshots/preedit, never an editor or IME engine.
    let context=UiInputContext {target:NodeId(7),target_generation:1,document:20,revision:1};
    let next=UiInputContext {revision:2,..context};let mut binding=UiInputBinding::new(10,1);
    let ticket=binding.bind(10,context).ok_or("initial host lease failed")?;
    let mut display=TextDisplayView::new(10,context,&bytes,"外部显示中文 A😀B",640,480);
    let updates=[ComponentUpdate::Appearance(TextAppearance {font_size_px:24,padding_px:12,foreground:[28,70,110],background:[240,245,250]}),
        ComponentUpdate::Flow {direction:ContentFlow::Column,gap_px:4},ComponentUpdate::AccessibleLabel(Some("External display"))];
    if !display.update(10,context,0,&updates) || display.update_version(10,context)!=Some(1) {return Err("Framework update frame failed".into());}
    let mut focused=false;
    if !display.focus(&binding,10,ticket,&mut |event|focused=event.focused) || !focused {return Err("host focus projection failed".into());}
    let semantics=display.semantics(10,context).ok_or("display semantics unavailable")?;
    if semantics.name!="External display" || !semantics.native_focused || semantics.component_version!=1 {
        return Err("display semantic projection failed".into());
    }
    let token=display.begin_preedit(&binding,10,ticket).ok_or("preedit presentation token failed")?;
    if !display.preedit(&binding,10,token,Some("外部预编辑")) || !display.selection(&binding,10,ticket,Some((0,6)))
        || display.selected_text(&binding,10,ticket).as_deref()!=Some("外部") || display.hit(&binding,10,ticket,30.0,20.0).is_none() {
        return Err("external display projection failed".into());
    }
    let input=HostKeyInput {key:"ArrowLeft",code:"ArrowLeft",pressed:true,repeat:false,composing:false,modifiers:Modifiers::default(),location:KeyLocation::Standard};
    let mut command=false;
    if !display.key(&binding,10,ticket,&input,&mut |_| {command=true;EventReply {prevent_default:true,stop_propagation:true}}) || !command {
        return Err("command key routing failed".into());
    }
    let mut host=CaptureFixture::default();
    let samples=std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));let output=samples.clone();
    let mut capture=TextPointerCapture::new(10,4,move |event| {
        output.borrow_mut().push((event.phase,(event.modifiers.shift,event.modifiers.control,event.modifiers.alt,event.modifiers.meta)));
        EventReply {prevent_default:true,stop_propagation:true}
    },|_|EventReply::default());
    let modifiers=Modifiers {shift:true,control:true,alt:false,meta:false};
    let pointer=capture.down_with_modifiers(&mut display,&mut host,10,context,18.0,18.0,modifiers).ok_or("capture grant failed")?;
    if !capture.move_with_modifiers(&mut display,10,pointer,-20.0,-20.0,modifiers)
        || !capture.up_with_modifiers(&mut display,&mut host,10,pointer,-20.0,-20.0,modifiers) || host.captures!=1 || host.releases!=1 {
        return Err("host capture projection failed".into());
    }
    use jasper_ui_blitz_host_probe::PointerAction;
    if *samples.borrow()!=vec![(PointerAction::Down,(true,true,false,false)),(PointerAction::Move,(true,true,false,false)),
        (PointerAction::Up,(true,true,false,false))] {return Err("capture prototype pointer metadata lost".into());}
    if !capture.focus_lost(&mut display,&mut host,&mut binding,10,ticket,&mut |_| {}) || binding.accepts(10,ticket) {
        return Err("exact focus lease cleanup failed".into());
    }
    if !display.snapshot(10,context,next,"服务后继快照 中文") {return Err("service successor snapshot failed".into());}
    let fresh=binding.bind(10,next).ok_or("successor host lease failed")?;
    let active=display.begin_preedit(&binding,10,fresh).ok_or("successor preedit token failed")?;
    if display.preedit(&binding,10,token,Some("Late")) || !display.preedit(&binding,10,active,Some("新")) {
        return Err("stale presentation identity admitted".into());
    }
    #[cfg(feature="cpu-pending")]
    println!("external client native CPU bytes={}; acceptance pending",display.render_cpu(10,next,4*1024*1024).ok_or("client render failed")?.len());
    if !display.close_focus(&mut binding,10,fresh,&mut |_| {}) || !capture.retire(&mut display,&mut host,10,next)
        || display.snapshot(10,next,next,"Retired") || display.preedit(&binding,10,active,Some("Late")) {
        return Err("client retirement failed".into());
    }
    Ok(())
}
