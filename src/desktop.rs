//! winit desktop host: raw window events to the host-neutral input model.
//! 这一层只做规范化；窗口创建、呈现和 GPU 选择由后续桌面宿主切片接入。

use std::time::Instant;

use winit::dpi::PhysicalPosition;
use winit::event::{
    ElementState, Ime, KeyEvent as WinitKeyEvent, MouseButton as WinitMouseButton,
    MouseScrollDelta, WindowEvent,
};
use winit::keyboard::{Key, NamedKey};

use crate::geometry::{Point, Size};
use crate::input::{
    FocusEvent, InputEvent, KeyEvent, KeyPhase, Modifiers, MouseButton, PointerEvent, PointerId,
    PointerKind, PointerPhase, ScrollEvent,
};
use crate::surface::rgb8_word;

// BEGIN VERIFIED DESKTOP IME NORMALIZE KERNEL
/// Keep preedit cursor coordinates separate from document replacement positions.
/// 保留预编辑局部坐标及 None，不伪造文档替换范围。
pub fn normalize_ime(event: &Ime) -> Option<InputEvent> {
    match event {
        Ime::Commit(text) => Some(InputEvent::Text(text.clone())),
        Ime::Preedit(text, cursor) => Some(InputEvent::Preedit {
            text: text.clone(),
            cursor: *cursor,
        }),
        Ime::Enabled => Some(InputEvent::ImeEnabled(true)),
        Ime::Disabled => Some(InputEvent::ImeEnabled(false)),
    }
}
// END VERIFIED DESKTOP IME NORMALIZE KERNEL

const LINE_SCROLL_PX: f32 = 48.0;

// BEGIN VERIFIED PIXEL ERROR TYPE
/// Pixel conversion failures for software presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelError {
    LengthMismatch,
}
// END VERIFIED PIXEL ERROR TYPE

// BEGIN VERIFIED PIXEL CONVERT KERNEL
/// Converts RGBA8 pixels to softbuffer's `0x00RRGGBB` words, ignoring alpha as before.
/// 将 RGBA8 转为 softbuffer 数值像素字；保持既有忽略 alpha 的行为。
pub fn rgba_to_u32(pixels: &[u8], out: &mut [u32]) -> Result<(), PixelError> {
    // Compare the ratio first so even the length check cannot overflow.
    // 先比较比例，确保长度检查本身也不会溢出；错误时尚未写入目标。
    if out.len() > pixels.len() / 4 || pixels.len() != out.len() * 4 {
        return Err(PixelError::LengthMismatch);
    }
    let mut index = 0usize;
    while index < out.len() {
        let base = index * 4;
        out[index] = rgb8_word(pixels[base], pixels[base + 1], pixels[base + 2]);
        index += 1;
    }
    Ok(())
}
// END VERIFIED PIXEL CONVERT KERNEL

// BEGIN VERIFIED DESKTOP POINTER KERNEL
/// Saturates a millisecond count instead of wrapping it into an earlier timestamp.
/// 毫秒数超出 u64 时饱和，不回绕成更早的时间戳。
pub fn desktop_timestamp_millis(elapsed: u128) -> u64 {
    if elapsed > u64::MAX as u128 {
        u64::MAX
    } else {
        elapsed as u64
    }
}

/// Total order of button identities; distinct buttons never share a key.
/// 按钮身份的全序键；不同按钮的键互不相同。
pub fn desktop_button_key(button: MouseButton) -> u32 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Right => 1,
        MouseButton::Middle => 2,
        MouseButton::Back => 3,
        MouseButton::Forward => 4,
        MouseButton::Other(code) => 5 + code as u32,
    }
}

/// First index of ascending `held` whose key is not below `key`.
/// 升序 `held` 中首个键不小于 `key` 的下标。
pub fn desktop_button_position(held: &[MouseButton], key: u32) -> usize {
    let mut at = 0usize;
    while at < held.len() && desktop_button_key(held[at]) < key {
        at += 1;
    }
    at
}

