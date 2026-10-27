//! Direct production source import; no surrogate slot or backend model.
#[path = "../../../src/native_adapter/view_slot.rs"]
mod actual;
pub use actual::ViewSlot;

pub fn check_trace(data: &[u8]) {
    let mut slot = ViewSlot::new(7u64);
    let mut oracle = Some(7u64);
    for &operation in data.iter().take(128) {
        match operation % 4 {
            0 => { assert_eq!(slot.close(), oracle.take().is_some()); }
            1 => {
                if let Some(value) = slot.as_mut() { *value = value.wrapping_add(operation as u64); }
                if let Some(value) = oracle.as_mut() { *value = value.wrapping_add(operation as u64); }
            }
            2 => { assert_eq!(slot.as_ref().copied(), oracle); }
            _ => { assert_eq!(slot.is_open(), oracle.is_some()); }
        }
        assert_eq!(slot.as_ref().copied(), oracle);
        assert_eq!(slot.is_open(), oracle.is_some());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn stateful_sequences(data in proptest::collection::vec(any::<u8>(), 0..129)) {
            check_trace(&data);
        }
    }
    #[test]
    fn miri_owned_drop_and_late_operations() {
        use std::{cell::Cell, rc::Rc};
        struct Counted(Rc<Cell<usize>>);
        impl Drop for Counted { fn drop(&mut self) { self.0.set(self.0.get() + 1); } }
        let drops = Rc::new(Cell::new(0));
        let mut slot = ViewSlot::new(Counted(drops.clone()));
        assert!(slot.as_ref().is_some());
        assert!(slot.close());
        assert_eq!(drops.get(), 1);
        assert!(!slot.close());
        assert!(slot.as_mut().is_none());
        assert!(slot.as_ref().is_none());
        drop(slot);
        assert_eq!(drops.get(), 1);
        for sequence in [&[][..], &[0][..], &[1,2,3,0,1,2,3,0][..]] { check_trace(sequence); }
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;
    #[kani::proof]
    fn close_and_reject_late_access() {
        let value: u64 = kani::any();
        let mut slot = ViewSlot::new(value);
        assert!(slot.is_open());
        assert_eq!(slot.as_ref(), Some(&value));
        if kani::any::<bool>() {
            if let Some(stored) = slot.as_mut() { *stored = 9; }
        }
        assert!(slot.close());
        assert!(!slot.close());
        assert!(!slot.is_open());
        assert!(slot.as_ref().is_none());
        assert!(slot.as_mut().is_none());
    }
}
