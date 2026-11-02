//! Whole-frame transaction acceptance through the root native Engine.
//! 根原生 Engine 的整帧事务验收：候选构建失败不改变活动视图。
#![cfg(feature="native-blitz-cpu-pending")]
use jasper_ui_engine::{engine::Engine,geometry::Edges,scene::{Node,NodeId,Scene,ScenePatch},
    semantics::{Role,Semantics},style::{Color,Length,Style}};
use jasper_ui_engine::native::SceneUpdate;

const OWNER:u64=17;
const W:u32=200;
const H:u32=120;
const BUDGET:usize=(W*H*4) as usize;

fn font()->Vec<u8> {
    std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap()
}

fn scene()->Scene {
    let action=Node::text(NodeId(2),"Action")
        .with_semantics(Semantics::default().role(Role::Button).label("Action").focusable(true))
        .with_style(Style::default().size(Length::Px(100.0),Length::Px(40.0))
            .background(Color::GREEN));
    Scene::new(Node::container(NodeId(1)).with_style(Style::default()
        .size(Length::Px(W as f32),Length::Px(H as f32))
        .padding(Edges::all(10.0)).background(Color::rgb(18,22,30))).child(action))
}

fn pixels(engine:&mut Engine,version:u64)->Vec<u8> {
    let mut bytes=vec![0;BUDGET];
    assert!(engine.render_to_surface(OWNER,version,&mut bytes,(W*4) as usize,BUDGET));
    bytes
}

fn apply(source:&mut Scene,engine:&mut Engine,patches:&[ScenePatch])->bool {
    let before=source.clone();
    let old=source.version();
    source.apply_batch_at(old,patches).unwrap();
    let accepted=engine.apply_update(OWNER,SceneUpdate {expected_version:old,next_version:source.version(),patches});
    if !accepted {*source=before;}
    accepted
}

#[test]
fn failed_candidate_build_leaves_the_active_frame_and_source_snapshot_unchanged() {
    let f=font();
    let mut source=scene();
    let mut live=Engine::new(OWNER,&source,&f,W,H).unwrap();
    let version=source.version();
    assert!(live.focus(OWNER,version,NodeId(2),&mut |_|{}));
    let before=pixels(&mut live,version);
    let before_bounds=live.node_bounds(OWNER,version,NodeId(2)).unwrap();
    let mut invalid_style=Style::default().width(Length::Px(80.0)).background(Color::BLUE);
    invalid_style.font_size=0.0;
    let invalid=[ScenePatch::UpdateStyle {id:NodeId(2),style:invalid_style}];
    assert!(!apply(&mut source,&mut live,&invalid));
    assert_eq!(live.snapshot_identity(OWNER),Some((version,2)));
    assert_eq!(live.focused_node(OWNER,version),Some(NodeId(2)));
    assert_eq!(live.node_bounds(OWNER,version,NodeId(2)),Some(before_bounds));
    assert_eq!(pixels(&mut live,version),before);

    // A rejected candidate cannot desynchronise the engine's retained source snapshot.
    let valid=[ScenePatch::UpdateStyle {id:NodeId(2),style:Style::default()
        .size(Length::Px(120.0),Length::Px(50.0)).background(Color::BLUE)}];
    assert!(apply(&mut source,&mut live,&valid));
    assert_eq!(live.snapshot_identity(OWNER),Some((source.version(),2)));
    let mut fresh=Engine::new(OWNER,&source,&f,W,H).unwrap();
    assert_eq!(pixels(&mut live,source.version()),pixels(&mut fresh,source.version()));
    assert_eq!(live.focused_node(OWNER,source.version()),Some(NodeId(2)));
}

#[test]
fn legal_native_tag_change_rebuilds_the_candidate_instead_of_partially_mutating() {
    let f=font();
    let mut source=scene();
    let mut live=Engine::new(OWNER,&source,&f,W,H).unwrap();
    let old=source.version();
    assert!(live.focus(OWNER,old,NodeId(2),&mut |_|{}));
    let next=Semantics::default().role(Role::Container).label("Panel").focusable(true);
    let patches=[ScenePatch::UpdateSemantics {id:NodeId(2),semantics:next.clone()}];
    source.apply_batch_at(old,&patches).unwrap();
    assert!(live.apply_update(OWNER,SceneUpdate {expected_version:old,next_version:source.version(),patches:&patches}));
    let mut fresh=Engine::new(OWNER,&source,&f,W,H).unwrap();
    assert_eq!(pixels(&mut live,source.version()),pixels(&mut fresh,source.version()));
    let snapshot=live.semantics(OWNER,source.version()).unwrap();
    let node=snapshot.nodes.iter().find(|node|node.node==NodeId(2)).unwrap();
    assert_eq!(node.semantics.role,Role::Container);
    assert_eq!(node.semantics.label,"Panel");
    assert_eq!(live.focused_node(OWNER,source.version()),Some(NodeId(2)));
}
