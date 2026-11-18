//! E14 in-scene text editing: the engine-local edit buffer of a focused TextField node.
//! Pure buffer logic (no layout, no Blitz): UTF-8 byte offsets, grapheme-cluster deletion and
//! movement, word movement, selection, IME preedit. The scene adapter (`native_scene.rs`) owns
//! the session lifecycle, display and painting.
//! E14 场景内文本编辑：聚焦 TextField 节点的引擎局部编辑缓冲。纯缓冲逻辑（无布局、无 Blitz）：UTF-8 字节偏移、
//! 按字素簇删除与移动、按词移动、选区、输入法预编辑。会话生命周期、显示与绘制由场景适配层（`native_scene.rs`）负责。
use jasper_ui_engine::{geometry::Rect, scene::NodeId};
use unicode_segmentation::UnicodeSegmentation;

/// What an in-scene edit session reports to the host (drained with `Engine::take_edit_events`).
/// 场景内编辑会话报告给宿主的内容（经 `Engine::take_edit_events` 取出）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneEditEventKind {
    /// The edit buffer text changed (typing, deletion, cut, paste, Escape revert). Hosts in
    /// live-text mode forward it as `text-commit`; otherwise informational.
    /// 编辑缓冲文本变化（输入、删除、剪切、粘贴、Esc 还原）。实时文本模式的宿主将其转发为 `text-commit`；否则仅供参考。
    Changed(String),
    /// JUI 2 `text-commit`: Enter or blur with text that differs from the node's scene text
    /// (and from the last commit); the payload is the whole text.
    /// JUI 2 `text-commit`：Enter 或失焦，且文本不同于节点的场景文本（及上次提交）；载荷为完整文本。
    Commit(String),
    /// Copy/cut: the host writes this text to its clipboard.
    /// 复制/剪切：宿主把该文本写入剪贴板。
    Copy(String),
    /// Paste: the host reads its clipboard and calls `Engine::text_input` with it.
    /// 粘贴：宿主读取剪贴板并以其调用 `Engine::text_input`。
    PasteRequested,
    /// The engine dropped an active IME composition (session ended, or an external text update
    /// replaced the buffer); the host should reset its input method.
    /// 引擎丢弃了进行中的输入法组合（会话结束或外部文本更新替换了缓冲）；宿主应重置输入法。
    CompositionCancelled,
}

/// One edit event of a TextField node at the scene version current when it happened.
/// 某 TextField 节点的一条编辑事件，附发生时的场景版本。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneEditEvent {
    pub scene_version: u64,
    pub node: NodeId,
    pub kind: SceneEditEventKind,
}

/// Read-only view of the active edit session (for IME placement and diagnostics).
/// 当前编辑会话的只读视图（供输入法定位与诊断）。
#[derive(Clone, Debug, PartialEq)]
pub struct SceneEditState {
    pub scene_version: u64,
    pub node: NodeId,
    /// Edit buffer (committed text, no preedit).
    /// 编辑缓冲（已提交文本，不含预编辑）。
    pub text: String,
    /// Selection anchor and caret, UTF-8 offsets into `text` (equal when collapsed).
    /// 选区锚点与光标，`text` 内 UTF-8 偏移（折叠时相等）。
    pub anchor: usize,
    pub caret: usize,
    /// Active preedit text, if any.
    /// 进行中的预编辑文本（若有）。
    pub preedit: Option<String>,
    /// Caret rectangle in node_bounds coordinates (untransformed layout frame), where the IME
    /// candidate window goes. None when the layout gives no geometry.
    /// node_bounds 坐标（未变换布局坐标系）下的光标矩形，输入法候选窗的位置；布局无几何时为 None。
    pub caret_rect: Option<Rect>,
}

/// Caret blink state from `Engine::caret_blink`.
/// `Engine::caret_blink` 给出的光标闪烁状态。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaretBlink {
    /// The caret is painted (false without an edit session).
    /// 光标是否绘制（无编辑会话时为 false）。
    pub visible: bool,
    /// Host clock time of the next toggle; None without a session or under reduce-motion
    /// (the caret then stays visible).
    /// 下一次切换的宿主时钟时间；无会话或减少动态时为 None（此时光标常亮）。
    pub next_toggle_ms: Option<f64>,
}

