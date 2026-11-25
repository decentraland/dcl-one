//! Polygon Third Party Registry snapshots, versioned by event position so
//! readers can stay on the published cursor while another window is indexed.

use std::collections::{BTreeSet, HashMap};

use alloy_primitives::{Address, B256, U256, hex};
use alloy_sol_types::{SolEvent, sol};
use anyhow::{Context, ensure};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::types::Json as SqlJson;
use sqlx::{PgConnection, PgPool};

use super::chain_walk::{
    ChainIndex, ChainNet, INDEXER_THIRD_PARTY_REGISTRY, LogGroup, LogSource, Owner,
};
use super::third_party::{ThirdPartyProvider, TpContract};
use super::{MarketplaceState, SharedState, Source, lambdas_error, unavailable};
use crate::chain_index::rpc::{RpcLog, hex_to_i64};

pub const CONTRACT: &str = "0x1c436c1efb4608dffdc8bace99d2b03c314f3348";
pub const START_BLOCK: u64 = 26_860_700;

sol! {
    interface Registry {
        event ThirdPartyAdded(string id, string metadata, string resolver, bool approved, address[] managers, uint256 slots, bool programmatic, address sender);
        event ThirdPartyUpdated(string id, string metadata, string resolver, address[] managers, bool[] managerValues, uint256 slots, address sender);
        event ThirdPartyItemSlotsBought(string id, uint256 price, uint256 slots, address sender);
        event ThirdPartyReviewed(string id, bool approved, address sender);
        event ThirdPartyReviewedWithRoot(string id, bytes32 root, bool approved, address sender);
        event ItemSlotsConsumed(string id, uint256 slots, address indexed signer, bytes32 messageHash, address indexed sender);
    }
    interface LegacyRegistry {
        event ThirdPartyAdded(string id, string metadata, string resolver, bool approved, address[] managers, uint256 slots, address sender);
    }
}

#[path = "third_party_registry_repair.rs"]
mod repair;

const TOPICS: [B256; 7] = [
    Registry::ThirdPartyAdded::SIGNATURE_HASH,
    LegacyRegistry::ThirdPartyAdded::SIGNATURE_HASH,
    Registry::ThirdPartyUpdated::SIGNATURE_HASH,
    Registry::ThirdPartyItemSlotsBought::SIGNATURE_HASH,
    Registry::ThirdPartyReviewed::SIGNATURE_HASH,
    Registry::ThirdPartyReviewedWithRoot::SIGNATURE_HASH,
    Registry::ItemSlotsConsumed::SIGNATURE_HASH,
];

#[derive(Deserialize, Serialize)]
struct Metadata {
    name: String,
    description: String,
    contracts: Vec<TpContract>,
}

#[derive(Deserialize, Serialize)]
struct Provider {
    id: String,
    approved: bool,
    root: Option<String>,
    resolver: Option<String>,
    raw_metadata: String,
    metadata: Option<Metadata>,
    managers: BTreeSet<String>,
    programmatic: bool,
    max_items: String,
    consumed_slots: String,
}

impl Provider {
    fn refresh_metadata(&mut self, raw: String, resolver: String) {
        self.resolver = (resolver.starts_with("https://") || resolver.starts_with("http://"))
            .then_some(resolver);
        self.raw_metadata = raw;
        let parts = self.raw_metadata.split(':').collect::<Vec<_>>();
        let (name, description, contracts) = match parts.as_slice() {
            ["tp", _, name, description] => (name, description, None),
            ["tp", _, name, description, contracts] => (name, description, Some(*contracts)),
            _ => {
                self.metadata = None;
                return;
            }
        };
        // The optional contract field replaces the previous linked list when supplied.
        let contracts = match contracts {
            Some(raw) => raw
                .split(';')
                .filter_map(
                    |part| match part.split('-').collect::<Vec<_>>().as_slice() {
                        [network, address] if !network.is_empty() && !address.is_empty() => {
                            Some(TpContract {
                                network: network.to_lowercase(),
                                address: address.to_lowercase(),
                            })
                        }
                        _ => None,
                    },
                )
                .collect(),
            None => self
                .metadata
                .take()
                .map(|m| m.contracts)
                .unwrap_or_default(),
        };
        self.metadata = Some(Metadata {
            name: (*name).to_owned(),
            description: (*description).to_owned(),
            contracts,
        });
    }

    fn details(&self) -> anyhow::Result<Value> {
        let max: U256 = self.max_items.parse()?;
        let consumed: U256 = self.consumed_slots.parse()?;
        Ok(json!({
            "id": self.id,
            "name": self.metadata.as_ref().map(|m| &m.name),
            "description": self.metadata.as_ref().map(|m| &m.description),
            "managers": self.managers,
            "isApproved": self.approved,
            "isProgrammatic": self.programmatic,
            "maxItems": self.max_items,
            "consumedSlots": self.consumed_slots,
            "availableSlots": max.saturating_sub(consumed).to_string(),
        }))
    }
}

