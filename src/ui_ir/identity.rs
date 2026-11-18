//! Reference generation ledger for Framework-assigned `UiKey` values.
//!
//! This ledger is deliberately separate from every backend `NodeId`. It never reuses a
//! `(semantic_id, generation)` pair and refuses exhaustion instead of wrapping.

use super::UiKey;
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Debug)]
struct Record {
    last_generation: u64,
    live: bool,
}

/// Rust-side identity allocator for one stable identity domain.
#[derive(Clone, Debug)]
pub struct UiIdentityLedger {
    identity_domain: String,
    records: BTreeMap<u64, Record>,
}

impl UiIdentityLedger {
    pub fn new(identity_domain: impl Into<String>) -> Result<Self, IdentityError> {
        let identity_domain = identity_domain.into();
        if identity_domain.is_empty() {
            return Err(IdentityError::InvalidDomain);
        }
        Ok(Self {
            identity_domain,
            records: BTreeMap::new(),
        })
    }

    pub fn identity_domain(&self) -> &str {
        &self.identity_domain
    }

    /// Allocate the first generation, or the next one after a retired generation.
    pub fn allocate(&mut self, semantic_id: u64) -> Result<UiKey, IdentityError> {
        let record = self.records.entry(semantic_id).or_insert(Record {
            last_generation: 0,
            live: false,
        });
        if record.live {
            return Err(IdentityError::AlreadyLive(semantic_id));
        }
        let Some(generation) = record.last_generation.checked_add(1) else {
            return Err(IdentityError::GenerationExhausted(semantic_id));
        };
        record.last_generation = generation;
        record.live = true;
        Ok(UiKey::new(semantic_id, generation))
    }

    /// Claim one exact Framework-assigned key. The generation must be the next generation for
    /// this semantic identity; a retired pair or skipped generation is rejected.
    pub fn claim(&mut self, key: UiKey) -> Result<(), IdentityError> {
        let record = self.records.entry(key.semantic_id).or_insert(Record {
            last_generation: 0,
            live: false,
        });
        if record.live {
            return Err(IdentityError::AlreadyLive(key.semantic_id));
        }
        let Some(generation) = record.last_generation.checked_add(1) else {
            return Err(IdentityError::GenerationExhausted(key.semantic_id));
        };
        let expected = UiKey::new(key.semantic_id, generation);
        if key != expected {
            return Err(IdentityError::UnexpectedGeneration { expected, actual: key });
        }
        record.last_generation = generation;
        record.live = true;
        Ok(())
    }

    /// Retire the exact live identity. A stale key cannot retire its successor.
    pub fn retire(&mut self, key: UiKey) -> Result<(), IdentityError> {
        let Some(record) = self.records.get_mut(&key.semantic_id) else {
            return Err(IdentityError::Unknown(key));
        };
        if !record.live || record.last_generation != key.generation {
            return Err(IdentityError::Stale(key));
        }
        record.live = false;
        Ok(())
    }

    pub fn admit(&self, key: UiKey) -> Result<(), IdentityError> {
        match self.records.get(&key.semantic_id) {
            Some(record) if record.live && record.last_generation == key.generation => Ok(()),
            Some(record) if record.live && record.last_generation > key.generation => {
                Err(IdentityError::Stale(key))
            }
            Some(_) => Err(IdentityError::Retired(key)),
            None => Err(IdentityError::Unknown(key)),
        }
    }

    pub fn current(&self, semantic_id: u64) -> Option<UiKey> {
        let record = self.records.get(&semantic_id)?;
        record
            .live
            .then_some(UiKey::new(semantic_id, record.last_generation))
    }

