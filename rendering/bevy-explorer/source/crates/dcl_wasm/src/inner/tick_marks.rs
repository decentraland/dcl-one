//! MEASUREMENT ONLY (local measurement build, not for merge): stage marks of the movement scene's
//! tick on a clock the workers share, `performance.timeOrigin + performance.now()` in ms.
//!
//! The engine names the movement scene and numbers its dispatches here. The scene's relay (A,
//! which runs on engine memory and so sees these statics) stamps when it took a dispatch off the
//! engine channel and when it forwarded the reply, adds the scene worker's own marks (onUpdate,
//! reply put: `relay_tick_mark` in sandbox_worker.js), and publishes them in a slot keyed by the
//! dispatch's number before the reply goes out. The engine reads the slot when the reply arrives.
//! A scene has at most one tick in flight, so the dispatch number the relay reads when it takes a
//! tick is that tick's.

use std::sync::atomic::{AtomicU64, Ordering};

use wasm_bindgen::{JsCast, JsValue};

static MOVEMENT_SCENE: AtomicU64 = AtomicU64::new(u64::MAX);
static DISPATCH_SEQ: AtomicU64 = AtomicU64::new(0);
// every relay's steps since startup: [waited for the engine's send, slept]
static RELAY_STEPS: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];

/// Relay: whether this step waits for the engine's send (`relay_wake_mode`) or sleeps.
pub fn count_relay_step(waits: bool) {
    RELAY_STEPS[usize::from(!waits)].fetch_add(1, Ordering::Relaxed);
}

/// Engine: every relay's steps since startup, (waited for the engine's send, slept).
pub fn relay_steps() -> (u64, u64) {
    (
        RELAY_STEPS[0].load(Ordering::Relaxed),
        RELAY_STEPS[1].load(Ordering::Relaxed),
    )
}

/// The relay's marks of one tick, in this order; ms on the scene worker's shared clock unless
/// named otherwise, NaN when not taken.
pub const RELAY_NAMES: [&str; RELAY_FIELDS] = [
    // start of the relay pump that took the dispatch
    "pick_pump",
    // ms the relay slept before that pump (since the previous pump ended)
    "pick_slept",
    // when it took the dispatch off the engine channel and handed it to the scene runtime
    "pick",
    // scene worker: end of the previous onUpdate (it ends once this dispatch reached it)
    "b_prev_end",
    // scene worker: start of the onUpdate that produced the reply
    "b_start",
    // scene worker: when op_crdt_send_to_renderer returned with the reply
    "b_put",
    // scene worker: its onUpdate count at b_start
    "b_n",
    // start of the relay pump that forwarded the reply
    "fwd_pump",
    // ms the relay slept before that pump
    "fwd_slept",
    // when the relay forwarded the reply to the engine
    "fwd",
    // relay pumps from the one that took the dispatch to the one that forwarded, both included
    "pumps",
    // 1 when the reply waited in the relay for the engine's tick credit
    "held",
];
pub const RELAY_FIELDS: usize = 12;
pub const PICK_PUMP: usize = 0;
pub const PICK_SLEPT: usize = 1;
pub const PICK: usize = 2;
pub const B_PREV_END: usize = 3;
pub const B_START: usize = 4;
pub const B_PUT: usize = 5;
pub const B_N: usize = 6;
pub const FWD_PUMP: usize = 7;
pub const FWD_SLEPT: usize = 8;
pub const FWD: usize = 9;
pub const PUMPS: usize = 10;
pub const HELD: usize = 11;

const SLOTS: usize = 8;
const EMPTY: u64 = u64::MAX;

struct Slot {
    seq: AtomicU64,
    fields: [AtomicU64; RELAY_FIELDS],
}

impl Slot {
    const fn new() -> Self {
        Self {
            seq: AtomicU64::new(EMPTY),
            fields: [const { AtomicU64::new(0) }; RELAY_FIELDS],
        }
    }
}

static TABLE: [Slot; SLOTS] = [const { Slot::new() }; SLOTS];

/// Engine: the movement scene this frame (its scene id's bits), or none.
pub fn set_movement_scene(bits: Option<u64>) {
    MOVEMENT_SCENE.store(bits.unwrap_or(EMPTY), Ordering::Relaxed);
}

/// Relay: whether the scene it relays is the movement scene.
pub fn is_movement_scene(bits: u64) -> bool {
    bits != EMPTY && MOVEMENT_SCENE.load(Ordering::Relaxed) == bits
}

/// Engine: the number of the movement scene's dispatch about to be sent.
pub fn set_dispatch_seq(seq: u64) {
    DISPATCH_SEQ.store(seq, Ordering::Release);
}

/// Relay: the number of the movement scene's last dispatch.
pub fn dispatch_seq() -> u64 {
    DISPATCH_SEQ.load(Ordering::Acquire)
}

/// Relay: publishes a tick's marks, before it forwards the reply.
pub fn publish(seq: u64, fields: &[f64; RELAY_FIELDS]) {
    let slot = &TABLE[(seq % SLOTS as u64) as usize];
    slot.seq.store(EMPTY, Ordering::Relaxed);
    for (cell, value) in slot.fields.iter().zip(fields) {
        cell.store(value.to_bits(), Ordering::Relaxed);
    }
    slot.seq.store(seq, Ordering::Release);
}

/// Engine: the relay's marks of dispatch `seq`, once its reply arrived.
pub fn read(seq: u64) -> Option<[f64; RELAY_FIELDS]> {
    let slot = &TABLE[(seq % SLOTS as u64) as usize];
    if slot.seq.load(Ordering::Acquire) != seq {
        return None;
    }
    let mut fields = [f64::NAN; RELAY_FIELDS];
    for (value, cell) in fields.iter_mut().zip(&slot.fields) {
        *value = f64::from_bits(cell.load(Ordering::Relaxed));
    }
    (slot.seq.load(Ordering::Acquire) == seq).then_some(fields)
}

fn performance() -> Option<web_sys::Performance> {
    js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("performance"))
        .ok()
        .and_then(|p| p.dyn_into().ok())
}

thread_local! {
    static PERFORMANCE: Option<(web_sys::Performance, f64)> =
        performance().map(|p| { let origin = p.time_origin(); (p, origin) });
}

/// Engine (a thread with the glue): the shared clock in ms. Never call it from the relay, which
/// has no glue: it reads `relay_now`.
pub fn engine_now_ms() -> f64 {
    PERFORMANCE.with(|p| p.as_ref().map_or(f64::NAN, |(p, origin)| origin + p.now()))
}

fn get(target: &JsValue, key: &str) -> Option<JsValue> {
    js_sys::Reflect::get(target, &JsValue::from_str(key)).ok()
}

/// Engine: (`performance.memory.usedJSHeapSize`, when the browser has it; the engine's wasm
/// memory), in MB.
pub fn heap_mb() -> (Option<f64>, Option<f64>) {
    let js = PERFORMANCE.with(|p| {
        p.as_ref()
            .and_then(|(p, _)| get(p, "memory"))
            .filter(|m| m.is_object())
            .and_then(|m| get(&m, "usedJSHeapSize"))
            .and_then(|v| v.as_f64())
    });
    let wasm = get(&wasm_bindgen::memory(), "buffer")
        .and_then(|b| get(&b, "byteLength"))
        .and_then(|v| v.as_f64());
    (js.map(|b| b / 1048576.0), wasm.map(|b| b / 1048576.0))
}