/// A motion sample is a drag while any button is held, otherwise a hover.
pub fn desktop_motion_phase(any_pressed: bool) -> PointerPhase {
    if any_pressed {
        PointerPhase::Move
    } else {
        PointerPhase::Hover
    }
}

/// Assembles one mouse sample; the timestamp is an input, never read from a clock here.
/// 组装一个鼠标样本；时间戳由调用方传入，此处不读时钟。
pub fn desktop_pointer_event(
    pointer_id: PointerId,
    phase: PointerPhase,
    position: Point,
    button: Option<MouseButton>,
    pressure: f32,
    modifiers: Modifiers,
    timestamp_millis: u64,
) -> PointerEvent {
    PointerEvent {
        pointer_id,
        kind: PointerKind::Mouse,
        phase,
        position,
        pressure,
        tilt: Point { x: 0.0, y: 0.0 },
        twist: 0.0,
        contact: Size {
            width: 0.0,
            height: 0.0,
        },
        is_primary: matches!(button, Some(MouseButton::Left) | None),
        eraser: false,
        button,
        modifiers,
        timestamp_millis,
    }
}
// END VERIFIED DESKTOP POINTER KERNEL

/// Clock-free pointer state of one desktop window; every transition takes its timestamp.
/// 单窗口无时钟指针状态；每次转换都显式接收时间戳。
// BEGIN VERIFIED DESKTOP POINTER STATE TYPE
pub struct DesktopPointerState {
    pointer_id: PointerId,
    position: Point,
    // Strictly ascending by `desktop_button_key`; a window holds only a handful of buttons.
    // 按 `desktop_button_key` 严格升序；一个窗口同时按下的按钮极少。
    pressed: Vec<MouseButton>,
    modifiers: Modifiers,
}
// END VERIFIED DESKTOP POINTER STATE TYPE

// BEGIN VERIFIED DESKTOP POINTER STATE KERNEL
impl DesktopPointerState {
    pub fn new() -> Self {
        Self {
            pointer_id: PointerId(0),
            position: Point { x: 0.0, y: 0.0 },
            pressed: Vec::new(),
            modifiers: Modifiers {
                shift: false,
                control: false,
                alt: false,
                meta: false,
            },
        }
    }

    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    pub fn set_modifiers(&mut self, modifiers: Modifiers) {
        self.modifiers = modifiers;
    }

    pub fn moved_at(&mut self, position: Point, timestamp_millis: u64) -> PointerEvent {
        self.position = position;
        let button = if self.pressed.is_empty() {
            None
        } else {
            Some(self.pressed[0])
        };
        desktop_pointer_event(
            self.pointer_id,
            desktop_motion_phase(!self.pressed.is_empty()),
            self.position,
            button,
            0.0,
            self.modifiers,
            timestamp_millis,
        )
    }

    pub fn pressed_at(&mut self, button: MouseButton, timestamp_millis: u64) -> PointerEvent {
        let key = desktop_button_key(button);
        let at = desktop_button_position(self.pressed.as_slice(), key);
        if at == self.pressed.len() || desktop_button_key(self.pressed[at]) != key {
            self.pressed.insert(at, button);
        }
        desktop_pointer_event(
            self.pointer_id,
            PointerPhase::Down,
            self.position,
            Some(button),
            1.0,
            self.modifiers,
            timestamp_millis,
        )
    }

    pub fn released_at(&mut self, button: MouseButton, timestamp_millis: u64) -> PointerEvent {
        let key = desktop_button_key(button);
        let at = desktop_button_position(self.pressed.as_slice(), key);
        if at < self.pressed.len() && desktop_button_key(self.pressed[at]) == key {
            self.pressed.remove(at);
        }
        desktop_pointer_event(
            self.pointer_id,
            PointerPhase::Up,
            self.position,
            Some(button),
            0.0,
            self.modifiers,
            timestamp_millis,
        )
    }
}
// END VERIFIED DESKTOP POINTER STATE KERNEL

