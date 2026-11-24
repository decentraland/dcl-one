//! The DCL NAME authority over the nft index ([`IndexNames`]): the
//! DCLRegistrar rows of `shop.nft` (Ethereum, `ens_subdomain` from
//! NameRegistered, `owner` from the transfers), each read in ONE statement
//! with the Ethereum cursor that says whether they may be trusted for the
//! read's purpose (a statement sees one snapshot at any isolation level, so a
//! read runs on a pooled connection or on a session the caller holds):
//!
//! - [`Purpose::Authorize`]: [`chain_walk::is_ready_on`]: the whole history
//!   walked, every walked block folded and published (`folded_block =
//!   last_block`), within [`chain_walk::READY_WINDOW`], bound to chain 1, out
//!   of quarantine; or, between walking and publishing a pass, the same with
//!   no NAME registrar event walked past the published fold, which is then
//!   still every NAME's state. While a pass has walked a registrar transfer
//!   it has not folded, the fold still names the former holder, so no
//!   holder is authorized.
//! - [`Purpose::Display`]: [`owned_items::Rule::Fresh`]: complete and
//!   published within [`chain_walk::READY_WINDOW`].
//!
//! Every answer carries the [`Observation`] of its read. A memo files it
//! under that observation and asks [`IndexNames::still_current`] on every
//! hit, which reads the cursor again. Labels arrive bare (no `.dcl.eth`) and
//! are compared lowercased; nothing here strips a suffix.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::postgres::PgRow;
use sqlx::{FromRow, PgConnection, PgPool, Row};

use crate::chain_index::names::{
    Answer, NameAuthority, NamesError, Observation, OwnedName, Purpose,
};

use super::chain_walk::{self, CURSOR_COLUMNS, ChainNet, Cursor, INDEXER_NFT};
use super::nft_index::DCL_REGISTRAR;
use super::owned_items;

/// The nft index as the DCL NAME authority.
pub struct IndexNames {
    pool: PgPool,
}

impl IndexNames {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn conn(&self) -> Result<sqlx::pool::PoolConnection<sqlx::Postgres>, NamesError> {
        self.pool.acquire().await.map_err(failed)
    }
}

/// Whether `cursor` may be trusted for `purpose` at `now`. `names_walked`:
/// whether the walk holds a NAME registrar event past the published fold
/// (a pass between walking and publishing it).
pub(crate) fn holds(
    cursor: &Cursor,
    names_walked: bool,
    purpose: Purpose,
    now: DateTime<Utc>,
) -> bool {
    match purpose {
        Purpose::Authorize => {
            let ethereum = chain_walk::mainnet_chain(ChainNet::Ethereum);
            if chain_walk::is_ready_on(cursor, ethereum, now) {
                return true;
            }
            // Each pass commits its walk before it folds and publishes, so
            // every new block opens a window in which the published fold
            // trails the walk. The fold is still every NAME's state there
            // when the walk holds no registrar event past it; otherwise it
            // may name a former holder, and nobody is authorized.
            !names_walked
                && cursor.folded_block.is_some_and(|f| f < cursor.last_block)
                && chain_walk::is_ready_on(
                    &Cursor {
                        folded_block: Some(cursor.last_block),
                        ..cursor.clone()
                    },
                    ethereum,
                    now,
                )
        }
        Purpose::Display => owned_items::Rule::Fresh.holds(cursor, now),
    }
}

/// The observation of a cursor that holds: its published fold.
pub(crate) fn observation(cursor: &Cursor) -> Option<Observation> {
    let (block, at) = cursor.folded_block.zip(cursor.folded_at)?;
    Some(Observation {
        folded_block: block,
        folded_at_us: at.and_utc().timestamp_micros(),
    })
}

fn not_established(purpose: Purpose) -> NamesError {
    NamesError::NotEstablished(match purpose {
        Purpose::Authorize => "DCL NAMEs read the ethereum nft index, which has walked a NAME registrar event it has not published or is not published (catching up, rewound after a reorg, quarantined, or MARKETPLACE_NFT_INDEX_ENABLED off): NAME ownership cannot be established".to_string(),
        Purpose::Display => "DCL NAMEs read the ethereum nft index, which is not published (catching up, rewound after a reorg, stopped, or MARKETPLACE_NFT_INDEX_ENABLED off)".to_string(),
    })
}

