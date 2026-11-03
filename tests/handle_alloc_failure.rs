//! Handle storage under a failing allocator: growth failure is reported, and release never allocates.
//! 失败分配器下的句柄存储：增长失败如实报告，释放从不分配。

use jasper_ui_engine::handle::{Handle, HandleAllocator, HandleStatus, HandleTable};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Rationed;

// Per thread, so the test harness's own threads stay unaffected.
// 按线程计，测试框架自己的线程不受影响。
thread_local! {
    static ALLOWANCE: Cell<Option<usize>> = const { Cell::new(None) };
}

unsafe impl GlobalAlloc for Rationed {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let granted = ALLOWANCE
            .try_with(|allowance| match allowance.get() {
                None => true,
                Some(0) => false,
                Some(left) => {
                    allowance.set(Some(left - 1));
                    true
                }
            })
            .unwrap_or(true);
        if granted {
            unsafe { System.alloc(layout) }
        } else {
            std::ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static RATIONED: Rationed = Rationed;

fn rationed<R>(allowance: usize, operation: impl FnOnce() -> R) -> R {
    ALLOWANCE.with(|cell| cell.set(Some(allowance)));
    let result = operation();
    ALLOWANCE.with(|cell| cell.set(None));
    result
}

#[test]
fn allocator_growth_failure_returns_none_and_changes_nothing() {
    // One reservation each for the slots and the free list.
    // 槽位与空闲表各预留一次。
    for allowance in 0..2 {
        let mut allocator = HandleAllocator::new(8);
        assert_eq!(rationed(allowance, || allocator.allocate()), None);
        assert_eq!(
            allocator.status(Handle {
                index: 0,
                generation: 1
            }),
            HandleStatus::Invalid
        );
        let issued = allocator.allocate().unwrap();
        assert_eq!((issued.index, issued.generation), (0, 1));
    }
}

#[test]
fn table_growth_failure_returns_the_value_and_keeps_existing_entries() {
    // Values, slots and free list: failing any of them leaves no half-issued slot.
    // 值、槽位、空闲表：任一失败都不留下半签发的槽位。
    for allowance in 0..3 {
        let mut table = HandleTable::new(8);
        let value = String::from("back");
        assert_eq!(
            rationed(allowance, || table.insert(value)).unwrap_err(),
            "back"
        );
        let issued = table.insert(String::from("kept")).unwrap();
        assert_eq!((issued.index, issued.generation), (0, 1));
        assert_eq!(table.get(issued).map(String::as_str), Some("kept"));
    }

    let mut table = HandleTable::new(64);
    let mut issued = vec![table.insert(0).unwrap()];
    // Fill the room the first growth left; the next growth is refused.
    // 填满首次增长留下的空间，下一次增长被拒。
    let refused = loop {
        match rationed(0, || table.insert(issued.len())) {
            Ok(handle) => issued.push(handle),
            Err(value) => break value,
        }
    };
    assert_eq!(refused, issued.len());
    for (value, handle) in issued.iter().enumerate() {
        assert_eq!(table.get(*handle), Some(&value));
    }
    let next = table.insert(issued.len()).unwrap();
    assert_eq!(next.index as usize, issued.len());
}

#[test]
fn release_and_reuse_never_allocate() {
    let mut allocator = HandleAllocator::new(64);
    let issued: Vec<Handle> = (0..33).map(|_| allocator.allocate().unwrap()).collect();
    let (released, reissued) = rationed(0, || {
        let released = issued.iter().all(|handle| allocator.release(*handle));
        let reissued = (0..33).all(|_| allocator.allocate().is_some());
        (released, reissued)
    });
    assert!(released && reissued);

    let mut table = HandleTable::new(64);
    let issued: Vec<Handle> = (0..33).map(|value| table.insert(value).unwrap()).collect();
    let (removed, reinserted) = rationed(0, || {
        let removed = issued.iter().all(|handle| table.remove(*handle).is_some());
        let reinserted = (0..33).all(|value| table.insert(value).is_ok());
        (removed, reinserted)
    });
    assert!(removed && reinserted);
}
