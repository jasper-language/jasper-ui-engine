//! E7 animation executor: easing mathematics (cubic Bézier, analytic damped spring) and the
//! per-view transition state that interpolates presented style values on a host clock.
//! E7 动画执行器：缓动数学（三次贝塞尔、解析阻尼弹簧）与按宿主时钟插值呈现样式值的每视图过渡状态。
//!
//! Jasper declares targets and transition rules (`Style::transitions`); the engine never receives
//! per-frame patches. A changed target starts a channel from the value presented at the last
//! clock sample (with its velocity, for springs), so retargeting never jumps. The scene version
//! is not consumed by animation. With no running channel and no clock attached, nothing here runs.
//! Jasper 只声明目标值与过渡规则（`Style::transitions`），引擎从不接收逐帧补丁。目标值变化时从上次时钟采样的
//! 呈现值（弹簧另带速度）起一个通道，因此中途改目标不跳变。动画不消耗场景版本。没有运行中的通道、未接时钟时，
//! 本模块不做任何工作。

use alloc::collections::{BTreeMap, BTreeSet};

use crate::nostd_prelude::*;
use crate::scene::NodeId;
use crate::style::{
    AnimatedProperties, Backdrop, BorderSide, BoxShadow, Color, Easing, Gradient, GradientShape,
    GradientStop, Length, Outline, Style, Transform2D, Transition, MAX_BACKDROP_BLUR,
    MAX_BACKDROP_SATURATION, MAX_EFFECT_EXTENT, MAX_TRANSFORM_ORIGIN, MAX_TRANSFORM_ROTATE, MAX_TRANSFORM_SCALE,
    MAX_TRANSFORM_TRANSLATE, MAX_TRANSITION_MS,
};

/// Longest a spring may run (seconds); a spring still outside tolerance then snaps to target.
/// 弹簧最长运行时间（秒）；届时仍未进入容差的弹簧直接落到目标值。
pub const MAX_SPRING_SECONDS: f64 = MAX_TRANSITION_MS as f64 / 1000.0;

/// CSS `cubic-bezier` progress at `p` in 0..=1 (exactly 0 at 0 and 1 at 1). x1, x2 must be in
/// 0..=1 so x(t) is monotone; t is found by Newton steps with a bisection fallback.
/// CSS `cubic-bezier` 在 `p`（0..=1）处的进度（0 处恰为 0，1 处恰为 1）。x1、x2 须在 0..=1，使 x(t) 单调；
/// 用牛顿迭代求 t，失败时二分。
pub fn cubic_bezier(x1: f64, y1: f64, x2: f64, y2: f64, p: f64) -> f64 {
    if !(p > 0.0) {
        return 0.0;
    }
    if p >= 1.0 {
        return 1.0;
    }
    let bez = |a: f64, b: f64, t: f64| {
        let u = 1.0 - t;
        3.0 * u * u * t * a + 3.0 * u * t * t * b + t * t * t
    };
    let slope = |a: f64, b: f64, t: f64| {
        let u = 1.0 - t;
        3.0 * u * u * a + 6.0 * u * t * (b - a) + 3.0 * t * t * (1.0 - b)
    };
    let mut t = p;
    let mut solved = false;
    for _ in 0..8 {
        let error = bez(x1, x2, t) - p;
        if error.abs() < 1e-9 {
            solved = true;
            break;
        }
        let d = slope(x1, x2, t);
        if d.abs() < 1e-9 {
            break;
        }
        t -= error / d;
        if !(0.0..=1.0).contains(&t) {
            break;
        }
    }
    if !solved {
        let (mut lo, mut hi) = (0.0, 1.0);
        t = p;
        for _ in 0..64 {
            let x = bez(x1, x2, t);
            if (x - p).abs() < 1e-9 {
                break;
            }
            if x < p {
                lo = t;
            } else {
                hi = t;
            }
            t = 0.5 * (lo + hi);
        }
    }
    bez(y1, y2, t)
}

/// Damped harmonic oscillator `m·x'' + c·x' + k·x = 0`, solved in closed form.
/// 阻尼谐振子 `m·x'' + c·x' + k·x = 0` 的闭式解。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    /// Natural angular frequency √(k/m), rad/s.
    pub omega: f64,
    /// Damping ratio c / (2√(k·m)).
    pub zeta: f64,
}

/// Damping ratios this close to 1 use the critically damped form (avoids the cancellation of
/// the under/over-damped forms near ζ = 1).
/// 与 1 相差在此之内的阻尼比按临界阻尼求解（避免在 ζ = 1 附近欠/过阻尼公式的相消误差）。
const CRITICAL_BAND: f64 = 1e-3;

impl Spring {
    /// None unless mass, stiffness and damping are finite and > 0.
    /// 质量、刚度、阻尼须有限且 > 0，否则为 None。
    pub fn new(mass: f64, stiffness: f64, damping: f64) -> Option<Self> {
        if !(mass > 0.0 && stiffness > 0.0 && damping > 0.0)
            || !(mass.is_finite() && stiffness.is_finite() && damping.is_finite())
        {
            return None;
        }
        Some(Self {
            omega: (stiffness / mass).sqrt(),
            zeta: damping / (2.0 * (stiffness * mass).sqrt()),
        })
    }

    /// Displacement from rest and velocity at `t` seconds, from displacement `x0` and velocity
    /// `v0` (units per second) at t = 0.
    /// 从 t = 0 的位移 `x0`、速度 `v0`（单位/秒）出发，`t` 秒时相对平衡位置的位移与速度。
    pub fn state(&self, x0: f64, v0: f64, t: f64) -> (f64, f64) {
        let (w, z) = (self.omega, self.zeta);
        if (z - 1.0).abs() < CRITICAL_BAND {
            let b = v0 + w * x0;
            let e = (-w * t).exp();
            ((x0 + b * t) * e, (b - w * (x0 + b * t)) * e)
        } else if z < 1.0 {
            let wd = w * (1.0 - z * z).sqrt();
            let b = (v0 + z * w * x0) / wd;
            let e = (-z * w * t).exp();
            let (s, c) = (wd * t).sin_cos();
            (
                e * (x0 * c + b * s),
                e * ((b * wd - z * w * x0) * c - (x0 * wd + z * w * b) * s),
            )
        } else {
            let root = (z * z - 1.0).sqrt();
            let (r1, r2) = (-w * (z - root), -w * (z + root));
            let c1 = (v0 - r2 * x0) / (r1 - r2);
            let c2 = x0 - c1;
            let (e1, e2) = ((r1 * t).exp(), (r2 * t).exp());
            (c1 * e1 + c2 * e2, c1 * r1 * e1 + c2 * r2 * e2)
        }
    }

