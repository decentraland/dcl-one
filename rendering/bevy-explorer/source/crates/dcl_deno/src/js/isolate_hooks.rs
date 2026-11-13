//! Isolate-wide V8 callbacks and the ArrayBuffer allocator. Scene code reaches these without
//! any op, so like ops they never panic: a panic inside a V8 callback aborts the sidecar.

use std::{
    ffi::c_void,
    fmt::Write as _,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use deno_core::v8;

/// Installs the hooks below, replacing deno_core's.
pub fn install(isolate: &mut v8::Isolate) {
    isolate.set_prepare_stack_trace_callback(prepare_stack_trace);
    isolate.set_wasm_streaming_callback(refuse_wasm_streaming);
    isolate.set_wasm_async_resolve_promise_callback(settle_wasm_compilation);
}

const MAX_HEADER_UNITS: usize = 64 * 1024;
const MAX_FRAME_PART_UNITS: usize = 4 * 1024;
const MAX_STACK_BYTES: usize = 1024 * 1024;

/// `error.stack` in deno_core's format, built without running scene code: V8 does not stop
/// JS running inside this callback, not even to terminate it. Only data properties are read,
/// so a getter, a proxy or a scene's `Error.prepareStackTrace` never runs, and every string is
/// cut to a bounded length.
pub fn prepare_stack_trace<'s, 'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    error: v8::Local<'s, v8::Value>,
    callsites: v8::Local<'s, v8::Array>,
) -> v8::Local<'s, v8::Value> {
    v8::tc_scope!(let scope, scope);
    let undefined: v8::Local<v8::Value> = v8::undefined(scope).into();
    if let Ok(object) = v8::Local::<v8::Object>::try_from(error) {
        // deno_core reads the call sites from here when it reports an uncaught error
        if let Some(name) = v8::String::new(scope, "deno_core::call_site_evals") {
            let key = v8::Private::for_api(scope, Some(name));
            object.set_private(scope, key, callsites.into());
        }
    }
    let mut stack = header(scope, error);
    for index in 0..callsites.length() {
        if stack.len() > MAX_STACK_BYTES {
            stack.push_str("\n    ...");
            break;
        }
        let Some(callsite) = callsites
            .get_index(scope, index)
            .and_then(|c| v8::Local::<v8::Object>::try_from(c).ok())
        else {
            break;
        };
        let Some(frame) = Frame::read(scope, callsite) else {
            break;
        };
        stack.push_str("\n    at ");
        frame.format(&mut stack);
    }
    v8::String::new(scope, &stack)
        .map(Into::into)
        .unwrap_or(undefined)
}

/// A data property found on `object` or its prototypes; `None` if it is missing, an accessor,
/// or behind a proxy.
fn data_property<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    object: v8::Local<'s, v8::Object>,
    name: &str,
) -> Option<v8::Local<'s, v8::Value>> {
    let key: v8::Local<v8::Name> = v8::String::new(scope, name)?.into();
    let get = v8::String::new(scope, "get")?.into();
    let set = v8::String::new(scope, "set")?.into();
    let value = v8::String::new(scope, "value")?.into();
    let mut current = object;
    for _ in 0..32 {
        if current.is_proxy() {
            return None;
        }
        let descriptor = current.get_own_property_descriptor(scope, key)?;
        if let Ok(descriptor) = v8::Local::<v8::Object>::try_from(descriptor) {
            if descriptor.has_own_property(scope, get)?
                || descriptor.has_own_property(scope, set)?
            {
                return None;
            }
            return descriptor.get(scope, value);
        }
        current = v8::Local::<v8::Object>::try_from(current.get_prototype(scope)?).ok()?;
    }
    None
}

fn header<'s>(scope: &mut v8::PinScope<'s, '_>, error: v8::Local<'s, v8::Value>) -> String {
    let Ok(object) = v8::Local::<v8::Object>::try_from(error) else {
        return String::new();
    };
    let message = data_property(scope, object, "message")
        .filter(|m| !m.is_undefined())
        .and_then(|m| bounded_string(scope, m, MAX_HEADER_UNITS))
        .unwrap_or_default();
    let name = data_property(scope, object, "name")
        .filter(|n| !n.is_undefined())
        .and_then(|n| bounded_string(scope, n, MAX_HEADER_UNITS))
        .unwrap_or_else(|| "Error".to_owned());
    match (message.is_empty(), name.is_empty()) {
        (false, false) => format!("{name}: {message}"),
        (false, true) => message,
        (true, false) => name,
        (true, true) => String::new(),
    }
}

/// A primitive as a string of at most `max_units` UTF-16 units. `None` for null, undefined,
/// symbols and objects, whose conversion could run scene code.
fn bounded_string(
    scope: &mut v8::PinScope,
    value: v8::Local<v8::Value>,
    max_units: usize,
) -> Option<String> {
    if value.is_null_or_undefined() || value.is_symbol() || value.is_object() {
        return None;
    }
    let string = value.to_string(scope)?;
    let length = string.length();
    let mut units = vec![0u16; length.min(max_units)];
    string.write_v2(scope, 0, &mut units, v8::WriteFlags::empty());
    let mut out = String::from_utf16_lossy(&units);
    if length > max_units {
        out.push('…');
    }
    Some(out)
}

