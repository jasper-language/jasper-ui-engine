//! Headless acceptance of damage reporting and damage-only surface writes on the root native
//! GPU path, sharing the frame baseline with the CPU path.
//! 根原生 GPU 路径损伤报告与只写损伤区域的无头验收，与 CPU 路径共用帧基准。
#![cfg(feature="gpu-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::native::host::{FrameDamage,GpuFrameError,GpuRenderer};
use jasper_ui_engine::scene::{Node,NodeId,Scene,ScenePatch};
use jasper_ui_engine::style::{Color,Length,Style};

const W:u32=160;
const H:u32=100;
const OWNER:u64=41;
const BUDGET:usize=1024*1024;
const ROW:usize=(W*4) as usize;
const STRIDE:usize=ROW+8;
const PAD:u8=0xAB;
const POISON:[u8;4]=[1,2,3,4];

fn rect_style(width:f32,color:Color)->Style {
    Style::default().size(Length::Px(width),Length::Px(30.0)).background(color)
}

fn scene()->Scene {
    Scene::new(Node::container(NodeId(1)).with_style(Style::default()
            .size(Length::Px(W as f32),Length::Px(H as f32)).background(Color::rgb(10,20,30)))
        .child(Node::rect(NodeId(2)).with_style(rect_style(40.0,Color::rgb(200,40,40))))
        .child(Node::text(NodeId(3),"Jasper").with_style(Style::default().text(14.0,Color::WHITE))))
}

fn font()->Vec<u8> {
    let path=std::env::var("JASPER_UI_TEST_FONT").expect("JASPER_UI_TEST_FONT must name an injected Latin font");
    std::fs::read(path).expect("injected font readable")
}

/// A missing adapter fails the test: GPU acceptance is never reported without a device.
/// 没有适配器则测试失败：没有设备就不报告 GPU 验收。
fn gpu()->GpuRenderer {GpuRenderer::new_headless().expect("a wgpu adapter is required for GPU acceptance")}

fn padded_surface()->Vec<u8> {vec![PAD;STRIDE*(H as usize-1)+ROW]}

fn pixel(surface:&[u8],x:u32,y:u32)->&[u8] {let i=y as usize*STRIDE+x as usize*4;&surface[i..i+4]}

fn poison(surface:&mut [u8],x:u32,y:u32) {let i=y as usize*STRIDE+x as usize*4;surface[i..i+4].copy_from_slice(&POISON);}

/// Independent specification: minimal rectangle covering every differing pixel of two
/// compact frames of the same extent.
/// 独立规格：同尺寸两帧所有差异像素的最小包围矩形。
fn hull(a:&[u8],b:&[u8])->FrameDamage {
    let mut bounds:Option<(u32,u32,u32,u32)>=None;
    for y in 0..H {for x in 0..W {
        let i=((y*W+x)*4) as usize;
        if a[i..i+4]!=b[i..i+4] {
            bounds=Some(match bounds {None=>(x,y,x,y),Some((x0,y0,x1,y1))=>(x0.min(x),y0.min(y),x1.max(x),y1.max(y))});
        }
    }}
    match bounds {
        None=>FrameDamage::Unchanged,
        Some((x0,y0,x1,y1))=>FrameDamage::Rect {x:x0,y:y0,width:x1-x0+1,height:y1-y0+1},
    }
}

fn inside(damage:FrameDamage,x:u32,y:u32)->bool {
    match damage {
        FrameDamage::Full=>true,
        FrameDamage::Unchanged=>false,
        FrameDamage::Rect {x:rx,y:ry,width,height}=>x>=rx && x<rx+width && y>=ry && y<ry+height,
    }
}

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
fn gpu_damage_is_exact_and_damaged_writes_touch_only_the_rect() {
    let source=scene();let font=font();let v=source.version();
    let mut engine=Engine::new(OWNER,&source,&font,W,H).expect("native import");
    let mut gpu=gpu();
    let mut surface=padded_surface();
    assert_eq!(engine.render_gpu_to_surface_damaged(OWNER,v,&mut gpu,&mut surface,STRIDE,BUDGET),Ok(FrameDamage::Full));
    // A clean view reuses the produced frame: render_gpu returns it unchanged.
    // 干净视图复用已产出帧：render_gpu 原样返回它。
    let first=engine.render_gpu(OWNER,v,&mut gpu,BUDGET).expect("reused gpu frame");
    assert_matches(&surface,&first,&[]);
    poison(&mut surface,W-1,H-1);
    assert_eq!(engine.render_gpu_to_surface_damaged(OWNER,v,&mut gpu,&mut surface,STRIDE,BUDGET),Ok(FrameDamage::Unchanged));
    assert_eq!(pixel(&surface,W-1,H-1),&POISON,"Unchanged writes nothing");
    // Recolour: damage equals the independent hull; inside poison is overwritten, outside kept.
    // 改色：损伤等于独立包围盒；矩形内被改写像素被覆盖，矩形外保留。
    poison(&mut surface,10,10);
    assert!(engine.apply(OWNER,v,v+1,&[ScenePatch::UpdateStyle {id:NodeId(2),style:rect_style(40.0,Color::rgb(40,200,40))}]));
    let damage=engine.render_gpu_to_surface_damaged(OWNER,v+1,&mut gpu,&mut surface,STRIDE,BUDGET).expect("damaged gpu write");
    let second=engine.render_gpu(OWNER,v+1,&mut gpu,BUDGET).expect("reused gpu frame");
    assert_eq!(damage,hull(&first,&second));
    eprintln!("gpu recolour damage: {damage:?}");
    assert_eq!(damage,FrameDamage::Rect {x:0,y:0,width:40,height:30});
    assert!(inside(damage,10,10) && !inside(damage,W-1,H-1));
    assert_matches(&surface,&second,&[(W-1,H-1)]);
    assert_eq!(pixel(&surface,W-1,H-1),&POISON,"outside the damage rect is never written");
    // Widen: damage covers the new columns and the surface converges to the full frame.
    // 加宽：损伤覆盖新增列，Surface 收敛到整帧。
    assert!(engine.apply(OWNER,v+1,v+2,&[ScenePatch::UpdateStyle {id:NodeId(2),style:rect_style(70.0,Color::rgb(40,200,40))}]));
    let mut whole=padded_surface();
    let widened=engine.render_gpu_to_surface_with_damage(OWNER,v+2,&mut gpu,&mut whole,STRIDE,BUDGET).expect("gpu write");
    let third=engine.render_gpu(OWNER,v+2,&mut gpu,BUDGET).expect("reused gpu frame");
    assert_eq!(widened,hull(&second,&third));
    assert!(inside(widened,40,0) && inside(widened,69,29));
    assert_matches(&whole,&third,&[]);
}

