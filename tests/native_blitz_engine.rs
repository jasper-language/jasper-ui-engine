//! Exercise the root crate engine entry, not the separate probe crate.
//! 验证根 crate 的真实引擎入口，不经独立实验 crate。
#![cfg(feature="native-blitz-pending")]
use jasper_ui_engine::{engine::Engine,scene::{Scene,Node,NodeId,ScenePatch},style::{Style,Length,Color}};
use jasper_ui_engine::native::{SceneUpdate,Rect};
#[test]
fn root_engine_consumes_typed_frame_and_host_surface() {
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
    let mut source=Scene::new(Node::container(NodeId(1)).child(Node::rect(NodeId(2))
        .with_style(Style::default().size(Length::Px(80.0),Length::Px(40.0)).background(Color::RED))));
    let mut engine=Engine::new(10,&source,&font,160,100).unwrap();let old=source.version();
    assert_eq!(engine.node_bounds(10,old,NodeId(2)),Some(Rect::new(0.0,0.0,80.0,40.0)));
    let patches=[ScenePatch::UpdateStyle {id:NodeId(2),style:Style::default().size(Length::Px(100.0),Length::Px(50.0)).background(Color::BLUE)}];
    source.apply_batch_at(old,&patches).unwrap();
    let update=SceneUpdate {expected_version:old,next_version:source.version(),patches:&patches};
    assert!(!engine.apply_update(11,update));assert!(engine.apply_update(10,update));assert!(!engine.apply_update(10,update));
    assert_eq!(engine.node_bounds(10,source.version(),NodeId(2)),Some(Rect::new(0.0,0.0,100.0,50.0)));
    #[cfg(feature="cpu-pending")]
    {
        let mut bytes=vec![0;160*100*4];
        assert!(engine.render_to_surface(10,source.version(),&mut bytes,160*4,1024*1024));
        assert!(bytes.chunks_exact(4).any(|pixel|pixel==[Color::BLUE.r,Color::BLUE.g,Color::BLUE.b,Color::BLUE.a]));
    }
    assert!(engine.close(10));assert!(engine.snapshot_identity(10).is_none());
}

#[cfg(feature="cpu-pending")]
#[test]
fn native_frame_reaches_existing_backend_without_legacy_paint_replay() {
    use jasper_ui_engine::{backend::{RenderBackend,BackendError,HeadlessBackend},batch::UiBatch,geometry::Size};
    struct LegacyOnly {presented:usize}
    impl RenderBackend for LegacyOnly {
        fn resize(&mut self,_:Size) {panic!("native submit must not mutate legacy extent first")}
        fn submit(&mut self,_:&UiBatch)->Result<(),BackendError> {panic!("legacy paint replay")}
        fn present(&mut self)->Result<(),BackendError> {self.presented+=1;Ok(())}
    }
    let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
    let source=Scene::new(Node::container(NodeId(1)).child(Node::rect(NodeId(2))
        .with_style(Style::default().size(Length::Px(80.0),Length::Px(40.0)).background(Color::BLUE)))
        .child(Node::text(NodeId(3),"Native text 中文")));
    let mut engine=Engine::new(10,&source,&font,160,100).unwrap();let version=source.version();
    let mut legacy=LegacyOnly {presented:0};
    assert_eq!(engine.render_to_backend(10,version,&mut legacy,1024*1024),
        Err(BackendError::Unsupported("native RGBA8 frame submission")));
    assert_eq!(legacy.presented,0);
    let mut backend=HeadlessBackend::new(2,2);
    assert!(engine.render_to_backend(11,version,&mut backend,1024*1024).is_err());
    assert_eq!(backend.canvas().width(),2);
    assert!(engine.render_to_backend(10,version+1,&mut backend,1024*1024).is_err());
    assert!(engine.render_to_backend(10,version,&mut backend,1).is_err());
    assert_eq!(backend.canvas().width(),2);
    engine.render_to_backend(10,version,&mut backend,1024*1024).unwrap();
    assert_eq!((backend.canvas().width(),backend.canvas().height()),(160,100));
    assert_eq!(backend.canvas().pixel(10,10),Some(Color::BLUE));
    assert!(backend.submit_rgba8(1,1,vec![0;3]).is_err());
    assert_eq!(backend.canvas().width(),160);assert_eq!(backend.canvas().pixel(10,10),Some(Color::BLUE));
    assert!(engine.close(10));assert!(engine.render_to_backend(10,version,&mut backend,1024*1024).is_err());
    assert_eq!(backend.canvas().pixel(10,10),Some(Color::BLUE));
}
