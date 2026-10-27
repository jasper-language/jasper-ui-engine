//! Host-neutral UI resource admission; storage and authenticated sessions belong to the host.
//! 宿主无关的 UI 资源准入；存储与已认证会话由宿主提供。

// BEGIN VERIFIED RESOURCE GATE KERNEL
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
    pub fn new(session: u64, slot: u64) -> Self {
        Self {
            session,
            slot,
            generation: 0,
            live: false,
        }
    }

    /// Issues the next generation only to the owning session and only while vacant.
    /// 仅在槽位空闲时向所属会话签发下一代；代数耗尽永久拒绝，不回绕。
    pub fn issue(&mut self, caller_session: u64) -> Option<ResourceKey> {
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

    pub fn allows(&self, caller_session: u64, key: ResourceKey) -> bool {
        self.live
            && self.generation != 0
            && caller_session == self.session
            && key.session == self.session
            && key.slot == self.slot
            && key.generation == self.generation
    }

    /// Revocation is idempotent rejection after the first success; it does not free storage.
    /// 首次撤销成功后重复调用拒绝；此方法不释放实际存储，也不取消在途操作。
    pub fn revoke(&mut self, caller_session: u64, key: ResourceKey) -> bool {
        if !self.allows(caller_session, key) {
            return false;
        }
        self.live = false;
        true
    }
    /// Host shutdown permanently retires this gate; clients must not control this call.
    /// 宿主关闭时永久退役此 gate；不得把该管理调用交给未认证客户端。
    pub fn retire(&mut self) {
        self.generation = u64::MAX;
        self.live = false;
    }
}
// END VERIFIED RESOURCE GATE KERNEL

// BEGIN VERIFIED BYTES SLOT KERNEL
/// Owned UI byte storage behind a persistent session/slot gate.
/// 在持久会话/槽位 gate 后持有 UI 字节存储；可供后续字体或其他字节资源服务使用。
/// Installation moves a Vec without copying; release returns ownership to the host.
/// 安装仅转移 Vec，不复制；释放将所有权返还宿主，不操作内核句柄。
pub struct UiBytesSlot {
    gate: ResourceGate,
    payload: Option<Vec<u8>>,
}

impl UiBytesSlot {
    pub fn new(session: u64, slot: u64) -> Self {
        Self {
            gate: ResourceGate::new(session, slot),
            payload: None,
        }
    }

    pub fn install(&mut self, caller_session: u64, bytes: Vec<u8>) -> Result<ResourceKey, Vec<u8>> {
        match self.gate.issue(caller_session) {
            None => Err(bytes),
            Some(key) => {
                self.payload = Some(bytes);
                Ok(key)
            }
        }
    }

    pub fn bytes(&self, caller_session: u64, key: ResourceKey) -> Option<&[u8]> {
        if !self.gate.allows(caller_session, key) {
            return None;
        }
        match self.payload.as_ref() {
            Some(bytes) => Some(bytes.as_slice()),
            None => None,
        }
    }

    /// Rust prevents releasing through &mut while a returned slice remains borrowed.
    /// 返回切片仍被借用时，Rust 禁止经 &mut 释放；不等同外部 DMA/IPC 在途取消。
    pub fn release(&mut self, caller_session: u64, key: ResourceKey) -> Option<Vec<u8>> {
        if self.gate.revoke(caller_session, key) {
            self.payload.take()
        } else {
            None
        }
    }
    /// Host shutdown returns any owned payload and permanently disables installation.
    /// 宿主关闭时取回已有载荷，并永久禁止再次安装；不取消外部在途请求。
    pub fn retire(&mut self) -> Option<Vec<u8>> {
        self.gate.retire();
        self.payload.take()
    }
}
// END VERIFIED BYTES SLOT KERNEL
