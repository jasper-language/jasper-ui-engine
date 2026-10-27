//! Pointer routing with capture, built on hit testing.
//! 平台无关：触摸/多点/触控笔共用同一套 PointerEvent。

use std::collections::BTreeMap;

use crate::hit_test::hit_test_path;
use crate::input::{InputError, PointerEvent, PointerId, PointerPhase, PointerTracker};
use crate::layout::LayoutTree;
use crate::resource::ResourceGate;
use crate::scene::NodeId;

/// Result of routing one pointer sample.
#[derive(Clone, Debug, PartialEq)]
pub struct PointerRoute {
    pub event: PointerEvent,
    pub target: Option<NodeId>,
    /// Root-to-target path for bubbling; empty when the press started outside.
    pub path: Vec<NodeId>,
    /// True when the pointer is captured (Down/Move/Up/Cancel after a Down).
    pub captured: bool,
}

// BEGIN VERIFIED POINTER ROUTER TYPE
#[derive(Clone, Debug)]
struct Capture {
    target: Option<NodeId>,
    path: Vec<NodeId>,
}

/// Routes pointer events to scene nodes and maintains per-pointer capture.
#[derive(Clone, Debug, Default)]
pub struct PointerRouter {
    tracker: PointerTracker,
    captures: BTreeMap<PointerId, Capture>,
}
// END VERIFIED POINTER ROUTER TYPE

impl PointerRouter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn route(
        &mut self,
        event: PointerEvent,
        layout: &LayoutTree,
    ) -> Result<PointerRoute, InputError> {
        let hit_path = hit_test_path(layout, event.position);
        let hit_target = hit_path.last().copied();

        match event.phase {
            PointerPhase::Down => {
                self.tracker.track(event.clone())?;
                self.captures.insert(
                    event.pointer_id,
                    Capture {
                        target: hit_target,
                        path: hit_path.clone(),
                    },
                );
                Ok(PointerRoute {
                    event,
                    target: hit_target,
                    path: hit_path,
                    captured: true,
                })
            }
            PointerPhase::Move => {
                self.tracker.track(event.clone())?;
                if let Some(capture) = self.captures.get(&event.pointer_id) {
                    Ok(PointerRoute {
                        event,
                        target: capture.target,
                        path: capture.path.clone(),
                        captured: true,
                    })
                } else {
                    Ok(PointerRoute {
                        event,
                        target: hit_target,
                        path: hit_path,
                        captured: false,
                    })
                }
            }
            PointerPhase::Up | PointerPhase::Cancel => {
                self.tracker.track(event.clone())?;
                if let Some(capture) = self.captures.remove(&event.pointer_id) {
                    Ok(PointerRoute {
                        event,
                        target: capture.target,
                        path: capture.path,
                        captured: true,
                    })
                } else {
                    Ok(PointerRoute {
                        event,
                        target: hit_target,
                        path: hit_path,
                        captured: false,
                    })
                }
            }
            PointerPhase::Hover => {
                self.tracker.track(event.clone())?;
                Ok(PointerRoute {
                    event,
                    target: hit_target,
                    path: hit_path,
                    captured: false,
                })
            }
        }
    }

    pub fn active_count(&self) -> usize {
        self.tracker.active_count()
    }
}

// BEGIN VERIFIED CAPTURE QUERY KERNEL
impl PointerRouter {
    pub fn has_capture(&self, pointer_id: PointerId) -> bool {
        self.captures.contains_key(&pointer_id)
    }

    /// Returns the original captured node, including when current hit testing differs.
    /// 返回捕获时的原目标，不按当前命中结果重新归属。
    /// None means absent capture or a capture that started outside all nodes.
    /// None 表示没有捕获或捕获起始于所有节点之外；用 has_capture 区分。
    pub fn captured_target(&self, pointer_id: PointerId) -> Option<NodeId> {
        if let Some(capture) = self.captures.get(&pointer_id) {
            capture.target
        } else {
            None
        }
    }
}
// END VERIFIED CAPTURE QUERY KERNEL

