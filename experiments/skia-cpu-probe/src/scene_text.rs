//! Unaccepted U1 experiment: actual Taffy scene measurement and final Paragraph paint.
//! 未验收U1实验：真实Taffy场景测量及最终Paragraph绘制，不修改生产协议。
use jasper_ui_engine::{LayoutEngine,TextEngine,TextMetrics,TaffyLayoutEngine,Node,NodeId,Scene,Style,Length,Size,Color,FlexDirection};
use skia_safe::{Data,FontMgr,ImageInfo,ColorType,AlphaType,surfaces};
use skia_safe::textlayout::{FontCollection,TypefaceFontProvider,TextStyle,ParagraphStyle,ParagraphBuilder,Paragraph};
use std::{fs,path::PathBuf};
struct InjectedText { fonts:FontCollection }
impl InjectedText {
    fn paragraph(&self,text:&str,size:f32,width:f32)->Paragraph {
        let mut ts=TextStyle::new();ts.set_font_families(&["InjectedUI","InjectedCJK"]);
        ts.set_font_size(size).set_color(skia_safe::Color::BLACK);
        let mut ps=ParagraphStyle::new();ps.set_text_style(&ts);
        let mut builder=ParagraphBuilder::new(&ps,self.fonts.clone());builder.add_text(text);
        let mut p=builder.build();p.layout(width);assert_eq!(p.unresolved_glyphs(),Some(0));p
    }
}
impl TextEngine for InjectedText {
    fn measure(&self,text:&str,size:f32,width:Option<f32>)->TextMetrics {
        let p=self.paragraph(text,size,width.unwrap_or(100000.0));
        // This fixed fixture does not consume ascent/line count; no general metric claim.
        // 本夹具布局不消费ascent/lines，不宣称这两个占位字段为完整排版指标。
        TextMetrics{width:p.longest_line(),height:p.height(),ascent:0.0,lines:0}
    }
}
fn main(){
    let args:Vec<_>=std::env::args_os().collect();let out=PathBuf::from(&args[1]);fs::create_dir_all(&out).unwrap();
    let manager=FontMgr::custom_empty().unwrap();let mut provider=TypefaceFontProvider::new();
    for (path,index,alias) in [(&args[2],None,"InjectedUI"),(&args[3],Some(2),"InjectedCJK")] {
        let bytes=fs::read(path).unwrap();let face=manager.new_from_data(Data::new_copy(&bytes),index).unwrap();provider.register_typeface(face,Some(alias));
    }
    let mut fonts=FontCollection::new();let fm:FontMgr=provider.into();fonts.set_asset_font_manager(fm);fonts.disable_font_fallback();
    let engine=InjectedText{fonts};let content="Jasper café 中文 مرحبا بالعالم Jasper café 中文 مرحبا بالعالم";
    let scene=Scene::try_new(Node::container(NodeId(1)).with_style(Style::default().size(Length::Px(520.0),Length::Px(420.0)).direction(FlexDirection::Column))
        .child(Node::text(NodeId(2),content).with_style(Style::default().size(Length::Px(120.0),Length::Auto).text(22.0,Color::BLACK)))
        .child(Node::text(NodeId(3),content).with_style(Style::default().size(Length::Px(280.0),Length::Auto).text(22.0,Color::BLACK)))).unwrap();
    let layout=TaffyLayoutEngine.layout(&scene,Size::new(520.0,420.0),&engine).unwrap();assert_eq!(layout.scene_version,scene.version());
    let mut surface=surfaces::raster_n32_premul((520,420)).unwrap();surface.canvas().clear(skia_safe::Color::WHITE);
    let mut heights=Vec::new();let mut log=String::from("production_accepted=false; final-layout direct consumer, no UiBatch integration\n");
    for id in [NodeId(2),NodeId(3)] {
        let node=layout.find(id).unwrap();let p=engine.paragraph(node.text.as_ref().unwrap(),node.style.font_size,node.content.width());
        assert!((p.height()-node.content.height()).abs()<0.1);
        log.push_str(&format!("node={id:?} width={} content_height={} paragraph_height={}\n",node.content.width(),node.content.height(),p.height()));
        heights.push(p.height());p.paint(surface.canvas(),(node.content.x(),node.content.y()));
    }
    assert!(heights[0]>heights[1]);
    let info=ImageInfo::new((520,420),ColorType::RGBA8888,AlphaType::Unpremul,None);let mut rgba=vec![0;520*420*4];assert!(surface.read_pixels(&info,&mut rgba,520*4,(0,0)));
    assert!(rgba.chunks_exact(4).filter(|p|p[..3]!=[255,255,255]).count()>100);
    let mut ppm=b"P6\n520 420\n255\n".to_vec();for p in rgba.chunks_exact(4){ppm.extend_from_slice(&p[..3]);}
    fs::write(out.join("scene.ppm"),ppm).unwrap();fs::write(out.join("layout.txt"),log).unwrap();
}