enum Change {
    Added {
        metadata: String,
        resolver: String,
        approved: bool,
        managers: Vec<Address>,
        slots: U256,
        programmatic: bool,
    },
    Updated(Registry::ThirdPartyUpdated),
    Bought(U256),
    Reviewed {
        approved: bool,
        root: Option<B256>,
    },
    Consumed(Registry::ItemSlotsConsumed),
}

struct Event {
    block: i64,
    index: i32,
    id: String,
    change: Change,
}

#[derive(Serialize)]
struct Receipt {
    hash: String,
    quantity: String,
    signer: String,
    curator: String,
}

fn decode(log: &RpcLog) -> anyhow::Result<Event> {
    ensure!(
        log.address.eq_ignore_ascii_case(CONTRACT),
        "registry log from another contract"
    );
    let topics = log
        .topics
        .iter()
        .map(|topic| topic.parse())
        .collect::<Result<Vec<B256>, _>>()?;
    let data = hex::decode(
        log.data
            .strip_prefix("0x")
            .context("registry log has no data prefix")?,
    )?;
    let (id, change) = match *topics.first().context("registry log has no topic")? {
        Registry::ThirdPartyAdded::SIGNATURE_HASH => {
            let event = Registry::ThirdPartyAdded::decode_raw_log_validate(&topics, &data)?;
            (
                event.id,
                Change::Added {
                    metadata: event.metadata,
                    resolver: event.resolver,
                    approved: event.approved,
                    managers: event.managers,
                    slots: event.slots,
                    programmatic: event.programmatic,
                },
            )
        }
        LegacyRegistry::ThirdPartyAdded::SIGNATURE_HASH => {
            let event = LegacyRegistry::ThirdPartyAdded::decode_raw_log_validate(&topics, &data)?;
            (
                event.id,
                Change::Added {
                    metadata: event.metadata,
                    resolver: event.resolver,
                    approved: event.approved,
                    managers: event.managers,
                    slots: event.slots,
                    programmatic: false,
                },
            )
        }
        Registry::ThirdPartyUpdated::SIGNATURE_HASH => {
            let event = Registry::ThirdPartyUpdated::decode_raw_log_validate(&topics, &data)?;
            (event.id.clone(), Change::Updated(event))
        }
        Registry::ThirdPartyItemSlotsBought::SIGNATURE_HASH => {
            let event =
                Registry::ThirdPartyItemSlotsBought::decode_raw_log_validate(&topics, &data)?;
            (event.id, Change::Bought(event.slots))
        }
        Registry::ThirdPartyReviewed::SIGNATURE_HASH => {
            let event = Registry::ThirdPartyReviewed::decode_raw_log_validate(&topics, &data)?;
            (
                event.id,
                Change::Reviewed {
                    approved: event.approved,
                    root: None,
                },
            )
        }
        Registry::ThirdPartyReviewedWithRoot::SIGNATURE_HASH => {
            let event =
                Registry::ThirdPartyReviewedWithRoot::decode_raw_log_validate(&topics, &data)?;
            (
                event.id,
                Change::Reviewed {
                    approved: event.approved,
                    root: Some(event.root),
                },
            )
        }
        Registry::ItemSlotsConsumed::SIGNATURE_HASH => {
            let event = Registry::ItemSlotsConsumed::decode_raw_log_validate(&topics, &data)?;
            (event.id.clone(), Change::Consumed(event))
        }
        _ => anyhow::bail!("unknown registry event"),
    };
    let block = hex_to_i64(&log.block_number)
        .filter(|n| *n >= 0)
        .context("invalid registry block")?;
    let index = hex_to_i64(&log.log_index)
        .and_then(|n| i32::try_from(n).ok())
        .filter(|n| *n >= 0)
        .context("invalid registry log index")?;
    Ok(Event {
        block,
        index,
        id,
        change,
    })
}

fn valid_id(id: &str) -> bool {
    let parts = id.split(':').collect::<Vec<_>>();
    matches!(
        parts.as_slice(),
        [
            "urn",
            "decentraland",
            _,
            "collections-thirdparty" | "collections-linked-wearables",
            _
        ]
    )
}

fn add_slots(count: &mut String, delta: U256) -> anyhow::Result<()> {
    *count = count
        .parse::<U256>()?
        .checked_add(delta)
        .context("registry slot count overflow")?
        .to_string();
    Ok(())
}

