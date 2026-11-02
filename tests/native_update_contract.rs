//! Public structured scene-update rejection contract.
//! 公共结构化场景更新拒绝契约。
#![cfg(feature="native-blitz-cpu-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::native::{SceneUpdate,SceneUpdateError};
use jasper_ui_engine::scene::{Node,NodeId,Scene,ScenePatch};
use jasper_ui_engine::style::{Color,Length,Style};

const OWNER:u64=27;
fn font()->Vec<u8> {
    let path=std::env::var("JASPER_UI_TEST_FONT").expect("injected Latin font");
    std::fs::read(path).expect("font readable")
}
fn scene()->Scene {
    Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"update")
        .with_style(Style::default().width(Length::Px(120.0)).text(14.0,Color::WHITE))))
}
fn patch()->ScenePatch {
    ScenePatch::UpdateStyle {id:NodeId(2),style:Style::default()
        .width(Length::Px(140.0)).text(14.0,Color::BLUE)}
}
fn update<'a>(old:u64,next:u64,patches:&'a [ScenePatch])->SceneUpdate<'a> {
    SceneUpdate {expected_version:old,next_version:next,patches}
}

#[test]
fn public_update_errors_distinguish_every_admission_and_construction_stage() {
    let source=scene();
    let v=source.version();
    let mut engine=Engine::new(OWNER,&source,&font(),160,80).expect("native import");
    let one=[patch()];
    let identity=engine.snapshot_identity(OWNER);

    assert_eq!(engine.try_apply_update(OWNER+1,update(v,v+1,&one)),Err(SceneUpdateError::OwnerMismatch));
    assert_eq!(engine.snapshot_identity(OWNER),identity);
    assert_eq!(engine.try_apply_update(OWNER,update(v,v+1,&[])),Err(SceneUpdateError::EmptyFrame));
    assert_eq!(engine.snapshot_identity(OWNER),identity);
    assert_eq!(engine.try_apply_update(OWNER,update(v+2,v+3,&one)),
        Err(SceneUpdateError::StaleVersion {expected:v+2,current:v}));
    assert_eq!(engine.snapshot_identity(OWNER),identity);
    assert_eq!(engine.try_apply_update(OWNER,update(v,v+7,&one)),
        Err(SceneUpdateError::InvalidSuccessor {expected:v,next:v+7}));
    assert_eq!(engine.snapshot_identity(OWNER),identity);

    let missing=[ScenePatch::UpdateStyle {id:NodeId(99),style:Style::default()}];
    assert_eq!(engine.try_apply_update(OWNER,update(v,v+1,&missing)),Err(SceneUpdateError::SourceRejected));
    assert_eq!(engine.snapshot_identity(OWNER),identity);

    let mut invalid_style=Style::default().width(Length::Px(120.0));
    invalid_style.font_size=0.0;
    let invalid=[ScenePatch::UpdateStyle {id:NodeId(2),style:invalid_style}];
    assert_eq!(engine.try_apply_update(OWNER,update(v,v+1,&invalid)),Err(SceneUpdateError::NativeRejected));
    assert_eq!(engine.snapshot_identity(OWNER),identity);

    let oversized=vec![patch();4097];
    assert_eq!(engine.try_apply_update(OWNER,update(v,v+1,&oversized)),
        Err(SceneUpdateError::CapacityExceeded));
    assert_eq!(engine.snapshot_identity(OWNER),identity);

    assert!(engine.close(OWNER));
    assert_eq!(engine.try_apply_update(OWNER,update(v,v+1,&one)),Err(SceneUpdateError::Closed));
}