/// Mouse hover transition.
// BEGIN VERIFIED HOVER CHANGE TYPE
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoverChange {
    Enter(NodeId),
    Leave(NodeId),
}
// END VERIFIED HOVER CHANGE TYPE

/// Tracks which node the mouse is over and emits enter/leave only on changes.
///
/// The caller feeds [`PointerRoute::target`] from hover samples; this keeps
/// per-frame mouse moves from producing redundant host work.
// BEGIN VERIFIED HOVER STORAGE TYPE
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HoverTracker {
    current: Option<NodeId>,
}
// END VERIFIED HOVER STORAGE TYPE

impl HoverTracker {
    pub fn new() -> Self {
        Self::default()
    }
}

// BEGIN VERIFIED HOVER UPDATE KERNEL
impl HoverTracker {
    pub fn hovered(&self) -> Option<NodeId> {
        self.current
    }

    /// Returns `Leave(old)` then `Enter(new)` when the target changes.
    pub fn update(&mut self, target: Option<NodeId>) -> Vec<HoverChange> {
        let unchanged = match (target, self.current) {
            (None, None) => true,
            (Some(next), Some(previous)) => next.0 == previous.0,
            _ => false,
        };
        if unchanged {
            return Vec::new();
        }
        let mut out = Vec::new();
        if let Some(old) = self.current {
            out.push(HoverChange::Leave(old));
        }
        if let Some(new) = target {
            out.push(HoverChange::Enter(new));
        }
        self.current = target;
        out
    }

    /// Clears hover, emitting a leave for the previous target.
    pub fn clear(&mut self) -> Vec<HoverChange> {
        self.update(None)
    }
}
// END VERIFIED HOVER UPDATE KERNEL

