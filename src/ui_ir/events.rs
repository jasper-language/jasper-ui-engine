//! Authenticated event-target admission after IR structural validation.

use super::{EventBinding, UiEventKind, UiFrame, UiKey};
use crate::nostd_prelude::*;
use alloc::collections::BTreeSet;
use core::fmt;
use serde::{Deserialize, Serialize};

/// The permission needed to route a bound event. Backend `NodeId` is never accepted here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
// BEGIN VERIFIED UI PERMISSION TYPE
pub enum UiPermission {
    Pointer,
    Activate,
    Focus,
    Keyboard,
    TextEdit,
    Ime,
    Scroll,
    Dismiss,
    AccessibilityAction,
}
// END VERIFIED UI PERMISSION TYPE

// BEGIN VERIFIED UI EVENT PERMISSION KERNEL
impl UiEventKind {
    pub const fn required_permission(self) -> UiPermission {
        match self {
            Self::Click | Self::Activate => UiPermission::Activate,
            Self::PointerDown | Self::PointerUp | Self::Hover => UiPermission::Pointer,
            Self::Focus => UiPermission::Focus,
            Self::Key => UiPermission::Keyboard,
            Self::TextCommit => UiPermission::TextEdit,
            Self::ImeCommit | Self::ImeCancel => UiPermission::Ime,
            Self::Scroll => UiPermission::Scroll,
            Self::Dismiss => UiPermission::Dismiss,
            Self::AccessibilityAction => UiPermission::AccessibilityAction,
        }
    }
}
// END VERIFIED UI EVENT PERMISSION KERNEL

/// Auditable host-side permission grant. The context name identifies the policy source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiPermissionContext {
    pub name: String,
    pub permissions: BTreeSet<UiPermission>,
}

impl UiPermissionContext {
    pub fn new(
        name: impl Into<String>,
        permissions: impl IntoIterator<Item = UiPermission>,
    ) -> Self {
        Self {
            name: name.into(),
            permissions: permissions.into_iter().collect(),
        }
    }

    pub fn permits(&self, permission: UiPermission) -> bool {
        self.permissions.contains(&permission)
    }
}

/// A host-authenticated event target for one identity domain, owner and session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiEventContext {
    pub target: UiKey,
    pub identity_domain: String,
    pub owner: String,
    pub session: String,
    pub frame_revision: u64,
    pub permission_context: UiPermissionContext,
}

