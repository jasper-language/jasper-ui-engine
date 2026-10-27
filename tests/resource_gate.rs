//! Real public API checks for host-owned resource admission.
//! 宿主持有资源准入的真实公开 API 检查。
use jasper_ui_engine::resource::ResourceGate;

#[test]
fn stale_cross_session_and_cross_slot_keys_are_rejected() {
    let mut first = ResourceGate::new(7, 42);
    let mut second = ResourceGate::new(7, 43);
    let old = first.issue(7).unwrap();
    let other = second.issue(7).unwrap();
    assert!(!first.allows(8, old));
    assert!(!first.allows(7, other));
    assert!(!first.revoke(8, old));
    assert!(first.revoke(7, old));
    let new = first.issue(7).unwrap();
    assert_eq!(new.generation, old.generation + 1);
    assert!(!first.allows(7, old));
    assert!(!first.revoke(7, old));
    assert!(first.allows(7, new));
    first.retire();
    assert!(!first.allows(7, new));
    assert_eq!(first.issue(7), None);
    first.retire();
    assert_eq!(first.issue(7), None);
}

#[test]
fn issuance_is_exclusive_and_wrong_session_preserves_vacancy() {
    let mut gate = ResourceGate::new(7, 42);
    assert_eq!(gate.issue(8), None);
    let key = gate.issue(7).unwrap();
    assert_eq!(key.generation, 1);
    assert_eq!(gate.issue(7), None);
    assert!(gate.allows(7, key));
    assert!(gate.revoke(7, key));
    assert!(!gate.revoke(7, key));
}
