//! ArrayBuffer memory lives outside the V8 heap limit, so each isolate gets
//! an allocator that counts it against a byte budget. Over budget V8 sees a
//! failed allocation and the scene is stopped, as on a heap overflow. Buffers
//! the host hands the scene are charged to the same budget.

use std::alloc::{Layout, alloc, alloc_zeroed, dealloc};
use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const ALIGN: usize = 16;

#[derive(Debug)]
pub(super) struct ArrayBufferBudget {
    used: AtomicUsize,
    limit: usize,
}

impl ArrayBufferBudget {
    pub(super) fn new(limit: usize) -> Arc<Self> {
        Arc::new(Self {
            used: AtomicUsize::new(0),
            limit,
        })
    }

    fn charge(&self, len: usize) -> bool {
        self.used
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |used| {
                used.checked_add(len).filter(|n| *n <= self.limit)
            })
            .is_ok()
    }

    fn refund(&self, len: usize) {
        self.used.fetch_sub(len, Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(super) fn used(&self) -> usize {
        self.used.load(Ordering::SeqCst)
    }
}

fn layout(len: usize) -> Option<Layout> {
    Layout::from_size_align(len.max(1), ALIGN).ok()
}

unsafe fn allocate_with(budget: &ArrayBufferBudget, len: usize, zeroed: bool) -> *mut c_void {
    let Some(layout) = layout(len) else {
        return std::ptr::null_mut();
    };
    if !budget.charge(len) {
        return std::ptr::null_mut();
    }
    let ptr = unsafe {
        if zeroed {
            alloc_zeroed(layout)
        } else {
            alloc(layout)
        }
    };
    if ptr.is_null() {
        budget.refund(len);
    }
    ptr as *mut c_void
}

unsafe extern "C" fn allocate(budget: &ArrayBufferBudget, len: usize) -> *mut c_void {
    unsafe { allocate_with(budget, len, true) }
}

unsafe extern "C" fn allocate_uninitialized(budget: &ArrayBufferBudget, len: usize) -> *mut c_void {
    unsafe { allocate_with(budget, len, false) }
}

unsafe extern "C" fn free(budget: &ArrayBufferBudget, data: *mut c_void, len: usize) {
    if data.is_null() {
        return;
    }
    if let Some(layout) = layout(len) {
        unsafe { dealloc(data as *mut u8, layout) };
        budget.refund(len);
    }
}

unsafe extern "C" fn drop_budget(budget: *const ArrayBufferBudget) {
    drop(unsafe { Arc::from_raw(budget) });
}

static VTABLE: v8::RustAllocatorVtable<ArrayBufferBudget> = v8::RustAllocatorVtable {
    allocate,
    allocate_uninitialized,
    free,
    drop: drop_budget,
};

/// The isolate's ArrayBuffer allocator, counting against `budget`.
pub(super) fn allocator(budget: &Arc<ArrayBufferBudget>) -> v8::UniqueRef<v8::Allocator> {
    unsafe { v8::new_rust_allocator(Arc::into_raw(Arc::clone(budget)), &VTABLE) }
}

unsafe extern "C" fn release_host_buffer(data: *mut c_void, len: usize, budget: *mut c_void) {
    let budget = unsafe { Arc::from_raw(budget as *const ArrayBufferBudget) };
    if let Some(layout) = layout(len) {
        unsafe { dealloc(data as *mut u8, layout) };
    }
    budget.refund(len);
}

/// A `Uint8Array` holding a copy of `bytes`, charged to `budget`; `None`
/// when the budget has no room.
pub(super) fn uint8array<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    budget: &Arc<ArrayBufferBudget>,
    bytes: &[u8],
) -> Option<v8::Local<'s, v8::Uint8Array>> {
    let len = bytes.len();
    let data = unsafe { allocate_with(budget, len, false) };
    if data.is_null() {
        return None;
    }
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), data as *mut u8, len) };
    let deleter_data = Arc::into_raw(Arc::clone(budget)) as *mut c_void;
    let store = unsafe {
        v8::ArrayBuffer::new_backing_store_from_ptr(data, len, release_host_buffer, deleter_data)
    }
    .make_shared();
    let ab = v8::ArrayBuffer::with_backing_store(scope, &store);
    v8::Uint8Array::new(scope, ab, 0, len)
}
