//! A DCL NAME world publish whose NAME source cannot answer: the names index
//! not current (it cannot establish a holder now) is a retryable 503 naming
//! `nft-index`, still refused, for the owner and a scoped grantee alike; an
//! ambiguous label (two holders, which waiting does not resolve) and any
//! other undetermined answer keep the route's fail-closed 403.

use crate::chain_index::names::{Answer, NameAuthority, NamesError, OwnedName, Purpose};

use super::*;

/// A NAME source whose every answer is `error`.
struct Unanswered(fn(String) -> NamesError);

impl Unanswered {
    fn error<T>(&self) -> Result<T, NamesError> {
        Err((self.0)("the names index cannot answer now".to_string()))
    }
}

#[async_trait::async_trait]
impl NameAuthority for Unanswered {
    async fn owned_names(
        &self,
        _: &str,
        _: Purpose,
    ) -> Result<Answer<Arc<Vec<OwnedName>>>, NamesError> {
        self.error()
    }

    async fn name_owner(&self, _: &str, _: Purpose) -> Result<Answer<Option<String>>, NamesError> {
        self.error()
    }

    async fn name_owners(
        &self,
        _: &[String],
        _: Purpose,
    ) -> Result<Answer<std::collections::HashMap<String, Option<String>>>, NamesError> {
        self.error()
    }

    async fn still_current(
        &self,
        _: &crate::chain_index::names::Observation,
        _: Purpose,
    ) -> Result<bool, NamesError> {
        Ok(true)
    }

    fn observes(&self) -> bool {
        true
    }
}

#[tokio::test]
async fn a_publish_the_names_index_cannot_answer_now_is_a_retryable_503() {
    for (error, status) in [
        (NamesError::NotEstablished as fn(String) -> NamesError, 503),
        (NamesError::Ambiguous as fn(String) -> NamesError, 403),
        (NamesError::Failed as fn(String) -> NamesError, 403),
    ] {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let f = fixture_with_names(
            pool,
            PersonalWorldsPolicy::disabled(),
            None,
            None,
            test_settings(),
            "",
            Some(Arc::new(Unanswered(error))),
        )
        .await;
        let (world, wallet, grantee) = (world_name(), Wallet::random(), Wallet::random());
        grant_parcels(&f.pool, &world, &grantee.address, &["0,0"]).await;
        for signer in [&wallet, &grantee] {
            let d = deployment(signer, &world, "Unanswered", "0,0", &["0,0"]).await;
            let body = post_entity(&f.app, &d, status).await;
            if status == 503 {
                assert_eq!(body["source"], "nft-index", "{body}");
                assert_eq!(body["configure"], "MARKETPLACE_NFT_INDEX_ENABLED", "{body}");
                assert_eq!(
                    body["message"], "the names index cannot answer now",
                    "{body}"
                );
            } else {
                assert_eq!(
                    body["message"],
                    "Not authorized: could not verify deployment permissions (deploy denied)",
                    "{body}"
                );
            }
        }
        assert!(deployed_entity_ids(&f.pool, &world).await.is_empty());
    }
}
