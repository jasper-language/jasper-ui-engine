//! Host-neutral scrollbar geometry, thumb dragging and painting.
//!
//! 滚动条是布局/场景的派生视图：不新增场景节点，也不改 `Node::scroll`。
//! 宿主在 wheel/手势之前调用 [`scrollbars`] 判断是否点中滚动条；
//! 拖动时把 [`ScrollbarDrag::update`] 产出的补丁走正常版本化补丁路径。
//! 绘制时先 [`crate::paint::paint`]，再用 [`paint_scrollbars`] 追加显示列表。
//!
//! 原型边界：只做每容器一条纵向/横向滚动条；无过冲回弹、无自动隐藏/淡出、
//! 无平台主题。

use crate::geometry::{Point, Rect, Size};
use crate::layout::LayoutTree;
use crate::paint::{DisplayList, PaintOp};
use crate::scene::{NodeId, Scene};
use crate::scroll::{scroll_bounds, ScrollBounds, ScrollUpdate};
use crate::style::{Color, Overflow};

const EPSILON: f32 = 0.01;

/// Axis a scrollbar belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollAxis {
    Vertical,
    Horizontal,
}

impl ScrollAxis {
    fn start(self, rect: Rect) -> f32 {
        match self {
            Self::Vertical => rect.y(),
            Self::Horizontal => rect.x(),
        }
    }

    fn extent(self, size: Size) -> f32 {
        match self {
            Self::Vertical => size.height,
            Self::Horizontal => size.width,
        }
    }

    fn get(self, point: Point) -> f32 {
        match self {
            Self::Vertical => point.y,
            Self::Horizontal => point.x,
        }
    }

    fn set(self, point: &mut Point, value: f32) {
        match self {
            Self::Vertical => point.y = value,
            Self::Horizontal => point.x = value,
        }
    }

    fn max_offset(self, bounds: ScrollBounds) -> f32 {
        let max = bounds.max_offset();
        match self {
            Self::Vertical => max.y,
            Self::Horizontal => max.x,
        }
    }
}

/// Scrollbar sizing and colors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollbarConfig {
    /// Thickness of the track/thumb along the cross axis (logical px).
    pub thickness: f32,
    /// Gap between the viewport edge and the track (logical px).
    pub margin: f32,
    /// Lower bound for the thumb length, so tiny ratios stay grabbable.
    pub min_thumb: f32,
    pub track_color: Color,
    pub thumb_color: Color,
}

impl Default for ScrollbarConfig {
    fn default() -> Self {
        Self {
            thickness: 6.0,
            margin: 2.0,
            min_thumb: 24.0,
            track_color: Color::new(0, 0, 0, 32),
            thumb_color: Color::new(0, 0, 0, 96),
        }
    }
}

/// Resolved scrollbar for one axis of one container.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scrollbar {
    pub node: NodeId,
    pub axis: ScrollAxis,
    /// Track rectangle inside the container's content box.
    pub track: Rect,
    /// Current thumb rectangle; always contained by [`Self::track`].
    pub thumb: Rect,
    pub bounds: ScrollBounds,
}

impl Scrollbar {
    /// Builds one axis' scrollbar. Returns `None` when that axis does not
    /// scroll or the track has no usable length.
    pub fn new(
        node: NodeId,
        axis: ScrollAxis,
        viewport: Rect,
        bounds: ScrollBounds,
        offset: Point,
        config: ScrollbarConfig,
    ) -> Option<Self> {
        let max = axis.max_offset(bounds);
        if max <= EPSILON || !max.is_finite() {
            return None;
        }
        let thickness = sanitize(config.thickness);
        if thickness <= 0.0 {
            return None;
        }
        let margin = sanitize(config.margin);
        let track = match axis {
            ScrollAxis::Vertical => Rect::new(
                viewport.right() - margin - thickness,
                viewport.y() + margin,
                thickness,
                (viewport.height() - margin * 2.0).max(0.0),
            ),
            ScrollAxis::Horizontal => Rect::new(
                viewport.x() + margin,
                viewport.bottom() - margin - thickness,
                (viewport.width() - margin * 2.0).max(0.0),
                thickness,
            ),
        };
        let track_len = axis.extent(track.size);
        if track_len <= EPSILON {
            return None;
        }
        let viewport_len = axis.extent(bounds.viewport);
        let content_len = axis.extent(bounds.content);
        if viewport_len <= 0.0 || content_len <= 0.0 {
            return None;
        }

        let ratio = (viewport_len / content_len).clamp(0.0, 1.0);
        let min_thumb = sanitize(config.min_thumb).min(track_len);
        let thumb_len = (track_len * ratio).clamp(min_thumb, track_len);
        let travel = (track_len - thumb_len).max(0.0);
        let position = sanitize_axis(axis.get(offset) / max).clamp(0.0, 1.0) * travel;
        let thumb = match axis {
            ScrollAxis::Vertical => {
                Rect::new(track.x(), track.y() + position, track.width(), thumb_len)
            }
            ScrollAxis::Horizontal => {
                Rect::new(track.x() + position, track.y(), thumb_len, track.height())
            }
        };
        Some(Self {
            node,
            axis,
            track,
            thumb,
            bounds,
        })
    }