    #[cfg(test)]
    fn force_retired_cursor(&mut self, semantic_id: u64, generation: u64) {
        self.records.insert(
            semantic_id,
            Record {
                last_generation: generation,
                live: false,
            },
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityError {
    InvalidDomain,
    AlreadyLive(u64),
    GenerationExhausted(u64),
    Unknown(UiKey),
    Stale(UiKey),
    Retired(UiKey),
    UnexpectedGeneration { expected: UiKey, actual: UiKey },
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDomain => write!(f, "identity domain must not be empty"),
            Self::AlreadyLive(id) => {
                write!(f, "semantic identity {id} is already live")
            }
            Self::GenerationExhausted(id) => {
                write!(f, "semantic identity {id} exhausted its generation space")
            }
            Self::Unknown(key) => write!(
                f,
                "identity {}:{} is unknown in this domain",
                key.semantic_id, key.generation
            ),
            Self::Stale(key) => write!(
                f,
                "identity {}:{} was superseded",
                key.semantic_id, key.generation
            ),
            Self::Retired(key) => write!(
                f,
                "identity {}:{} is retired",
                key.semantic_id, key.generation
            ),
            Self::UnexpectedGeneration { expected, actual } => write!(
                f,
                "identity {}:{} is not the next generation {}:{}",
                actual.semantic_id, actual.generation, expected.semantic_id, expected.generation
            ),
        }
    }
}

impl std::error::Error for IdentityError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_generations_are_never_reused() {
        let mut ledger = UiIdentityLedger::new("orders-app/window-main").unwrap();
        let first = ledger.allocate(7).unwrap();
        assert_eq!(first, UiKey::new(7, 1));
        assert_eq!(ledger.allocate(7), Err(IdentityError::AlreadyLive(7)));
        ledger.retire(first).unwrap();
        assert_eq!(ledger.admit(first), Err(IdentityError::Retired(first)));
        let second = ledger.allocate(7).unwrap();
        assert_eq!(second, UiKey::new(7, 2));
        assert_eq!(ledger.admit(first), Err(IdentityError::Stale(first)));
        assert_eq!(ledger.admit(second), Ok(()));
    }

    #[test]
    fn empty_identity_domain_is_rejected() {
        assert!(matches!(
            UiIdentityLedger::new(""),
            Err(IdentityError::InvalidDomain)
        ));
    }

    #[test]
    fn exhausted_generation_is_rejected_not_wrapped() {
        let mut ledger = UiIdentityLedger::new("orders-app/window-main").unwrap();
        ledger.force_retired_cursor(7, u64::MAX);
        assert_eq!(
            ledger.allocate(7),
            Err(IdentityError::GenerationExhausted(7))
        );
    }

    #[test]
    fn stale_identity_cannot_retire_its_successor() {
        let mut ledger = UiIdentityLedger::new("orders-app/window-main").unwrap();
        let first = ledger.allocate(7).unwrap();
        ledger.retire(first).unwrap();
        let second = ledger.allocate(7).unwrap();
        assert_eq!(ledger.retire(first), Err(IdentityError::Stale(first)));
        assert_eq!(ledger.admit(second), Ok(()));
    }

    #[test]
    fn exact_claim_rejects_skips_and_never_reuses_a_retired_pair() {
        let mut ledger = UiIdentityLedger::new("orders-app/window-main").unwrap();
        assert_eq!(
            ledger.claim(UiKey::new(7, 2)),
            Err(IdentityError::UnexpectedGeneration {
                expected: UiKey::new(7, 1),
                actual: UiKey::new(7, 2),
            })
        );
        ledger.claim(UiKey::new(7, 1)).unwrap();
        ledger.retire(UiKey::new(7, 1)).unwrap();
        assert_eq!(
            ledger.claim(UiKey::new(7, 1)),
            Err(IdentityError::UnexpectedGeneration {
                expected: UiKey::new(7, 2),
                actual: UiKey::new(7, 1),
            })
        );
        ledger.claim(UiKey::new(7, 2)).unwrap();
        assert_eq!(ledger.admit(UiKey::new(7, 1)), Err(IdentityError::Stale(UiKey::new(7, 1))));
    }
}
