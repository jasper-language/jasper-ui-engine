//! Scroll extent resolution, wheel routing and fling (inertia) physics.
//! 宿主中立：只消费布局树/场景并产出 `ScenePatch::SetScroll`，不碰平台事件。
//!
//! 约定：
//! - 滚动偏移 `Node::scroll` 是“内容相对视口的位移”，`(0, 0)` 表示顶部/最左；
//! - `delta` 为逻辑像素，正值表示内容向视口上方滚动（即 offset 增加）；
//! - 一次滚轮事件沿祖先链从内到外消费：内层到边界后，残余 delta 继续交给外层；
//! - 仍在原型阶段：没有滚动条、没有过冲回弹。

use std::collections::BTreeMap;

use crate::geometry::{Point, Size};
use crate::layout::LayoutTree;
use crate::scene::{NodeId, Scene, ScenePatch};
use crate::style::Overflow;

const EPSILON: f32 = 0.01;

/// Resolved scrollable range of one container.
///
/// `viewport` is the container's content box; `content` is the union of its
/// direct children, at least as large as the viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollBounds {
    pub viewport: Size,
    pub content: Size,
}

impl ScrollBounds {
    /// Largest offset that keeps the content edge aligned with the viewport.
    pub fn max_offset(self) -> Point {
        Point::new(
            (self.content.width - self.viewport.width).max(0.0),
            (self.content.height - self.viewport.height).max(0.0),
        )
    }

    /// True when the content overflows the viewport on either axis.
    pub fn is_scrollable(self) -> bool {
        let max = self.max_offset();
        max.x > 0.0 || max.y > 0.0
    }
}

/// One resolved scroll movement, ready to become a scene patch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollUpdate {
    pub node: NodeId,
    pub offset: Point,
    /// Offset change actually applied after clamping (`offset - previous`).
    pub applied: Point,
}

impl ScrollUpdate {
    pub fn patch(self) -> ScenePatch {
        ScenePatch::SetScroll {
            id: self.node,
            offset: self.offset,
        }
    }
}

/// Tunable fling physics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollConfig {
    /// Fraction of velocity retained after one second of deceleration.
    pub friction: f32,
    /// Flings slower than this (logical px/s) stop immediately.
    pub min_velocity: f32,
    /// Upper bound applied to a release velocity (logical px/s).
    pub max_velocity: f32,
    /// Multiplier applied to the release velocity before clamping.
    pub fling_gain: f32,
}

impl Default for ScrollConfig {
    fn default() -> Self {
        Self {
            friction: 0.05,
            min_velocity: 20.0,
            max_velocity: 8000.0,
            fling_gain: 1.0,
        }
    }
}

/// Wheel routing plus per-node fling state.
///
/// The controller itself does not mutate the scene: callers apply
/// [`ScrollUpdate::patch`] through the normal versioned patch path.
#[derive(Clone, Debug, Default)]
pub struct ScrollController {
    config: ScrollConfig,
    velocities: BTreeMap<NodeId, Point>,
}

impl ScrollController {
    pub fn new(config: ScrollConfig) -> Self {
        Self {
            config,
            velocities: BTreeMap::new(),
        }
    }

    pub fn config(&self) -> ScrollConfig {
        self.config
    }

    /// Routes a wheel/trackpad delta through the scrollable ancestors of
    /// `point`, innermost first. Each container consumes as much of the
    /// remaining delta as it can and the residual is handed to the next one.
    ///
    /// Returns one update per container that actually moved, in inner-to-outer
    /// order, so callers can apply them in sequence. Any fling on the chain is
    /// cancelled even when the delta cannot be consumed.
    pub fn wheel(
        &mut self,
        layout: &LayoutTree,
        scene: &Scene,
        point: Point,
        delta: Point,
    ) -> Vec<ScrollUpdate> {
        if delta.x == 0.0 && delta.y == 0.0 {
            return Vec::new();
        }
        let chain = scroll_chain(layout, point);
        if chain.is_empty() {
            return Vec::new();
        }
        let updates = resolve_in_chain(&chain, layout, scene, delta);
        for id in &chain {
            self.velocities.remove(id);
        }
        updates
    }

