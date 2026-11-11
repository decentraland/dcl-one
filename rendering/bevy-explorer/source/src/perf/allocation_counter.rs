//! Allocations per frame: a `GlobalAlloc` wrapper that counts every allocation and its bytes,
//! read once a frame into the `perf/allocation_count_per_frame` and `perf/allocated_bytes_per_frame`
//! diagnostics.

use std::{
    alloc::{GlobalAlloc, Layout},
    sync::atomic::{AtomicU64, Ordering},
};

use bevy::diagnostic::Diagnostics;
use common::perf::{PERF_ALLOCATED_BYTES_PER_FRAME, PERF_ALLOCATION_COUNT_PER_FRAME};

static ALLOCATION_COUNT: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES_SUM: AtomicU64 = AtomicU64::new(0);

pub struct CountingAllocator<A: GlobalAlloc>(pub A);

fn record_allocation(bytes: usize) {
    ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
    ALLOCATED_BYTES_SUM.fetch_add(bytes as u64, Ordering::Relaxed);
}

// SAFETY: every call forwards to the wrapped allocator unchanged; the counters are atomics.
unsafe impl<A: GlobalAlloc> GlobalAlloc for CountingAllocator<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation(layout.size());
        self.0.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0.dealloc(ptr, layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation(layout.size());
        self.0.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record_allocation(new_size);
        self.0.realloc(ptr, layout, new_size)
    }
}

/// `Last`: records the counts since the last frame and resets them; zero when nothing was
/// counted.
pub fn publish_frame_allocations_system(mut diagnostics: Diagnostics) {
    let allocation_count = ALLOCATION_COUNT.swap(0, Ordering::Relaxed);
    let allocated_bytes_sum = ALLOCATED_BYTES_SUM.swap(0, Ordering::Relaxed);
    diagnostics.add_measurement(&PERF_ALLOCATION_COUNT_PER_FRAME, || allocation_count as f64);
    diagnostics.add_measurement(&PERF_ALLOCATED_BYTES_PER_FRAME, || {
        allocated_bytes_sum as f64
    });
}
