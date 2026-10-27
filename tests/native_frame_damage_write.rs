//! Headless acceptance of damage-only surface writes on the root native CPU path.
//! 根原生 CPU 路径只写损伤区域的无头验收。
#![cfg(feature="native-blitz-cpu-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::native::host::FrameDamage;
use jasper_ui_engine::scene::{Node,NodeId,Scene,ScenePatch};
use jasper_ui_engine::style::{Color,Length,Style};

const W:u32=160;
const H:u32=100;
const OWNER:u64=31;
const BUDGET:usize=1024*1024;
const ROW:usize=(W*4) as usize;
const STRIDE:usize=ROW+8;
const PAD:u8=0xAB;
const POISON:[u8;4]=[1,2,3,4];

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

fn padded_surface()->Vec<u8> {vec![PAD;STRIDE*(H as usize-1)+ROW]}

/// Packed reference frame through the whole-frame write path (reuses the current cached frame).
/// 经整帧写入路径得到的紧凑参考帧（复用当前缓存帧，不改变损伤基准）。
fn reference(engine:&mut Engine,version:u64)->Vec<u8> {
    let mut bytes=vec![0;ROW*H as usize];
    assert!(engine.render_to_surface(OWNER,version,&mut bytes,ROW,BUDGET));
    bytes
}

fn pixel(surface:&[u8],x:u32,y:u32)->&[u8] {let i=y as usize*STRIDE+x as usize*4;&surface[i..i+4]}

fn poison(surface:&mut [u8],x:u32,y:u32) {let i=y as usize*STRIDE+x as usize*4;surface[i..i+4].copy_from_slice(&POISON);}

/// Every active pixel equals the reference except `skip`; every padding byte is untouched.
/// 除 `skip` 外每个有效像素等于参考帧；每个 padding 字节保持原值。
fn assert_matches(surface:&[u8],reference:&[u8],skip:&[(u32,u32)]) {
    for y in 0..H {
        let row=y as usize*STRIDE;
        for x in 0..W {
            if skip.contains(&(x,y)) {continue;}
            let r=(y*W+x) as usize*4;
            assert_eq!(pixel(surface,x,y),&reference[r..r+4],"pixel ({x},{y})");
        }
        if y+1<H {assert!(surface[row+ROW..row+STRIDE].iter().all(|b|*b==PAD),"padding of row {y} untouched");}
    }
}

#[test]
fn damaged_writes_match_full_writes_and_touch_only_the_rect() {
    let source=scene();let font=font();let v=source.version();
    let mut engine=Engine::new(OWNER,&source,&font,W,H).expect("native import");
    let mut surface=padded_surface();
    assert_eq!(engine.render_to_surface_damaged(OWNER,v,&mut surface,STRIDE,BUDGET),Some(FrameDamage::Full));
    let first=reference(&mut engine,v);
    assert_matches(&surface,&first,&[]);
    // Unchanged writes nothing: a poisoned pixel survives.
    poison(&mut surface,W-1,H-1);
    assert_eq!(engine.render_to_surface_damaged(OWNER,v,&mut surface,STRIDE,BUDGET),Some(FrameDamage::Unchanged));
    assert_eq!(pixel(&surface,W-1,H-1),&POISON);
    // A recoloured rect writes exactly its rectangle: inside poison is overwritten, outside kept.
    poison(&mut surface,10,10);
    let style=Style::default().size(Length::Px(40.0),Length::Px(30.0)).background(Color::rgb(40,200,40));
    assert!(engine.apply(OWNER,v,v+1,&[ScenePatch::UpdateStyle {id:NodeId(2),style}]));
    assert_eq!(engine.render_to_surface_damaged(OWNER,v+1,&mut surface,STRIDE,BUDGET),
        Some(FrameDamage::Rect {x:0,y:0,width:40,height:30}));
    let second=reference(&mut engine,v+1);
    assert_ne!(first,second);
    assert_matches(&surface,&second,&[(W-1,H-1)]);
    assert_eq!(pixel(&surface,W-1,H-1),&POISON,"outside the damage rect is never written");
    // Growing the rect also moves text: the surface still converges on the full frame.
    let style=Style::default().size(Length::Px(70.0),Length::Px(30.0)).background(Color::rgb(40,200,40));
    assert!(engine.apply(OWNER,v+1,v+2,&[ScenePatch::UpdateStyle {id:NodeId(2),style}]));
    let damage=engine.render_to_surface_damaged(OWNER,v+2,&mut surface,STRIDE,BUDGET).expect("admitted");
    let FrameDamage::Rect {x,y,width,height}=damage else {panic!("local change must be a rect: {damage:?}")};
    assert!(x+width<W-1 || y+height<H-1,"poisoned corner lies outside {damage:?}");
    assert_matches(&surface,&reference(&mut engine,v+2),&[(W-1,H-1)]);
    eprintln!("damage-write: grow {damage:?}, copied {} of {} bytes",width as usize*height as usize*4,ROW*H as usize);
}

#[test]
fn rejection_writes_nothing_and_buffer_swap_needs_invalidation() {
    let source=scene();let font=font();let v=source.version();
    let mut engine=Engine::new(OWNER,&source,&font,W,H).expect("native import");
    let mut surface=padded_surface();
    let untouched=surface.clone();
    assert_eq!(engine.render_to_surface_damaged(OWNER+1,v,&mut surface,STRIDE,BUDGET),None,"foreign owner");
    assert_eq!(engine.render_to_surface_damaged(OWNER,v+9,&mut surface,STRIDE,BUDGET),None,"stale version");
    assert_eq!(engine.render_to_surface_damaged(OWNER,v,&mut surface,ROW-4,BUDGET),None,"overlapping stride");
    let mut short=vec![PAD;STRIDE*(H as usize-1)+ROW-1];
    assert_eq!(engine.render_to_surface_damaged(OWNER,v,&mut short,STRIDE,BUDGET),None,"short surface");
    assert!(short.iter().all(|b|*b==PAD));
    assert_eq!(surface,untouched,"rejected calls write nothing");
    // Rejected surfaces never advanced the damage base: the first admitted write is Full.
    assert_eq!(engine.render_to_surface_damaged(OWNER,v,&mut surface,STRIDE,BUDGET),Some(FrameDamage::Full));
    // Swapping to a fresh buffer: without invalidation the report would be Unchanged and the
    // buffer stay blank; the host contract is to invalidate first, which yields Full.
    let mut swapped=padded_surface();
    assert!(engine.invalidate_frame(OWNER));
    assert_eq!(engine.render_to_surface_damaged(OWNER,v,&mut swapped,STRIDE,BUDGET),Some(FrameDamage::Full));
    assert_matches(&swapped,&reference(&mut engine,v),&[]);
    assert!(engine.close(OWNER));
    assert_eq!(engine.render_to_surface_damaged(OWNER,v,&mut swapped,STRIDE,BUDGET),None,"closed view");
}
