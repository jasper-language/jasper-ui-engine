use vstd::prelude::*;
verus! {



#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiEventKind {
    Click,
    Activate,
    PointerDown,
    PointerUp,
    Hover,
    Focus,
    Key,
    TextCommit,
    ImeCommit,
    ImeCancel,
    Scroll,
    Dismiss,
    AccessibilityAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

impl UiEventKind {
    pub const fn required_permission(self) -> (result: UiPermission)
    ensures match self {
        UiEventKind::Click | UiEventKind::Activate => result == UiPermission::Activate,
        UiEventKind::PointerDown | UiEventKind::PointerUp | UiEventKind::Hover => result == UiPermission::Pointer,
        UiEventKind::Focus => result == UiPermission::Focus,
        UiEventKind::Key => result == UiPermission::Keyboard,
        UiEventKind::TextCommit => result == UiPermission::TextEdit,
        UiEventKind::ImeCommit | UiEventKind::ImeCancel => result == UiPermission::Ime,
        UiEventKind::Scroll => result == UiPermission::Scroll,
        UiEventKind::Dismiss => result == UiPermission::Dismiss,
        UiEventKind::AccessibilityAction => result == UiPermission::AccessibilityAction,
    },
{
        match self {
            Self::Click | Self::Activate => UiPermission::Activate,
            Self::PointerDown | Self::PointerUp | Self::Hover => UiPermission::Pointer,
            Self::Focus => UiPermission::Focus,
            Self::Key => UiPermission::Keyboard,
            Self::TextCommit => UiPermission::TextEdit,
            Self::ImeCommit | Self::ImeCancel => UiPermission::Ime,
            Self::Scroll => UiPermission::Scroll,
            Self::Dismiss => UiPermission::Activate,
            Self::AccessibilityAction => UiPermission::AccessibilityAction,
        }
    }
}

}
