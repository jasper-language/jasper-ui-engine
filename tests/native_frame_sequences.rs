//! Ordered multi-frame Engine acceptance for additions, removals and replacements.
//! 正式 Engine 的多帧验收：新增、删除、同批换序、版本推进及拒绝后整帧保持。
#![cfg(feature = "native-blitz-cpu-pending")]

use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::geometry::Edges;
use jasper_ui_engine::native::{EventReply, PointerAction, SceneEventKind, SceneUpdateError};
use jasper_ui_engine::scene::{Node, NodeId, Scene, ScenePatch};
use jasper_ui_engine::semantics::{Role, Semantics};
use jasper_ui_engine::style::{Color, Display, FlexDirection, Length, Style};
use proptest::prelude::*;

const OWNER: u64 = 41;
const ROOT: NodeId = NodeId(1);
const FOCUS: NodeId = NodeId(2);
const W: u32 = 96;
const H: u32 = 72;
const STRIDE: usize = (W * 4) as usize;
const BUDGET: usize = (W * H * 4) as usize;

fn font() -> Vec<u8> {
    std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap()
}

fn leaf(id: NodeId, color: Color, width: f32, height: f32) -> Node {
    Node::rect(id).with_style(
        Style::default()
            .size(Length::Px(width), Length::Px(height))
            .background(color),
    )
}

fn scene() -> Scene {
    let focus = Node::text(FOCUS, "go")
        .with_semantics(
            Semantics::default()
                .role(Role::Button)
                .label("go")
                .focusable(true),
        )
        .with_style(
            Style::default()
                .size(Length::Px(28.0), Length::Px(20.0))
                .background(Color::GREEN),
        );
    Scene::new(
        Node::container(ROOT)
            .with_style(
                Style::default()
                    .size(Length::Px(W as f32), Length::Px(H as f32))
                    .padding(Edges::all(4.0))
                    .gap(2.0)
                    .direction(FlexDirection::Row)
                    .background(Color::rgb(18, 22, 30)),
            )
            .child(focus)
            .child(leaf(NodeId(3), Color::RED, 18.0, 16.0))
            .child(leaf(NodeId(4), Color::BLUE, 22.0, 16.0)),
    )
}

fn pixels(engine: &mut Engine, version: u64) -> Vec<u8> {
    let mut bytes = vec![0; BUDGET];
    assert!(engine.render_to_surface(OWNER, version, &mut bytes, STRIDE, BUDGET));
    bytes
}

fn node_count(node: &Node) -> usize {
    1 + node.children.iter().map(node_count).sum::<usize>()
}

fn removable(scene: &Scene) -> Vec<NodeId> {
    scene
        .root
        .children
        .iter()
        .filter_map(|node| (node.id != FOCUS).then_some(node.id))
        .collect()
}

fn styled(id: NodeId, background: Color, width: f32, display: Display, weight: u16) -> ScenePatch {
    ScenePatch::UpdateStyle {
        id,
        style: Style::default()
            .size(Length::Px(width), Length::Px(16.0))
            .background(background)
            .display(display)
            .font(weight, jasper_ui_engine::style::FontStyle::Normal),
    }
}

fn assert_matches_fresh(live: &mut Engine, source: &Scene, font: &[u8]) {
    let version = source.version();
    assert_eq!(
        live.snapshot_identity(OWNER),
        Some((version, node_count(&source.root)))
    );
    assert_eq!(live.focused_node(OWNER, version), Some(FOCUS));
    let live_pixels = pixels(live, version);
    let mut fresh = Engine::new(OWNER, source, font, W, H).unwrap();
    assert!(fresh.focus(OWNER, version, FOCUS, &mut |_| {}));
    assert_eq!(live_pixels, pixels(&mut fresh, version));
}

#[test]
fn ordered_add_remove_and_same_batch_reorder_match_fresh_native_import() {
    let font = font();
    let mut source = scene();
    let mut live = Engine::new(OWNER, &source, &font, W, H).unwrap();
    assert!(live.focus(OWNER, source.version(), FOCUS, &mut |_| {}));

    let inserted = Node::container(NodeId(5))
        .with_style(
            Style::default()
                .size(Length::Px(26.0), Length::Px(16.0))
                .background(Color::GREEN),
        )
        .child(leaf(NodeId(6), Color::rgb(220, 140, 30), 14.0, 16.0));
    let frames: &[&[ScenePatch]] = &[
        &[ScenePatch::Insert {
            parent: ROOT,
            index: 1,
            node: inserted,
        }],
        &[ScenePatch::Remove { id: NodeId(3) }],
        &[
            ScenePatch::Remove { id: NodeId(4) },
            ScenePatch::Insert {
                parent: ROOT,
                index: 1,
                node: leaf(NodeId(4), Color::BLUE, 18.0, 16.0),
            },
        ],
        &[ScenePatch::UpdateStyle {
            id: FOCUS,
            style: Style::default()
                .size(Length::Px(32.0), Length::Px(22.0))
                .background(Color::rgb(40, 170, 100)),
        }],
    ];
    for patches in frames {
        let old = source.version();
        source.apply_batch_at(old, patches).unwrap();
        assert_eq!(
            live.try_apply(OWNER, old, source.version(), patches),
            Ok(())
        );
        assert_matches_fresh(&mut live, &source, &font);
    }
    assert_eq!(
        live.snapshot_identity(OWNER).map(|(_, count)| count),
        Some(5)
    );
}

