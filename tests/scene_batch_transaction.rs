//! Real `Scene::apply_batch_at` against an independent tree model; test evidence, not a proof.
//! 真实 `Scene::apply_batch_at` 对照独立树模型；这是测试证据，不是证明。
use jasper_ui_engine::canvas::Canvas;
use jasper_ui_engine::geometry::{Point, Rect};
use jasper_ui_engine::scene::{Node, NodeId, NodeKind, Scene, SceneError, ScenePatch};
use jasper_ui_engine::semantics::Semantics;
use jasper_ui_engine::style::Style;
use proptest::prelude::*;

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Container,
    Text(String),
    Canvas(u32),
}

#[derive(Clone, Debug, PartialEq)]
struct Tree {
    id: u64,
    kind: Kind,
    gap: f32,
    label: String,
    scroll: f32,
    children: Vec<Tree>,
}

impl Tree {
    fn new(id: u64, kind: Kind) -> Self {
        Self {
            id,
            kind,
            gap: 0.0,
            label: String::new(),
            scroll: 0.0,
            children: Vec::new(),
        }
    }
    fn holds(&self, id: u64) -> bool {
        self.id == id || self.children.iter().any(|child| child.holds(id))
    }
    fn at(&mut self, id: u64) -> Option<&mut Tree> {
        if self.id == id {
            return Some(self);
        }
        self.children.iter_mut().find_map(|child| child.at(id))
    }
    fn drop_node(&mut self, id: u64) -> bool {
        if let Some(index) = self.children.iter().position(|child| child.id == id) {
            self.children.remove(index);
            return true;
        }
        self.children.iter_mut().any(|child| child.drop_node(id))
    }
}

/// Generated operation; the same value drives the production patch and the model.
#[derive(Clone, Debug)]
enum Op {
    Insert {
        parent: u64,
        index: usize,
        id: u64,
        kind: Kind,
        child: Option<u64>,
    },
    Style {
        id: u64,
        gap: u8,
    },
    Semantics {
        id: u64,
        label: u8,
    },
    Text {
        id: u64,
        text: u8,
    },
    Canvas {
        id: u64,
        width: u8,
    },
    Scroll {
        id: u64,
        x: u8,
    },
    Remove {
        id: u64,
    },
}

/// A canvas identified by its hit-region count, so the model never compares drawing operations.
fn canvas_with(regions: u32) -> Canvas {
    let mut canvas = Canvas::new();
    for id in 0..regions {
        canvas.hit_region(Rect::default(), id);
    }
    canvas
}

fn native_kind(id: u64, kind: &Kind) -> Node {
    match kind {
        Kind::Container => Node::container(NodeId(id)),
        Kind::Text(text) => Node::text(NodeId(id), text.clone()),
        Kind::Canvas(width) => Node::canvas(NodeId(id), canvas_with(*width)),
    }
}

fn patch_of(op: &Op) -> ScenePatch {
    match op {
        Op::Insert {
            parent,
            index,
            id,
            kind,
            child,
        } => {
            let mut node = native_kind(*id, kind);
            if let Some(child) = child {
                node = node.child(Node::container(NodeId(*child)));
            }
            ScenePatch::Insert {
                parent: NodeId(*parent),
                index: *index,
                node,
            }
        }
        Op::Style { id, gap } => ScenePatch::UpdateStyle {
            id: NodeId(*id),
            style: Style {
                gap: *gap as f32,
                ..Style::default()
            },
        },
        Op::Semantics { id, label } => ScenePatch::UpdateSemantics {
            id: NodeId(*id),
            semantics: Semantics {
                label: label.to_string(),
                ..Semantics::default()
            },
        },
        Op::Text { id, text } => ScenePatch::UpdateText {
            id: NodeId(*id),
            text: text.to_string(),
        },
        Op::Canvas { id, width } => ScenePatch::SetCanvas {
            id: NodeId(*id),
            canvas: canvas_with(*width as u32),
        },
        Op::Scroll { id, x } => ScenePatch::SetScroll {
            id: NodeId(*id),
            offset: Point {
                x: *x as f32,
                y: 0.0,
            },
        },
        Op::Remove { id } => ScenePatch::Remove { id: NodeId(*id) },
    }
}

fn model_apply(tree: &mut Tree, op: &Op) -> Result<(), SceneError> {
    let missing = |id: u64| SceneError::NodeNotFound(NodeId(id));
    match op {
        Op::Insert {
            parent,
            index,
            id,
            kind,
            child,
        } => {
            if tree.holds(*id) {
                return Err(SceneError::DuplicateNode(NodeId(*id)));
            }
            if let Some(child) = child {
                if tree.holds(*child) || child == id {
                    return Err(SceneError::DuplicateNode(NodeId(*child)));
                }
            }
            let target = tree.at(*parent).ok_or(missing(*parent))?;
            let mut inserted = Tree::new(*id, kind.clone());
            if let Some(child) = child {
                inserted.children.push(Tree::new(*child, Kind::Container));
            }
            let at = (*index).min(target.children.len());
            target.children.insert(at, inserted);
        }
        Op::Style { id, gap } => tree.at(*id).ok_or(missing(*id))?.gap = *gap as f32,
        Op::Semantics { id, label } => tree.at(*id).ok_or(missing(*id))?.label = label.to_string(),
        Op::Text { id, text } => {
            let target = tree.at(*id).ok_or(missing(*id))?;
            if !matches!(target.kind, Kind::Text(_)) {
                return Err(SceneError::NotText(NodeId(*id)));
            }
            target.kind = Kind::Text(text.to_string());
        }
        Op::Canvas { id, width } => {
            let target = tree.at(*id).ok_or(missing(*id))?;
            if !matches!(target.kind, Kind::Canvas(_)) {
                return Err(SceneError::NotCanvas(NodeId(*id)));
            }
            target.kind = Kind::Canvas(*width as u32);
        }
        Op::Scroll { id, x } => tree.at(*id).ok_or(missing(*id))?.scroll = *x as f32,
        Op::Remove { id } => {
            if tree.id == *id {
                return Err(SceneError::CannotRemoveRoot);
            }
            if !tree.drop_node(*id) {
                return Err(missing(*id));
            }
        }
    }
    Ok(())
}