    /// Envelope `A·e^(−λt)` bounding |x(t)| for every t ≥ 0: returns (A, λ).
    /// 对所有 t ≥ 0 界住 |x(t)| 的包络 `A·e^(−λt)`：返回 (A, λ)。
    pub fn envelope(&self, x0: f64, v0: f64) -> (f64, f64) {
        let (w, z) = (self.omega, self.zeta);
        if (z - 1.0).abs() < CRITICAL_BAND {
            // |x0 + b·t|·e^(−wt) ≤ (|x0| + 2|b|/(e·w))·e^(−wt/2), as t·e^(−wt/2) ≤ 2/(e·w).
            let b = v0 + w * x0;
            (
                x0.abs() + 2.0 * b.abs() / (core::f64::consts::E * w),
                w / 2.0,
            )
        } else if z < 1.0 {
            let wd = w * (1.0 - z * z).sqrt();
            let b = (v0 + z * w * x0) / wd;
            ((x0 * x0 + b * b).sqrt(), z * w)
        } else {
            let root = (z * z - 1.0).sqrt();
            let (r1, r2) = (-w * (z - root), -w * (z + root));
            let c1 = (v0 - r2 * x0) / (r1 - r2);
            let c2 = x0 - c1;
            (c1.abs() + c2.abs(), -r1)
        }
    }

    /// Earliest time (seconds) after which the envelope stays within `tolerance`; capped at
    /// `MAX_SPRING_SECONDS`. 0 when already within tolerance.
    /// 包络此后始终不超过 `tolerance` 的最早时刻（秒），上限 `MAX_SPRING_SECONDS`；已在容差内时为 0。
    pub fn settle_time(&self, x0: f64, v0: f64, tolerance: f64) -> f64 {
        let (a, rate) = self.envelope(x0, v0);
        if !(a > tolerance) {
            return 0.0;
        }
        if !(rate > 0.0) || !a.is_finite() {
            return MAX_SPRING_SECONDS;
        }
        ((a / tolerance).ln() / rate).clamp(0.0, MAX_SPRING_SECONDS)
    }
}

/// Timing of one running channel.
#[derive(Clone, Debug, PartialEq)]
enum Timing {
    Bezier {
        duration: f64,
        curve: [f64; 4],
    },
    Spring {
        spring: Spring,
        /// Seconds after the delay at which every component is within tolerance.
        settle: f64,
    },
}

/// Structure an interpolated value must share with both endpoints; mismatches change at once.
#[derive(Clone, Debug, PartialEq)]
enum Shape {
    Plain,
    /// Inset flag of each shadow layer.
    Shadows(Vec<bool>),
    /// Length unit (true: percent).
    Length(bool),
    /// Which per-side borders are present (their colours follow the uniform colour slot).
    Borders([bool; 4]),
    /// Outline presence.
    Outline(bool),
    /// E5 material: presence, stroke presence and the (not interpolated) fallback colour.
    Backdrop(Option<(bool, Option<Color>)>),
    /// E6 gradient: None, or (linear?, explicit radius?, stop count).
    Gradient(Option<(bool, bool, usize)>),
}

/// One animating property of one node.
#[derive(Clone, Debug, PartialEq)]
struct Channel {
    property: AnimatedProperties,
    shape: Shape,
    from: Vec<f64>,
    to: Vec<f64>,
    /// Initial velocity per component (units per second); springs only.
    velocity: Vec<f64>,
    /// Clock time (ms) of the first sample; None until the next `advance`.
    start: Option<f64>,
    delay: f64,
    timing: Timing,
}

impl Channel {
    /// Value and velocity (per second) `elapsed` ms after the start.
    fn sample(&self, elapsed: f64) -> (Vec<f64>, Vec<f64>) {
        let t = elapsed - self.delay;
        if !(t > 0.0) {
            return (self.from.clone(), vec![0.0; self.from.len()]);
        }
        match &self.timing {
            Timing::Bezier { duration, curve } => {
                if t >= *duration {
                    return (self.to.clone(), vec![0.0; self.to.len()]);
                }
                let [x1, y1, x2, y2] = *curve;
                let p = t / duration;
                let e = cubic_bezier(x1, y1, x2, y2, p);
                let dp = 1e-3 / duration;
                let e2 = cubic_bezier(x1, y1, x2, y2, (p + dp).min(1.0));
                let rate = (e2 - e) / 1e-3 * 1000.0;
                let mut value = Vec::with_capacity(self.from.len());
                let mut velocity = Vec::with_capacity(self.from.len());
                for (a, b) in self.from.iter().zip(&self.to) {
                    value.push(a + (b - a) * e);
                    velocity.push((b - a) * rate);
                }
                (value, velocity)
            }
            Timing::Spring { spring, settle } => {
                if t / 1000.0 >= *settle {
                    return (self.to.clone(), vec![0.0; self.to.len()]);
                }
                let mut value = Vec::with_capacity(self.from.len());
                let mut velocity = Vec::with_capacity(self.from.len());
                for ((a, b), v0) in self.from.iter().zip(&self.to).zip(&self.velocity) {
                    let (x, v) = spring.state(a - b, *v0, t / 1000.0);
                    value.push(b + x);
                    velocity.push(v);
                }
                (value, velocity)
            }
        }
    }

    fn finished(&self, elapsed: f64) -> bool {
        let t = elapsed - self.delay;
        match &self.timing {
            Timing::Bezier { duration, .. } => t >= *duration,
            Timing::Spring { settle, .. } => t / 1000.0 >= *settle,
        }
    }

    fn elapsed(&self, clock: Option<f64>) -> f64 {
        match (self.start, clock) {
            (Some(start), Some(now)) => (now - start).max(0.0),
            _ => 0.0,
        }
    }
}

fn color_components(c: Color, out: &mut Vec<f64>) {
    // Premultiplied sRGB, as CSS interpolates colours.
    let a = f64::from(c.a);
    out.push(f64::from(c.r) * a / 255.0);
    out.push(f64::from(c.g) * a / 255.0);
    out.push(f64::from(c.b) * a / 255.0);
    out.push(a);
}

