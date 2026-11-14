//! E14 in-scene text editing on the native CPU path: a focused TextField shows caret,
//! selection and IME preedit in the scene; keyboard/pointer editing; commit on Enter/blur;
//! external updates; accessibility value/selection; the scene version is never consumed.
//! 原生 CPU 路径上的 E14 场景内文本编辑：聚焦的 TextField 在场景中显示光标、选区与输入法预编辑；键盘/指针编辑；
//! Enter/失焦提交；外部更新；无障碍 value/选区；从不消耗场景版本。
#![cfg(feature = "native-blitz-cpu-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::geometry::{Edges, Rect};
use jasper_ui_engine::input::Modifiers;
use jasper_ui_engine::native::{
    EventReply, HostKeyInput, KeyLocation, PointerAction, SceneEditEvent, SceneEditEventKind,
};
use jasper_ui_engine::scene::{Node, NodeId, Scene, ScenePatch};
use jasper_ui_engine::semantics::{Role, Semantics};
use jasper_ui_engine::style::{Color, Length, Style, TextAlignVertical};

const OWNER: u64 = 14;
const W: u32 = 240;
const H: u32 = 100;
const FIELD: NodeId = NodeId(2);
const BUTTON: NodeId = NodeId(3);

fn font() -> Vec<u8> {
    std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("JASPER_UI_TEST_FONT")).unwrap()
}

fn field_style() -> Style {
    Style::default()
        .size(Length::Px(200.0), Length::Px(32.0))
        .padding(Edges::new(0.0, 6.0, 0.0, 6.0))
        .text(16.0, Color::BLACK)
        .text_align_vertical(Some(TextAlignVertical::Center))
}

fn scene(text: &str) -> Scene {
    Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .size(Length::Px(W as f32), Length::Px(H as f32))
                    .background(Color::WHITE),
            )
            .child(
                Node::text(FIELD, text).with_style(field_style()).with_semantics(
                    Semantics::default()
                        .role(Role::TextField)
                        .label("name")
                        .focusable(true),
                ),
            )
            .child(
                Node::text(BUTTON, "OK")
                    .with_style(Style::default().size(Length::Px(60.0), Length::Px(30.0)))
                    .with_semantics(Semantics::default().role(Role::Button).label("ok").focusable(true)),
            ),
    )
}

struct Fixture {
    e: Engine,
    v: u64,
}

impl Fixture {
    fn new(text: &str) -> Self {
        let s = scene(text);
        let v = s.version();
        Self {
            e: Engine::with_fonts(OWNER, &s, &[&font()], W, H).expect("native import"),
            v,
        }
    }
    fn focus_field(&mut self) {
        assert!(self.e.focus(OWNER, self.v, FIELD, &mut |_| {}));
    }
    fn key_mod(&mut self, key: &str, code: &str, modifiers: Modifiers) -> Vec<String> {
        let mut keys = Vec::new();
        assert!(self.e.key(
            OWNER,
            self.v,
            &HostKeyInput {
                key,
                code,
                pressed: true,
                repeat: false,
                composing: false,
                modifiers,
                location: KeyLocation::Standard,
            },
            &mut |k| {
                keys.push(k.key.clone());
                EventReply::default()
            },
            &mut |_| {},
        ));
        keys
    }
    fn key(&mut self, key: &str, code: &str) -> Vec<String> {
        self.key_mod(key, code, Modifiers::default())
    }
    fn shift(&mut self, key: &str, code: &str) -> Vec<String> {
        self.key_mod(key, code, Modifiers { shift: true, ..Modifiers::default() })
    }
    fn ctrl(&mut self, key: &str, code: &str) -> Vec<String> {
        self.key_mod(key, code, Modifiers { control: true, ..Modifiers::default() })
    }
    fn state(&self) -> jasper_ui_engine::native::SceneEditState {
        self.e.edit_state(OWNER, self.v).unwrap().expect("edit session")
    }
    fn events(&mut self) -> Vec<SceneEditEvent> {
        self.e.take_edit_events(OWNER).unwrap()
    }
    fn render(&mut self) -> Vec<u8> {
        let mut bytes = vec![0u8; (W * H * 4) as usize];
        assert!(self.e.render_to_surface(OWNER, self.v, &mut bytes, (W * 4) as usize, 1 << 24));
        bytes
    }
    fn click(&mut self, x: f32, y: f32, shift: bool) {
        for phase in [PointerAction::Down, PointerAction::Up] {
            assert!(self.e.pointer_input(
                OWNER,
                self.v,
                &jasper_ui_engine::native::ScenePointerInput {
                    phase,
                    position: (x, y),
                    primary_pressed: phase == PointerAction::Down,
                    modifiers: Modifiers { shift, ..Modifiers::default() },
                },
                &mut |_| EventReply::default(),
                &mut |_| {},
            ));
        }
    }
}

