//! Headless acceptance of the root native CPU path: scene -> layout -> RGBA pixels.
//! 根原生 CPU 路径的无头验收：场景 → 布局 → RGBA 像素。
#![cfg(feature="native-blitz-cpu-pending")]
use jasper_ui_engine::engine::{Engine,LegacyEngine};
use jasper_ui_engine::backend::HeadlessBackend;
use jasper_ui_engine::geometry::{Edges,Rect,Size};
use jasper_ui_engine::scene::{Node,NodeId,Scene};
use jasper_ui_engine::style::{Color,Display,FlexDirection,Length,Style};
use jasper_ui_engine::text::SimpleTextEngine;

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
    let caption=Node::text(NodeId(7),"Jasper 界面").with_style(Style::default().text(20.0,Color::rgb(240,200,80)));
    Scene::new(Node::container(NodeId(1)).with_style(Style::default()
            .size(Length::Px(W as f32),Length::Px(H as f32)).padding(Edges::all(10.0)).gap(8.0)
            .background(Color::rgb(18,22,30)))
        .child(header).child(row).child(caption))
}

/// Injected Latin+Han font; the host owns acquisition, the test never scans the system.
/// 注入覆盖拉丁与汉字的字体；由宿主取得，测试不扫描系统字体。
fn font()->Vec<u8> {
    let path=std::env::var("JASPER_UI_TEST_CJK_FONT")
        .expect("JASPER_UI_TEST_CJK_FONT must name an injected font covering Latin and Han");
    std::fs::read(path).expect("injected CJK font readable")
}

fn render(engine:&mut Engine,version:u64)->Vec<u8> {
    let mut bytes=vec![0;(W*H*4) as usize];
    assert!(engine.render_to_surface(OWNER,version,&mut bytes,(W*4) as usize,BUDGET));
    bytes
}

fn px(bytes:&[u8],x:u32,y:u32)->[u8;4] {
    let i=((y*W+x)*4) as usize;[bytes[i],bytes[i+1],bytes[i+2],bytes[i+3]]
}

fn engine_for(source:&Scene,font:&[u8])->Engine {Engine::new(OWNER,source,font,W,H).expect("native import")}

/// Columns inside `area` whose pixels differ from `fill`, i.e. glyph ink over a known fill.
/// 区域内与已知底色不同的列：即文字墨迹所在列。
fn ink_columns(bytes:&[u8],area:Rect,fill:[u8;4])->Vec<u32> {
    let (x0,y0)=(area.x() as u32,area.y() as u32);
    let (x1,y1)=((area.x()+area.size.width) as u32,(area.y()+area.size.height) as u32);
    (x0..x1).filter(|&x|(y0..y1).any(|y|px(bytes,x,y)!=fill)).collect()
}

const ROOT_FILL:[u8;4]=[18,22,30,255];
const HEADER_FILL:[u8;4]=[36,48,68,255];

#[test]
fn layout_boxes_are_exact() {
    let source=scene();let font=font();let engine=engine_for(&source,&font);let v=source.version();
    let expected=[(1,Rect::new(0.0,0.0,320.0,200.0)),(2,Rect::new(10.0,10.0,300.0,40.0)),
        (3,Rect::new(16.0,16.0,288.0,24.0)),(4,Rect::new(10.0,58.0,300.0,60.0)),
        (5,Rect::new(10.0,58.0,100.0,60.0)),(6,Rect::new(120.0,58.0,120.0,60.0)),
        (7,Rect::new(10.0,126.0,300.0,29.0))];
    for (id,rect) in expected {assert_eq!(engine.node_bounds(OWNER,v,NodeId(id)),Some(rect),"node {id}");}
    assert_eq!(engine.node_bounds(OWNER,v,NodeId(99)),None);
    assert_eq!(engine.node_bounds(OWNER+1,v,NodeId(1)),None);
    assert_eq!(engine.node_bounds(OWNER,v+1,NodeId(1)),None);
}

