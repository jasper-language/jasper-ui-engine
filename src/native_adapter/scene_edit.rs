//! E14 in-scene text editing on the native scene: session lifecycle tied to scene focus, display
//! of the edit buffer in the native document (never the source scene, never a version), keys,
//! text input, IME preedit, pointer caret placement, caret blink and the edit event outbox.
//! E14 原生场景上的场景内文本编辑：会话生命周期跟随场景焦点；编辑缓冲显示在原生文档中（从不写源场景、从不消耗版本）；
//! 键盘、文字输入、输入法预编辑、指针定位光标、光标闪烁与编辑事件发件箱。
use super::*;
use crate::text_edit::{CaretBlink, EditBuffer, SceneEditEvent, SceneEditEventKind, SceneEditState};

/// Caret blink half-period (ms): visible for one, hidden for the next.
/// 光标闪烁半周期（毫秒）：亮一个周期、灭一个周期。
pub(crate) const CARET_BLINK_MS: f64 = 530.0;
/// Caret width in logical px.
/// 光标宽度（逻辑像素）。
pub(crate) const CARET_WIDTH: f32 = 1.0;
/// Selection highlight (premultiplied by node opacity when painted).
/// 选区高亮色（绘制时再乘节点透明度）。
pub(crate) const SELECTION_RGBA: [u8; 4] = [51, 132, 255, 96];
/// Laid out instead of an empty display text so the caret gets the line's metrics; offset 0
/// is the only position (zero-width space).
/// 显示文本为空时改为排版此字符，使光标获得行度量；只有偏移 0 一个位置（零宽空格）。
const EMPTY_PLACEHOLDER: &str = "\u{200B}";

/// Engine-local edit state; carried across frames, never part of the source scene.
/// 引擎局部编辑状态；跨帧携带，从不属于源场景。
#[derive(Clone, Debug, Default)]
pub(crate) struct EditState {
    session: Option<EditSession>,
    events: Vec<SceneEditEvent>,
    /// A primary press placed the caret in the session's node; moves extend the selection.
    /// 主键按下已在会话节点内放置光标；移动时扩展选区。
    drag: bool,
    /// Text committed on blur, shown until the next applied frame sets the node's text.
    /// 失焦时提交的文本，显示到下一个已应用帧设定该节点文字为止。
    held: Option<(NodeId, String)>,
}

#[derive(Clone, Debug)]
struct EditSession {
    node: NodeId,
    buffer: EditBuffer,
    /// The node's scene text the buffer started from (or the last external update).
    /// 缓冲起始时（或最近一次外部更新后）节点的场景文本。
    source: String,
    /// Last committed text, so Enter then blur does not commit twice.
    /// 上次提交的文本，避免 Enter 后失焦重复提交。
    committed: Option<String>,
    blink_origin: Option<f64>,
    caret_visible: bool,
}

