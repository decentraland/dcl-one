//! The standard publication attempt (`builder.collection_publications`):
//! begin a reviewed revision, claim one wallet-signing attempt, record the
//! submitted transaction, finish with the verified outcome, or cancel an
//! unsubmitted attempt. Every transition takes the collection row `FOR
//! UPDATE` by `(id, eth_address)` first and the items `FOR UPDATE ORDER BY
//! id` before a snapshot, so two writers over one collection serialise; the
//! state CHECKs of the schema declare the states, these statements make the
//! transitions.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::http::ApiError;
use super::items::ItemsComponent;
use super::publication::{PublicationPreparation, SNAPSHOT_QUERY, prepare};

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct PublicationState {
    pub preparation: PublicationPreparation,
    pub status: String,
    pub tx_hash: Option<String>,
    pub contract_address: Option<String>,
}

const STATE_QUERY: &str = "SELECT jsonb_build_object('preparation', p.preparation, 'status', p.status, \
    'tx_hash', p.tx_hash, 'contract_address', p.contract_address) FROM builder.collection_publications p \
    JOIN builder.collections c ON c.id=p.collection_id WHERE c.id=$1 AND c.eth_address=$2";

const LOCK_OWNED: &str =
    "SELECT id FROM builder.collections WHERE id=$1 AND eth_address=$2 FOR UPDATE";

fn decode(value: Value) -> Result<PublicationState, ApiError> {
    serde_json::from_value(value)
        .map_err(|e| ApiError::internal(format!("Invalid saved publication: {e}")))
}

impl ItemsComponent {
    pub async fn publication_state(
        &self,
        signer: &str,
        id: Uuid,
    ) -> Result<Option<PublicationState>, ApiError> {
        let value: Option<Value> = sqlx::query_scalar(STATE_QUERY)
            .bind(id)
            .bind(signer)
            .fetch_optional(&self.pool)
            .await?;
        value.map(decode).transpose()
    }

