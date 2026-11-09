//! Platform-independent rules of the iOS/iPadOS reference host.
//! iOS/iPadOS 参考宿主中与平台无关的规则。
//!
//! These rules are compiled and tested on the build host, so application lifecycle, viewport and
//! safe-area arithmetic are checked without a simulator runtime. The UIKit adapter in `main.rs`
//! supplies the raw events.
//!
//! 这些规则在构建机上编译并测试，因此应用生命周期、viewport 与安全区换算无须模拟器运行时即可检查；
//! `main.rs` 的 UIKit 适配器提供原始事件。

/// UIKit application lifecycle events the host consumes.
/// 宿主消费的 UIKit 应用生命周期事件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleEvent {
    DidBecomeActive,
    WillResignActive,
    DidEnterBackground,
    WillEnterForeground,
    WillTerminate,
}

/// What the host must do to the engine for one lifecycle event.
/// 一个生命周期事件要求宿主对引擎执行的动作。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleAction {
    None,
    /// First activation: configure the surface at the current generation.
    /// 首次激活：按当前代数配置 Surface。
    Activate,
    /// Foreground return: rebuild the surface at the successor generation.
    /// 回到前台：按后继代数重建 Surface。
    Resume,
    /// Resign active: release the drawable and keep scene state.
    /// 失去激活：释放 drawable，保留场景状态。
    Suspend,
    Terminate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleState {
    Launching,
    Active,
    Inactive,
    Background,
    Terminating,
}

/// UIKit lifecycle state machine. Duplicate and out-of-order notifications are no-ops instead of
/// double resuming or suspending the engine.
/// UIKit 生命周期状态机；重复或乱序通知不重复恢复/暂停引擎。
#[derive(Clone, Copy, Debug)]
pub struct Lifecycle {
    state: LifecycleState,
}

impl Lifecycle {
    pub const fn new() -> Self {
        Self {
            state: LifecycleState::Launching,
        }
    }

    pub const fn state(self) -> LifecycleState {
        self.state
    }

    pub fn transition(&mut self, event: LifecycleEvent) -> LifecycleAction {
        use LifecycleAction as Action;
        use LifecycleEvent as Event;
        use LifecycleState as State;
        if event == Event::WillTerminate {
            if self.state == State::Terminating {
                return Action::None;
            }
            self.state = State::Terminating;
            return Action::Terminate;
        }
        match (self.state, event) {
            (State::Terminating, _) => Action::None,
            (State::Launching, Event::DidBecomeActive) => {
                self.state = State::Active;
                Action::Activate
            }
            (State::Active, Event::WillResignActive) => {
                self.state = State::Inactive;
                Action::Suspend
            }
            (State::Inactive, Event::DidBecomeActive) => {
                self.state = State::Active;
                Action::Resume
            }
            (State::Inactive, Event::DidEnterBackground) => {
                self.state = State::Background;
                Action::None
            }
            (State::Background, Event::WillEnterForeground) => {
                self.state = State::Inactive;
                Action::None
            }
            (State::Background, Event::DidBecomeActive) => {
                self.state = State::Active;
                Action::Resume
            }
            _ => Action::None,
        }
    }
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self::new()
    }
}

/// Logic-point inset of the content away from the physical viewport edges.
/// 内容相对物理视口边缘的逻辑点内缩。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Insets {
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Reject negative or non-finite insets and keep the sum strictly inside the viewport.
    /// 拒绝负值或非有限内缩，并把总和严格限制在视口之内。
    pub fn clamp(self, physical_width: u32, physical_height: u32, scale: f32) -> Self {
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let width = physical_width as f32 / scale;
        let height = physical_height as f32 / scale;
        let clean = |value: f32| if value.is_finite() && value > 0.0 { value } else { 0.0 };
        let mut insets = Self {
            top: clean(self.top),
            right: clean(self.right),
            bottom: clean(self.bottom),
            left: clean(self.left),
        };
        // Keep at least one logical pixel of content on each axis.
        // 每个轴至少保留一个逻辑像素的内容。
        if insets.left + insets.right >= width {
            let total = insets.left + insets.right;
            insets.left = insets.left / total * (width - 1.0).max(0.0);
            insets.right = insets.right / total * (width - 1.0).max(0.0);
        }
        if insets.top + insets.bottom >= height {
            let total = insets.top + insets.bottom;
            insets.top = insets.top / total * (height - 1.0).max(0.0);
            insets.bottom = insets.bottom / total * (height - 1.0).max(0.0);
        }
        insets
    }
}

