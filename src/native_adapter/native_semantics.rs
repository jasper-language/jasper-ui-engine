//! Jasper-owned view semantics for platform consumers, not a platform accessibility bridge.
//! 供平台消费的 Jasper 自有视图语义；不实现任何系统无障碍 API。
use jasper_ui_engine::input_identity::UiInputContext;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticRole { Text, Button }
pub struct ViewSemantics {
    pub context: UiInputContext,
    pub component_version: u64,
    pub role: SemanticRole,
    pub name: String,
    pub disabled: bool,
    /// Native tree focus only; the platform must combine window/application focus.
    pub native_focused: bool,
    pub can_activate: bool,
}

// BEGIN VERIFIED NATIVE ACTIVATION ADMISSION KERNEL
/// Eligibility after owner-admitted semantics; not authentication or replay protection.
/// 已准入语义快照的动作资格，不代替宿主认证或防重放。
pub(crate) fn accepts_activation(current_version: u64, expected_version: u64, can_activate: bool) -> bool {
    current_version == expected_version && can_activate
}
// END VERIFIED NATIVE ACTIVATION ADMISSION KERNEL
