//! Native text selection geometry through the root entry: a source UTF-8 range maps to
//! per-line selection rectangles from the same layout that paints the text and places carets.
//! 根入口原生文字选区几何：源 UTF-8 区间映射到逐行选区矩形，与绘制、光标共用同一布局。
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
const E:f32=0.01;

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

fn right(r:Rect)->f32 {r.x()+r.size.width}

#[test]
fn single_line_selection_spans_caret_to_caret() {
    let text="Hello 你好 world";
    let source=scene(text,300.0);let v=source.version();
    let engine=engine(&source);
    let bounds=engine.node_bounds(OWNER,v,TEXT).unwrap();
    let cuts=boundaries(text);
    let carets:Vec<Rect>=cuts.iter().map(|&i|engine.text_caret(OWNER,v,TEXT,i,1.0).unwrap()).collect();
    let mut checked=0;
    for (a,&start) in cuts.iter().enumerate() {
        for (b,&end) in cuts.iter().enumerate().skip(a) {
            let rects=engine.text_selection(OWNER,v,TEXT,start,end).unwrap_or_else(||panic!("range {start}..{end}"));
            if start==end {assert!(rects.is_empty(),"empty range {start} has {rects:?}");continue;}
            // One LTR line: exactly one rectangle from the start caret to the end caret, as tall
            // as the caret's line box.
            assert_eq!(rects.len(),1,"range {start}..{end}: {rects:?}");
            let r=rects[0];
            assert!((r.x()-carets[a].x()).abs()<E && (right(r)-carets[b].x()).abs()<E,
                "range {start}..{end}: {r:?} vs carets {:?} {:?}",carets[a],carets[b]);
            assert!((r.y()-carets[a].y()).abs()<E && (r.size.height-carets[a].size.height).abs()<E,"{r:?} vs {:?}",carets[a]);
            assert!(r.y()>=bounds.y()-E && r.y()+r.size.height<=bounds.y()+bounds.size.height+E);
            checked+=1;
        }
    }
    assert_eq!(checked,cuts.len()*(cuts.len()-1)/2);
}

#[test]
fn wrapped_selection_has_one_rect_per_line() {
    let text="The quick brown fox jumps over the lazy dog";
    let source=scene_with(text,Style::default().width(Length::Px(140.0)).padding(Edges::all(7.0)).text(16.0,Color::WHITE));
    let v=source.version();
    let engine=engine(&source);
    let bounds=engine.node_bounds(OWNER,v,TEXT).unwrap();
    // Line starts and rows, from the carets of the same layout.
    let mut lines:Vec<(usize,Rect)>=Vec::new();
    for offset in boundaries(text) {
        let caret=engine.text_caret(OWNER,v,TEXT,offset,1.0).unwrap();
        if lines.last().map_or(true,|(_,c)|c.y()!=caret.y()) {lines.push((offset,caret));}
    }
    assert!(lines.len()>=3,"expected wrapping, got {lines:?}");
    let all=engine.text_selection(OWNER,v,TEXT,0,text.len()).unwrap();
    assert_eq!(all.len(),lines.len(),"rects {all:?} vs lines {lines:?}");
    for (r,(_,caret)) in all.iter().zip(&lines) {
        assert!((r.y()-caret.y()).abs()<E && (r.size.height-caret.size.height).abs()<E,"{r:?} vs line caret {caret:?}");
        // Every line starts at the padding edge.
        assert!((r.x()-(bounds.x()+7.0)).abs()<E,"{r:?} vs {bounds:?}");
        assert!(r.size.width>0.0);
    }
    // A range from inside line 0 to inside line 1 gives the tail of line 0 and the head of line 1.
    let (second,_)=lines[1];
    let (start,end)=(2,second+3);
    let rects=engine.text_selection(OWNER,v,TEXT,start,end).unwrap();
    assert_eq!(rects.len(),2,"{rects:?}");
    let (cs,ce)=(engine.text_caret(OWNER,v,TEXT,start,1.0).unwrap(),engine.text_caret(OWNER,v,TEXT,end,1.0).unwrap());
    assert!((rects[0].x()-cs.x()).abs()<E && (rects[0].y()-cs.y()).abs()<E,"{:?} vs {cs:?}",rects[0]);
    assert!((right(rects[0])-right(all[0])).abs()<E,"line 0 tail {:?} vs full {:?}",rects[0],all[0]);
    assert!((rects[1].x()-(bounds.x()+7.0)).abs()<E && (right(rects[1])-ce.x()).abs()<E && (rects[1].y()-ce.y()).abs()<E,
        "{:?} vs {ce:?}",rects[1]);
    // A range ending exactly at a soft break stays on its own line.
    let rects=engine.text_selection(OWNER,v,TEXT,0,second).unwrap();
    assert_eq!(rects.len(),1,"{rects:?}");
    assert!((rects[0].y()-lines[0].1.y()).abs()<E);
}

