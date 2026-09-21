//! Optional, bounded CPU contention for the no-device managed probe.
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub struct BusyLoad {
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}
impl BusyLoad {
    pub fn start() -> (Self, usize) {
        let count = std::env::var("OXITONE_VST3_LOAD_THREADS")
            .map(|s| s.parse::<usize>().expect("load threads must be an integer"))
            .unwrap_or(0);
        assert!(count <= 64);
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = mpsc::sync_channel(count.max(1));
        let mut load = Self {
            stop,
            threads: Vec::with_capacity(count),
        };
        for index in 0..count {
            let stop = load.stop.clone();
            let ready = ready_tx.clone();
            load.threads.push(thread::spawn(move || {
                // No barrier: if a later spawn fails, Drop can still stop/join earlier threads.
                if ready.send(()).is_err() {
                    return;
                }
                let deadline = Instant::now() + Duration::from_secs(45);
                let mut value = index as u64 + 1;
                while !stop.load(Ordering::Relaxed) && Instant::now() < deadline {
                    for _ in 0..4096 {
                        value = std::hint::black_box(
                            value.wrapping_mul(6364136223846793005).wrapping_add(1),
                        );
                    }
                }
            }));
        }
        drop(ready_tx);
        for _ in 0..count {
            ready_rx.recv().unwrap();
        }
        (load, count)
    }
}
impl Drop for BusyLoad {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for thread in self.threads.drain(..) {
            thread.join().unwrap();
        }
    }
}
