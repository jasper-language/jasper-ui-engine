//! Headless acceptance of frame damage reporting on the root native CPU path.
//! 根原生 CPU 路径帧损伤报告的无头验收。
#![cfg(feature="native-blitz-cpu-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::native::host::FrameDamage;
use jasper_ui_engine::scene::{Node,NodeId,Scene,ScenePatch};
use jasper_ui_engine::style::{Color,Length,Style};

const W:u32=160;
const H:u32=100;
const OWNER:u64=21;
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

fn render(engine:&mut Engine,version:u64)->(Vec<u8>,FrameDamage) {
    let mut bytes=vec![0;(W*H*4) as usize];
    let damage=engine.render_to_surface_with_damage(OWNER,version,&mut bytes,(W*4) as usize,BUDGET).expect("admitted render");
    (bytes,damage)
}

/// Independent brute-force oracle: bounding box of every differing pixel.
/// 独立穷举规格：所有不同像素的包围盒。
fn oracle(before:&[u8],after:&[u8])->FrameDamage {
    let mut found:Option<(u32,u32,u32,u32)>=None;
    for y in 0..H {for x in 0..W {
        let i=((y*W+x)*4) as usize;
        if before[i..i+4]==after[i..i+4] {continue;}
        found=Some(match found {None=>(x,y,x,y),Some((a,b,c,d))=>(a.min(x),b.min(y),c.max(x),d.max(y))});
    }}
    match found {None=>FrameDamage::Unchanged,Some((a,b,c,d))=>FrameDamage::Rect {x:a,y:b,width:c-a+1,height:d-b+1}}
}

#[test]
fn first_frame_full_then_exact_rect_for_local_update() {
    let source=scene();let font=font();let v=source.version();
    let mut engine=Engine::new(OWNER,&source,&font,W,H).expect("native import");
    let (before,damage)=render(&mut engine,v);
    assert_eq!(damage,FrameDamage::Full,"no previous frame");
    assert_eq!(render(&mut engine,v).1,FrameDamage::Unchanged,"reused cache");
    let style=Style::default().size(Length::Px(40.0),Length::Px(30.0)).background(Color::rgb(40,200,40));
    assert!(engine.apply(OWNER,v,v+1,&[ScenePatch::UpdateStyle {id:NodeId(2),style}]));
    let (after,damage)=render(&mut engine,v+1);
    assert_eq!(damage,FrameDamage::Rect {x:0,y:0,width:40,height:30},"recoloured rect only");
    assert_eq!(damage,oracle(&before,&after));
    // Growing the rect moves the text too: still the exact box of all changed pixels.
    let style=Style::default().size(Length::Px(70.0),Length::Px(30.0)).background(Color::rgb(40,200,40));
    assert!(engine.apply(OWNER,v+1,v+2,&[ScenePatch::UpdateStyle {id:NodeId(2),style}]));
    let (grown,damage)=render(&mut engine,v+2);
    assert_eq!(damage,oracle(&after,&grown));
    let FrameDamage::Rect {x,width,..}=damage else {panic!("grown rect must damage a region: {damage:?}")};
    assert!(x<=40 && x+width>=70,"damage covers the new rect columns: {damage:?}");
    eprintln!("frame-damage: grow {damage:?}");
}

#[test]
fn repaint_without_visible_change_is_unchanged() {
    let source=scene();let font=font();let v=source.version();
    let mut engine=Engine::new(OWNER,&source,&font,W,H).expect("native import");
    render(&mut engine,v);
    // A rejected stale update marks dirty conservatively; the repaint is pixel-identical.
    assert!(!engine.apply(OWNER,v,v+5,&[]));
    assert_eq!(engine.needs_frame(OWNER),Some(true));
    assert_eq!(render(&mut engine,v).1,FrameDamage::Unchanged);
}

#[test]
fn extent_change_host_invalidation_and_rejection() {
    let source=scene();let font=font();let v=source.version();
    let mut engine=Engine::new(OWNER,&source,&font,W,H).expect("native import");
    render(&mut engine,v);
    // Host lost its surface: identical pixels must still be reported Full.
    assert!(engine.invalidate_frame(OWNER));
    assert_eq!(render(&mut engine,v).1,FrameDamage::Full);
    assert!(engine.set_viewport(OWNER,W/2,H/2,1.0));
    let mut half=vec![0;(W/2*H/2*4) as usize];
    assert_eq!(engine.render_to_surface_with_damage(OWNER,v,&mut half,(W/2*4) as usize,BUDGET),Some(FrameDamage::Full));
    assert!(engine.set_viewport(OWNER,W,H,1.0));
    assert_eq!(render(&mut engine,v).1,FrameDamage::Full);
    let mut bytes=vec![0;(W*H*4) as usize];
    assert_eq!(engine.render_to_surface_with_damage(OWNER+1,v,&mut bytes,(W*4) as usize,BUDGET),None,"foreign owner");
    assert_eq!(engine.render_to_surface_with_damage(OWNER,v+9,&mut bytes,(W*4) as usize,BUDGET),None,"stale version");
    let mut short=vec![0;(W*H*4) as usize-1];
    assert_eq!(engine.render_to_surface_with_damage(OWNER,v,&mut short,(W*4) as usize,BUDGET),None,"short surface");
    assert!(engine.close(OWNER));
    assert_eq!(engine.render_to_surface_with_damage(OWNER,v,&mut bytes,(W*4) as usize,BUDGET),None,"closed view");
}
