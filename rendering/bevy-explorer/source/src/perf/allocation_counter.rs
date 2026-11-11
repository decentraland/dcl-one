//! Allocations per frame: a `GlobalAlloc` wrapper that counts every allocation and its bytes,
//! read once a frame into the `perf/allocs_count_per_frame` and `perf/alloc_bytes_per_frame`
//! diagnostics.

use std::{
    alloc::{GlobalAlloc, Layout},
    sync::atomic::{AtomicU64, Ordering},
};

use bevy::diagnostic::Diagnostics;
use common::perf::{PERF_ALLOCS_COUNT_PER_FRAME, PERF_ALLOC_BYTES_PER_FRAME};

static ALLOCS_COUNT: AtomicU64 = AtomicU64::new(0);
static BYTES_SUM: AtomicU64 = AtomicU64::new(0);

pub struct CountingAllocator<A: GlobalAlloc>(pub A);

fn count(bytes: usize) {
    ALLOCS_COUNT.fetch_add(1, Ordering::Relaxed);
    BYTES_SUM.fetch_add(bytes as u64, Ordering::Relaxed);
}

// SAFETY: every call forwards to the wrapped allocator unchanged; the counters are atomics.
unsafe impl<A: GlobalAlloc> GlobalAlloc for CountingAllocator<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        self.0.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0.dealloc(ptr, layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        self.0.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count(new_size);
        self.0.realloc(ptr, layout, new_size)
    }
}

/// `Last`: records the counts since the last frame and resets them; zero when nothing was
/// counted.
pub fn publish_frame_allocations_system(mut diagnostics: Diagnostics) {
    let allocs_count = ALLOCS_COUNT.swap(0, Ordering::Relaxed);
    let bytes_sum = BYTES_SUM.swap(0, Ordering::Relaxed);
    diagnostics.add_measurement(&PERF_ALLOCS_COUNT_PER_FRAME, || allocs_count as f64);
    diagnostics.add_measurement(&PERF_ALLOC_BYTES_PER_FRAME, || bytes_sum as f64);
}