fn failed(e: sqlx::Error) -> NamesError {
    NamesError::Failed(format!("ethereum nft index: {e}"))
}

/// The Ethereum nft cursor (`c`) as a NAME read selects it beside its rows,
/// every row carrying it: its columns, and `names_walked`, whether the walk
/// holds an event of the registrar (`registrar`, a bind placeholder) past
/// the published fold. A pass commits its walked events with its
/// `last_block`, so the statement's snapshot sees both or neither.
fn cursor_select(registrar: &str) -> String {
    let columns = CURSOR_COLUMNS
        .split(", ")
        .map(|column| format!("c.{column}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{columns}, EXISTS (SELECT 1 FROM shop.nft_event e \
             WHERE e.network = 'ethereum' AND e.contract_address = {registrar} \
               AND e.block_number > c.folded_block AND e.block_number <= c.last_block) AS names_walked"
    )
}

/// The cursor a one-statement read carried (and whether it walked a
/// registrar event past its fold), `None` when there is none (the
/// statement then returned no row).
fn cursor_of(rows: &[PgRow]) -> Result<Option<(Cursor, bool)>, NamesError> {
    rows.first()
        .map(|row| Ok((Cursor::from_row(row)?, row.try_get("names_walked")?)))
        .transpose()
        .map_err(failed)
}

/// `cursor` checked for `purpose` at `now`: its observation, or the refusal.
fn checked(
    cursor: Option<(Cursor, bool)>,
    purpose: Purpose,
    now: DateTime<Utc>,
) -> Result<Observation, NamesError> {
    cursor
        .filter(|(c, walked)| holds(c, *walked, purpose, now))
        .and_then(|(c, _)| observation(&c))
        .ok_or_else(|| not_established(purpose))
}

/// The Ethereum cursor read on `conn`, checked for `purpose`.
pub(crate) async fn check(
    conn: &mut PgConnection,
    purpose: Purpose,
    now: DateTime<Utc>,
) -> Result<Observation, NamesError> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {cursor} FROM shop.chain_cursor c WHERE c.indexer = $1 AND c.network = 'ethereum'",
        cursor = cursor_select("$2")
    )))
    .bind(INDEXER_NFT)
    .bind(DCL_REGISTRAR)
    .fetch_all(conn)
    .await
    .map_err(|e| NamesError::Failed(format!("ethereum nft cursor: {e}")))?;
    checked(cursor_of(&rows)?, purpose, now)
}

/// The NAMEs the owners in `$1` (lowercased) hold, `$2` the registrar, `$3`
/// now (unix ms): minted spelling, the price of the token's open, unexpired
/// legacy order when its seller still holds it, and the subgraph's order key
/// (by NFT id, `<contract>-<token id>` as text). One statement for
/// [`owned_on`] and [`read_owned_names_of`].
const OWNED_NAMES_SQL: &str = "\
SELECT n.owner, n.ens_subdomain AS name, n.contract_address AS name_contract, n.token_id::text AS name_token, \
       o.price::text AS price, n.contract_address || '-' || n.token_id::text AS sort_key \
FROM shop.nft n \
LEFT JOIN shop.nft_order o ON o.network = n.network AND o.contract_address = n.contract_address \
    AND o.token_id = n.token_id AND o.status = 'open' AND o.expires_at > $3 AND o.seller = n.owner \
WHERE n.owner = ANY($1) AND n.network = 'ethereum' AND n.contract_address = $2 AND n.ens_subdomain IS NOT NULL";

fn owned_name(r: &PgRow) -> Option<OwnedName> {
    Some(OwnedName {
        name: r.get::<Option<String>, _>("name")?,
        contract_address: r.get("name_contract"),
        token_id: r.get("name_token"),
        price: r.get("price"),
    })
}

