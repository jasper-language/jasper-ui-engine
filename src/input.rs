//! Host-independent input model.
//! 平台适配层把原始事件规范化到这里；手势/输入法由 Jasper UI 层后续实现。

use crate::geometry::{Point, Size};

/// Stable pointer identity within one input session.
// BEGIN VERIFIED POINTER ID TYPE
#[derive(Clone, Copy, Debug, Default, Eq, Hash)]
pub struct PointerId(pub u64);
use core::cmp::Ordering;
// END VERIFIED POINTER ID TYPE

// BEGIN VERIFIED POINTER ORDER KERNEL
// Explicit comparison keeps the original u64 identity and total ordering.
// 显式比较保持原有 u64 身份及全序语义。
impl PartialEq for PointerId {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl PartialOrd for PointerId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.0 < other.0 {
            Some(Ordering::Less)
        } else if self.0 > other.0 {
            Some(Ordering::Greater)
        } else {
            Some(Ordering::Equal)
        }
    }
}
impl Ord for PointerId {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.0 < other.0 {
            Ordering::Less
        } else if self.0 > other.0 {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}
// END VERIFIED POINTER ORDER KERNEL

/// What produced the pointer event.
// BEGIN VERIFIED POINTER KIND TYPES
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PointerKind {
    Mouse,
    Touch,
    Pen,
    #[default]
    Unknown,
}

/// Pointer lifecycle phase.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PointerPhase {
    Down,
    Move,
    Up,
    Cancel,
    #[default]
    Hover,
}

/// Mouse/pen button identity. Touch uses `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u16),
}

// END VERIFIED POINTER KIND TYPES

/// Keyboard modifiers.
// BEGIN VERIFIED MODIFIERS TYPE
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}
// END VERIFIED MODIFIERS TYPE

/// Normalized pointer sample.
///
/// Pressure/tilt/twist/contact are meaningful for pen and touch; mouse adapters
/// set pressure to 0.0/1.0 as appropriate. `tilt` is in degrees; `twist` is in
/// degrees for pens that report barrel rotation. `contact` is the contact area
/// in logical pixels (touch major/minor axes, or a small value for pens).
// BEGIN VERIFIED POINTER EVENT TYPE
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointerEvent {
    pub pointer_id: PointerId,
    pub kind: PointerKind,
    pub phase: PointerPhase,
    pub position: Point,
    pub pressure: f32,
    pub tilt: Point,
    pub twist: f32,
    pub contact: Size,
    pub is_primary: bool,
    pub eraser: bool,
    pub button: Option<MouseButton>,
    pub modifiers: Modifiers,
    pub timestamp_millis: u64,
}

// END VERIFIED POINTER EVENT TYPE

/// Keyboard key phase.
// BEGIN VERIFIED KEY PHASE TYPE
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyPhase {
    Down,
    Up,
}
// END VERIFIED KEY PHASE TYPE

/// Normalized key event.
// BEGIN VERIFIED KEY EVENT TYPE
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: String,
    pub phase: KeyPhase,
    pub modifiers: Modifiers,
    pub repeat: bool,
}
// END VERIFIED KEY EVENT TYPE

/// IME composition update. Offsets are UTF-8 byte offsets into the editor text.
// BEGIN VERIFIED COMPOSITION EVENT TYPE
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompositionEvent {
    pub text: String,
    pub start: usize,
    pub end: usize,
}

// END VERIFIED COMPOSITION EVENT TYPE

// BEGIN VERIFIED PREEDIT EVENT TYPE
/// Host preedit text and optional UTF-8 cursor/selection within that text.
/// 宿主预编辑文本及其内部可选 UTF-8 光标/选区；不表示文档替换区间。
pub struct PreeditEvent {
    pub text: String,
    pub cursor: Option<(usize, usize)>,
}
// END VERIFIED PREEDIT EVENT TYPE

// BEGIN VERIFIED SCROLL FOCUS TYPES
/// Scroll delta in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollEvent {
    pub delta: Point,
}

/// Focus change notification.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FocusEvent {
    pub focused: bool,
}

// END VERIFIED SCROLL FOCUS TYPES

