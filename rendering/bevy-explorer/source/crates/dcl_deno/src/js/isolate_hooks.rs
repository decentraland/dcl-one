//! Isolate-wide V8 callbacks. Scene code reaches these without any op, and a panic inside a V8
//! callback aborts the sidecar. The stack-trace callback runs no scene code; both tolerate
//! termination and never panic. deno_core's stack-trace callback reads getters and calls
//! `Error.prepareStackTrace`, and its WebAssembly callback unwraps a settle that fails while the
//! isolate terminates. Settling a compile result still reads `then` on it, as any promise
//! resolution does.

use std::fmt::Write as _;

use deno_core::v8;

/// Installs the hooks below, replacing deno_core's.
pub fn install(isolate: &mut v8::Isolate) {
    isolate.set_prepare_stack_trace_callback(prepare_stack_trace);
    isolate.set_wasm_async_resolve_promise_callback(settle_wasm_compilation);
}

const MAX_HEADER_UNITS: usize = 64 * 1024;
const MAX_FRAME_PART_UNITS: usize = 4 * 1024;
const MAX_STACK_BYTES: usize = 1024 * 1024;

/// `error.stack` in deno_core's format, built without running scene code: V8 does not stop
/// JS running inside this callback, not even to terminate it. Only data properties are read,
/// so a getter, a proxy or a scene's `Error.prepareStackTrace` never runs, and every string is
/// cut to a bounded length.
fn prepare_stack_trace<'s, 'i>(
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
            let _ = object.set_private(scope, key, callsites.into());
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
    scope: &v8::PinScope<'s, '_>,
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

fn header<'s>(scope: &v8::PinScope<'s, '_>, error: v8::Local<'s, v8::Value>) -> String {
    let Ok(object) = v8::Local::<v8::Object>::try_from(error) else {
        return String::new();
    };
    let field = |name: &str| {
        data_property(scope, object, name)
            .or_else(|| own_symbol_field(scope, object, name))
            .filter(|v| !v.is_undefined())
            .and_then(|v| {
                if v.is_null() {
                    Some("null".to_owned())
                } else {
                    bounded_string(scope, v, MAX_HEADER_UNITS)
                }
            })
    };
    let message = field("message").unwrap_or_default();
    let name = field("name").unwrap_or_else(|| "Error".to_owned());
    match (message.is_empty(), name.is_empty()) {
        (false, false) => format!("{name}: {message}"),
        (false, true) => message,
        (true, false) => name,
        (true, true) => String::new(),
    }
}

/// A primitive as a string of at most `max_units` UTF-16 units. `None` for null, undefined,
/// symbols and objects, whose conversion could run scene code.
/// The value of an own data property keyed by a symbol described as `description`. deno_web's
/// `DOMException` keeps its name and message under such symbols and exposes them only through
/// prototype getters, which this file does not call.
fn own_symbol_field<'s>(
    scope: &v8::PinScope<'s, '_>,
    object: v8::Local<'s, v8::Object>,
    description: &str,
) -> Option<v8::Local<'s, v8::Value>> {
    if object.is_proxy() {
        return None;
    }
    let keys = object.get_own_property_names(
        scope,
        v8::GetPropertyNamesArgs {
            mode: v8::KeyCollectionMode::OwnOnly,
            property_filter: v8::PropertyFilter::SKIP_STRINGS,
            index_filter: v8::IndexFilter::SkipIndices,
            key_conversion: v8::KeyConversionMode::KeepNumbers,
        },
    )?;
    let value = v8::String::new(scope, "value")?;
    for index in 0..keys.length().min(64) {
        let Some(symbol) = keys
            .get_index(scope, index)
            .and_then(|k| v8::Local::<v8::Symbol>::try_from(k).ok())
        else {
            continue;
        };
        let named = symbol.description(scope);
        if !named.is_string() || named.to_rust_string_lossy(scope) != description {
            continue;
        }
        let descriptor = object.get_own_property_descriptor(scope, symbol.into())?;
        let descriptor = v8::Local::<v8::Object>::try_from(descriptor).ok()?;
        if !descriptor.has_own_property(scope, value.into())? {
            return None;
        }
        return descriptor.get(scope, value.into());
    }
    None
}

fn bounded_string(
    scope: &v8::PinScope<'_, '_>,
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
    fn read<'s>(scope: &v8::PinScope<'s, '_>, callsite: v8::Local<'s, v8::Object>) -> Option<Self> {
        let call = |name: &str| -> Option<v8::Local<'s, v8::Value>> {
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
                let parts = deno_core::error::format_file_name(file_name, None);
                if let Some(dir) = parts.working_dir_path {
                    out.push_str(&dir);
                }
                out.push_str(&parts.file_name);
            }
            None => {
                if let Some(origin) = self.eval_origin.as_deref().filter(|_| self.is_eval) {
                    let _ = write!(out, "{origin}, ");
                }
                out.push_str("<anonymous>");
            }
        }
        let is_wasm = self
            .file_name
            .as_deref()
            .is_some_and(|f| f.starts_with("wasm://"));
        if is_wasm {
            // V8 reports a wasm frame's function index (1-based) as its line and its byte offset
            // (1-based) as its column; deno_core prints them as the WebAssembly Web API does
            if let Some(line) = self.line {
                let _ = write!(out, ":wasm-function[{}]", line.saturating_sub(1));
            }
            if let Some(column) = self.column {
                let _ = write!(out, ":0x{:x}", column.saturating_sub(1));
            }
        } else if let Some(line) = self.line {
            let _ = write!(out, ":{line}");
            if let Some(column) = self.column {
                let _ = write!(out, ":{column}");
            }
        }
    }
}

/// Settles an asynchronous WebAssembly compile; the promise may already be unusable when the
/// isolate is terminating.
extern "C" fn settle_wasm_compilation(
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
