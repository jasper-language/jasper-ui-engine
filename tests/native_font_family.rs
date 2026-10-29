//! Native font family selection: Framework declares Style.font_family, Parley/fontique selects
//! the named injected family, glyphs it lacks fall back in host order, and a name no injected
//! font registered is rejected instead of silently replaced.
//! 原生字体族选择：Framework 声明 Style.font_family，由 Parley/fontique 选择宿主注入的同名族，
//! 所缺字形按宿主顺序回退；任何注入字体都未登记的族名被拒绝，不静默替换。
#![cfg(feature="native-blitz-cpu-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::scene::{Node,NodeId,Scene,ScenePatch};
use jasper_ui_engine::style::{Color,Length,Style};

const W:u32=320;
const H:u32=80;
const OWNER:u64=9;
const BUDGET:usize=4*1024*1024;
const TEXT:NodeId=NodeId(2);
const LATIN:&str="Jasper family 42";
const HAN:&str="汉字回退";
const SANS:&str="DejaVu Sans";
// One collection file registers several families; selection works inside it by name.
// 同一字体集文件登记多个族；可在其中按名选择。
const CJK_SANS:&str="Noto Sans CJK SC";
const CJK_MONO:&str="Noto Sans Mono CJK SC";

fn latin()->Vec<u8> {read("JASPER_UI_TEST_LATIN_FONT")}
fn cjk()->Vec<u8> {read("JASPER_UI_TEST_CJK_FONT")}
fn read(var:&str)->Vec<u8> {
    let path=std::env::var(var).unwrap_or_else(|_|panic!("{var} must name an injected font file"));
    std::fs::read(path).expect("injected font readable")
}

fn text_style(family:Option<&str>)->Style {
    let style=Style::default().width(Length::Px(300.0)).text(18.0,Color::WHITE);
    match family {Some(name)=>style.family(name),None=>style}
}

fn scene(text:&str,family:Option<&str>)->Scene {
    Scene::new(Node::container(NodeId(1)).with_style(Style::default()
            .size(Length::Px(W as f32),Length::Px(H as f32)).background(Color::rgb(18,22,30)))
        .child(Node::text(TEXT,text).with_style(text_style(family))))
}

fn render(engine:&mut Engine,version:u64)->Vec<u8> {
    let mut bytes=vec![0;(W*H*4) as usize];
    assert!(engine.render_to_surface(OWNER,version,&mut bytes,(W*4) as usize,BUDGET));
    bytes
}

fn frame(fonts:&[&[u8]],text:&str,family:Option<&str>)->Vec<u8> {
    let source=scene(text,family);
    let mut engine=Engine::with_fonts(OWNER,&source,fonts,W,H).expect("native import");
    render(&mut engine,source.version())
}

#[test]
fn family_is_selected_by_name_not_by_injection_order() {
    let (latin,cjk)=(latin(),cjk());
    for stack in [[&latin[..],&cjk[..]],[&cjk[..],&latin[..]]] {
        assert_eq!(frame(&stack,LATIN,Some(SANS)),frame(&[&latin],LATIN,Some(SANS)));
        assert_eq!(frame(&stack,LATIN,Some(CJK_MONO)),frame(&[&cjk],LATIN,Some(CJK_MONO)));
        assert_eq!(frame(&stack,LATIN,Some(CJK_SANS)),frame(&[&cjk],LATIN,Some(CJK_SANS)));
    }
    // No declared family keeps the host order: the first injected file wins.
    // 未声明族时沿用宿主顺序：先注入的文件优先。
    assert_eq!(frame(&[&latin,&cjk],LATIN,None),frame(&[&latin],LATIN,Some(SANS)));
    let (sans,mono,cjk_sans)=(frame(&[&latin,&cjk],LATIN,Some(SANS)),
        frame(&[&latin,&cjk],LATIN,Some(CJK_MONO)),frame(&[&latin,&cjk],LATIN,Some(CJK_SANS)));
    assert_ne!(sans,mono);assert_ne!(sans,cjk_sans);assert_ne!(mono,cjk_sans);
}

#[test]
fn glyphs_missing_from_the_family_fall_back_in_host_order() {
    let (latin,cjk)=(latin(),cjk());
    // DejaVu Sans has no Han glyphs: they come from the host-ordered stack, never a tofu box.
    // DejaVu Sans 无汉字：由宿主顺序栈回退，而非缺字框。
    let fallback=frame(&[&cjk,&latin],HAN,Some(SANS));
    assert_eq!(fallback,frame(&[&cjk],HAN,None));
    assert_ne!(fallback,frame(&[&latin],HAN,Some(SANS)));
    assert_eq!(frame(&[&latin,&cjk],HAN,Some(SANS)),frame(&[&latin,&cjk],HAN,None));
}

#[test]
fn unregistered_or_unsafe_names_are_rejected_and_updates_repaint() {
    let (latin,cjk)=(latin(),cjk());
    let fonts:&[&[u8]]=&[&latin,&cjk];
    let long="x".repeat(129);
    for name in ["Noto Sans CJK","dejavu sans","DejaVu Sans ","","DejaVu\"Sans","DejaVu\\Sans","DejaVu\nSans",long.as_str()] {
        assert!(Engine::with_fonts(OWNER,&scene(LATIN,Some(name)),fonts,W,H).is_none(),"family {name:?} admitted");
    }
    // Admission is against the injected stack of this view, not every font the host owns.
    // 准入针对本视图注入的字体栈，而非宿主拥有的全部字体。
    assert!(Engine::with_fonts(OWNER,&scene(LATIN,Some(CJK_SANS)),&[&latin],W,H).is_none());
    let source=scene(LATIN,None);
    let v=source.version();
    let mut engine=Engine::with_fonts(OWNER,&source,fonts,W,H).expect("native import");
    let plain=render(&mut engine,v);
    for name in ["Noto Sans CJK","DejaVu\"Sans"] {
        assert!(!engine.apply(OWNER,v,v+1,&[ScenePatch::UpdateStyle {id:TEXT,style:text_style(Some(name))}]));
        let child=Node::text(NodeId(3),LATIN).with_style(text_style(Some(name)));
        assert!(!engine.apply(OWNER,v,v+1,&[ScenePatch::Insert {parent:NodeId(1),index:1,node:child}]));
    }
    assert!(engine.invalidate_frame(OWNER));
    assert_eq!(render(&mut engine,v),plain);
    assert!(engine.apply(OWNER,v,v+1,&[ScenePatch::UpdateStyle {id:TEXT,style:text_style(Some(CJK_MONO))}]));
    assert_eq!(engine.needs_frame(OWNER),Some(true));
    assert_eq!(render(&mut engine,v+1),frame(fonts,LATIN,Some(CJK_MONO)));
    // Clearing the family returns to the host order.
    // 清除族名后回到宿主顺序。
    assert!(engine.apply(OWNER,v+1,v+2,&[ScenePatch::UpdateStyle {id:TEXT,style:text_style(None)}]));
    assert_eq!(render(&mut engine,v+2),plain);
}