// BEGIN VERIFIED NORMALIZED INPUT TYPE
/// All normalized input events.
#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    Pointer(PointerEvent),
    Key(KeyEvent),
    Text(String),
    Composition(CompositionEvent),
    /// Preedit-local coordinates; None hides the cursor, empty text clears preedit.
    /// 预编辑局部坐标；None 隐藏光标，空文本表示清除预编辑。
    Preedit {
        text: String,
        cursor: Option<(usize, usize)>,
    },
    Scroll(ScrollEvent),
    Focus(FocusEvent),
    /// IME availability lifecycle; disabling composition does not remove keyboard focus.
    /// 输入法可用性生命周期；禁用组合不等于移除键盘焦点。
    ImeEnabled(bool),
}
// END VERIFIED NORMALIZED INPUT TYPE

/// Bounded per-frame pointer-motion coalescer used by platform event loops.
/// 平台事件循环使用的单帧有界指针移动合并器。
///
/// Only consecutive move/hover samples from the same pointer are merged. Button, touch
/// begin/end/cancel, wheel, keyboard, text and focus events flush the pending sample before
/// they are returned, so ordering and lifecycle transitions are never dropped.
/// 只合并同一指针连续的 Move/Hover；按键、触控开始/结束/取消、滚轮、键盘、文本和焦点事件
/// 都会先冲刷待处理样本，因此顺序和生命周期转换不会被丢弃。
#[derive(Clone, Debug, Default)]
pub struct PointerMotionCoalescer {
    pending: Option<PointerEvent>,
}

impl PointerMotionCoalescer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one event and return events ready for the UI queue.
    /// 添加一个事件，返回可送入 UI 队列的事件。
    ///
    /// The final pending motion is returned by [`Self::flush`] at the frame boundary.
    /// 最后一个待处理移动样本由帧边界调用 [`Self::flush`] 返回。
    pub fn push(&mut self, event: InputEvent) -> Vec<InputEvent> {
        let InputEvent::Pointer(sample) = event else {
            return self.flush_then(event);
        };
        if !matches!(sample.phase, PointerPhase::Move | PointerPhase::Hover) {
            return self.flush_then(InputEvent::Pointer(sample));
        }
        let replace = self.pending.as_ref().is_some_and(|pending| {
            pending.pointer_id == sample.pointer_id && pending.kind == sample.kind
        });
        if replace {
            // The newest sample already contains every field the platform can observe.
            // 最新样本已包含平台当前可观察的全部字段，因此直接替换旧样本。
            self.pending = Some(sample);
            return Vec::new();
        }
        let mut output = self.flush();
        self.pending = Some(sample);
        output.shrink_to_fit();
        output
    }

    /// Return and clear the final pending motion sample.
    /// 返回并清除最后一个待处理移动样本。
    pub fn flush(&mut self) -> Vec<InputEvent> {
        self.pending
            .take()
            .map(InputEvent::Pointer)
            .into_iter()
            .collect()
    }

    fn flush_then(&mut self, event: InputEvent) -> Vec<InputEvent> {
        let mut output = self.flush();
        output.push(event);
        output
    }
}

/// Linear short-horizon pointer predictor over host timestamps.
/// 基于宿主时间戳的线性短时指针预测器。
///
/// Prediction is advisory and never replaces an authoritative sample. A platform adapter should
/// pass only movement-derived positions into presentation, then reconcile with the next real
/// Move/Hover sample. Different pointer identities and lifecycle events reset the history.
/// 预测仅作提示，不能替代权威样本；平台适配器只能用于呈现，并须在下一真实 Move/Hover 到达时
/// 校正。不同指针身份及生命周期事件会清空历史。
#[derive(Clone, Debug, Default)]
pub struct PointerMotionPredictor {
    last: Option<PointerEvent>,
    velocity_per_millisecond: Point,
}

