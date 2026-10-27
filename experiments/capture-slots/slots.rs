// Candidate only; not exported by the UI crate and not delivered before all gates.
// 仅为候选，UI crate 尚未导出；全链完成之前不作为交付。
struct CaptureSlots {
    entries: Vec<(u64, Capture)>,
}

impl CaptureSlots {
    fn new() -> Self {
        Self { entries: Vec::new() }
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn get(&self, key: u64) -> Option<&Capture> {
        match self.position(key) {
            Some(index) => Some(&self.entries[index].1),
            None => None,
        }
    }

    fn insert(&mut self, key: u64, value: Capture) -> Option<Capture> {
        match self.position(key) {
            Some(index) => {
                let previous = self.entries.remove(index);
                self.entries.insert(index, (key, value));
                Some(previous.1)
            },
            None => {
                self.entries.push((key, value));
                None
            }
        }
    }

    fn remove(&mut self, key: u64) -> Option<Capture> {
        match self.position(key) {
            Some(index) => Some(self.entries.remove(index).1),
            None => None,
        }
    }

    fn position(&self, key: u64) -> Option<usize> {
        let mut index = 0;
        while index < self.entries.len() {
            if self.entries[index].0 == key {
                return Some(index);
            }
            index += 1;
        }
        None
    }
}