impl Default for DesktopPointerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Stateful translator for one desktop window; the clock is read only in `now_millis`.
/// 单窗口有状态转换器；只有 `now_millis` 读时钟。
// BEGIN VERIFIED DESKTOP INPUT TYPE
pub struct DesktopInput {
    pointer: DesktopPointerState,
    start: Instant,
}
// END VERIFIED DESKTOP INPUT TYPE

impl Default for DesktopInput {
    fn default() -> Self {
        Self::new()
    }
}

impl DesktopInput {
    pub fn new() -> Self {
        Self {
            pointer: DesktopPointerState::new(),
            start: Instant::now(),
        }
    }

    fn now_millis(&self) -> u64 {
        desktop_timestamp_millis(self.start.elapsed().as_millis())
    }

    /// Translates one winit window event.
    pub fn translate(&mut self, event: &WindowEvent) -> Vec<InputEvent> {
        match event {
            WindowEvent::Focused(focused) => {
                vec![InputEvent::Focus(FocusEvent { focused: *focused })]
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                let state = modifiers.state();
                self.pointer.set_modifiers(Modifiers {
                    shift: state.shift_key(),
                    control: state.control_key(),
                    alt: state.alt_key(),
                    meta: state.super_key(),
                });
                Vec::new()
            }
            WindowEvent::CursorMoved { position, .. } => {
                vec![InputEvent::Pointer(self.cursor_moved(*position))]
            }
            WindowEvent::MouseInput { state, button, .. } => {
                vec![InputEvent::Pointer(self.mouse_input(*state, *button))]
            }
            WindowEvent::MouseWheel { delta, .. } => {
                vec![InputEvent::Scroll(self.mouse_wheel(*delta))]
            }
            WindowEvent::KeyboardInput { event, .. } => self.keyboard(event),
            WindowEvent::Ime(ime) => self.ime(ime).into_iter().collect(),
            _ => Vec::new(),
        }
    }

    pub fn cursor_moved(&mut self, position: PhysicalPosition<f64>) -> PointerEvent {
        let now = self.now_millis();
        self.pointer
            .moved_at(Point::new(position.x as f32, position.y as f32), now)
    }

    pub fn mouse_input(&mut self, state: ElementState, button: WinitMouseButton) -> PointerEvent {
        let button = map_button(button);
        let now = self.now_millis();
        match state {
            ElementState::Pressed => self.pointer.pressed_at(button, now),
            ElementState::Released => self.pointer.released_at(button, now),
        }
    }

    /// winit reports positive deltas as content moving right/down; the engine
    /// uses positive offsets for content moving up/left, so both axes invert.
    pub fn mouse_wheel(&mut self, delta: MouseScrollDelta) -> ScrollEvent {
        let delta = match delta {
            MouseScrollDelta::LineDelta(x, y) => {
                Point::new(-x * LINE_SCROLL_PX, -y * LINE_SCROLL_PX)
            }
            MouseScrollDelta::PixelDelta(position) => {
                Point::new(-(position.x as f32), -(position.y as f32))
            }
        };
        ScrollEvent { delta }
    }

    pub fn keyboard(&mut self, event: &WinitKeyEvent) -> Vec<InputEvent> {
        let phase = match event.state {
            ElementState::Pressed => KeyPhase::Down,
            ElementState::Released => KeyPhase::Up,
        };
        let name = key_name(&event.logical_key);
        let modifiers = self.pointer.modifiers();
        let mut out = vec![InputEvent::Key(KeyEvent {
            key: name.clone(),
            phase,
            modifiers,
            repeat: event.repeat,
        })];

        // Text commits come from IME on modern platforms. For plain character
        // keys without command modifiers, also emit the text sample so simple
        // editors work before a full IME pipeline is attached.
        if phase == KeyPhase::Down && !modifiers.control && !modifiers.alt && !modifiers.meta {
            if let Key::Character(text) = &event.logical_key {
                out.push(InputEvent::Text(text.to_string()));
            } else if event.logical_key == Key::Named(NamedKey::Space) {
                out.push(InputEvent::Text(" ".into()));
            }
        }
        out
    }