impl PointerMotionPredictor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Observe an authoritative sample. Only same-pointer Move/Hover contributes velocity.
    /// 观察权威样本；只有同一指针的 Move/Hover 参与速度计算。
    pub fn observe(&mut self, event: &PointerEvent) {
        if !matches!(event.phase, PointerPhase::Move | PointerPhase::Hover) {
            self.reset();
            return;
        }
        if let Some(previous) = self.last.as_ref() {
            if previous.pointer_id == event.pointer_id
                && previous.kind == event.kind
                && event.timestamp_millis > previous.timestamp_millis
            {
                let elapsed = (event.timestamp_millis - previous.timestamp_millis) as f32;
                self.velocity_per_millisecond = Point::new(
                    (event.position.x - previous.position.x) / elapsed,
                    (event.position.y - previous.position.y) / elapsed,
                );
            } else {
                self.velocity_per_millisecond = Point::ZERO;
            }
        } else {
            self.velocity_per_millisecond = Point::ZERO;
        }
        self.last = Some(event.clone());
    }

    /// Return an advisory copy of the latest sample projected by `horizon_millis`.
    /// 返回最新样本按 `horizon_millis` 外推得到的提示副本。
    pub fn predict(&self, horizon_millis: u64) -> Option<PointerEvent> {
        let mut predicted = self.last.clone()?;
        let horizon = horizon_millis as f32;
        predicted.position = Point::new(
            predicted.position.x + self.velocity_per_millisecond.x * horizon,
            predicted.position.y + self.velocity_per_millisecond.y * horizon,
        );
        predicted.timestamp_millis = predicted.timestamp_millis.saturating_add(horizon_millis);
        Some(predicted)
    }

    pub fn velocity_per_millisecond(&self) -> Point {
        self.velocity_per_millisecond
    }

    pub fn reset(&mut self) {
        self.last = None;
        self.velocity_per_millisecond = Point::ZERO;
    }
}

/// Active pointer tracking for multi-touch and multi-pen input.
// BEGIN VERIFIED POINTER TRACKER TYPE
#[derive(Clone, Debug, Default)]
pub struct PointerTracker {
    active: BTreeMap<PointerId, PointerEvent>,
}
use std::collections::BTreeMap;
// END VERIFIED POINTER TRACKER TYPE

impl PointerTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn track(&mut self, event: PointerEvent) -> Result<(), InputError> {
        match event.phase {
            PointerPhase::Down => {
                if self.active.contains_key(&event.pointer_id) {
                    return Err(InputError::DuplicatePointer(event.pointer_id));
                }
                self.active.insert(event.pointer_id, event);
            }
            PointerPhase::Move | PointerPhase::Hover => {
                // Mouse hover/move without a pressed button is not an active
                // pointer; keep it out of the active map instead of failing.
                if self.active.contains_key(&event.pointer_id) {
                    self.active.insert(event.pointer_id, event);
                }
            }
            PointerPhase::Up | PointerPhase::Cancel => {
                if self.active.remove(&event.pointer_id).is_none() {
                    return Err(InputError::UnknownPointer(event.pointer_id));
                }
            }
        }
        Ok(())
    }

    pub fn get(&self, pointer_id: PointerId) -> Option<&PointerEvent> {
        self.active.get(&pointer_id)
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    pub fn active(&self) -> impl Iterator<Item = &PointerEvent> {
        self.active.values()
    }

    pub fn clear(&mut self) {
        self.active.clear();
    }
}

