//! winit desktop host: raw window events to the host-neutral input model.
//! 这一层只做规范化；窗口创建、呈现和 GPU 选择由后续桌面宿主切片接入。

use std::collections::BTreeSet;
use std::time::Instant;

use winit::dpi::PhysicalPosition;
use winit::event::{
    ElementState, Ime, KeyEvent as WinitKeyEvent, MouseButton as WinitMouseButton,
    MouseScrollDelta, WindowEvent,
};
use winit::keyboard::{Key, NamedKey};

use crate::geometry::{Point, Size};
use crate::input::{
    FocusEvent, InputEvent, KeyEvent, KeyPhase, Modifiers, MouseButton,
    PointerEvent, PointerId, PointerKind, PointerPhase, ScrollEvent,
};
use crate::surface::rgb8_word;

// BEGIN VERIFIED DESKTOP IME NORMALIZE KERNEL
/// Keep preedit cursor coordinates separate from document replacement positions.
/// 保留预编辑局部坐标及 None，不伪造文档替换范围。
pub fn normalize_ime(event: &Ime) -> Option<InputEvent> {
    match event {
        Ime::Commit(text) => Some(InputEvent::Text(text.clone())),
        Ime::Preedit(text, cursor) => Some(InputEvent::Preedit {
            text: text.clone(), cursor: *cursor,
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

/// Stateful translator for one desktop window.
// BEGIN VERIFIED DESKTOP INPUT TYPE
pub struct DesktopInput {
    pointer_id: PointerId,
    position: Point,
    pressed: BTreeSet<MouseButton>,
    modifiers: Modifiers,
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
            pointer_id: PointerId(0),
            position: Point::ZERO,
            pressed: BTreeSet::new(),
            modifiers: Modifiers::default(),
            start: Instant::now(),
        }
    }

    /// Translates one winit window event.
    pub fn translate(&mut self, event: &WindowEvent) -> Vec<InputEvent> {
        match event {
            WindowEvent::Focused(focused) => {
                vec![InputEvent::Focus(FocusEvent { focused: *focused })]
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                let state = modifiers.state();
                self.modifiers = Modifiers {
                    shift: state.shift_key(),
                    control: state.control_key(),
                    alt: state.alt_key(),
                    meta: state.super_key(),
                };
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
        self.position = Point::new(position.x as f32, position.y as f32);
        let phase = if self.pressed.is_empty() {
            PointerPhase::Hover
        } else {
            PointerPhase::Move
        };
        self.pointer(phase, self.pressed.iter().next().copied(), 0.0)
    }

    pub fn mouse_input(&mut self, state: ElementState, button: WinitMouseButton) -> PointerEvent {
        let button = map_button(button);
        let phase = match state {
            ElementState::Pressed => {
                self.pressed.insert(button);
                PointerPhase::Down
            }
            ElementState::Released => {
                self.pressed.remove(&button);
                PointerPhase::Up
            }
        };
        let pressure = if state == ElementState::Pressed {
            1.0
        } else {
            0.0
        };
        self.pointer(phase, Some(button), pressure)
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
        let mut out = vec![InputEvent::Key(KeyEvent {
            key: name.clone(),
            phase,
            modifiers: self.modifiers,
            repeat: event.repeat,
        })];

        // Text commits come from IME on modern platforms. For plain character
        // keys without command modifiers, also emit the text sample so simple
        // editors work before a full IME pipeline is attached.
        if phase == KeyPhase::Down
            && !self.modifiers.control
            && !self.modifiers.alt
            && !self.modifiers.meta
        {
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

    fn pointer(
        &self,
        phase: PointerPhase,
        button: Option<MouseButton>,
        pressure: f32,
    ) -> PointerEvent {
        PointerEvent {
            pointer_id: self.pointer_id,
            kind: PointerKind::Mouse,
            phase,
            position: self.position,
            pressure,
            tilt: Point::ZERO,
            twist: 0.0,
            contact: Size::ZERO,
            is_primary: button == Some(MouseButton::Left) || button.is_none(),
            eraser: false,
            button,
            modifiers: self.modifiers,
            timestamp_millis: self.start.elapsed().as_millis() as u64,
        }
    }
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
            Some(InputEvent::Preedit { text: "ni".into(), cursor: Some((0,2)) })
        );
    }

    #[test]
    fn preedit_preserves_absent_and_local_cursor() {
        let mut input=DesktopInput::new();
        for (text,cursor) in [("你好",None),("",None),("你好",Some((0,6))),("ni",Some((0,2)))] {
            assert_eq!(input.ime(&Ime::Preedit(text.into(),cursor)),Some(InputEvent::Preedit{text:text.into(),cursor}));
        }
        assert_eq!(input.ime(&Ime::Enabled),Some(InputEvent::ImeEnabled(true)));assert_eq!(input.ime(&Ime::Disabled),Some(InputEvent::ImeEnabled(false)));
        assert_eq!(input.translate(&WindowEvent::Ime(Ime::Preedit("你好".into(),None))),
            vec![InputEvent::Preedit{text:"你好".into(),cursor:None}]);
        assert_eq!(input.translate(&WindowEvent::Ime(Ime::Preedit("".into(),None))),
            vec![InputEvent::Preedit{text:"".into(),cursor:None}]);
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
