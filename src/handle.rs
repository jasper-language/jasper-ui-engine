//! Generation-checked opaque handles for the future native boundary; this freezes no ABI.
//! 面向后续原生边界的代际检查不透明句柄；此处不冻结任何 ABI。

// BEGIN VERIFIED HANDLE TYPE
/// Opaque index and generation; the all-zero value is never live.
/// 不透明的下标与代数；全零值永远无效。
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handle {
    pub index: u32,
    pub generation: u32,
}

/// Slot generations only move upward: odd is live, even is vacant, and the retired value
/// is permanently vacant, so generations never wrap.
/// 槽位代数只增不减：奇数存活、偶数空闲，退役值永久空闲，代数不回绕。
pub struct HandleAllocator {
    slots: Vec<u32>,
    free: Vec<u32>,
    capacity: u32,
    first_generation: u32,
}

/// Owned values addressed through an allocator; a value is reachable only by its live handle.
/// 经分配器寻址的自有值；只有存活句柄才能取到对应值。
pub struct HandleTable<T> {
    allocator: HandleAllocator,
    values: Vec<Option<T>>,
}
// END VERIFIED HANDLE TYPE

// BEGIN VERIFIED HANDLE KERNEL
pub const HANDLE_GENERATION_RETIRED: u32 = 0xFFFF_FFFE;

fn handle_generation_is_live(generation: u32) -> bool {
    generation & 1 == 1
}

impl HandleAllocator {
    pub fn new(capacity: u32) -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            capacity,
            first_generation: 1,
        }
    }

    /// A restarted host may start past the handles of its previous run; the first
    /// generation must be odd and below the retired value.
    /// 重启的宿主可越过上次运行签发的句柄；首代必须为奇数且低于退役值。
    pub fn with_first_generation(capacity: u32, first_generation: u32) -> Option<Self> {
        if !handle_generation_is_live(first_generation)
            || first_generation >= HANDLE_GENERATION_RETIRED
        {
            return None;
        }
        Some(Self {
            slots: Vec::new(),
            free: Vec::new(),
            capacity,
            first_generation,
        })
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    pub fn is_live(&self, handle: Handle) -> bool {
        let index = handle.index as usize;
        index < self.slots.len()
            && handle_generation_is_live(handle.generation)
            && self.slots[index] == handle.generation
    }

    /// Reuses a vacant slot under a newer generation or appends one within capacity.
    /// 以更新的代数复用空闲槽位，或在容量内追加；已满且无可复用槽位时拒绝。
    pub fn allocate(&mut self) -> Option<Handle> {
        match self.free.pop() {
            Some(index) => {
                let generation = self.slots[index as usize] + 1;
                self.slots[index as usize] = generation;
                Some(Handle { index, generation })
            }
            None => {
                if self.slots.len() >= self.capacity as usize {
                    return None;
                }
                let index = self.slots.len() as u32;
                let generation = self.first_generation;
                self.slots.push(generation);
                Some(Handle { index, generation })
            }
        }
    }

    /// Stale, foreign and repeated releases are rejected without any state change.
    /// 过期、外来与重复释放一律拒绝且不改变状态；代数耗尽的槽位退役，不再复用。
    pub fn release(&mut self, handle: Handle) -> bool {
        if !self.is_live(handle) {
            return false;
        }
        let generation = handle.generation + 1;
        self.slots[handle.index as usize] = generation;
        if generation != HANDLE_GENERATION_RETIRED {
            self.free.push(handle.index);
        }
        true
    }
}

impl<T> HandleTable<T> {
    pub fn new(capacity: u32) -> Self {
        Self {
            allocator: HandleAllocator::new(capacity),
            values: Vec::new(),
        }
    }

    pub fn with_first_generation(capacity: u32, first_generation: u32) -> Option<Self> {
        match HandleAllocator::with_first_generation(capacity, first_generation) {
            None => None,
            Some(allocator) => Some(Self {
                allocator,
                values: Vec::new(),
            }),
        }
    }

    pub fn is_live(&self, handle: Handle) -> bool {
        self.allocator.is_live(handle)
    }

    /// A full table returns the value to the caller instead of dropping it.
    /// 表满时把值交还调用方，不丢弃。
    pub fn insert(&mut self, value: T) -> Result<Handle, T> {
        match self.allocator.allocate() {
            None => Err(value),
            Some(handle) => {
                let index = handle.index as usize;
                if index == self.values.len() {
                    self.values.push(Some(value));
                } else {
                    self.values[index] = Some(value);
                }
                Ok(handle)
            }
        }
    }

    pub fn get(&self, handle: Handle) -> Option<&T> {
        if !self.allocator.is_live(handle) {
            return None;
        }
        self.values[handle.index as usize].as_ref()
    }

    pub fn get_mut(&mut self, handle: Handle) -> Option<&mut T> {
        if !self.allocator.is_live(handle) {
            return None;
        }
        self.values[handle.index as usize].as_mut()
    }

    pub fn remove(&mut self, handle: Handle) -> Option<T> {
        if !self.allocator.release(handle) {
            return None;
        }
        self.values[handle.index as usize].take()
    }
}
// END VERIFIED HANDLE KERNEL