fn color_from(v: &[f64]) -> Color {
    let a = v[3].clamp(0.0, 255.0);
    let channel = |p: f64| {
        if a < 0.5 {
            0
        } else {
            (p * 255.0 / a).round().clamp(0.0, 255.0) as u8
        }
    };
    Color::new(channel(v[0]), channel(v[1]), channel(v[2]), a.round() as u8)
}

const COLOR_TOLERANCE: f64 = 0.25;
const LENGTH_TOLERANCE: f64 = 0.01;

/// Single-style extraction: (shape, components, per-component tolerance).
fn snapshot(property: AnimatedProperties, style: &Style) -> (Shape, Vec<f64>, Vec<f64>) {
    let mut v = Vec::new();
    let mut tol = Vec::new();
    let shape = match property {
        AnimatedProperties::BACKGROUND => {
            color_components(style.background, &mut v);
            Shape::Plain
        }
        AnimatedProperties::COLOR => {
            color_components(style.color, &mut v);
            Shape::Plain
        }
        AnimatedProperties::BORDER_COLOR => {
            color_components(style.border_color, &mut v);
            let sides = [
                style.border_top,
                style.border_right,
                style.border_bottom,
                style.border_left,
            ];
            for side in sides.iter().flatten() {
                color_components(side.color, &mut v);
            }
            Shape::Borders(sides.map(|s| s.is_some()))
        }
        AnimatedProperties::OPACITY => {
            v.push(f64::from(style.opacity));
            tol.push(0.5 / 255.0);
            Shape::Plain
        }
        AnimatedProperties::CORNER_RADIUS => {
            let r = style.corner_radius;
            v.extend([r.top_left, r.top_right, r.bottom_right, r.bottom_left].map(f64::from));
            Shape::Plain
        }
        AnimatedProperties::SHADOW => {
            for s in &style.box_shadows {
                v.extend([s.x, s.y, s.blur, s.spread].map(f64::from));
                tol.extend([LENGTH_TOLERANCE; 4]);
                color_components(s.color, &mut v);
                tol.extend([COLOR_TOLERANCE; 4]);
            }
            Shape::Shadows(style.box_shadows.iter().map(|s| s.inset).collect())
        }
        AnimatedProperties::OUTLINE => match style.outline {
            Some(o) => {
                v.extend([o.width, o.offset].map(f64::from));
                tol.extend([LENGTH_TOLERANCE; 2]);
                color_components(o.color, &mut v);
                Shape::Outline(true)
            }
            None => Shape::Outline(false),
        },
        AnimatedProperties::WIDTH | AnimatedProperties::HEIGHT => {
            let length = if property == AnimatedProperties::WIDTH {
                style.width
            } else {
                style.height
            };
            match length {
                Length::Px(x) => {
                    v.push(f64::from(x));
                    Shape::Length(false)
                }
                Length::Percent(x) => {
                    v.push(f64::from(x));
                    Shape::Length(true)
                }
                // Auto is not interpolable: never equal to a numeric shape.
                Length::Auto => Shape::Plain,
            }
        }
        AnimatedProperties::TRANSFORM => {
            let t = style.transform;
            v.extend(
                [
                    t.translate_x,
                    t.translate_y,
                    t.rotate,
                    t.scale_x,
                    t.scale_y,
                    t.origin_x,
                    t.origin_y,
                ]
                .map(f64::from),
            );
            tol.extend([
                LENGTH_TOLERANCE,
                LENGTH_TOLERANCE,
                0.01,
                1e-4,
                1e-4,
                1e-4,
                1e-4,
            ]);
            Shape::Plain
        }
        AnimatedProperties::BACKDROP => match style.backdrop {
            Some(b) => {
                v.extend([b.blur, b.saturation].map(f64::from));
                tol.extend([LENGTH_TOLERANCE, 1e-4]);
                color_components(b.tint, &mut v);
                tol.extend([COLOR_TOLERANCE; 4]);
                if let Some(stroke) = b.stroke {
                    v.push(f64::from(stroke.width));
                    tol.push(LENGTH_TOLERANCE);
                    color_components(stroke.color, &mut v);
                    tol.extend([COLOR_TOLERANCE; 4]);
                }
                Shape::Backdrop(Some((b.stroke.is_some(), b.fallback)))
            }
            None => Shape::Backdrop(None),
        },
        AnimatedProperties::GRADIENT => match &style.background_gradient {
            Some(g) => {
                let (linear, explicit) = match g.shape {
                    GradientShape::Linear { angle } => {
                        v.push(f64::from(angle));
                        tol.push(0.01);
                        (true, false)
                    }
                    GradientShape::Radial {
                        center_x,
                        center_y,
                        radius,
                    } => {
                        v.extend([center_x, center_y].map(f64::from));
                        tol.extend([1e-4, 1e-4]);
                        if let Some(r) = radius {
                            v.push(f64::from(r));
                            tol.push(LENGTH_TOLERANCE);
                        }
                        (false, radius.is_some())
                    }
                };
                for stop in &g.stops {
                    v.push(f64::from(stop.position));
                    tol.push(1e-4);
                    color_components(stop.color, &mut v);
                    tol.extend([COLOR_TOLERANCE; 4]);
                }
                Shape::Gradient(Some((linear, explicit, g.stops.len())))
            }
            None => Shape::Gradient(None),
        },
        _ => Shape::Plain,
    };
    // Defaults for the remaining components: colours vs lengths.
    while tol.len() < v.len() {
        let index = tol.len();
        tol.push(match property {
            AnimatedProperties::CORNER_RADIUS
            | AnimatedProperties::WIDTH
            | AnimatedProperties::HEIGHT => LENGTH_TOLERANCE,
            AnimatedProperties::OUTLINE if index < 2 => LENGTH_TOLERANCE,
            _ => COLOR_TOLERANCE,
        });
    }
    (shape, v, tol)
}