#[test]
fn remove_and_reinsert_id_does_not_transfer_focus_or_pressed_identity() {
    let font = font();
    let button = || {
        Node::text(FOCUS, "go")
            .with_semantics(
                Semantics::default()
                    .role(Role::Button)
                    .label("go")
                    .focusable(true),
            )
            .with_style(
                Style::default()
                    .size(Length::Px(28.0), Length::Px(20.0))
                    .background(Color::GREEN),
            )
    };
    let mut source = Scene::new(Node::container(ROOT).child(button()));
    let mut live = Engine::new(OWNER, &source, &font, W, H).unwrap();
    let version = source.version();
    assert!(live.focus(OWNER, version, FOCUS, &mut |_| {}));
    assert!(live.pointer(
        OWNER,
        version,
        PointerAction::Down,
        8.0,
        8.0,
        true,
        &mut |_| EventReply::default(),
        &mut |_| {},
    ));

    let replacement = [
        ScenePatch::Remove { id: FOCUS },
        ScenePatch::Insert {
            parent: ROOT,
            index: 0,
            node: button(),
        },
    ];
    let next = version + 1;
    source.apply_batch_at(version, &replacement).unwrap();
    assert_eq!(live.try_apply(OWNER, version, next, &replacement), Ok(()));
    assert_eq!(live.focused_node(OWNER, next), None);

    let mut activations = Vec::new();
    assert!(live.pointer(
        OWNER,
        next,
        PointerAction::Up,
        8.0,
        8.0,
        false,
        &mut |event| {
            if event.kind == SceneEventKind::Activate {
                activations.push(event.node);
            }
            EventReply::default()
        },
        &mut |_| {},
    ));
    assert!(activations.is_empty());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn random_frame_sequences_commit_only_complete_successors(choices in prop::collection::vec(any::<u8>(),4..12)) {
        let font=font();
        let mut source=scene();
        let mut live=Engine::new(OWNER,&source,&font,W,H).unwrap();
        assert!(live.focus(OWNER,source.version(),FOCUS,&mut |_|{}));
        let _=pixels(&mut live,source.version());
        assert_eq!(live.needs_frame(OWNER),Some(false));
        let mut next_id=10u64;

        for (step,choice) in choices.into_iter().enumerate() {
            let old=source.version();
            let before=source.clone();
            let patches:Vec<ScenePatch>=match choice%8 {
                0|1=>{
                    let id=NodeId(next_id);next_id+=1;
                    vec![ScenePatch::Insert {parent:ROOT,index:choice as usize,
                        node:leaf(id,Color::rgb(choice,120,220u8.saturating_sub(choice)),12.0+choice as f32%20.0,16.0)}]
                }
                2|3=>{
                    let ids=removable(&source);
                    if ids.is_empty() {continue;}
                    vec![ScenePatch::Remove {id:ids[choice as usize%ids.len()]}]
                }
                4=>{
                    let ids=removable(&source);
                    if ids.is_empty() {continue;}
                    vec![styled(ids[choice as usize%ids.len()],Color::rgb(90,choice,180),
                        16.0+choice as f32%28.0,
                        if choice&0x40==0 {Display::Block}else{Display::None},
                        if choice&1==0 {400}else{700})]
                }
                5=>{
                    let ids=removable(&source);
                    if ids.is_empty() {continue;}
                    let id=ids[choice as usize%ids.len()];
                    let node=source.find(id).unwrap().clone();
                    vec![ScenePatch::Remove {id},
                        ScenePatch::Insert {parent:ROOT,index:choice as usize,node}]
                }
                6=>vec![ScenePatch::UpdateSemantics {id:FOCUS,semantics:Semantics::default()
                    .role(Role::Button).focusable(true).label(format!("go-{step}"))}],
                _=>{
                    // Exercise both source rejection and a source-valid frame rejected by
                    // native candidate construction; neither may consume a version.
                    if choice&1==0 {
                        vec![ScenePatch::Remove {id:NodeId(99_999)}]
                    } else {
                        let mut invalid=Style::default().size(Length::Px(20.0),Length::Px(16.0));
                        invalid.font_size=0.0;
                        vec![ScenePatch::UpdateStyle {id:FOCUS,style:invalid}]
                    }
                }
            };

            let source_result=source.apply_batch_at(old,&patches);
            let engine_result=live.try_apply(OWNER,old,old.checked_add(1).unwrap(),&patches);
            if source_result.is_err() {
                prop_assert!(matches!(&engine_result,Err(SceneUpdateError::SourceRejected)));
                source=before;
            } else if choice%8==7 {
                prop_assert!(matches!(&engine_result,Err(SceneUpdateError::NativeRejected)));
                source=before;
            } else {
                prop_assert_eq!(&engine_result,&Ok(()));
            }

            if engine_result.is_err() {
                prop_assert_eq!(live.snapshot_identity(OWNER),Some((old,node_count(&source.root))));
                prop_assert_eq!(live.focused_node(OWNER,old),Some(FOCUS));
                prop_assert_eq!(live.needs_frame(OWNER),Some(false));
                let rejected=pixels(&mut live,old);
                let mut fresh=Engine::new(OWNER,&source,&font,W,H).unwrap();
                let focused=fresh.focus(OWNER,old,FOCUS,&mut |_|{});
                prop_assert!(focused);
                prop_assert_eq!(rejected,pixels(&mut fresh,old));
            } else {
                assert_matches_fresh(&mut live,&source,&font);
            }
        }
    }
}