/// Physical viewport plus density and safe-area insets.
/// 物理视口加上像素密度与安全区内缩。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub physical_width: u32,
    pub physical_height: u32,
    pub scale: f32,
    pub insets: Insets,
}

/// Logical content rectangle derived from a viewport.
/// 由 viewport 推导的逻辑内容矩形。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Content {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Viewport {
    pub fn new(physical_width: u32, physical_height: u32, scale: f32, insets: Insets) -> Self {
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        Self {
            physical_width,
            physical_height,
            scale,
            insets: insets.clamp(physical_width, physical_height, scale),
        }
    }

    /// Whole-viewport logical size, used as the engine surface extent.
    /// 整个视口的逻辑尺寸，作为引擎 Surface 尺寸。
    pub fn logical_size(self) -> (f32, f32) {
        (
            self.physical_width as f32 / self.scale,
            self.physical_height as f32 / self.scale,
        )
    }

    /// Logical content rectangle inside the safe area, used for the root scene node.
    /// 安全区内的逻辑内容矩形，用于根场景节点。
    pub fn content(self) -> Content {
        let (width, height) = self.logical_size();
        Content {
            x: self.insets.left,
            y: self.insets.top,
            width: (width - self.insets.left - self.insets.right).max(1.0),
            height: (height - self.insets.top - self.insets.bottom).max(1.0),
        }
    }

    /// Convert one physical pointer sample into the engine's logical coordinates.
    /// 把一个物理指针样本转换为引擎逻辑坐标。
    pub fn to_logical(self, x: f32, y: f32) -> (f32, f32) {
        (x / self.scale, y / self.scale)
    }
}

/// Stable primary-contact tracking for multi-touch routing.
/// 多点触控路由中的稳定主触点跟踪。
#[derive(Clone, Debug, Default)]
pub struct PrimaryContact {
    active: Vec<u64>,
    primary: Option<u64>,
}

impl PrimaryContact {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a contact start; the first contact stays primary until it ends.
    /// 登记触点开始；首个触点在被结束前保持主触点。
    pub fn started(&mut self, id: u64) -> bool {
        if !self.active.contains(&id) {
            self.active.push(id);
        }
        if self.primary.is_none() {
            self.primary = Some(id);
            return true;
        }
        self.primary == Some(id)
    }

    /// Remove an ended contact and hand primary status to the oldest remaining contact.
    /// 移除已结束触点，并把主触点身份交给最早剩余的触点。
    pub fn ended(&mut self, id: u64) -> bool {
        self.active.retain(|contact| *contact != id);
        let was_primary = self.primary == Some(id);
        if was_primary {
            self.primary = self.active.first().copied();
        }
        was_primary
    }

    pub fn primary(&self) -> Option<u64> {
        self.primary
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }
}


/// Calibrated force as winit reports it for a touch or Apple Pencil sample.
/// winit 对触控或 Apple Pencil 样本给出的校准压力。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CalibratedForce {
    pub force: f64,
    pub max_possible_force: f64,
    /// `None` for a finger; `Some(radians)` only for a stylus (winit sets it for Pencil).
    /// 手指为 `None`；只有触控笔（winit 对 Pencil 设置）才是 `Some(弧度)`。
    pub altitude_angle: Option<f64>,
}

/// Host-side stylus sample derived from one touch.
/// 由一次触控推导出的宿主侧触控笔样本。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PencilSample {
    /// Force perpendicular to the screen, normalized to 0..=1.
    /// 垂直于屏幕的压力，归一到 0..=1。
    pub pressure: f32,
    /// Stylus altitude above the surface in degrees; `None` for a finger.
    /// 触控笔相对屏幕的高度角（度）；手指为 `None`。
    pub altitude_degrees: Option<f32>,
    pub stylus: bool,
}

fn fallback_pressure(pressed: bool) -> f32 {
    if pressed {
        1.0
    } else {
        0.0
    }
}

