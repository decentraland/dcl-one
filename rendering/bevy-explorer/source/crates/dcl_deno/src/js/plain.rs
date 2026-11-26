use deno_core::{serde_v8, v8};
use deno_error::JsErrorBox;

const MAX_DEPTH: usize = 64;

/// An op result handed to scene code as ordinary objects.
///
/// serde_v8 builds every object with a null prototype, so `String(x)` and `"" + x` throw on
/// it. Every object in the result gets `Object.prototype` instead, down to [`MAX_DEPTH`];
/// deeper objects keep the null prototype. Bytes belong in a buffer type (`ToJsBuffer`): a
/// `Vec<u8>` becomes an array of numbers, one property per byte.
pub struct JsPlain<T>(pub T);

impl<'a, T: serde::Serialize> deno_core::ToV8<'a> for JsPlain<T> {
    type Error = JsErrorBox;

    fn to_v8<'i>(
        self,
        scope: &mut v8::PinScope<'a, 'i>,
    ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
        let value =
            serde_v8::to_v8(scope, &self.0).map_err(|e| JsErrorBox::type_error(e.to_string()))?;
        let object_prototype = v8::Object::new(scope)
            .get_prototype(scope)
            .ok_or_else(|| JsErrorBox::type_error("no Object.prototype"))?;
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