fn apply(
    id: String,
    change: Change,
    previous: Option<Provider>,
) -> anyhow::Result<Option<(Provider, Option<Receipt>)>> {
    let change = match change {
        Change::Added {
            metadata,
            resolver,
            approved,
            managers,
            slots,
            programmatic,
        } => {
            let mut provider = Provider {
                id,
                approved,
                root: None,
                resolver: None,
                raw_metadata: String::new(),
                metadata: None,
                managers: managers.iter().map(|a| format!("{a:#x}")).collect(),
                programmatic,
                max_items: slots.to_string(),
                consumed_slots: "0".into(),
            };
            provider.refresh_metadata(metadata, resolver);
            return Ok(Some((provider, None)));
        }
        change => change,
    };
    let Some(mut provider) = previous else {
        return Ok(None);
    };
    let mut receipt = None;
    match change {
        Change::Added { .. } => unreachable!("provider creation handled above"),
        Change::Updated(event) => {
            ensure!(
                event.managers.len() == event.managerValues.len(),
                "registry manager/value count mismatch"
            );
            for (manager, enabled) in event.managers.into_iter().zip(event.managerValues) {
                let manager = format!("{manager:#x}");
                if enabled {
                    provider.managers.insert(manager);
                } else {
                    provider.managers.remove(&manager);
                }
            }
            provider.refresh_metadata(event.metadata, event.resolver);
            add_slots(&mut provider.max_items, event.slots)?;
        }
        Change::Bought(slots) => add_slots(&mut provider.max_items, slots)?,
        Change::Reviewed { approved, root } => {
            if let Some(root) = root {
                provider.root = Some(format!("{root:#x}"));
            }
            provider.approved = approved;
        }
        Change::Consumed(event) => {
            add_slots(&mut provider.consumed_slots, event.slots)?;
            receipt = Some(Receipt {
                hash: format!("{:#x}", event.messageHash),
                quantity: event.slots.to_string(),
                signer: format!("{:#x}", event.signer),
                curator: format!("{:#x}", event.sender),
            });
        }
    }
    Ok(Some((provider, receipt)))
}

fn log_group() -> LogGroup {
    let topics: Vec<String> = TOPICS.iter().map(|topic| format!("{topic:#x}")).collect();
    LogGroup {
        addresses: vec![CONTRACT.into()],
        topics: json!([topics]),
    }
}

pub struct ThirdPartyRegistryIndex;

#[async_trait::async_trait]
impl ChainIndex for ThirdPartyRegistryIndex {
    fn indexer(&self) -> &'static str {
        INDEXER_THIRD_PARTY_REGISTRY
    }

    fn net(&self) -> ChainNet {
        ChainNet::Matic
    }

    fn start_block(&self) -> u64 {
        START_BLOCK
    }

    async fn groups(
        &self,
        _: &PgPool,
        _: u64,
        _: u64,
        _: (u64, i64),
    ) -> anyhow::Result<Vec<LogGroup>> {
        Ok(vec![log_group()])
    }

    async fn write(
        &self,
        conn: &mut PgConnection,
        logs: &[RpcLog],
        times: &HashMap<i64, i64>,
    ) -> anyhow::Result<usize> {
        let mut events = logs
            .iter()
            .map(decode)
            .collect::<anyhow::Result<Vec<_>>>()?;
        events.sort_unstable_by_key(|event| (event.block, event.index));
        let mut written = 0;
        for event in events {
            if !valid_id(&event.id) {
                continue;
            }
            let previous = sqlx::query_scalar::<_, SqlJson<Provider>>(
                "SELECT state FROM shop.third_party_registry_event
                 WHERE provider_id = $1 AND (block_number, log_index) < ($2, $3)
                 ORDER BY block_number DESC, log_index DESC LIMIT 1",
            )
            .bind(&event.id)
            .bind(event.block)
            .bind(event.index)
            .fetch_optional(&mut *conn)
            .await?
            .map(|state| state.0);
            let is_review = matches!(event.change, Change::Reviewed { .. });
            if let Some((provider, receipt)) = apply(event.id, event.change, previous)? {
                written += sqlx::query(
                    "INSERT INTO shop.third_party_registry_event
                     (block_number, log_index, block_time, provider_id, state, receipt, is_review)
                     VALUES ($1, $2, $3, $4, $5, $6, $7)
                     ON CONFLICT (block_number, log_index) DO NOTHING",
                )
                .bind(event.block)
                .bind(event.index)
                .bind(
                    times
                        .get(&event.block)
                        .context("missing registry block timestamp")?,
                )
                .bind(&provider.id)
                .bind(SqlJson(&provider))
                .bind(receipt.map(SqlJson))
                .bind(is_review)
                .execute(&mut *conn)
                .await?
                .rows_affected();
            }
        }
        Ok(written as usize)
    }

    async fn after_publish(
        &self,
        pool: &PgPool,
        source: &dyn LogSource,
        owner: &Owner,
        window_blocks: u64,
        remaining: usize,
    ) -> anyhow::Result<usize> {
        repair::backfill_reviews(pool, source, owner, window_blocks, remaining).await
    }

    async fn rewind(&self, conn: &mut PgConnection, to: i64) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM shop.third_party_registry_event WHERE block_number > $1")
            .bind(to)
            .execute(conn)
            .await?;
        Ok(())
    }

    async fn fold(&self, _: &PgPool, _: Option<i64>, _: i64, _: &Owner) -> anyhow::Result<bool> {
        Ok(true)
    }
}

