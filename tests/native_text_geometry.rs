//! Native text hit/caret geometry through the root entry: source UTF-8 offsets map to caret
//! rectangles from the same layout that paints the text, and points map back to offsets.
//! 根入口原生文字命中/光标几何：源 UTF-8 偏移映射到与绘制同一布局的光标矩形，点映射回偏移。
#![cfg(feature="native-blitz-cpu-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::geometry::{Edges,Rect};
use jasper_ui_engine::scene::{Node,NodeId,Scene,ScenePatch};
use jasper_ui_engine::semantics::Semantics;
use jasper_ui_engine::style::{Color,Length,Style};

const W:u32=320;
const H:u32=200;
const OWNER:u64=11;
const BUDGET:usize=4*1024*1024;
const ROOT_FILL:[u8;4]=[18,22,30,255];
const TEXT:NodeId=NodeId(2);

fn latin()->Vec<u8> {read("JASPER_UI_TEST_LATIN_FONT")}
fn cjk()->Vec<u8> {read("JASPER_UI_TEST_CJK_FONT")}
fn read(var:&str)->Vec<u8> {
    let path=std::env::var(var).unwrap_or_else(|_|panic!("{var} must name an injected font file"));
    std::fs::read(path).expect("injected font readable")
}

fn scene_with(text:&str,style:Style)->Scene {
    Scene::new(Node::container(NodeId(1)).with_style(Style::default()
            .size(Length::Px(W as f32),Length::Px(H as f32)).background(Color::rgb(18,22,30)))
        .child(Node::text(TEXT,text).with_style(style)))
}

fn scene(text:&str,width:f32)->Scene {
    scene_with(text,Style::default().width(Length::Px(width)).text(16.0,Color::WHITE))
}

fn engine(source:&Scene)->Engine {
    let (l,c)=(latin(),cjk());
    Engine::with_fonts(OWNER,source,&[&l[..],&c[..]],W,H).expect("native import")
}

fn boundaries(text:&str)->Vec<usize> {
    (0..=text.len()).filter(|&i|text.is_char_boundary(i)).collect()
}

fn centre(r:Rect)->(f32,f32) {(r.x()+r.size.width/2.0,r.y()+r.size.height/2.0)}

fn inside(inner:Rect,outer:Rect)->bool {
    const E:f32=0.01;
    inner.x()>=outer.x()-E && inner.y()>=outer.y()-E
        && inner.x()+inner.size.width<=outer.x()+outer.size.width+E
        && inner.y()+inner.size.height<=outer.y()+outer.size.height+E
}

#[test]
fn caret_and_offset_round_trip_on_one_line() {
    let text="Hello 你好 world";
    let source=scene(text,300.0);let v=source.version();
    let engine=engine(&source);
    let bounds=engine.node_bounds(OWNER,v,TEXT).unwrap();
    let mut previous:Option<Rect>=None;
    for offset in boundaries(text) {
        let caret=engine.text_caret(OWNER,v,TEXT,offset,1.0).unwrap_or_else(||panic!("caret at {offset}"));
        assert!(inside(caret,bounds),"caret {caret:?} at {offset} outside {bounds:?}");
        assert!(caret.size.height>10.0 && (caret.size.width-1.0).abs()<0.01);
        if let Some(p)=previous {
            // One LTR line: same row, carets strictly advance.
            assert_eq!(caret.y(),p.y());
            assert!(caret.x()>p.x(),"caret did not advance at {offset}: {caret:?} after {p:?}");
        }
        previous=Some(caret);
        let (x,y)=centre(caret);
        assert_eq!(engine.text_offset_at(OWNER,v,TEXT,x,y),Some(offset),"round trip at {offset}");
    }
    // Before the line start and past its end, the projection clamps to the ends.
    let first=engine.text_caret(OWNER,v,TEXT,0,1.0).unwrap();
    let (_,y)=centre(first);
    assert_eq!(engine.text_offset_at(OWNER,v,TEXT,bounds.x()-50.0,y),Some(0));
    assert_eq!(engine.text_offset_at(OWNER,v,TEXT,bounds.x()+bounds.size.width+50.0,y),Some(text.len()));
    // Non-boundary (inside 你, bytes 6..9) and out-of-range offsets are rejected.
    assert_eq!(engine.text_caret(OWNER,v,TEXT,7,1.0),None);
    assert_eq!(engine.text_caret(OWNER,v,TEXT,text.len()+1,1.0),None);
    assert_eq!(engine.text_caret(OWNER,v,TEXT,usize::MAX,1.0),None);
}

