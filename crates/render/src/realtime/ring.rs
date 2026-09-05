//! Lock-free single-producer/single-consumer primitives for the realtime
//! path (03-audio-runtime-spec.md §线程模型: HAL callback 只做 ring →
//! output 的 copy；transport/parameter 命令经 lock-free command queue).
//!
//! - [`SpscRing`]: frame-granular interleaved f32 audio ring. Producer is
//!   the render worker, consumer is the HAL callback pull closure.
//! - [`SpscQueue`]: bounded message queue (control → worker, worker/callback
//!   → control diagnostics). Full means drop + `queueDrops++` at the caller.
//!
//! Both are preallocated at session setup; `push`/`pop` are RT-safe.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

/// Frame-based SPSC ring of interleaved f32 samples.
pub struct SpscRing {
    buffer: Vec<AtomicU32>,
    writing: AtomicBool,
    reading: AtomicBool,
    /// Total capacity in samples (frames * channels).
    capacity: usize,
    channels: usize,
    /// Monotonic sample counters; available = write - read.
    write: AtomicUsize,
    read: AtomicUsize,
}

// SAFETY: one thread only ever calls `write_frames` (producer) and one
// thread only ever calls `read_frames` (consumer). The producer only
// touches slots in [read, read + free) and publishes them by releasing
// `write`; the consumer only touches slots in [read, read + available) and
// publishes consumption by releasing `read`. The index ranges never
// overlap, so no slot is accessed by both threads at once.