    /// Starts a fling from a release velocity. Returns the stored (gained and
    /// capped) velocity, or `None` when it is too slow to matter.
    pub fn begin_fling(&mut self, node: NodeId, velocity: Point) -> Option<Point> {
        let gain = self.config.fling_gain.max(0.0);
        let gained = Point::new(velocity.x * gain, velocity.y * gain);
        let speed = hypot(gained);
        if !speed.is_finite() || speed < self.config.min_velocity {
            self.velocities.remove(&node);
            return None;
        }
        let scale = if speed > self.config.max_velocity {
            self.config.max_velocity / speed
        } else {
            1.0
        };
        let stored = Point::new(gained.x * scale, gained.y * scale);
        self.velocities.insert(node, stored);
        Some(stored)
    }

    /// Advances all flings by `dt_seconds` and returns the movements to apply.
    /// `dt` is capped so a stalled host does not teleport the content.
    pub fn tick(
        &mut self,
        layout: &LayoutTree,
        scene: &Scene,
        dt_seconds: f32,
    ) -> Vec<ScrollUpdate> {
        if dt_seconds <= 0.0 || !dt_seconds.is_finite() {
            return Vec::new();
        }
        let dt = dt_seconds.min(0.1);
        let active: Vec<(NodeId, Point)> = self
            .velocities
            .iter()
            .map(|(id, velocity)| (*id, *velocity))
            .collect();
        let mut updates = Vec::new();
        let mut settled = Vec::new();
        for (id, velocity) in active {
            let Some(node) = scene.find(id) else {
                settled.push(id);
                continue;
            };
            let Some(bounds) = scroll_bounds(layout, scene, id) else {
                settled.push(id);
                continue;
            };
            let max = bounds.max_offset();
            let requested = Point::new(
                node.scroll.x + velocity.x * dt,
                node.scroll.y + velocity.y * dt,
            );
            let offset = clamp_offset(requested, max);
            let decay = self.config.friction.powf(dt);
            let mut next_velocity = Point::new(velocity.x * decay, velocity.y * decay);
            if (offset.x - requested.x).abs() > EPSILON {
                next_velocity.x = 0.0;
            }
            if (offset.y - requested.y).abs() > EPSILON {
                next_velocity.y = 0.0;
            }
            let applied = Point::new(offset.x - node.scroll.x, offset.y - node.scroll.y);
            if applied.x.abs() > EPSILON || applied.y.abs() > EPSILON {
                updates.push(ScrollUpdate {
                    node: id,
                    offset,
                    applied,
                });
            }
            if hypot(next_velocity) < self.config.min_velocity {
                settled.push(id);
            } else {
                self.velocities.insert(id, next_velocity);
            }
        }
        for id in settled {
            self.velocities.remove(&id);
        }
        updates
    }

    /// Current fling velocity for `node`, if it is still moving.
    pub fn velocity(&self, node: NodeId) -> Option<Point> {
        self.velocities.get(&node).copied()
    }

    pub fn is_flinging(&self) -> bool {
        !self.velocities.is_empty()
    }

    pub fn cancel_fling(&mut self, node: NodeId) {
        self.velocities.remove(&node);
    }

    pub fn cancel_all(&mut self) {
        self.velocities.clear();
    }
}

/// Resolves the scrollable range of `node` from a layout pass.
///
/// Direct children are unioned in the container's unscrolled coordinate space
/// (the layout tree already has `Node::scroll` applied to child positions).
pub fn scroll_bounds(layout: &LayoutTree, scene: &Scene, node: NodeId) -> Option<ScrollBounds> {
    let item = layout.find(node)?;
    let state = scene.find(node)?;
    let content_origin = item.content.origin;
    let mut content = item.content.size;
    for child in layout
        .boxes
        .iter()
        .filter(|child| child.parent == Some(node))
    {
        // The layout pass shifted children by `-scroll`; add it back so the
        // extent does not shrink as the container scrolls.
        let right = child.rect.right() + state.scroll.x - content_origin.x;
        let bottom = child.rect.bottom() + state.scroll.y - content_origin.y;
        content.width = content.width.max(right);
        content.height = content.height.max(bottom);
    }
    Some(ScrollBounds {
        viewport: item.content.size,
        content,
    })
}

