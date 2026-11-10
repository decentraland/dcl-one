use std::{cell::Cell, sync::Once};

use deno_core::{serde_v8, v8, FromV8};
use serde::de::DeserializeOwned;

/// An op argument deserialized from a plain copy of the JS value.
///
/// serde_v8 unwraps every property read, so a getter, proxy or `toString` that throws
/// while it walks a value panics inside the op, which aborts the process. The copy is made
/// with every read checked: own enumerable data properties only (no getter runs),
/// null-prototype objects and arrays, and no symbols, boxed primitives, proxies, functions, or
/// objects whose data lives in internal slots (`Date`, `Map`, `Set`, `RegExp`, ...). Buffers
/// are re-wrapped over the same memory with a null prototype, so no scene code runs while
/// serde_v8 decodes; the decode still runs under `catch_unwind`.
pub struct Plain<T>(pub T);

#[derive(Debug, deno_error::JsError)]
#[class(type)]
pub struct PlainError(String);

impl std::fmt::Display for PlainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PlainError {}

impl<'a, T: DeserializeOwned> FromV8<'a> for Plain<T> {
    type Error = PlainError;

    fn from_v8<'i>(
        scope: &mut v8::PinScope<'a, 'i>,
        value: v8::Local<'a, v8::Value>,
    ) -> Result<Self, Self::Error> {
        let mut budget = MAX_NODES;
        let copy = plain_copy(scope, value, 0, &mut budget)?;
        v8::tc_scope!(let scope, scope);
        quiet_decode_panics();
        DECODING.set(true);
        // a panic here unwinds through serde_v8's Rust frames only; V8 has returned by then
        let decoded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            serde_v8::from_v8(scope, copy)
        }));
        DECODING.set(false);
        decoded
            .map_err(|_| PlainError("reading the argument threw".to_owned()))?
            .map(Plain)
            .map_err(|e| PlainError(e.to_string()))
    }
}

thread_local! {
    static DECODING: Cell<bool> = const { Cell::new(false) };
}

// a scene can trigger the caught decode panic at will, so it must not reach the log; any
// other panic still does
fn quiet_decode_panics() {
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let in_serde_v8 = info
                .location()
                .is_some_and(|location| location.file().contains("serde_v8"));
            if !(DECODING.get() && in_serde_v8) {
                previous(info);
            }
        }));
    });
}

const MAX_DEPTH: usize = 64;
// bounds shared subtrees too: `v = [v, v]` repeated stays shallow but doubles per level
const MAX_NODES: usize = 1 << 20;