#[test]
fn gpu_and_cpu_frames_share_one_damage_baseline() {
    let source=scene();let font=font();let v=source.version();
    let mut engine=Engine::new(OWNER,&source,&font,W,H).expect("native import");
    let mut gpu=gpu();
    let mut cpu=vec![0;ROW*H as usize];
    assert_eq!(engine.render_to_surface_with_damage(OWNER,v,&mut cpu,ROW,BUDGET),Some(FrameDamage::Full));
    // A GPU frame after a scene change is diffed against the CPU frame it replaces.
    // 场景变化后的 GPU 帧与它替换的 CPU 帧做差分。
    assert!(engine.apply(OWNER,v,v+1,&[ScenePatch::UpdateStyle {id:NodeId(2),style:rect_style(40.0,Color::rgb(40,200,40))}]));
    let mut frame=vec![0;ROW*H as usize];
    let damage=engine.render_gpu_to_surface_with_damage(OWNER,v+1,&mut gpu,&mut frame,ROW,BUDGET).expect("gpu write");
    eprintln!("cpu→gpu damage: {damage:?}");
    assert_eq!(damage,hull(&cpu,&frame));
    assert!(inside(damage,0,0) && inside(damage,39,29),"covers the recoloured rect");
    // And a CPU frame after the next change is diffed against that GPU frame.
    // 下一次变化后的 CPU 帧与该 GPU 帧做差分。
    assert!(engine.apply(OWNER,v+1,v+2,&[ScenePatch::UpdateStyle {id:NodeId(2),style:rect_style(40.0,Color::rgb(40,40,200))}]));
    let mut next=vec![0;ROW*H as usize];
    let back=engine.render_to_surface_with_damage(OWNER,v+2,&mut next,ROW,BUDGET).expect("cpu write");
    eprintln!("gpu→cpu damage: {back:?}");
    assert_eq!(back,hull(&frame,&next));
    assert!(inside(back,0,0) && inside(back,39,29));
}

#[test]
fn gpu_rejection_writes_nothing_and_keeps_the_baseline() {
    let source=scene();let font=font();let v=source.version();
    let mut engine=Engine::new(OWNER,&source,&font,W,H).expect("native import");
    let mut gpu=gpu();
    let mut surface=padded_surface();
    assert_eq!(engine.render_gpu_to_surface_damaged(OWNER,v,&mut gpu,&mut surface,STRIDE,BUDGET),Ok(FrameDamage::Full));
    let kept=surface.clone();
    let mut short=vec![PAD;STRIDE*(H as usize-2)+ROW];
    for (who,version,bytes,stride) in [(OWNER+1,v,&mut surface.clone(),STRIDE),(OWNER,v+1,&mut surface.clone(),STRIDE),
        (OWNER,v,&mut surface.clone(),ROW-4),(OWNER,v,&mut short,STRIDE)] {
        let before=bytes.clone();
        assert_eq!(engine.render_gpu_to_surface_damaged(who,version,&mut gpu,bytes,stride,BUDGET),Err(GpuFrameError::Rejected));
        assert_eq!(*bytes,before,"rejected call writes nothing");
    }
    assert_eq!(engine.render_gpu_to_surface_with_damage(OWNER+1,v,&mut gpu,&mut surface,STRIDE,BUDGET),Err(GpuFrameError::Rejected));
    assert_eq!(surface,kept);
    // Rejections do not move the baseline: the next render of the unchanged scene is Unchanged.
    // 拒绝不移动基准：未变化场景的下一次渲染为 Unchanged。
    assert_eq!(engine.render_gpu_to_surface_damaged(OWNER,v,&mut gpu,&mut surface,STRIDE,BUDGET),Ok(FrameDamage::Unchanged));
    assert_eq!(surface,kept);
    // A swapped buffer needs invalidation, after which the write is Full and complete.
    // 换缓冲须先失效，之后为 Full 且完整写入。
    let mut fresh=padded_surface();
    assert!(engine.invalidate_frame(OWNER));
    assert_eq!(engine.render_gpu_to_surface_damaged(OWNER,v,&mut gpu,&mut fresh,STRIDE,BUDGET),Ok(FrameDamage::Full));
    assert_eq!(fresh,kept);
    assert!(engine.close(OWNER));
    assert_eq!(engine.render_gpu_to_surface_damaged(OWNER,v,&mut gpu,&mut fresh,STRIDE,BUDGET),Err(GpuFrameError::Rejected));
}
