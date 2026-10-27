//! UI-side identity admission; authenticated context and gate lifetime belong to the host.
//! UI 侧身份准入；认证上下文、当前目标与 gate 唯一生命周期由宿主维护。
use crate::{
    resource::{ResourceGate, ResourceKey},
    scene::NodeId,
};

// BEGIN VERIFIED UI INPUT ADMISSION KERNEL
/// Trusted current UI binding, independent of third-party editor/IME private types.
/// 可信的当前 UI 绑定，不暴露第三方编辑器或输入法私有类型。
#[derive(Clone, Copy, Debug)]
pub struct UiInputContext {
    pub target: NodeId,
    pub target_generation: u64,
    pub document: u64,
    pub revision: u64,
}

/// Identity carried by delayed input; it is not an authentication credential.
/// 延迟输入携带的身份，不是认证凭证；字段可被客户端伪造，不能代替宿主认证。
#[derive(Clone, Copy, Debug)]
pub struct UiInputTicket {
    pub key: ResourceKey,
    pub context: UiInputContext,
}

/// Admit only the live session/focus generation and exact current target/document revision.
/// 仅允许存活会话/焦点代数和精确匹配的当前目标/文档版本进入 UI 消费者。
/// The host must revoke/retire the gate on focus/session closure before publishing a new binding.
/// 宿主必须在关闭焦点/会话时先撤销或退役 gate，再发布新绑定；本函数不取消外部任务。
pub fn accepts_ui_input(
    gate: &ResourceGate,
    caller_session: u64,
    current: UiInputContext,
    ticket: UiInputTicket,
) -> bool {
    gate.allows(caller_session, ticket.key)
        && current.target.0 == ticket.context.target.0
        && current.target_generation == ticket.context.target_generation
        && current.document == ticket.context.document
        && current.revision == ticket.context.revision
}
// END VERIFIED UI INPUT ADMISSION KERNEL

// BEGIN VERIFIED UI INPUT BINDING KERNEL
/// Persistent single-focus UI binding; never clone/recreate a live session/slot gate.
/// 持久的单焦点 UI 绑定；同一会话/槽位的 gate 不得克隆或在旧票据存活时重建。
pub struct UiInputBinding {
    gate: ResourceGate,
    current: Option<UiInputContext>,
}

impl UiInputBinding {
    pub fn new(session: u64, slot: u64) -> Self {
        Self { gate: ResourceGate::new(session, slot), current: None }
    }

    /// Bind a vacant focus slot. Rejection leaves the previous binding intact.
    /// 仅绑定空闲焦点槽位；拒绝时保留原绑定，返回票据后才能发布给消费者。
    pub fn bind(&mut self, caller_session: u64, current: UiInputContext) -> Option<UiInputTicket> {
        match self.gate.issue(caller_session) {
            Some(key) => {
                self.current = Some(current);
                Some(UiInputTicket { key, context: current })
            }
            None => None,
        }
    }

    pub fn accepts(&self, caller_session: u64, ticket: UiInputTicket) -> bool {
        match self.current {
            Some(current) => accepts_ui_input(&self.gate, caller_session, current, ticket),
            None => false,
        }
    }

    /// Close only the exact active binding; stale callbacks cannot close its successor.
    /// 仅关闭精确匹配的当前绑定；旧回调不能关闭后继绑定，不负责外部任务取消。
    pub fn close(&mut self, caller_session: u64, ticket: UiInputTicket) -> bool {
        if !self.accepts(caller_session, ticket) {
            return false;
        }
        if self.gate.revoke(caller_session, ticket.key) {
            self.current = None;
            true
        } else {
            false
        }
    }

    /// Host-only permanent shutdown, independent of client-provided ticket values.
    /// 仅宿主可调用的永久退役；不依赖客户端提供的票据，不代表外部任务已终止。
    pub fn retire(&mut self) {
        self.gate.retire();
        self.current = None;
    }
}
// END VERIFIED UI INPUT BINDING KERNEL