/// Deepest-to-root chain of boxes under `point`.
///
/// Unlike hit testing this keeps transparent containers, because a scroll
/// container must still receive wheel input over its blank areas.
pub fn scroll_chain(layout: &LayoutTree, point: Point) -> Vec<NodeId> {
    let Some(hit) = layout.boxes.iter().rev().find(|item| {
        item.rect.contains(point) && item.clip.map(|clip| clip.contains(point)).unwrap_or(true)
    }) else {
        return Vec::new();
    };
    let mut chain = Vec::new();
    let mut current = Some(hit.node_id);
    while let Some(id) = current {
        chain.push(id);
        current = layout.find(id).and_then(|item| item.parent);
    }
    chain
}

fn resolve_in_chain(
    chain: &[NodeId],
    layout: &LayoutTree,
    scene: &Scene,
    delta: Point,
) -> Vec<ScrollUpdate> {
    let mut remaining = delta;
    let mut updates = Vec::new();
    for id in chain {
        if remaining.x.abs() <= EPSILON && remaining.y.abs() <= EPSILON {
            break;
        }
        let Some(node) = scene.find(*id) else {
            continue;
        };
        if node.style.overflow != Overflow::Scroll {
            continue;
        }
        let Some(max) = scroll_bounds(layout, scene, *id).map(ScrollBounds::max_offset) else {
            continue;
        };
        let previous = node.scroll;
        let mut offset = previous;
        if remaining.x != 0.0 {
            offset.x = (previous.x + remaining.x).clamp(0.0, max.x);
        }
        if remaining.y != 0.0 {
            offset.y = (previous.y + remaining.y).clamp(0.0, max.y);
        }
        let applied = Point::new(offset.x - previous.x, offset.y - previous.y);
        // Whatever this container could not absorb continues outward. Keeping
        // the per-axis residual means a vertical wheel can still hand its
        // leftover to an outer container after the inner one hits its edge.
        remaining = Point::new(remaining.x - applied.x, remaining.y - applied.y);
        if applied.x.abs() > EPSILON || applied.y.abs() > EPSILON {
            updates.push(ScrollUpdate {
                node: *id,
                offset,
                applied,
            });
        }
    }
    updates
}

fn clamp_offset(offset: Point, max: Point) -> Point {
    Point::new(
        offset.x.clamp(0.0, max.x.max(0.0)),
        offset.y.clamp(0.0, max.y.max(0.0)),
    )
}

