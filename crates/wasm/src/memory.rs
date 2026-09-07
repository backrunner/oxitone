//! Counters let hosts verify the whole process path, including linked plugins.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU32, Ordering};
static ALLOCS: AtomicU32 = AtomicU32::new(0);
static FREES: AtomicU32 = AtomicU32::new(0);
struct Tracked;
unsafe impl GlobalAlloc for Tracked {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        FREES.fetch_add(1, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        FREES.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Tracked = Tracked;
#[no_mangle]
pub extern "C" fn oxi_allocations() -> u32 {
    ALLOCS.load(Ordering::Relaxed)
}
#[no_mangle]
pub extern "C" fn oxi_deallocations() -> u32 {
    FREES.load(Ordering::Relaxed)
}
