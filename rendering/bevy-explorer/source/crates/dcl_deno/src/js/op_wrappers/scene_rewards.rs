use std::{cell::RefCell, rc::Rc};

use common::rpc::GrantRewardAnswer;
use deno_core::{anyhow, op2, OpDecl, OpState};

// list of op declarations
pub fn ops() -> Vec<OpDecl> {
    vec![op_grant_reward()]
}

#[op2(async)]
#[serde]
async fn op_grant_reward(
    state: Rc<RefCell<OpState>>,
    #[string] program_id: String,
    #[string] recipient: String,
    #[string] outcome_id: String,
) -> Result<GrantRewardAnswer, anyhow::Error> {
    let rx = dcl_js::scene_rewards::start_grant_reward(state, program_id, recipient, outcome_id)?;
    Ok(dcl_js::scene_rewards::finish_grant_reward(rx).await)
}
