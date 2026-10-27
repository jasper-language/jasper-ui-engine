//! Headless acceptance of on-demand frame scheduling on the root native CPU path.
//! 根原生 CPU 路径按需帧调度的无头验收。
#![cfg(feature="native-blitz-cpu-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::native::PointerAction;
use jasper_ui_engine::native::EventReply;
use jasper_ui_engine::scene::{Node,NodeId,Scene,ScenePatch};
use jasper_ui_engine::style::{Color,Length,Style};
use std::time::Instant;

const W:u32=160;
const H:u32=100;
const OWNER:u64=11;
const BUDGET:usize=1024*1024;

fn scene()->Scene {
    Scene::new(Node::container(NodeId(1)).with_style(Style::default()
            .size(Length::Px(W as f32),Length::Px(H as f32)).background(Color::rgb(10,20,30)))
        .child(Node::rect(NodeId(2)).with_style(Style::default()
            .size(Length::Px(40.0),Length::Px(30.0)).background(Color::rgb(200,40,40))))
        .child(Node::text(NodeId(3),"Jasper").with_style(Style::default().text(14.0,Color::WHITE))))
}

fn font()->Vec<u8> {
    let path=std::env::var("JASPER_UI_TEST_FONT").expect("JASPER_UI_TEST_FONT must name an injected Latin font");
    std::fs::read(path).expect("injected font readable")
}

fn render(engine:&mut Engine,version:u64)->Vec<u8> {
    let mut bytes=vec![0;(W*H*4) as usize];
    assert!(engine.render_to_surface(OWNER,version,&mut bytes,(W*4) as usize,BUDGET));
    bytes
}

fn px(bytes:&[u8],x:u32,y:u32)->[u8;4] {
    let i=((y*W+x)*4) as usize;[bytes[i],bytes[i+1],bytes[i+2],bytes[i+3]]
}

#[test]
fn unchanged_view_needs_no_frame_and_reuses_pixels() {
    let source=scene();let font=font();let v=source.version();
    let mut engine=Engine::new(OWNER,&source,&font,W,H).expect("native import");
    assert_eq!(engine.needs_frame(OWNER),Some(true),"first frame is always due");
    assert_eq!(engine.needs_frame(OWNER+1),None);
    let started=Instant::now();let first=render(&mut engine,v);let painted=started.elapsed();
    assert_eq!(engine.needs_frame(OWNER),Some(false));
    assert_eq!(px(&first,5,5),[200,40,40,255]);
    let started=Instant::now();let again=render(&mut engine,v);let reused=started.elapsed();
    assert_eq!(first,again,"reused frame is byte-identical");
    assert_eq!(engine.needs_frame(OWNER),Some(false));
    eprintln!("frame-schedule: paint {painted:?}, reuse {reused:?}");
    // Host-forced repaint produces the same pixels from a fresh paint.
    assert!(!engine.invalidate_frame(OWNER+1));
    assert_eq!(engine.needs_frame(OWNER),Some(false));
    assert!(engine.invalidate_frame(OWNER));
    assert_eq!(engine.needs_frame(OWNER),Some(true));
    assert_eq!(render(&mut engine,v),first);
    assert_eq!(engine.needs_frame(OWNER),Some(false));
}

#[test]
fn scene_update_invalidates_and_repaints() {
    let source=scene();let font=font();let v=source.version();
    let mut engine=Engine::new(OWNER,&source,&font,W,H).expect("native import");
    let before=render(&mut engine,v);
    let style=Style::default().size(Length::Px(40.0),Length::Px(30.0)).background(Color::rgb(40,200,40));
    assert!(engine.apply(OWNER,v,v+1,&[ScenePatch::UpdateStyle {id:NodeId(2),style}]));
    assert_eq!(engine.needs_frame(OWNER),Some(true));
    let after=render(&mut engine,v+1);
    assert_eq!(px(&before,5,5),[200,40,40,255]);
    assert_eq!(px(&after,5,5),[40,200,40,255],"cached frame must not be reused after an update");
    assert_eq!(engine.needs_frame(OWNER),Some(false));
    // A stale-version update is rejected; conservative damage only costs one extra paint.
    assert!(!engine.apply(OWNER,v,v+2,&[]));
    assert_eq!(engine.needs_frame(OWNER),Some(true));
    assert_eq!(render(&mut engine,v+1),after);
}

#[test]
fn viewport_pointer_and_close_drive_scheduling() {
    let source=scene();let font=font();let v=source.version();
    let mut engine=Engine::new(OWNER,&source,&font,W,H).expect("native import");
    let full=render(&mut engine,v);
    assert!(engine.set_viewport(OWNER,W/2,H/2,1.0));
    assert_eq!(engine.needs_frame(OWNER),Some(true));
    let mut half=vec![0;(W/2*H/2*4) as usize];
    assert!(engine.render_to_surface(OWNER,v,&mut half,(W/2*4) as usize,BUDGET));
    assert_eq!(&half[..4],&full[..4]);
    assert_eq!(engine.needs_frame(OWNER),Some(false));
    // The half-size cache must not satisfy the restored extent: a full frame is repainted.
    assert!(engine.set_viewport(OWNER,W,H,1.0));
    assert_eq!(engine.needs_frame(OWNER),Some(true));
    assert_eq!(render(&mut engine,v),full);
    let mut events=|_|EventReply::default();
    assert!(engine.pointer(OWNER,v,PointerAction::Move,5.0,5.0,false,&mut events,&mut |_|{}));
    assert_eq!(engine.needs_frame(OWNER),Some(true),"hover may change painted state");
    render(&mut engine,v);
    assert!(engine.set_viewport_scroll(OWNER,v,0.0,0.0));
    assert_eq!(engine.needs_frame(OWNER),Some(true));
    render(&mut engine,v);
    assert!(engine.close(OWNER));
    assert_eq!(engine.needs_frame(OWNER),None);
    assert!(!engine.invalidate_frame(OWNER));
    let mut closed=vec![0;(W*H*4) as usize];
    assert!(!engine.render_to_surface(OWNER,v,&mut closed,(W*4) as usize,BUDGET),"closed view never serves its cache");
}