/// The NAMEs `owner` holds ([`OWNED_NAMES_SQL`]), read on `conn` in one
/// statement with the cursor.
async fn owned_on(
    conn: &mut PgConnection,
    owner: &str,
    purpose: Purpose,
) -> Result<Answer<Arc<Vec<OwnedName>>>, NamesError> {
    let now = Utc::now();
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {cursor}, x.name, x.name_contract, x.name_token, x.price \
         FROM shop.chain_cursor c \
         LEFT JOIN LATERAL ({OWNED_NAMES_SQL}) x ON true \
         WHERE c.indexer = $4 AND c.network = 'ethereum' \
         ORDER BY x.sort_key",
        cursor = cursor_select("$2")
    )))
    .bind(vec![owner.to_lowercase()])
    .bind(DCL_REGISTRAR)
    .bind(now.timestamp_millis())
    .bind(INDEXER_NFT)
    .fetch_all(conn)
    .await
    .map_err(failed)?;
    let observed = checked(cursor_of(&rows)?, purpose, now)?;
    Ok(Answer {
        value: Arc::new(rows.iter().filter_map(owned_name).collect()),
        observation: Some(observed),
    })
}

/// Every lowercased label's distinct live holders, read on `conn` in one
/// statement with the cursor (more than one holder is an ambiguity the
/// caller refuses).
async fn holders_on(
    conn: &mut PgConnection,
    labels: &[String],
    purpose: Purpose,
) -> Result<(Observation, HashMap<String, Vec<String>>), NamesError> {
    let now = Utc::now();
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {cursor}, h.label, h.holder \
         FROM shop.chain_cursor c \
         LEFT JOIN LATERAL ( \
             SELECT DISTINCT lower(n.ens_subdomain) AS label, n.owner AS holder FROM shop.nft n \
             WHERE lower(n.ens_subdomain) = ANY($1) AND n.ens_subdomain IS NOT NULL \
               AND n.network = 'ethereum' AND n.contract_address = $2 AND n.owner IS NOT NULL \
         ) h ON true \
         WHERE c.indexer = $3 AND c.network = 'ethereum'",
        cursor = cursor_select("$2")
    )))
    .bind(labels)
    .bind(DCL_REGISTRAR)
    .bind(INDEXER_NFT)
    .fetch_all(conn)
    .await
    .map_err(failed)?;
    let observed = checked(cursor_of(&rows)?, purpose, now)?;
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for r in &rows {
        if let (Some(label), Some(holder)) = (
            r.get::<Option<String>, _>("label"),
            r.get::<Option<String>, _>("holder"),
        ) {
            out.entry(label).or_default().push(holder.to_lowercase());
        }
    }
    Ok((observed, out))
}

/// The NAMEs of many owners ([`OWNED_NAMES_SQL`]) in one statement on
/// `conn`, for a caller that checks the cursor in the same snapshot (the
/// profiles' and outfits' composite read): owner (lowercased) -> its NAMEs
/// in the subgraph's order; an owner holding none is absent.
pub(crate) async fn read_owned_names_of(
    conn: &mut PgConnection,
    owners: &[String],
) -> Result<HashMap<String, Vec<OwnedName>>, sqlx::Error> {
    let owners: Vec<String> = owners.iter().map(|o| o.to_lowercase()).collect();
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT x.* FROM ({OWNED_NAMES_SQL}) x ORDER BY x.owner, x.sort_key"
    )))
    .bind(&owners)
    .bind(DCL_REGISTRAR)
    .bind(Utc::now().timestamp_millis())
    .fetch_all(conn)
    .await?;
    let mut out: HashMap<String, Vec<OwnedName>> = HashMap::new();
    for r in &rows {
        if let Some(name) = owned_name(r) {
            out.entry(r.get::<String, _>("owner").to_lowercase())
                .or_default()
                .push(name);
        }
    }
    Ok(out)
}

/// One holder per label, or the ambiguity refused.
fn sole_holder(label: &str, holders: Option<&Vec<String>>) -> Result<Option<String>, NamesError> {
    match holders.map(Vec::as_slice) {
        None | Some([]) => Ok(None),
        Some([owner]) => Ok(Some(owner.clone())),
        Some(_) => Err(NamesError::NotEstablished(format!(
            "the DCL NAME {label} has more than one holder in the nft index once lowercased: its owner cannot be established"
        ))),
    }
}

