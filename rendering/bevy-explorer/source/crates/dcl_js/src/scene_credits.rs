use std::{cell::RefCell, rc::Rc};

use bevy::log::debug;
use common::rpc::{
    GrantCreditsAnswer, GrantCreditsError, GrantCreditsRequest, RpcCall, RpcResultReceiver,
    RpcResultSender,
};
use dcl_types::RpcCalls;

use crate::State;

/// Queue `grantCredits` for the engine. Only the engine can answer it: it alone knows whether this is an
/// authoritative server holding the scene's Credits delegation.
pub fn start_grant_credits(
    state: Rc<RefCell<impl State>>,
    program_id: String,
    recipient: String,
    outcome_id: String,
) -> Result<RpcResultReceiver<GrantCreditsAnswer>, anyhow::Error> {
    debug!("op_grant_credits");
    let (sx, rx) = RpcResultSender::channel();
    state
        .borrow_mut()
        .borrow_mut::<RpcCalls>()
        .push(RpcCall::GrantCredits(GrantCreditsRequest {
            program_id,
            recipient,
            outcome_id,
            response: sx,
        }))?;
    Ok(rx)
}

/// A dropped answer (the engine stopped serving the scene) is ambiguous: the Credits may have been paid.
pub async fn finish_grant_credits(rx: RpcResultReceiver<GrantCreditsAnswer>) -> GrantCreditsAnswer {
    rx.await
        .unwrap_or_else(|_| GrantCreditsAnswer::refused(GrantCreditsError::Unavailable))
}