/// Input tracking failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputError {
    DuplicatePointer(PointerId),
    UnknownPointer(PointerId),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pointer(id: u64, phase: PointerPhase, kind: PointerKind) -> PointerEvent {
        PointerEvent {
            pointer_id: PointerId(id),
            kind,
            phase,
            ..Default::default()
        }
    }

    #[test]
    fn tracks_multiple_touches_and_pen_axes() {
        let mut tracker = PointerTracker::new();
        tracker
            .track(pointer(1, PointerPhase::Down, PointerKind::Touch))
            .unwrap();
        let mut pen = pointer(2, PointerPhase::Down, PointerKind::Pen);
        pen.pressure = 0.7;
        pen.tilt = Point::new(10.0, -5.0);
        pen.twist = 45.0;
        pen.contact = Size::new(1.0, 1.0);
        tracker.track(pen.clone()).unwrap();
        assert_eq!(tracker.active_count(), 2);
        assert_eq!(tracker.get(PointerId(2)).unwrap().pressure, 0.7);
        tracker
            .track(pointer(1, PointerPhase::Up, PointerKind::Touch))
            .unwrap();
        assert_eq!(tracker.active_count(), 1);
    }

    #[test]
    fn rejects_duplicate_down_and_unknown_up() {
        let mut tracker = PointerTracker::new();
        tracker
            .track(pointer(1, PointerPhase::Down, PointerKind::Touch))
            .unwrap();
        assert_eq!(
            tracker.track(pointer(1, PointerPhase::Down, PointerKind::Touch)),
            Err(InputError::DuplicatePointer(PointerId(1)))
        );
        assert_eq!(
            tracker.track(pointer(9, PointerPhase::Up, PointerKind::Touch)),
            Err(InputError::UnknownPointer(PointerId(9)))
        );
    }

    fn touch_pointer(id: u64, phase: PointerPhase) -> PointerEvent {
        PointerEvent {
            pointer_id: PointerId(id),
            kind: PointerKind::Touch,
            phase,
            position: Point::new(id as f32, 0.0),
            ..Default::default()
        }
    }

    #[test]
    fn coalescer_keeps_lifecycle_events_and_latest_motion_per_pointer() {
        let mut coalescer = PointerMotionCoalescer::new();
        let down = InputEvent::Pointer(touch_pointer(1, PointerPhase::Down));
        assert_eq!(coalescer.push(down.clone()), vec![down]);

        let mut first = touch_pointer(1, PointerPhase::Move);
        first.position = Point::new(10.0, 20.0);
        let mut second = touch_pointer(1, PointerPhase::Move);
        second.position = Point::new(30.0, 40.0);
        assert!(coalescer.push(InputEvent::Pointer(first)).is_empty());
        assert!(coalescer
            .push(InputEvent::Pointer(second.clone()))
            .is_empty());

        let up = InputEvent::Pointer(touch_pointer(1, PointerPhase::Up));
        assert_eq!(
            coalescer.push(up.clone()),
            vec![InputEvent::Pointer(second), up]
        );
        assert!(coalescer.flush().is_empty());
    }

    #[test]
    fn coalescer_flushes_before_non_pointer_and_separates_pointer_identities() {
        let mut coalescer = PointerMotionCoalescer::new();
        let mut one = touch_pointer(1, PointerPhase::Move);
        one.position = Point::new(1.0, 2.0);
        let mut two = touch_pointer(2, PointerPhase::Move);
        two.position = Point::new(3.0, 4.0);
        assert!(coalescer.push(InputEvent::Pointer(one.clone())).is_empty());
        assert_eq!(
            coalescer.push(InputEvent::Pointer(two.clone())),
            vec![InputEvent::Pointer(one)]
        );
        assert_eq!(
            coalescer.push(InputEvent::Text("kept".into())),
            vec![InputEvent::Pointer(two), InputEvent::Text("kept".into())]
        );
    }

    #[test]
    fn predictor_uses_same_pointer_velocity_and_resets_on_lifecycle() {
        let mut predictor = PointerMotionPredictor::new();
        let mut first = touch_pointer(1, PointerPhase::Move);
        first.position = Point::new(10.0, 20.0);
        first.timestamp_millis = 100;
        predictor.observe(&first);
        assert_eq!(predictor.predict(8).unwrap().position, first.position);

        let mut second = touch_pointer(1, PointerPhase::Move);
        second.position = Point::new(30.0, 40.0);
        second.timestamp_millis = 110;
        predictor.observe(&second);
        assert_eq!(predictor.velocity_per_millisecond(), Point::new(2.0, 2.0));
        let predicted = predictor.predict(5).unwrap();
        assert_eq!(predicted.position, Point::new(40.0, 50.0));
        assert_eq!(predicted.timestamp_millis, 115);

        let mut other = touch_pointer(2, PointerPhase::Move);
        other.position = Point::new(100.0, 100.0);
        other.timestamp_millis = 120;
        predictor.observe(&other);
        assert_eq!(predictor.velocity_per_millisecond(), Point::ZERO);
        assert_eq!(predictor.predict(5).unwrap().position, other.position);

        let mut foreign = other.clone();
        foreign.pointer_id = PointerId(1);
        predictor.observe(&foreign);
        assert_eq!(predictor.velocity_per_millisecond(), Point::ZERO);
        predictor.observe(&touch_pointer(1, PointerPhase::Up));
        assert!(predictor.predict(1).is_none());
    }
}