    /// Largest offset on this axis.
    pub fn max_offset(self) -> f32 {
        self.axis.max_offset(self.bounds)
    }

    /// True when `point` is on the thumb (track clicks are not a grab).
    pub fn contains(self, point: Point) -> bool {
        self.thumb.contains(point)
    }

    /// Starts a thumb drag at `pointer`. Returns `None` unless the press
    /// landed on the thumb.
    pub fn begin_drag(self, pointer: Point) -> Option<ScrollbarDrag> {
        if !self.thumb.contains(pointer) {
            return None;
        }
        let grab = self.axis.get(pointer) - self.axis.start(self.thumb);
        Some(ScrollbarDrag {
            node: self.node,
            axis: self.axis,
            track: self.track,
            thumb_len: self.axis.extent(self.thumb.size),
            grab,
            max_offset: self.max_offset(),
        })
    }
}

/// In-progress thumb drag. The grab offset inside the thumb is preserved so
/// the thumb does not jump to the pointer on the first move.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollbarDrag {
    pub node: NodeId,
    pub axis: ScrollAxis,
    track: Rect,
    thumb_len: f32,
    grab: f32,
    max_offset: f32,
}

impl ScrollbarDrag {
    /// Maps the current pointer position to a scroll offset. The cross-axis
    /// component of `current` is preserved.
    pub fn update(self, pointer: Point, current: Point) -> ScrollUpdate {
        let track_len = self.axis.extent(self.track.size);
        let travel = (track_len - self.thumb_len).max(0.0);
        let desired =
            sanitize_axis(self.axis.get(pointer) - self.grab - self.axis.start(self.track))
                .clamp(0.0, travel);
        let ratio = if travel <= EPSILON {
            0.0
        } else {
            desired / travel
        };
        let value = ratio * self.max_offset;
        let mut offset = current;
        self.axis.set(&mut offset, value);
        let mut applied = Point::ZERO;
        self.axis.set(&mut applied, value - self.axis.get(current));
        ScrollUpdate {
            node: self.node,
            offset,
            applied,
        }
    }
}

/// Scrollbars resolved for one container, plus the clip they must obey.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scrollbars {
    pub node: NodeId,
    /// Effective ancestor clip from layout; `None` at the root.
    pub clip: Option<Rect>,
    pub vertical: Option<Scrollbar>,
    pub horizontal: Option<Scrollbar>,
}

impl Scrollbars {
    pub fn is_empty(self) -> bool {
        self.vertical.is_none() && self.horizontal.is_none()
    }

    /// Hit-tests the two thumbs, vertical first. Track hits return `None`
    /// because this prototype only supports thumb dragging.
    pub fn hit(self, point: Point) -> Option<Scrollbar> {
        self.vertical
            .filter(|bar| bar.contains(point))
            .or_else(|| self.horizontal.filter(|bar| bar.contains(point)))
    }
}

/// Resolves both scrollbar axes for `node`.
///
/// Returns `None` unless `node` is an `Overflow::Scroll` container present in
/// both trees. A scroll container whose content fits returns an empty
/// [`Scrollbars`] rather than a fake thumb.
pub fn scrollbars(
    layout: &LayoutTree,
    scene: &Scene,
    node: NodeId,
    config: ScrollbarConfig,
) -> Option<Scrollbars> {
    let item = layout.find(node)?;
    let state = scene.find(node)?;
    if state.style.overflow != Overflow::Scroll {
        return None;
    }
    let bounds = scroll_bounds(layout, scene, node)?;
    Some(Scrollbars {
        node,
        clip: item.clip,
        vertical: Scrollbar::new(
            node,
            ScrollAxis::Vertical,
            item.content,
            bounds,
            state.scroll,
            config,
        ),
        horizontal: Scrollbar::new(
            node,
            ScrollAxis::Horizontal,
            item.content,
            bounds,
            state.scroll,
            config,
        ),
    })
}

/// Appends track/thumb fills to a display list. Call after `paint`.
///
/// The scrollbars carry their ancestor clip, so clipped containers cannot
/// paint bars outside their visible region.
pub fn paint_scrollbars(display: &mut DisplayList, bars: Scrollbars, config: ScrollbarConfig) {
    if bars.is_empty() {
        return;
    }
    if let Some(clip) = bars.clip {
        display.push(PaintOp::PushClip(clip));
    }
    for bar in [bars.vertical, bars.horizontal].into_iter().flatten() {
        if config.track_color.a > 0 {
            display.push(PaintOp::FillRect {
                rect: bar.track,
                color: config.track_color,
            });
        }
        if config.thumb_color.a > 0 {
            display.push(PaintOp::FillRect {
                rect: bar.thumb,
                color: config.thumb_color,
            });
        }
    }
    if bars.clip.is_some() {
        display.push(PaintOp::PopClip);
    }
}