// BEGIN VERIFIED HOVER RETIRE KERNEL
impl HoverTracker {
    /// Host close: retire admission and return the real hover leave intent once.
    /// 宿主关闭：永久封闭准入，并取回真实悬停离开意图。
    /// The host pairs ownership and gates every input; delivery remains its responsibility.
    /// 宿主负责配对归属、所有输入入口准入检查和意图投递。
    /// This does not cancel pointer capture, presses, gestures or IME composition.
    /// 此方法不取消指针捕获、按压、手势或输入法组合。
    pub fn retire_hover(&mut self, gate: &mut ResourceGate) -> Vec<HoverChange> {
        gate.retire();
        self.clear()
    }
}
// END VERIFIED HOVER RETIRE KERNEL

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Point, Rect, Size};
    use crate::input::PointerKind;
    use crate::layout::{LayoutBox, LayoutTree};
    use crate::style::{Color, Style};

    fn layout() -> LayoutTree {
        let root_style = Style::default().background(Color::BLACK);
        let child_style = Style::default().background(Color::RED);
        LayoutTree {
            viewport: Size::new(100.0, 100.0),
            scene_version: 1,
            boxes: vec![
                LayoutBox {
                    node_id: NodeId(1),
                    parent: None,
                    rect: Rect::new(0.0, 0.0, 100.0, 100.0),
                    content: Rect::new(0.0, 0.0, 100.0, 100.0),
                    style: root_style,
                    text: None,
                    canvas: None,
                    clip: None,
                },
                LayoutBox {
                    node_id: NodeId(2),
                    parent: Some(NodeId(1)),
                    rect: Rect::new(10.0, 10.0, 30.0, 30.0),
                    content: Rect::new(10.0, 10.0, 30.0, 30.0),
                    style: child_style,
                    text: None,
                    canvas: None,
                    clip: None,
                },
            ],
        }
    }

    fn event(id: u64, phase: PointerPhase, x: f32, y: f32) -> PointerEvent {
        PointerEvent {
            pointer_id: PointerId(id),
            kind: PointerKind::Touch,
            phase,
            position: Point::new(x, y),
            ..Default::default()
        }
    }

    #[test]
    fn captured_target_retains_original_owner_until_cancel() {
        let tree = layout();
        let mut router = PointerRouter::new();
        assert_eq!(router.captured_target(PointerId(1)), None);
        router
            .route(event(1, PointerPhase::Down, 15.0, 15.0), &tree)
            .unwrap();
        router
            .route(event(1, PointerPhase::Move, 90.0, 90.0), &tree)
            .unwrap();
        assert_eq!(router.captured_target(PointerId(1)), Some(NodeId(2)));
        router
            .route(event(1, PointerPhase::Cancel, 90.0, 90.0), &tree)
            .unwrap();
        assert!(!router.has_capture(PointerId(1)));
        assert_eq!(router.captured_target(PointerId(1)), None);
        router
            .route(event(2, PointerPhase::Down, -1.0, -1.0), &tree)
            .unwrap();
        assert!(router.has_capture(PointerId(2)));
        assert_eq!(router.captured_target(PointerId(2)), None);
    }

    #[test]
    fn host_retirement_returns_hover_leave_once() {
        let mut hover = HoverTracker::new();
        assert_eq!(
            hover.update(Some(NodeId(2))),
            vec![HoverChange::Enter(NodeId(2))]
        );
        let mut gate = ResourceGate::new(7, 42);
        let key = gate.issue(7).unwrap();
        assert_eq!(
            hover.retire_hover(&mut gate),
            vec![HoverChange::Leave(NodeId(2))]
        );
        assert_eq!(hover.hovered(), None);
        assert!(hover.retire_hover(&mut gate).is_empty());
        assert!(!gate.allows(7, key));
        assert_eq!(gate.issue(7), None);
    }

    #[test]
    fn down_move_up_keep_capture() {
        let tree = layout();
        let mut router = PointerRouter::new();
        let down = router
            .route(event(1, PointerPhase::Down, 15.0, 15.0), &tree)
            .unwrap();
        assert_eq!(down.target, Some(NodeId(2)));
        assert_eq!(down.path, vec![NodeId(1), NodeId(2)]);
        assert!(down.captured);

        let moved = router
            .route(event(1, PointerPhase::Move, 80.0, 80.0), &tree)
            .unwrap();
        assert_eq!(
            moved.target,
            Some(NodeId(2)),
            "capture must survive leaving the node"
        );
        assert!(moved.captured);

        let up = router
            .route(event(1, PointerPhase::Up, 80.0, 80.0), &tree)
            .unwrap();
        assert_eq!(up.target, Some(NodeId(2)));
        assert!(up.captured);
        assert!(!router.has_capture(PointerId(1)));
    }

    #[test]
    fn multi_touch_captures_are_independent() {
        let tree = layout();
        let mut router = PointerRouter::new();
        let first = router
            .route(event(1, PointerPhase::Down, 15.0, 15.0), &tree)
            .unwrap();
        let second = router
            .route(event(2, PointerPhase::Down, 90.0, 90.0), &tree)
            .unwrap();
        assert_eq!(first.target, Some(NodeId(2)));
        assert_eq!(second.target, Some(NodeId(1)));
        assert_eq!(router.active_count(), 2);
        assert!(router.has_capture(PointerId(1)));
        assert!(router.has_capture(PointerId(2)));
    }

    #[test]
    fn hover_is_not_captured() {
        let tree = layout();
        let mut router = PointerRouter::new();
        let hover = router
            .route(event(1, PointerPhase::Hover, 15.0, 15.0), &tree)
            .unwrap();
        assert_eq!(hover.target, Some(NodeId(2)));
        assert!(!hover.captured);
        assert!(!router.has_capture(PointerId(1)));
    }

    #[test]
    fn hover_tracker_emits_changes_only_on_target_switch() {
        let mut hover = HoverTracker::new();
        assert_eq!(
            hover.update(Some(NodeId(1))),
            vec![HoverChange::Enter(NodeId(1))]
        );
        assert_eq!(hover.update(Some(NodeId(1))), Vec::new());
        assert_eq!(
            hover.update(Some(NodeId(2))),
            vec![HoverChange::Leave(NodeId(1)), HoverChange::Enter(NodeId(2))]
        );
        assert_eq!(hover.hovered(), Some(NodeId(2)));
        assert_eq!(hover.clear(), vec![HoverChange::Leave(NodeId(2))]);
        assert_eq!(hover.hovered(), None);
    }
}
