//! Native incremental frame diagnostics against fresh import of the same source state.
//! 原生增量帧与相同声明终态的重新导入比较；这是诊断，不是实现证明。
#![cfg(feature="native-blitz-cpu-pending")]
use jasper_ui_engine::{engine::Engine,scene::{Node,NodeId,Scene,ScenePatch},style::{Style,Length,Color,Overflow,Display},geometry::Point};
use jasper_ui_engine::native::SceneUpdate;
fn pixels(view:&mut Engine,version:u64)->Vec<u8> {
    let mut bytes=vec![0;160*120*4];
    assert!(view.render_to_surface(10,version,&mut bytes,160*4,160*120*4));bytes
}
#[test]
fn ordered_scroll_and_style_frames_match_fresh_source_import() {
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
    for offset in [0.0,16.0,64.0,256.0] {for scroll_first in [false,true] {for display in [Display::Block,Display::None] {
        let mut style=Style::default().size(Length::Px(80.0),Length::Px(40.0));style.overflow=Overflow::Scroll;style.display=Display::Block;
        let red=Node::rect(NodeId(3)).with_style(Style::default().size(Length::Px(80.0),Length::Px(64.0)).background(Color::RED));
        let blue=Node::rect(NodeId(4)).with_style(Style::default().size(Length::Px(80.0),Length::Px(64.0)).background(Color::BLUE));
        let scroll_node=Node::container(NodeId(2)).with_style(style.clone()).child(red).child(blue);
        let mut source=Scene::new(Node::container(NodeId(1)).child(scroll_node));
        let mut live=Engine::new(10,&source,&font,160,120).unwrap();
        style.display=display;
        let scroll=ScenePatch::SetScroll {id:NodeId(2),offset:Point::new(0.0,offset)};
        let change=ScenePatch::UpdateStyle {id:NodeId(2),style};
        let patches=if scroll_first {vec![scroll,change]} else {vec![change,scroll]};
        let old=source.version();source.apply_batch_at(old,&patches).unwrap();
        assert!(live.apply_update(10,SceneUpdate {expected_version:old,next_version:source.version(),patches:&patches}));
        let mut fresh=Engine::new(10,&source,&font,160,120).unwrap();
        assert_eq!(pixels(&mut live,source.version()),pixels(&mut fresh,source.version()),"offset={offset}, scroll_first={scroll_first}, display={display:?}");
    }}}
}
