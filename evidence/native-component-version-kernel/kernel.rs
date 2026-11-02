use vstd::prelude::*;
verus! {
impl<T> ViewSlot<T> { pub closed spec fn owned_value(&self) -> Option<T> { self.value } }
pub closed spec fn resource_model(gate: &ResourceGate) -> (u64, u64, u64, bool) {
    (gate.session, gate.slot, gate.generation, gate.live)
}
pub open spec fn resource_allowed(state: (u64,u64,u64,bool), caller: u64, key: ResourceKey) -> bool {
    state.3 && state.2 != 0 && caller == state.0 && key.session == state.0 && key.slot == state.1 && key.generation == state.2
}


pub closed spec fn binding_model(binding: &UiInputBinding) -> ((u64,u64,u64,bool), Option<UiInputContext>) {
    (resource_model(&binding.gate), binding.current)
}
pub open spec fn binding_allowed(state: ((u64,u64,u64,bool), Option<UiInputContext>), caller: u64, ticket: UiInputTicket) -> bool {
    match state.1 {
        Some(current) => resource_allowed(state.0, caller, ticket.key)
            && current.target.0 == ticket.context.target.0
            && current.target_generation == ticket.context.target_generation
            && current.document == ticket.context.document
            && current.revision == ticket.context.revision,
        None => false,
    }
}
pub closed spec fn composition_model(slot: &UiCompositionSlot) -> (u64, Option<UiCompositionToken>) {
    (slot.generation, slot.active)
}
pub open spec fn composition_active(state: (u64, Option<UiCompositionToken>), token: UiCompositionToken) -> bool {
    token.serial != 0 && token.serial == state.0 && state.1 == Some(token)
}
pub mod blitz_dom { #[verifier::external_body] pub struct NodeId {} }
#[verifier::external_body] pub struct BaseDocument {}
pub closed spec fn native_component_version_model(view: &NativeTextView) -> Option<u64> { if view.document.owned_value().is_some() { Some(view.component_version) } else { None } }
pub struct ViewSlot<T> {
    value: Option<T>,
}

impl<T> ViewSlot<T> {
    pub fn new(value: T) -> (result: Self)
    ensures result.owned_value() == Some(value),
{
        Self { value: Some(value) }
    }

    /// Retires the owned value before its destructor runs; never reopens the slot.
    /// This is logical ownership retirement, not an external cancellation ACK.
    pub fn close(&mut self) -> (result: bool)
    ensures final(self).owned_value().is_none(), result == old(self).owned_value().is_some(),
{
        self.value.take().is_some()
    }

    pub fn is_open(&self) -> (result: bool)
    ensures result == self.owned_value().is_some(),
{
        self.value.is_some()
    }
}

/// Wire-visible identity, not a secret capability or a backing-memory owner.
/// 可在线上传输的身份，不是秘密能力凭证，也不拥有底层内存。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceKey {
    pub session: u64,
    pub slot: u64,
    pub generation: u64,
}

/// One host-owned slot. Never duplicate a live gate for the same session/slot pair.
/// 一个宿主持有的槽位；同一会话/槽位不得复制或重建并行存活的 gate。
/// The host supplies a trusted caller session; client-supplied identity is insufficient.
/// caller_session 必须来自宿主认证上下文，不能直接信任客户端自报值。
pub struct ResourceGate {
    session: u64,
    slot: u64,
    generation: u64,
    live: bool,
}

impl ResourceGate {
    pub fn new(session: u64, slot: u64) -> (result: Self)
    ensures resource_model(&result) == (session, slot, 0u64, false),
{ reveal(resource_model);
        Self {
            session,
            slot,
            generation: 0,
            live: false,
        }
    }

    /// Issues the next generation only to the owning session and only while vacant.
    /// 仅在槽位空闲时向所属会话签发下一代；代数耗尽永久拒绝，不回绕。
    pub fn issue(&mut self, caller_session: u64) -> (result: Option<ResourceKey>)
    ensures match result {
        Some(key) => caller_session == resource_model(old(self)).0 && !resource_model(old(self)).3
            && resource_model(old(self)).2 < u64::MAX
            && resource_model(final(self)).0 == resource_model(old(self)).0
            && resource_model(final(self)).1 == resource_model(old(self)).1
            && resource_model(final(self)).2 as int == resource_model(old(self)).2 as int + 1
            && resource_model(final(self)).3 && resource_model(final(self)).2 > 0
            && key.session == resource_model(final(self)).0 && key.slot == resource_model(final(self)).1
            && key.generation == resource_model(final(self)).2,
        None => resource_model(final(self)) == resource_model(old(self))
            && (caller_session != resource_model(old(self)).0 || resource_model(old(self)).3 || resource_model(old(self)).2 == u64::MAX),
    },
{ reveal(resource_model);
        if caller_session != self.session || self.live || self.generation == u64::MAX {
            return None;
        }
        self.generation += 1;
        self.live = true;
        Some(ResourceKey {
            session: self.session,
            slot: self.slot,
            generation: self.generation,
        })
    }

    pub fn allows(&self, caller_session: u64, key: ResourceKey) -> (result: bool)
    ensures result == resource_allowed(resource_model(self), caller_session, key),
{ reveal(resource_model);
        self.live
            && self.generation != 0
            && caller_session == self.session
            && key.session == self.session
            && key.slot == self.slot
            && key.generation == self.generation
    }

    /// Revocation is idempotent rejection after the first success; it does not free storage.
    /// 首次撤销成功后重复调用拒绝；此方法不释放实际存储，也不取消在途操作。
    pub fn revoke(&mut self, caller_session: u64, key: ResourceKey) -> (result: bool)
    ensures result == resource_allowed(resource_model(old(self)), caller_session, key),
        resource_model(final(self)).0 == resource_model(old(self)).0,
        resource_model(final(self)).1 == resource_model(old(self)).1,
        resource_model(final(self)).2 == resource_model(old(self)).2,
        resource_model(final(self)).3 == (resource_model(old(self)).3 && !result),
{ reveal(resource_model);
        if !self.allows(caller_session, key) {
            return false;
        }
        self.live = false;
        true
    }
    /// Host shutdown permanently retires this gate; clients must not control this call.
    /// 宿主关闭时永久退役此 gate；不得把该管理调用交给未认证客户端。
    pub fn retire(&mut self)
    ensures resource_model(final(self)) == (resource_model(old(self)).0, resource_model(old(self)).1, u64::MAX, false),
{ reveal(resource_model);
        self.generation = u64::MAX;
        self.live = false;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u64);

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

) -> (result: bool)
    ensures result == (resource_allowed(resource_model(gate), caller_session, ticket.key)
        && current.target.0 == ticket.context.target.0
        && current.target_generation == ticket.context.target_generation
        && current.document == ticket.context.document
        && current.revision == ticket.context.revision),
{
    gate.allows(caller_session, ticket.key)
        && current.target.0 == ticket.context.target.0
        && current.target_generation == ticket.context.target_generation
        && current.document == ticket.context.document
        && current.revision == ticket.context.revision
}

/// Persistent single-focus UI binding; never clone/recreate a live session/slot gate.
/// 持久的单焦点 UI 绑定；同一会话/槽位的 gate 不得克隆或在旧票据存活时重建。
pub struct UiInputBinding {
    gate: ResourceGate,
    current: Option<UiInputContext>,
}

impl UiInputBinding {
    pub fn new(session: u64, slot: u64) -> (result: Self)
    ensures binding_model(&result) == ((session, slot, 0u64, false), None),
{ reveal(binding_model);
        Self { gate: ResourceGate::new(session, slot), current: None }
    }

    /// Bind a vacant focus slot. Rejection leaves the previous binding intact.
    /// 仅绑定空闲焦点槽位；拒绝时保留原绑定，返回票据后才能发布给消费者。
    pub fn bind(&mut self, caller_session: u64, current: UiInputContext) -> (result: Option<UiInputTicket>)
    ensures match result {
        Some(ticket) => caller_session == binding_model(old(self)).0.0
            && !binding_model(old(self)).0.3 && binding_model(old(self)).0.2 < u64::MAX
            && binding_model(final(self)).0.0 == binding_model(old(self)).0.0
            && binding_model(final(self)).0.1 == binding_model(old(self)).0.1
            && binding_model(final(self)).0.2 as int == binding_model(old(self)).0.2 as int + 1
            && binding_model(final(self)).0.3 && binding_model(final(self)).1 == Some(current)
            && ticket.context == current && resource_allowed(binding_model(final(self)).0, caller_session, ticket.key),
        None => binding_model(final(self)) == binding_model(old(self))
            && (caller_session != binding_model(old(self)).0.0 || binding_model(old(self)).0.3 || binding_model(old(self)).0.2 == u64::MAX),
    },
{ reveal(binding_model);
        match self.gate.issue(caller_session) {
            Some(key) => {
                self.current = Some(current);
                Some(UiInputTicket { key, context: current })
            }
            None => None,
        }
    }

    pub fn accepts(&self, caller_session: u64, ticket: UiInputTicket) -> (result: bool)
    ensures result == binding_allowed(binding_model(self), caller_session, ticket),
{ reveal(binding_model);
        match self.current {
            Some(current) => accepts_ui_input(&self.gate, caller_session, current, ticket),
            None => false,
        }
    }

    /// Close only the exact active binding; stale callbacks cannot close its successor.
    /// 仅关闭精确匹配的当前绑定；旧回调不能关闭后继绑定，不负责外部任务取消。
    pub fn close(&mut self, caller_session: u64, ticket: UiInputTicket) -> (result: bool)
    ensures result == binding_allowed(binding_model(old(self)), caller_session, ticket),
        !result ==> binding_model(final(self)) == binding_model(old(self)),
        result ==> binding_model(final(self)) == ((binding_model(old(self)).0.0, binding_model(old(self)).0.1, binding_model(old(self)).0.2, false), None),
{ reveal(binding_model);
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
    pub fn retire(&mut self)
    ensures binding_model(final(self)) == ((binding_model(old(self)).0.0, binding_model(old(self)).0.1, u64::MAX, false), None),
{ reveal(binding_model);
        self.gate.retire();
        self.current = None;
    }
}

fn same_input_ticket(a: UiInputTicket, b: UiInputTicket) -> (result: bool)
    ensures result == (a == b),
{
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
    pub fn new() -> (result: Self)
    ensures composition_model(&result) == (0u64, None),
{ reveal(composition_model);
        Self { generation: 0, active: None }
    }

    /// Start only for an admitted active focus; overlapping starts and exhaustion reject unchanged.
    /// 仅为已准入活动焦点开始；重叠开始或代数耗尽保持原状态。
    pub fn begin(&mut self, binding: &UiInputBinding, caller: u64, owner: UiInputTicket) -> (result: Option<UiCompositionToken>)
    ensures match result {
        Some(token) => composition_model(old(self)).1 == None::<UiCompositionToken>
            && composition_model(old(self)).0 < u64::MAX
            && binding_allowed(binding_model(binding), caller, owner)
            && composition_model(final(self)).0 as int == composition_model(old(self)).0 as int + 1
            && token.owner == owner && token.serial == composition_model(final(self)).0
            && composition_model(final(self)).1 == Some(token),
        None => composition_model(final(self)) == composition_model(old(self))
            && (composition_model(old(self)).1.is_some() || composition_model(old(self)).0 == u64::MAX || !binding_allowed(binding_model(binding), caller, owner)),
    },
{ reveal(composition_model);
        if self.active.is_some() || self.generation == u64::MAX || !binding.accepts(caller, owner) {
            return None;
        }
        self.generation += 1;
        let token = UiCompositionToken { owner, serial: self.generation };
        self.active = Some(token);
        Some(token)
    }

    pub fn accepts(&self, binding: &UiInputBinding, caller: u64, token: UiCompositionToken) -> (result: bool)
    ensures result == (composition_active(composition_model(self), token) && binding_allowed(binding_model(binding), caller, token.owner)),
{ reveal(composition_model);
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
    pub fn cancel(&mut self, caller: u64, token: UiCompositionToken) -> (result: bool)
    ensures result == (caller == token.owner.key.session && composition_active(composition_model(old(self)), token)),
        composition_model(final(self)).0 == composition_model(old(self)).0,
        result ==> composition_model(final(self)).1 == None::<UiCompositionToken>,
        !result ==> composition_model(final(self)) == composition_model(old(self)),
{ reveal(composition_model);
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
    pub fn retire(&mut self) -> (result: Option<UiCompositionToken>)
    ensures result == composition_model(old(self)).1,
        composition_model(final(self)) == (u64::MAX, None),
{ reveal(composition_model);
        let previous = self.active;
        self.active = None;
        self.generation = u64::MAX;
        previous
    }
}

pub(crate) struct NativeTextView {
    bold_aliases:std::collections::BTreeSet<u64>,
    document: ViewSlot<BaseDocument>,
    preedit: UiCompositionSlot,
    component_version: u64,
    session: u64,
    context: UiInputContext,
    content_node: blitz_dom::NodeId,
    text_node: blitz_dom::NodeId,
    text_element: blitz_dom::NodeId,
    preedit_node: blitz_dom::NodeId,
    preedit_element: blitz_dom::NodeId,
}

impl NativeTextView {
    /// Independent UI update sequence, not the external text document revision.
    pub(crate) fn component_version(&self) -> (result: Option<u64>)
        ensures result == native_component_version_model(self),
    { proof { reveal(native_component_version_model); }
        if self.document.is_open() { Some(self.component_version) } else { None }
    }
}

}