async fn name_owner_on(
    conn: &mut PgConnection,
    label: &str,
    purpose: Purpose,
) -> Result<Answer<Option<String>>, NamesError> {
    let label = label.to_lowercase();
    let (observed, holders) = holders_on(conn, std::slice::from_ref(&label), purpose).await?;
    Ok(Answer {
        value: sole_holder(&label, holders.get(&label))?,
        observation: Some(observed),
    })
}

#[async_trait::async_trait]
impl NameAuthority for IndexNames {
    async fn owned_names(
        &self,
        owner: &str,
        purpose: Purpose,
    ) -> Result<Answer<Arc<Vec<OwnedName>>>, NamesError> {
        owned_on(&mut *self.conn().await?, owner, purpose).await
    }

    async fn owned_names_on(
        &self,
        conn: &mut PgConnection,
        owner: &str,
        purpose: Purpose,
    ) -> Result<Answer<Arc<Vec<OwnedName>>>, NamesError> {
        owned_on(conn, owner, purpose).await
    }

    async fn name_owner(
        &self,
        label: &str,
        purpose: Purpose,
    ) -> Result<Answer<Option<String>>, NamesError> {
        name_owner_on(&mut *self.conn().await?, label, purpose).await
    }

    async fn name_owner_on(
        &self,
        conn: &mut PgConnection,
        label: &str,
        purpose: Purpose,
    ) -> Result<Answer<Option<String>>, NamesError> {
        name_owner_on(conn, label, purpose).await
    }

    /// An ambiguous label is left out of the map: unknown, never `None`.
    async fn name_owners(
        &self,
        labels: &[String],
        purpose: Purpose,
    ) -> Result<Answer<HashMap<String, Option<String>>>, NamesError> {
        let mut labels: Vec<String> = labels.iter().map(|l| l.to_lowercase()).collect();
        labels.sort_unstable();
        labels.dedup();
        let (observed, holders) = holders_on(&mut *self.conn().await?, &labels, purpose).await?;
        let mut out = HashMap::with_capacity(labels.len());
        for label in labels {
            match sole_holder(&label, holders.get(&label)) {
                Ok(owner) => {
                    out.insert(label, owner);
                }
                Err(e) => tracing::warn!(%label, error = %e, "DCL NAME owner left unknown"),
            }
        }
        Ok(Answer {
            value: out,
            observation: Some(observed),
        })
    }

    async fn still_current(
        &self,
        observation: &Observation,
        purpose: Purpose,
    ) -> Result<bool, NamesError> {
        Ok(
            match check(&mut *self.conn().await?, purpose, Utc::now()).await {
                Ok(now) => {
                    now.folded_block == observation.folded_block
                        && now.folded_at_us == observation.folded_at_us
                }
                Err(NamesError::NotEstablished(_)) => false,
                Err(e) => return Err(e),
            },
        )
    }

