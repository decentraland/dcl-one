use crate::js::errors::OpError;
use dcl_component::proto_components::common::Vector3 as DclVector3;
use dcl_js::restricted_actions::UiFocusResult;
use deno_core::{op2, OpDecl, OpState};
use std::{cell::RefCell, rc::Rc};

use crate::js::plain::Plain;

// list of op declarations
pub fn ops() -> Vec<OpDecl> {
    vec![
        op_move_player_to(),
        op_walk_player_to(),
        op_teleport_to(),
        op_change_realm(),
        op_external_url(),
        op_emote(),
        op_stop_emote(),
        op_scene_emote(),
        op_open_nft_dialog(),
        op_open_explorer_ui(),
        op_ui_focus(),
        op_copy_to_clipboard(),
    ]
}

#[op2]
async fn op_move_player_to(
    state: Rc<RefCell<OpState>>,
    #[scoped] position: Plain<DclVector3>,
    #[scoped] camera_target: Plain<Option<DclVector3>>,
    #[scoped] avatar_target: Plain<Option<DclVector3>>,
    duration: Option<f32>,
) -> Result<bool, OpError> {
    Ok(dcl_js::restricted_actions::op_move_player_to(
        state,
        position.0,
        camera_target.0,
        avatar_target.0,
        duration,
    )
    .await?)
}

#[op2]
async fn op_walk_player_to(
    state: Rc<RefCell<OpState>>,
    #[scoped] position: Plain<DclVector3>,
    stop_threshold: f32,
    timeout: Option<f32>,
) -> Result<bool, OpError> {
    Ok(
        dcl_js::restricted_actions::op_walk_player_to(state, position.0, stop_threshold, timeout)
            .await?,
    )
}

#[op2]
async fn op_teleport_to(
    state: Rc<RefCell<OpState>>,
    position_x: Option<i32>,
    position_y: Option<i32>,
    #[string] realm: Option<String>,
) -> Result<bool, OpError> {
    Ok(dcl_js::restricted_actions::op_teleport_to(state, position_x, position_y, realm).await?)
}

#[op2]
async fn op_change_realm(
    state: Rc<RefCell<OpState>>,
    #[string] realm: String,
    #[string] message: Option<String>,
) -> Result<bool, OpError> {
    Ok(dcl_js::restricted_actions::op_change_realm(state, realm, message).await?)
}

#[op2]
async fn op_external_url(
    state: Rc<RefCell<OpState>>,
    #[string] url: String,
) -> Result<bool, OpError> {
    Ok(dcl_js::restricted_actions::op_external_url(state, url).await?)
}

#[op2(fast)]
fn op_emote(
    op_state: &mut OpState,
    #[string] emote: String,
    upper_body: bool,
) -> Result<(), OpError> {
    Ok(dcl_js::restricted_actions::op_emote(
        op_state, emote, upper_body,
    )?)
}

#[op2(fast)]
fn op_stop_emote(op_state: &mut OpState) -> Result<(), OpError> {
    Ok(dcl_js::restricted_actions::op_stop_emote(op_state)?)
}

#[op2]
async fn op_scene_emote(
    op_state: Rc<RefCell<OpState>>,
    #[string] emote: String,
    looping: bool,
    upper_body: bool,
) -> Result<(), OpError> {
    Ok(dcl_js::restricted_actions::op_scene_emote(op_state, emote, looping, upper_body).await?)
}

#[op2]
async fn op_open_nft_dialog(
    op_state: Rc<RefCell<OpState>>,
    #[string] urn: String,
) -> Result<(), OpError> {
    Ok(dcl_js::restricted_actions::op_open_nft_dialog(op_state, urn).await?)
}

#[op2]
async fn op_open_explorer_ui(op_state: Rc<RefCell<OpState>>, ui: i32) -> Result<i32, OpError> {
    Ok(dcl_js::restricted_actions::op_open_explorer_ui(op_state, ui).await?)
}

#[op2]
#[serde]
async fn op_ui_focus(
    op_state: Rc<RefCell<OpState>>,
    apply: bool,
    #[string] element_id: Option<String>,
) -> Result<UiFocusResult, OpError> {
    Ok(dcl_js::restricted_actions::op_ui_focus(op_state, apply, element_id).await?)
}

#[op2]
async fn op_copy_to_clipboard(
    op_state: Rc<RefCell<OpState>>,
    #[string] text: String,
) -> Result<(), OpError> {
    Ok(dcl_js::restricted_actions::op_copy_to_clipboard(op_state, text).await?)
}
