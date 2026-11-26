//! What the crate's tests in headless Chrome (the Bevy Explorer CI job "Relay wake in the
//! browser") share: the views a worker's `Atomics` calls take, and the fields of their results.

use std::sync::atomic::AtomicI32;

use js_sys::{Int32Array, Reflect, WebAssembly};
use wasm_bindgen::{JsCast, JsValue};

/// A view of one word of the shared memory, as a worker waits on it (sandbox_worker.js
/// `wakeView`).
pub fn word_view(word: &AtomicI32) -> Int32Array {
    let memory = wasm_bindgen::memory().unchecked_into::<WebAssembly::Memory>();
    let address = word as *const AtomicI32 as usize as u32;
    Int32Array::new_with_byte_offset_and_length(&memory.buffer(), address, 1)
}

/// A field of a JS object.
pub fn field(object: &JsValue, name: &str) -> JsValue {
    Reflect::get(object, &JsValue::from_str(name)).unwrap()
}