fn px(frame: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    frame[i..i + 4].try_into().unwrap()
}

fn kinds(events: &[SceneEditEvent]) -> Vec<SceneEditEventKind> {
    events.iter().map(|e| e.kind.clone()).collect()
}

fn dark(p: [u8; 4]) -> bool {
    p[0] < 100 && p[1] < 100 && p[2] < 100
}

/// Dark pixels in column `x` between rows of `r`.
fn column_dark(frame: &[u8], x: u32, r: Rect) -> usize {
    (r.y().ceil() as u32..(r.y() + r.height()).floor() as u32)
        .filter(|&y| dark(px(frame, x, y)))
        .count()
}

#[test]
fn focus_starts_a_session_and_typing_moves_the_caret_without_versions() {
    let mut f = Fixture::new("ab");
    assert_eq!(f.e.edit_state(OWNER, f.v), Some(None));
    let identity = f.e.snapshot_identity(OWNER).unwrap();
    f.focus_field();
    let s = f.state();
    assert_eq!((s.node, s.text.as_str(), s.anchor, s.caret), (FIELD, "ab", 2, 2));
    let before = s.caret_rect.expect("caret geometry");
    let frame = f.render();
    let cx = before.x().round() as u32;
    assert!(column_dark(&frame, cx, before) >= 10, "caret painted at {before:?}");

    assert!(f.e.text_input(OWNER, f.v, "c\nd"));
    let s = f.state();
    assert_eq!((s.text.as_str(), s.caret), ("abcd", 4));
    let after = s.caret_rect.unwrap();
    assert!(after.x() > before.x() + 10.0, "caret moved right: {before:?} -> {after:?}");
    let frame = f.render();
    assert!(column_dark(&frame, after.x().round() as u32, after) >= 10);
    // The old caret column is now inside glyphs or blank, never a full-height bar.
    assert!(column_dark(&frame, cx, before) < 10);
    assert_eq!(kinds(&f.events()), vec![SceneEditEventKind::Changed("abcd".into())]);
    // Nothing consumed a version; the source text is untouched.
    assert_eq!(f.e.snapshot_identity(OWNER).unwrap(), identity);
    assert_eq!(f.e.text_input(OWNER, f.v + 1, "x"), false);
}

#[test]
fn caret_blinks_on_the_host_clock_and_stays_under_reduce_motion() {
    let mut f = Fixture::new("ab");
    assert_eq!(f.e.caret_blink(OWNER, 0.0).unwrap().visible, false);
    f.focus_field();
    let rect = f.state().caret_rect.unwrap();
    let col = rect.x().round() as u32;
    let b = f.e.caret_blink(OWNER, 1000.0).unwrap();
    assert!(b.visible);
    assert_eq!(b.next_toggle_ms, Some(1530.0));
    assert!(column_dark(&f.render(), col, rect) >= 10);
    let b = f.e.caret_blink(OWNER, 1600.0).unwrap();
    assert!(!b.visible);
    assert_eq!(f.e.needs_frame(OWNER), Some(true));
    assert_eq!(column_dark(&f.render(), col, rect), 0);
    assert!(f.e.caret_blink(OWNER, 2100.0).unwrap().visible);
    // An edit restarts the blink visible.
    f.e.caret_blink(OWNER, 1600.0);
    f.key("ArrowLeft", "ArrowLeft");
    assert!(f.e.caret_blink(OWNER, 5000.0).unwrap().visible);
    f.e.caret_blink(OWNER, 5600.0);
    assert!(f.e.set_reduce_motion(OWNER, true));
    let b = f.e.caret_blink(OWNER, 5700.0).unwrap();
    assert_eq!((b.visible, b.next_toggle_ms), (true, None));
    let rect = f.state().caret_rect.unwrap();
    assert!(column_dark(&f.render(), rect.x().round() as u32, rect) >= 10);
}

