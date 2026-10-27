//! Unaccepted U1 text integration experiment: mature Paragraph, no Canvas feature work.
//! U1文字接入实验，未验收；复用成熟Paragraph，不扩展Canvas功能。
use jasper_ui_engine::resource::UiBytesSlot;
use skia_safe::{surfaces, AlphaType, Color, ColorType, Data, Font, FontMgr, ImageInfo};
use skia_safe::textlayout::{FontCollection, ParagraphBuilder, ParagraphStyle, TextStyle, TypefaceFontProvider, RectHeightStyle, RectWidthStyle};
use std::{fs, path::PathBuf};
fn main() {
    let destination=PathBuf::from(std::env::args_os().nth(1).expect("output directory"));
    let font_path=PathBuf::from(std::env::args_os().nth(2).expect("explicit font bytes"));
    let cjk_path=std::env::args_os().nth(3).map(PathBuf::from);
    fs::create_dir_all(&destination).unwrap();
    let mut slot=UiBytesSlot::new(7,99);
    let key=slot.install(7,fs::read(font_path).unwrap()).unwrap();
    assert!(slot.bytes(8,key).is_none());
    let manager=FontMgr::custom_empty().expect("memory font manager");
    let face=manager.new_from_data(Data::new_copy(slot.bytes(7,key).unwrap()),None).expect("font parses");
    let font=Font::from_typeface(face.clone(),22.0);
    assert_ne!(font.unichar_to_glyph('J' as i32),0);
    assert_ne!(font.unichar_to_glyph('م' as i32),0);
    let mut provider=TypefaceFontProvider::new();
    provider.register_typeface(face,Some("InjectedUI"));
    let mut cjk_resource=None;
    if let Some(path)=cjk_path {
        let mut injected=UiBytesSlot::new(7,100);let cjk_key=injected.install(7,fs::read(path).unwrap()).unwrap();
        assert!(injected.bytes(8,cjk_key).is_none());
        let cjk_face=manager.new_from_data(Data::new_copy(injected.bytes(7,cjk_key).unwrap()),Some(2)).expect("explicit SC TTC face");
        let family=cjk_face.family_name();assert!(family.contains("SC"));
        let glyph_check=Font::from_typeface(cjk_face.clone(),22.0);assert_ne!(glyph_check.unichar_to_glyph('你' as i32),0);drop(glyph_check);
        provider.register_typeface(cjk_face,Some("InjectedCJK"));
        fs::write(destination.join("cjk-face.txt"),format!("TTC index=2, family={family}\n")).unwrap();
        cjk_resource=Some((injected,cjk_key));
    }
    let mut collection=FontCollection::new();
    let registered:FontMgr=provider.into();
    collection.set_asset_font_manager(registered);
    collection.disable_font_fallback();
    let mut text_style=TextStyle::new();
    if cjk_resource.is_some() {text_style.set_font_families(&["InjectedUI","InjectedCJK"]);} else {text_style.set_font_families(&["InjectedUI"]);}
    text_style.set_font_size(22.0).set_color(Color::BLACK);
    let mut style=ParagraphStyle::new();style.set_text_style(&text_style);
    let mut builder=ParagraphBuilder::new(&style,collection.clone());
    let text=if cjk_resource.is_some() {"Jasper café e\u{301} 中文\nمرحبا بالعالم"} else {"Jasper café e\u{301}\nمرحبا بالعالم"};
    builder.add_text(text);
    let mut paragraph=builder.build();
    paragraph.layout(480.0);
    assert_eq!(paragraph.unresolved_glyphs(),Some(0));
    let height=paragraph.height();let width=paragraph.longest_line();
    assert!(height.is_finite() && height>0.0 && height<140.0);
    assert!(width.is_finite() && width>0.0 && width<=480.0);
    let boxes=paragraph.get_rects_for_range(0..6,RectHeightStyle::Tight,RectWidthStyle::Tight);
    assert!(!boxes.is_empty());
    let mut surface=surfaces::raster_n32_premul((520,160)).unwrap();
    surface.canvas().clear(Color::WHITE);
    paragraph.paint(surface.canvas(),(16.0,12.0));
    let info=ImageInfo::new((520,160),ColorType::RGBA8888,AlphaType::Unpremul,None);
    let mut pixels=vec![0;520*160*4];assert!(surface.read_pixels(&info,&mut pixels,520*4,(0,0)));
    let changed=pixels.chunks_exact(4).filter(|p|p[..3]!=[255,255,255]).count();assert!(changed>100);
    surface.canvas().clear(Color::WHITE);paragraph.paint(surface.canvas(),(16.0,12.0));
    let mut repeated=vec![0;pixels.len()];assert!(surface.read_pixels(&info,&mut repeated,520*4,(0,0)));assert_eq!(pixels,repeated);
    let mut ppm=b"P6\n520 160\n255\n".to_vec();for p in pixels.chunks_exact(4){ppm.extend_from_slice(&p[..3]);}
    fs::write(destination.join("paragraph.ppm"),ppm).unwrap();
    fs::write(destination.join("paragraph.rgba"),pixels).unwrap();
    paragraph.layout(80.0);let wrapped=paragraph.height();assert!(wrapped>height);
    // Missing CJK is explicit rather than an implicit system-font fallback.
    // CJK缺字显式记录，不偷偷调用系统字体回退，不将本夹具冒充中文支持。
    let mut missing_builder=ParagraphBuilder::new(&style,collection.clone());missing_builder.add_text("你好");
    let mut missing=missing_builder.build();missing.layout(480.0);let unresolved=missing.unresolved_glyphs().unwrap();if cjk_resource.is_some() {assert_eq!(unresolved,0);} else {assert!(unresolved>0);}
    let metrics=format!("{{\"width\":{width},\"height\":{height},\"wrapped_height\":{wrapped},\"changed_pixels\":{changed},\"missing_cjk_glyphs\":{unresolved},\"range_boxes\":{},\"production_accepted\":false,\"formal_proof_complete\":false}}\n",boxes.len());
    fs::write(destination.join("metrics.json"),metrics).unwrap();
    // Fixed Unicode coordinate probe only; this is not a document index converter.
    // 固定Unicode坐标实验，不实现公共文档索引转换，也不宣称正式验收。
    let mut index_builder=ParagraphBuilder::new(&style,collection.clone());
    index_builder.add_text("A😀e\u{301}中B");
    let mut indexed=index_builder.build();indexed.layout(480.0);
    let mut observations=String::from("fixture=A😀e\u{301}中B; utf8_bytes=12; utf16_units=7; production_accepted=false\n");
    for (label,range) in [("ascii",0..1),("non_bmp",1..3),("trailing_surrogate",2..3),("combining_cluster",3..5),("combining_tail",4..5),("cjk",5..6),("final_ascii",6..7)] {
        let rects=indexed.get_rects_for_range(range,RectHeightStyle::Tight,RectWidthStyle::Tight);
        observations.push_str(&format!("{label}: boxes={} {:?}\n",rects.len(),rects));
        if matches!(label,"ascii"|"non_bmp"|"combining_cluster"|"cjk"|"final_ascii") {assert!(!rects.is_empty());}
        else {assert!(rects.is_empty());}
    }
    fs::write(destination.join("unicode-range-observations.txt"),observations).unwrap();
    drop(indexed);drop(index_builder);
    drop(missing);drop(missing_builder);drop(paragraph);drop(builder);drop(collection);drop(font);drop(manager);
    if let Some((mut injected,cjk_key))=cjk_resource.take(){assert!(injected.release(7,cjk_key).is_some());assert!(injected.bytes(7,cjk_key).is_none());}
    assert!(slot.release(7,key).is_some());assert!(slot.bytes(7,key).is_none());
    println!("Experimental shared Paragraph layout+paint passed fixed injected-font fixture; CJK unresolved={unresolved}; not accepted production Rust.");
}
