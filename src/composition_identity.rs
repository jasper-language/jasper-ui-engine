//! UI preedit/candidate-view ownership; not a general IME engine or document store.
//! UI 预编辑/候选视图归属；不是通用输入法引擎或文档存储。
use crate::input_identity::{UiInputBinding, UiInputTicket};
use crate::input::{CompositionEvent, PreeditEvent, InputEvent};

// BEGIN VERIFIED UI COMPOSITION IDENTITY KERNEL
fn same_input_ticket(a: UiInputTicket, b: UiInputTicket) -> bool {
    a.key.session == b.key.session
        && a.key.slot == b.key.slot
        && a.key.generation == b.key.generation
        && a.context.target.0 == b.context.target.0
        && a.context.target_generation == b.context.target_generation
        && a.context.document == b.context.document
        && a.context.revision == b.context.revision
}

/// Public correlation value, not a secret capability or native engine handle.
/// 公开的关联值，不是秘密凭证或原生引擎句柄。
#[derive(Clone, Copy, Debug)]
pub struct UiCompositionToken {
    pub owner: UiInputTicket,
    pub serial: u64,
}

/// One UI composition view slot; retain it so serials never restart on cancellation.
/// 一个 UI 组合视图槽位；取消后必须保留该对象，避免重置序号令旧回调复活。
pub struct UiCompositionSlot {
    generation: u64,
    active: Option<UiCompositionToken>,
}

impl UiCompositionSlot {
    pub fn new() -> Self {
        Self { generation: 0, active: None }
    }

    /// Start only for an admitted active focus; overlapping starts and exhaustion reject unchanged.
    /// 仅为已准入活动焦点开始；重叠开始或代数耗尽保持原状态。
    pub fn begin(&mut self, binding: &UiInputBinding, caller: u64, owner: UiInputTicket) -> Option<UiCompositionToken> {
        if self.active.is_some() || self.generation == u64::MAX || !binding.accepts(caller, owner) {
            return None;
        }
        self.generation += 1;
        let token = UiCompositionToken { owner, serial: self.generation };
        self.active = Some(token);
        Some(token)
    }

    pub fn accepts(&self, binding: &UiInputBinding, caller: u64, token: UiCompositionToken) -> bool {
        match self.active {
            Some(active) => token.serial != 0
                && token.serial == self.generation
                && token.serial == active.serial
                && same_input_ticket(token.owner, active.owner)
                && binding.accepts(caller, token.owner),
            None => false,
        }
    }

    /// Clear the exact old UI composition even after focus closure; never clear a later serial.
    /// 即使焦点已关闭，也可清除精确旧 UI 组合；旧序号不能清除后续组合。
    /// caller must be host-authenticated. Native task cancellation/delivery is separate.
    /// caller 必须经宿主认证；本方法不投递或终止原生输入法任务。
    pub fn cancel(&mut self, caller: u64, token: UiCompositionToken) -> bool {
        match self.active {
            Some(active) => {
                if caller != token.owner.key.session || token.serial == 0
                    || token.serial != self.generation || token.serial != active.serial
                    || !same_input_ticket(token.owner, active.owner) {
                    return false;
                }
                self.active = None;
                true
            }
            None => false,
        }
    }

    /// Host shutdown returns the old fully qualified cancellation correlation and permanently closes.
    /// 宿主关闭时返回旧的完整取消关联并永久退役；返回不等于外部任务已确认取消。
    pub fn retire(&mut self) -> Option<UiCompositionToken> {
        let previous = self.active;
        self.active = None;
        self.generation = u64::MAX;
        previous
    }
}
// END VERIFIED UI COMPOSITION IDENTITY KERNEL

// BEGIN VERIFIED UI COMPOSITION RANGE KERNEL
/// Validate byte replacement offsets in the trusted current editor snapshot, not in preedit text.
/// 校验可信当前编辑器快照内的字节替换范围，不把它解释为预编辑文本的光标/选区。
pub fn valid_composition_range(document: &str, start: usize, end: usize) -> bool {
    start <= end && end <= document.as_bytes().len()
        && document.is_char_boundary(start) && document.is_char_boundary(end)
}

impl UiCompositionSlot {
    /// Admit the actual legacy composition payload only for its live serial and valid UTF-8 range.
    /// 仅为存活组合序号和有效 UTF-8 替换范围准入既有真实组合负载；不应用文档编辑。
    /// The host must pair document with the owner document/revision; all returned borrows lock state.
    /// 宿主须保证快照对应归属文档/版本；返回借用同时约束绑定、槽位和快照生命周期。
    pub fn admit_composition<'a>(
        &'a self,
        binding: &'a UiInputBinding,
        caller: u64,
        token: UiCompositionToken,
        document: &'a str,
        event: &'a CompositionEvent,
    ) -> Option<&'a CompositionEvent> {
        if self.accepts(binding, caller, token)
            && valid_composition_range(document, event.start, event.end) {
            Some(event)
        } else {
            None
        }
    }
}
// END VERIFIED UI COMPOSITION RANGE KERNEL