fn hypot(point: Point) -> f32 {
    (point.x * point.x + point.y * point.y).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{LayoutEngine, TaffyLayoutEngine};
    use crate::scene::Node;
    use crate::style::{Length, Style};
    use crate::text::SimpleTextEngine;

    fn layout(scene: &Scene, viewport: Size) -> LayoutTree {
        TaffyLayoutEngine
            .layout(scene, viewport, &SimpleTextEngine)
            .unwrap()
    }

    fn scroll_scene(offset: Point, child_height: f32) -> Scene {
        Scene::new(
            Node::container(NodeId(1))
                .with_style(
                    Style::default()
                        .size(Length::Px(100.0), Length::Px(100.0))
                        .overflow(Overflow::Scroll),
                )
                .scroll(offset)
                .child(Node::rect(NodeId(2)).with_style(Style {
                    min_height: Length::Px(child_height),
                    ..Style::default().size(Length::Px(100.0), Length::Px(child_height))
                })),
        )
    }

    fn nested_scroll_scene() -> Scene {
        Scene::new(
            Node::container(NodeId(1))
                .with_style(
                    Style::default()
                        .size(Length::Px(200.0), Length::Px(200.0))
                        .overflow(Overflow::Scroll),
                )
                .child(
                    Node::container(NodeId(2))
                        .with_style(
                            Style::default()
                                .size(Length::Px(100.0), Length::Px(100.0))
                                .overflow(Overflow::Scroll),
                        )
                        .child(Node::rect(NodeId(3)).with_style(Style {
                            min_height: Length::Px(200.0),
                            ..Style::default().size(Length::Px(100.0), Length::Px(200.0))
                        })),
                )
                .child(Node::rect(NodeId(4)).with_style(Style {
                    min_height: Length::Px(400.0),
                    ..Style::default().size(Length::Px(100.0), Length::Px(400.0))
                })),
        )
    }

    #[test]
    fn resolves_content_extent_and_max_offset() {
        let scene = scroll_scene(Point::ZERO, 300.0);
        let tree = layout(&scene, Size::new(100.0, 100.0));
        let bounds = scroll_bounds(&tree, &scene, NodeId(1)).unwrap();
        assert_eq!(bounds.viewport, Size::new(100.0, 100.0));
        assert_eq!(bounds.content, Size::new(100.0, 300.0));
        assert_eq!(bounds.max_offset(), Point::new(0.0, 200.0));
        assert!(bounds.is_scrollable());

        let fitting = scroll_scene(Point::ZERO, 40.0);
        let tree = layout(&fitting, Size::new(100.0, 100.0));
        let bounds = scroll_bounds(&tree, &fitting, NodeId(1)).unwrap();
        assert!(!bounds.is_scrollable());
        assert_eq!(bounds.max_offset(), Point::ZERO);
    }

    #[test]
    fn wheel_scrolls_and_clamps_to_bounds() {
        let mut scene = scroll_scene(Point::ZERO, 300.0);
        let tree = layout(&scene, Size::new(100.0, 100.0));
        let mut controller = ScrollController::default();

        let updates =
            controller.wheel(&tree, &scene, Point::new(50.0, 50.0), Point::new(0.0, 30.0));
        assert_eq!(
            updates,
            vec![ScrollUpdate {
                node: NodeId(1),
                offset: Point::new(0.0, 30.0),
                applied: Point::new(0.0, 30.0),
            }]
        );
        scene.apply(updates[0].patch()).unwrap();

        let tree = layout(&scene, Size::new(100.0, 100.0));
        let updates = controller.wheel(
            &tree,
            &scene,
            Point::new(50.0, 50.0),
            Point::new(0.0, 500.0),
        );
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].offset, Point::new(0.0, 200.0));
        assert_eq!(updates[0].applied, Point::new(0.0, 170.0));
        scene.apply(updates[0].patch()).unwrap();

        let tree = layout(&scene, Size::new(100.0, 100.0));
        assert!(
            controller
                .wheel(&tree, &scene, Point::new(50.0, 50.0), Point::new(0.0, 10.0))
                .is_empty(),
            "at the bottom the event must not invent an offset"
        );
    }

    #[test]
    fn wheel_at_origin_cannot_scroll_up() {
        let scene = scroll_scene(Point::ZERO, 300.0);
        let tree = layout(&scene, Size::new(100.0, 100.0));
        let mut controller = ScrollController::default();
        assert!(controller
            .wheel(
                &tree,
                &scene,
                Point::new(50.0, 50.0),
                Point::new(0.0, -10.0)
            )
            .is_empty());
    }

    #[test]
    fn wheel_chains_to_the_outer_scroll_container() {
        let mut scene = nested_scroll_scene();
        let tree = layout(&scene, Size::new(200.0, 200.0));
        let mut controller = ScrollController::default();

        let inner = controller
            .wheel(
                &tree,
                &scene,
                Point::new(50.0, 50.0),
                Point::new(0.0, 150.0),
            )
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(inner.node, NodeId(2));
        assert_eq!(inner.offset, Point::new(0.0, 100.0));
        scene.apply(inner.patch()).unwrap();

        let tree = layout(&scene, Size::new(200.0, 200.0));
        let outer = controller
            .wheel(&tree, &scene, Point::new(50.0, 50.0), Point::new(0.0, 50.0))
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(
            outer.node,
            NodeId(1),
            "an inner container at its limit should hand off to the parent"
        );
        assert_eq!(outer.offset, Point::new(0.0, 50.0));
    }

    #[test]
    fn wheel_splits_residual_delta_across_the_chain() {
        let scene = nested_scroll_scene();
        let tree = layout(&scene, Size::new(200.0, 200.0));
        let mut controller = ScrollController::default();

        let updates = controller.wheel(
            &tree,
            &scene,
            Point::new(50.0, 50.0),
            Point::new(0.0, 250.0),
        );
        assert_eq!(
            updates,
            vec![
                ScrollUpdate {
                    node: NodeId(2),
                    offset: Point::new(0.0, 100.0),
                    applied: Point::new(0.0, 100.0),
                },
                ScrollUpdate {
                    node: NodeId(1),
                    offset: Point::new(0.0, 150.0),
                    applied: Point::new(0.0, 150.0),
                },
            ],
            "inner consumes its full range, the 150px residual goes to the outer container"
        );
    }

    #[test]
    fn wheel_residual_is_clamped_by_the_outer_container() {
        let scene = nested_scroll_scene();
        let tree = layout(&scene, Size::new(200.0, 200.0));
        let mut controller = ScrollController::default();

        let updates = controller.wheel(
            &tree,
            &scene,
            Point::new(50.0, 50.0),
            Point::new(0.0, 10_000.0),
        );
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].node, NodeId(2));
        assert_eq!(updates[0].offset, Point::new(0.0, 100.0));
        assert_eq!(updates[1].node, NodeId(1));
        assert_eq!(
            updates[1].offset,
            Point::new(0.0, 300.0),
            "outer max offset is content (500) minus viewport (200)"
        );
        assert_eq!(updates[1].applied, Point::new(0.0, 300.0));
    }

    #[test]
    fn wheel_cancels_fling_even_when_already_at_the_edge() {
        let scene = scroll_scene(Point::new(0.0, 200.0), 300.0);
        let tree = layout(&scene, Size::new(100.0, 100.0));
        let mut controller = ScrollController::default();
        controller.begin_fling(NodeId(1), Point::new(0.0, 1000.0));

        let updates =
            controller.wheel(&tree, &scene, Point::new(50.0, 50.0), Point::new(0.0, 10.0));
        assert!(updates.is_empty());
        assert!(
            !controller.is_flinging(),
            "wheel input must stop inertia even without movement"
        );
    }

    #[test]
    fn fling_decays_and_stops_at_the_bottom_edge() {
        let mut scene = scroll_scene(Point::ZERO, 300.0);
        let mut controller = ScrollController::default();
        assert!(controller
            .begin_fling(NodeId(1), Point::new(0.0, 1000.0))
            .is_some());

        let mut last = 0.0;
        let mut frames = 0;
        loop {
            let tree = layout(&scene, Size::new(100.0, 100.0));
            let updates = controller.tick(&tree, &scene, 0.016);
            if updates.is_empty() {
                break;
            }
            for update in updates {
                assert!(update.offset.y >= last, "fling must move monotonically");
                assert!(update.offset.y <= 200.0, "fling must stay clamped");
                last = update.offset.y;
                scene.apply(update.patch()).unwrap();
            }
            frames += 1;
            assert!(frames < 1000, "fling did not settle");
        }
        assert!((last - 200.0).abs() < EPSILON);
        assert!(!controller.is_flinging());
    }

    #[test]
    fn fling_stops_when_velocity_falls_below_threshold() {
        let scene = scroll_scene(Point::ZERO, 3000.0);
        let tree = layout(&scene, Size::new(100.0, 100.0));
        let mut controller = ScrollController::new(ScrollConfig {
            min_velocity: 100.0,
            ..Default::default()
        });
        assert!(controller
            .begin_fling(NodeId(1), Point::new(0.0, 101.0))
            .is_some());
        let updates = controller.tick(&tree, &scene, 0.05);
        assert_eq!(updates.len(), 1);
        assert!(updates[0].offset.y > 0.0 && updates[0].offset.y < 200.0);
        assert!(!controller.is_flinging(), "slow tail must not keep ticking");
    }

    #[test]
    fn slow_fling_is_rejected_and_wheel_cancels_an_active_fling() {
        let scene = scroll_scene(Point::ZERO, 300.0);
        let tree = layout(&scene, Size::new(100.0, 100.0));
        let mut controller = ScrollController::new(ScrollConfig {
            min_velocity: 100.0,
            ..Default::default()
        });
        assert!(controller
            .begin_fling(NodeId(1), Point::new(0.0, 20.0))
            .is_none());
        assert!(!controller.is_flinging());

        controller.begin_fling(NodeId(1), Point::new(0.0, 1000.0));
        assert!(controller.is_flinging());
        let updates =
            controller.wheel(&tree, &scene, Point::new(50.0, 50.0), Point::new(0.0, 10.0));
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].offset, Point::new(0.0, 10.0));
        assert!(!controller.is_flinging(), "wheel input must stop inertia");
    }

    #[test]
    fn transparent_scroll_container_is_part_of_the_chain() {
        let scene = scroll_scene(Point::ZERO, 300.0);
        let tree = layout(&scene, Size::new(100.0, 100.0));
        assert_eq!(
            scroll_chain(&tree, Point::new(50.0, 50.0)),
            vec![NodeId(2), NodeId(1)]
        );
    }
}