#[test]
fn keyboard_editing_selection_and_clipboard_hooks() {
    let mut f = Fixture::new("hello");
    f.focus_field();
    assert_eq!(f.key("Backspace", "Backspace"), vec!["Backspace"]);
    assert_eq!(f.state().text, "hell");
    f.key("Home", "Home");
    f.key("Delete", "Delete");
    assert_eq!((f.state().text.as_str(), f.state().caret), ("ell", 0));
    f.key("ArrowRight", "ArrowRight");
    f.shift("End", "End");
    let s = f.state();
    assert_eq!((s.anchor, s.caret), (1, 3));
    // Selection highlight: a blue-tinted pixel inside the selection, none outside.
    let sel = f.e.text_selection(OWNER, f.v, FIELD, 1, 3).unwrap();
    let r = sel[0];
    let frame = f.render();
    let tinted = |p: [u8; 4]| i32::from(p[2]) > i32::from(p[0]) + 30 && p[2] > 150;
    let (sx, sy) = ((r.x() + 1.0) as u32, (r.y() + 1.0) as u32);
    assert!(tinted(px(&frame, sx, sy)), "{:?}", px(&frame, sx, sy));
    assert!(!tinted(px(&frame, 3, sy)));
    // No caret while a range is selected.
    let caret = f.state().caret_rect.unwrap();
    assert!(column_dark(&frame, caret.x().round() as u32, caret) < 10);
    f.events();
    f.ctrl("c", "KeyC");
    assert_eq!(kinds(&f.events()), vec![SceneEditEventKind::Copy("ll".into())]);
    f.ctrl("x", "KeyX");
    assert_eq!(
        kinds(&f.events()),
        vec![SceneEditEventKind::Copy("ll".into()), SceneEditEventKind::Changed("e".into())]
    );
    f.ctrl("v", "KeyV");
    assert_eq!(kinds(&f.events()), vec![SceneEditEventKind::PasteRequested]);
    assert!(f.e.text_input(OWNER, f.v, "llo"));
    assert_eq!(f.state().text, "ello");
    f.ctrl("a", "KeyA");
    let s = f.state();
    assert_eq!((s.anchor, s.caret), (0, 4));
    f.key("ArrowLeft", "ArrowLeft");
    assert_eq!((f.state().anchor, f.state().caret), (0, 0));
    f.ctrl("ArrowRight", "ArrowRight");
    assert_eq!(f.state().caret, 4);
    f.ctrl("Backspace", "Backspace");
    assert_eq!(f.state().text, "");
    // Empty field still has a caret with line metrics.
    let c = f.state().caret_rect.expect("empty caret");
    assert!(c.height() >= 14.0, "{c:?}");
}

#[test]
fn enter_and_blur_commit_once_and_escape_reverts() {
    let mut f = Fixture::new("ab");
    f.focus_field();
    // Enter without change: no commit, key still reported.
    assert_eq!(f.key("Enter", "Enter"), vec!["Enter"]);
    assert!(f.events().is_empty());
    f.e.text_input(OWNER, f.v, "c");
    f.events();
    f.key("Enter", "Enter");
    let ev = f.events();
    assert_eq!(kinds(&ev), vec![SceneEditEventKind::Commit("abc".into())]);
    assert_eq!((ev[0].node, ev[0].scene_version), (FIELD, f.v));
    // Blur right after: already committed, nothing new; display keeps the committed text.
    assert!(f.e.focus(OWNER, f.v, BUTTON, &mut |_| {}));
    assert_eq!(f.e.edit_state(OWNER, f.v), Some(None));
    assert!(f.events().is_empty());
    // Refocus continues from the held committed text; typing then blur commits once.
    f.focus_field();
    assert_eq!(f.state().text, "abc");
    f.e.text_input(OWNER, f.v, "d");
    f.events();
    let mut focus = Vec::new();
    assert!(f.e.clear_focus(OWNER, f.v, &mut |e| focus.push((e.node, e.focused))));
    assert_eq!(kinds(&f.events()), vec![SceneEditEventKind::Commit("abcd".into())]);
    assert_eq!(focus, vec![(FIELD, false)]);
    // Escape reverts to the scene text (Jasper never echoed the commits here).
    f.focus_field();
    f.e.text_input(OWNER, f.v, "zz");
    f.events();
    f.key("Escape", "Escape");
    assert_eq!(f.state().text, "ab");
    assert_eq!(kinds(&f.events()), vec![SceneEditEventKind::Changed("ab".into())]);
}

