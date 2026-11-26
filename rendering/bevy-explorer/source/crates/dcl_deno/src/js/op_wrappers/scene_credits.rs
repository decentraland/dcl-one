use std::{cell::RefCell, rc::Rc};

use common::rpc::GrantCreditsAnswer;
use deno_core::{anyhow, op2, OpDecl, OpState};

// list of op declarations
pub fn ops() -> Vec<OpDecl> {
    vec![op_grant_credits()]
}

#[op2(async)]
#[serde]
async fn op_grant_credits(
    state: Rc<RefCell<OpState>>,
    #[string] program_id: String,
    #[string] recipient: String,
    #[string] outcome_id: String,
) -> Result<GrantCreditsAnswer, anyhow::Error> {
    let rx = dcl_js::scene_credits::start_grant_credits(state, program_id, recipient, outcome_id)?;
    Ok(dcl_js::scene_credits::finish_grant_credits(rx).await)
}
