//! Preallocated voice pool with a compile-time capacity bound and
//! quietest-then-oldest stealing. No allocation after `new`; all methods are
//! RT-safe (03-audio-runtime-spec.md §CPU 尖峰防线).

/// Slot state tracked next to the caller's voice payload.
pub struct VoiceSlot<V> {
    pub voice: V,
    active: bool,
    age: u64,
    level: f32,
}

impl<V> VoiceSlot<V> {
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Last level reported via `allocate`/`update_level`; the steal metric.
    pub fn level(&self) -> f32 {
        self.level
    }
}

/// Fixed-capacity pool. `V: Default` so the backing array is built once.
pub struct VoicePool<V: Default, const N: usize> {
    slots: [VoiceSlot<V>; N],
    clock: u64,
}

impl<V: Default, const N: usize> VoicePool<V, N> {
    pub fn new() -> Self {
        Self {
            slots: core::array::from_fn(|_| VoiceSlot {
                voice: V::default(),
                active: false,
                age: 0,
                level: 0.0,
            }),
            clock: 0,
        }
    }

    pub const fn capacity(&self) -> usize {
        N
    }

    /// Take a free slot, or steal the quietest active voice (ties broken by
    /// oldest age). Returns the slot index. RT-safe, never allocates.
    pub fn allocate(&mut self, initial_level: f32) -> usize {
        self.clock += 1;
        let index = match self.slots.iter().position(|s| !s.active) {
            Some(i) => i,
            None => self.steal_candidate(),
        };
        let slot = &mut self.slots[index];
        slot.active = true;
        slot.age = self.clock;
        slot.level = initial_level;
        index
    }

    fn steal_candidate(&self) -> usize {
        let mut best = 0usize;
        for i in 1..N {
            let (candidate, current) = (&self.slots[i], &self.slots[best]);
            let quieter = candidate.level < current.level;
            let tie_older = candidate.level == current.level && candidate.age < current.age;
            if quieter || tie_older {
                best = i;
            }
        }
        best
    }

    /// Return a slot to the free list.
    pub fn release(&mut self, index: usize) {
        self.slots[index].active = false;
    }

    /// Update the steal metric (e.g. current envelope level, each block).
    pub fn update_level(&mut self, index: usize, level: f32) {
        self.slots[index].level = level;
    }

    pub fn slot(&self, index: usize) -> &VoiceSlot<V> {
        &self.slots[index]
    }

    pub fn slot_mut(&mut self, index: usize) -> &mut VoiceSlot<V> {
        &mut self.slots[index]
    }

    pub fn active_count(&self) -> usize {
        self.slots.iter().filter(|s| s.active).count()
    }

    /// Indices of active slots in slot order (deterministic summation).
    pub fn iter_active(&self) -> impl Iterator<Item = usize> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.active.then_some(i))
    }
}

impl<V: Default, const N: usize> Default for VoicePool<V, N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_free_slots_first() {
        let mut pool = VoicePool::<f32, 4>::new();
        let a = pool.allocate(0.5);
        let b = pool.allocate(0.5);
        assert_ne!(a, b);
        assert_eq!(pool.active_count(), 2);
        pool.release(a);
        assert_eq!(pool.allocate(0.5), a);
    }

    #[test]
    fn steals_quietest_then_oldest() {
        let mut pool = VoicePool::<f32, 4>::new();
        for level in [0.5, 0.1, 0.9, 0.1] {
            pool.allocate(level);
        }
        // Slots 1 and 3 tie at 0.1; slot 1 is older.
        assert_eq!(pool.allocate(1.0), 1);
        // Slot 3 is now the quietest.
        assert_eq!(pool.allocate(1.0), 3);
        pool.update_level(0, 0.01);
        assert_eq!(pool.allocate(1.0), 0);
    }

    #[test]
    fn iter_active_yields_slot_order() {
        let mut pool = VoicePool::<f32, 8>::new();
        pool.allocate(1.0);
        pool.allocate(1.0);
        pool.allocate(1.0);
        pool.release(1);
        let active: Vec<usize> = pool.iter_active().collect();
        assert_eq!(active, vec![0, 2]);
    }
}