/// Interpolable endpoints of `property` between two styles: shadows are padded with
/// transparent layers of matching inset (CSS); any other structural mismatch is None.
/// 两个样式间 `property` 的可插值端点：阴影层数不同时按 CSS 以透明同类层补齐；其他结构不一致为 None。
fn endpoints(
    property: AnimatedProperties,
    from: &Style,
    to: &Style,
) -> Option<(Shape, Vec<f64>, Vec<f64>, Vec<f64>)> {
    if property == AnimatedProperties::SHADOW {
        let n = from.box_shadows.len().max(to.box_shadows.len());
        let pad = |style: &Style, other: &Style| -> Style {
            let mut style = style.clone();
            for i in style.box_shadows.len()..n {
                style.box_shadows.push(BoxShadow {
                    inset: other.box_shadows[i].inset,
                    ..BoxShadow::new(0.0, 0.0, 0.0, 0.0, Color::TRANSPARENT)
                });
            }
            style
        };
        let (a, b) = (pad(from, to), pad(to, from));
        let (sa, va, ta) = snapshot(property, &a);
        let (sb, vb, _) = snapshot(property, &b);
        return (sa == sb).then_some((sa, va, vb, ta));
    }
    let (sa, va, ta) = snapshot(property, from);
    let (sb, vb, _) = snapshot(property, to);
    if sa != sb || va.len() != vb.len() || va.is_empty() {
        return None;
    }
    Some((sa, va, vb, ta))
}

fn clamp_length(v: f64) -> f32 {
    (v as f32).clamp(0.0, MAX_EFFECT_EXTENT)
}

fn clamp_signed(v: f64, bound: f32) -> f32 {
    (v as f32).clamp(-bound, bound)
}

/// Writes interpolated components into `out`, clamped into the validated ranges (a spring may
/// overshoot past an endpoint).
/// 把插值分量写入 `out`，并钳制到合法范围（弹簧可能越过端点）。
fn write(property: AnimatedProperties, shape: &Shape, v: &[f64], out: &mut Style) {
    match (property, shape) {
        (AnimatedProperties::BACKGROUND, _) => out.background = color_from(v),
        (AnimatedProperties::COLOR, _) => out.color = color_from(v),
        (AnimatedProperties::BORDER_COLOR, _) => {
            out.border_color = color_from(&v[0..4]);
            let mut index = 4;
            for side in [
                &mut out.border_top,
                &mut out.border_right,
                &mut out.border_bottom,
                &mut out.border_left,
            ] {
                if let Some(BorderSide { color, .. }) = side {
                    *color = color_from(&v[index..index + 4]);
                    index += 4;
                }
            }
        }
        (AnimatedProperties::OPACITY, _) => out.opacity = (v[0] as f32).clamp(0.0, 1.0),
        (AnimatedProperties::CORNER_RADIUS, _) => {
            out.corner_radius.top_left = clamp_length(v[0]);
            out.corner_radius.top_right = clamp_length(v[1]);
            out.corner_radius.bottom_right = clamp_length(v[2]);
            out.corner_radius.bottom_left = clamp_length(v[3]);
        }
        (AnimatedProperties::SHADOW, Shape::Shadows(insets)) => {
            out.box_shadows = insets
                .iter()
                .enumerate()
                .map(|(i, inset)| {
                    let c = &v[i * 8..i * 8 + 8];
                    BoxShadow {
                        x: clamp_signed(c[0], MAX_EFFECT_EXTENT),
                        y: clamp_signed(c[1], MAX_EFFECT_EXTENT),
                        blur: clamp_length(c[2]),
                        spread: clamp_signed(c[3], MAX_EFFECT_EXTENT),
                        color: color_from(&c[4..8]),
                        inset: *inset,
                    }
                })
                .collect();
        }
        (AnimatedProperties::OUTLINE, Shape::Outline(true)) => {
            out.outline = Some(Outline {
                width: clamp_length(v[0]),
                offset: clamp_signed(v[1], MAX_EFFECT_EXTENT),
                color: color_from(&v[2..6]),
            });
        }
        (AnimatedProperties::WIDTH, Shape::Length(percent))
        | (AnimatedProperties::HEIGHT, Shape::Length(percent)) => {
            let x = (v[0] as f32).max(0.0);
            let length = if *percent {
                Length::Percent(x)
            } else {
                Length::Px(x)
            };
            if property == AnimatedProperties::WIDTH {
                out.width = length;
            } else {
                out.height = length;
            }
        }
        (AnimatedProperties::TRANSFORM, _) => {
            out.transform = Transform2D {
                translate_x: clamp_signed(v[0], MAX_TRANSFORM_TRANSLATE),
                translate_y: clamp_signed(v[1], MAX_TRANSFORM_TRANSLATE),
                rotate: clamp_signed(v[2], MAX_TRANSFORM_ROTATE),
                scale_x: clamp_signed(v[3], MAX_TRANSFORM_SCALE),
                scale_y: clamp_signed(v[4], MAX_TRANSFORM_SCALE),
                origin_x: clamp_signed(v[5], MAX_TRANSFORM_ORIGIN),
                origin_y: clamp_signed(v[6], MAX_TRANSFORM_ORIGIN),
            };
        }
        (AnimatedProperties::BACKDROP, Shape::Backdrop(Some((stroke, fallback)))) => {
            out.backdrop = Some(Backdrop {
                blur: (v[0] as f32).clamp(0.0, MAX_BACKDROP_BLUR),
                saturation: (v[1] as f32).clamp(0.0, MAX_BACKDROP_SATURATION),
                tint: color_from(&v[2..6]),
                stroke: stroke.then(|| BorderSide {
                    // A stroke never animates to zero width (it would be invalid); 1e-3 px is invisible.
                    width: clamp_length(v[6]).max(1e-3),
                    color: color_from(&v[7..11]),
                }),
                fallback: *fallback,
            });
        }
        (AnimatedProperties::GRADIENT, Shape::Gradient(Some((linear, explicit, count)))) => {
            let (shape, mut index) = if *linear {
                (
                    GradientShape::Linear {
                        angle: clamp_signed(v[0], MAX_TRANSFORM_ROTATE),
                    },
                    1,
                )
            } else {
                let radius = explicit.then(|| clamp_length(v[2]).max(1e-3));
                (
                    GradientShape::Radial {
                        center_x: clamp_signed(v[0], MAX_TRANSFORM_ORIGIN),
                        center_y: clamp_signed(v[1], MAX_TRANSFORM_ORIGIN),
                        radius,
                    },
                    if *explicit { 3 } else { 2 },
                )
            };
            let mut stops = Vec::with_capacity(*count);
            let mut floor = 0.0f32;
            for _ in 0..*count {
                // Positions stay in 0..=1 and non-decreasing even when a spring overshoots.
                let position = (v[index] as f32).clamp(floor, 1.0);
                floor = position;
                stops.push(GradientStop::new(position, color_from(&v[index + 1..index + 5])));
                index += 5;
            }
            out.background_gradient = Some(Gradient { shape, stops });
        }
        _ => {}
    }
}