#[test]
fn selection_encloses_painted_ink() {
    let text="WWW";
    let source=scene(text,300.0);let v=source.version();
    let mut engine=engine(&source);
    let mut bytes=vec![0;(W*H*4) as usize];
    assert!(engine.render_to_surface(OWNER,v,&mut bytes,(W*4) as usize,BUDGET));
    let rects=engine.text_selection(OWNER,v,TEXT,0,text.len()).unwrap();
    assert_eq!(rects.len(),1);
    let r=rects[0];
    let inked:Vec<(u32,u32)>=(0..H).flat_map(|y|(0..W).map(move|x|(x,y)))
        .filter(|&(x,y)|{let i=((y*W+x)*4) as usize;bytes[i..i+4]!=ROOT_FILL}).collect();
    assert!(!inked.is_empty());
    // One pixel of antialiasing slack on each side.
    assert!(inked.iter().all(|&(x,y)|x as f32>=r.x()-1.0 && (x as f32)<=right(r)+1.0
        && y as f32>=r.y()-1.0 && (y as f32)<=r.y()+r.size.height+1.0),"ink outside selection {r:?}");
    // The middle glyph's selection holds ink and lies strictly inside the full selection.
    let mid=engine.text_selection(OWNER,v,TEXT,1,2).unwrap()[0];
    assert!(mid.x()>r.x() && right(mid)<right(r));
    assert!(inked.iter().any(|&(x,_)|x as f32>=mid.x() && (x as f32)<right(mid)));
}

#[test]
fn empty_ranges_and_rejections_are_explicit() {
    let source=scene("a你b",300.0);let v=source.version();
    let mut engine=engine(&source);
    assert_eq!(engine.text_selection(OWNER,v,TEXT,0,0),Some(vec![]));
    assert_eq!(engine.text_selection(OWNER,v,TEXT,5,5),Some(vec![]));
    assert_eq!(engine.text_selection(OWNER,v,TEXT,0,5).unwrap().len(),1);
    // Reversed, non-boundary (你 is bytes 1..4) and out-of-range ranges.
    assert_eq!(engine.text_selection(OWNER,v,TEXT,4,1),None);
    assert_eq!(engine.text_selection(OWNER,v,TEXT,0,2),None);
    assert_eq!(engine.text_selection(OWNER,v,TEXT,2,4),None);
    assert_eq!(engine.text_selection(OWNER,v,TEXT,0,6),None);
    assert_eq!(engine.text_selection(OWNER,v,TEXT,0,usize::MAX),None);
    // Foreign owner, container, unknown node.
    assert_eq!(engine.text_selection(OWNER+1,v,TEXT,0,1),None);
    assert_eq!(engine.text_selection(OWNER,v,NodeId(1),0,1),None);
    assert_eq!(engine.text_selection(OWNER,v,NodeId(99),0,1),None);
    // An update makes the old version stale and lengthens the selectable text.
    let before=engine.text_selection(OWNER,v,TEXT,0,5).unwrap()[0];
    assert!(engine.apply(OWNER,v,v+1,&[ScenePatch::UpdateText {id:TEXT,text:"a你bcd".into()}]));
    assert_eq!(engine.text_selection(OWNER,v,TEXT,0,5),None);
    let v=v+1;
    assert_eq!(engine.text_selection(OWNER,v,TEXT,0,5).unwrap()[0],before);
    assert!(right(engine.text_selection(OWNER,v,TEXT,0,7).unwrap()[0])>right(before));
    // Hidden text and a closed engine have no geometry.
    let hidden=Semantics {hidden:true,..Default::default()};
    assert!(engine.apply(OWNER,v,v+1,&[ScenePatch::UpdateSemantics {id:TEXT,semantics:hidden}]));
    assert_eq!(engine.text_selection(OWNER,v+1,TEXT,0,1),None);
    assert!(engine.close(OWNER));
    assert_eq!(engine.text_selection(OWNER,v+1,TEXT,0,1),None);
    // Collapsed whitespace has no source mapping.
    let collapsed=scene("a   b",300.0);
    assert_eq!(self::engine(&collapsed).text_selection(OWNER,collapsed.version(),TEXT,0,1),None);
}

#[test]
fn hidpi_scale_keeps_logical_selection() {
    let text="Hi 你好";
    let source=scene(text,300.0);let v=source.version();
    let mut engine=engine(&source);
    let before=engine.text_selection(OWNER,v,TEXT,0,text.len()).unwrap();
    assert!(engine.set_viewport(OWNER,W*2,H*2,2.0));
    let after=engine.text_selection(OWNER,v,TEXT,0,text.len()).unwrap();
    assert_eq!((before.len(),after.len()),(1,1));
    // Upstream lays text out in device pixels: x edges stay within rounding, and the height
    // matches that scale's own line box.
    let scaled_line=engine.node_bounds(OWNER,v,TEXT).unwrap().size.height;
    assert!((after[0].x()-before[0].x()).abs()<0.6 && (right(after[0])-right(before[0])).abs()<0.6
        && (after[0].size.height-scaled_line).abs()<E,"{after:?} vs {before:?} (line {scaled_line})");
    let first=engine.text_caret(OWNER,v,TEXT,3,1.0).unwrap();
    let tail=engine.text_selection(OWNER,v,TEXT,3,text.len()).unwrap()[0];
    assert!((tail.x()-first.x()).abs()<E);
}
