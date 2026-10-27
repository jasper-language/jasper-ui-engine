//! Native text: host-ordered multi-font fallback, wrapping, and one shared glyph layout
//! measured by the layout box and painted by the CPU raster.
//! 原生文字：宿主有序多字体回退、折行，以及布局框测量与 CPU 绘制共用的同一份字形布局。
#![cfg(feature="native-blitz-cpu-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::geometry::Rect;
use jasper_ui_engine::native::MAX_INJECTED_FONTS;
use jasper_ui_engine::scene::{Node,NodeId,Scene};
use jasper_ui_engine::style::{Color,Length,Style};

const W:u32=320;
const H:u32=200;
const OWNER:u64=9;
const BUDGET:usize=4*1024*1024;
const ROOT_FILL:[u8;4]=[18,22,30,255];
const TEXT:NodeId=NodeId(2);

/// Two independent injected files: Latin-only first, CJK second; the test never scans the system.
/// 两个独立注入文件：先仅拉丁、后中日韩；测试不扫描系统字体。
fn latin()->Vec<u8> {read("JASPER_UI_TEST_LATIN_FONT")}
fn cjk()->Vec<u8> {read("JASPER_UI_TEST_CJK_FONT")}
fn read(var:&str)->Vec<u8> {
    let path=std::env::var(var).unwrap_or_else(|_|panic!("{var} must name an injected font file"));
    std::fs::read(path).expect("injected font readable")
}

fn scene(text:&str,width:Length,size:f32,color:Color)->Scene {
    Scene::new(Node::container(NodeId(1)).with_style(Style::default()
            .size(Length::Px(W as f32),Length::Px(H as f32)).background(Color::rgb(18,22,30)))
        .child(Node::text(TEXT,text).with_style(Style::default().width(width).text(size,color))))
}

struct Frame {bytes:Vec<u8>,text:Rect}

fn frame_of(source:&Scene,fonts:&[&[u8]])->Frame {
    let mut engine=Engine::with_fonts(OWNER,source,fonts,W,H).expect("native import");
    let v=source.version();
    let mut bytes=vec![0;(W*H*4) as usize];
    assert!(engine.render_to_surface(OWNER,v,&mut bytes,(W*4) as usize,BUDGET));
    Frame {bytes,text:engine.node_bounds(OWNER,v,TEXT).unwrap()}
}

fn frame(fonts:&[&[u8]],text:&str)->Frame {
    frame_of(&scene(text,Length::Px(300.0),16.0,Color::WHITE),fonts)
}

fn px(bytes:&[u8],x:u32,y:u32)->[u8;4] {
    let i=((y*W+x)*4) as usize;[bytes[i],bytes[i+1],bytes[i+2],bytes[i+3]]
}

fn inked(bytes:&[u8],x:u32,y:u32)->bool {px(bytes,x,y)!=ROOT_FILL}

fn ink_columns(f:&Frame)->Vec<u32> {
    let (x0,y0,x1,y1)=span(f.text);
    (x0..x1).filter(|&x|(y0..y1).any(|y|inked(&f.bytes,x,y))).collect()
}

fn ink_rows(f:&Frame)->Vec<u32> {
    (0..H).filter(|&y|(0..W).any(|x|inked(&f.bytes,x,y))).collect()
}

fn span(r:Rect)->(u32,u32,u32,u32) {
    (r.x() as u32,r.y() as u32,(r.x()+r.size.width).ceil() as u32,(r.y()+r.size.height).ceil() as u32)
}

/// Han columns past the end of the same line's Latin-only run.
/// 同一行纯拉丁串末端之后的汉字墨迹列数。
fn han_columns(fonts:&[&[u8]])->usize {
    let latin_end=*ink_columns(&frame(fonts,"Hello")).last().unwrap();
    ink_columns(&frame(fonts,"Hello 你好")).iter().filter(|&&x|x>latin_end+2).count()
}

#[test]
fn fallback_follows_host_order() {
    let (l,c)=(latin(),cjk());
    // Latin text resolves in the first file that covers it: stack frame equals that file alone.
    // 拉丁文字落在首个覆盖它的文件：栈帧与只注入该文件逐字节一致。
    for (stack,alone) in [([&l[..],&c[..]],&l[..]),([&c[..],&l[..]],&c[..])] {
        let (s,a)=(frame(&stack,"Hello"),frame(&[alone],"Hello"));
        assert_eq!(s.text,a.text);
        assert!(s.bytes==a.bytes,"stack must use its first covering file");
    }
    assert!(frame(&[&l,&c],"Hello").bytes!=frame(&[&c,&l],"Hello").bytes,"order must matter");
    // Han falls through the Latin-only file to the second file instead of .notdef boxes.
    // 汉字越过仅拉丁文件回退到第二个文件，而不是缺字框。
    let (fallback,notdef)=(han_columns(&[&l,&c]),han_columns(&[&l]));
    assert!(fallback>=26,"Han ink columns with CJK fallback: {fallback}");
    assert!(notdef<fallback,"Latin-only stack must not render Han glyphs: {notdef} vs {fallback}");
}