/// Timing a rule gives `property`, or None for an immediate change. Under reduce-motion,
/// movement properties are always immediate and the rest use the rule's reduced timing (or are
/// immediate without one).
/// 规则赋予 `property` 的时间曲线；None 表示立即变化。减少动态下移动类属性总是立即，其余属性用规则的减少动态
/// 替代曲线（没有则立即）。
fn effective_timing(
    rule: &Transition,
    property: AnimatedProperties,
    reduce_motion: bool,
) -> Option<(f64, Easing, f64)> {
    let (duration, easing) = if reduce_motion {
        if AnimatedProperties::MOVEMENT.contains(property) {
            return None;
        }
        let reduced = rule.reduced?;
        (reduced.duration_ms, reduced.easing)
    } else {
        (rule.duration_ms, rule.easing)
    };
    let delay = f64::from(rule.delay_ms);
    if matches!(easing, Easing::CubicBezier { .. }) && duration <= 0.0 && delay <= 0.0 {
        return None;
    }
    Some((delay, easing, f64::from(duration)))
}

/// Result of advancing the clock.
/// 推进时钟的结果。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnimationTick {
    /// Some presented value changed (a repaint is due).
    /// 有呈现值变化（需要重绘）。
    pub changed: bool,
    /// Channels are still running: call again on the next refresh tick.
    /// 仍有通道在运行：下一个刷新节拍需再次调用。
    pub running: bool,
}

/// Transition state of one view.
/// 单个视图的过渡状态。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MotionState {
    channels: BTreeMap<NodeId, Vec<Channel>>,
    clock: Option<f64>,
    reduce_motion: bool,
}

impl MotionState {
    /// Last host clock sample in ms; None until the host first advances the clock (until then
    /// every transition is immediate, so a host that never drives the clock sees no animation).
    /// 宿主上次给出的时钟（毫秒）；宿主首次推进时钟前为 None（此前所有过渡立即生效，从不驱动时钟的宿主不会看到动画）。
    pub fn clock(&self) -> Option<f64> {
        self.clock
    }

    pub fn reduce_motion(&self) -> bool {
        self.reduce_motion
    }

    /// Some channel is running.
    pub fn running(&self) -> bool {
        !self.channels.is_empty()
    }

    /// Nodes with running channels.
    pub fn animated_nodes(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.channels.keys().copied()
    }

    /// Presented style of `id` at the last clock sample: `target` with every running channel's
    /// current value. None when nothing of `id` is animating (the target is presented as is).
    /// `id` 在上次时钟采样时的呈现样式：`target` 叠加各运行中通道的当前值。`id` 无动画时为 None（直接呈现目标值）。
    pub fn presented(&self, id: NodeId, target: &Style) -> Option<Style> {
        let channels = self.channels.get(&id)?;
        let mut out = target.clone();
        for channel in channels {
            let (value, _) = channel.sample(channel.elapsed(self.clock));
            write(channel.property, &channel.shape, &value, &mut out);
        }
        Some(out)
    }

