use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
    static FREES: Cell<usize> = const { Cell::new(0) };
}

struct Counting;
#[global_allocator]
static ALLOCATOR: Counting = Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = TRACKING.try_with(|active| {
            if active.get() {
                ALLOCS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let _ = TRACKING.try_with(|active| {
            if active.get() {
                FREES.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.dealloc(ptr, layout) }
    }
}

pub fn count(run: impl FnOnce()) -> (usize, usize) {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            TRACKING.with(|a| a.set(false));
        }
    }
    ALLOCS.with(|n| n.set(0));
    FREES.with(|n| n.set(0));
    TRACKING.with(|a| a.set(true));
    let guard = Guard;
    run();
    drop(guard);
    (ALLOCS.with(Cell::get), FREES.with(Cell::get))
}