/// IME composition text shown at the caret, with an optional IME cursor/selection inside it
/// (UTF-8 offsets into the preedit text).
/// 显示在光标处的输入法组合文本，及其中可选的输入法光标/选区（预编辑文本内的 UTF-8 偏移）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Preedit {
    pub text: String,
    pub cursor: Option<(usize, usize)>,
}

/// Edit buffer: committed text, selection (`anchor`..`caret`, either order) and preedit.
/// Invariant: `anchor` and `caret` are char boundaries of `text`; text holds no control chars.
/// 编辑缓冲：已提交文本、选区（`anchor`..`caret`，可为任一顺序）与预编辑。
/// 不变式：`anchor` 与 `caret` 是 `text` 的字符边界；文本不含控制字符。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EditBuffer {
    text: String,
    anchor: usize,
    caret: usize,
    preedit: Option<Preedit>,
    multiline: bool,
}

/// Field text admits no control characters (a single-line field): they are dropped on input.
/// 文本框文本不含控制字符（单行文本框）：输入时丢弃。
pub(crate) fn sanitize(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

/// Multiline text admits LF while dropping other controls; CRLF and bare CR become LF.
/// 多行文本保留 LF、丢弃其他控制字符；CRLF 与裸 CR 规范化为 LF。
pub(crate) fn sanitize_multiline(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push('\n');
            }
            '\n' => out.push('\n'),
            c if !c.is_control() => out.push(c),
            _ => {}
        }
    }
    out
}

impl EditBuffer {
    /// Buffer holding `text` (control characters dropped) with the caret at its end.
    /// 持有 `text`（丢弃控制字符）、光标在末尾的缓冲。
    pub fn new(text: &str) -> Self {
        Self::new_with_mode(text, false)
    }

    /// Multiline buffer preserving paragraph separators.
    /// 保留段落分隔符的多行缓冲。
    pub fn new_multiline(text: &str) -> Self {
        Self::new_with_mode(text, true)
    }

