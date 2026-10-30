use core::alloc::Layout;

#[cfg(not(feature = "std"))]
use alloc::alloc::{alloc, dealloc, handle_alloc_error};
#[cfg(feature = "std")]
use std::alloc::{alloc, dealloc, handle_alloc_error};

#[cfg(all(test, feature = "std"))]
std::thread_local! {
    static ALLOCATION_FILL: core::cell::Cell<Option<u8>> = const { core::cell::Cell::new(None) };
    static ALLOCATION_PEAK: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

#[cfg(all(test, feature = "std"))]
pub(crate) fn enet_test_allocation_peak(reset: bool) -> usize {
    ALLOCATION_PEAK.with(|peak| {
        let value = peak.get();
        if reset {
            peak.set(0);
        }
        value
    })
}

// Scoped initialization makes pre-fix packet regression reads defined and deterministic.
#[cfg(all(test, feature = "std"))]
pub(crate) fn enet_test_with_allocation_fill<R>(fill: u8, run: impl FnOnce() -> R) -> R {
    struct Restore(Option<u8>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ALLOCATION_FILL.with(|value| value.set(self.0));
        }
    }
    let _restore = Restore(ALLOCATION_FILL.with(|value| value.replace(Some(fill))));
    run()
}

pub(crate) unsafe fn enet_malloc(layout: Layout) -> *mut u8 {
    #[cfg(all(test, feature = "std"))]
    ALLOCATION_PEAK.with(|peak| peak.set(peak.get().max(layout.size())));
    let ptr = unsafe { alloc(layout) };
    if ptr.is_null() {
        handle_alloc_error(layout);
    }
    #[cfg(all(test, feature = "std"))]
    ALLOCATION_FILL.with(|fill| {
        if let Some(value) = fill.get() {
            unsafe { ptr.write_bytes(value, layout.size()) };
        }
    });
    ptr
}

pub(crate) unsafe fn enet_free(ptr: *mut u8, layout: Layout) {
    dealloc(ptr, layout);
}
