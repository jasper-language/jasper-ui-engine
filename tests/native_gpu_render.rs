//! Headless acceptance of the root native GPU path (Vello hybrid on wgpu) against the CPU path.
//! 根原生 GPU 路径（wgpu 上的 Vello hybrid）无头验收，以 CPU 路径为对照。
#![cfg(feature="gpu-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::geometry::Edges;
use jasper_ui_engine::native::host::{FrameDamage,GpuFrameError,GpuRenderer};
use jasper_ui_engine::scene::{Node,NodeId,Scene};
use jasper_ui_engine::style::{Color,Display,FlexDirection,Length,Style};

const W:u32=320;
const H:u32=200;
const OWNER:u64=7;
const BUDGET:usize=4*1024*1024;

fn scene()->Scene {
    let header=Node::container(NodeId(2)).with_style(Style::default().display(Display::Block)
            .height(Length::Px(40.0)).padding(Edges::all(4.0))
            .background(Color::rgb(36,48,68)).border(2.0,Color::rgb(200,60,60)))
        .child(Node::text(NodeId(3),"Hello 你好").with_style(Style::default().text(16.0,Color::WHITE)));
    let row=Node::container(NodeId(4)).with_style(Style::default().direction(FlexDirection::Row)
            .height(Length::Px(60.0)).gap(10.0))
        .child(Node::rect(NodeId(5)).with_style(Style::default().size(Length::Px(100.0),Length::Px(60.0))
            .background(Color::rgb(46,160,67)).border(1.0,Color::WHITE)))
        .child(Node::rect(NodeId(6)).with_style(Style::default().size(Length::Px(120.0),Length::Px(60.0))
            .background(Color::rgb(30,110,220))));
    Scene::new(Node::container(NodeId(1)).with_style(Style::default()
            .size(Length::Px(W as f32),Length::Px(H as f32)).padding(Edges::all(10.0)).gap(8.0)
            .background(Color::rgb(18,22,30)))
        .child(header).child(row))
}

/// Injected Latin+Han font; the host owns acquisition, the test never scans the system.
/// 注入覆盖拉丁与汉字的字体；由宿主取得，测试不扫描系统字体。
fn font()->Vec<u8> {
    let path=std::env::var("JASPER_UI_TEST_CJK_FONT")
        .expect("JASPER_UI_TEST_CJK_FONT must name an injected font covering Latin and Han");
    std::fs::read(path).expect("injected CJK font readable")
}

/// A missing adapter fails the test: GPU acceptance is never reported without a device.
/// 没有适配器则测试失败：没有设备就不报告 GPU 验收。
fn gpu()->GpuRenderer {
    let gpu=GpuRenderer::new_headless().expect("a wgpu adapter is required for GPU acceptance");
    eprintln!("gpu adapter: {:?}",gpu.adapter_info());
    gpu
}

fn px(bytes:&[u8],x:u32,y:u32)->[u8;4] {
    let i=((y*W+x)*4) as usize;[bytes[i],bytes[i+1],bytes[i+2],bytes[i+3]]
}

fn cpu_frame(engine:&mut Engine,version:u64)->Vec<u8> {
    let mut bytes=vec![0;(W*H*4) as usize];
    assert!(engine.render_to_surface(OWNER,version,&mut bytes,(W*4) as usize,BUDGET));
    bytes
}

