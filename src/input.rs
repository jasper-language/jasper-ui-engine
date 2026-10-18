//! Host-independent input model.
//! 平台适配层把原始事件规范化到这里；手势/输入法由 Jasper UI 层后续实现。

use std::collections::BTreeMap;

use crate::geometry::{Point, Size};

/// Stable pointer identity within one input session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PointerId(pub u64);

/// What produced the pointer event.
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u16),
}

/// Keyboard modifiers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}

/// Normalized pointer sample.
///
/// Pressure/tilt/twist/contact are meaningful for pen and touch; mouse adapters
/// set pressure to 0.0/1.0 as appropriate. `tilt` is in degrees; `twist` is in
/// degrees for pens that report barrel rotation. `contact` is the contact area
/// in logical pixels (touch major/minor axes, or a small value for pens).
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

/// Keyboard key phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyPhase {
    Down,
    Up,
}

/// Normalized key event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: String,
    pub phase: KeyPhase,
    pub modifiers: Modifiers,
    pub repeat: bool,
}

/// IME composition update. Offsets are UTF-8 byte offsets into the editor text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompositionEvent {
    pub text: String,
    pub start: usize,
    pub end: usize,
}

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

/// All normalized input events.
#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    Pointer(PointerEvent),
    Key(KeyEvent),
    Text(String),
    Composition(CompositionEvent),
    Scroll(ScrollEvent),
    Focus(FocusEvent),
}

/// Active pointer tracking for multi-touch and multi-pen input.
#[derive(Clone, Debug, Default)]
pub struct PointerTracker {
    active: BTreeMap<PointerId, PointerEvent>,
}

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
}
