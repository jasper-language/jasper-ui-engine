//! Native font weight and slant: Framework declares Style.font_weight/font_style, the injected
//! faces of one family are matched by Parley/fontique, and out-of-range weights are rejected.
//! 原生字重与倾斜：Framework 声明 Style.font_weight/font_style，由 Parley/fontique 在宿主注入的
//! 同族字面中匹配；越界字重被拒绝。
#![cfg(feature="native-blitz-cpu-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::scene::{Node,NodeId,Scene,ScenePatch};
use jasper_ui_engine::style::{Color,FontStyle,Length,Style};

const W:u32=320;
const H:u32=80;
const OWNER:u64=9;
const BUDGET:usize=4*1024*1024;
const ROOT_FILL:[u8;4]=[18,22,30,255];
const TEXT:NodeId=NodeId(2);
const SAMPLE:&str="Jasper weight 700";

/// Faces of one family injected as separate files; the test never scans the system.
/// 同一字族的各字面作为独立文件注入；测试不扫描系统字体。
fn regular()->Vec<u8> {read("JASPER_UI_TEST_LATIN_FONT")}
fn bold()->Vec<u8> {read("JASPER_UI_TEST_BOLD_FONT")}
fn italic()->Vec<u8> {read("JASPER_UI_TEST_ITALIC_FONT")}
fn read(var:&str)->Vec<u8> {
    let path=std::env::var(var).unwrap_or_else(|_|panic!("{var} must name an injected font file"));
    std::fs::read(path).expect("injected font readable")
}

fn text_style(weight:u16,slant:FontStyle)->Style {
    Style::default().width(Length::Px(300.0)).text(18.0,Color::WHITE).font(weight,slant)
}

fn scene(weight:u16,slant:FontStyle)->Scene {
    Scene::new(Node::container(NodeId(1)).with_style(Style::default()
            .size(Length::Px(W as f32),Length::Px(H as f32)).background(Color::rgb(18,22,30)))
        .child(Node::text(TEXT,SAMPLE).with_style(text_style(weight,slant))))
}

fn render(engine:&mut Engine,version:u64)->Vec<u8> {
    let mut bytes=vec![0;(W*H*4) as usize];
    assert!(engine.render_to_surface(OWNER,version,&mut bytes,(W*4) as usize,BUDGET));
    bytes
}

fn frame(fonts:&[&[u8]],weight:u16,slant:FontStyle)->Vec<u8> {
    let source=scene(weight,slant);
    let mut engine=Engine::with_fonts(OWNER,&source,fonts,W,H).expect("native import");
    render(&mut engine,source.version())
}

fn ink(bytes:&[u8])->usize {bytes.chunks_exact(4).filter(|p|*p!=ROOT_FILL).count()}

#[test]
fn weight_selects_the_matching_injected_face() {
    let (regular,bold)=(regular(),bold());
    for stack in [[&regular[..],&bold[..]],[&bold[..],&regular[..]]] {
        // Matching is by declared weight, not by injection order.
        // 按声明字重匹配，与注入顺序无关。
        assert_eq!(frame(&stack,700,FontStyle::Normal),frame(&[&bold],700,FontStyle::Normal));
        assert_eq!(frame(&stack,400,FontStyle::Normal),frame(&[&regular],400,FontStyle::Normal));
    }
    let (heavy,plain)=(frame(&[&regular,&bold],700,FontStyle::Normal),frame(&[&regular,&bold],400,FontStyle::Normal));
    assert_ne!(heavy,plain);
    let (heavy_ink,plain_ink)=(ink(&heavy),ink(&plain));
    println!("ink pixels: weight 700 {heavy_ink}, weight 400 {plain_ink}");
    assert!(heavy_ink>plain_ink,"bold face must cover more pixels ({heavy_ink} vs {plain_ink})");
    // The default style is weight 400 / Normal: identical to the explicit declaration.
    // 默认样式即 400 / Normal：与显式声明一致。
    let source=Scene::new(Node::container(NodeId(1)).with_style(Style::default()
            .size(Length::Px(W as f32),Length::Px(H as f32)).background(Color::rgb(18,22,30)))
        .child(Node::text(TEXT,SAMPLE).with_style(Style::default().width(Length::Px(300.0)).text(18.0,Color::WHITE))));
    let mut engine=Engine::with_fonts(OWNER,&source,&[&regular,&bold],W,H).expect("native import");
    assert_eq!(render(&mut engine,source.version()),plain);
}

#[test]
fn italic_selects_the_oblique_injected_face() {
    let (regular,italic)=(regular(),italic());
    for stack in [[&regular[..],&italic[..]],[&italic[..],&regular[..]]] {
        assert_eq!(frame(&stack,400,FontStyle::Italic),frame(&[&italic],400,FontStyle::Italic));
        assert_eq!(frame(&stack,400,FontStyle::Normal),frame(&[&regular],400,FontStyle::Normal));
    }
    assert_ne!(frame(&[&regular,&italic],400,FontStyle::Italic),frame(&[&regular,&italic],400,FontStyle::Normal));
}

#[test]
fn out_of_range_weight_is_rejected_and_updates_repaint() {
    let (regular,bold)=(regular(),bold());
    let fonts:&[&[u8]]=&[&regular,&bold];
    for weight in [0,1001,u16::MAX] {
        assert!(Engine::with_fonts(OWNER,&scene(weight,FontStyle::Normal),fonts,W,H).is_none(),"weight {weight} admitted");
    }
    for weight in [1,1000] {
        assert!(Engine::with_fonts(OWNER,&scene(weight,FontStyle::Normal),fonts,W,H).is_some(),"weight {weight} rejected");
    }
    let source=scene(400,FontStyle::Normal);
    let v=source.version();
    let mut engine=Engine::with_fonts(OWNER,&source,fonts,W,H).expect("native import");
    let plain=render(&mut engine,v);
    // A rejected patch changes nothing: same version, same frame.
    // 被拒补丁不产生任何变化：版本与帧均不变。
    assert!(!engine.apply(OWNER,v,v+1,&[ScenePatch::UpdateStyle {id:TEXT,style:text_style(0,FontStyle::Normal)}]));
    assert!(engine.invalidate_frame(OWNER));
    assert_eq!(render(&mut engine,v),plain);
    // A weight update repaints with the bold face: equal to a fresh bold import.
    // 字重更新后以粗体字面重绘：等于新导入的粗体场景。
    assert!(engine.apply(OWNER,v,v+1,&[ScenePatch::UpdateStyle {id:TEXT,style:text_style(700,FontStyle::Normal)}]));
    assert_eq!(engine.needs_frame(OWNER),Some(true));
    assert_eq!(render(&mut engine,v+1),frame(fonts,700,FontStyle::Normal));
}