fn plain_copy<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    value: v8::Local<'s, v8::Value>,
    depth: usize,
    budget: &mut usize,
) -> Result<v8::Local<'s, v8::Value>, PlainError> {
    let threw = || PlainError("reading the argument threw".to_owned());
    if depth > MAX_DEPTH {
        return Err(PlainError("argument is nested too deeply".to_owned()));
    }
    *budget = budget
        .checked_sub(1)
        .ok_or_else(|| PlainError("argument is too large".to_owned()))?;
    if value.is_null_or_undefined()
        || value.is_boolean()
        || value.is_number()
        || value.is_string()
        || value.is_big_int()
    {
        return Ok(value);
    }
    if value.is_array_buffer() || value.is_array_buffer_view() {
        let copy = rewrap_buffer(scope, value).ok_or_else(|| unsupported(scope, value))?;
        let null = v8::null(scope).into();
        copy.set_prototype(scope, null)
            .ok_or_else(|| PlainError("reading the argument threw".to_owned()))?;
        return Ok(copy.into());
    }
    // boxed primitives convert by calling into JS
    if value.is_symbol()
        || value.is_proxy()
        || value.is_function()
        || value.is_string_object()
        || value.is_number_object()
        || value.is_boolean_object()
        || value.is_big_int_object()
        || value.is_symbol_object()
    {
        return Err(unsupported(scope, value));
    }
    if let Some(kind) = slot_object_kind(&value) {
        return Err(PlainError(format!(
            "argument holds a {kind} that cannot be passed to this op"
        )));
    }
    v8::tc_scope!(let scope, scope);
    if let Ok(array) = v8::Local::<v8::Array>::try_from(value) {
        let length = array.length() as usize;
        if length > *budget {
            return Err(PlainError("argument is too large".to_owned()));
        }
        let mut items = Vec::with_capacity(length);
        for index in 0..length as u32 {
            let key = v8::Integer::new_from_unsigned(scope, index)
                .to_string(scope)
                .ok_or_else(threw)?;
            let item = own_data(scope, array.into(), key.into())?;
            items.push(plain_copy(scope, item, depth + 1, budget)?);
        }
        let copy = v8::Array::new_with_elements(scope, &items);
        let null = v8::null(scope).into();
        copy.set_prototype(scope, null).ok_or_else(threw)?;
        return Ok(copy.into());
    }
    let Ok(object) = v8::Local::<v8::Object>::try_from(value) else {
        return Err(unsupported(scope, value));
    };
    let keys = object
        .get_own_property_names(
            scope,
            v8::GetPropertyNamesArgs {
                property_filter: v8::PropertyFilter::ONLY_ENUMERABLE
                    | v8::PropertyFilter::SKIP_SYMBOLS,
                key_conversion: v8::KeyConversionMode::ConvertToString,
                ..Default::default()
            },
        )
        .ok_or_else(threw)?;
    let null = v8::null(scope).into();
    let copy = v8::Object::with_prototype_and_properties(scope, null, &[], &[]);
    for index in 0..keys.length() {
        let key = keys.get_index(scope, index).ok_or_else(threw)?;
        let Ok(key) = v8::Local::<v8::Name>::try_from(key) else {
            return Err(threw());
        };
        let item = own_data(scope, object, key)?;
        let item = plain_copy(scope, item, depth + 1, budget)?;
        copy.create_data_property(scope, key, item)
            .ok_or_else(threw)?;
    }
    Ok(copy.into())
}

/// The value of an own data property, read without running scene code: an accessor is
/// refused and a missing property (an array hole) is undefined.
fn own_data<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    object: v8::Local<'s, v8::Object>,
    key: v8::Local<'s, v8::Name>,
) -> Result<v8::Local<'s, v8::Value>, PlainError> {
    let threw = || PlainError("reading the argument threw".to_owned());
    let descriptor = object
        .get_own_property_descriptor(scope, key)
        .ok_or_else(threw)?;
    let Ok(descriptor) = v8::Local::<v8::Object>::try_from(descriptor) else {
        return Ok(v8::undefined(scope).into());
    };
    let name = |scope: &mut v8::PinScope<'s, '_>,
                name: &str|
     -> Result<v8::Local<'s, v8::Name>, PlainError> {
        Ok(v8::String::new(scope, name).ok_or_else(threw)?.into())
    };
    for accessor in ["get", "set"] {
        let accessor = name(scope, accessor)?;
        if descriptor.has_own_property(scope, accessor) != Some(false) {
            return Err(PlainError(
                "argument has a getter or setter, which cannot be passed to this op".to_owned(),
            ));
        }
    }
    let value = name(scope, "value")?;
    descriptor.get(scope, value.into()).ok_or_else(threw)
}

/// Objects whose contents have no own properties, so a copy would silently be `{}`.
fn slot_object_kind(value: &v8::Value) -> Option<&'static str> {
    [
        (value.is_date(), "Date"),
        (value.is_map(), "Map"),
        (value.is_set(), "Set"),
        (value.is_reg_exp(), "RegExp"),
        (value.is_weak_map(), "WeakMap"),
        (value.is_weak_set(), "WeakSet"),
        (value.is_promise(), "Promise"),
    ]
    .into_iter()
    .find_map(|(is, kind)| is.then_some(kind))
}