impl UiEventContext {
    /// Admit a binding in a frame that contains the target. Stale generations cannot select a
    /// successor, and an event without the required grant cannot invoke Framework code.
    pub fn admit(
        &self,
        frame: &UiFrame,
        event: UiEventKind,
    ) -> Result<AdmittedEvent, UiEventAdmissionError> {
        frame
            .validate()
            .map_err(|error| UiEventAdmissionError::InvalidFrame(error.to_string()))?;
        check_authority_values(
            AuthorityRef {
                identity_domain: &self.identity_domain,
                owner: &self.owner,
                session: &self.session,
                revision: self.frame_revision,
            },
            AuthorityRef {
                identity_domain: &frame.identity_domain,
                owner: &frame.owner,
                session: &frame.session,
                revision: frame.next_revision,
            },
        )?;
        let node = frame
            .node(self.target)
            .ok_or(UiEventAdmissionError::StaleGeneration(self.target))?;
        let binding = node
            .events
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find(|binding| binding.event == event)
            .ok_or(UiEventAdmissionError::EventNotBound {
                target: self.target,
                event,
            })?;
        check_permission(&self.permission_context, event, binding)?;
        Ok(AdmittedEvent {
            target: self.target,
            event,
            action: binding.action.clone(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdmittedEvent {
    pub target: UiKey,
    pub event: UiEventKind,
    pub action: String,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct AuthorityRef<'a> {
    pub identity_domain: &'a str,
    pub owner: &'a str,
    pub session: &'a str,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// BEGIN VERIFIED AUTHORITY FAILURE TYPE
pub(crate) enum AuthorityFailure {
    IdentityDomain,
    Owner,
    Session,
    Revision,
}
// END VERIFIED AUTHORITY FAILURE TYPE

// BEGIN VERIFIED AUTHORITY MATCH KERNEL
pub(crate) fn authority_failure_from_matches(
    identity_domain_matches: bool,
    owner_matches: bool,
    session_matches: bool,
    revision_matches: bool,
) -> Option<AuthorityFailure> {
    if !identity_domain_matches {
        return Some(AuthorityFailure::IdentityDomain);
    }
    if !owner_matches {
        return Some(AuthorityFailure::Owner);
    }
    if !session_matches {
        return Some(AuthorityFailure::Session);
    }
    if !revision_matches {
        return Some(AuthorityFailure::Revision);
    }
    None
}
// END VERIFIED AUTHORITY MATCH KERNEL

pub(super) fn check_authority_values(
    actual: AuthorityRef<'_>,
    expected: AuthorityRef<'_>,
) -> Result<(), UiEventAdmissionError> {
    match authority_failure_from_matches(
        actual.identity_domain == expected.identity_domain,
        actual.owner == expected.owner,
        actual.session == expected.session,
        actual.revision == expected.revision,
    ) {
        None => Ok(()),
        Some(AuthorityFailure::IdentityDomain) => {
            Err(UiEventAdmissionError::IdentityDomainMismatch {
                expected: expected.identity_domain.into(),
                actual: actual.identity_domain.into(),
            })
        }
        Some(AuthorityFailure::Owner) => Err(UiEventAdmissionError::OwnerMismatch {
            expected: expected.owner.into(),
            actual: actual.owner.into(),
        }),
        Some(AuthorityFailure::Session) => Err(UiEventAdmissionError::SessionMismatch {
            expected: expected.session.into(),
            actual: actual.session.into(),
        }),
        Some(AuthorityFailure::Revision) => Err(UiEventAdmissionError::StaleRevision {
            expected: expected.revision,
            actual: actual.revision,
        }),
    }
}

pub(super) fn check_permission(
    context: &UiPermissionContext,
    event: UiEventKind,
    binding: &EventBinding,
) -> Result<(), UiEventAdmissionError> {
    let permission = event.required_permission();
    match permission_failure(!context.name.is_empty(), context.permits(permission)) {
        None => Ok(()),
        Some(PermissionFailure::UnknownContext) => {
            Err(UiEventAdmissionError::UnknownPermissionContext)
        }
        Some(PermissionFailure::Denied) => Err(UiEventAdmissionError::PermissionDenied {
            permission,
            action: binding.action.clone(),
        }),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// BEGIN VERIFIED PERMISSION FAILURE TYPE
pub(crate) enum PermissionFailure {
    UnknownContext,
    Denied,
}
// END VERIFIED PERMISSION FAILURE TYPE

// BEGIN VERIFIED PERMISSION MATCH KERNEL
pub(crate) fn permission_failure(
    context_named: bool,
    permission_granted: bool,
) -> Option<PermissionFailure> {
    if !context_named {
        return Some(PermissionFailure::UnknownContext);
    }
    if !permission_granted {
        return Some(PermissionFailure::Denied);
    }
    None
}
// END VERIFIED PERMISSION MATCH KERNEL

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiEventAdmissionError {
    InvalidFrame(String),
    IdentityDomainMismatch {
        expected: String,
        actual: String,
    },
    OwnerMismatch {
        expected: String,
        actual: String,
    },
    SessionMismatch {
        expected: String,
        actual: String,
    },
    StaleRevision {
        expected: u64,
        actual: u64,
    },
    StaleGeneration(UiKey),
    EventNotBound {
        target: UiKey,
        event: UiEventKind,
    },
    UnknownPermissionContext,
    PermissionDenied {
        permission: UiPermission,
        action: String,
    },
    CapabilityDenied {
        permission: UiPermission,
    },
}

impl fmt::Display for UiEventAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFrame(detail) => write!(f, "invalid UI IR frame: {detail}"),
            Self::IdentityDomainMismatch { expected, actual } => {
                write!(
                    f,
                    "event identity domain {actual} does not match {expected}"
                )
            }
            Self::OwnerMismatch { expected, actual } => {
                write!(f, "event owner {actual} does not match {expected}")
            }
            Self::SessionMismatch { expected, actual } => {
                write!(f, "event session {actual} does not match {expected}")
            }
            Self::StaleRevision { expected, actual } => {
                write!(f, "event revision {actual} does not match {expected}")
            }
            Self::StaleGeneration(key) => write!(
                f,
                "event target {}:{} is unknown or stale in the admitted frame",
                key.semantic_id, key.generation
            ),
            Self::EventNotBound { target, event } => write!(
                f,
                "target {}:{} has no {event:?} binding",
                target.semantic_id, target.generation
            ),
            Self::UnknownPermissionContext => write!(f, "permission context has no policy name"),
            Self::PermissionDenied { permission, action } => write!(
                f,
                "permission {permission:?} is required to invoke action {action:?}"
            ),
            Self::CapabilityDenied { permission } => {
                write!(
                    f,
                    "permission {permission:?} is required for this Native target"
                )
            }
        }
    }
}

impl core::error::Error for UiEventAdmissionError {}
