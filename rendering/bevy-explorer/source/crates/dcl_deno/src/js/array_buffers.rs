//! ArrayBuffer memory lives outside the V8 heap limit, so each scene isolate gets an allocator
//! that counts it. Past the scene's budget the allocation still succeeds and the scene is
//! terminated, as on its heap cap: V8 aborts the process when an allocation it makes for the
//! host fails (`v8::ArrayBuffer::New`, `serde_v8` byte copies), so refusing at the budget
//! would turn it into a way to end every scene. Only a request past twice the budget is
//! refused; the scene sees a `RangeError`, and V8's last-resort collection for it reaches the
//! heap-limit callback, which ends the scene.
//!
//! Not counted, because their memory never goes through the allocator: resizable buffers,
//! `SharedArrayBuffer`, WebAssembly memory, and results V8 adopts from Rust memory (encoded
//! text, request bodies).
//!
//! Adapted from the worlds runtime's budget (`services/catalyst/worlds`), which pins a
//! different V8.

use std::alloc::{alloc, alloc_zeroed, dealloc, Layout};
use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use deno_core::v8;

const ALIGN: usize = 16;

pub(crate) struct ArrayBufferBudget {
    used: AtomicUsize,
    limit: usize,
    over: OnceLock<Box<dyn Fn() + Send + Sync>>,
}

impl ArrayBufferBudget {
    pub(super) fn new(limit: usize) -> Arc<Self> {
        Arc::new(Self {
            used: AtomicUsize::new(0),
            limit,
            over: OnceLock::new(),
        })
    }

    /// What to do when the scene goes past its budget; set once the isolate exists.
    pub(super) fn on_over(&self, over: impl Fn() + Send + Sync + 'static) {
        let _ = self.over.set(Box::new(over));
    }

    fn charge(&self, len: usize) -> bool {
        let ceiling = self.limit.saturating_mul(2);
        let Ok(before) = self
            .used
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |used| {
                used.checked_add(len).filter(|n| *n <= ceiling)
            })
        else {
            return false;
        };
        if before.saturating_add(len) > self.limit {
            if let Some(over) = self.over.get() {
                over();
            }
        }
        true
    }

    fn refund(&self, len: usize) {
        self.used.fetch_sub(len, Ordering::SeqCst);
    }

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
    // SAFETY: `layout` never has a zero size (`len.max(1)`).
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
    // SAFETY: allocate_with has no caller preconditions.
    unsafe { allocate_with(budget, len, true) }
}

unsafe extern "C" fn allocate_uninitialized(budget: &ArrayBufferBudget, len: usize) -> *mut c_void {
    // SAFETY: allocate_with has no caller preconditions.
    unsafe { allocate_with(budget, len, false) }
}

unsafe extern "C" fn free(budget: &ArrayBufferBudget, data: *mut c_void, len: usize) {
    if data.is_null() {
        return;
    }
    if let Some(layout) = layout(len) {
        // SAFETY: V8 frees a buffer from `allocate` with the length it asked for, so this is
        // its layout.
        unsafe { dealloc(data as *mut u8, layout) };
        budget.refund(len);
    }
}

unsafe extern "C" fn drop_budget(budget: *const ArrayBufferBudget) {
    // SAFETY: the `Arc::into_raw` reference `allocator` gave V8, which drops it once.
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
    // SAFETY: the handle is an owned `Arc` reference that `drop_budget` releases, and the
    // callbacks count every live buffer against it.
    unsafe { v8::new_rust_allocator(Arc::into_raw(Arc::clone(budget)), &VTABLE) }
}
