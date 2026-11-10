use crate::js::errors::OpError;
use crate::js::plain::JsPlain;
use dcl_component::proto_components::sdk::components::PbRealmInfo;
use dcl_js::runtime::{SceneInfoResponse, WorldTime};
use deno_core::{op2, OpDecl, OpState, ToJsBuffer};
use serde::Serialize;
use std::{cell::RefCell, rc::Rc};

// list of op declarations
pub fn ops() -> Vec<OpDecl> {
    vec![
        op_read_file(),
        op_scene_information(),
        op_realm_information(),
        op_world_time(),
        op_camera_fov(),
        op_get_platform(),
        op_get_player_language(),
    ]
}

/// The file's bytes arrive as a `Uint8Array`.
#[derive(Serialize)]
pub(crate) struct ReadFile {
    pub content: ToJsBuffer,
    pub hash: String,
}

#[op2]
async fn op_read_file(
    op_state: Rc<RefCell<OpState>>,
    #[string] filename: String,
) -> Result<JsPlain<ReadFile>, OpError> {
    let file = dcl_js::runtime::op_read_file(op_state, filename).await?;
    Ok(JsPlain(ReadFile {
        content: file.content.into(),
        hash: file.hash,
    }))
}

#[op2]
async fn op_scene_information(
    op_state: Rc<RefCell<OpState>>,
) -> Result<JsPlain<SceneInfoResponse>, OpError> {
    Ok(JsPlain(
        dcl_js::runtime::op_scene_information(op_state).await?,
    ))
}

#[op2]
async fn op_realm_information(
    op_state: Rc<RefCell<OpState>>,
) -> Result<JsPlain<PbRealmInfo>, OpError> {
    Ok(JsPlain(
        dcl_js::runtime::op_realm_information(op_state).await?,
    ))
}

#[op2]
async fn op_world_time(op_state: Rc<RefCell<OpState>>) -> Result<JsPlain<WorldTime>, OpError> {
    Ok(JsPlain(dcl_js::runtime::op_world_time(op_state).await?))
}

#[op2]
async fn op_camera_fov(op_state: Rc<RefCell<OpState>>) -> Result<f32, OpError> {
    Ok(dcl_js::runtime::op_camera_fov(op_state).await?)
}

#[op2]
#[string]
fn op_get_platform(state: &mut OpState) -> String {
    dcl_js::runtime::op_get_platform(state)
}

#[op2]
#[string]
fn op_get_player_language(state: &mut OpState) -> Option<String> {
    dcl_js::runtime::op_get_player_language(state)
}