/// Normalize one calibrated force sample into screen-perpendicular pressure and altitude.
///
/// Apple measures Pencil force along the stylus axis, so the perpendicular pressure divides by
/// `sin(altitude)`; a finger reports no altitude and uses the raw ratio. Non-finite or
/// out-of-range input falls back to the pressed/released default instead of leaking NaN.
///
/// Apple 沿笔轴测量 Pencil 压力，因此垂直压力要除以 `sin(高度角)`；手指没有高度角，直接用比值。
/// 非有限或越界输入回退到按下/抬起默认值，不让 NaN 进入布局或手势层。
pub fn pencil_sample(force: Option<CalibratedForce>, pressed: bool) -> PencilSample {
    let Some(force) = force else {
        return PencilSample {
            pressure: fallback_pressure(pressed),
            altitude_degrees: None,
            stylus: false,
        };
    };
    if !force.max_possible_force.is_finite() || force.max_possible_force <= 0.0 {
        return PencilSample {
            pressure: fallback_pressure(pressed),
            altitude_degrees: None,
            stylus: force.altitude_angle.is_some(),
        };
    }
    let altitude = force.altitude_angle.filter(|angle| angle.is_finite() && *angle >= 0.0);
    let perpendicular = match altitude {
        Some(angle) => {
            let sine = angle.sin();
            if sine > 1e-6 {
                force.force / sine
            } else {
                force.force
            }
        }
        None => force.force,
    };
    let pressure = perpendicular / force.max_possible_force;
    let pressure = if pressure.is_finite() {
        pressure.clamp(0.0, 1.0) as f32
    } else {
        fallback_pressure(pressed)
    };
    PencilSample {
        pressure,
        altitude_degrees: altitude.map(|angle| angle.to_degrees().clamp(0.0, 90.0) as f32),
        stylus: force.altitude_angle.is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_lifecycle_cycle_alternates_suspend_and_resume() {
        let mut lifecycle = Lifecycle::new();
        assert_eq!(
            lifecycle.transition(LifecycleEvent::DidBecomeActive),
            LifecycleAction::Activate
        );
        assert_eq!(lifecycle.state(), LifecycleState::Active);
        assert_eq!(
            lifecycle.transition(LifecycleEvent::WillResignActive),
            LifecycleAction::Suspend
        );
        assert_eq!(
            lifecycle.transition(LifecycleEvent::DidEnterBackground),
            LifecycleAction::None
        );
        assert_eq!(lifecycle.state(), LifecycleState::Background);
        assert_eq!(
            lifecycle.transition(LifecycleEvent::WillEnterForeground),
            LifecycleAction::None
        );
        assert_eq!(
            lifecycle.transition(LifecycleEvent::DidBecomeActive),
            LifecycleAction::Resume
        );
        assert_eq!(lifecycle.state(), LifecycleState::Active);
    }

    #[test]
    fn duplicate_notifications_do_not_double_suspend_or_resume() {
        let mut lifecycle = Lifecycle::new();
        assert_eq!(
            lifecycle.transition(LifecycleEvent::DidBecomeActive),
            LifecycleAction::Activate
        );
        assert_eq!(
            lifecycle.transition(LifecycleEvent::WillResignActive),
            LifecycleAction::Suspend
        );
        assert_eq!(
            lifecycle.transition(LifecycleEvent::WillResignActive),
            LifecycleAction::None
        );
        assert_eq!(
            lifecycle.transition(LifecycleEvent::DidBecomeActive),
            LifecycleAction::Resume
        );
        assert_eq!(
            lifecycle.transition(LifecycleEvent::DidBecomeActive),
            LifecycleAction::None
        );
    }

    #[test]
    fn termination_is_final_and_never_terminates_twice() {
        let mut lifecycle = Lifecycle::new();
        lifecycle.transition(LifecycleEvent::DidBecomeActive);
        assert_eq!(
            lifecycle.transition(LifecycleEvent::WillTerminate),
            LifecycleAction::Terminate
        );
        assert_eq!(
            lifecycle.transition(LifecycleEvent::WillTerminate),
            LifecycleAction::None
        );
        assert_eq!(
            lifecycle.transition(LifecycleEvent::DidBecomeActive),
            LifecycleAction::None
        );
        assert_eq!(lifecycle.state(), LifecycleState::Terminating);
    }

    #[test]
    fn background_activation_still_resumes_once() {
        let mut lifecycle = Lifecycle::new();
        lifecycle.transition(LifecycleEvent::DidBecomeActive);
        lifecycle.transition(LifecycleEvent::WillResignActive);
        lifecycle.transition(LifecycleEvent::DidEnterBackground);
        assert_eq!(
            lifecycle.transition(LifecycleEvent::DidBecomeActive),
            LifecycleAction::Resume
        );
        assert_eq!(lifecycle.state(), LifecycleState::Active);
    }

    #[test]
    fn ipad_safe_area_shrinks_content_but_not_the_surface() {
        // iPad Pro 11" portrait: 1668x2388 physical at scale 2, 24pt top and 20pt bottom insets.
        // iPad Pro 11 英寸竖屏：1668x2388 物理像素、scale 2、上 24pt 下 20pt 内缩。
        let viewport = Viewport::new(1668, 2388, 2.0, Insets::new(24.0, 0.0, 20.0, 0.0));
        assert_eq!(viewport.logical_size(), (834.0, 1194.0));
        let content = viewport.content();
        assert_eq!(content.x, 0.0);
        assert_eq!(content.y, 24.0);
        assert_eq!(content.width, 834.0);
        assert_eq!(content.height, 1150.0);
        assert_eq!(viewport.to_logical(1668.0, 2388.0), (834.0, 1194.0));
    }

    #[test]
    fn oversized_or_invalid_insets_keep_at_least_one_pixel_of_content() {
        let viewport = Viewport::new(200, 100, 1.0, Insets::new(500.0, 500.0, 500.0, 500.0));
        let content = viewport.content();
        assert!(content.width >= 1.0);
        assert!(content.height >= 1.0);
        let invalid = Viewport::new(300, 200, f32::NAN, Insets::new(f32::NAN, -4.0, 0.0, 0.0));
        assert_eq!(invalid.scale, 1.0);
        assert_eq!(invalid.insets, Insets::default());
    }

    #[test]
    fn primary_contact_hands_off_to_the_oldest_remaining_touch() {
        let mut contacts = PrimaryContact::new();
        assert!(contacts.started(7));
        assert!(!contacts.started(9));
        assert_eq!(contacts.primary(), Some(7));
        assert!(contacts.ended(7));
        assert_eq!(contacts.primary(), Some(9));
        assert!(!contacts.started(11));
        assert_eq!(contacts.primary(), Some(9));
        assert!(!contacts.ended(11));
        assert_eq!(contacts.active_count(), 1);
    }

    #[test]
    fn pencil_pressure_is_perpendicular_to_the_screen() {
        // 0.5 of maximum force at 30 degrees: sin(30) = 0.5, so the perpendicular pressure is 1.
        // 30 度角下 0.5 倍最大力：sin(30)=0.5，垂直压力为 1。
        let sample = pencil_sample(
            Some(CalibratedForce {
                force: 0.5,
                max_possible_force: 1.0,
                altitude_angle: Some(std::f64::consts::FRAC_PI_6),
            }),
            true,
        );
        assert!(sample.stylus);
        assert!((sample.pressure - 1.0).abs() < 1e-3, "pressure={}", sample.pressure);
        let altitude = sample.altitude_degrees.unwrap();
        assert!((altitude - 30.0).abs() < 1e-3, "altitude={altitude}");
    }

    #[test]
    fn finger_pressure_uses_the_raw_ratio_and_reports_no_altitude() {
        let sample = pencil_sample(
            Some(CalibratedForce {
                force: 0.25,
                max_possible_force: 1.0,
                altitude_angle: None,
            }),
            true,
        );
        assert!(!sample.stylus);
        assert_eq!(sample.altitude_degrees, None);
        assert!((sample.pressure - 0.25).abs() < 1e-6);
    }

    #[test]
    fn missing_or_invalid_force_falls_back_to_the_touch_phase() {
        let down = pencil_sample(None, true);
        assert_eq!(down.pressure, 1.0);
        let up = pencil_sample(None, false);
        assert_eq!(up.pressure, 0.0);
        let invalid = pencil_sample(
            Some(CalibratedForce {
                force: f64::NAN,
                max_possible_force: f64::NAN,
                altitude_angle: None,
            }),
            true,
        );
        assert_eq!(invalid.pressure, 1.0);
        assert!(!invalid.stylus);
    }

    #[test]
    fn hard_stylus_pressure_clamps_and_flat_angle_does_not_divide_by_zero() {
        let hard = pencil_sample(
            Some(CalibratedForce {
                force: 0.5,
                max_possible_force: 1.0,
                altitude_angle: Some(10.0_f64.to_radians()),
            }),
            true,
        );
        assert_eq!(hard.pressure, 1.0);
        let flat = pencil_sample(
            Some(CalibratedForce {
                force: 0.4,
                max_possible_force: 1.0,
                altitude_angle: Some(0.0),
            }),
            true,
        );
        assert!((flat.pressure - 0.4).abs() < 1e-6);
        assert_eq!(flat.altitude_degrees, Some(0.0));
    }
}
