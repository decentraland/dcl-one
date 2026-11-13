//! Production-accepted commands are dispatched only from the captured head.
use crate::store::{self, Outcome};
use anyhow::Result;
use reqwest::{Client, StatusCode};
use sqlx::PgPool;

/// Transport errors/timeouts leave the command pending: the follower may have
/// committed. Retry the same identity; never infer failure or repair from HTTP.
pub async fn process_next(
    pool: &PgPool,
    stream: &str,
    client: &Client,
    endpoint: &str,
    token: &str,
) -> Result<Outcome> {
    let outcome = store::process_next(pool, stream).await?;
    if outcome != Outcome::AwaitingCommand {
        return Ok(outcome);
    }
    let Some(pointer) = store::pending_command(pool, stream).await? else {
        return Ok(Outcome::Idle);
    };
    match client
        .post(endpoint)
        .bearer_auth(token)
        .json(&pointer)
        .send()
        .await
    {
        Ok(response)
            if response.status().is_client_error()
                && response.status() != StatusCode::REQUEST_TIMEOUT
                && response.status() != StatusCode::TOO_MANY_REQUESTS =>
        {
            store::reject_command(pool, &pointer).await?;
        }
        Ok(_) | Err(_) => {}
    }
    // The DB receipt, never a claimed HTTP success, is the execution evidence.
    store::process_next(pool, stream).await
}