#[test]
fn preedit_is_underlined_at_the_caret_and_commit_replaces_it() {
    let mut f = Fixture::new("ab");
    f.focus_field();
    f.key("ArrowLeft", "ArrowLeft");
    assert!(f.e.set_preedit(OWNER, f.v, Some("xyz"), None));
    let s = f.state();
    assert_eq!((s.text.as_str(), s.preedit.as_deref()), ("ab", Some("xyz")));
    // Display text is a·xyz·b; underline under bytes 1..4.
    let r = f.e.text_selection(OWNER, f.v, FIELD, 1, 4).unwrap()[0];
    let frame = f.render();
    let row = (r.y() + r.height()).floor() as u32 - 1;
    let cols = (r.x().ceil() as u32 + 1)..((r.x() + r.width()).floor() as u32 - 1);
    let n = cols.len();
    let under = cols.filter(|&x| dark(px(&frame, x, row))).count();
    assert!(under * 10 >= n * 9, "underline {under}/{n} on row {row}");
    // Editing keys belong to the IME while composing.
    f.key("Backspace", "Backspace");
    assert_eq!(f.state().text, "ab");
    // Clicking does not move the caret either.
    assert!(f.e.text_input(OWNER, f.v, "日本"));
    let s = f.state();
    assert_eq!((s.text.as_str(), s.caret, s.preedit), ("a日本b", 7, None));
    // Clearing an absent preedit is a no-op.
    assert!(f.e.set_preedit(OWNER, f.v, None, None));
    // Losing focus while composing reports the dropped composition.
    f.e.set_preedit(OWNER, f.v, Some("q"), None);
    f.events();
    assert!(f.e.focus(OWNER, f.v, BUTTON, &mut |_| {}));
    assert_eq!(
        kinds(&f.events()),
        vec![
            SceneEditEventKind::Commit("a日本b".into()),
            SceneEditEventKind::CompositionCancelled
        ]
    );
}

#[test]
fn pointer_places_caret_and_drags_a_selection() {
    let mut f = Fixture::new("hello world");
    let b = f.e.node_bounds(OWNER, f.v, FIELD).unwrap();
    let mid = b.y() + b.height() / 2.0;
    // A click on the unfocused field focuses it and puts the caret at the click.
    let start = f.e.text_caret(OWNER, f.v, FIELD, 0, 1.0).unwrap();
    f.click(start.x() + 1.0, mid, false);
    assert_eq!(f.e.focused_node(OWNER, f.v), Some(FIELD));
    assert_eq!((f.state().anchor, f.state().caret), (0, 0));
    let five = f.e.text_caret(OWNER, f.v, FIELD, 5, 1.0).unwrap();
    f.click(five.x(), mid, true);
    assert_eq!((f.state().anchor, f.state().caret), (0, 5));
    // Drag from offset 6 to the end.
    let six = f.e.text_caret(OWNER, f.v, FIELD, 6, 1.0).unwrap();
    let send = |f: &mut Fixture, phase, x: f32, pressed| {
        assert!(f.e.pointer(OWNER, f.v, phase, x, mid, pressed, &mut |_| EventReply::default(), &mut |_| {}));
    };
    send(&mut f, PointerAction::Down, six.x(), true);
    send(&mut f, PointerAction::Move, 230.0, true);
    send(&mut f, PointerAction::Up, 230.0, false);
    assert_eq!((f.state().anchor, f.state().caret), (6, 11));
    send(&mut f, PointerAction::Move, start.x(), false);
    assert_eq!(f.state().caret, 11);
    // Clicking the button blurs (no change, no commit).
    let ob = f.e.node_bounds(OWNER, f.v, BUTTON).unwrap();
    f.click(ob.x() + 5.0, ob.y() + 5.0, false);
    assert_eq!(f.e.edit_state(OWNER, f.v), Some(None));
    assert!(f.events().is_empty());
}