fn observed(node: &Node) -> Tree {
    Tree {
        id: node.id.0,
        kind: match &node.kind {
            NodeKind::Container => Kind::Container,
            NodeKind::Text(text) => Kind::Text(text.clone()),
            NodeKind::Canvas(canvas) => Kind::Canvas(canvas.hit_regions().len() as u32),
            other => panic!("scene built only from generated kinds, found {other:?}"),
        },
        gap: node.style.gap,
        label: node.semantics.label.clone(),
        scroll: node.scroll.x,
        children: node.children.iter().map(observed).collect(),
    }
}

/// Runs each batch at a current or stale version and checks the whole observable outcome.
fn check_batches(batches: &[(u8, Vec<Op>)]) {
    let mut scene = Scene::new(Node::container(NodeId(0)));
    let mut model = Tree::new(0, Kind::Container);
    let mut version = scene.version();
    for (stale_by, ops) in batches {
        let before = scene.clone();
        let patches: Vec<ScenePatch> = ops.iter().map(patch_of).collect();
        let expected_version = if *stale_by % 4 == 3 {
            version + 1 + *stale_by as u64
        } else {
            version
        };
        let result = scene.apply_batch_at(expected_version, &patches);
        if expected_version != version {
            assert_eq!(
                result,
                Err(SceneError::StaleVersion {
                    expected: expected_version,
                    actual: version
                })
            );
            assert_eq!(scene, before);
            continue;
        }
        let mut staged = model.clone();
        let outcome = ops.iter().try_for_each(|op| model_apply(&mut staged, op));
        assert_eq!(result, outcome);
        if outcome.is_ok() {
            model = staged;
            if !ops.is_empty() {
                version += 1;
            }
        } else {
            assert_eq!(scene, before);
        }
        assert_eq!(scene.version(), version);
        assert_eq!(observed(&scene.root), model);
        assert_eq!(scene.validate(), Ok(()));
    }
}

#[test]
fn fixed_batches_commit_whole_or_not_at_all() {
    let insert = |parent, id, kind| Op::Insert {
        parent,
        index: usize::MAX,
        id,
        kind,
        child: None,
    };
    check_batches(&[
        (0, vec![]),
        (
            0,
            vec![
                insert(0, 1, Kind::Text("a".into())),
                insert(0, 2, Kind::Canvas(0)),
            ],
        ),
        // The second patch fails, so the applied first patch must not survive.
        (
            0,
            vec![Op::Text { id: 1, text: 7 }, Op::Text { id: 2, text: 8 }],
        ),
        // A later patch observes an earlier patch of the same batch.
        (
            0,
            vec![
                insert(2, 3, Kind::Container),
                Op::Remove { id: 3 },
                Op::Canvas { id: 2, width: 4 },
            ],
        ),
        (3, vec![Op::Remove { id: 1 }]),
        (0, vec![insert(0, 4, Kind::Container), Op::Remove { id: 0 }]),
        (
            0,
            vec![Op::Insert {
                parent: 0,
                index: 0,
                id: 5,
                kind: Kind::Container,
                child: Some(5),
            }],
        ),
        (
            0,
            vec![Op::Style { id: 9, gap: 1 }, Op::Scroll { id: 0, x: 2 }],
        ),
        (
            0,
            vec![
                Op::Style { id: 0, gap: 1 },
                Op::Semantics { id: 1, label: 3 },
                Op::Scroll { id: 0, x: 2 },
            ],
        ),
    ]);
}

fn op() -> impl Strategy<Value = Op> {
    let id = 0u64..7;
    let kind = prop_oneof![
        Just(Kind::Container),
        any::<u8>().prop_map(|text| Kind::Text(text.to_string())),
        (0u32..3).prop_map(Kind::Canvas),
    ];
    prop_oneof![
        3 => (id.clone(), 0usize..4, id.clone(), kind, prop::option::of(id.clone()))
            .prop_map(|(parent, index, id, kind, child)| Op::Insert { parent, index, id, kind, child }),
        1 => (id.clone(), any::<u8>()).prop_map(|(id, gap)| Op::Style { id, gap }),
        1 => (id.clone(), any::<u8>()).prop_map(|(id, label)| Op::Semantics { id, label }),
        1 => (id.clone(), any::<u8>()).prop_map(|(id, text)| Op::Text { id, text }),
        1 => (id.clone(), any::<u8>()).prop_map(|(id, width)| Op::Canvas { id, width: width % 3 }),
        1 => (id.clone(), any::<u8>()).prop_map(|(id, x)| Op::Scroll { id, x }),
        1 => id.prop_map(|id| Op::Remove { id }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    #[cfg_attr(miri, ignore)]
    fn batches_match_the_model(batches in prop::collection::vec((any::<u8>(), prop::collection::vec(op(), 0..5)), 0..12)) {
        check_batches(&batches);
    }
}
