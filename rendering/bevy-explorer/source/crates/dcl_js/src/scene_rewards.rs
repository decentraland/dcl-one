use std::{cell::RefCell, rc::Rc};

use bevy::log::debug;
use common::rpc::{
    GrantRewardAnswer, GrantRewardError, GrantRewardRequest, RpcCall, RpcResultReceiver,
    RpcResultSender,
};
use dcl_types::RpcCalls;

use crate::State;

/// Queue `grantReward` for the engine. Only the engine can answer it: it alone knows whether this is an
/// authoritative server holding the scene's Credits delegation.
pub fn start_grant_reward(
    state: Rc<RefCell<impl State>>,
    program_id: String,
    recipient: String,
    outcome_id: String,
) -> Result<RpcResultReceiver<GrantRewardAnswer>, anyhow::Error> {
    debug!("op_grant_reward");
    let (sx, rx) = RpcResultSender::channel();
    state
        .borrow_mut()
        .borrow_mut::<RpcCalls>()
        .push(RpcCall::GrantReward(GrantRewardRequest {
            program_id,
            recipient,
            outcome_id,
            response: sx,
        }))?;
    Ok(rx)
}

/// A dropped answer (the engine stopped serving the scene) is ambiguous: the reward may have been paid.
pub async fn finish_grant_reward(rx: RpcResultReceiver<GrantRewardAnswer>) -> GrantRewardAnswer {
    rx.await
        .unwrap_or_else(|_| GrantRewardAnswer::refused(GrantRewardError::Unavailable))
}