impl SpscRing {
    /// `frames` must be > 0 and is rounded up to the next power of two so
    /// slot math can use a mask.
    pub fn new(frames: usize, channels: usize) -> Self {
        assert!(frames > 0 && channels > 0);
        let capacity = frames.next_power_of_two() * channels;
        Self {
            buffer: (0..capacity).map(|_| AtomicU32::new(0)).collect(),
            writing: AtomicBool::new(false),
            reading: AtomicBool::new(false),
            capacity,
            channels,
            write: AtomicUsize::new(0),
            read: AtomicUsize::new(0),
        }
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Capacity in frames (power-of-two sample capacity / channels).
    pub fn capacity_frames(&self) -> usize {
        self.capacity / self.channels
    }

    pub fn available_to_read_frames(&self) -> usize {
        self.write
            .load(Ordering::Acquire)
            .wrapping_sub(self.read.load(Ordering::Acquire))
            .min(self.capacity)
            / self.channels
    }

    pub fn available_to_write_frames(&self) -> usize {
        self.capacity_frames() - self.available_to_read_frames()
    }

    /// Producer: copy up to `input.len() / channels` frames in. Returns
    /// frames actually written.
    pub fn write_frames(&self, input: &[f32]) -> usize {
        if self.writing.swap(true, Ordering::Acquire) {
            return 0;
        }
        let frames = (input.len() / self.channels).min(self.available_to_write_frames());
        if frames == 0 {
            self.writing.store(false, Ordering::Release);
            return 0;
        }
        let samples = frames * self.channels;
        let write = self.write.load(Ordering::Relaxed);
        // SAFETY: the [write, write + samples) range lies inside the free
        // region checked above; only the producer writes there.
        let buffer = &self.buffer;
        let start = write % self.capacity;
        let first = samples.min(self.capacity - start);
        for i in 0..first {
            buffer[start + i].store(input[i].to_bits(), Ordering::Relaxed);
        }
        if first < samples {
            for i in first..samples {
                buffer[i - first].store(input[i].to_bits(), Ordering::Relaxed);
            }
        }
        self.write
            .store(write.wrapping_add(samples), Ordering::Release);
        self.writing.store(false, Ordering::Release);
        frames
    }

    /// Consumer: copy up to `out.len() / channels` frames out. Returns
    /// frames actually read; callers zero-fill the remainder (underrun).
    pub fn read_frames(&self, out: &mut [f32]) -> usize {
        if self.reading.swap(true, Ordering::Acquire) {
            return 0;
        }
        let frames = (out.len() / self.channels).min(self.available_to_read_frames());
        if frames == 0 {
            self.reading.store(false, Ordering::Release);
            return 0;
        }
        let samples = frames * self.channels;
        let read = self.read.load(Ordering::Relaxed);
        // SAFETY: the [read, read + samples) range lies inside the filled
        // region checked above; only the consumer reads there.
        let buffer = &self.buffer;
        let start = read % self.capacity;
        let first = samples.min(self.capacity - start);
        for i in 0..first {
            out[i] = f32::from_bits(buffer[start + i].load(Ordering::Relaxed));
        }
        if first < samples {
            for i in first..samples {
                out[i] = f32::from_bits(buffer[i - first].load(Ordering::Relaxed));
            }
        }
        self.read
            .store(read.wrapping_add(samples), Ordering::Release);
        self.reading.store(false, Ordering::Release);
        frames
    }
}

/// Bounded MPMC queue: control and device monitor may publish concurrently.
pub struct SpscQueue<T> {
    queue: crossbeam_queue::ArrayQueue<T>,
}

impl<T> SpscQueue<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: crossbeam_queue::ArrayQueue::new(capacity.max(2)),
        }
    }
    pub fn push(&self, value: T) -> Result<(), T> {
        self.queue.push(value)
    }
    pub fn pop(&self) -> Option<T> {
        self.queue.pop()
    }
    pub fn is_full(&self) -> bool {
        self.queue.is_full()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_roundtrip_preserves_order() {
        let ring = SpscRing::new(8, 2);
        let input: Vec<f32> = (0..16).map(|i| i as f32).collect();
        assert_eq!(ring.write_frames(&input), 8);
        assert_eq!(ring.available_to_read_frames(), 8);
        assert_eq!(ring.write_frames(&input), 0, "full ring refuses writes");
        let mut out = vec![0.0; 10];
        assert_eq!(ring.read_frames(&mut out), 5);
        assert_eq!(out, input[..10]);
        assert_eq!(ring.available_to_read_frames(), 3);
    }

    #[test]
    fn ring_wraps_around() {
        let ring = SpscRing::new(4, 2);
        let a = [1.0, 2.0, 3.0, 4.0];
        let b = [5.0, 6.0];
        assert_eq!(ring.write_frames(&a), 2);
        let mut out = vec![0.0; 2];
        assert_eq!(ring.read_frames(&mut out), 1);
        assert_eq!(ring.write_frames(&a), 2);
        assert_eq!(ring.write_frames(&b), 1);
        let mut all = vec![0.0; 10];
        assert_eq!(ring.read_frames(&mut all), 4);
        assert_eq!(all, [3.0, 4.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 0.0, 0.0]);
    }

    #[test]
    fn ring_survives_concurrent_stress() {
        // Producer/consumer threads exchange a monotonically increasing
        // sequence; any race shows up as a discontinuity.
        let ring = std::sync::Arc::new(SpscRing::new(256, 2));
        let producer_ring = ring.clone();
        let consumer_ring = ring;
        let total_frames = 2_000_000usize;
        let producer = std::thread::spawn(move || {
            let mut value = 0.0f32;
            let mut buffer = vec![0.0; 128 * 2];
            let mut sent = 0;
            while sent < total_frames {
                for slot in &mut buffer {
                    *slot = value;
                    value += 1.0;
                }
                let frames = buffer.len() / 2;
                let written = producer_ring.write_frames(&buffer);
                sent += written;
                // Rewind unwritten samples (they must be resent).
                if written < frames {
                    value -= ((frames - written) * 2) as f32;
                }
            }
        });
        let consumer = std::thread::spawn(move || {
            let mut expected = 0.0f32;
            let mut got = 0;
            let mut buffer = vec![0.0; 96 * 2];
            while got < total_frames {
                let read = consumer_ring.read_frames(&mut buffer);
                for &sample in &buffer[..read * 2] {
                    assert_eq!(sample, expected, "sequence broke");
                    expected += 1.0;
                }
                got += read;
                if read == 0 {
                    std::thread::yield_now();
                }
            }
        });
        producer.join().unwrap();
        consumer.join().unwrap();
    }

    #[test]
    fn queue_push_pop_and_full() {
        let queue = SpscQueue::new(2);
        assert!(queue.push(1).is_ok());
        assert!(queue.push(2).is_ok());
        assert_eq!(queue.push(3), Err(3));
        assert_eq!(queue.pop(), Some(1));
        assert_eq!(queue.pop(), Some(2));
        assert_eq!(queue.pop(), None);
    }
}
