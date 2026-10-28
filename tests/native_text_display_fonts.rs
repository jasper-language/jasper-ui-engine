//! TextDisplayView host-ordered multi-font fallback: the same admission as the scene engine,
//! with measurement, hit testing and painting sharing the one Parley layout.
//! TextDisplayView 宿主有序多字体回退：与场景引擎同一准入，测量、命中与绘制共用同一布局。
#![cfg(feature="native-blitz-cpu-pending")]
use jasper_ui_engine::native::MAX_INJECTED_FONTS;
use jasper_ui_engine::native::host::{TextDisplayView,UiInputBinding,UiInputContext};
use jasper_ui_engine::scene::NodeId;

const W:u32=320;
const H:u32=120;
const OWNER:u64=10;
const BUDGET:usize=4*1024*1024;
const CONTEXT:UiInputContext=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};

/// Two independent injected files: Latin-only first, CJK second; the test never scans the system.
/// 两个独立注入文件：先仅拉丁、后中日韩；测试不扫描系统字体。
fn latin()->Vec<u8> {read("JASPER_UI_TEST_LATIN_FONT")}
fn cjk()->Vec<u8> {read("JASPER_UI_TEST_CJK_FONT")}
fn read(var:&str)->Vec<u8> {
    let path=std::env::var(var).unwrap_or_else(|_|panic!("{var} must name an injected font file"));
    std::fs::read(path).expect("injected font readable")
}

fn view(fonts:&[&[u8]],text:&str)->TextDisplayView {
    TextDisplayView::with_fonts(OWNER,CONTEXT,fonts,text,W,H).expect("font stack admitted")
}

fn frame(fonts:&[&[u8]],text:&str)->Vec<u8> {
    let bytes=view(fonts,text).render_cpu(OWNER,CONTEXT,BUDGET).expect("frame");
    assert_eq!(bytes.len(),(W*H*4) as usize);
    bytes
}

/// Background is the first pixel (the text starts inside the body margin).
/// 背景取首像素（文字位于 body 外边距之内）。
fn ink_columns(bytes:&[u8])->Vec<u32> {
    let bg=&bytes[0..4];
    (0..W).filter(|&x|(0..H).any(|y|{let i=((y*W+x)*4) as usize;&bytes[i..i+4]!=bg})).collect()
}

#[test]
fn fallback_follows_host_order() {
    let (latin,cjk)=(latin(),cjk());
    let both:&[&[u8]]=&[&latin,&cjk];
    let latin_text="Jasper display";
    // Latin text resolves to the first covering file: same frame as the Latin file alone.
    // 拉丁文字取栈中首个覆盖文件：帧与只注入拉丁一致。
    assert_eq!(frame(both,latin_text),frame(&[&latin],latin_text));
    assert_eq!(frame(&[&cjk,&latin],latin_text),frame(&[&cjk],latin_text));
    assert_ne!(frame(both,latin_text),frame(&[&cjk,&latin],latin_text));
    // Han falls through the Latin-only file to the CJK file instead of a missing-glyph box.
    // 汉字越过仅拉丁文件回退到 CJK，而非缺字框。
    let han="中文显示";
    let fallback=frame(both,han);
    let tofu=frame(&[&latin],han);
    assert_ne!(fallback,tofu);
    assert_eq!(fallback,frame(&[&cjk],han));
    let (ink,tofu_ink)=(ink_columns(&fallback).len(),ink_columns(&tofu).len());
    println!("han ink columns: fallback {ink}, latin-only {tofu_ink}");
    assert!(ink>tofu_ink,"CJK glyphs must be wider than missing-glyph boxes ({ink} vs {tofu_ink})");
}

#[test]
fn stack_admission_rejects_explicitly() {
    let (latin,cjk)=(latin(),cjk());
    let junk=vec![7u8;512];
    assert!(TextDisplayView::with_fonts(OWNER,CONTEXT,&[],"x",W,H).is_none());
    assert!(TextDisplayView::with_fonts(OWNER,CONTEXT,&[&junk],"x",W,H).is_none());
    assert!(TextDisplayView::with_fonts(OWNER,CONTEXT,&[&latin,&junk],"x",W,H).is_none());
    assert!(TextDisplayView::with_fonts(OWNER,CONTEXT,&[&junk,&cjk],"x",W,H).is_none());
    let over:Vec<&[u8]>=(0..=MAX_INJECTED_FONTS).map(|_|latin.as_slice()).collect();
    assert!(TextDisplayView::with_fonts(OWNER,CONTEXT,&over,"x",W,H).is_none());
    // A full stack of one repeated file registers its families once and matches the single file.
    // 满额重复同一文件只登记一次族，帧与单文件一致。
    let full:Vec<&[u8]>=(0..MAX_INJECTED_FONTS).map(|_|latin.as_slice()).collect();
    assert_eq!(frame(&full,"repeat"),frame(&[&latin],"repeat"));
    // The single-font constructor is unchanged and equals a one-file stack.
    // 单字体构造不变，与单文件栈一致。
    let single=TextDisplayView::new(OWNER,CONTEXT,&latin,"single",W,H).render_cpu(OWNER,CONTEXT,BUDGET).unwrap();
    assert_eq!(single,frame(&[&latin],"single"));
    // Host ownership still gates a multi-font view.
    // 多字体视图仍受宿主归属检查。
    assert!(view(&[&latin,&cjk],"owned").render_cpu(OWNER+1,CONTEXT,BUDGET).is_none());
}

#[test]
fn hit_uses_the_fallback_layout() {
    let (latin,cjk)=(latin(),cjk());
    let text="Ab中文字";
    let mut binding=UiInputBinding::new(OWNER,1);
    let ticket=binding.bind(OWNER,CONTEXT).expect("binding");
    let fallback=view(&[&latin,&cjk],text);
    let tofu=view(&[&latin],text);
    // Scan one text row: offsets are UTF-8 boundaries and never decrease left to right.
    // 扫描一行：偏移均为 UTF-8 边界且自左向右不减。
    let scan=|v:&TextDisplayView|->Vec<usize> {
        let mut hits=Vec::new();
        for x in 0..200 {
            if let Some(o)=v.hit(&binding,OWNER,ticket,x as f32,14.0) {
                assert!(text.is_char_boundary(o),"offset {o} must be a char boundary");
                if let Some(&last)=hits.last() {assert!(o>=last,"hit offsets must be monotonic");}
                hits.push(o);
            }
        }
        hits
    };
    let (a,b)=(scan(&fallback),scan(&tofu));
    assert!(a.len()>=100,"row must hit text across most of its width");
    assert_eq!(a.first(),Some(&0));
    assert_eq!(a.last(),Some(&text.len()));
    // The x where the hit first passes the Latin prefix agrees (shared Latin glyphs),
    // while the end of the Han run moves with glyph width: hit follows the painted layout.
    // 越过拉丁前缀的位置一致（拉丁字形相同），汉字段终点随字形宽度变化：命中跟随绘制布局。
    let first_past=|h:&[usize],o:usize|h.iter().position(|&v|v>=o);
    assert_eq!(first_past(&a,2),first_past(&b,2));
    assert_ne!(first_past(&a,text.len()),first_past(&b,text.len()));
}