#[test]
fn gpu_frame_matches_cpu_frame() {
    let source=scene();let font=font();let v=source.version();
    let mut engine=Engine::new(OWNER,&source,&font,W,H).expect("native import");
    let mut gpu=gpu();
    let cpu=cpu_frame(&mut engine,v);
    // Frames share one cache: force a GPU repaint instead of reusing the CPU frame.
    // 帧共用缓存：强制 GPU 重绘，而非复用 CPU 帧。
    assert!(engine.invalidate_frame(OWNER));
    let frame=engine.render_gpu(OWNER,v,&mut gpu,BUDGET).expect("gpu frame");
    assert_eq!(frame.len(),cpu.len());
    // Opaque fills and borders are exact on both paths.
    // 不透明填充与边框在两条路径上逐像素一致。
    for (x,y,expected) in [(2,2,[18,22,30,255]),(12,30,[36,48,68,255]),(10,30,[200,60,60,255]),
        (60,90,[46,160,67,255]),(10,90,[255,255,255,255]),(180,90,[30,110,220,255]),(300,190,[18,22,30,255])] {
        assert_eq!(px(&frame,x,y),expected,"gpu pixel ({x},{y})");
        assert_eq!(px(&cpu,x,y),expected,"cpu pixel ({x},{y})");
    }
    let mut differing=0usize;let mut max_delta=0u8;
    for (a,b) in frame.chunks_exact(4).zip(cpu.chunks_exact(4)) {
        let delta=a.iter().zip(b).map(|(p,q)|p.abs_diff(*q)).max().unwrap_or(0);
        if delta>0 {differing+=1;}
        max_delta=max_delta.max(delta);
    }
    eprintln!("gpu vs cpu: {differing}/{} pixels differ, max channel delta {max_delta}",W*H);
    // Differences are confined to anti-aliased glyph edges inside the header text box.
    // 差异只允许出现在标题文字框内的抗锯齿字形边缘。
    for (index,(a,b)) in frame.chunks_exact(4).zip(cpu.chunks_exact(4)).enumerate() {
        if a!=b {
            let (x,y)=((index as u32)%W,(index as u32)/W);
            assert!((16..304).contains(&x) && (16..40).contains(&y),"difference outside text at ({x},{y}): {a:?} vs {b:?}");
        }
    }
    assert!(max_delta<=1,"channel delta {max_delta} exceeds 1");
    assert!(differing<(W*H/100) as usize,"more than 1% of pixels differ");
    // Glyph ink exists on the GPU frame in the same columns as on the CPU frame.
    // GPU 帧上字形墨迹存在，且所在列与 CPU 帧一致。
    let fill=[36,48,68,255];
    let ink=|bytes:&[u8]|(16..304).filter(|&x|(16..40).any(|y|px(bytes,x,y)!=fill)).collect::<Vec<u32>>();
    let (gpu_ink,cpu_ink)=(ink(&frame),ink(&cpu));
    eprintln!("ink columns: gpu {} cpu {}",gpu_ink.len(),cpu_ink.len());
    assert!(gpu_ink.len()>40);
    assert_eq!(gpu_ink.first(),cpu_ink.first());assert_eq!(gpu_ink.last(),cpu_ink.last());
}

#[test]
fn gpu_surface_write_preserves_padding_and_rejects_before_gpu_work() {
    let source=scene();let font=font();let v=source.version();
    let mut engine=Engine::new(OWNER,&source,&font,W,H).expect("native import");
    let mut gpu=gpu();
    let compact=engine.render_gpu(OWNER,v,&mut gpu,BUDGET).expect("gpu frame");
    let stride=(W*4+16) as usize;
    let mut surface=vec![0xAB;stride*H as usize];
    engine.render_gpu_to_surface(OWNER,v,&mut gpu,&mut surface,stride,BUDGET).expect("surface write");
    for y in 0..H as usize {
        let row=&surface[y*stride..y*stride+(W*4) as usize];
        assert_eq!(row,&compact[y*(W*4) as usize..(y+1)*(W*4) as usize],"row {y}");
        assert!(surface[y*stride+(W*4) as usize..(y+1)*stride].iter().all(|&b|b==0xAB),"padding row {y}");
    }
    let mut short=vec![0;stride*(H as usize-1)];
    assert_eq!(engine.render_gpu_to_surface(OWNER,v,&mut gpu,&mut short,stride,BUDGET),Err(GpuFrameError::Rejected));
    assert!(short.iter().all(|&b|b==0));
    assert_eq!(engine.render_gpu(OWNER+1,v,&mut gpu,BUDGET),Err(GpuFrameError::Rejected));
    assert_eq!(engine.render_gpu(OWNER,v+1,&mut gpu,BUDGET),Err(GpuFrameError::Rejected));
    assert_eq!(engine.render_gpu(OWNER,v,&mut gpu,(W*H*4) as usize-1),Err(GpuFrameError::Rejected));
}

#[test]
fn gpu_frame_schedules_and_resizes() {
    let source=scene();let font=font();let v=source.version();
    let mut engine=Engine::new(OWNER,&source,&font,W,H).expect("native import");
    let mut gpu=gpu();
    assert_eq!(engine.needs_frame(OWNER),Some(true));
    let frame=engine.render_gpu(OWNER,v,&mut gpu,BUDGET).expect("gpu frame");
    assert_eq!(engine.needs_frame(OWNER),Some(false));
    // The GPU frame is the last produced frame: an unchanged CPU render reuses it.
    // GPU 帧即最近产出的帧：未变化时 CPU 渲染复用它。
    let mut bytes=vec![0;(W*H*4) as usize];
    assert_eq!(engine.render_to_surface_with_damage(OWNER,v,&mut bytes,(W*4) as usize,BUDGET),Some(FrameDamage::Unchanged));
    assert_eq!(bytes,frame);
    assert!(engine.set_viewport(OWNER,160,100,1.0));
    let small=engine.render_gpu(OWNER,v,&mut gpu,BUDGET).expect("resized gpu frame");
    assert_eq!(small.len(),160*100*4);
    assert_eq!(&small[..4],&[18,22,30,255]);
    assert!(engine.close(OWNER));
    assert_eq!(engine.render_gpu(OWNER,v,&mut gpu,BUDGET),Err(GpuFrameError::Rejected));
}