    fn new_with_mode(text: &str, multiline: bool) -> Self {
        let text = if multiline {
            sanitize_multiline(text)
        } else {
            sanitize(text)
        };
        let end = text.len();
        Self {
            text,
            anchor: end,
            caret: end,
            preedit: None,
            multiline,
        }
    }
    pub fn is_multiline(&self) -> bool {
        self.multiline
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn caret(&self) -> usize {
        self.caret
    }
    pub fn anchor(&self) -> usize {
        self.anchor
    }
    /// Ordered selection range; empty when collapsed.
    /// 有序选区；折叠时为空区间。
    pub fn selection(&self) -> (usize, usize) {
        (self.anchor.min(self.caret), self.anchor.max(self.caret))
    }
    pub fn has_selection(&self) -> bool {
        self.anchor != self.caret
    }
    pub fn preedit(&self) -> Option<&Preedit> {
        self.preedit.as_ref()
    }
    pub fn selected_text(&self) -> Option<&str> {
        let (start, end) = self.selection();
        (start < end).then(|| &self.text[start..end])
    }
    /// Displayed text: the buffer with the preedit inserted at the caret (replacing nothing; a
    /// selection under composition stays in the buffer until the commit replaces it). Returns
    /// the text, the caret and the preedit range in display offsets.
    /// 显示文本：在光标处插入预编辑的缓冲（不替换任何内容；组合期间的选区留在缓冲中直到提交替换）。
    /// 返回文本，以及显示偏移下的光标与预编辑区间。
    pub fn display(&self) -> (String, usize, Option<(usize, usize)>) {
        match &self.preedit {
            None => (self.text.clone(), self.caret, None),
            Some(p) => {
                let mut out = String::with_capacity(self.text.len() + p.text.len());
                out.push_str(&self.text[..self.caret]);
                out.push_str(&p.text);
                out.push_str(&self.text[self.caret..]);
                let start = self.caret;
                let end = start + p.text.len();
                let caret = match p.cursor {
                    Some((_, focus)) => start + focus,
                    None => end,
                };
                (out, caret, Some((start, end)))
            }
        }
    }
    /// Map a display offset (from layout hit testing) back to a buffer offset: inside or after
    /// the preedit it collapses onto the caret / shifts left by the preedit length.
    /// 把显示偏移（来自布局命中）映射回缓冲偏移：落在预编辑内或之后时折叠到光标 / 左移预编辑长度。
    pub fn buffer_offset(&self, display: usize) -> usize {
        let offset = match &self.preedit {
            None => display,
            Some(p) => {
                if display <= self.caret {
                    display
                } else if display <= self.caret + p.text.len() {
                    self.caret
                } else {
                    display - p.text.len()
                }
            }
        };
        self.floor_boundary(offset.min(self.text.len()))
    }
    fn floor_boundary(&self, mut offset: usize) -> usize {
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }
    fn replace_selection(&mut self, insert: &str) {
        let (start, end) = self.selection();
        self.text.replace_range(start..end, insert);
        self.caret = start + insert.len();
        self.anchor = self.caret;
    }
    /// Insert committed text over the selection (control characters dropped); ends any
    /// composition. False when nothing changed.
    /// 在选区处插入已提交文本（丢弃控制字符）；结束组合。无变化时为 false。
    pub fn insert(&mut self, text: &str) -> bool {
        self.preedit = None;
        let text = if self.multiline {
            sanitize_multiline(text)
        } else {
            sanitize(text)
        };
        if text.is_empty() && !self.has_selection() {
            return false;
        }
        self.replace_selection(&text);
        true
    }
    /// Delete the selection, else the grapheme cluster (or word with `word`) before the caret.
    /// 删除选区；无选区时删除光标前一个字素簇（`word` 时为一个词）。
    pub fn backspace(&mut self, word: bool) -> bool {
        if self.has_selection() {
            self.replace_selection("");
            return true;
        }
        let to = if word {
            self.word_before(self.caret)
        } else {
            self.grapheme_before(self.caret)
        };
        if to == self.caret {
            return false;
        }
        self.anchor = to;
        self.replace_selection("");
        true
    }
    /// Delete the selection, else the grapheme cluster (or word) after the caret.
    /// 删除选区；无选区时删除光标后一个字素簇（或词）。
    pub fn delete(&mut self, word: bool) -> bool {
        if self.has_selection() {
            self.replace_selection("");
            return true;
        }
        let to = if word {
            self.word_after(self.caret)
        } else {
            self.grapheme_after(self.caret)
        };
        if to == self.caret {
            return false;
        }
        self.anchor = to;
        self.replace_selection("");
        true
    }
    fn grapheme_before(&self, offset: usize) -> usize {
        self.text[..offset]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(i, _)| i)
    }
    fn grapheme_after(&self, offset: usize) -> usize {
        self.text[offset..]
            .graphemes(true)
            .next()
            .map_or(offset, |g| offset + g.len())
    }
    /// Start of the word at or before `offset` (skipping whitespace/punctuation first).
    /// `offset` 处或之前的词首（先跳过空白/标点）。
    fn word_before(&self, offset: usize) -> usize {
        let mut out = 0;
        for (i, w) in self.text[..offset].split_word_bound_indices().rev() {
            out = i;
            if w.chars().any(char::is_alphanumeric) {
                break;
            }
        }
        out.min(offset)
    }
    /// End of the word at or after `offset` (skipping whitespace/punctuation first).
    /// `offset` 处或之后的词尾（先跳过空白/标点）。
    fn word_after(&self, offset: usize) -> usize {
        let mut out = offset;
        for (i, w) in self.text[offset..].split_word_bound_indices() {
            out = offset + i + w.len();
            if w.chars().any(char::is_alphanumeric) {
                break;
            }
        }
        out
    }
    fn place(&mut self, to: usize, extend: bool) -> bool {
        let before = (self.anchor, self.caret);
        self.caret = to;
        if !extend {
            self.anchor = to;
        }
        before != (self.anchor, self.caret)
    }
    /// Left one grapheme (or word); without `extend` a selection collapses to its start.
    /// 左移一个字素簇（或词）；不扩展时选区折叠到起点。
    pub fn move_left(&mut self, extend: bool, word: bool) -> bool {
        if !extend && self.has_selection() && !word {
            let start = self.selection().0;
            return self.place(start, false);
        }
        let to = if word {
            self.word_before(self.caret)
        } else {
            self.grapheme_before(self.caret)
        };
        self.place(to, extend)
    }
    /// Right one grapheme (or word); without `extend` a selection collapses to its end.
    /// 右移一个字素簇（或词）；不扩展时选区折叠到终点。
    pub fn move_right(&mut self, extend: bool, word: bool) -> bool {
        if !extend && self.has_selection() && !word {
            let end = self.selection().1;
            return self.place(end, false);
        }
        let to = if word {
            self.word_after(self.caret)
        } else {
            self.grapheme_after(self.caret)
        };
        self.place(to, extend)
    }
    fn line_start(&self, offset: usize) -> usize {
        self.text[..offset].rfind('\n').map_or(0, |index| index + 1)
    }
    fn line_end(&self, offset: usize) -> usize {
        self.text[offset..]
            .find('\n')
            .map_or(self.text.len(), |index| offset + index)
    }
    fn line_column(&self, line_start: usize, offset: usize) -> usize {
        self.text[line_start..offset].graphemes(true).count()
    }
    fn offset_at_column(&self, line_start: usize, column: usize) -> usize {
        self.text[line_start..]
            .grapheme_indices(true)
            .take_while(|(_, grapheme)| *grapheme != "\n")
            .nth(column)
            .map_or_else(
                || self.line_end(line_start),
                |(index, _)| line_start + index,
            )
    }
    /// Caret to the start of the current visual line.
    pub fn home(&mut self, extend: bool) -> bool {
        self.place(self.line_start(self.caret), extend)
    }
    /// Caret to the end of the current visual line.
    pub fn end(&mut self, extend: bool) -> bool {
        self.place(self.line_end(self.caret), extend)
    }
    /// Caret to the start or end of the whole document.
    pub fn document_home(&mut self, extend: bool) -> bool {
        self.place(0, extend)
    }
    pub fn document_end(&mut self, extend: bool) -> bool {
        self.place(self.text.len(), extend)
    }
    /// Move one visual line while preserving the grapheme column where possible.
    pub fn move_vertical(&mut self, down: bool, extend: bool) -> bool {
        if !self.multiline {
            return false;
        }
        let start = self.line_start(self.caret);
        let column = self.line_column(start, self.caret);
        let target_start = if down {
            let end = self.line_end(self.caret);
            if end == self.text.len() {
                return false;
            }
            end + 1
        } else {
            if start == 0 {
                return false;
            }
            self.line_start(start - 1)
        };
        self.place(self.offset_at_column(target_start, column), extend)
    }
    pub fn select_all(&mut self) -> bool {
        let before = (self.anchor, self.caret);
        self.anchor = 0;
        self.caret = self.text.len();
        before != (self.anchor, self.caret)
    }
    /// Caret to a buffer offset (rounded down to a char boundary); `extend` keeps the anchor.
    /// 光标移到缓冲偏移（向下取到字符边界）；`extend` 时保留锚点。
    pub fn set_caret(&mut self, offset: usize, extend: bool) -> bool {
        let to = self.floor_boundary(offset.min(self.text.len()));
        self.place(to, extend)
    }
    /// Set or clear the composition. Text is sanitized; an IME cursor outside the preedit or off
    /// a char boundary is dropped. False when nothing changed.
    /// 设置或清除组合。文本经清理；超出预编辑或不在字符边界的输入法光标被丢弃。无变化时为 false。
    pub fn set_preedit(&mut self, preedit: Option<(&str, Option<(usize, usize)>)>) -> bool {
        let next = preedit.and_then(|(text, cursor)| {
            let text = sanitize(text);
            if text.is_empty() {
                return None;
            }
            let cursor = cursor.filter(|&(a, b)| {
                a <= b && b <= text.len() && text.is_char_boundary(a) && text.is_char_boundary(b)
            });
            Some(Preedit { text, cursor })
        });
        if next == self.preedit {
            return false;
        }
        self.preedit = next;
        true
    }
    /// Replace everything (external update): caret at the end, no selection, no composition.
    /// 整体替换（外部更新）：光标在末尾，无选区，无组合。
    pub fn replace_all(&mut self, text: &str) {
        *self = Self::new_with_mode(text, self.multiline);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_backspace_by_grapheme() {
        let mut b = EditBuffer::new("ab");
        assert_eq!((b.caret(), b.anchor()), (2, 2));
        assert!(b.insert("c\nd"));
        assert_eq!(b.text(), "abcd");
        assert_eq!(b.caret(), 4);
        // Family emoji (ZWJ sequence) and a combining mark delete as one cluster each.
        b.insert("e\u{301}👨‍👩‍👧");
        assert!(b.backspace(false));
        assert_eq!(b.text(), "abcde\u{301}");
        assert!(b.backspace(false));
        assert_eq!(b.text(), "abcd");
        b.home(false);
        assert!(!b.backspace(false));
        assert!(b.delete(false));
        assert_eq!(b.text(), "bcd");
        assert_eq!(b.caret(), 0);
    }

    #[test]
    fn multiline_mode_preserves_lf_and_navigates_visual_lines() {
        let mut b = EditBuffer::new_multiline("ab\r\ncde\rf\tg");
        assert!(b.is_multiline());
        assert_eq!(b.text(), "ab\ncde\nfg");
        assert_eq!(b.caret(), b.text().len());
        b.set_caret(4, false);
        assert!(b.move_vertical(false, false));
        assert_eq!(b.caret(), 1);
        assert!(b.move_vertical(true, false));
        assert_eq!(b.caret(), 4);
        assert!(b.end(false));
        assert_eq!(b.caret(), 6);
        assert!(b.home(false));
        assert_eq!(b.caret(), 3);
        assert!(b.insert("\nQ"));
        assert_eq!(b.text(), "ab\n\nQcde\nfg");
        b.replace_all("x\r\ny");
        assert_eq!(b.text(), "x\ny");
        assert!(b.is_multiline());
    }

    #[test]
    fn selection_movement_and_replacement() {
        let mut b = EditBuffer::new("hello world");
        b.move_left(true, true);
        assert_eq!(b.selected_text(), Some("world"));
        b.move_left(false, false);
        assert_eq!((b.caret(), b.has_selection()), (6, false));
        b.end(true);
        assert_eq!(b.selection(), (6, 11));
        assert!(b.insert("there"));
        assert_eq!(b.text(), "hello there");
        b.select_all();
        assert_eq!(b.selected_text(), Some("hello there"));
        b.move_right(false, false);
        assert_eq!(b.caret(), 11);
        b.home(false);
        b.move_right(false, true);
        assert_eq!(b.caret(), 5);
        b.move_right(false, true);
        assert_eq!(b.caret(), 11);
        assert!(b.backspace(true));
        assert_eq!(b.text(), "hello ");
        b.home(false);
        assert!(b.delete(true));
        assert_eq!(b.text(), " ");
    }

    #[test]
    fn utf8_offsets_stay_on_char_boundaries() {
        let mut b = EditBuffer::new("中文");
        assert_eq!(b.caret(), 6);
        b.move_left(false, false);
        assert_eq!(b.caret(), 3);
        assert!(!b.set_caret(4, false));
        assert_eq!(b.caret(), 3);
        b.set_caret(1, false);
        assert_eq!(b.caret(), 0);
    }

    #[test]
    fn preedit_display_and_offset_mapping() {
        let mut b = EditBuffer::new("ab");
        b.set_caret(1, false);
        assert!(b.set_preedit(Some(("にほ", Some((3, 3))))));
        let (text, caret, range) = b.display();
        assert_eq!(text, "aにほb");
        assert_eq!(caret, 4);
        assert_eq!(range, Some((1, 7)));
        assert_eq!(b.buffer_offset(0), 0);
        assert_eq!(b.buffer_offset(4), 1);
        assert_eq!(b.buffer_offset(8), 2);
        assert!(!b.set_preedit(Some(("にほ", Some((3, 3))))));
        // Bad IME cursor dropped; empty preedit clears.
        b.set_preedit(Some(("x", Some((0, 5)))));
        assert_eq!(b.preedit().unwrap().cursor, None);
        assert!(b.set_preedit(Some(("", None))));
        assert!(b.preedit().is_none());
        b.set_preedit(Some(("に", None)));
        assert!(b.insert("日"));
        assert_eq!(b.text(), "a日b");
        assert!(b.preedit().is_none());
    }

    #[test]
    fn replace_all_resets_caret_selection_and_composition() {
        let mut b = EditBuffer::new("abc");
        b.select_all();
        b.set_preedit(Some(("x", None)));
        b.replace_all("Q\tR");
        assert_eq!(b.text(), "QR");
        assert_eq!((b.anchor(), b.caret()), (2, 2));
        assert!(b.preedit().is_none());
    }
}