#[test]
fn fills_borders_and_backgrounds_sample_correctly() {
    let source=scene();let font=font();let mut engine=engine_for(&source,&font);
    let bytes=render(&mut engine,source.version());
    let near=|a:[u8;4],b:[u8;4]|a.iter().zip(b).all(|(x,y)|x.abs_diff(y)<=2);
    for (x,y,want) in [(1,1,ROOT_FILL),(315,195,ROOT_FILL),(250,90,ROOT_FILL),
        (11,11,[200,60,60,255]),(308,48,[200,60,60,255]),(15,15,HEADER_FILL),(300,40,HEADER_FILL),
        (10,58,[255,255,255,255]),(109,117,[255,255,255,255]),(15,62,[46,160,67,255]),(60,90,[46,160,67,255]),
        (115,90,ROOT_FILL),(120,58,[30,110,220,255]),(239,117,[30,110,220,255]),(240,90,ROOT_FILL)] {
        assert!(near(px(&bytes,x,y),want),"pixel {x},{y}: {:?} != {want:?}",px(&bytes,x,y));
    }
    assert_eq!(render(&mut engine,source.version()),bytes,"repeated frame must be identical");
}

#[test]
fn latin_and_han_text_leave_ink_only_where_glyphs_are() {
    let font=font();
    let full=scene();let mut engine=engine_for(&full,&font);let v=full.version();
    let bytes=render(&mut engine,v);
    let header_text=engine.node_bounds(OWNER,v,NodeId(3)).unwrap();
    let caption=engine.node_bounds(OWNER,v,NodeId(7)).unwrap();
    let header_ink=ink_columns(&bytes,header_text,HEADER_FILL);
    let caption_ink=ink_columns(&bytes,caption,ROOT_FILL);
    assert!(!header_ink.is_empty()&&!caption_ink.is_empty());
    // Ink starts at the line start and stops well before the box end: no ink past the glyph run.
    // 墨迹从行首开始、远早于盒末端结束：字形之外无墨迹。
    assert!(header_ink[0]<header_text.x() as u32+4&&*header_ink.last().unwrap()<200);
    assert!(caption_ink[0]<caption.x() as u32+4&&*caption_ink.last().unwrap()<200);
    // Han glyphs add ink beyond the Latin-only run of the same line.
    // 汉字在同一行仅拉丁文本墨迹之外新增墨迹。
    let mut latin=scene();
    latin.apply_batch_at(latin.version(),&[jasper_ui_engine::scene::ScenePatch::UpdateText{
        id:NodeId(3),text:"Hello".into()}]).unwrap();
    let mut latin_engine=engine_for(&latin,&font);
    let latin_bytes=render(&mut latin_engine,latin.version());
    let latin_ink=ink_columns(&latin_bytes,header_text,HEADER_FILL);
    let latin_end=*latin_ink.last().unwrap();
    let han_columns=header_ink.iter().filter(|&&x|x>latin_end+2).count();
    // Measured: 32 columns for two Han glyphs at 16px; a .notdef pair from a Latin-only font gives 18.
    // 实测：16px 两个汉字 32 列；仅拉丁字体的两个缺字框为 18 列。
    assert!(han_columns>=26,"Han ink columns past Latin run: {han_columns}");
    // Glyph colour reaches the declared text colour on at least one fully covered pixel.
    // 至少一个完全覆盖像素达到声明的文字颜色。
    let covered=|area:Rect,want:[u8;4]|(area.y() as u32..(area.y()+area.size.height) as u32)
        .flat_map(|y|(area.x() as u32..(area.x()+area.size.width) as u32).map(move|x|(x,y)))
        .any(|(x,y)|px(&bytes,x,y).iter().zip(want).all(|(a,b)|a.abs_diff(b)<=8));
    assert!(covered(header_text,[255,255,255,255]));
    assert!(covered(caption,[240,200,80,255]));
}

/// Legacy comparison is recorded, not required to match; this pins the documented table.
/// 旧引擎对照只记录、不要求一致；此测试锁定文档中的差异表。
#[test]
fn legacy_layout_difference_table() {
    let source=scene();let font=font();let engine=engine_for(&source,&font);
    let legacy=LegacyEngine::new(Size::new(W as f32,H as f32),HeadlessBackend::new(W,H),Box::new(SimpleTextEngine));
    let tree=legacy.layout(&source).unwrap();
    let mut differing=Vec::new();
    for id in 1..=7 {
        let native=engine.node_bounds(OWNER,source.version(),NodeId(id)).unwrap();
        let old=tree.find(NodeId(id)).unwrap().rect;
        if native!=old {differing.push((id,old.size.height,native.size.height));}
        assert_eq!((native.x(),native.y(),native.size.width),(old.x(),old.y(),old.size.width),"node {id}");
    }
    assert_eq!(differing,vec![(3,20.0,24.0),(7,25.0,29.0)]);
}