pub(super) fn enabled(state: &MarketplaceState) -> bool {
    state.settings.market.indexes.third_party_registry_enabled
}

async fn read(
    pool: &PgPool,
    id: Option<&str>,
    manager: Option<&str>,
    approved: bool,
) -> anyhow::Result<Option<Vec<Provider>>> {
    let result = sqlx::query_scalar::<_, Option<SqlJson<Vec<Provider>>>>(
        "WITH published AS (
           SELECT folded_block FROM shop.chain_cursor
           WHERE indexer = $1 AND network = 'matic' AND chain_id = 137 AND full_history
             AND quarantined_at IS NULL AND diverged_at IS NULL AND folded_block IS NOT NULL
             AND folded_at BETWEEN timezone('UTC', now()) - interval '30 minutes' AND timezone('UTC', now())
         ), latest AS (
           SELECT DISTINCT ON (e.provider_id) e.provider_id, e.state
           FROM shop.third_party_registry_event e CROSS JOIN published p
           WHERE e.block_number <= p.folded_block AND ($2::text IS NULL OR e.provider_id = $2)
           ORDER BY e.provider_id, e.block_number DESC, e.log_index DESC
         )
         SELECT CASE WHEN EXISTS (SELECT 1 FROM published)
           THEN coalesce(jsonb_agg(state ORDER BY provider_id), '[]'::jsonb)
         END
         FROM latest
         WHERE ($3::text IS NULL OR state->'managers' ? $3)
           AND (NOT $4 OR state->'approved' = 'true'::jsonb)",
    )
    .bind(INDEXER_THIRD_PARTY_REGISTRY)
    .bind(id)
    .bind(manager)
    .bind(approved)
    .fetch_one(pool)
    .await?;
    Ok(result.map(|rows| rows.0))
}

pub(super) async fn approved_providers(pool: &PgPool) -> Result<Vec<ThirdPartyProvider>, String> {
    let providers = read(pool, None, None, true)
        .await
        .map_err(|err| format!("third-party registry index: {err}"))?
        .ok_or_else(|| "third-party registry index has no current publication".to_owned())?;
    Ok(providers
        .into_iter()
        .map(|provider| {
            let (name, description, contracts) = match provider.metadata {
                Some(data) => (
                    Some(json!(data.name)),
                    Some(json!(data.description)),
                    data.contracts,
                ),
                None => (None, None, Vec::new()),
            };
            ThirdPartyProvider {
                id: provider.id,
                name,
                description,
                contracts,
            }
        })
        .collect())
}

async fn details(
    state: &MarketplaceState,
    id: Option<&str>,
    manager: Option<&str>,
) -> Result<Vec<Value>, Response> {
    if !enabled(state) {
        return Err(unavailable(
            Source::ThirdPartyRegistryIndex,
            "The third-party registry index is disabled",
        ));
    }
    match read(&state.pool, id, manager, false)
        .await
        .and_then(|providers| {
            providers
                .map(|providers| providers.iter().map(Provider::details).collect())
                .transpose()
        }) {
        Ok(Some(providers)) => Ok(providers),
        Ok(None) => Err(unavailable(
            Source::ThirdPartyRegistryIndex,
            "The third-party registry index has no current publication",
        )),
        Err(error) => {
            tracing::error!(%error, "third-party registry index query failed");
            Err(unavailable(
                Source::ThirdPartyRegistryIndex,
                "Cannot read the third-party registry index",
            ))
        }
    }
}

pub(super) async fn managed_providers(
    State(state): State<SharedState>,
    Path(address): Path<String>,
) -> Response {
    if !super::is_valid_eth_address(&address) {
        return lambdas_error(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "Invalid manager address",
        );
    }
    match details(&state, None, Some(&address.to_ascii_lowercase())).await {
        Ok(providers) => Json(providers).into_response(),
        Err(response) => response,
    }
}

