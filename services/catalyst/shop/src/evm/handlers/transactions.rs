use axum::Json;
use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::evm::SharedState;
use crate::evm::errors::ApiError;
use crate::evm::transaction::{
    MetaTxSender, ReservationDisposition, TransactionRow, is_paid_call,
    parse_send_transaction_request, reservation_disposition,
};
use crate::evm::upstream::UpstreamForwarder;

/// `POST /transactions`. An error answered before anything was sent, or after a send attempt
/// that proves nothing went out, carries `"not_sent": true`: the only evidence a client takes
/// that it may release the buyer's credit and sign again. Every other failure, and every answer
/// forwarded from an upstream relay, is answered without it.
pub async fn send_transaction(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    match relay(&state, &headers, body).await {
        Ok(response) => response,
        Err(Failure::NotSent(err)) => not_sent(err).await,
        Err(Failure::MaybeSent(err)) => err.into_response(),
    }
}

enum Failure {
    /// Nothing was sent: no send was attempted, or the attempt proves nothing went out.
    NotSent(ApiError),
    /// The transaction may be out.
    MaybeSent(ApiError),
}

async fn relay(
    state: &SharedState,
    headers: &HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Result<Response, Failure> {
    use Failure::NotSent;
    let body = body.map_err(|e| NotSent(ApiError::MalformedBody(e.body_text())))?;
    let tx = parse_send_transaction_request(&body).map_err(NotSent)?;

    let sender = state
        .transaction
        .check_data(&state.settings, &state.contracts, &tx)
        .await
        .map_err(NotSent)?;

    crate::guards::relay::guard_meta_transaction(
        &state.pool,
        state.settings.collections_chain_id,
        sender.as_str(),
        &tx.params[0],
        &tx.params[1],
    )
    .await
    .map_err(NotSent)?;

    let session_id = uuid::Uuid::new_v4().to_string();

    // Sales and credits spends are never held back by the free-call budget.
    if is_paid_call(&state.settings, &tx) {
        state
            .transaction
            .reserve_quota(
                state.settings.max_transactions_per_day,
                &sender,
                &session_id,
                state.settings.collections_chain_id,
                &tx.params[0],
                crate::evm::now(),
            )
            .await
            .map_err(NotSent)?;
    } else {
        state
            .transaction
            .reserve_free_call(
                state.settings.max_transactions_per_day,
                state.settings.max_relayed_transactions_per_day,
                &sender,
                &session_id,
                state.settings.collections_chain_id,
                &tx.params[0],
                crate::evm::now(),
            )
            .await
            .map_err(NotSent)?;
    }

    if let Some(upstream) = state.transaction.upstream_route() {
        return forward_upstream(state, upstream, headers, &body, &sender, &session_id).await;
    }

    let tx_hash = match state.transaction.send_meta_transaction(&tx).await {
        Ok(hash) => hash,
        Err(err) => return Err(failed_send(state, &session_id, &sender, err).await),
    };

    // The transaction is out, so the answer carries its hash whatever recording it does. An error
    // here would tell the client nothing was sent, and it would sign again while this one lands. A
    // reservation left without its hash still counts against the buyer's daily quota.
    if let Err(err) = state
        .transaction
        .confirm_reservation(&session_id, &tx_hash)
        .await
    {
        tracing::error!(
            tx_hash = %tx_hash,
            error = %err,
            "economy: relayed a transaction but could not record its hash on the quota reservation"
        );
    }

    Ok(Json(json!({ "ok": true, "txHash": tx_hash })).into_response())
}

/// A failed send or forward: the slot is released, and the answer says nothing was sent, only
/// for an error that proves it (`reservation_disposition`).
async fn failed_send(
    state: &SharedState,
    session_id: &str,
    sender: &MetaTxSender,
    err: ApiError,
) -> Failure {
    match reservation_disposition(&err) {
        ReservationDisposition::Release => {
            release_quota_slot(state, session_id, sender).await;
            Failure::NotSent(err)
        }
        ReservationDisposition::Keep => Failure::MaybeSent(err),
    }
}

/// The error's envelope with `"not_sent": true` added (`marked_not_sent`).
async fn not_sent(err: ApiError) -> Response {
    let (mut parts, body) = err.into_response().into_parts();
    let bytes = axum::body::to_bytes(body, 64 * 1024)
        .await
        .unwrap_or_default();
    let body = match marked_not_sent(&bytes) {
        Some(marked) => {
            parts.headers.remove(header::CONTENT_LENGTH);
            marked
        }
        None => bytes.to_vec(),
    };
    Response::from_parts(parts, axum::body::Body::from(body))
}

/// An error body with `"not_sent": true` added, or `None` for a body that is not a JSON object:
/// that one is answered as it is, without the evidence, which a client reads as maybe sent.
fn marked_not_sent(body: &[u8]) -> Option<Vec<u8>> {
    match serde_json::from_slice::<serde_json::Value>(body) {
        Ok(serde_json::Value::Object(mut envelope)) => {
            envelope.insert("not_sent".into(), serde_json::Value::Bool(true));
            serde_json::to_vec(&envelope).ok()
        }
        _ => None,
    }
}

/// Forwards to the upstream relay. Its answer passes through as it is: whatever its status, it is
/// never evidence from this relay that nothing was sent. Only a forward that never reached the
/// upstream (`RelayerUnavailable`) is answered as not sent.
async fn forward_upstream(
    state: &SharedState,
    upstream: &UpstreamForwarder,
    headers: &HeaderMap,
    body: &[u8],
    sender: &MetaTxSender,
    session_id: &str,
) -> Result<Response, Failure> {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());

    let forwarded = match upstream.forward(body, content_type).await {
        Ok(f) => f,
        Err(err) => return Err(failed_send(state, session_id, sender, err).await),
    };

    if forwarded.is_success() {
        match forwarded.tx_hash() {
            Some(hash) => {
                if let Err(e) = state
                    .transaction
                    .confirm_reservation(session_id, &hash)
                    .await
                {
                    tracing::error!(
                        session_id = %session_id,
                        user_address = %sender.as_str(),
                        error = %e,
                        "failed to confirm reservation after an upstream broadcast"
                    );
                }
            }
            None => tracing::warn!(
                session_id = %session_id,
                user_address = %sender.as_str(),
                "upstream 2xx response carries no txHash; the quota slot stays consumed"
            ),
        }
    } else if refunds_quota_slot(forwarded.status()) {
        release_quota_slot(state, session_id, sender).await;
    }

    Ok(forwarded.into_response())
}