/// The parts of a V8 call site that deno_core's formatter reads.
#[derive(Default)]
struct Frame {
    file_name: Option<String>,
    line: Option<i64>,
    column: Option<i64>,
    eval_origin: Option<String>,
    type_name: Option<String>,
    function_name: Option<String>,
    method_name: Option<String>,
    is_top_level: bool,
    is_eval: bool,
    is_native: bool,
    is_constructor: bool,
    is_async: bool,
    is_promise_all: bool,
    promise_index: Option<i64>,
}

impl Frame {
    fn read<'s>(
        scope: &mut v8::PinScope<'s, '_>,
        callsite: v8::Local<'s, v8::Object>,
    ) -> Option<Self> {
        let mut call = |name: &str| -> Option<v8::Local<'s, v8::Value>> {
            let method = data_property(scope, callsite, name)?;
            let method = v8::Local::<v8::Function>::try_from(method).ok()?;
            method.call(scope, callsite.into(), &[])
        };
        let file_name = call("getFileName")?;
        let line = call("getLineNumber")?;
        let column = call("getColumnNumber")?;
        let eval_origin = call("getEvalOrigin")?;
        let type_name = call("getTypeName")?;
        let function_name = call("getFunctionName")?;
        let method_name = call("getMethodName")?;
        let is_top_level = call("isToplevel")?;
        let is_eval = call("isEval")?;
        let is_native = call("isNative")?;
        let is_constructor = call("isConstructor")?;
        let is_async = call("isAsync")?;
        let is_promise_all = call("isPromiseAll")?;
        let promise_index = call("getPromiseIndex")?;
        let number =
            |v: v8::Local<v8::Value>| v.is_number().then(|| v.integer_value(scope)).flatten();
        let line = number(line);
        let column = number(column);
        let promise_index = number(promise_index);
        Some(Self {
            file_name: bounded_string(scope, file_name, MAX_FRAME_PART_UNITS),
            line,
            column,
            eval_origin: bounded_string(scope, eval_origin, MAX_FRAME_PART_UNITS),
            type_name: bounded_string(scope, type_name, MAX_FRAME_PART_UNITS),
            function_name: bounded_string(scope, function_name, MAX_FRAME_PART_UNITS),
            method_name: bounded_string(scope, method_name, MAX_FRAME_PART_UNITS),
            is_top_level: is_top_level.is_true(),
            is_eval: is_eval.is_true(),
            is_native: is_native.is_true(),
            is_constructor: is_constructor.is_true(),
            is_async: is_async.is_true(),
            is_promise_all: is_promise_all.is_true(),
            promise_index,
        })
    }

    /// deno_core's `format_frame` without colours.
    fn format(&self, out: &mut String) {
        if self.is_async {
            out.push_str("async ");
        }
        if self.is_promise_all {
            let _ = write!(
                out,
                "Promise.all (index {})",
                self.promise_index.unwrap_or(0)
            );
            return;
        }
        let function_name = self.function_name.as_deref().filter(|f| !f.is_empty());
        if !(self.is_top_level || self.is_constructor) {
            match function_name {
                Some(function_name) => {
                    if let Some(type_name) = &self.type_name {
                        if !function_name.starts_with(type_name.as_str()) {
                            let _ = write!(out, "{type_name}.");
                        }
                    }
                    out.push_str(function_name);
                    if let Some(method_name) = &self.method_name {
                        if !function_name.ends_with(method_name.as_str()) {
                            let _ = write!(out, " [as {method_name}]");
                        }
                    }
                }
                None => {
                    if let Some(type_name) = &self.type_name {
                        let _ = write!(out, "{type_name}.");
                    }
                    out.push_str(self.method_name.as_deref().unwrap_or("<anonymous>"));
                }
            }
        } else if self.is_constructor {
            out.push_str("new ");
            out.push_str(function_name.unwrap_or("<anonymous>"));
        } else if let Some(function_name) = function_name {
            out.push_str(function_name);
        } else {
            self.format_location(out);
            return;
        }
        out.push_str(" (");
        self.format_location(out);
        out.push(')');
    }

    fn format_location(&self, out: &mut String) {
        if self.is_native {
            out.push_str("native");
            return;
        }
        match self.file_name.as_deref().filter(|f| !f.is_empty()) {
            Some(file_name) => {
                out.push_str(&deno_core::error::format_file_name(file_name, None).file_name)
            }
            None => {
                if let Some(origin) = self.eval_origin.as_deref().filter(|_| self.is_eval) {
                    let _ = write!(out, "{origin}, ");
                }
                out.push_str("<anonymous>");
            }
        }
        if let Some(line) = self.line {
            let _ = write!(out, ":{line}");
            if let Some(column) = self.column {
                let _ = write!(out, ":{column}");
            }
        }
    }
}

