//! ArrayBuffer memory lives outside the V8 heap limit, so each isolate gets
//! an allocator that counts it against a byte budget. Over budget V8's
//! allocation fails; its last-resort GC then reports the heap limit, the
//! near-heap-limit callback terminates the scene and the scene thread exits
//! normally (not parked; pinned by `no_buffer_path_allocates_past_the_budget`).
//! Buffers the host hands the scene are charged to the same budget and throw
//! a `RangeError`.
//!
//! Resizable and growable buffers and WebAssembly memory bypass the
//! allocator and this V8 has no flag to turn them off, so [`HARDEN_JS`]
//! removes them.

use std::alloc::{Layout, alloc, alloc_zeroed, dealloc};
use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const ALIGN: usize = 16;

/// Runs before the scene: buffers whose memory the allocator never sees are
/// not constructible (`maxByteLength` is ignored; `resize`,
/// `SharedArrayBuffer` and `WebAssembly` are gone).
pub(super) const HARDEN_JS: &str = r#"(function () {
  'use strict';
  var AB = ArrayBuffer;
  var construct = Reflect.construct;
  var define = Object.defineProperty;
  var handler = Object.freeze({
    construct: function (target, args, newTarget) {
      // Coerced here, so no scene code runs inside the native constructor.
      var length = args.length > 0 ? +args[0] : undefined;
      return construct(AB, [length], newTarget);
    }
  });
  var Fixed = new Proxy(AB, handler);
  define(AB.prototype, 'constructor', { value: Fixed, writable: false, configurable: false });
  delete AB.prototype.resize;
  define(globalThis, 'ArrayBuffer', { value: Fixed, writable: true, configurable: true });
  delete globalThis.SharedArrayBuffer;
  delete globalThis.WebAssembly;
})();"#;

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

#[allow(
    unsafe_code,
    reason = "raw allocation behind the V8 ArrayBuffer allocator"
)]
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

#[allow(unsafe_code, reason = "V8 ArrayBuffer allocator callbacks")]
unsafe extern "C" fn allocate(budget: &ArrayBufferBudget, len: usize) -> *mut c_void {
    // SAFETY: allocate_with has no caller preconditions.
    unsafe { allocate_with(budget, len, true) }
}

#[allow(unsafe_code, reason = "V8 ArrayBuffer allocator callbacks")]
unsafe extern "C" fn allocate_uninitialized(budget: &ArrayBufferBudget, len: usize) -> *mut c_void {
    // SAFETY: allocate_with has no caller preconditions.
    unsafe { allocate_with(budget, len, false) }
}

#[allow(unsafe_code, reason = "V8 ArrayBuffer allocator callbacks")]
unsafe extern "C" fn free(budget: &ArrayBufferBudget, data: *mut c_void, len: usize) {
    if data.is_null() {
        return;
    }
    if let Some(layout) = layout(len) {
        // SAFETY: V8 frees a buffer from `allocate` with the length it asked for, so this is its layout.
        unsafe { dealloc(data as *mut u8, layout) };
        budget.refund(len);
    }
}

#[allow(unsafe_code, reason = "V8 ArrayBuffer allocator callbacks")]
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
#[allow(unsafe_code, reason = "installs the budgeted allocator in V8")]
pub(super) fn allocator(budget: &Arc<ArrayBufferBudget>) -> v8::UniqueRef<v8::Allocator> {
    // SAFETY: the handle is an owned `Arc` reference that `drop_budget` releases, and the
    // callbacks count every live buffer against it.
    unsafe { v8::new_rust_allocator(Arc::into_raw(Arc::clone(budget)), &VTABLE) }
}

#[allow(unsafe_code, reason = "V8 ArrayBuffer allocator callbacks")]
unsafe extern "C" fn release_host_buffer(data: *mut c_void, len: usize, budget: *mut c_void) {
    // SAFETY: the `Arc::into_raw` reference `uint8array` passed as deleter data; V8 calls this once.
    let budget = unsafe { Arc::from_raw(budget as *const ArrayBufferBudget) };
    if let Some(layout) = layout(len) {
        // SAFETY: `uint8array` allocated `data` with `layout(len)` for this same `len`.
        unsafe { dealloc(data as *mut u8, layout) };
    }
    budget.refund(len);
}

/// A `Uint8Array` holding a copy of `bytes`, charged to `budget`; `None`
/// when the budget has no room.
#[allow(unsafe_code, reason = "hands V8 a budgeted backing store")]
pub(super) fn uint8array<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    budget: &Arc<ArrayBufferBudget>,
    bytes: &[u8],
) -> Option<v8::Local<'s, v8::Uint8Array>> {
    let len = bytes.len();
    // SAFETY: allocate_with has no caller preconditions.
    let data = unsafe { allocate_with(budget, len, false) };
    if data.is_null() {
        return None;
    }
    // SAFETY: `data` is a fresh allocation of at least `len` bytes, so it cannot overlap `bytes`.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), data as *mut u8, len) };
    let deleter_data = Arc::into_raw(Arc::clone(budget)) as *mut c_void;
    // SAFETY: the store owns `data` (`len` bytes) from here; release_host_buffer frees it once.
    let store = unsafe {
        v8::ArrayBuffer::new_backing_store_from_ptr(data, len, release_host_buffer, deleter_data)
    }
    .make_shared();
    let ab = v8::ArrayBuffer::with_backing_store(scope, &store);
    v8::Uint8Array::new(scope, ab, 0, len)
}