/// A new buffer or view of the same type over the same memory.
fn rewrap_buffer<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    value: v8::Local<'s, v8::Value>,
) -> Option<v8::Local<'s, v8::Object>> {
    if let Ok(buffer) = v8::Local::<v8::ArrayBuffer>::try_from(value) {
        let store = buffer.get_backing_store();
        return Some(v8::ArrayBuffer::with_backing_store(scope, &store).into());
    }
    let view = v8::Local::<v8::ArrayBufferView>::try_from(value).ok()?;
    let buffer = view.buffer(scope).filter(|buffer| !buffer.was_detached())?;
    let offset = view.byte_offset();
    macro_rules! same_type {
        ($($ty:ident),*) => {$(
            if let Ok(typed) = v8::Local::<v8::$ty>::try_from(value) {
                return v8::$ty::new(scope, buffer, offset, typed.length()).map(Into::into);
            }
        )*};
    }
    same_type!(
        Uint8Array,
        Uint8ClampedArray,
        Int8Array,
        Uint16Array,
        Int16Array,
        Uint32Array,
        Int32Array,
        Float32Array,
        Float64Array,
        BigUint64Array,
        BigInt64Array
    );
    if value.is_data_view() {
        return Some(v8::DataView::new(scope, buffer, offset, view.byte_length()).into());
    }
    None
}

fn unsupported(scope: &mut v8::PinScope, value: v8::Local<v8::Value>) -> PlainError {
    let kind = value.type_of(scope).to_rust_string_lossy(scope);
    PlainError(format!(
        "argument holds a {kind} that cannot be passed to this op"
    ))
}

/// An op result handed to scene code as ordinary objects.
///
/// serde_v8 builds every object with a null prototype, so `String(x)` and `"" + x` throw on
/// it. Every object in the result gets `Object.prototype` instead, down to [`MAX_DEPTH`];
/// deeper objects keep the null prototype. Bytes belong in a buffer type (`ToJsBuffer`): a
/// `Vec<u8>` becomes an array of numbers, one property per byte.
pub struct JsPlain<T>(pub T);

impl<'a, T: serde::Serialize> deno_core::ToV8<'a> for JsPlain<T> {
    type Error = PlainError;

    fn to_v8<'i>(
        self,
        scope: &mut v8::PinScope<'a, 'i>,
    ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
        let value = serde_v8::to_v8(scope, &self.0).map_err(|e| PlainError(e.to_string()))?;
        let object_prototype = v8::Object::new(scope)
            .get_prototype(scope)
            .ok_or_else(|| PlainError("no Object.prototype".to_owned()))?;
        adopt_prototype(scope, value, object_prototype, 0);
        Ok(value)
    }
}

/// Walks a fresh serde_v8 result: plain data properties only, so no read runs JS.
fn adopt_prototype<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    value: v8::Local<'s, v8::Value>,
    prototype: v8::Local<'s, v8::Value>,
    depth: usize,
) {
    if depth > MAX_DEPTH || value.is_array_buffer() || value.is_array_buffer_view() {
        return;
    }
    // a scope per item keeps the live handles bounded by the depth, not the size
    if let Ok(array) = v8::Local::<v8::Array>::try_from(value) {
        for index in 0..array.length() {
            v8::scope!(let scope, scope);
            if let Some(item) = array.get_index(scope, index) {
                adopt_prototype(scope, item, prototype, depth + 1);
            }
        }
        return;
    }
    let Ok(object) = v8::Local::<v8::Object>::try_from(value) else {
        return;
    };
    if object.get_prototype(scope).is_some_and(|p| p.is_null()) {
        object.set_prototype(scope, prototype);
    }
    let Some(keys) = object.get_own_property_names(scope, Default::default()) else {
        return;
    };
    for index in 0..keys.length() {
        v8::scope!(let scope, scope);
        let Some(key) = keys.get_index(scope, index) else {
            continue;
        };
        if let Some(item) = object.get(scope, key) {
            adopt_prototype(scope, item, prototype, depth + 1);
        }
    }
}