#[test]
fn stack_admission_rejects_explicitly() {
    let (l,c)=(latin(),cjk());
    let source=scene("Hello",Length::Px(300.0),16.0,Color::WHITE);
    let garbage:&[u8]=b"not a font file";
    assert!(Engine::with_fonts(OWNER,&source,&[],W,H).is_none());
    assert!(Engine::with_fonts(OWNER,&source,&[garbage],W,H).is_none());
    assert!(Engine::with_fonts(OWNER,&source,&[&l,garbage],W,H).is_none());
    assert!(Engine::with_fonts(OWNER,&source,&[&l,&c],W,H).is_some());
    let full=vec![&l[..];MAX_INJECTED_FONTS];
    let over=vec![&l[..];MAX_INJECTED_FONTS+1];
    assert!(Engine::with_fonts(OWNER,&source,&over,W,H).is_none());
    // Repeated files register one family once; the frame equals the single-file frame.
    // 重复文件只登记一次族；帧与单文件帧一致。
    assert!(frame(&full,"Hello").bytes==frame(&[&l],"Hello").bytes);
}

#[test]
fn wrapped_lines_match_measured_box() {
    let l=latin();
    let color=Color::rgb(230,230,230);
    let line=frame_of(&scene("Jasper",Length::Px(120.0),16.0,color),&[&l]).text.size.height;
    let wrapped=frame_of(&scene("Jasper wraps this sentence across several lines",
        Length::Px(120.0),16.0,color),&[&l]);
    let (x0,y0,x1,y1)=span(wrapped.text);
    let lines=(wrapped.text.size.height/line).round();
    assert!(lines>=3.0,"expected wrapping, got {lines} lines of {line}px");
    assert!((wrapped.text.size.height-lines*line).abs()<0.5,"box height is whole lines");
    assert_eq!(wrapped.text.size.width,120.0);
    // Every measured line slot carries ink, and no ink escapes the measured box.
    // 每个测量行槽都有墨迹，且墨迹不越出测量框。
    for k in 0..lines as u32 {
        let (top,bottom)=(y0+(k as f32*line) as u32,y0+((k+1) as f32*line) as u32);
        assert!((top..bottom).any(|y|(x0..x1).any(|x|inked(&wrapped.bytes,x,y))),"line {k} has no ink");
    }
    for y in 0..H {for x in 0..W {
        if !(x0..x1).contains(&x)||!(y0..y1).contains(&y) {assert!(!inked(&wrapped.bytes,x,y),"ink at {x},{y}");}
    }}
    // A wider box measures fewer lines for the same text.
    // 同一文字在更宽的框中测量出更少的行。
    let wide=frame_of(&scene("Jasper wraps this sentence across several lines",
        Length::Px(300.0),16.0,color),&[&l]);
    assert!(wide.text.size.height<wrapped.text.size.height);
}

#[test]
fn mixed_script_wrap_stays_inside_box() {
    let (l,c)=(latin(),cjk());
    let one=frame_of(&scene("Hello 你好",Length::Px(120.0),16.0,Color::WHITE),&[&l,&c]).text.size.height;
    let f=frame_of(&scene("Hello 你好世界 Jasper 界面布局与绘制一致",Length::Px(120.0),16.0,Color::WHITE),&[&l,&c]);
    let (x0,y0,x1,y1)=span(f.text);
    assert!(f.text.size.height>=2.0*one,"mixed text must wrap: {} vs line {one}",f.text.size.height);
    let rows=ink_rows(&f);
    assert!(rows[0]>=y0&&rows[0]<y0+one as u32,"first ink row inside first line");
    assert!(*rows.last().unwrap()<y1&&*rows.last().unwrap()+one as u32>=y1,"last ink row inside last line");
    for y in 0..H {for x in 0..W {
        if !(x0..x1).contains(&x)||!(y0..y1).contains(&y) {assert!(!inked(&f.bytes,x,y),"ink at {x},{y}");}
    }}
}

#[test]
fn font_size_and_colour_are_applied() {
    let l=latin();
    let colour=Color::rgb(200,40,90);
    let small=frame_of(&scene("Jasper",Length::Px(300.0),16.0,colour),&[&l]);
    let large=frame_of(&scene("Jasper",Length::Px(300.0),32.0,colour),&[&l]);
    let ratio=large.text.size.height/small.text.size.height;
    assert!((1.8..=2.2).contains(&ratio),"line height ratio {ratio}");
    let height=|f:&Frame|{let r=ink_rows(f);(r.last().unwrap()-r[0]+1) as f32};
    let ink_ratio=height(&large)/height(&small);
    assert!((1.7..=2.3).contains(&ink_ratio),"ink height ratio {ink_ratio}");
    let width=|f:&Frame|{let c=ink_columns(f);(c.last().unwrap()-c[0]+1) as f32};
    assert!((1.7..=2.3).contains(&(width(&large)/width(&small))));
    // Fully covered glyph pixels reach the declared colour.
    // 完全覆盖的字形像素达到声明颜色。
    let want=[200,40,90,255];
    assert!(large.bytes.chunks(4).any(|p|p.iter().zip(want).all(|(a,b)|a.abs_diff(b)<=4)));
}
