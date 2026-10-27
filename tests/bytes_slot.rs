//! Actual public storage API, including borrowed access and ownership return.
//! 检查实际公开存储 API 的借用访问与所有权返还。
use jasper_ui_engine::resource::UiBytesSlot;

#[test]
fn payload_lifecycle_preserves_storage_and_rejects_stale_keys() {
    let mut slot = UiBytesSlot::new(7, 42);
    let bytes = vec![1, 2, 3, 4];
    let pointer = bytes.as_ptr();
    let key = slot.install(7, bytes).unwrap();
    {
        let borrowed = slot.bytes(7, key).unwrap();
        assert_eq!(borrowed, &[1, 2, 3, 4]);
        assert_eq!(borrowed.as_ptr(), pointer);
    }
    let returned = slot.release(7, key).unwrap();
    assert_eq!(returned.as_ptr(), pointer);
    let current = slot.install(7, returned).unwrap();
    assert_eq!(slot.bytes(7, key), None);
    assert_eq!(slot.release(7, key), None);
    assert_eq!(slot.bytes(7, current), Some(&[1, 2, 3, 4][..]));
    let retired = slot.retire().unwrap();
    assert_eq!(retired.as_ptr(), pointer);
    assert_eq!(slot.bytes(7, current), None);
    assert_eq!(slot.install(7, retired), Err(vec![1, 2, 3, 4]));
    assert_eq!(slot.retire(), None);
}

#[test]
fn wrong_session_and_duplicate_install_preserve_payload() {
    let mut slot = UiBytesSlot::new(7, 42);
    assert_eq!(slot.install(8, vec![9]), Err(vec![9]));
    let key = slot.install(7, vec![1, 2]).unwrap();
    assert_eq!(slot.install(7, vec![9]), Err(vec![9]));
    assert_eq!(slot.bytes(8, key), None);
    assert_eq!(slot.release(8, key), None);
    assert_eq!(slot.bytes(7, key), Some(&[1, 2][..]));
}