    // BEGIN VERIFIED DESKTOP IME DELEGATE KERNEL
    pub fn ime(&mut self, event: &Ime) -> Option<InputEvent> {
        normalize_ime(event)
    }
    // END VERIFIED DESKTOP IME DELEGATE KERNEL
}

fn map_button(button: WinitMouseButton) -> MouseButton {
    match button {
        WinitMouseButton::Left => MouseButton::Left,
        WinitMouseButton::Right => MouseButton::Right,
        WinitMouseButton::Middle => MouseButton::Middle,
        WinitMouseButton::Back => MouseButton::Back,
        WinitMouseButton::Forward => MouseButton::Forward,
        WinitMouseButton::Other(code) => MouseButton::Other(code),
    }
}

fn key_name(key: &Key) -> String {
    match key {
        Key::Character(text) => text.to_string(),
        Key::Named(named) => match named {
            NamedKey::Enter => "Enter".into(),
            NamedKey::Tab => "Tab".into(),
            NamedKey::Escape => "Escape".into(),
            NamedKey::Space => "Space".into(),
            NamedKey::Backspace => "Backspace".into(),
            NamedKey::Delete => "Delete".into(),
            NamedKey::Insert => "Insert".into(),
            NamedKey::Home => "Home".into(),
            NamedKey::End => "End".into(),
            NamedKey::PageUp => "PageUp".into(),
            NamedKey::PageDown => "PageDown".into(),
            NamedKey::ArrowUp => "ArrowUp".into(),
            NamedKey::ArrowDown => "ArrowDown".into(),
            NamedKey::ArrowLeft => "ArrowLeft".into(),
            NamedKey::ArrowRight => "ArrowRight".into(),
            NamedKey::Shift => "Shift".into(),
            NamedKey::Control => "Control".into(),
            NamedKey::Alt => "Alt".into(),
            NamedKey::Super => "Super".into(),
            other => format!("{other:?}"),
        },
        Key::Dead(_) => "Dead".into(),
        Key::Unidentified(_) => "Unidentified".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_mouse_buttons_and_tracks_pressed_state() {
        let mut input = DesktopInput::new();
        let down = input.mouse_input(ElementState::Pressed, WinitMouseButton::Right);
        assert_eq!(down.phase, PointerPhase::Down);
        assert_eq!(down.button, Some(MouseButton::Right));
        assert!(!down.is_primary);

        let moved = input.cursor_moved(PhysicalPosition::new(10.0, 20.0));
        assert_eq!(moved.phase, PointerPhase::Move);
        assert_eq!(moved.position, Point::new(10.0, 20.0));

        let up = input.mouse_input(ElementState::Released, WinitMouseButton::Right);
        assert_eq!(up.phase, PointerPhase::Up);
        assert_eq!(
            input.cursor_moved(PhysicalPosition::new(11.0, 21.0)).phase,
            PointerPhase::Hover
        );
    }

    #[test]
    fn pointer_state_transitions_echo_explicit_timestamps() {
        let mut state = DesktopPointerState::new();
        let hover = state.moved_at(Point::new(1.0, 2.0), 7);
        assert_eq!(
            (hover.phase, hover.button, hover.timestamp_millis),
            (PointerPhase::Hover, None, 7)
        );
        assert!(hover.is_primary);
        let down = state.pressed_at(MouseButton::Right, u64::MAX);
        assert_eq!(
            (
                down.phase,
                down.position,
                down.pressure,
                down.timestamp_millis
            ),
            (PointerPhase::Down, Point::new(1.0, 2.0), 1.0, u64::MAX)
        );
        assert!(!down.is_primary);
        // An earlier host timestamp is reported as given, not clamped to the previous sample.
        let drag = state.moved_at(Point::new(3.0, 4.0), 3);
        assert_eq!(
            (drag.phase, drag.button, drag.timestamp_millis),
            (PointerPhase::Move, Some(MouseButton::Right), 3)
        );
        let up = state.released_at(MouseButton::Right, 9);
        assert_eq!((up.phase, up.pressure), (PointerPhase::Up, 0.0));
        assert_eq!(state.moved_at(Point::ZERO, 10).phase, PointerPhase::Hover);
    }

    #[test]
    fn elapsed_millis_saturate_instead_of_wrapping() {
        assert_eq!(desktop_timestamp_millis(0), 0);
        assert_eq!(desktop_timestamp_millis(u64::MAX as u128), u64::MAX);
        assert_eq!(desktop_timestamp_millis(u64::MAX as u128 + 1), u64::MAX);
        assert_eq!(desktop_timestamp_millis(u128::MAX), u64::MAX);
    }

    #[test]
    fn scroll_delta_is_inverted_for_engine_offsets() {
        let mut input = DesktopInput::new();
        let lines = input.mouse_wheel(MouseScrollDelta::LineDelta(0.0, 2.0));
        assert_eq!(lines.delta, Point::new(0.0, -96.0));
        let pixels = input.mouse_wheel(MouseScrollDelta::PixelDelta(PhysicalPosition::new(
            3.0, 4.0,
        )));
        assert_eq!(pixels.delta, Point::new(-3.0, -4.0));
    }

    #[test]
    fn maps_named_and_character_keys() {
        assert_eq!(key_name(&Key::Named(NamedKey::Enter)), "Enter");
        assert_eq!(key_name(&Key::Named(NamedKey::ArrowLeft)), "ArrowLeft");
        assert_eq!(key_name(&Key::Character("a".into())), "a");
    }

    #[test]
    fn maps_ime_commit_and_preedit() {
        let mut input = DesktopInput::new();
        assert_eq!(
            input.ime(&Ime::Commit("你好".into())),
            Some(InputEvent::Text("你好".into()))
        );
        assert_eq!(
            input.ime(&Ime::Preedit("ni".into(), Some((0, 2)))),
            Some(InputEvent::Preedit {
                text: "ni".into(),
                cursor: Some((0, 2))
            })
        );
    }

    #[test]
    fn preedit_preserves_absent_and_local_cursor() {
        let mut input = DesktopInput::new();
        for (text, cursor) in [
            ("你好", None),
            ("", None),
            ("你好", Some((0, 6))),
            ("ni", Some((0, 2))),
        ] {
            assert_eq!(
                input.ime(&Ime::Preedit(text.into(), cursor)),
                Some(InputEvent::Preedit {
                    text: text.into(),
                    cursor
                })
            );
        }
        assert_eq!(input.ime(&Ime::Enabled), Some(InputEvent::ImeEnabled(true)));
        assert_eq!(
            input.ime(&Ime::Disabled),
            Some(InputEvent::ImeEnabled(false))
        );
        assert_eq!(
            input.translate(&WindowEvent::Ime(Ime::Preedit("你好".into(), None))),
            vec![InputEvent::Preedit {
                text: "你好".into(),
                cursor: None
            }]
        );
        assert_eq!(
            input.translate(&WindowEvent::Ime(Ime::Preedit("".into(), None))),
            vec![InputEvent::Preedit {
                text: "".into(),
                cursor: None
            }]
        );
    }

    #[test]
    fn converts_rgba_to_softbuffer_words() {
        let mut out = [0u32; 2];
        rgba_to_u32(&[255, 0, 0, 255, 0, 128, 255, 255], &mut out).unwrap();
        assert_eq!(out, [0x00FF0000, 0x000080FF]);
        assert_eq!(
            rgba_to_u32(&[1, 2, 3, 4], &mut []),
            Err(PixelError::LengthMismatch)
        );
    }
}
