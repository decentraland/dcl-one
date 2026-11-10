use std::{cell::Cell, sync::Once};

use deno_core::{serde_v8, v8, FromV8};
use serde::de::DeserializeOwned;

/// An op argument deserialized from a plain copy of the JS value.
///
/// serde_v8 unwraps every property read, so a getter, proxy or `toString` that throws
/// while it walks a value panics inside the op, which aborts the process. The copy is made
/// with every read checked: own enumerable data only, null-prototype objects and arrays,
/// and no symbols, boxed primitives, proxies or functions. Buffers pass through uncopied
/// and keep their prototype, so the decode itself still runs under `catch_unwind`.
pub struct Plain<T>(pub T);

#[derive(Debug)]
pub struct PlainError(String);

impl std::fmt::Display for PlainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PlainError {}

impl<'a, T: DeserializeOwned> FromV8<'a> for Plain<T> {
    type Error = PlainError;

    fn from_v8(
        scope: &mut v8::HandleScope<'a>,
        value: v8::Local<'a, v8::Value>,
    ) -> Result<Self, Self::Error> {
        let mut budget = MAX_NODES;
        let copy = plain_copy(scope, value, 0, &mut budget)?;
        let scope = &mut v8::TryCatch::new(scope);
        quiet_decode_panics();
        // a getter on a buffer's prototype can decode another op's argument meanwhile
        let outer = DECODING.replace(true);
        // a panic here unwinds through serde_v8's Rust frames only; V8 has returned by then
        let decoded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            serde_v8::from_v8(scope, copy)
        }));
        DECODING.set(outer);
        decoded
            .map_err(|_| PlainError("reading the argument threw".to_owned()))?
            .map(Plain)
            .map_err(|e| PlainError(e.to_string()))
    }
}

thread_local! {
    static DECODING: Cell<bool> = const { Cell::new(false) };
}

// a scene can trigger the caught decode panic at will, so it must not reach the log
fn quiet_decode_panics() {
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if !DECODING.get() {
                previous(info);
            }
        }));
    });
}

const MAX_DEPTH: usize = 64;
// bounds shared subtrees too: `v = [v, v]` repeated stays shallow but doubles per level
const MAX_NODES: usize = 1 << 20;

fn plain_copy<'s>(
    scope: &mut v8::HandleScope<'s>,
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
        || value.is_array_buffer()
        || value.is_array_buffer_view()
    {
        return Ok(value);
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
    let scope = &mut v8::TryCatch::new(scope);
    if let Ok(array) = v8::Local::<v8::Array>::try_from(value) {
        let length = array.length() as usize;
        if length > *budget {
            return Err(PlainError("argument is too large".to_owned()));
        }
        let mut items = Vec::with_capacity(length);
        for index in 0..length as u32 {
            let item = array.get_index(scope, index).ok_or_else(threw)?;
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
        let item = object.get(scope, key.into()).ok_or_else(threw)?;
        let item = plain_copy(scope, item, depth + 1, budget)?;
        copy.create_data_property(scope, key, item)
            .ok_or_else(threw)?;
    }
    Ok(copy.into())
}

fn unsupported(scope: &mut v8::HandleScope, value: v8::Local<v8::Value>) -> PlainError {
    let kind = value.type_of(scope).to_rust_string_lossy(scope);
    PlainError(format!(
        "argument holds a {kind} that cannot be passed to this op"
    ))
}