    /// Re-targets after a committed frame. `old`/`new` give each node's target style before and
    /// after the frame, `updated` the nodes the frame restyled, `retired` the nodes the frame
    /// removed (a reinserted id starts fresh). Returns the nodes whose presented style changed
    /// relative to plain target projection (they need their presented values written).
    /// 已提交帧之后重新定目标。`old`/`new` 给出帧前后各节点的目标样式，`updated` 为本帧改了样式的节点，`retired` 为
    /// 本帧删除的节点（同 id 重新插入的视为新节点）。新通道在下一次 `advance` 时开始，起点为上次时钟采样的呈现值。
    pub fn retarget<'a>(
        &mut self,
        old: impl Fn(NodeId) -> Option<&'a Style>,
        new: impl Fn(NodeId) -> Option<&'a Style>,
        updated: &BTreeSet<NodeId>,
        retired: impl Fn(NodeId) -> bool,
    ) {
        let mut ids: BTreeSet<NodeId> = self.channels.keys().copied().collect();
        ids.extend(updated.iter().copied());
        for id in ids {
            let (Some(before), Some(after)) = (old(id), new(id)) else {
                self.channels.remove(&id);
                continue;
            };
            if retired(id) {
                self.channels.remove(&id);
                continue;
            }
            if before == after && !self.channels.contains_key(&id) {
                continue;
            }
            let current = self.presented(id, before).unwrap_or_else(|| before.clone());
            let mut running = self.channels.remove(&id).unwrap_or_default();
            let mut next = Vec::new();
            for (_, property) in AnimatedProperties::NAMED {
                let existing = running
                    .iter()
                    .position(|c| c.property == property)
                    .map(|i| running.swap_remove(i));
                let rule = after.transition_for(property);
                let changed = snapshot(property, before) != snapshot(property, after);
                if !changed {
                    // Keep a running channel only while its property is still covered.
                    if let (Some(channel), Some(_)) = (existing, rule) {
                        next.push(channel);
                    }
                    continue;
                }
                let Some((delay, easing, duration)) =
                    rule.and_then(|r| effective_timing(r, property, self.reduce_motion))
                else {
                    continue;
                };
                if self.clock.is_none() {
                    continue;
                }
                let Some((shape, from, to, tolerance)) = endpoints(property, &current, after)
                else {
                    continue;
                };
                if from == to {
                    continue;
                }
                let velocity = match &existing {
                    Some(channel) if channel.from.len() == from.len() => {
                        channel.sample(channel.elapsed(self.clock)).1
                    }
                    _ => vec![0.0; from.len()],
                };
                let timing = match easing {
                    Easing::CubicBezier { x1, y1, x2, y2 } => Timing::Bezier {
                        duration,
                        curve: [x1, y1, x2, y2].map(f64::from),
                    },
                    Easing::Spring {
                        mass,
                        stiffness,
                        damping,
                    } => {
                        let Some(spring) =
                            Spring::new(f64::from(mass), f64::from(stiffness), f64::from(damping))
                        else {
                            continue;
                        };
                        let settle = from
                            .iter()
                            .zip(&to)
                            .zip(&velocity)
                            .zip(&tolerance)
                            .map(|(((a, b), v), tol)| spring.settle_time(a - b, *v, *tol))
                            .fold(0.0, f64::max);
                        Timing::Spring { spring, settle }
                    }
                };
                next.push(Channel {
                    property,
                    shape,
                    from,
                    to,
                    velocity,
                    start: None,
                    delay,
                    timing,
                });
            }
            if !next.is_empty() {
                self.channels.insert(id, next);
            }
        }
    }

    /// Advances to host time `now_ms` (monotone: an earlier time counts as the last one).
    /// Channels started since the last call begin at `now_ms`; finished ones are dropped.
    /// Returns the nodes whose presented style must be rewritten (every node animating before
    /// the call), or None for a non-finite time.
    /// 推进到宿主时间 `now_ms`（单调：更早的时间按上次处理）。上次调用后新起的通道从 `now_ms` 开始；结束的通道移除。
    /// 返回需要重写呈现样式的节点（调用前处于动画中的全部节点）；时间非有限值时为 None。
    pub fn advance(&mut self, now_ms: f64) -> Option<Vec<NodeId>> {
        if !now_ms.is_finite() {
            return None;
        }
        let now = self.clock.map_or(now_ms, |last| now_ms.max(last));
        self.clock = Some(now);
        let touched: Vec<NodeId> = self.channels.keys().copied().collect();
        self.channels.retain(|_, channels| {
            for channel in channels.iter_mut() {
                channel.start.get_or_insert(now);
            }
            channels.retain(|c| !c.finished(c.elapsed(Some(now))));
            !channels.is_empty()
        });
        Some(touched)
    }

    /// Host reduce-motion setting. Turning it on finishes every running channel at once and
    /// returns the nodes that need their target written.
    /// 宿主「减少动态」设置。开启时所有运行中的通道立即结束，返回需写回目标值的节点。
    pub fn set_reduce_motion(&mut self, on: bool) -> Vec<NodeId> {
        self.reduce_motion = on;
        if !on {
            return Vec::new();
        }
        let touched = self.channels.keys().copied().collect();
        self.channels.clear();
        touched
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{CornerRadii, Transition};

    #[test]
    fn bezier_endpoints_are_exact_and_standard_curves_monotone() {
        for (x1, y1, x2, y2) in [
            (0.0, 0.0, 1.0, 1.0),
            (0.25, 0.1, 0.25, 1.0),
            (0.42, 0.0, 1.0, 1.0),
            (0.0, 0.0, 0.58, 1.0),
            (0.42, 0.0, 0.58, 1.0),
            (0.2, 0.0, 0.0, 1.0),
            (1.0, 0.0, 0.0, 1.0),
            (0.0, 1.0, 1.0, 0.0),
        ] {
            assert_eq!(cubic_bezier(x1, y1, x2, y2, 0.0), 0.0);
            assert_eq!(cubic_bezier(x1, y1, x2, y2, 1.0), 1.0);
            assert_eq!(cubic_bezier(x1, y1, x2, y2, -0.5), 0.0);
            assert_eq!(cubic_bezier(x1, y1, x2, y2, 1.5), 1.0);
            assert_eq!(cubic_bezier(x1, y1, x2, y2, f64::NAN), 0.0);
            let mut previous = 0.0;
            for i in 1..=1000 {
                let y = cubic_bezier(x1, y1, x2, y2, i as f64 / 1000.0);
                assert!(y.is_finite());
                assert!(
                    y >= previous - 1e-9,
                    "({x1},{y1},{x2},{y2}) at {i}: {y} < {previous}"
                );
                previous = y;
            }
        }
        // Linear is the identity; symmetric ease-in-out passes through the midpoint.
        for i in 0..=20 {
            let p = i as f64 / 20.0;
            assert!((cubic_bezier(0.0, 0.0, 1.0, 1.0, p) - p).abs() < 1e-6);
        }
        assert!((cubic_bezier(0.42, 0.0, 0.58, 1.0, 0.5) - 0.5).abs() < 1e-6);
        // Overshooting y is allowed (back easing) and stays finite.
        let back = cubic_bezier(0.34, 1.56, 0.64, 1.0, 0.6);
        assert!(back > 1.0 && back.is_finite());
    }

    #[test]
    fn spring_converges_without_nan_in_every_regime() {
        // Under-, critically and over-damped; Apple-style defaults included.
        for (m, k, c) in [
            (1.0, 170.0, 26.0),
            (1.0, 100.0, 10.0),
            (1.0, 100.0, 20.0),
            (1.0, 100.0, 20.0005),
            (1.0, 100.0, 60.0),
            (0.01, 100_000.0, 0.01),
            (1000.0, 0.01, 100_000.0),
            (1.0, 300.0, 1.0),
        ] {
            let spring = Spring::new(m, k, c).unwrap();
            for (x0, v0) in [(1.0, 0.0), (-40.0, 300.0), (0.0, 5.0), (255.0, -1e4)] {
                let tol = 0.01;
                let settle = spring.settle_time(x0, v0, tol);
                assert!(settle.is_finite() && settle >= 0.0 && settle <= MAX_SPRING_SECONDS);
                let (a, rate) = spring.envelope(x0, v0);
                for i in 0..=400 {
                    let t = settle * i as f64 / 400.0;
                    let (x, v) = spring.state(x0, v0, t);
                    assert!(x.is_finite() && v.is_finite(), "({m},{k},{c}) t={t}");
                    assert!(
                        x.abs() <= a * (-rate * t).exp() + 1e-6,
                        "envelope ({m},{k},{c}) t={t}"
                    );
                }
                if settle < MAX_SPRING_SECONDS {
                    let (x, _) = spring.state(x0, v0, settle);
                    assert!(x.abs() <= tol + 1e-9, "({m},{k},{c}) settles: {x}");
                }
            }
            // Initial conditions are reproduced.
            let (x, v) = spring.state(3.0, -2.0, 0.0);
            assert!((x - 3.0).abs() < 1e-9 && (v + 2.0).abs() < 1e-6);
        }
        assert!(Spring::new(0.0, 1.0, 1.0).is_none());
        assert!(Spring::new(1.0, -1.0, 1.0).is_none());
        assert!(Spring::new(1.0, 1.0, f64::NAN).is_none());
        // Velocity is the derivative of position.
        let spring = Spring::new(1.0, 170.0, 26.0).unwrap();
        let (x1, v) = spring.state(10.0, 0.0, 0.1);
        let (x2, _) = spring.state(10.0, 0.0, 0.1 + 1e-6);
        assert!(((x2 - x1) / 1e-6 - v).abs() < 1e-3);
    }

    fn style() -> Style {
        Style::default().opacity(1.0).transition(Transition::new(
            AnimatedProperties::OPACITY,
            200.0,
            Easing::LINEAR,
        ))
    }

    fn step(state: &mut MotionState, before: &Style, after: &Style) {
        let id = NodeId(1);
        state.retarget(
            |_| Some(before),
            |_| Some(after),
            &BTreeSet::from([id]),
            |_| false,
        );
    }

    fn opacity(state: &MotionState, target: &Style) -> f32 {
        state
            .presented(NodeId(1), target)
            .map_or(target.opacity, |s| s.opacity)
    }

    #[test]
    fn no_clock_or_no_rule_is_immediate_and_free() {
        let mut state = MotionState::default();
        let (a, b) = (style(), style().opacity(0.0));
        step(&mut state, &a, &b);
        assert!(!state.running(), "no clock attached: immediate");
        state.advance(0.0).unwrap();
        let plain = Style::default();
        step(&mut state, &plain, &plain.clone().opacity(0.0));
        assert!(!state.running(), "no rule: immediate");
        assert!(state.advance(f64::NAN).is_none());
    }

    #[test]
    fn linear_opacity_and_retarget_from_current_value() {
        let mut state = MotionState::default();
        state.advance(1000.0).unwrap();
        let (a, b) = (style(), style().opacity(0.0));
        step(&mut state, &a, &b);
        assert!(state.running());
        // Not started yet: presents the old value.
        assert_eq!(opacity(&state, &b), 1.0);
        state.advance(1000.0).unwrap();
        assert_eq!(opacity(&state, &b), 1.0);
        state.advance(1100.0).unwrap();
        assert!((opacity(&state, &b) - 0.5).abs() < 1e-3);
        // Retarget back to 1 at 0.5: the new channel starts exactly at the presented value.
        let c = style();
        let before = opacity(&state, &b);
        step(&mut state, &b, &c);
        assert_eq!(opacity(&state, &c), before);
        state.advance(1100.0).unwrap();
        assert!((opacity(&state, &c) - before).abs() < 1e-6, "continuous");
        state.advance(1200.0).unwrap();
        assert!((opacity(&state, &c) - 0.75).abs() < 1e-3);
        let tick = state.advance(1300.0).unwrap();
        assert_eq!(tick, vec![NodeId(1)]);
        assert!(!state.running());
        assert_eq!(state.presented(NodeId(1), &c), None);
        // Time never goes backwards.
        state.advance(10.0).unwrap();
        assert_eq!(state.clock(), Some(1300.0));
    }

    #[test]
    fn spring_retarget_keeps_value_and_velocity_continuous() {
        let rule = Transition::spring(AnimatedProperties::TRANSFORM, 1.0, 170.0, 26.0);
        let a = Style::default().transition(rule);
        let b = a.clone().transform(Transform2D::translate(100.0, 0.0));
        let c = a.clone().transform(Transform2D::translate(-50.0, 0.0));
        let mut state = MotionState::default();
        state.advance(0.0).unwrap();
        step(&mut state, &a, &b);
        state.advance(0.0).unwrap();
        let tx = |state: &MotionState, target: &Style| {
            state
                .presented(NodeId(1), target)
                .unwrap()
                .transform
                .translate_x
        };
        let mut previous = 0.0f32;
        for i in 1..=6 {
            state.advance(i as f64 * 16.0).unwrap();
            let x = tx(&state, &b);
            assert!(x > previous && x.is_finite());
            previous = x;
        }
        let channel = &state.channels[&NodeId(1)][0];
        let (_, velocity_before) = channel.sample(channel.elapsed(state.clock()));
        step(&mut state, &b, &c);
        assert_eq!(tx(&state, &c), previous, "position continuous");
        let channel = &state.channels[&NodeId(1)][0];
        assert!(
            (channel.velocity[0] - velocity_before[0]).abs() < 1e-6,
            "velocity carried"
        );
        assert!(channel.velocity[0] > 0.0);
        // Momentum keeps it moving right for a moment before turning towards -50.
        state.advance(96.0).unwrap();
        state.advance(100.0).unwrap();
        assert!(tx(&state, &c) > previous);
        let mut t = 100.0;
        while state.running() {
            t += 16.0;
            state.advance(t).unwrap();
            if let Some(s) = state.presented(NodeId(1), &c) {
                assert!(s.transform.translate_x.is_finite());
            }
            assert!(t < 100.0 + MAX_SPRING_SECONDS * 1000.0 + 32.0);
        }
        assert!(t < 3000.0, "Apple-like spring settles within seconds: {t}");
    }

    #[test]
    fn reduce_motion_rules() {
        let rule = Transition::new(
            AnimatedProperties::OPACITY.union(AnimatedProperties::TRANSFORM),
            300.0,
            Easing::EASE,
        );
        let a = Style::default().transition(rule.reduced(150.0, Easing::LINEAR));
        let b = a.clone().opacity(0.0).transform(Transform2D::scale(0.5));
        let mut state = MotionState::default();
        state.advance(0.0).unwrap();
        assert!(state.set_reduce_motion(true).is_empty());
        step(&mut state, &a, &b);
        // Opacity cross-fades with the declared alternative; transform is immediate.
        let channels = &state.channels[&NodeId(1)];
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].property, AnimatedProperties::OPACITY);
        state.advance(0.0).unwrap();
        state.advance(75.0).unwrap();
        let s = state.presented(NodeId(1), &b).unwrap();
        assert!((s.opacity - 0.5).abs() < 1e-3);
        assert_eq!(s.transform, Transform2D::scale(0.5));
        // Without an alternative everything is immediate.
        let plain = Style::default().transition(rule);
        let mut state = MotionState::default();
        state.advance(0.0).unwrap();
        state.set_reduce_motion(true);
        step(&mut state, &plain, &plain.clone().opacity(0.0));
        assert!(!state.running());
        // Turning reduce-motion on finishes running channels.
        let mut state = MotionState::default();
        state.advance(0.0).unwrap();
        step(&mut state, &plain, &plain.clone().opacity(0.0));
        assert!(state.running());
        assert_eq!(state.set_reduce_motion(true), vec![NodeId(1)]);
        assert!(!state.running());
    }

    #[test]
    fn interpolation_shapes() {
        let all = Transition::new(AnimatedProperties::ALL, 100.0, Easing::LINEAR);
        let a = Style::default()
            .transition(all)
            .background(Color::TRANSPARENT)
            .corner_radius(0.0)
            .size(Length::Px(10.0), Length::Auto);
        let b = a
            .clone()
            .background(Color::rgb(200, 0, 0))
            .corner_radii(CornerRadii::new(8.0, 8.0, 0.0, 0.0))
            .shadow(BoxShadow::new(0.0, 4.0, 8.0, 0.0, Color::new(0, 0, 0, 100)))
            .size(Length::Px(30.0), Length::Px(50.0));
        let mut state = MotionState::default();
        state.advance(0.0).unwrap();
        step(&mut state, &a, &b);
        state.advance(0.0).unwrap();
        state.advance(50.0).unwrap();
        let s = state.presented(NodeId(1), &b).unwrap();
        // Premultiplied: from transparent the hue stays red, alpha halves.
        assert_eq!(s.background, Color::new(200, 0, 0, 128));
        assert_eq!(s.corner_radius, CornerRadii::new(4.0, 4.0, 0.0, 0.0));
        assert_eq!(s.box_shadows.len(), 1);
        assert!((s.box_shadows[0].y - 2.0).abs() < 1e-4);
        assert_eq!(s.box_shadows[0].color.a, 50);
        assert_eq!(s.width, Length::Px(20.0));
        // Auto → Px is not interpolable: the target applies at once.
        assert_eq!(s.height, Length::Px(50.0));
        // Padded shadow layer keeps its inset kind; mismatched inset changes at once.
        let inset = |i: bool| {
            Style::default().transition(all).shadow(BoxShadow {
                inset: i,
                ..BoxShadow::new(0.0, 0.0, 4.0, 0.0, Color::BLACK)
            })
        };
        assert!(endpoints(AnimatedProperties::SHADOW, &inset(false), &inset(true)).is_none());
        assert!(endpoints(AnimatedProperties::SHADOW, &Style::default(), &inset(true)).is_some());
    }

    #[test]
    fn material_and_gradient_interpolate_only_between_like_shapes() {
        use crate::style::{Backdrop, Gradient, GradientStop};
        let all = Transition::new(AnimatedProperties::ALL, 100.0, Easing::LINEAR);
        let stops = |c: Color, p: f32| {
            vec![
                GradientStop::new(0.0, Color::WHITE),
                GradientStop::new(p, c),
                GradientStop::new(1.0, Color::BLACK),
            ]
        };
        let a = Style::default()
            .transition(all)
            .background_gradient(Some(Gradient::linear(0.0, stops(Color::rgb(0, 0, 0), 0.2))))
            .backdrop(Some(
                Backdrop::new(10.0, Color::new(255, 255, 255, 100)).fallback(Color::WHITE),
            ));
        let b = Style::default()
            .transition(all)
            .background_gradient(Some(Gradient::linear(
                90.0,
                stops(Color::rgb(200, 100, 0), 0.6),
            )))
            .backdrop(Some(
                Backdrop::new(30.0, Color::new(255, 255, 255, 200))
                    .saturate(2.0)
                    .fallback(Color::WHITE),
            ));
        let mut state = MotionState::default();
        state.advance(0.0).unwrap();
        step(&mut state, &a, &b);
        state.advance(0.0).unwrap();
        state.advance(50.0).unwrap();
        let s = state.presented(NodeId(1), &b).unwrap();
        let g = s.background_gradient.clone().unwrap();
        assert_eq!(g.shape, crate::style::GradientShape::Linear { angle: 45.0 });
        assert!((g.stops[1].position - 0.4).abs() < 1e-5);
        assert_eq!(g.stops[1].color, Color::rgb(100, 50, 0));
        let m = s.backdrop.unwrap();
        assert_eq!((m.blur, m.saturation), (20.0, 1.5));
        assert_eq!(m.tint.a, 150);
        assert!(s.extended_style_valid());
        // Different kind, stop count, stroke presence or fallback: no channel (immediate).
        let radial = b.clone().background_gradient(Some(Gradient::radial(
            0.5,
            0.5,
            stops(Color::BLACK, 0.5),
        )));
        assert!(endpoints(AnimatedProperties::GRADIENT, &a, &radial).is_none());
        let two = b.clone().background_gradient(Some(Gradient::linear(
            0.0,
            vec![
                GradientStop::new(0.0, Color::WHITE),
                GradientStop::new(1.0, Color::BLACK),
            ],
        )));
        assert!(endpoints(AnimatedProperties::GRADIENT, &a, &two).is_none());
        let stroked = b.clone().backdrop(Some(
            Backdrop::new(30.0, Color::WHITE)
                .stroke(1.0, Color::BLACK)
                .fallback(Color::WHITE),
        ));
        assert!(endpoints(AnimatedProperties::BACKDROP, &a, &stroked).is_none());
        let none = b.clone().backdrop(None);
        assert!(endpoints(AnimatedProperties::BACKDROP, &a, &none).is_none());
        // A spring overshoot keeps stop positions ordered and inside 0..=1.
        let mut out = Style::default();
        write(
            AnimatedProperties::GRADIENT,
            &Shape::Gradient(Some((true, false, 2))),
            &[0.0, 1.3, 255.0, 0.0, 0.0, 255.0, -0.2, 0.0, 0.0, 0.0, 255.0],
            &mut out,
        );
        let g = out.background_gradient.unwrap();
        assert_eq!((g.stops[0].position, g.stops[1].position), (1.0, 1.0));
        assert!(g.valid());
    }

    #[test]
    fn delay_holds_the_old_value_and_uncovered_change_cancels() {
        let rule = Transition::new(AnimatedProperties::OPACITY, 100.0, Easing::LINEAR).delay(50.0);
        let a = Style::default().transition(rule);
        let b = a.clone().opacity(0.0);
        let mut state = MotionState::default();
        state.advance(0.0).unwrap();
        step(&mut state, &a, &b);
        state.advance(0.0).unwrap();
        state.advance(50.0).unwrap();
        assert_eq!(opacity(&state, &b), 1.0);
        state.advance(100.0).unwrap();
        assert!((opacity(&state, &b) - 0.5).abs() < 1e-3);
        // The new style drops the rule: CSS cancels the running transition.
        let c = Style::default().opacity(0.0);
        step(&mut state, &b, &c);
        assert!(!state.running());
        // Removed (retired) nodes lose their channels.
        let mut state = MotionState::default();
        state.advance(0.0).unwrap();
        step(&mut state, &a, &b);
        state.retarget(|_| Some(&b), |_| Some(&b), &BTreeSet::new(), |_| true);
        assert!(!state.running());
    }
}