/// `WebAssembly.compileStreaming` and `instantiateStreaming` are removed in init.js; were
/// either reached, it rejects instead of reaching deno_core's handler, which needs a JS
/// callback scenes do not have.
fn refuse_wasm_streaming<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    _source: v8::Local<'s, v8::Value>,
    streaming: v8::WasmStreaming<false>,
) {
    let error = v8::String::new(scope, "WebAssembly streaming is not available to scenes")
        .map(|message| v8::Exception::type_error(scope, message));
    streaming.abort(error);
}

/// Settles an asynchronous WebAssembly compile; the promise may already be unusable when the
/// isolate is terminating.
unsafe extern "C" fn settle_wasm_compilation(
    _isolate: v8::UnsafeRawIsolatePtr,
    context: v8::Local<v8::Context>,
    resolver: v8::Local<v8::PromiseResolver>,
    result: v8::Local<v8::Value>,
    success: v8::WasmAsyncSuccess,
) {
    // SAFETY: V8 passes a live context for this callback
    v8::callback_scope!(unsafe scope, context);
    if success == v8::WasmAsyncSuccess::Success {
        let _ = resolver.resolve(scope, result);
    } else {
        let _ = resolver.reject(scope, result);
    }
}

/// ArrayBuffer memory lives outside the V8 heap, so the heap cap does not bound it; this
/// allocator refuses an allocation that would take a scene past its own budget.
pub struct ArrayBufferBudget {
    used: AtomicUsize,
    cap: usize,
}

impl ArrayBufferBudget {
    fn reserve(&self, len: usize) -> bool {
        self.used
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |used| {
                used.checked_add(len).filter(|total| *total <= self.cap)
            })
            .is_ok()
    }

    fn release(&self, len: usize) {
        let _ = self
            .used
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |used| {
                Some(used.saturating_sub(len))
            });
    }

    #[cfg(test)]
    pub fn used(&self) -> usize {
        self.used.load(Ordering::SeqCst)
    }
}

// V8 frees a backing store with the length it allocated, so the layout can be rebuilt
fn layout(len: usize) -> Option<std::alloc::Layout> {
    // a zero-length buffer still needs a unique pointer
    std::alloc::Layout::from_size_align(len.max(1), 16).ok()
}

unsafe extern "C" fn allocate_zeroed(budget: &ArrayBufferBudget, len: usize) -> *mut c_void {
    // SAFETY: `layout` has a non-zero size
    allocate_with(budget, len, |layout| unsafe {
        std::alloc::alloc_zeroed(layout)
    })
}

unsafe extern "C" fn allocate_uninitialized(budget: &ArrayBufferBudget, len: usize) -> *mut c_void {
    // SAFETY: `layout` has a non-zero size
    allocate_with(budget, len, |layout| unsafe { std::alloc::alloc(layout) })
}

fn allocate_with(
    budget: &ArrayBufferBudget,
    len: usize,
    allocate: impl FnOnce(std::alloc::Layout) -> *mut u8,
) -> *mut c_void {
    let Some(layout) = layout(len) else {
        return std::ptr::null_mut();
    };
    if !budget.reserve(len) {
        return std::ptr::null_mut();
    }
    let data = allocate(layout);
    if data.is_null() {
        budget.release(len);
    }
    data.cast()
}

unsafe extern "C" fn free(budget: &ArrayBufferBudget, data: *mut c_void, len: usize) {
    if let Some(layout) = layout(len) {
        // SAFETY: `data` came from `allocate_with` with this length
        unsafe { std::alloc::dealloc(data.cast(), layout) };
    }
    budget.release(len);
}

unsafe extern "C" fn drop_budget(budget: *const ArrayBufferBudget) {
    // SAFETY: the pointer is the `Arc::into_raw` from `array_buffer_allocator`
    drop(unsafe { Arc::from_raw(budget) });
}

static ALLOCATOR: v8::RustAllocatorVtable<ArrayBufferBudget> = v8::RustAllocatorVtable {
    allocate: allocate_zeroed,
    allocate_uninitialized,
    free,
    drop: drop_budget,
};

/// An allocator for one isolate whose ArrayBuffers together stay within `cap` bytes.
pub fn array_buffer_allocator(
    cap: usize,
) -> (v8::UniqueRef<v8::Allocator>, Arc<ArrayBufferBudget>) {
    let budget = Arc::new(ArrayBufferBudget {
        used: AtomicUsize::new(0),
        cap,
    });
    // SAFETY: the vtable matches the handle's type, and `drop_budget` releases this reference
    let allocator = unsafe { v8::new_rust_allocator(Arc::into_raw(budget.clone()), &ALLOCATOR) };
    (allocator, budget)
}
