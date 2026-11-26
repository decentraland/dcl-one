use deno_core::{op2, v8, OpDecl};

/// deno_core's `op_drain_pending_rejections`, which never drains.
///
/// Upstream holds the rejection queue's borrow while it writes into a fresh array, so a scene
/// setter on `Array.prototype` that rejects a promise re-enters the queue and aborts. The queue
/// is private to deno_core, so this override leaves it to the event loop, which takes it out
/// before handing the rejections to the same JS handler.
pub fn override_ops() -> Vec<OpDecl> {
    vec![op_drain_pending_rejections()]
}

#[op2]
fn op_drain_pending_rejections<'s>(scope: &mut v8::PinScope<'s, '_>) -> v8::Local<'s, v8::Value> {
    v8::undefined(scope).into()
}
