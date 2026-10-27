//! Owned UI-view lifetime, independent of the upstream renderer and host.
// BEGIN VERIFIED UI VIEW SLOT KERNEL
pub struct ViewSlot<T> {
    value: Option<T>,
}

impl<T> ViewSlot<T> {
    pub fn new(value: T) -> Self {
        Self { value: Some(value) }
    }

    /// Retires the owned value before its destructor runs; never reopens the slot.
    /// This is logical ownership retirement, not an external cancellation ACK.
    pub fn close(&mut self) -> bool {
        self.value.take().is_some()
    }

    pub fn is_open(&self) -> bool {
        self.value.is_some()
    }
}
// END VERIFIED UI VIEW SLOT KERNEL

// Standard Option borrow delegation; no native pointer or callback escapes.
// BEGIN VERIFIED UI VIEW READ BORROW KERNEL
impl<T> ViewSlot<T> {
    pub fn as_ref(&self) -> Option<&T> { self.value.as_ref() }
}
// END VERIFIED UI VIEW READ BORROW KERNEL
// BEGIN VERIFIED UI VIEW WRITE BORROW KERNEL
impl<T> ViewSlot<T> {
    pub fn as_mut(&mut self) -> Option<&mut T> { self.value.as_mut() }
}
// END VERIFIED UI VIEW WRITE BORROW KERNEL
