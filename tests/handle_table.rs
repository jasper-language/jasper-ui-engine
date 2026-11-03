//! Public-API behavior of generation-checked handles.
//! 代际句柄的公开接口行为。

use jasper_ui_engine::handle::{Handle, HandleAllocator, HandleTable, HANDLE_GENERATION_RETIRED};

#[test]
fn handle_layout_is_two_fixed_width_words() {
    assert_eq!(std::mem::size_of::<Handle>(), 8);
    assert_eq!(std::mem::align_of::<Handle>(), 4);
    assert_eq!(std::mem::offset_of!(Handle, index), 0);
    assert_eq!(std::mem::offset_of!(Handle, generation), 4);
}

#[test]
fn stale_foreign_and_repeated_handles_are_rejected_without_effect() {
    let mut table = HandleTable::new(2);
    let first = table.insert("a").unwrap();
    let second = table.insert("b").unwrap();
    assert_eq!(table.remove(first), Some("a"));
    assert_eq!(table.remove(first), None);
    assert_eq!(table.get(first), None);

    let reused = table.insert("c").unwrap();
    assert_eq!(reused.index, first.index);
    assert!(reused.generation > first.generation);
    assert_eq!(table.get(first), None);
    assert_eq!(table.get(reused), Some(&"c"));
    assert_eq!(table.get(second), Some(&"b"));

    for foreign in [
        Handle {
            index: 0,
            generation: 0,
        },
        Handle {
            index: 9,
            generation: 1,
        },
        Handle {
            index: second.index,
            generation: second.generation + 1,
        },
    ] {
        assert!(!table.is_live(foreign));
        assert_eq!(table.remove(foreign), None);
    }
    assert_eq!(table.get(second), Some(&"b"));
}

#[test]
fn full_table_returns_the_value_and_recovers_after_release() {
    let mut table = HandleTable::new(1);
    let only = table.insert(String::from("kept")).unwrap();
    assert_eq!(
        table.insert(String::from("back")),
        Err(String::from("back"))
    );
    table.get_mut(only).unwrap().push('!');
    assert_eq!(table.remove(only).as_deref(), Some("kept!"));
    assert!(table.insert(String::from("again")).is_ok());
}

#[test]
fn exhausted_slot_retires_while_other_slots_keep_working() {
    let last = HANDLE_GENERATION_RETIRED - 1;
    let mut allocator = HandleAllocator::with_first_generation(2, last).unwrap();
    let a = allocator.allocate().unwrap();
    assert!(allocator.release(a));
    let b = allocator.allocate().unwrap();
    assert_eq!(b.index, 1);
    assert!(allocator.release(b));
    assert_eq!(allocator.allocate(), None);
    assert!(!allocator.is_live(a));
    assert!(!allocator.is_live(b));
}

#[test]
fn first_generation_must_be_odd_and_below_the_retired_value() {
    assert!(HandleAllocator::with_first_generation(1, 0).is_none());
    assert!(HandleAllocator::with_first_generation(1, 2).is_none());
    assert!(HandleAllocator::with_first_generation(1, HANDLE_GENERATION_RETIRED).is_none());
    assert!(HandleAllocator::with_first_generation(1, u32::MAX).is_none());
    assert!(HandleTable::<u8>::with_first_generation(1, 3).is_some());
    assert!(HandleTable::<u8>::with_first_generation(1, 4).is_none());
    assert_eq!(HandleAllocator::new(7).capacity(), 7);
}