    fn observes(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::market::owned_items::test_support::publish;
    use crate::market::tests::support::OwnDb;

    const A: &str = "0xaaaa000000000000000000000000000000000001";
    const B: &str = "0xbbbb000000000000000000000000000000000002";

    async fn mint(pool: &PgPool, token: i64, owner: &str, name: &str) {
        sqlx::query(
            "INSERT INTO shop.nft (network, contract_address, token_id, owner, ens_subdomain, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
             VALUES ('ethereum', $1, $2, $3, $4, 1, 1, 1, 0, 1)",
        )
        .bind(DCL_REGISTRAR)
        .bind(token)
        .bind(owner)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    }

    /// The Ethereum walk's cursor: walked through `last`, folded and
    /// published through `folded` now.
    async fn cursor_at(pool: &PgPool, last: i64, folded: i64) {
        sqlx::query(
            "UPDATE shop.chain_cursor SET last_block = $1, fold_through = $2, folded_block = $2, \
                 folded_at = CASE WHEN folded_block = $2 THEN folded_at ELSE timezone('UTC', now()) END \
             WHERE indexer = 'nft' AND network = 'ethereum'",
        )
        .bind(last)
        .bind(folded)
        .execute(pool)
        .await
        .unwrap();
    }

    /// A walked event at `block` of `contract`'s `token` (a pass commits it
    /// with its `last_block`, before folding it).
    async fn walked(pool: &PgPool, block: i64, contract: &str, token: i64) {
        sqlx::query(
            "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address) \
             VALUES ('ethereum', $1, 0, 1, 'transfer', $2, $2, $3, $4, $5)",
        )
        .bind(block)
        .bind(contract)
        .bind(token)
        .bind(A)
        .bind(B)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn published() -> Option<(OwnDb, IndexNames)> {
        let db = OwnDb::create("names").await?;
        publish(&db.pool, &[ChainNet::Ethereum], false, Duration::ZERO).await;
        let names = IndexNames::new(db.pool.clone());
        Some((db, names))
    }

    /// An authorization waits for the fold: a pass that walked a transfer
    /// and has not folded it establishes no holder (the fold still names the
    /// former one) and withdraws every observation read before it; a display
    /// read still shows the publication. The published fold names B under a
    /// new observation.
    #[tokio::test]
    async fn an_authorization_waits_for_the_fold() {
        let Some((db, names)) = published().await else {
            return;
        };
        mint(&db.pool, 7, A, "Alice").await;
        let warm = names.name_owner("ALICE", Purpose::Authorize).await.unwrap();
        assert_eq!(warm.value.as_deref(), Some(A));
        // A label arrives bare: nothing here strips a suffix.
        let suffixed = names
            .name_owner("alice.dcl.eth", Purpose::Authorize)
            .await
            .unwrap();
        assert_eq!(suffixed.value, None);
        let observed = warm.observation.unwrap();
        assert!(
            names
                .still_current(&observed, Purpose::Authorize)
                .await
                .unwrap()
        );

        walked(&db.pool, 101, DCL_REGISTRAR, 7).await;
        cursor_at(&db.pool, 101, 100).await;
        for refused in [
            names
                .name_owner("alice", Purpose::Authorize)
                .await
                .map(|_| ()),
            names.owned_names(A, Purpose::Authorize).await.map(|_| ()),
            names
                .name_owners(&["alice".to_string()], Purpose::Authorize)
                .await
                .map(|_| ()),
        ] {
            assert!(
                matches!(refused, Err(NamesError::NotEstablished(_))),
                "{refused:?}"
            );
        }
        assert!(
            !names
                .still_current(&observed, Purpose::Authorize)
                .await
                .unwrap()
        );
        let shown = names.name_owner("alice", Purpose::Display).await.unwrap();
        assert_eq!(
            shown.value.as_deref(),
            Some(A),
            "a display read shows the publication"
        );

        sqlx::query("UPDATE shop.nft SET owner = $1 WHERE token_id = 7")
            .bind(B)
            .execute(&db.pool)
            .await
            .unwrap();
        cursor_at(&db.pool, 101, 101).await;
        let folded = names.name_owner("alice", Purpose::Authorize).await.unwrap();
        assert_eq!(folded.value.as_deref(), Some(B));
        assert_ne!(folded.observation, Some(observed));
        assert!(
            !names
                .still_current(&observed, Purpose::Authorize)
                .await
                .unwrap()
        );
        assert!(
            names
                .owned_names(A, Purpose::Authorize)
                .await
                .unwrap()
                .value
                .is_empty()
        );
        db.drop().await;
    }

    /// Two holders of one label once lowercased establish nobody: the owner
    /// read refuses and the batch leaves the label out; a label nobody holds
    /// is `None`.
    #[tokio::test]
    async fn an_ambiguous_label_fails_closed() {
        let Some((db, names)) = published().await else {
            return;
        };
        mint(&db.pool, 1, A, "Twin").await;
        mint(&db.pool, 2, B, "TWIN").await;
        mint(&db.pool, 3, B, "solo").await;
        assert!(matches!(
            names.name_owner("twin", Purpose::Authorize).await,
            Err(NamesError::NotEstablished(_))
        ));
        let labels = ["Twin".to_string(), "solo".into(), "nobody".into()];
        let owners = names
            .name_owners(&labels, Purpose::Authorize)
            .await
            .unwrap()
            .value;
        assert_eq!(
            owners,
            HashMap::from([
                ("solo".to_string(), Some(B.to_string())),
                ("nobody".into(), None)
            ])
        );
        db.drop().await;
    }

    /// A wallet's list as the subgraph listed it: minted spelling, by NFT id
    /// (`<contract>-<token id>` as text, so token 10 before token 9), and the
    /// price of an open, unexpired order its holder placed (a former
    /// holder's order or an expired one is no price).
    #[tokio::test]
    async fn a_wallet_list_keeps_spelling_order_and_price() {
        let Some((db, names)) = published().await else {
            return;
        };
        mint(&db.pool, 9, A, "zed").await;
        mint(&db.pool, 10, A, "Ann").await;
        mint(&db.pool, 11, A, "Bob").await;
        mint(&db.pool, 12, B, "other").await;
        let now = Utc::now().timestamp_millis();
        for (token, seller, expires, order) in [
            (9i64, A, now + 86_400_000, "0x09"),
            (10, B, now + 86_400_000, "0x10"),
            (11, A, now - 1, "0x11"),
        ] {
            sqlx::query(
                "INSERT INTO shop.nft_order (network, contract_address, token_id, order_id, marketplace_address, status, seller, price, expires_at, created_at, updated_at, updated_block, updated_log) \
                 VALUES ('ethereum', $1, $2, $3, '0x8e5660b4ab70168b5a6feea0e0315cb49c8cd539', 'open', $4, 5000, $5, 1, 1, 5, 2)",
            )
            .bind(DCL_REGISTRAR)
            .bind(token)
            .bind(order)
            .bind(seller)
            .bind(expires)
            .execute(&db.pool)
            .await
            .unwrap();
        }
        let list = names.owned_names(A, Purpose::Display).await.unwrap().value;
        let got: Vec<(&str, &str, Option<&str>)> = list
            .iter()
            .map(|n| (n.name.as_str(), n.token_id.as_str(), n.price.as_deref()))
            .collect();
        assert_eq!(
            got,
            [
                ("Ann", "10", None),
                ("Bob", "11", None),
                ("zed", "9", Some("5000"))
            ]
        );
        db.drop().await;
    }

    /// The Admin Toolkit's owner read tells a NAME nobody holds (`Ok(None)`)
    /// from an index that cannot say (an error): withdrawn, the moderation
    /// write is refused rather than aimed at nobody.
    #[tokio::test]
    async fn the_moderation_owner_tells_absence_from_an_unready_index() {
        let Some((db, _)) = published().await else {
            return;
        };
        mint(&db.pool, 7, A, "Alice").await;
        let state = crate::market::MarketplaceState::new(
            db.pool.clone(),
            crate::http::Outbound::for_tests(),
            crate::market::owned_items::test_support::walking(crate::market::Settings::for_tests()),
        );
        assert_eq!(
            state
                .name_owner("alice")
                .await
                .unwrap()
                .map(|a| a.as_str().to_string()),
            Some(A.to_string())
        );
        assert_eq!(state.name_owner("nobody").await.unwrap(), None);
        // A raw name's `.dcl.eth` is parsed once, at this boundary.
        assert_eq!(
            state
                .name_owner("Alice.DCL.ETH")
                .await
                .unwrap()
                .map(|a| a.as_str().to_string()),
            Some(A.to_string())
        );
        assert_eq!(
            state.name_owner("alice.dcl.eth.dcl.eth").await.unwrap(),
            None
        );
        cursor_at(&db.pool, 101, 100).await;
        assert!(state.name_owner("nobody").await.is_err());
        assert!(state.name_owner("alice").await.is_err());
        db.drop().await;
    }

    /// The source worlds and communities authorize with exists only where
    /// the node walks the index or reads the subgraph; a process reading a
    /// database another one publishes keeps it for the lookups alone.
    #[tokio::test]
    async fn only_a_walking_or_subgraph_node_authorizes_with_its_names() {
        let lazy = PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap();
        let state = |settings| {
            crate::market::MarketplaceState::new(
                lazy.clone(),
                crate::http::Outbound::for_tests(),
                settings,
            )
        };
        let walking = state(crate::market::owned_items::test_support::walking(
            crate::market::Settings::for_tests(),
        ));
        assert!(walking.authorizing_names.is_some());
        let testnet = state(crate::market::Settings {
            eth_network: "sepolia".into(),
            marketplace_subgraph_url: Some("https://reader.example/subgraph".into()),
            ..crate::market::Settings::for_tests()
        });
        assert!(testnet.authorizing_names.is_some());
        let shared = state(crate::market::Settings::for_tests());
        assert!(shared.name_authority.is_some());
        assert!(shared.authorizing_names.is_none());
        let none = state(crate::market::Settings {
            eth_network: "sepolia".into(),
            ..crate::market::Settings::for_tests()
        });
        assert!(none.name_authority.is_none() && none.authorizing_names.is_none());
    }

    /// Every new block opens a window in which the published fold trails the
    /// walk. With no registrar event walked past it the fold is still every
    /// NAME's state: an authorization holds under the same observation, a
    /// memo keeps it, and another contract's events change nothing. The
    /// first registrar event walked refuses until the pass publishes.
    #[tokio::test]
    async fn a_walk_past_the_fold_without_registrar_events_keeps_authorizing() {
        let Some((db, names)) = published().await else {
            return;
        };
        mint(&db.pool, 7, A, "Alice").await;
        let warm = names.name_owner("alice", Purpose::Authorize).await.unwrap();
        let observed = warm.observation.unwrap();
        walked(&db.pool, 101, "0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d", 1).await;
        cursor_at(&db.pool, 101, 100).await;
        let gap = names.name_owner("alice", Purpose::Authorize).await.unwrap();
        assert_eq!(gap.value.as_deref(), Some(A));
        assert_eq!(gap.observation, Some(observed));
        assert!(names.still_current(&observed, Purpose::Authorize).await.unwrap());
        assert!(!names.owned_names(A, Purpose::Authorize).await.unwrap().value.is_empty());

        walked(&db.pool, 102, DCL_REGISTRAR, 7).await;
        cursor_at(&db.pool, 102, 100).await;
        assert!(matches!(
            names.name_owner("alice", Purpose::Authorize).await,
            Err(NamesError::NotEstablished(_))
        ));
        assert!(!names.still_current(&observed, Purpose::Authorize).await.unwrap());
        db.drop().await;
    }

    /// The cursor states no authorization trusts: none at all, quarantined,
    /// bound to another chain, published past the freshness window (which a
    /// display read refuses too).
    #[tokio::test]
    async fn an_unusable_cursor_establishes_nothing() {
        let Some((db, names)) = published().await else {
            return;
        };
        mint(&db.pool, 7, A, "Alice").await;
        let refused = |purpose| {
            let names = &names;
            async move {
                matches!(
                    names.name_owner("alice", purpose).await,
                    Err(NamesError::NotEstablished(_))
                )
            }
        };
        let set = |sql: &'static str| {
            let pool = db.pool.clone();
            async move {
                sqlx::query(sql).execute(&pool).await.unwrap();
            }
        };
        set("UPDATE shop.chain_cursor SET quarantined_at = timezone('UTC', now()) WHERE indexer = 'nft' AND network = 'ethereum'").await;
        assert!(refused(Purpose::Authorize).await, "quarantined");
        set("UPDATE shop.chain_cursor SET quarantined_at = NULL, chain_id = 137 WHERE indexer = 'nft' AND network = 'ethereum'").await;
        assert!(refused(Purpose::Authorize).await, "another chain");
        set("UPDATE shop.chain_cursor SET chain_id = 1, folded_at = timezone('UTC', now()) - interval '31 minutes' WHERE indexer = 'nft' AND network = 'ethereum'").await;
        assert!(refused(Purpose::Authorize).await, "stale for an authorization");
        assert!(refused(Purpose::Display).await, "stale for a display read");
        set("DELETE FROM shop.chain_cursor WHERE indexer = 'nft' AND network = 'ethereum'").await;
        assert!(refused(Purpose::Authorize).await, "no cursor");
        assert!(refused(Purpose::Display).await, "no cursor");
        db.drop().await;
    }
}
