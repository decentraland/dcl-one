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
        (NamesError::Transient as fn(String) -> NamesError, 503),
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
                // A transient fault's own text stays in the log.
                let expected = if error("x".into()) == NamesError::Transient("x".into()) {
                    "the ethereum nft index could not be read just now: NAME ownership cannot be established; retry shortly"
                } else {
                    "the names index cannot answer now"
                };
                assert_eq!(body["message"], expected, "{body}");
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

/// A NAME source whose first owner answer is `holder` and every later one
/// `error`: the holder the permission check reads, then a lookup that
/// changes.
struct Turning {
    holder: String,
    error: fn(String) -> NamesError,
    calls: std::sync::atomic::AtomicUsize,
}

impl Turning {
    fn owner(&self) -> Result<Answer<Option<String>>, NamesError> {
        if self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            return Ok(Answer {
                value: Some(self.holder.clone()),
                observation: None,
            });
        }
        Err((self.error)(
            "the names index answers differently now".to_string(),
        ))
    }
}

#[async_trait::async_trait]
impl NameAuthority for Turning {
    async fn owned_names(
        &self,
        _: &str,
        _: Purpose,
    ) -> Result<Answer<Arc<Vec<OwnedName>>>, NamesError> {
        Ok(Answer {
            value: Arc::default(),
            observation: None,
        })
    }

    async fn name_owner(&self, _: &str, _: Purpose) -> Result<Answer<Option<String>>, NamesError> {
        self.owner()
    }

    async fn name_owners(
        &self,
        _: &[String],
        _: Purpose,
    ) -> Result<Answer<std::collections::HashMap<String, Option<String>>>, NamesError> {
        Err((self.error)(
            "the names index answers differently now".to_string(),
        ))
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

/// A scoped grantee's publish reads the world's owner twice: the permission
/// check, then the owner the write records. When the first read names the
/// holder and the second cannot, the second is answered as the permission
/// check answers it (the names index not current is its 503, an ambiguous
/// label or a failed read the 403), and the world is left as it was.
#[tokio::test]
async fn a_grantee_whose_owner_lookup_changes_is_refused_as_the_check_is() {
    for (error, status) in [
        (NamesError::NotEstablished as fn(String) -> NamesError, 503),
        (NamesError::Transient as fn(String) -> NamesError, 503),
        (NamesError::Ambiguous as fn(String) -> NamesError, 403),
        (NamesError::Failed as fn(String) -> NamesError, 403),
    ] {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let holder = Wallet::random();
        let f = fixture_with_names(
            pool,
            PersonalWorldsPolicy::disabled(),
            None,
            None,
            test_settings(),
            "",
            Some(Arc::new(Turning {
                holder: holder.address.as_str().to_string(),
                error,
                calls: Default::default(),
            })),
        )
        .await;
        let (world, grantee) = (world_name(), Wallet::random());
        grant_parcels(&f.pool, &world, &grantee.address, &["0,0"]).await;
        let before = stored_owner(&f.pool, &world).await;
        let d = deployment(&grantee, &world, "Turning", "0,0", &["0,0"]).await;
        let body = post_entity(&f.app, &d, status).await;
        if status == 503 {
            assert_eq!(body["source"], "nft-index", "{body}");
            assert_eq!(body["configure"], "MARKETPLACE_NFT_INDEX_ENABLED", "{body}");
        } else {
            assert_eq!(
                body["message"],
                "Not authorized: could not verify deployment permissions (deploy denied)",
                "{body}"
            );
        }
        assert!(deployed_entity_ids(&f.pool, &world).await.is_empty());
        assert_eq!(stored_owner(&f.pool, &world).await, before);
    }
}
