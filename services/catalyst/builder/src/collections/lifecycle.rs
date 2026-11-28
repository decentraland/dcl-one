//! The check a standard save that changes a collection's address passes, as
//! builder-server answers it: the Polygon RPC ([`super::deployment`]) is read
//! before the save's transaction, never while it holds a connection.

use crate::State;
use crate::access::CollectionEdit;
use crate::collections::deployment::Onchain;
use crate::http::{ApiError, Subject};

const CHAIN_UNKNOWN: &str = "The collection's chain state is not available. Try again later.";

/// Whether the RPC shows code at `contract`; an RPC that can't tell, or
/// none configured, is 503.
async fn deployed(state: &State, contract: &str) -> Result<bool, ApiError> {
    match state.deployments.of(state, contract).await {
        Onchain::Deployed(_) => Ok(true),
        Onchain::Absent => Ok(false),
        Onchain::Unknown | Onchain::Unconfigured => {
            Err(ApiError::service_unavailable(CHAIN_UNKNOWN))
        }
    }
}

/// A standard save that gives the stored collection another address is
/// refused when the address it holds is deployed: the row would no longer
/// name its contract.
pub async fn guard_address_change(
    state: &State,
    edit: &CollectionEdit,
    next: &str,
) -> Result<(), ApiError> {
    let stored: Option<Option<String>> = sqlx::query_scalar(
        "SELECT contract_address FROM builder.collections WHERE id = $1 AND third_party_id IS NULL",
    )
    .bind(edit.id())
    .fetch_optional(state.items.pool())
    .await?;
    let Some(Some(stored)) = stored else {
        return Ok(());
    };
    if stored != next && deployed(state, &stored).await? {
        return Err(ApiError::published(
            Subject::Collection,
            &edit.id().to_string(),
            "updated",
        ));
    }
    Ok(())
}
