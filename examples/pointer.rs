//! Root native mouse routing candidate; FONT is injected by the embedding host.
//! 根原生鼠标路由候选；FONT 由宿主注入。触摸/capture 兼容演示见 legacy_pointer。
use jasper_ui_engine::{engine::Engine,scene::{Node,NodeId,Scene,ScenePatch},
    style::{Style,Length,Color,FlexDirection},geometry::Edges,semantics::{Semantics,Role}};
use jasper_ui_engine::native::{PointerAction,SceneEventKind,SceneUpdate,EventReply};
fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut arguments=std::env::args().skip(1);
    let path=arguments.next().ok_or("supply an injected font file")?;
    if arguments.next().is_some() {return Err("expected FONT".into());}
    let font=std::fs::read(path)?;
    let button=|id,color|Node::rect(NodeId(id)).with_semantics(Semantics::default().role(Role::Button).focusable(true))
        .with_style(Style::default().size(Length::Px(120.0),Length::Px(80.0)).background(color));
    let mut scene=Scene::new(Node::container(NodeId(1)).with_style(Style::default()
        .size(Length::Px(320.0),Length::Px(120.0)).padding(Edges::all(16.0)).gap(12.0).direction(FlexDirection::Row))
        .child(button(2,Color::GREEN)).child(button(3,Color::BLUE)));
    let mut engine=Engine::new(10,&scene,&font,320,120).ok_or("native scene rejected")?;
    let mut activations=Vec::new();
    // Generic native buttons require an explicit Framework focus request.
    // 固定后端的通用按钮需要 Framework 显式请求焦点。
    if !engine.focus(10,scene.version(),NodeId(2),&mut |_|{}) {return Err("native focus rejected".into());}
    // The native tree retains press/focus across a typed visual update.
    // 原生树在类型化视觉更新前后保留按下/焦点；不伪造 OS capture。
    if !engine.pointer(10,scene.version(),PointerAction::Down,30.0,30.0,true,
        &mut |event|{println!("down: {:?} {:?}",event.node,event.kind);EventReply::default()},&mut |_|{}) {return Err("native down rejected".into());}
    let old=scene.version();let patches=[ScenePatch::UpdateStyle {id:NodeId(2),
        style:Style::default().size(Length::Px(120.0),Length::Px(80.0)).background(Color::RED)}];
    scene.apply_batch_at(old,&patches).map_err(|error|format!("source frame failed: {error:?}"))?;
    if !engine.apply_update(10,SceneUpdate {expected_version:old,next_version:scene.version(),patches:&patches}) {
        return Err("native style frame rejected".into());
    }
    if engine.focused_node(10,scene.version())!=Some(NodeId(2)) {return Err("visual update lost native focus".into());}
    for (label,phase,x,y,pressed) in [
        ("up-after-style",PointerAction::Up,30.0,30.0,false),
        ("duplicate-up",PointerAction::Up,30.0,30.0,false),
        ("hover-blue",PointerAction::Move,200.0,30.0,false),
    ] {
        if !engine.pointer(10,scene.version(),phase,x,y,pressed,&mut |event| {
            println!("{label}: node={:?} kind={:?}",event.node,event.kind);
            if event.kind==SceneEventKind::Activate {activations.push(event.node);}
            EventReply::default()
        },&mut |_|{}) {return Err("native sample rejected".into());}
    }
    if activations!=[NodeId(2)] {return Err("native activation sequence mismatch".into());}
    if !engine.deactivate(10,scene.version(),&mut |_|{}) || engine.focused_node(10,scene.version()).is_some() {
        return Err("native deactivation failed".into());
    }
    if !engine.close(10) || engine.snapshot_identity(10).is_some() {return Err("native retirement failed".into());}
    println!("native mouse/style/deactivation/retirement candidate; acceptance pending");
    Ok(())
}
