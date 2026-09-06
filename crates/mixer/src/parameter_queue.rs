//! Preallocated plugin parameter staging. One value per parameter per segment.
use oxitone_core::wire::ParameterSpec;
use oxitone_graph::ParameterEvent;
use std::mem::MaybeUninit;

pub struct ParameterQueue {
    specs: Vec<ParameterSpec>,
    pending: Vec<(usize, f64)>,
    // Layout-only storage. No initialized 'static references are created.
    storage: Box<[MaybeUninit<ParameterEvent<'static>>]>,
}

impl ParameterQueue {
    /// Control thread; capacity covers every declared parameter.
    pub fn new(specs: &[ParameterSpec]) -> Self {
        Self {
            specs: specs.to_vec(),
            pending: Vec::with_capacity(specs.len()),
            storage: Box::new_uninit_slice(specs.len()),
        }
    }

    /// Resolved parameter index; latest value wins at the segment boundary.
    pub fn set(&mut self, index: usize, value: f64) {
        assert!(index < self.specs.len(), "resolved parameter index");
        if let Some(entry) = self.pending.iter_mut().find(|(i, _)| *i == index) {
            entry.1 = value;
        } else {
            self.pending.push((index, value));
        }
    }

    /// Build ABI events by integer index, without allocation or string lookup.
    pub fn with_events<R>(
        &mut self,
        consume: impl for<'a> FnOnce(&'a [ParameterEvent<'a>]) -> R,
    ) -> R {
        let count = self.pending.len();
        let ptr = self.storage.as_mut_ptr().cast::<ParameterEvent<'_>>();
        // SAFETY: storage has ParameterEvent's layout/alignment and capacity for
        // every unique validated index. Each of the first `count` slots is written
        // before being read. IDs borrow `self.specs` only for this call; the HRTB
        // prevents `consume` (including its result) from retaining these borrows.
        // ParameterEvent has no destructor. The backing MaybeUninit allocation is
        // never read/dropped as initialized 'static events, including on unwind.
        unsafe {
            for (slot, &(index, value)) in self.pending.iter().enumerate() {
                ptr.add(slot).write(ParameterEvent {
                    frame_offset: 0,
                    parameter_id: &self.specs[index].id,
                    value,
                });
            }
            self.pending.clear();
            consume(std::slice::from_raw_parts(ptr, count))
        }
    }
}
