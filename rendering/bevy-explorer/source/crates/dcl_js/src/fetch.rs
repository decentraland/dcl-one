use std::{cell::RefCell, rc::Rc};

use anyhow::anyhow;
use bevy::log::debug;
use common::rpc::{RpcCall, RpcResultReceiver, RpcResultSender};
use sha2::{Digest, Sha256};

use crate::State;
use dcl_types::RpcCalls;

pub type SignedFetchResult = Result<Vec<(String, String)>, String>;
pub type SignedFetchReceiver = RpcResultReceiver<SignedFetchResult>;

/// Hash the scene's UTF-8 body before queuing so only its digest crosses the engine boundary.
pub fn start_signed_fetch_headers(
    state: Rc<RefCell<impl State>>,
    uri: String,
    method: Option<String>,
    body: Option<String>,
) -> Result<SignedFetchReceiver, anyhow::Error> {
    debug!("op_signed_fetch_headers");
    let body_hash = body.map(|body| format!("{:x}", Sha256::digest(body.as_bytes())));
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<RpcCalls>()
        .push(RpcCall::SignRequest(common::rpc::SignRequest {
            method: method.unwrap_or_else(|| String::from("get")),
            uri,
            body_hash,
            response: sx,
        }))?;

    Ok(rx)
}

pub async fn finish_signed_fetch_headers(
    rx: SignedFetchReceiver,
) -> Result<Vec<(String, String)>, anyhow::Error> {
    rx.await?.map_err(|e| anyhow!(e))
}
