//! A DCL NAME publish re-checks its NAME authorization inside the
//! transaction that writes it, under the names index's publication held
//! `FOR SHARE` (`chain_index::names::hold_index_publication`) until that
//! transaction ends. A fold withdrawn between validation and the write
//! refuses it as the names index's 503, a refold that replaced the holder
//! refuses the owner's publish (400) and asks a grantee's to be retried
//! (409), and in every case the world keeps its scene and its owner. The
//! hold lasts only that short transaction: a fold's publication commits at
//! once while a publish is still before it, and right after its commit
//! while it is inside it.

use std::time::{Duration, Instant};

use tokio::sync::Notify;

use crate::chain_index::names::{
    Answer, NameAuthority, NamesError, Observation, OwnedName, PublicationHold, Purpose,
    hold_index_publication,
};

use super::super::deploy::BEFORE_PERSIST;
use super::super::store::AFTER_NAME_CHECK;
use super::*;

const REGISTRAR: &str = "0x2a187453064356c898cae034eaed119e1663acb8";

/// The names index as tables: the Ethereum nft cursor's publication and the
/// registrar's rows of `shop.nft`, held with the shared fence.
struct TableNames {
    pool: PgPool,
}

/// The publication an authorization reads: published within 30 minutes.
async fn current(conn: &mut sqlx::PgConnection) -> Result<Observation, NamesError> {
    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT folded_block, (extract(epoch FROM folded_at) * 1000000)::bigint FROM shop.chain_cursor \
         WHERE indexer = 'nft' AND network = 'ethereum' AND folded_block IS NOT NULL \
           AND folded_at >= timezone('UTC', now()) - interval '30 minutes'",
    )
    .fetch_optional(conn)
    .await
    .map_err(|e| NamesError::Failed(e.to_string()))?;
    row.map(|(folded_block, folded_at_us)| Observation {
        folded_block,
        folded_at_us,
    })
    .ok_or_else(|| NamesError::NotEstablished("the names index is not published".into()))
}

async fn holder_on(
    conn: &mut sqlx::PgConnection,
    label: &str,
) -> Result<Answer<Option<String>>, NamesError> {
    let observation = current(conn).await?;
    let value: Option<String> = sqlx::query_scalar(
        "SELECT owner FROM shop.nft WHERE network = 'ethereum' AND contract_address = $1 \
           AND lower(ens_subdomain) = lower($2) AND owner IS NOT NULL",
    )
    .bind(REGISTRAR)
    .bind(label)
    .fetch_optional(conn)
    .await
    .map_err(|e| NamesError::Failed(e.to_string()))?;
    Ok(Answer {
        value,
        observation: Some(observation),
    })
}

impl TableNames {
    async fn conn(&self) -> Result<sqlx::pool::PoolConnection<sqlx::Postgres>, NamesError> {
        self.pool
            .acquire()
            .await
            .map_err(|e| NamesError::Failed(e.to_string()))
    }
}

