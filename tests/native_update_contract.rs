//! Public structured scene-update rejection contract.
//! 公共结构化场景更新拒绝契约。
#![cfg(feature = "native-blitz-cpu-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::native::{SceneQuota, SceneUpdate, SceneUpdateError};
use jasper_ui_engine::scene::{Node, NodeId, Scene, ScenePatch};
use jasper_ui_engine::style::{Color, Length, Style};

const OWNER: u64 = 27;
fn font() -> Vec<u8> {
    let path = std::env::var("JASPER_UI_TEST_FONT").expect("injected Latin font");
    std::fs::read(path).expect("font readable")
}
fn scene() -> Scene {
    Scene::new(
        Node::container(NodeId(1)).child(
            Node::text(NodeId(2), "update").with_style(
                Style::default()
                    .width(Length::Px(120.0))
                    .text(14.0, Color::WHITE),
            ),
        ),
    )
}
fn patch() -> ScenePatch {
    ScenePatch::UpdateStyle {
        id: NodeId(2),
        style: Style::default()
            .width(Length::Px(140.0))
            .text(14.0, Color::BLUE),
    }
}
fn update<'a>(old: u64, next: u64, patches: &'a [ScenePatch]) -> SceneUpdate<'a> {
    SceneUpdate {
        expected_version: old,
        next_version: next,
        patches,
    }
}

#[test]
fn public_update_errors_distinguish_every_admission_and_construction_stage() {
    let source = scene();
    let v = source.version();
    let mut engine = Engine::new(OWNER, &source, &font(), 160, 80).expect("native import");
    let one = [patch()];
    let identity = engine.snapshot_identity(OWNER);

    assert_eq!(
        engine.try_apply_update(OWNER + 1, update(v, v + 1, &one)),
        Err(SceneUpdateError::OwnerMismatch)
    );
    assert_eq!(engine.snapshot_identity(OWNER), identity);
    assert_eq!(
        engine.try_apply_update(OWNER, update(v, v + 1, &[])),
        Err(SceneUpdateError::EmptyFrame)
    );
    assert_eq!(engine.snapshot_identity(OWNER), identity);
    assert_eq!(
        engine.try_apply_update(OWNER, update(v + 2, v + 3, &one)),
        Err(SceneUpdateError::StaleVersion {
            expected: v + 2,
            current: v
        })
    );
    assert_eq!(engine.snapshot_identity(OWNER), identity);
    assert_eq!(
        engine.try_apply_update(OWNER, update(v, v + 7, &one)),
        Err(SceneUpdateError::InvalidSuccessor {
            expected: v,
            next: v + 7
        })
    );
    assert_eq!(engine.snapshot_identity(OWNER), identity);

    let missing = [ScenePatch::UpdateStyle {
        id: NodeId(99),
        style: Style::default(),
    }];
    assert_eq!(
        engine.try_apply_update(OWNER, update(v, v + 1, &missing)),
        Err(SceneUpdateError::SourceRejected)
    );
    assert_eq!(engine.snapshot_identity(OWNER), identity);

    let mut invalid_style = Style::default().width(Length::Px(120.0));
    invalid_style.font_size = 0.0;
    let invalid = [ScenePatch::UpdateStyle {
        id: NodeId(2),
        style: invalid_style,
    }];
    assert_eq!(
        engine.try_apply_update(OWNER, update(v, v + 1, &invalid)),
        Err(SceneUpdateError::NativeRejected)
    );
    assert_eq!(engine.snapshot_identity(OWNER), identity);

    // Host quota: the frame below would make 3 nodes / 9 text bytes.
    // 宿主额度：下面这一帧会使场景达到 3 个节点 / 9 字节文字。
    let insert = [ScenePatch::Insert {
        parent: NodeId(1),
        index: 1,
        node: Node::text(NodeId(3), "new"),
    }];
    let nodes = SceneQuota {
        max_nodes: Some(2),
        max_text_bytes: None,
    };
    let error = engine
        .try_apply_within(OWNER, v, v + 1, &insert, nodes)
        .unwrap_err();
    assert_eq!(
        error,
        SceneUpdateError::NodeQuotaExceeded {
            limit: 2,
            required: 3
        }
    );
    assert_eq!(
        error.to_string(),
        "scene update needs 3 nodes but the host node quota is 2"
    );
    assert_eq!(engine.snapshot_identity(OWNER), identity);
    let text = SceneQuota {
        max_nodes: Some(3),
        max_text_bytes: Some(8),
    };
    let error = engine
        .try_apply_within(OWNER, v, v + 1, &insert, text)
        .unwrap_err();
    assert_eq!(
        error,
        SceneUpdateError::TextQuotaExceeded {
            limit: 8,
            required: 9
        }
    );
    assert_eq!(
        error.to_string(),
        "scene update needs 9 bytes of text but the host text quota is 8 bytes"
    );
    assert_eq!(engine.snapshot_identity(OWNER), identity);

    assert!(engine.close(OWNER));
    assert_eq!(
        engine.try_apply_update(OWNER, update(v, v + 1, &one)),
        Err(SceneUpdateError::Closed)
    );
}

/// No fixed limit: frames and scenes far beyond the former 4096 cap are admitted atomically.
/// 没有固定上限：远超原 4096 上限的帧与场景被原子接受。
#[test]
fn frames_and_scenes_have_no_fixed_size_limit() {
    let source = scene();
    let v = source.version();
    let mut engine = Engine::new(OWNER, &source, &font(), 160, 80).expect("native import");

    // 5000 operations in one frame.
    let many_ops = vec![patch(); 5000];
    assert_eq!(
        engine.try_apply_update(OWNER, update(v, v + 1, &many_ops)),
        Ok(())
    );
    assert_eq!(engine.snapshot_identity(OWNER), Some((v + 1, 2)));

    // One Insert carrying a 5000-node subtree, then 5000 separate inserts in one frame.
    let subtree = Node::container(NodeId(10)).with_children(
        (0..4999)
            .map(|i| Node::text(NodeId(100 + i), "row"))
            .collect(),
    );
    let insert = [ScenePatch::Insert {
        parent: NodeId(1),
        index: 1,
        node: subtree,
    }];
    assert_eq!(
        engine.try_apply_update(OWNER, update(v + 1, v + 2, &insert)),
        Ok(())
    );
    assert_eq!(engine.snapshot_identity(OWNER), Some((v + 2, 5002)));
    let inserts: Vec<ScenePatch> = (0..5000)
        .map(|i| ScenePatch::Insert {
            parent: NodeId(10),
            index: usize::MAX,
            node: Node::text(NodeId(10_000 + i), "more"),
        })
        .collect();

    // A quota one node short rejects the whole frame and consumes no version...
    let short = SceneQuota {
        max_nodes: Some(10_001),
        max_text_bytes: None,
    };
    assert_eq!(
        engine.try_apply_within(OWNER, v + 2, v + 3, &inserts, short),
        Err(SceneUpdateError::NodeQuotaExceeded {
            limit: 10_001,
            required: 10_002
        })
    );
    assert_eq!(engine.snapshot_identity(OWNER), Some((v + 2, 5002)));
    // ...and the exact quota admits the same frame.
    let exact = SceneQuota {
        max_nodes: Some(10_002),
        max_text_bytes: None,
    };
    assert_eq!(
        engine.try_apply_within(OWNER, v + 2, v + 3, &inserts, exact),
        Ok(())
    );
    assert_eq!(engine.snapshot_identity(OWNER), Some((v + 3, 10_002)));
    assert!(engine.node_bounds(OWNER, v + 3, NodeId(14_999)).is_some());
}