impl NativeScene {
    /// A node the engine edits in place: an enabled, visible Text node with role TextField.
    /// 引擎就地编辑的节点：已启用、可见、角色为 TextField 的 Text 节点。
    fn editable(&self, id: NodeId) -> bool {
        self.source
            .find(id)
            .is_some_and(|n| TextShape::of(n) == TextShape::Field)
            && self.semantics.get(&id).is_some_and(|s| !s.disabled)
            && matches!(self.nodes.get(&id), Some((_, Some(_))))
            && self.source_visible(id)
    }
    fn scene_text(&self, id: NodeId) -> String {
        self.source
            .find(id)
            .and_then(|n| n.text_content())
            .unwrap_or("")
            .to_owned()
    }
    fn push_edit_event(&mut self, node: NodeId, kind: SceneEditEventKind) {
        self.edit.events.push(SceneEditEvent {
            scene_version: self.scene_version,
            node,
            kind,
        });
    }
    /// Write `text` into the node's native text (no source, no version); relayout on change.
    /// 把 `text` 写入节点的原生文字（不写源、不耗版本）；有变化时重新排版。
    fn write_node_text(&mut self, id: NodeId, text: &str) {
        let Some((_, Some(text_node))) = self.nodes.get(&id).copied() else {
            return;
        };
        let Some(document) = self.document.as_mut() else {
            return;
        };
        let unchanged = document
            .get_node(text_node)
            .and_then(|n| n.text_data())
            .is_some_and(|t| t.content == text);
        if unchanged {
            return;
        }
        document.mutate().set_node_text(text_node, text);
        document.resolve(0.0);
        self.invalidate_frame();
    }
    /// Show the session's display text (buffer plus preedit) and repaint.
    /// 显示会话的显示文本（缓冲加预编辑）并重绘。
    fn write_edit_display(&mut self) {
        let Some(session) = self.edit.session.as_ref() else {
            return;
        };
        let node = session.node;
        let (display, _, _) = session.buffer.display();
        let shown = if display.is_empty() {
            EMPTY_PLACEHOLDER.to_owned()
        } else {
            display
        };
        self.write_node_text(node, &shown);
        self.invalidate_frame();
    }
    fn restart_blink(&mut self) {
        if let Some(session) = self.edit.session.as_mut() {
            session.blink_origin = None;
            session.caret_visible = true;
        }
        self.invalidate_frame();
    }
    fn commit_if_changed(&mut self) {
        let Some(session) = self.edit.session.as_mut() else {
            return;
        };
        let text = session.buffer.text().to_owned();
        if text == session.source || session.committed.as_deref() == Some(text.as_str()) {
            return;
        }
        session.committed = Some(text.clone());
        let node = session.node;
        self.push_edit_event(node, SceneEditEventKind::Commit(text));
    }
    /// Bring the session in line with scene focus: a session whose node lost focus (or is no
    /// longer editable) ends — committing when `commit` (blur) and its text changed, else
    /// silently (the frame that removed/disabled it is authoritative) — and a focused editable
    /// node without a session starts one (buffer = its scene text, caret at the end).
    /// 使会话与场景焦点一致：节点失焦（或不再可编辑）的会话结束——`commit`（失焦）且文本有变时提交，否则静默结束
    /// （移除/禁用它的帧为准）；聚焦且可编辑而无会话的节点开始新会话（缓冲 = 其场景文本，光标在末尾）。
    pub(super) fn sync_edit_session(&mut self, commit: bool) {
        let focused = self
            .focused_node(self.owner, self.scene_version)
            .filter(|id| self.editable(*id));
        if self.edit.session.as_ref().map(|s| s.node) == focused {
            return;
        }
        if self.edit.session.is_some() {
            if commit {
                self.commit_if_changed();
            }
            let session = self.edit.session.take().expect("checked");
            self.edit.drag = false;
            if session.buffer.preedit().is_some() {
                self.push_edit_event(session.node, SceneEditEventKind::CompositionCancelled);
            }
            let shown = match &session.committed {
                Some(text) if commit => {
                    self.edit.held = Some((session.node, text.clone()));
                    text.clone()
                }
                _ => self.scene_text(session.node),
            };
            self.write_node_text(session.node, &shown);
            self.invalidate_frame();
        }
        if let Some(node) = focused {
            let source = self.scene_text(node);
            let start = match &self.edit.held {
                Some((held, text)) if *held == node => text.clone(),
                _ => source.clone(),
            };
            let committed = (start != source).then(|| start.clone());
            self.edit.session = Some(EditSession {
                node,
                buffer: EditBuffer::new(&start),
                source,
                committed,
                blink_origin: None,
                caret_visible: true,
            });
            self.write_edit_display();
        }
    }
    /// After a frame: carry the session, pending events and drag from the previous view into
    /// this candidate. An external `text` update of the edited node wins: the buffer becomes
    /// the new text with the caret at the end (an echo equal to the buffer keeps caret and
    /// selection). A node the frame removed or made non-editable ends its session without a
    /// commit. A held committed display is dropped (the frame's text is shown).
    /// 帧之后：把上一视图的会话、待取事件与拖动状态带入本候选。被编辑节点的外部 `text` 更新为准：缓冲变为新文本、
    /// 光标移到末尾（与缓冲相同的回显保留光标与选区）。帧移除或变为不可编辑的节点结束会话且不提交。失焦提交后的保持显示
    /// 被丢弃（显示帧的文字）。
    pub(super) fn carry_edit_from(&mut self, previous: &NativeScene, patches: &[ScenePatch]) {
        self.edit.events = previous.edit.events.clone();
        if let Some(mut session) = previous.edit.session.clone() {
            let node = session.node;
            let kept = !previous.frame_retires_node(node, patches)
                && self.editable(node)
                && self.focused_node(self.owner, self.scene_version) == Some(node);
            if kept {
                let text = self.scene_text(node);
                if text != session.source {
                    if text != session.buffer.text() {
                        if session.buffer.preedit().is_some() {
                            self.push_edit_event(node, SceneEditEventKind::CompositionCancelled);
                        }
                        session.buffer.replace_all(&text);
                    }
                    session.source = text;
                    session.committed = None;
                }
                self.edit.session = Some(session);
                self.edit.drag = previous.edit.drag;
                self.write_edit_display();
            } else if session.buffer.preedit().is_some() {
                self.push_edit_event(node, SceneEditEventKind::CompositionCancelled);
            }
        }
        self.sync_edit_session(false);
    }
    /// Editing keys on the focused session node, after the key sink did not prevent the
    /// default. True when the editor consumed the key. Ctrl or Meta is the command modifier
    /// (select all / copy / cut / paste); Ctrl or Alt moves and deletes by word.
    /// 聚焦会话节点上的编辑键（键事件未被阻止默认行为之后）。编辑器消费该键时为 true。Ctrl 或 Meta 为命令修饰键
    /// （全选/复制/剪切/粘贴）；Ctrl 或 Alt 按词移动与删除。
    pub(super) fn edit_key(&mut self, node: NodeId, input: &crate::native_keys::HostKeyInput<'_>) -> bool {
        enum Effect {
            Nothing,
            Moved,
            Edited,
            Commit,
            Revert,
            Copy(String),
            Paste,
        }
        if !input.pressed || input.composing {
            return false;
        }
        let Some(session) = self.edit.session.as_mut().filter(|s| s.node == node) else {
            return false;
        };
        if session.buffer.preedit().is_some() {
            // The input method owns keys while composing.
            return false;
        }
        let m = input.modifiers;
        let command = m.control || m.meta;
        let word = m.control || m.alt;
        let b = &mut session.buffer;
        let changed = |c: bool, effect: Effect| if c { effect } else { Effect::Nothing };
        let effect = match input.key {
            "Backspace" => changed(b.backspace(word), Effect::Edited),
            "Delete" => changed(b.delete(word), Effect::Edited),
            "ArrowLeft" => changed(b.move_left(m.shift, word), Effect::Moved),
            "ArrowRight" => changed(b.move_right(m.shift, word), Effect::Moved),
            "Home" => changed(b.home(m.shift), Effect::Moved),
            "End" => changed(b.end(m.shift), Effect::Moved),
            "Enter" => Effect::Commit,
            "Escape" => Effect::Revert,
            k if command && k.eq_ignore_ascii_case("a") => changed(b.select_all(), Effect::Moved),
            k if command && k.eq_ignore_ascii_case("c") => match b.selected_text() {
                Some(text) => Effect::Copy(text.to_owned()),
                None => Effect::Nothing,
            },
            k if command && k.eq_ignore_ascii_case("x") => match b.selected_text() {
                Some(text) => {
                    let text = text.to_owned();
                    b.backspace(false);
                    self.push_edit_event(node, SceneEditEventKind::Copy(text));
                    Effect::Edited
                }
                None => Effect::Nothing,
            },
            k if command && k.eq_ignore_ascii_case("v") => Effect::Paste,
            _ => return false,
        };
        match effect {
            Effect::Nothing => {}
            Effect::Moved => self.restart_blink(),
            Effect::Edited => self.after_edit(node),
            Effect::Commit => self.commit_if_changed(),
            Effect::Revert => {
                if let Some(session) = self.edit.session.as_mut() {
                    if session.buffer.text() != session.source {
                        let source = session.source.clone();
                        session.buffer.replace_all(&source);
                        self.after_edit(node);
                    }
                }
            }
            Effect::Copy(text) => self.push_edit_event(node, SceneEditEventKind::Copy(text)),
            Effect::Paste => self.push_edit_event(node, SceneEditEventKind::PasteRequested),
        }
        true
    }
    fn after_edit(&mut self, node: NodeId) {
        let Some(text) = self.edit.session.as_ref().map(|s| s.buffer.text().to_owned()) else {
            return;
        };
        self.push_edit_event(node, SceneEditEventKind::Changed(text));
        self.write_edit_display();
        self.restart_blink();
    }
    /// Committed text input (typed characters, IME commit, paste) into the edited node.
    /// 已提交的文字输入（键入字符、输入法提交、粘贴）写入被编辑节点。
    pub(crate) fn text_input_from_host(&mut self, caller: u64, version: u64, text: &str) -> bool {
        if caller != self.owner || version != self.scene_version || !self.document.is_open() {
            return false;
        }
        let Some(session) = self.edit.session.as_mut() else {
            return false;
        };
        let node = session.node;
        let had_preedit = session.buffer.preedit().is_some();
        if session.buffer.insert(text) {
            self.after_edit(node);
        } else if had_preedit {
            self.write_edit_display();
        }
        true
    }
    /// Set (Some) or clear (None) the IME composition shown at the caret; `cursor` is the IME
    /// cursor/selection inside the preedit text (UTF-8 offsets).
    /// 设置（Some）或清除（None）显示在光标处的输入法组合；`cursor` 为预编辑文本内的输入法光标/选区（UTF-8 偏移）。
    pub(crate) fn preedit_from_host(
        &mut self,
        caller: u64,
        version: u64,
        text: Option<&str>,
        cursor: Option<(usize, usize)>,
    ) -> bool {
        if caller != self.owner || version != self.scene_version || !self.document.is_open() {
            return false;
        }
        let Some(session) = self.edit.session.as_mut() else {
            return false;
        };
        if session.buffer.set_preedit(text.map(|t| (t, cursor))) {
            self.write_edit_display();
            self.restart_blink();
        }
        true
    }
    /// Display offset (of the native text) nearest a view point, mapped to a buffer offset.
    /// 离视图点最近的显示偏移（原生文字中），映射为缓冲偏移。
    fn edit_offset_at(&self, node: NodeId, x: f32, y: f32) -> Option<usize> {
        let session = self.edit.session.as_ref()?;
        let (lx, ly) = self.to_local(node, x, y)?;
        let (text, shown, origin_x, origin_y) = self.node_text_layout(node)?;
        let scale = text.layout.scale() as f64;
        let px = (f64::from(lx) - origin_x) * scale;
        let py = (f64::from(ly) - origin_y) * scale;
        if !px.is_finite() || !py.is_finite() || px.abs() > f32::MAX as f64 || py.abs() > f32::MAX as f64 {
            return None;
        }
        let display = parley::Cursor::from_point(&text.layout, px as f32, py as f32).index();
        let display = if shown == EMPTY_PLACEHOLDER { 0 } else { display };
        Some(session.buffer.buffer_offset(display))
    }
    /// Pointer caret placement after native routing and focus: a primary press (default not
    /// prevented) inside the session node puts the caret at the nearest offset (Shift extends);
    /// moves with the button held extend the selection; release or cancel ends the drag.
    /// 原生路由与聚焦之后的指针光标定位：会话节点内的主键按下（未阻止默认行为）把光标放到最近偏移（Shift 扩展）；
    /// 按住移动扩展选区；抬起或取消结束拖动。
    pub(super) fn edit_pointer(
        &mut self,
        phase: crate::native_view::PointerAction,
        hit: Option<(NodeId, bool)>,
        (x, y): (f32, f32),
        primary: bool,
        shift: bool,
    ) {
        use crate::native_view::PointerAction;
        let Some(node) = self.edit.session.as_ref().map(|s| s.node) else {
            self.edit.drag = false;
            return;
        };
        let composing = self
            .edit
            .session
            .as_ref()
            .is_some_and(|s| s.buffer.preedit().is_some());
        match phase {
            PointerAction::Down => {
                let inside = hit.is_some_and(|(target, prevented)| {
                    !prevented && self.nearest_focus_eligible(target) == Some(node)
                });
                if !(primary && inside) || composing {
                    return;
                }
                if let Some(offset) = self.edit_offset_at(node, x, y) {
                    if let Some(session) = self.edit.session.as_mut() {
                        session.buffer.set_caret(offset, shift);
                    }
                    self.edit.drag = true;
                    self.restart_blink();
                }
            }
            PointerAction::Move if self.edit.drag && primary && !composing => {
                if let Some(offset) = self.edit_offset_at(node, x, y) {
                    let moved = self
                        .edit
                        .session
                        .as_mut()
                        .is_some_and(|s| s.buffer.set_caret(offset, true));
                    if moved {
                        self.restart_blink();
                    }
                }
            }
            PointerAction::Move => {}
            PointerAction::Up | PointerAction::Cancel | PointerAction::Leave => {
                if phase != PointerAction::Leave || !primary {
                    self.edit.drag = false;
                }
            }
        }
    }
    /// Drain the edit events reported since the last call. None for a foreign caller.
    /// 取出自上次调用以来的编辑事件。非归属方为 None。
    pub(crate) fn take_edit_events_from_host(&mut self, caller: u64) -> Option<Vec<SceneEditEvent>> {
        if caller != self.owner {
            return None;
        }
        Some(std::mem::take(&mut self.edit.events))
    }
    /// The active session at `version`, or Some(None) without one. None for a foreign caller,
    /// a stale version or a closed view.
    /// `version` 时的当前会话；无会话时为 Some(None)。非归属方、过期版本或已关闭视图为 None。
    pub(crate) fn edit_state_from_host(
        &self,
        caller: u64,
        version: u64,
    ) -> Option<Option<SceneEditState>> {
        if caller != self.owner || version != self.scene_version || !self.document.is_open() {
            return None;
        }
        let Some(session) = self.edit.session.as_ref() else {
            return Some(None);
        };
        let (_, caret, _) = session.buffer.display();
        Some(Some(SceneEditState {
            scene_version: self.scene_version,
            node: session.node,
            text: session.buffer.text().to_owned(),
            anchor: session.buffer.anchor(),
            caret: session.buffer.caret(),
            preedit: session.buffer.preedit().map(|p| p.text.clone()),
            caret_rect: self.caret_geometry(session.node, caret, CARET_WIDTH),
        }))
    }
    /// Edited text and selection for the accessibility tree: (value, (anchor, caret)).
    /// 供无障碍树使用的编辑文本与选区：(value, (anchor, caret))。
    pub(super) fn edit_accessibility(&self, id: NodeId) -> Option<(String, (usize, usize))> {
        let session = self.edit.session.as_ref().filter(|s| s.node == id)?;
        Some((
            session.buffer.text().to_owned(),
            (session.buffer.anchor(), session.buffer.caret()),
        ))
    }
    /// Caret blink on the host clock (`now_ms`, monotone): the caret shows for
    /// `CARET_BLINK_MS`, hides for as long, and restarts visible after every edit or caret
    /// move. Under reduce-motion it stays visible. A toggle invalidates the frame; the scene
    /// version is never consumed. None for a foreign caller, a closed view or a non-finite time.
    /// 按宿主时钟（`now_ms`，单调）的光标闪烁：亮 `CARET_BLINK_MS`、灭同样时长，每次编辑或移动光标后从「亮」重新开始；
    /// 减少动态时常亮。切换时使帧失效；从不消耗场景版本。非归属方、已关闭视图或时间非有限值时为 None。
    pub(crate) fn caret_blink_from_host(&mut self, caller: u64, now_ms: f64) -> Option<CaretBlink> {
        if caller != self.owner || !self.document.is_open() || !now_ms.is_finite() {
            return None;
        }
        let reduce = self.motion.reduce_motion();
        let Some(session) = self.edit.session.as_mut() else {
            return Some(CaretBlink {
                visible: false,
                next_toggle_ms: None,
            });
        };
        let (visible, next_toggle_ms) = if reduce {
            (true, None)
        } else {
            let origin = *session.blink_origin.get_or_insert(now_ms);
            let phase = ((now_ms - origin).max(0.0) / CARET_BLINK_MS).floor();
            (
                phase % 2.0 == 0.0,
                Some(origin + (phase + 1.0) * CARET_BLINK_MS),
            )
        };
        if visible != session.caret_visible {
            session.caret_visible = visible;
            self.invalidate_frame();
        }
        Some(CaretBlink {
            visible,
            next_toggle_ms,
        })
    }
    /// Reduce-motion turned on: show a hidden caret at once.
    /// 开启减少动态：立即显示隐藏中的光标。
    pub(super) fn edit_reduce_motion(&mut self, on: bool) {
        if let Some(session) = self.edit.session.as_mut() {
            if on && !session.caret_visible {
                session.caret_visible = true;
                self.invalidate_frame();
            }
        }
    }
    /// Edit overlays of the session node, in paint order: selection highlight, preedit
    /// underline, caret. Clipped like the node's text (its own overflow included) and drawn in
    /// its E8 frame; alpha folds the presented opacity chain.
    /// 会话节点的编辑叠加层（绘制顺序）：选区高亮、预编辑下划线、光标。与节点文字同样裁剪（含自身 overflow），
    /// 在其 E8 坐标系中绘制；透明度乘以呈现的透明度链。
    #[cfg(feature = "cpu-pending")]
    pub(super) fn edit_overlays(&self, out: &mut Vec<Overlay>) {
        let Some(session) = self.edit.session.as_ref() else {
            return;
        };
        let node = session.node;
        if !self.source_displayed(node) {
            return;
        }
        let Some((alpha, clips)) = self.paint_context(node, true) else {
            return;
        };
        let Some(style) = self.style_of(node) else {
            return;
        };
        let color = style.color;
        let matrix = self.chain_matrix(node);
        let mut push = |rect: jasper_ui_engine::geometry::Rect, rgba: [u8; 4]| {
            let a = (f32::from(rgba[3]) * alpha).round().clamp(0.0, 255.0) as u8;
            if a == 0 || !(rect.width() > 0.0 && rect.height() > 0.0) {
                return;
            }
            out.push(Overlay {
                outer: crate::style_effects::LogicalBox {
                    x: rect.x(),
                    y: rect.y(),
                    width: rect.width(),
                    height: rect.height(),
                },
                outer_radii: [0.0; 4],
                inner: None,
                rgba: [rgba[0], rgba[1], rgba[2], a],
                clips: clips.clone(),
                matrix,
            });
        };
        let (_, caret, preedit) = session.buffer.display();
        let (start, end) = session.buffer.selection();
        if preedit.is_none() && start < end {
            for rect in self.selection_geometry(node, start, end).unwrap_or_default() {
                push(rect, SELECTION_RGBA);
            }
        }
        let ink = [color.r, color.g, color.b, color.a];
        if let Some((s, e)) = preedit {
            for rect in self.selection_geometry(node, s, e).unwrap_or_default() {
                // Snapped to whole logical px so the 1px line stays crisp.
                let thickness = 1.0_f32.min(rect.height());
                push(
                    jasper_ui_engine::geometry::Rect::new(
                        rect.x(),
                        (rect.y() + rect.height()).floor() - thickness,
                        rect.width(),
                        thickness,
                    ),
                    ink,
                );
            }
        }
        if session.caret_visible && (start == end || preedit.is_some()) {
            if let Some(rect) = self.caret_geometry(node, caret, CARET_WIDTH) {
                push(rect, ink);
            }
        }
    }
}
