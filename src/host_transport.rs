//! Bounded, host-neutral inbox for an embedded system UI service.
//! 嵌入式系统 UI 服务使用的有界、宿主中立的收件队列。
//!
//! This module defines ordering and backpressure only. It performs no IPC, allocation policy,
//! authentication or platform calls. A desktop, mobile, or jasper-os host can carry `UiBatch`
//! bytes, scene updates or normalized input through the same rules.
//! 本模块只定义顺序与背压，不执行 IPC、认证或平台调用。桌面、移动或 jasper-os 宿主都可以用
//! 同一规则承载 `UiBatch`、场景更新或规范化输入。

use std::collections::VecDeque;

/// Authenticated host session and reconnect epoch. The transport must obtain both from the host.
/// 已认证的宿主会话与重连代数；传输层必须从宿主取得这两个值，不能信任消息载荷。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostSessionId {
    pub session: u64,
    pub epoch: u64,
}

/// Rejection that returns the payload so the sender can report or retry without data loss.
/// 拒绝时返还载荷，使发送方可报告或重试而不丢数据。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostInboxError<T> {
    WrongSession(T),
    Closed(T),
    SequenceGap {
        expected: u64,
        actual: u64,
        payload: T,
    },
    SequenceExhausted(T),
    Full(T),
}

/// One bounded ordered inbox for exactly one host session lineage.
/// 恰好服务一条宿主会话 lineage 的有界有序收件队列。
#[derive(Clone, Debug)]
pub struct HostInbox<T> {
    session: HostSessionId,
    expected_sequence: u64,
    capacity: usize,
    messages: VecDeque<T>,
    closed: bool,
}

impl<T> HostInbox<T> {
    /// Create an inbox with a positive capacity and exact first inbound sequence.
    /// 使用正容量和精确的首个入站序号创建收件队列。
    pub fn new(session: HostSessionId, first_sequence: u64, capacity: usize) -> Option<Self> {
        if capacity == 0 {
            return None;
        }
        Some(Self {
            session,
            expected_sequence: first_sequence,
            capacity,
            messages: VecDeque::with_capacity(capacity),
            closed: false,
        })
    }

    pub fn session(&self) -> HostSessionId {
        self.session
    }

    pub fn expected_sequence(&self) -> u64 {
        self.expected_sequence
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Admit exactly the next sequence when capacity remains. All failures preserve inbox state.
    /// 仅当序号精确后继且仍有容量时准入；所有失败都保持队列状态不变。
    pub fn enqueue(
        &mut self,
        session: HostSessionId,
        sequence: u64,
        payload: T,
    ) -> Result<(), HostInboxError<T>> {
        if session != self.session {
            return Err(HostInboxError::WrongSession(payload));
        }
        if self.closed {
            return Err(HostInboxError::Closed(payload));
        }
        if sequence != self.expected_sequence {
            return Err(HostInboxError::SequenceGap {
                expected: self.expected_sequence,
                actual: sequence,
                payload,
            });
        }
        let Some(next) = self.expected_sequence.checked_add(1) else {
            // Do not admit a sequence that cannot produce a successor; the host must reconnect.
            // 不接纳无法产生后继的序号；宿主必须重新连接。
            return Err(HostInboxError::SequenceExhausted(payload));
        };
        if self.messages.len() == self.capacity {
            return Err(HostInboxError::Full(payload));
        }
        self.messages.push_back(payload);
        self.expected_sequence = next;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<T> {
        self.messages.pop_front()
    }

    /// Close the inbox and return undelivered payloads for host diagnostics.
    /// 关闭收件队列并返还未交付载荷，供宿主诊断。
    pub fn close(&mut self) -> Vec<T> {
        self.closed = true;
        self.messages.drain(..).collect()
    }

    /// Start a new authenticated epoch and return all messages from the stale epoch.
    /// 开始新的已认证 epoch，并返回旧 epoch 的全部消息。
    pub fn replace_session(&mut self, session: HostSessionId, first_sequence: u64) -> Vec<T> {
        self.session = session;
        self.expected_sequence = first_sequence;
        self.closed = false;
        self.messages.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(epoch: u64) -> HostSessionId {
        HostSessionId { session: 10, epoch }
    }

    #[test]
    fn exact_sequence_and_bounded_backpressure_preserve_errors() {
        let mut inbox = HostInbox::new(session(1), 77, 2).unwrap();
        inbox.enqueue(session(1), 77, "a").unwrap();
        assert_eq!(
            inbox.enqueue(session(1), 79, "gap"),
            Err(HostInboxError::SequenceGap {
                expected: 78,
                actual: 79,
                payload: "gap"
            })
        );
        inbox.enqueue(session(1), 78, "b").unwrap();
        assert_eq!(
            inbox.enqueue(session(1), 79, "full"),
            Err(HostInboxError::Full("full"))
        );
        assert_eq!(inbox.len(), 2);
        assert_eq!(inbox.pop(), Some("a"));
        inbox.enqueue(session(1), 79, "c").unwrap();
        assert_eq!(inbox.pop(), Some("b"));
        assert_eq!(inbox.pop(), Some("c"));
    }

    #[test]
    fn wrong_session_close_and_epoch_replacement_are_explicit() {
        let mut inbox = HostInbox::new(session(1), 1, 2).unwrap();
        inbox.enqueue(session(1), 1, "old").unwrap();
        assert_eq!(
            inbox.enqueue(
                HostSessionId {
                    session: 11,
                    epoch: 1
                },
                2,
                "foreign"
            ),
            Err(HostInboxError::WrongSession("foreign"))
        );
        assert_eq!(
            inbox.replace_session(session(2), 20),
            vec!["old"],
            "reconnect drops only the stale epoch payloads"
        );
        assert_eq!(inbox.session(), session(2));
        assert_eq!(inbox.expected_sequence(), 20);
        assert_eq!(inbox.close(), Vec::<&str>::new());
        assert_eq!(
            inbox.enqueue(session(2), 20, "late"),
            Err(HostInboxError::Closed("late"))
        );
    }

    #[test]
    fn zero_capacity_and_sequence_exhaustion_are_rejected_without_mutation() {
        assert!(HostInbox::<()>::new(session(1), 0, 0).is_none());
        let mut inbox = HostInbox::new(session(1), u64::MAX, 1).unwrap();
        assert_eq!(
            inbox.enqueue(session(1), u64::MAX, 7),
            Err(HostInboxError::SequenceExhausted(7))
        );
        assert!(inbox.is_empty());
        assert_eq!(inbox.expected_sequence(), u64::MAX);
    }
}