#[test]
fn external_updates_win_and_echoes_keep_the_caret() {
    let mut f = Fixture::new("ab");
    f.focus_field();
    f.e.text_input(OWNER, f.v, "cd");
    f.key("ArrowLeft", "ArrowLeft");
    f.events();
    // Echo of the buffer (live-text): caret and selection kept.
    assert!(f.e.apply(OWNER, f.v, f.v + 1, &[ScenePatch::UpdateText { id: FIELD, text: "abcd".into() }]));
    f.v += 1;
    let s = f.state();
    assert_eq!((s.text.as_str(), s.caret), ("abcd", 3));
    // A different text replaces the buffer, caret to the end.
    assert!(f.e.apply(OWNER, f.v, f.v + 1, &[ScenePatch::UpdateText { id: FIELD, text: "XY".into() }]));
    f.v += 1;
    let s = f.state();
    assert_eq!((s.text.as_str(), s.anchor, s.caret, s.scene_version), ("XY", 2, 2, f.v));
    // Unchanged buffer now: blur commits nothing.
    assert!(f.e.focus(OWNER, f.v, BUTTON, &mut |_| {}));
    assert!(f.events().is_empty());
    // A style frame keeps the session.
    f.focus_field();
    f.e.text_input(OWNER, f.v, "!");
    assert!(f.e.apply(OWNER, f.v, f.v + 1, &[ScenePatch::UpdateStyle { id: FIELD, style: field_style().background(Color::rgb(250, 250, 200)) }]));
    f.v += 1;
    assert_eq!(f.state().text, "XY!");
    // Disabling ends the session without a commit.
    f.events();
    assert!(f.e.apply(OWNER, f.v, f.v + 1, &[ScenePatch::UpdateSemantics {
        id: FIELD,
        semantics: Semantics::default().role(Role::TextField).focusable(true).disabled(true),
    }]));
    f.v += 1;
    assert_eq!(f.e.edit_state(OWNER, f.v), Some(None));
    assert!(f.events().is_empty());
}

#[test]
fn removal_ends_the_session_silently() {
    let mut f = Fixture::new("ab");
    f.focus_field();
    f.e.text_input(OWNER, f.v, "c");
    f.events();
    assert!(f.e.apply(OWNER, f.v, f.v + 1, &[ScenePatch::Remove { id: FIELD }]));
    f.v += 1;
    assert_eq!(f.e.edit_state(OWNER, f.v), Some(None));
    assert!(f.events().is_empty());
    assert!(!f.e.text_input(OWNER, f.v, "x"));
}

#[test]
fn accessibility_tree_reports_buffer_and_selection() {
    let mut f = Fixture::new("abc");
    let node = |f: &Fixture| {
        f.e.accessibility_tree(OWNER, f.v)
            .unwrap()
            .nodes
            .into_iter()
            .find(|n| n.node == FIELD)
            .unwrap()
    };
    assert_eq!(node(&f).text_selection, None);
    f.focus_field();
    f.e.text_input(OWNER, f.v, "d");
    f.shift("ArrowLeft", "ArrowLeft");
    let n = node(&f);
    assert_eq!((n.value.as_str(), n.text_selection), ("abcd", Some((4, 3))));
    assert!(n.focused);
    assert!(f.e.focus(OWNER, f.v, BUTTON, &mut |_| {}));
    assert_eq!(node(&f).text_selection, None);
}

#[test]
fn non_field_text_and_disabled_fields_never_edit() {
    let mut f = Fixture::new("ab");
    assert!(f.e.focus(OWNER, f.v, BUTTON, &mut |_| {}));
    assert_eq!(f.e.edit_state(OWNER, f.v), Some(None));
    assert!(!f.e.text_input(OWNER, f.v, "x"));
    assert!(!f.e.set_preedit(OWNER, f.v, Some("x"), None));
    assert_eq!(f.e.take_edit_events(OWNER + 1), None);
    assert_eq!(f.e.caret_blink(OWNER + 1, 0.0), None);
}