/// Whether a non-2xx upstream status proves the transaction was never
/// broadcast, so the quota slot can be refunded. This is the relay's own
/// quota decision; the answer passes through unchanged and is never marked
/// `not_sent` for a client. The evidence for a 4xx is
/// the transactions-server's own code, as inspected at
/// `decentraland/transactions-server@8b9549bbe51a8bf14da5efbe0847da18e9b208bd`
/// (`sendTransaction` in `src/controllers/handlers.ts`, the OpenZeppelin port
/// in `src/ports/openzeppelin/components.ts`): it answers a 4xx only for
/// validation, quota and rate limits, and for a relayer that created no
/// transaction (a 400/422 to the submission, or a terminal status whose
/// `hash` is `null`). It reports a failure after a submission (a terminal
/// status with a hash, a timeout) as a 5xx. That covers the transactions-server
/// itself, not a 4xx from an intermediary in front of it. Every 5xx is
/// indeterminate: the upstream sits behind Cloudflare, so 502/504 and
/// Cloudflare's own 522/524 arrive after the request reached the intermediary,
/// and even a 500/503 can be emitted by the origin after it already submitted
/// the transaction. A kept slot merely ages out of the 24h quota window; a
/// false refund breaks the daily-quota invariant, so every 5xx keeps the slot.
/// So do a 408 and a 499 (`a_hop_gave_up`): a proxy can answer either after it
/// forwarded the request, and the status does not say who answered.
fn refunds_quota_slot(status: u16) -> bool {
    (400..500).contains(&status) && !crate::evm::relayer::a_hop_gave_up(status)
}

async fn release_quota_slot(state: &SharedState, session_id: &str, sender: &MetaTxSender) {
    if let Err(release_err) = state.transaction.release_reservation(session_id).await {
        tracing::error!(
            session_id = %session_id,
            user_address = %sender.as_str(),
            error = %release_err,
            "failed to release reservation after a pre-broadcast failure"
        );
    }
}

pub async fn get_user_transactions(
    State(state): State<SharedState>,
    Path(user_address): Path<String>,
) -> Result<Json<Vec<TransactionRow>>, ApiError> {
    let rows = state
        .transaction
        .get_by_user_address(&user_address.to_lowercase())
        .await?;
    Ok(Json(rows))
}

#[cfg(test)]
mod tests {
    use super::{marked_not_sent, refunds_quota_slot};

    #[test]
    fn only_a_json_object_body_is_marked_not_sent() {
        let marked =
            marked_not_sent(br#"{"ok":false,"error":"invalid_transaction","message":"m"}"#)
                .expect("an error envelope is marked");
        let marked: serde_json::Value = serde_json::from_slice(&marked).unwrap();
        assert_eq!(
            marked,
            serde_json::json!({"ok": false, "error": "invalid_transaction", "message": "m", "not_sent": true})
        );
        for body in [
            &b""[..],
            b"Bad Gateway",
            b"[]",
            br#""ok""#,
            b"null",
            b"{\"ok\":false",
        ] {
            assert_eq!(
                marked_not_sent(body),
                None,
                "{}",
                String::from_utf8_lossy(body)
            );
        }
    }

    #[test]
    fn only_4xx_upstream_statuses_refund_the_quota_slot() {
        for status in [400u16, 404, 422, 429] {
            assert!(
                refunds_quota_slot(status),
                "{status} is a pre-broadcast rejection and refunds the slot"
            );
        }
        for status in [408u16, 499, 500, 501, 502, 503, 504, 520, 522, 524] {
            assert!(
                !refunds_quota_slot(status),
                "{status} is broadcast-indeterminate and keeps the slot"
            );
        }
    }
}