#[test]
fn caret_columns_enclose_painted_ink() {
    let text="WWW";
    let source=scene(text,300.0);let v=source.version();
    let mut engine=engine(&source);
    let mut bytes=vec![0;(W*H*4) as usize];
    assert!(engine.render_to_surface(OWNER,v,&mut bytes,(W*4) as usize,BUDGET));
    let carets:Vec<Rect>=(0..=3).map(|i|engine.text_caret(OWNER,v,TEXT,i,1.0).unwrap()).collect();
    let (top,bottom)=(carets[0].y() as u32,(carets[0].y()+carets[0].size.height).ceil() as u32);
    let inked=|x:u32|(0..H).any(|y|{let i=((y*W+x)*4) as usize;bytes[i..i+4]!=ROOT_FILL});
    let columns:Vec<u32>=(0..W).filter(|&x|inked(x)).collect();
    assert!(!columns.is_empty());
    // All ink lies between the first and last caret (one pixel of antialiasing slack) and
    // every glyph cell between consecutive carets carries ink.
    let (left,right)=(carets[0].x()-1.0,carets[3].x()+1.0);
    assert!(columns.iter().all(|&x|x as f32>=left && (x as f32)<=right),"ink {columns:?} outside [{left},{right}]");
    for pair in carets.windows(2) {
        assert!(columns.iter().any(|&x|x as f32>=pair[0].x() && (x as f32)<pair[1].x()),"empty cell {pair:?}");
    }
    // Ink rows stay inside the caret's line box.
    let rows:Vec<u32>=(0..H).filter(|&y|(0..W).any(|x|{let i=((y*W+x)*4) as usize;bytes[i..i+4]!=ROOT_FILL})).collect();
    assert!(rows.iter().all(|&y|y>=top && y<bottom),"ink rows {rows:?} outside line [{top},{bottom})");
}

#[test]
fn wrapped_carets_follow_lines_and_padding() {
    let text="The quick brown fox jumps over the lazy dog";
    let source=scene_with(text,Style::default().width(Length::Px(140.0)).padding(Edges::all(7.0)).text(16.0,Color::WHITE));
    let v=source.version();
    let engine=engine(&source);
    let bounds=engine.node_bounds(OWNER,v,TEXT).unwrap();
    let first=engine.text_caret(OWNER,v,TEXT,0,1.0).unwrap();
    // The text starts at the padding edge of the border box.
    assert!((first.x()-(bounds.x()+7.0)).abs()<0.01 && (first.y()-(bounds.y()+7.0)).abs()<0.01,"{first:?} vs {bounds:?}");
    let mut rows:Vec<f32>=Vec::new();
    for offset in boundaries(text) {
        let caret=engine.text_caret(OWNER,v,TEXT,offset,1.0).unwrap();
        assert!(inside(caret,bounds));
        if rows.last()!=Some(&caret.y()) {
            assert!(rows.last().map_or(true,|&last|caret.y()>last),"lines must go down at {offset}");
            rows.push(caret.y());
        }
        let (x,y)=centre(caret);
        let back=engine.text_offset_at(OWNER,v,TEXT,x,y).unwrap();
        // A soft break offset maps to the following line, so the round trip is exact.
        assert_eq!(back,offset,"round trip at {offset}");
    }
    assert!(rows.len()>=3,"expected wrapping, got rows {rows:?}");
    let line=rows[1]-rows[0];
    assert!(rows.windows(2).all(|r|(r[1]-r[0]-line).abs()<0.01),"uneven lines {rows:?}");
    assert!((bounds.size.height-14.0-line*rows.len() as f32).abs()<0.5,"box {bounds:?} vs {} lines of {line}",rows.len());
}