#[async_trait::async_trait]
impl NameAuthority for TableNames {
    async fn owned_names(
        &self,
        owner: &str,
        _: Purpose,
    ) -> Result<Answer<Arc<Vec<OwnedName>>>, NamesError> {
        let mut conn = self.conn().await?;
        let observation = current(&mut conn).await?;
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT ens_subdomain, token_id::text FROM shop.nft WHERE network = 'ethereum' \
               AND contract_address = $1 AND owner = $2 AND ens_subdomain IS NOT NULL",
        )
        .bind(REGISTRAR)
        .bind(owner.to_lowercase())
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| NamesError::Failed(e.to_string()))?;
        Ok(Answer {
            value: Arc::new(
                rows.into_iter()
                    .map(|(name, token_id)| OwnedName {
                        name,
                        contract_address: REGISTRAR.into(),
                        token_id,
                        price: None,
                    })
                    .collect(),
            ),
            observation: Some(observation),
        })
    }

    async fn name_owner(
        &self,
        label: &str,
        _: Purpose,
    ) -> Result<Answer<Option<String>>, NamesError> {
        holder_on(&mut *self.conn().await?, label).await
    }

    async fn name_owner_on(
        &self,
        conn: &mut sqlx::PgConnection,
        label: &str,
        _: Purpose,
    ) -> Result<Answer<Option<String>>, NamesError> {
        holder_on(conn, label).await
    }

    async fn name_owners(
        &self,
        labels: &[String],
        _: Purpose,
    ) -> Result<Answer<std::collections::HashMap<String, Option<String>>>, NamesError> {
        let mut conn = self.conn().await?;
        let mut value = std::collections::HashMap::new();
        let mut observation = None;
        for label in labels {
            let answer = holder_on(&mut conn, label).await?;
            observation = answer.observation;
            value.insert(label.to_lowercase(), answer.value);
        }
        Ok(Answer { value, observation })
    }

    async fn still_current(
        &self,
        observation: &Observation,
        _: Purpose,
    ) -> Result<bool, NamesError> {
        match current(&mut *self.conn().await?).await {
            Ok(now) => Ok(now == *observation),
            Err(NamesError::NotEstablished(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    fn observes(&self) -> bool {
        true
    }

    async fn hold_publication_on(&self, conn: &mut sqlx::PgConnection) -> Result<(), NamesError> {
        match hold_index_publication(conn).await {
            Ok(PublicationHold::Held) => Ok(()),
            Ok(PublicationHold::Unpublished | PublicationHold::Busy) => Err(
                NamesError::NotEstablished("the names index is not published".into()),
            ),
            Err(e) => Err(NamesError::Failed(e.to_string())),
        }
    }
}

/// A fixture on a database of its own whose NAME source is [`TableNames`],
/// `holder` holding `world`'s NAME under a fresh publication.
async fn fenced(holder: &Wallet) -> Option<(Fixture, WorldName)> {
    let pool = crate::db::scratch::isolated_pool(6).await?;
    let world = world_name();
    sqlx::query(
        "INSERT INTO shop.nft (network, contract_address, token_id, owner, ens_subdomain, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
         VALUES ('ethereum', $1, 7, $2, $3, 1, 1, 1, 0, 1)",
    )
    .bind(REGISTRAR)
    .bind(holder.address.as_str())
    .bind(world.label())
    .execute(&pool)
    .await
    .unwrap();
    republish(&pool).await;
    let names = Arc::new(TableNames { pool: pool.clone() });
    let f = fixture_with_names(
        pool,
        PersonalWorldsPolicy::disabled(),
        None,
        None,
        test_settings(),
        "",
        Some(names),
    )
    .await;
    Some((f, world))
}

/// The nft fold publishes (again): what a walker's publication writes.
async fn republish(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, fold_through, folded_block, folded_at, full_history, chain_id) \
         VALUES ('nft', 'ethereum', 1, 100, 100, 100, timezone('UTC', now()), true, 1) \
         ON CONFLICT (indexer, network) DO UPDATE SET folded_block = 100, folded_at = timezone('UTC', now())",
    )
    .execute(pool)
    .await
    .unwrap();
}

/// A publish of `d` paused at `hook` while `meanwhile` runs, then resumed;
/// its answer.
async fn paused<F>(
    hook: &'static tokio::task::LocalKey<(Arc<Notify>, Arc<Notify>)>,
    app: &Router,
    d: &Deployment,
    status: u16,
    meanwhile: F,
) -> Value
where
    F: std::future::Future<Output = ()>,
{
    let (reached, resume) = (Arc::new(Notify::new()), Arc::new(Notify::new()));
    let publish = hook.scope(
        (reached.clone(), resume.clone()),
        post_entity(app, d, status),
    );
    let (body, ()) = tokio::join!(publish, async {
        reached.notified().await;
        meanwhile.await;
        resume.notify_one();
    });
    body
}

/// What the world serves: its scenes and its stored owner.
async fn world_state(f: &Fixture, world: &WorldName) -> (Vec<String>, Option<String>) {
    (
        deployed_entity_ids(&f.pool, world).await,
        stored_owner(&f.pool, world).await,
    )
}

/// A publish whose names fold is withdrawn after its validation is refused
/// at its write with the names index's 503, and the world keeps the scene
/// and owner it had.
#[tokio::test]
async fn a_fold_withdrawn_before_the_write_refuses_it_and_keeps_the_world() {
    let holder = Wallet::random();
    let Some((f, world)) = fenced(&holder).await else {
        return;
    };
    let now = chrono::Utc::now().timestamp_millis();
    let first = deployment_at(
        &holder,
        &world,
        "First",
        "0,0",
        &["0,0"],
        "thumb.png",
        b"first",
        now,
    )
    .await;
    post_entity(&f.app, &first, 200).await;
    let before = world_state(&f, &world).await;
    assert_eq!(before.0, std::slice::from_ref(&first.entity_id));
    let second = deployment_at(
        &holder,
        &world,
        "Second",
        "0,0",
        &["0,0"],
        "thumb.png",
        b"second",
        now + 1,
    )
    .await;
    let body = paused(&BEFORE_PERSIST, &f.app, &second, 503, async {
        sqlx::query(
            "UPDATE shop.chain_cursor SET folded_block = NULL, folded_at = NULL WHERE indexer = 'nft' AND network = 'ethereum'",
        )
        .execute(&f.pool)
        .await
        .unwrap();
    })
    .await;
    assert_eq!(body["source"], "nft-index", "{body}");
    assert_eq!(body["configure"], "MARKETPLACE_NFT_INDEX_ENABLED", "{body}");
    assert_eq!(world_state(&f, &world).await, before);
    // Once the fold is published again the same publish goes through.
    republish(&f.pool).await;
    post_entity(&f.app, &second, 200).await;
    assert_eq!(
        deployed_entity_ids(&f.pool, &world).await,
        std::slice::from_ref(&second.entity_id)
    );
}

/// A refold that moved the NAME to someone else between the validation and
/// the write refuses the holder's publish (the 400 a non-holder gets) and a
/// scoped grantee's (409, to retry against the new owner); the world keeps
/// its scene and its owner.
#[tokio::test]
async fn a_refold_that_replaced_the_holder_refuses_the_write() {
    let holder = Wallet::random();
    let Some((f, world)) = fenced(&holder).await else {
        return;
    };
    let grantee = Wallet::random();
    grant_parcels(&f.pool, &world, &grantee.address, &["0,0"]).await;
    let now = chrono::Utc::now().timestamp_millis();
    let first = deployment_at(
        &holder,
        &world,
        "First",
        "0,0",
        &["0,0"],
        "thumb.png",
        b"first",
        now,
    )
    .await;
    post_entity(&f.app, &first, 200).await;
    let before = world_state(&f, &world).await;
    let other = Wallet::random();
    for (signer, status, stamp) in [(&holder, 400, now + 1), (&grantee, 409, now + 2)] {
        let d = deployment_at(
            signer,
            &world,
            "Next",
            "0,0",
            &["0,0"],
            "thumb.png",
            b"next",
            stamp,
        )
        .await;
        let body = paused(&BEFORE_PERSIST, &f.app, &d, status, async {
            sqlx::query("UPDATE shop.nft SET owner = $1 WHERE network = 'ethereum' AND contract_address = $2")
                .bind(other.address.as_str())
                .bind(REGISTRAR)
                .execute(&f.pool)
                .await
                .unwrap();
            republish(&f.pool).await;
        })
        .await;
        if status == 400 {
            assert!(
                body["message"]
                    .as_str()
                    .is_some_and(|m| m.contains("does not have permission to deploy under")),
                "{body}"
            );
        } else {
            assert!(
                body["message"]
                    .as_str()
                    .is_some_and(|m| m.contains("retry it")),
                "{body}"
            );
        }
        assert_eq!(world_state(&f, &world).await, before);
        // The NAME back with its holder for the grantee's turn.
        sqlx::query(
            "UPDATE shop.nft SET owner = $1 WHERE network = 'ethereum' AND contract_address = $2",
        )
        .bind(holder.address.as_str())
        .bind(REGISTRAR)
        .execute(&f.pool)
        .await
        .unwrap();
        republish(&f.pool).await;
    }
}

/// The publication is held only inside the write's transaction: while a
/// publish is still before it (validated, its content stored), a fold's
/// publication commits at once; while the write holds it, the publication
/// waits and commits right after the write's commit.
#[tokio::test]
async fn a_fold_waits_only_for_the_write_transaction() {
    let holder = Wallet::random();
    let Some((f, world)) = fenced(&holder).await else {
        return;
    };
    let publish_fold = |pool: PgPool| async move {
        sqlx::query(
            "UPDATE shop.chain_cursor SET folded_at = timezone('UTC', now()) WHERE indexer = 'nft' AND network = 'ethereum'",
        )
        .execute(&pool)
        .await
        .unwrap();
        Instant::now()
    };
    let now = chrono::Utc::now().timestamp_millis();
    let first = deployment_at(
        &holder,
        &world,
        "First",
        "0,0",
        &["0,0"],
        "thumb.png",
        b"first",
        now,
    )
    .await;
    let pool = f.pool.clone();
    paused(&BEFORE_PERSIST, &f.app, &first, 200, async {
        let started = Instant::now();
        let done = tokio::time::timeout(Duration::from_secs(2), publish_fold(pool))
            .await
            .expect("a fold publishes while a publish has not begun its write");
        assert!(
            done - started < Duration::from_secs(2),
            "{:?}",
            done - started
        );
    })
    .await;

    let second = deployment_at(
        &holder,
        &world,
        "Second",
        "0,0",
        &["0,0"],
        "thumb.png",
        b"second",
        now + 1,
    )
    .await;
    let (reached, resume) = (Arc::new(Notify::new()), Arc::new(Notify::new()));
    let publish = AFTER_NAME_CHECK.scope(
        (reached.clone(), resume.clone()),
        post_entity(&f.app, &second, 200),
    );
    let pool = f.pool.clone();
    let (answered, folded) = tokio::join!(
        async {
            publish.await;
            Instant::now()
        },
        async {
            reached.notified().await;
            let fold = tokio::spawn(publish_fold(pool));
            tokio::time::sleep(Duration::from_millis(300)).await;
            assert!(
                !fold.is_finished(),
                "the fold waits while the write holds its publication"
            );
            resume.notify_one();
            fold.await.unwrap()
        }
    );
    // The fold commits right after the write: within a generous bound of the
    // publish's answer, either side of it.
    let gap = folded
        .saturating_duration_since(answered)
        .max(answered.saturating_duration_since(folded));
    assert!(
        gap < Duration::from_secs(2),
        "the fold commits right after the write: {gap:?}"
    );
    assert_eq!(
        deployed_entity_ids(&f.pool, &world).await,
        std::slice::from_ref(&second.entity_id)
    );
}

/// A publish that reaches its write while a fold holds the publication past
/// the bounded wait (`chain_index::names::HOLD_WAIT`) is refused as the
/// names index's 503. It holds the world's locks no longer than that wait,
/// and the world keeps its scene and owner.
#[tokio::test]
async fn a_publish_behind_a_long_fold_answers_the_names_index_503() {
    let holder = Wallet::random();
    let Some((f, world)) = fenced(&holder).await else {
        return;
    };
    let now = chrono::Utc::now().timestamp_millis();
    let first = deployment_at(
        &holder,
        &world,
        "First",
        "0,0",
        &["0,0"],
        "thumb.png",
        b"first",
        now,
    )
    .await;
    post_entity(&f.app, &first, 200).await;
    let before = world_state(&f, &world).await;
    let mut fold = f.pool.begin().await.unwrap();
    sqlx::query(
        "SELECT 1 FROM shop.chain_cursor WHERE indexer = 'nft' AND network = 'ethereum' FOR UPDATE",
    )
    .execute(&mut *fold)
    .await
    .unwrap();
    let second = deployment_at(
        &holder,
        &world,
        "Second",
        "0,0",
        &["0,0"],
        "thumb.png",
        b"second",
        now + 1,
    )
    .await;
    let started = Instant::now();
    let body = post_entity(&f.app, &second, 503).await;
    let waited = started.elapsed();
    assert_eq!(body["source"], "nft-index", "{body}");
    let wait = crate::chain_index::names::HOLD_WAIT;
    assert!(waited < wait + Duration::from_secs(3), "{waited:?}");
    fold.rollback().await.unwrap();
    assert_eq!(world_state(&f, &world).await, before);
}