pub(super) async fn provider(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    match details(&state, Some(&id), None).await {
        Ok(mut providers) => match providers.pop() {
            Some(provider) => Json(provider).into_response(),
            None => lambdas_error(
                StatusCode::NOT_FOUND,
                "Not Found",
                "No such third-party provider",
            ),
        },
        Err(response) => response,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain_index::{
        logs::{Read, Window},
        rpc::{BlockHeader, RpcError},
    };
    use crate::market::tests::support::OwnDb;
    use crate::market::{Settings, router};
    use alloy_primitives::address;
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use std::sync::Arc;
    use tower::ServiceExt;

    const ID: &str = "urn:decentraland:matic:collections-linked-wearables:test";
    const LEGACY_ID: &str = "urn:decentraland:matic:collections-thirdparty:legacy";
    const MANAGER: Address = address!("d8da6bf26964af9d7eed9e03e53415d37aa96045");
    const NEW_MANAGER: Address = address!("0000000000000000000000000000000000000010");
    const START: i64 = START_BLOCK as i64;

    fn log(event: impl SolEvent, block: i64, index: i32) -> RpcLog {
        let log = event.encode_log_data();
        RpcLog {
            address: CONTRACT.into(),
            topics: log
                .topics()
                .iter()
                .map(|topic| format!("{topic:#x}"))
                .collect(),
            data: format!("0x{}", hex::encode(log.data)),
            block_number: format!("{block:#x}"),
            log_index: format!("{index:#x}"),
            block_timestamp: None,
            block_hash: None,
            transaction_hash: None,
        }
    }

    async fn get(state: &SharedState, path: &str) -> (StatusCode, Value) {
        let response = router(state.clone())
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }

    #[test]
    fn validates_event_emitter_and_indexed_topics() {
        let mut log = log(
            Registry::ItemSlotsConsumed {
                id: ID.into(),
                slots: U256::from(3),
                signer: MANAGER,
                messageHash: B256::repeat_byte(1),
                sender: NEW_MANAGER,
            },
            START,
            0,
        );
        let event = decode(&log).unwrap();
        assert!(
            matches!(event.change, Change::Consumed(e) if e.signer == MANAGER && e.sender == NEW_MANAGER)
        );
        log.topics.pop();
        assert!(decode(&log).is_err());
        log.address = format!("{MANAGER:#x}");
        assert!(decode(&log).is_err());
    }

    #[test]
    fn registry_approval_is_independent_of_a_merkle_root() {
        let add = log(
            Registry::ThirdPartyAdded {
                id: ID.into(),
                metadata: "tp:1:Provider:Example".into(),
                resolver: String::new(),
                approved: true,
                managers: vec![MANAGER],
                slots: U256::ZERO,
                programmatic: false,
                sender: MANAGER,
            },
            START,
            0,
        );
        let event = decode(&add).unwrap();
        let (mut provider, _) = apply(event.id, event.change, None).unwrap().unwrap();
        assert!(provider.approved && provider.root.is_none());

        provider.approved = false;
        let review = log(
            Registry::ThirdPartyReviewed {
                id: ID.into(),
                approved: true,
                sender: MANAGER,
            },
            START,
            1,
        );
        let event = decode(&review).unwrap();
        let (provider, _) = apply(event.id, event.change, Some(provider))
            .unwrap()
            .unwrap();
        assert!(provider.approved && provider.root.is_none());
    }

    struct RepairArchive {
        logs: Vec<RpcLog>,
        tip: u64,
        changed_tip_read: Option<usize>,
        tip_reads: std::sync::atomic::AtomicUsize,
    }

    fn repair_header(number: u64) -> crate::chain_index::rpc::BlockHeader {
        crate::chain_index::rpc::BlockHeader {
            number,
            hash: format!("0x{number:064x}"),
            timestamp: number as i64,
        }
    }

    #[async_trait::async_trait]
    impl LogSource for RepairArchive {
        async fn chain_id(&self) -> Result<Read<u64>, RpcError> {
            unreachable!("the writer already proved its chain")
        }

        async fn head(&self) -> Result<Read<u64>, RpcError> {
            unreachable!("repair uses the published fold")
        }

        async fn finalized(&self) -> Result<Read<BlockHeader>, RpcError> {
            unreachable!("repair checks the publication's specific header")
        }

        async fn header(&self, number: u64) -> Result<Read<BlockHeader>, RpcError> {
            let mut value = repair_header(number);
            if number == self.tip
                && self.changed_tip_read
                    == Some(
                        self.tip_reads
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                    )
            {
                value.hash = repair_header(number + 1).hash;
            }
            Ok(Read { value, requests: 1 })
        }

        async fn window(
            &self,
            _: u64,
            _: u64,
            _: &[LogGroup],
            _: &mut usize,
        ) -> Result<Window, RpcError> {
            unreachable!("repair uses the bounded archive reader")
        }

        async fn archive_window(
            &self,
            from: u64,
            to: u64,
            groups: &[LogGroup],
            _: &mut usize,
            budget: usize,
        ) -> Result<Window, RpcError> {
            assert_eq!(groups.len(), 1);
            assert_eq!(groups[0].addresses, log_group().addresses);
            assert_eq!(groups[0].topics, log_group().topics);
            assert!(budget >= 1);
            Ok(Window {
                logs: self
                    .logs
                    .iter()
                    .filter(|log| {
                        (from as i64..=to as i64)
                            .contains(&crate::chain_index::logs::log_position(log).0)
                    })
                    .cloned()
                    .collect(),
                reached: to,
                requests: 1,
                charge: 1,
                ..Window::default()
            })
        }

        async fn block_times(&self, _: &[RpcLog]) -> Result<Read<HashMap<i64, i64>>, RpcError> {
            unreachable!("the archived flags do not change block timestamps")
        }
    }

    #[tokio::test]
    async fn review_repair_classifies_old_rows_only_after_a_complete_canonical_read() {
        let Some(db) = OwnDb::create("third_party_review_repair").await else {
            return;
        };
        let pool = &db.pool;
        let tip = START_BLOCK + 3;
        let mut logs = vec![
            log(
                Registry::ThirdPartyAdded {
                    id: ID.into(),
                    metadata: "tp:1:Provider:Example".into(),
                    resolver: String::new(),
                    approved: false,
                    managers: vec![MANAGER],
                    slots: U256::ZERO,
                    programmatic: false,
                    sender: MANAGER,
                },
                START,
                0,
            ),
            log(
                Registry::ThirdPartyReviewed {
                    id: ID.into(),
                    approved: true,
                    sender: MANAGER,
                },
                START + 1,
                0,
            ),
            log(
                Registry::ThirdPartyReviewedWithRoot {
                    id: ID.into(),
                    root: B256::repeat_byte(1),
                    approved: true,
                    sender: MANAGER,
                },
                START + 2,
                0,
            ),
        ];
        for (offset, entry) in logs.iter_mut().enumerate() {
            entry.block_hash = Some(repair_header(START_BLOCK + offset as u64).hash);
        }
        let index = ThirdPartyRegistryIndex;
        let settings = crate::market::chain_walk::IndexSettings::default();
        let owner = crate::market::chain_walk::Holdings::default()
            .owner(pool, None, &index, &settings)
            .await
            .unwrap();
        let mut tx = pool.begin().await.unwrap();
        let times = (START..=START + 2).map(|block| (block, block)).collect();
        assert_eq!(index.write(&mut tx, &logs, &times).await.unwrap(), 3);
        sqlx::query(
            "UPDATE shop.chain_cursor SET last_block=$2::bigint+2,folded_block=$2,folded_at=timezone('UTC',now())
             WHERE indexer=$1 AND network='matic'",
        ).bind(index.indexer()).bind(tip as i64).execute(&mut *tx).await.unwrap();
        sqlx::query(
            "INSERT INTO shop.chain_checkpoint(indexer,network,block_number,block_hash,block_time)
             VALUES ($1,'matic',$2,$3,$2)",
        )
        .bind(index.indexer())
        .bind(tip as i64)
        .bind(repair_header(tip).hash)
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let flags = || {
            sqlx::query_scalar::<_, Option<bool>>(
            "SELECT is_review FROM shop.third_party_registry_event ORDER BY block_number,log_index",
        ).fetch_all(pool)
        };
        assert_eq!(
            flags().await.unwrap(),
            [Some(false), Some(true), Some(true)]
        );
        sqlx::query(
            "UPDATE shop.third_party_registry_event SET is_review=CASE WHEN block_number<$1 THEN NULL ELSE false END",
        )
        .bind(START + 2)
        .execute(pool)
        .await
        .unwrap();
        let pending = [None, None, Some(false)];
        assert_eq!(flags().await.unwrap(), pending);

        let mut settings = Settings::for_tests();
        settings.market.indexes.third_party_registry_enabled = true;
        let state = Arc::new(MarketplaceState::new(
            pool.clone(),
            crate::http::Outbound::for_tests(),
            settings,
        ));
        let path = format!("/lambdas/third-party-integrations/providers/{ID}");
        let current_provider = get(&state, &path).await;
        assert_eq!(current_provider.0, StatusCode::OK);

        let archive = |logs, changed_tip_read| RepairArchive {
            logs,
            tip,
            changed_tip_read,
            tip_reads: 0.into(),
        };
        let mut wrong_branch = logs.clone();
        wrong_branch[1].block_hash = Some(repair_header(tip + 1).hash);
        for (source, expected) in [
            (
                archive(vec![logs[0].clone(), logs[2].clone()], None),
                "archive omitted an indexed event",
            ),
            (
                archive(wrong_branch, None),
                "log was not on the canonical branch",
            ),
            (archive(logs.clone(), Some(0)), "anchor changed"),
            (archive(logs.clone(), Some(1)), "anchor changed"),
        ] {
            let error = index
                .after_publish(pool, &source, &owner, 10, 10)
                .await
                .unwrap_err();
            assert!(error.to_string().contains(expected), "{error:#}");
            assert_eq!(flags().await.unwrap(), pending);
        }

        let source = archive(logs, None);
        assert_eq!(
            index
                .after_publish(pool, &source, &owner, 10, 4)
                .await
                .unwrap(),
            4
        );
        assert_eq!(flags().await.unwrap(), [Some(false), None, Some(false)]);
        let new_owner = crate::market::chain_walk::Holdings::default()
            .owner(
                pool,
                None,
                &index,
                &crate::market::chain_walk::IndexSettings::default(),
            )
            .await
            .unwrap();
        let error = index
            .after_publish(pool, &source, &owner, 10, 10)
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("another job took it"),
            "{error:#}"
        );
        assert_eq!(flags().await.unwrap(), [Some(false), None, Some(false)]);
        index
            .after_publish(pool, &source, &new_owner, 10, 10)
            .await
            .unwrap();
        assert_eq!(
            flags().await.unwrap(),
            [Some(false), Some(true), Some(false)]
        );
        assert_eq!(get(&state, &path).await, current_provider);
        db.drop().await;
    }

    #[tokio::test]
    async fn provider_publication_is_versioned_and_rewindable() {
        let Some(db) = OwnDb::create("third_party_registry").await else {
            return;
        };
        let pool = &db.pool;
        let mut settings = Settings::for_tests();
        settings.market.indexes.third_party_registry_enabled = true;
        let state = Arc::new(MarketplaceState::new(
            pool.clone(),
            crate::http::Outbound::for_tests(),
            settings,
        ));
        let mut fallback_settings = Settings::for_tests();
        fallback_settings.eth_network = "sepolia".into();
        fallback_settings
            .market
            .indexes
            .third_party_registry_enabled = true;
        fallback_settings.third_party_registry_subgraph_url =
            Some(crate::market::tests::support::serve_empty_graph().await);
        let fallback_state = Arc::new(MarketplaceState::new(
            pool.clone(),
            crate::http::Outbound::for_tests(),
            fallback_settings,
        ));
        fallback_state
            .test_allow_loopback
            .store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(fallback_state.polygon_mainnet_rpc_url().is_none());
        let path = format!("/lambdas/third-party-integrations/providers/{ID}");
        let manager = format!("{MANAGER:#x}");
        let managers_path = format!(
            "/lambdas/third-party-integrations/managers/0x{}",
            manager[2..].to_ascii_uppercase()
        );
        let integrations = "/lambdas/third-party-integrations";
        assert_eq!(get(&state, &path).await.0, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(get(&state, integrations).await.0, StatusCode::BAD_GATEWAY);
        assert_eq!(
            get(&fallback_state, integrations).await,
            (StatusCode::OK, json!({"data": []}))
        );
        assert_eq!(
            get(&fallback_state, &path).await.0,
            StatusCode::SERVICE_UNAVAILABLE
        );

        let logs = [
            log(
                LegacyRegistry::ThirdPartyAdded {
                    id: LEGACY_ID.into(),
                    metadata: "tp:1:Legacy:Old:matic-0xabc".into(),
                    resolver: "ignored".into(),
                    approved: true,
                    managers: vec![MANAGER],
                    slots: U256::from(7),
                    sender: MANAGER,
                },
                START,
                0,
            ),
            log(
                Registry::ThirdPartyAdded {
                    id: ID.into(),
                    metadata: "tp:1:Provider:Example:MATIC-0xAbC;malformed;MAINNET-0xDeF".into(),
                    resolver: "https://example.org/resolve".into(),
                    approved: true,
                    managers: vec![MANAGER],
                    slots: U256::from(12),
                    programmatic: true,
                    sender: MANAGER,
                },
                START,
                1,
            ),
            log(
                Registry::ThirdPartyReviewedWithRoot {
                    id: ID.into(),
                    root: B256::repeat_byte(1),
                    approved: true,
                    sender: MANAGER,
                },
                START,
                2,
            ),
            log(
                Registry::ThirdPartyUpdated {
                    id: ID.into(),
                    metadata: "tp:1:Renamed:Changed".into(),
                    resolver: String::new(),
                    managers: vec![MANAGER, NEW_MANAGER],
                    managerValues: vec![false, true],
                    slots: U256::from(2),
                    sender: MANAGER,
                },
                START + 1,
                0,
            ),
            log(
                Registry::ThirdPartyItemSlotsBought {
                    id: ID.into(),
                    price: U256::from(20),
                    slots: U256::from(9),
                    sender: NEW_MANAGER,
                },
                START + 1,
                1,
            ),
            log(
                Registry::ItemSlotsConsumed {
                    id: ID.into(),
                    slots: U256::from(3),
                    signer: NEW_MANAGER,
                    messageHash: B256::repeat_byte(2),
                    sender: MANAGER,
                },
                START + 1,
                2,
            ),
            log(
                Registry::ThirdPartyReviewed {
                    id: ID.into(),
                    approved: false,
                    sender: MANAGER,
                },
                START + 1,
                3,
            ),
            log(
                Registry::ThirdPartyUpdated {
                    id: LEGACY_ID.into(),
                    metadata: "invalid metadata".into(),
                    resolver: String::new(),
                    managers: vec![],
                    managerValues: vec![],
                    slots: U256::ZERO,
                    sender: MANAGER,
                },
                START + 1,
                4,
            ),
        ];

        let indexer = ThirdPartyRegistryIndex;
        let times = HashMap::from([(START, 1234), (START + 1, 1235)]);
        let mut tx = pool.begin().await.unwrap();
        assert_eq!(indexer.write(&mut tx, &logs, &times).await.unwrap(), 8);
        assert_eq!(indexer.write(&mut tx, &logs, &times).await.unwrap(), 0);
        sqlx::query(
            "INSERT INTO shop.chain_cursor
             (indexer, network, start_block, last_block, full_history, folded_block, folded_at, chain_id)
             VALUES ($1, 'matic', $2, $2 + 1, true, $2, timezone('UTC', now()), 137)",
        ).bind(INDEXER_THIRD_PARTY_REGISTRY).bind(START).execute(&mut *tx).await.unwrap();
        tx.commit().await.unwrap();

        assert_eq!(
            get(&state, &path).await,
            (
                StatusCode::OK,
                json!({
                    "id": ID, "name": "Provider", "description": "Example", "managers": [format!("{MANAGER:#x}")],
                    "isApproved": true, "isProgrammatic": true, "maxItems": "12", "consumedSlots": "0", "availableSlots": "12",
                })
            )
        );
        assert_eq!(
            get(&fallback_state, integrations).await,
            get(&state, integrations).await
        );
        let approved = approved_providers(pool).await.unwrap();
        assert_eq!(approved.len(), 2);
        assert_eq!(approved[1].id, LEGACY_ID);
        assert_eq!(approved[1].contracts.len(), 1);
        assert_eq!(
            approved[0].contracts,
            [
                TpContract {
                    network: "matic".into(),
                    address: "0xabc".into()
                },
                TpContract {
                    network: "mainnet".into(),
                    address: "0xdef".into()
                },
            ]
        );
        assert_eq!(
            get(&state, integrations).await.1,
            json!({
                "data": [
                    {"name": "Provider", "description": "Example", "urn": ID},
                    {"name": "Legacy", "description": "Old", "urn": LEGACY_ID},
                ],
            })
        );

        sqlx::query("UPDATE shop.chain_cursor SET folded_block = last_block WHERE indexer = $1")
            .bind(INDEXER_THIRD_PARTY_REGISTRY)
            .execute(pool)
            .await
            .unwrap();
        let (status, provider) = get(&state, &path).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(provider["name"], "Renamed");
        assert_eq!(provider["managers"], json!([format!("{NEW_MANAGER:#x}")]));
        assert_eq!(provider["maxItems"], "23");
        assert_eq!(provider["consumedSlots"], "3");
        assert_eq!(provider["availableSlots"], "20");
        assert_eq!(provider["isApproved"], false);
        let (status, managed) = get(&state, &managers_path).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            managed.as_array().unwrap().len(),
            1,
            "removed manager retains just the legacy provider"
        );
        assert_eq!(managed[0]["id"], LEGACY_ID);
        let approved = approved_providers(pool).await.unwrap();
        let [legacy] = approved.as_slice() else {
            panic!("the approved legacy provider remains listed");
        };
        assert_eq!(legacy.id, LEGACY_ID);
        assert_eq!(legacy.name, None);
        assert_eq!(legacy.description, None);
        assert!(legacy.contracts.is_empty());
        assert_eq!(
            get(&state, integrations).await.1,
            json!({"data": [{"urn": LEGACY_ID}]})
        );
        let receipt: Value = sqlx::query_scalar(
            "SELECT receipt FROM shop.third_party_registry_event WHERE block_number = $1 AND receipt IS NOT NULL",
        ).bind(START + 1).fetch_one(pool).await.unwrap();
        assert_eq!(
            receipt,
            json!({
                "hash": format!("{:#x}", B256::repeat_byte(2)), "quantity": "3",
                "signer": format!("{NEW_MANAGER:#x}"), "curator": format!("{MANAGER:#x}"),
            })
        );

        let mut tx = pool.begin().await.unwrap();
        indexer.rewind(&mut tx, START).await.unwrap();
        sqlx::query(
            "UPDATE shop.chain_cursor SET folded_block = $2, last_block = $2 WHERE indexer = $1",
        )
        .bind(INDEXER_THIRD_PARTY_REGISTRY)
        .bind(START)
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(get(&state, &path).await.1["availableSlots"], "12");
        assert_eq!(get(&state, &path).await.1["isApproved"], true);
        assert_eq!(
            get(
                &state,
                "/lambdas/third-party-integrations/providers/missing"
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            get(
                &state,
                "/lambdas/third-party-integrations/managers/0xinvalid"
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );

        sqlx::query(
            "UPDATE shop.chain_cursor SET folded_at = timezone('UTC', now()) - interval '31 minutes' WHERE indexer = $1",
        ).bind(INDEXER_THIRD_PARTY_REGISTRY).execute(pool).await.unwrap();
        assert_eq!(get(&state, &path).await.0, StatusCode::SERVICE_UNAVAILABLE);
        drop(fallback_state);
        drop(state);
        db.drop().await;
    }
}