#[test]
fn update_moves_geometry_and_rejections_are_explicit() {
    let source=scene("abc",300.0);let v=source.version();
    let mut engine=engine(&source);
    let end=engine.text_caret(OWNER,v,TEXT,3,1.0).unwrap();
    assert!(engine.apply(OWNER,v,v+1,&[ScenePatch::UpdateText {id:TEXT,text:"abcdef".into()}]));
    // Old version is stale; the new text extends the line.
    assert_eq!(engine.text_caret(OWNER,v,TEXT,3,1.0),None);
    assert_eq!(engine.text_offset_at(OWNER,v,TEXT,end.x(),end.y()),None);
    let moved=engine.text_caret(OWNER,v+1,TEXT,6,1.0).unwrap();
    assert!(moved.x()>end.x());
    assert_eq!(engine.text_caret(OWNER,v+1,TEXT,3,1.0).unwrap(),end);
    let v=v+1;
    let (x,y)=centre(end);
    // Foreign owner, container node, unknown node, non-finite inputs, negative width.
    assert_eq!(engine.text_offset_at(OWNER+1,v,TEXT,x,y),None);
    assert_eq!(engine.text_caret(OWNER+1,v,TEXT,0,1.0),None);
    assert_eq!(engine.text_offset_at(OWNER,v,NodeId(1),x,y),None);
    assert_eq!(engine.text_caret(OWNER,v,NodeId(1),0,1.0),None);
    assert_eq!(engine.text_caret(OWNER,v,NodeId(99),0,1.0),None);
    assert_eq!(engine.text_offset_at(OWNER,v,TEXT,f32::NAN,y),None);
    assert_eq!(engine.text_offset_at(OWNER,v,TEXT,x,f32::INFINITY),None);
    assert_eq!(engine.text_caret(OWNER,v,TEXT,0,-1.0),None);
    assert_eq!(engine.text_caret(OWNER,v,TEXT,0,f32::NAN),None);
    // Zero width is a valid (degenerate) caret.
    assert_eq!(engine.text_caret(OWNER,v,TEXT,0,0.0).unwrap().size.width,0.0);
    // Hidden text has no geometry.
    let hidden=Semantics {hidden:true,..Default::default()};
    assert!(engine.apply(OWNER,v,v+1,&[ScenePatch::UpdateSemantics {id:TEXT,semantics:hidden}]));
    assert_eq!(engine.text_caret(OWNER,v+1,TEXT,0,1.0),None);
    assert!(engine.close(OWNER));
    assert_eq!(engine.text_caret(OWNER,v+1,TEXT,0,1.0),None);
}

#[test]
fn collapsed_whitespace_has_no_source_mapping() {
    let source=scene("a   b",300.0);let v=source.version();
    let engine=engine(&source);
    // The layout collapses the run of spaces, so layout offsets are not source offsets.
    assert_eq!(engine.text_caret(OWNER,v,TEXT,0,1.0),None);
    assert_eq!(engine.text_offset_at(OWNER,v,TEXT,10.0,10.0),None);
    // A single space survives layout unchanged, so the same shape maps.
    let single=scene("a b",300.0);
    assert!(self::engine(&single).text_caret(OWNER,single.version(),TEXT,2,1.0).is_some());
}

#[test]
fn hidpi_scale_keeps_logical_geometry() {
    let text="Hi 你好";
    let source=scene(text,300.0);let v=source.version();
    let mut engine=engine(&source);
    let before:Vec<Rect>=boundaries(text).into_iter().map(|i|engine.text_caret(OWNER,v,TEXT,i,1.0).unwrap()).collect();
    let line=engine.node_bounds(OWNER,v,TEXT).unwrap().size.height;
    assert!(before.iter().all(|c|c.size.height==line));
    assert!(engine.set_viewport(OWNER,W*2,H*2,2.0));
    // Upstream lays text out in device pixels, so the line height may round differently at 2x;
    // the caret must still match that scale's own single-line box and keep its x positions.
    let scaled_line=engine.node_bounds(OWNER,v,TEXT).unwrap().size.height;
    for (offset,expected) in boundaries(text).into_iter().zip(&before) {
        let caret=engine.text_caret(OWNER,v,TEXT,offset,1.0).unwrap();
        assert!((caret.x()-expected.x()).abs()<0.6 && (caret.size.height-scaled_line).abs()<0.01
            && (caret.size.width-1.0).abs()<0.01,
            "scaled caret {caret:?} vs {expected:?} (line {scaled_line}) at {offset}");
        let (x,y)=centre(caret);
        assert_eq!(engine.text_offset_at(OWNER,v,TEXT,x,y),Some(offset));
    }
}