// BEGIN VERIFIED UI PREEDIT ADMISSION KERNEL
/// Validate preedit-local coordinates, preserving an absent cursor.
/// 校验预编辑内部坐标；无光标信息仍保持 None，不推断文档选区。
pub fn valid_preedit_cursor(text: &str, cursor: Option<(usize, usize)>) -> bool {
    match cursor {
        Some((start, end)) => valid_composition_range(text, start, end),
        None => true,
    }
}

impl UiCompositionSlot {
    /// Borrow a preedit payload only for the current owner/serial and valid local cursor.
    /// 仅为当前归属/序号及有效局部光标返回原预编辑负载借用；不修改文档。
    pub fn admit_preedit<'a>(
        &'a self,
        binding: &'a UiInputBinding,
        caller: u64,
        token: UiCompositionToken,
        event: &'a PreeditEvent,
    ) -> Option<&'a PreeditEvent> {
        if self.accepts(binding, caller, token)
            && valid_preedit_cursor(event.text.as_str(), event.cursor) {
            Some(event)
        } else {
            None
        }
    }
}
// END VERIFIED UI PREEDIT ADMISSION KERNEL


// BEGIN VERIFIED NORMALIZED PREEDIT ADMISSION KERNEL
impl UiCompositionSlot {
    /// Admit the normalized host Preedit variant without changing coordinate domains.
    /// 直接准入宿主规范化 Preedit，拒绝其他变体，不转换为文档替换坐标。
    pub fn admit_normalized_preedit<'a>(
        &'a self,
        binding: &'a UiInputBinding,
        caller: u64,
        token: UiCompositionToken,
        event: &'a InputEvent,
    ) -> Option<&'a InputEvent> {
        match event {
            InputEvent::Preedit { text, cursor } => {
                if self.accepts(binding, caller, token)
                    && valid_preedit_cursor(text.as_str(), *cursor) {
                    Some(event)
                } else { None }
            },
            _ => None,
        }
    }
}
// END VERIFIED NORMALIZED PREEDIT ADMISSION KERNEL

// BEGIN VERIFIED UI PREEDIT VIEW KERNEL
/// Owned UI presentation data, separate from committed editor text.
/// UI 自有呈现数据，与编辑器已提交正文分离。
pub struct UiPreeditFrame {
    pub token: UiCompositionToken,
    pub text: String,
    pub cursor: Option<(usize, usize)>,
}

/// A presentation slot whose updates and clearing retain exact composition ownership.
/// 更新和清除均保留精确组合归属的呈现槽位；不执行编辑事务或原生任务取消。
pub struct UiPreeditView {
    frame: Option<UiPreeditFrame>,
}

impl UiPreeditView {
    pub fn new() -> Self { Self { frame: None } }

    pub fn current(&self) -> Option<&UiPreeditFrame> { self.frame.as_ref() }

    /// Move an admitted normalized preedit into presentation state; rejection keeps old state.
    /// 将准入的规范化预编辑移入呈现状态；拒绝时保留旧状态，不更改编辑器正文。
    pub fn apply(&mut self, slot: &UiCompositionSlot, binding: &UiInputBinding, caller: u64, token: UiCompositionToken, event: InputEvent) -> bool {
        match event {
            InputEvent::Preedit { text, cursor } => {
                if !slot.accepts(binding, caller, token) || !valid_preedit_cursor(text.as_str(), cursor) {
                    return false;
                }
                self.frame = Some(UiPreeditFrame { token, text, cursor });
                true
            },
            _ => false,
        }
    }

    /// Clear only the matching old presentation, including after its input binding closed.
    /// 仅清除匹配的旧呈现，允许绑定关闭后清理；旧组合不能清除后继组合。
    pub fn clear(&mut self, caller: u64, token: UiCompositionToken) -> bool {
        match self.frame.as_ref() {
            Some(frame) => {
                if caller != token.owner.key.session || token.serial == 0
                    || frame.token.serial != token.serial || !same_input_ticket(frame.token.owner, token.owner) {
                    return false;
                }
                self.frame = None;
                true
            },
            None => false,
        }
    }
}
// END VERIFIED UI PREEDIT VIEW KERNEL

// BEGIN VERIFIED UI PREEDIT CANCEL KERNEL
impl UiCompositionSlot {
    /// Cancel the exact slot first, then clear only its matching owned UI text.
    /// 先取消精确槽位，再清除其匹配的 UI 自有文本；拒绝时两个对象均保持原样。
    /// Exclusive borrows cover this synchronous pair, not native task delivery or pixels.
    /// 独占借用约束此同步双对象操作，不代表原生任务投递或像素更新已完成。
    pub fn cancel_preedit(&mut self, view: &mut UiPreeditView, caller: u64, token: UiCompositionToken) -> bool {
        if !self.cancel(caller, token) {
            return false;
        }
        let _ = view.clear(caller, token);
        true
    }
}
// END VERIFIED UI PREEDIT CANCEL KERNEL

// BEGIN VERIFIED COMPOSITION CURRENT TOKEN KERNEL
impl UiCompositionSlot {
    /// Inspect correlation metadata; it is not independent input authorization.
    /// 检查关联元数据；返回值本身不构成输入授权。
    pub fn current_token(&self) -> Option<UiCompositionToken> { self.active }
}
// END VERIFIED COMPOSITION CURRENT TOKEN KERNEL
