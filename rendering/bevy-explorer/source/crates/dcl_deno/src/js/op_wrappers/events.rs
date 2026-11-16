use dcl_js::events::{Event, LegacyAvatarEvent};
use deno_core::{anyhow, op2, OpDecl, OpState};
use std::{cell::RefCell, rc::Rc};

// list of op declarations
pub fn ops() -> Vec<OpDecl> {
    vec![
        op_subscribe(),
        op_unsubscribe(),
        op_send_batch(),
        op_subscribe_legacy_avatar_events(),
        op_pull_legacy_avatar_events(),
    ]
}

#[op2(fast)]
fn op_subscribe(state: &mut OpState, #[string] id: &str) -> Result<(), anyhow::Error> {
    dcl_js::events::op_subscribe(state, id)
}

#[op2(fast)]
fn op_unsubscribe(state: &mut OpState, #[string] id: &str) {
    dcl_js::events::op_unsubscribe(state, id)
}

#[op2]
#[serde]
fn op_send_batch(state: &mut OpState) -> Vec<Event> {
    dcl_js::events::op_send_batch(state)
}

#[op2(fast)]
fn op_subscribe_legacy_avatar_events(state: &mut OpState) -> Result<(), anyhow::Error> {
    dcl_js::events::op_subscribe_legacy_avatar_events(state)
}

#[op2(async)]
#[serde]
async fn op_pull_legacy_avatar_events(
    state: Rc<RefCell<OpState>>,
) -> Result<Vec<LegacyAvatarEvent>, anyhow::Error> {
    dcl_js::events::op_pull_legacy_avatar_events(state).await
}