fn sanitize(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn sanitize_axis(value: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{LayoutEngine, TaffyLayoutEngine};
    use crate::scene::Node;
    use crate::style::{Length, Style};
    use crate::text::SimpleTextEngine;

    const TEST_CONFIG: ScrollbarConfig = ScrollbarConfig {
        thickness: 6.0,
        margin: 2.0,
        min_thumb: 24.0,
        track_color: Color::new(0, 0, 0, 32),
        thumb_color: Color::new(0, 0, 0, 96),
    };

    fn layout(scene: &Scene) -> LayoutTree {
        TaffyLayoutEngine
            .layout(scene, Size::new(100.0, 100.0), &SimpleTextEngine)
            .unwrap()
    }

    fn scroll_scene(child: Size, offset: Point) -> Scene {
        Scene::new(
            Node::container(NodeId(1))
                .with_style(
                    Style::default()
                        .size(Length::Px(100.0), Length::Px(100.0))
                        .overflow(Overflow::Scroll),
                )
                .scroll(offset)
                .child(Node::rect(NodeId(2)).with_style(Style {
                    min_height: Length::Px(child.height),
                    ..Style::default().size(Length::Px(child.width), Length::Px(child.height))
                })),
        )
    }

    fn bars(scene: &Scene) -> Scrollbars {
        let tree = layout(scene);
        scrollbars(&tree, scene, NodeId(1), TEST_CONFIG).unwrap()
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < EPSILON,
            "expected {expected}, got {actual}"
        );
    }

    fn assert_rect(actual: Rect, expected: Rect) {
        assert_close(actual.x(), expected.x());
        assert_close(actual.y(), expected.y());
        assert_close(actual.width(), expected.width());
        assert_close(actual.height(), expected.height());
    }

    #[test]
    fn vertical_track_and_thumb_follow_offset() {
        let scene = scroll_scene(Size::new(100.0, 300.0), Point::ZERO);
        let resolved = bars(&scene);
        assert!(resolved.horizontal.is_none());
        let bar = resolved.vertical.unwrap();
        assert_eq!(bar.node, NodeId(1));
        assert_eq!(bar.track, Rect::new(92.0, 2.0, 6.0, 96.0));
        assert_rect(bar.thumb, Rect::new(92.0, 2.0, 6.0, 32.0));
        assert_eq!(bar.max_offset(), 200.0);

        let scene = scroll_scene(Size::new(100.0, 300.0), Point::new(0.0, 100.0));
        let bar = bars(&scene).vertical.unwrap();
        assert_rect(bar.thumb, Rect::new(92.0, 34.0, 6.0, 32.0));

        let scene = scroll_scene(Size::new(100.0, 300.0), Point::new(0.0, 200.0));
        let bar = bars(&scene).vertical.unwrap();
        assert_rect(bar.thumb, Rect::new(92.0, 66.0, 6.0, 32.0));
    }

    #[test]
    fn horizontal_track_and_thumb_are_derived_separately() {
        let scene = scroll_scene(Size::new(300.0, 100.0), Point::ZERO);
        let resolved = bars(&scene);
        assert!(resolved.vertical.is_none());
        let bar = resolved.horizontal.unwrap();
        assert_eq!(bar.track, Rect::new(2.0, 92.0, 96.0, 6.0));
        assert_rect(bar.thumb, Rect::new(2.0, 92.0, 32.0, 6.0));
        assert_eq!(bar.max_offset(), 200.0);
    }

    #[test]
    fn both_axes_are_reported_when_content_overflows_both() {
        let scene = scroll_scene(Size::new(300.0, 300.0), Point::ZERO);
        let resolved = bars(&scene);
        assert!(resolved.vertical.is_some());
        assert!(resolved.horizontal.is_some());
        assert!(!resolved.is_empty());
    }

    #[test]
    fn tiny_ratio_respects_min_thumb() {
        let scene = scroll_scene(Size::new(100.0, 10_000.0), Point::ZERO);
        let bar = bars(&scene).vertical.unwrap();
        assert_close(bar.thumb.height(), 24.0);
    }

    #[test]
    fn fitting_or_non_scroll_containers_have_no_bars() {
        let fitting = scroll_scene(Size::new(100.0, 40.0), Point::ZERO);
        assert!(bars(&fitting).is_empty());

        let visible = Scene::new(
            Node::container(NodeId(1))
                .with_style(Style::default().size(Length::Px(100.0), Length::Px(100.0)))
                .child(
                    Node::rect(NodeId(2))
                        .with_style(Style::default().size(Length::Px(100.0), Length::Px(300.0))),
                ),
        );
        assert!(scrollbars(&layout(&visible), &visible, NodeId(1), TEST_CONFIG).is_none());
    }

    #[test]
    fn thumb_hit_and_drag_map_pointer_to_offset() {
        let scene = scroll_scene(Size::new(100.0, 300.0), Point::ZERO);
        let bar = bars(&scene).vertical.unwrap();
        assert!(bar.contains(Point::new(95.0, 18.0)));
        assert!(!bar.contains(Point::new(95.0, 60.0)), "track is not a grab");
        assert!(bar.begin_drag(Point::new(95.0, 60.0)).is_none());

        let drag = bar.begin_drag(Point::new(95.0, 18.0)).unwrap();
        let update = drag.update(Point::new(95.0, 50.0), Point::ZERO);
        assert_eq!(update.node, NodeId(1));
        assert_close(update.offset.y, 100.0);
        assert_close(update.applied.y, 100.0);

        let bottom = drag.update(Point::new(95.0, 500.0), update.offset);
        assert_close(bottom.offset.y, 200.0);
        assert_close(bottom.applied.y, 100.0);

        let top = drag.update(Point::new(95.0, -500.0), bottom.offset);
        assert_close(top.offset.y, 0.0);
        assert_close(top.applied.y, -200.0);
    }

    #[test]
    fn drag_preserves_the_cross_axis_offset() {
        let scene = scroll_scene(Size::new(300.0, 300.0), Point::new(120.0, 40.0));
        let bar = bars(&scene).vertical.unwrap();
        let drag = bar.begin_drag(Point::new(95.0, 18.0)).unwrap();
        let update = drag.update(Point::new(95.0, 18.0), Point::new(120.0, 40.0));
        assert_close(update.offset.x, 120.0);
        assert_close(update.offset.y, 40.0);
        assert_close(update.applied.x, 0.0);
    }

    #[test]
    fn painting_emits_track_then_thumb() {
        let scene = scroll_scene(Size::new(100.0, 300.0), Point::ZERO);
        let bar = bars(&scene).vertical.unwrap();
        let mut display = DisplayList::default();
        paint_scrollbars(&mut display, bars(&scene), TEST_CONFIG);

        match display.ops.as_slice() {
            [PaintOp::FillRect {
                rect: track,
                color: track_color,
            }, PaintOp::FillRect {
                rect: thumb,
                color: thumb_color,
            }] => {
                assert_eq!(*track, bar.track);
                assert_eq!(*thumb, bar.thumb);
                assert_eq!(*track_color, TEST_CONFIG.track_color);
                assert_eq!(*thumb_color, TEST_CONFIG.thumb_color);
            }
            other => panic!("unexpected paint ops: {other:?}"),
        }
    }

    #[test]
    fn painting_wraps_bars_in_the_ancestor_clip() {
        let scene = Scene::new(
            Node::container(NodeId(1))
                .with_style(
                    Style::default()
                        .size(Length::Px(100.0), Length::Px(100.0))
                        .overflow(Overflow::Hidden),
                )
                .child(
                    Node::container(NodeId(2))
                        .with_style(
                            Style::default()
                                .size(Length::Px(100.0), Length::Px(100.0))
                                .overflow(Overflow::Scroll),
                        )
                        .child(Node::rect(NodeId(3)).with_style(Style {
                            min_height: Length::Px(300.0),
                            ..Style::default().size(Length::Px(100.0), Length::Px(300.0))
                        })),
                ),
        );
        let tree = layout(&scene);
        let resolved = scrollbars(&tree, &scene, NodeId(2), TEST_CONFIG).unwrap();
        assert_eq!(resolved.clip, Some(Rect::new(0.0, 0.0, 100.0, 100.0)));

        let mut display = DisplayList::default();
        paint_scrollbars(&mut display, resolved, TEST_CONFIG);
        match display.ops.as_slice() {
            [PaintOp::PushClip(clip), PaintOp::FillRect { .. }, PaintOp::FillRect { .. }, PaintOp::PopClip] =>
            {
                assert_eq!(*clip, Rect::new(0.0, 0.0, 100.0, 100.0))
            }
            other => panic!("unexpected paint ops: {other:?}"),
        }
    }

    #[test]
    fn zero_thickness_produces_no_bar() {
        let scene = scroll_scene(Size::new(100.0, 300.0), Point::ZERO);
        let tree = layout(&scene);
        let config = ScrollbarConfig {
            thickness: 0.0,
            ..TEST_CONFIG
        };
        let resolved = scrollbars(&tree, &scene, NodeId(1), config).unwrap();
        assert!(resolved.is_empty());
    }
}
