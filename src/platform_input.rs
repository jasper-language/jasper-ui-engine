//! Host-neutral raw pointer/stylus sample admission.
//! 宿主中立的原始指针/触控笔样本准入。
//!
//! Windows Ink, Apple Pencil, Android MotionEvent and Linux libinput adapters should validate
//! their device-specific values here before publishing [`PointerEvent`] to Jasper UI. This
//! module contains no platform API and does not claim any physical device has been exercised.
//! Windows Ink、Apple Pencil、Android MotionEvent 与 Linux libinput 适配器应先在此验证设备值，
//! 再发布 [`PointerEvent`]。本模块不含平台 API，也不声明已实测任何物理设备。

use crate::geometry::{Point, Size};
use crate::input::{Modifiers, MouseButton, PointerEvent, PointerId, PointerKind, PointerPhase};

/// Platform-independent subset required from every raw touch/stylus adapter.
/// 每个原始触控/触控笔适配器必须提供的平台无关字段。
#[derive(Clone, Debug, PartialEq)]
pub struct RawPointerSample {
    pub pointer_id: u64,
    pub kind: PointerKind,
    pub phase: PointerPhase,
    pub position: Point,
    pub pressure: Option<f32>,
    pub tilt_x_degrees: Option<f32>,
    pub tilt_y_degrees: Option<f32>,
    pub twist_degrees: Option<f32>,
    pub contact: Size,
    pub is_primary: bool,
    pub eraser: bool,
    pub button: Option<MouseButton>,
    pub modifiers: Modifiers,
    pub timestamp_millis: u64,
}

/// Invalid raw field. Rejection never reaches Jasper gesture or layout code.
/// 无效原始字段；拒绝值不会进入 Jasper 手势或布局代码。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RawPointerError {
    UnknownKind,
    NonFinitePosition,
    PressureOutOfRange,
    TiltOutOfRange,
    TwistNotFinite,
    ContactOutOfRange,
    TouchHasButton,
    NonPenHasEraser,
}

fn finite(value: f32) -> bool {
    value.is_finite()
}

/// Convert one adapter sample into the normalized input model.
/// 把一个适配器样本转换为规范化输入模型。
pub fn normalize_pointer(sample: RawPointerSample) -> Result<PointerEvent, RawPointerError> {
    if sample.kind == PointerKind::Unknown {
        return Err(RawPointerError::UnknownKind);
    }
    if !finite(sample.position.x) || !finite(sample.position.y) {
        return Err(RawPointerError::NonFinitePosition);
    }
    if sample
        .pressure
        .is_some_and(|pressure| !finite(pressure) || !(0.0..=1.0).contains(&pressure))
    {
        return Err(RawPointerError::PressureOutOfRange);
    }
    let tilt_x = sample.tilt_x_degrees.unwrap_or(0.0);
    let tilt_y = sample.tilt_y_degrees.unwrap_or(0.0);
    if !finite(tilt_x)
        || !finite(tilt_y)
        || !(-90.0..=90.0).contains(&tilt_x)
        || !(-90.0..=90.0).contains(&tilt_y)
    {
        return Err(RawPointerError::TiltOutOfRange);
    }
    let twist = sample.twist_degrees.unwrap_or(0.0);
    if !finite(twist) {
        return Err(RawPointerError::TwistNotFinite);
    }
    if !finite(sample.contact.width)
        || !finite(sample.contact.height)
        || sample.contact.width < 0.0
        || sample.contact.height < 0.0
    {
        return Err(RawPointerError::ContactOutOfRange);
    }
    if sample.kind == PointerKind::Touch && sample.button.is_some() {
        return Err(RawPointerError::TouchHasButton);
    }
    if sample.kind != PointerKind::Pen && sample.eraser {
        return Err(RawPointerError::NonPenHasEraser);
    }
    let pressure = sample.pressure.unwrap_or_else(|| {
        if matches!(sample.phase, PointerPhase::Down | PointerPhase::Move) {
            1.0
        } else {
            0.0
        }
    });
    Ok(PointerEvent {
        pointer_id: PointerId(sample.pointer_id),
        kind: sample.kind,
        phase: sample.phase,
        position: sample.position,
        pressure,
        tilt: Point::new(tilt_x, tilt_y),
        twist: twist.rem_euclid(360.0),
        contact: sample.contact,
        is_primary: sample.is_primary,
        eraser: sample.eraser,
        button: sample.button,
        modifiers: sample.modifiers,
        timestamp_millis: sample.timestamp_millis,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pen() -> RawPointerSample {
        RawPointerSample {
            pointer_id: 7,
            kind: PointerKind::Pen,
            phase: PointerPhase::Move,
            position: Point::new(12.5, 21.0),
            pressure: Some(0.65),
            tilt_x_degrees: Some(20.0),
            tilt_y_degrees: Some(-10.0),
            twist_degrees: Some(-45.0),
            contact: Size::new(2.0, 1.5),
            is_primary: true,
            eraser: false,
            button: Some(MouseButton::Left),
            modifiers: Modifiers::default(),
            timestamp_millis: 99,
        }
    }

    #[test]
    fn preserves_pen_axes_after_validation() {
        let event = normalize_pointer(pen()).unwrap();
        assert_eq!(event.pointer_id, PointerId(7));
        assert_eq!(event.kind, PointerKind::Pen);
        assert_eq!(event.pressure, 0.65);
        assert_eq!(event.tilt, Point::new(20.0, -10.0));
        assert_eq!(event.twist, 315.0);
        assert_eq!(event.contact, Size::new(2.0, 1.5));
    }

    #[test]
    fn rejects_invalid_physical_ranges_without_a_partial_event() {
        let mut sample = pen();
        sample.pressure = Some(1.01);
        assert_eq!(
            normalize_pointer(sample),
            Err(RawPointerError::PressureOutOfRange)
        );
        let mut sample = pen();
        sample.tilt_x_degrees = Some(91.0);
        assert_eq!(
            normalize_pointer(sample),
            Err(RawPointerError::TiltOutOfRange)
        );
        let mut sample = pen();
        sample.kind = PointerKind::Touch;
        sample.button = Some(MouseButton::Left);
        assert_eq!(
            normalize_pointer(sample),
            Err(RawPointerError::TouchHasButton)
        );
        let mut sample = pen();
        sample.kind = PointerKind::Touch;
        sample.button = None;
        sample.eraser = true;
        assert_eq!(
            normalize_pointer(sample),
            Err(RawPointerError::NonPenHasEraser)
        );
    }

    #[test]
    fn absent_pressure_uses_explicit_phase_fallback() {
        let mut sample = pen();
        sample.pressure = None;
        assert_eq!(normalize_pointer(sample.clone()).unwrap().pressure, 1.0);
        sample.phase = PointerPhase::Cancel;
        assert_eq!(normalize_pointer(sample).unwrap().pressure, 0.0);
    }
}