    pub async fn begin_publication(
        &self,
        signer: &str,
        id: Uuid,
        revision: &str,
    ) -> Result<PublicationState, ApiError> {
        let mut tx = self.pool.begin().await?;
        let owned: Option<Uuid> = sqlx::query_scalar(LOCK_OWNED)
            .bind(id)
            .bind(signer)
            .fetch_optional(&mut *tx)
            .await?;
        if owned.is_none() {
            return Err(ApiError::not_found("Collection not found"));
        }
        let prior: Option<Value> = sqlx::query_scalar(STATE_QUERY)
            .bind(id)
            .bind(signer)
            .fetch_optional(&mut *tx)
            .await?;
        if let Some(prior) = prior {
            let prior = decode(prior)?;
            if prior.status != "reverted" {
                if prior.preparation.revision != revision {
                    return Err(ApiError::http(
                        409,
                        "A different collection revision is already being published",
                    ));
                }
                return Ok(prior);
            }
        }
        sqlx::query("SELECT id FROM builder.items WHERE collection_id=$1 ORDER BY id FOR UPDATE")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
        let snapshot: Value = sqlx::query_scalar(SNAPSHOT_QUERY)
            .bind(id)
            .bind(signer)
            .fetch_one(&mut *tx)
            .await?;
        let preparation = prepare(snapshot, signer)?;
        if preparation.revision != revision {
            return Err(ApiError::http(
                409,
                "Collection changed. Review its items and fee again before publishing.",
            ));
        }
        sqlx::query("INSERT INTO builder.collection_publications (collection_id, preparation, status) VALUES ($1,$2,'prepared') \
            ON CONFLICT (collection_id) DO UPDATE SET preparation=$2, tx_hash=NULL, contract_address=NULL, status='prepared', updated_at=timezone('UTC', now())")
            .bind(id).bind(serde_json::to_value(&preparation).map_err(|e| ApiError::internal(e.to_string()))?).execute(&mut *tx).await?;
        sqlx::query("UPDATE builder.collections SET publication_pending=true WHERE id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(PublicationState {
            preparation,
            status: "prepared".into(),
            tx_hash: None,
            contract_address: None,
        })
    }

    pub async fn claim_publication(
        &self,
        signer: &str,
        id: Uuid,
        revision: &str,
    ) -> Result<(), ApiError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(LOCK_OWNED)
            .bind(id)
            .bind(signer)
            .fetch_all(&mut *tx)
            .await?;
        let changed = sqlx::query(
            "UPDATE builder.collection_publications p SET status='signing', updated_at=timezone('UTC', now()) \
            FROM builder.collections c WHERE c.id=p.collection_id AND c.id=$1 AND c.eth_address=$2 \
            AND c.publication_pending AND p.status='prepared' AND p.preparation->>'revision'=$3",
        )
        .bind(id)
        .bind(signer)
        .bind(revision)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if changed != 1 {
            return Err(ApiError::http(
                409,
                "Another wallet request is already active. Recover its transaction before retrying.",
            ));
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn cancel_publication(&self, signer: &str, id: Uuid) -> Result<(), ApiError> {
        let mut tx = self.pool.begin().await?;
        let owned: Option<Uuid> = sqlx::query_scalar(LOCK_OWNED)
            .bind(id)
            .bind(signer)
            .fetch_optional(&mut *tx)
            .await?;
        if owned.is_none() {
            return Err(ApiError::not_found("Collection not found"));
        }
        let state: Option<Value> = sqlx::query_scalar(STATE_QUERY)
            .bind(id)
            .bind(signer)
            .fetch_optional(&mut *tx)
            .await?;
        let Some(state) = state else {
            return Ok(());
        };
        let state = decode(state)?;
        if state.tx_hash.is_some() && state.status != "reverted" {
            return Err(ApiError::http(
                409,
                "A submitted publication cannot be canceled here. Wait for its receipt.",
            ));
        }
        sqlx::query("DELETE FROM builder.collection_publications WHERE collection_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE builder.collections SET publication_pending=false WHERE id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub(super) async fn record_publication_tx(
        &self,
        signer: &str,
        id: Uuid,
        hash: &str,
        revision: &str,
    ) -> Result<(), ApiError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(LOCK_OWNED)
            .bind(id)
            .bind(signer)
            .fetch_all(&mut *tx)
            .await?;
        let changed = sqlx::query("UPDATE builder.collection_publications p SET tx_hash=$3, status='submitted', updated_at=timezone('UTC', now()) \
            FROM builder.collections c WHERE c.id=p.collection_id AND c.id=$1 AND c.eth_address=$2 AND c.publication_pending \
            AND p.status IN ('prepared','signing','submitted') AND (p.tx_hash IS NULL OR p.tx_hash=$3) AND p.preparation->>'revision'=$4")
            .bind(id).bind(signer).bind(hash).bind(revision).execute(&mut *tx).await?.rows_affected();
        if changed != 1 {
            return Err(ApiError::http(
                409,
                "Publication is no longer pending or has another transaction",
            ));
        }
        tx.commit().await?;
        Ok(())
    }

    /// The verified outcome: `published` assigns every item its
    /// `blockchain_item_id` and collections-v2 urn in `preparation.items`
    /// order and the collection its contract, urn and salt; `reverted`
    /// unfreezes the draft. Idempotent once terminal.
    pub(super) async fn finish_publication(
        &self,
        signer: &str,
        id: Uuid,
        hash: &str,
        contract: Option<&str>,
    ) -> Result<PublicationState, ApiError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(LOCK_OWNED)
            .bind(id)
            .bind(signer)
            .fetch_all(&mut *tx)
            .await?;
        let state: Value = sqlx::query_scalar(STATE_QUERY)
            .bind(id)
            .bind(signer)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| ApiError::not_found("Publication not found"))?;
        let mut state = decode(state)?;
        if state.tx_hash.as_deref() != Some(hash) {
            return Err(ApiError::http(409, "Publication transaction changed"));
        }
        if state.status == "published" || state.status == "reverted" {
            return Ok(state);
        }
        if let Some(contract) = contract {
            for (token, item) in state.preparation.items.iter().enumerate() {
                let item_id = Uuid::parse_str(&item.id)
                    .map_err(|_| ApiError::internal("Invalid saved item id"))?;
                let changed = sqlx::query("UPDATE builder.items SET is_published=true, blockchain_item_id=$2, urn_suffix=$3, \
                    price=$4, beneficiary=$5, updated_at=timezone('UTC', now()) WHERE id=$1 AND collection_id=$6 AND eth_address=$7 AND NOT is_published")
                    .bind(item_id).bind(token.to_string()).bind(format!("urn:decentraland:matic:collections-v2:{contract}:{token}"))
                    .bind(&item.price).bind(&item.beneficiary).bind(id).bind(signer).execute(&mut *tx).await?.rows_affected();
                if changed != 1 {
                    return Err(ApiError::http(
                        409,
                        "Collection items changed during publication",
                    ));
                }
            }
            sqlx::query("UPDATE builder.collections SET is_published=true, publication_pending=false, contract_address=$2, \
                urn_suffix=$3, salt=$4, updated_at=timezone('UTC', now()) WHERE id=$1")
                .bind(id).bind(contract).bind(format!("urn:decentraland:matic:collections-v2:{contract}"))
                .bind(&state.preparation.salt).execute(&mut *tx).await?;
            state.status = "published".into();
            state.contract_address = Some(contract.into());
        } else {
            sqlx::query("UPDATE builder.collections SET publication_pending=false WHERE id=$1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            state.status = "reverted".into();
        }
        sqlx::query("UPDATE builder.collection_publications SET status=$2, contract_address=$3, updated_at=timezone('UTC', now()) WHERE collection_id=$1")
            .bind(id).bind(&state.status).bind(&state.contract_address).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(state)
    }
}
