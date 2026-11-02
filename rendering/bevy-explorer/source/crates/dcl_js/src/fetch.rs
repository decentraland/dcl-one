use std::{cell::RefCell, rc::Rc};

use anyhow::anyhow;
use bevy::log::debug;
use common::rpc::{RpcCall, RpcResultReceiver, RpcResultSender};

use crate::State;
use dcl_types::RpcCalls;

pub async fn op_signed_fetch_headers(
    state: Rc<RefCell<impl State>>,
    uri: String,
    method: Option<String>,
    body: Option<String>,
) -> Result<Vec<(String, String)>, anyhow::Error> {
    let rx = start_signed_fetch_headers(state, uri, method, body)?;
    finish_signed_fetch_headers(rx).await
}

/// Queue synchronously so a WASM caller cannot accumulate body copies in pending futures.
pub fn start_signed_fetch_headers(
    state: Rc<RefCell<impl State>>,
    uri: String,
    method: Option<String>,
    body: Option<String>,
) -> Result<RpcResultReceiver<Result<Vec<(String, String)>, String>>, anyhow::Error> {
    debug!("op_signed_fetch_headers");
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<RpcCalls>()
        .push(RpcCall::SignRequest(common::rpc::SignRequest {
            method: method.unwrap_or_else(|| String::from("get")),
            uri,
            body,
            response: sx,
        }))?;

    Ok(rx)
}

pub async fn finish_signed_fetch_headers(
    rx: RpcResultReceiver<Result<Vec<(String, String)>, String>>,
) -> Result<Vec<(String, String)>, anyhow::Error> {
    rx.await?.map_err(|e| anyhow!(e))
}