// BEGIN VERIFIED UI INPUT PAYLOAD KERNEL
/// Borrowed payload admitted for one UI target; fields cannot be forged by clients.
/// 已向单个 UI 目标准入的借用负载；字段私有，调用方不能直接伪造。
/// Its lifetime borrows the binding, preventing safe Rust close/rebind during consumption.
/// 生命周期借用绑定，安全 Rust 中消费期间不能关闭或重新绑定；不授权异步逃逸。
pub struct AdmittedUiInput<'a, T> {
    payload: &'a T,
    target: NodeId,
}

impl<'a, T> AdmittedUiInput<'a, T> {
    pub fn payload(&self) -> &'a T {
        self.payload
    }

    pub fn target(&self) -> NodeId {
        self.target
    }
}

impl UiInputBinding {
    /// A single admission bridge for normalized input or host-owned input payloads.
    /// 统一的规范化输入/宿主输入负载准入桥；拒绝不暴露负载，不调用消费者。
    pub fn admit<'a, T>(
        &'a self,
        caller_session: u64,
        ticket: UiInputTicket,
        payload: &'a T,
    ) -> Option<AdmittedUiInput<'a, T>> {
        if self.accepts(caller_session, ticket) {
            Some(AdmittedUiInput { payload, target: ticket.context.target })
        } else {
            None
        }
    }
}
// END VERIFIED UI INPUT PAYLOAD KERNEL

// BEGIN VERIFIED UI INPUT REPLACE KERNEL
impl UiInputBinding {
    /// Atomically rotate an active binding under exclusive access; failed replacement is unchanged.
    /// 独占访问下轮换活动绑定；失败不改变原绑定，代数耗尽不先关闭旧绑定。
    /// Host supplies the next trusted target/document context; this does not move keyboard focus.
    /// 下一个可信目标/文档上下文由宿主提供；此方法自身不移动键盘焦点。
    pub fn replace(
        &mut self,
        caller_session: u64,
        ticket: UiInputTicket,
        next: UiInputContext,
    ) -> Option<UiInputTicket> {
        if ticket.key.generation == u64::MAX || !self.accepts(caller_session, ticket) {
            return None;
        }
        if !self.close(caller_session, ticket) {
            return None;
        }
        self.bind(caller_session, next)
    }
}
// END VERIFIED UI INPUT REPLACE KERNEL

// BEGIN VERIFIED UI VIEW OWNER KERNEL
/// Match a host-bound view instance; caller identity still requires host authentication.
/// This is a consumer guard, not an input-service protocol or authentication grant.
pub fn matches_ui_view_owner(
    session: u64, current: UiInputContext, caller: u64, incoming: UiInputContext,
) -> bool {
    session == caller
        && current.target.0 == incoming.target.0
        && current.target_generation == incoming.target_generation
        && current.document == incoming.document
        && current.revision == incoming.revision
}
// END VERIFIED UI VIEW OWNER KERNEL

// BEGIN VERIFIED UI SNAPSHOT ADMISSION KERNEL
/// Consumer compare-before-update gate; revisions remain opaque host-owned identities.
pub fn accepts_ui_display_snapshot(
    session: u64, current: UiInputContext, caller: u64,
    expected: UiInputContext, next: UiInputContext,
) -> bool {
    matches_ui_view_owner(session, current, caller, expected)
        && next.target.0 == expected.target.0
        && next.target_generation == expected.target_generation
        && next.document == expected.document
}
// END VERIFIED UI SNAPSHOT ADMISSION KERNEL

// BEGIN VERIFIED UI UPDATE NEXT KERNEL
/// Prepare a next UI update version without mutating or reserving backend state.
pub fn next_ui_update_version(current: u64, expected: u64) -> Option<u64> {
    if current != expected || current == u64::MAX { return None; }
    Some(current + 1)
}
// END VERIFIED UI UPDATE NEXT KERNEL
