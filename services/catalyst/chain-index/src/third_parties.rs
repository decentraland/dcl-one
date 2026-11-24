//! The third-party registry: every provider the registry subgraph holds,
//! approved or not, with its managers, merkle root, item allowance, approval,
//! programmatic flag and metadata, as builder-server's `thirdPartyFragment`
//! reads them. The walk pages by id under [`MAX_PAGES`] through
//! [`crate::graph::graph_query`] (the SSRF guard, the body cap and the retry
//! ladder), and every record is validated before use. A record whose id is
//! a string but whose other fields break the shape stands for a provider
//! nobody manages, reported once; one without a string id is left out. An
//! answer that breaks the walk fails the whole read, so no decision is made
//! on part of the registry.

use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};

use serde_json::{Value, json};

use crate::ChainIndex;
use crate::graph::{THE_GRAPH_PAGE_SIZE, graph_query};
use crate::shared::identity::is_valid_eth_address;

/// Pages of [`THE_GRAPH_PAGE_SIZE`] providers walked before the read fails.
pub const MAX_PAGES: usize = 10;

const QUERY: &str = "query registry($idFrom: ID!, $first: Int!) {\
  thirdParties(first: $first, where: { id_gt: $idFrom }, orderBy: id, orderDirection: asc) {\
    id root managers maxItems isApproved isProgrammatic \
    metadata { type thirdParty { name description contracts { address network } } }\
  }\
}";

/// A contract a provider links its items to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedContract {
    pub address: String,
    pub network: String,
}

/// One provider of the registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provider {
    pub id: String,
    pub root: Option<String>,
    /// Lowercased addresses.
    pub managers: Vec<String>,
    /// A decimal integer, as the subgraph answers it.
    pub max_items: String,
    pub is_approved: bool,
    pub is_programmatic: bool,
    pub name: String,
    pub description: String,
    pub contracts: Vec<LinkedContract>,
}

impl Provider {
    /// Whether `address` is one of its managers, compared case-insensitively.
    pub fn managed_by(&self, address: &str) -> bool {
        self.managers
            .iter()
            .any(|m| m.eq_ignore_ascii_case(address))
    }
}

/// Whether `id` is a provider id: `urn:decentraland:<network>:collections-thirdparty:<name>`,
/// the network lowercase letters and digits and the name free of `:`, `|`
/// and whitespace.
pub fn is_provider_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split(':').collect();
    let [
        "urn",
        "decentraland",
        network,
        "collections-thirdparty",
        name,
    ] = parts.as_slice()
    else {
        return false;
    };
    !network.is_empty()
        && network
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && !name.is_empty()
        && !name
            .chars()
            .any(|c| c == '|' || c.is_whitespace() || c == '\u{FEFF}')
}

/// Every provider of the registry subgraph at `url`, sorted by id; `Err`
/// when the subgraph fails or an answer breaks the walk.
pub async fn registry(index: &ChainIndex, url: &str) -> Result<Vec<Provider>, String> {
    let mut providers: Vec<Provider> = Vec::new();
    for _ in 0..MAX_PAGES {
        let id_from = providers
            .iter()
            .map(|p| p.id.as_str())
            .max()
            .unwrap_or_default()
            .to_string();
        let data = graph_query(
            index,
            url,
            QUERY,
            json!({ "idFrom": id_from, "first": THE_GRAPH_PAGE_SIZE }),
        )
        .await?;
        let page = data
            .get("thirdParties")
            .and_then(Value::as_array)
            .ok_or("registry answered no thirdParties")?;
        let id_from_count = providers.len();
        for record in page {
            match (parse(record), record.get("id").and_then(Value::as_str)) {
                (Ok(provider), _) => providers.push(provider),
                (Err(error), Some(id)) => {
                    if first_report(id) {
                        tracing::warn!(%error, "third-party registry record unreadable; nobody manages it");
                    }
                    providers.push(nobody_manages(id));
                }
                (Err(error), None) => {
                    if first_report("") {
                        tracing::warn!(%error, "third-party registry record without an id left out");
                    }
                }
            }
        }
        if providers[id_from_count..].iter().any(|p| p.id <= id_from) {
            return Err("registry answered a provider before the page's start".into());
        }
        if page.len() < THE_GRAPH_PAGE_SIZE as usize {
            providers.sort_by(|a, b| a.id.cmp(&b.id));
            if providers.windows(2).any(|w| w[0].id == w[1].id) {
                return Err("registry answered a provider twice".into());
            }
            return Ok(providers);
        }
    }
    Err(format!("registry longer than {MAX_PAGES} pages"))
}

const RECEIPT: &str = "query receipt($id: ID!) { receipts(first: 1, where: { id: $id }) { id } }";

/// Whether the registry holds the receipt `id`, the digest of a slot cheque
/// it consumed (lowercase `0x` hex) at the registry subgraph `url`; `Err`
/// when the subgraph fails or its answer names another receipt.
pub async fn receipt_exists(index: &ChainIndex, url: &str, id: &str) -> Result<bool, String> {
    let data = graph_query(index, url, RECEIPT, json!({ "id": id })).await?;
    match data
        .get("receipts")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    {
        Some([]) => Ok(false),
        Some([receipt]) if receipt.get("id").and_then(Value::as_str) == Some(id) => Ok(true),
        _ => Err("registry answered an unreadable receipt".into()),
    }
}

fn first_report(id: &str) -> bool {
    static REPORTED: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);
    REPORTED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(id.to_string())
}

fn nobody_manages(id: &str) -> Provider {
    Provider {
        id: id.to_string(),
        root: None,
        managers: Vec::new(),
        max_items: "0".into(),
        is_approved: false,
        is_programmatic: false,
        name: String::new(),
        description: String::new(),
        contracts: Vec::new(),
    }
}

fn text(v: Option<&Value>) -> Result<String, String> {
    match v {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(s)) => Ok(s.clone()),
        Some(_) => Err("registry answered a text field that is not a string".into()),
    }
}

fn parse(record: &Value) -> Result<Provider, String> {
    let id = record
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| is_provider_id(id))
        .ok_or("registry answered a provider without a valid id")?;
    let root = match record.get("root") {
        None | Some(Value::Null) => None,
        Some(Value::String(root)) if root.is_empty() => None,
        Some(Value::String(root))
            if root.len() == 66
                && root.starts_with("0x")
                && root[2..].bytes().all(|b| b.is_ascii_hexdigit()) =>
        {
            Some(root.clone())
        }
        Some(_) => return Err(format!("registry answered an invalid root for {id}")),
    };
    let managers = record
        .get("managers")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("registry answered no managers for {id}"))?
        .iter()
        .map(|m| {
            m.as_str()
                .filter(|m| is_valid_eth_address(m))
                .map(str::to_ascii_lowercase)
                .ok_or_else(|| {
                    format!("registry answered a manager of {id} that is not an address")
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let max_items = record
        .get("maxItems")
        .and_then(Value::as_str)
        .filter(|n| !n.is_empty() && n.len() <= 78 && n.bytes().all(|b| b.is_ascii_digit()))
        .ok_or_else(|| format!("registry answered an invalid maxItems for {id}"))?
        .to_string();
    let is_approved = record
        .get("isApproved")
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("registry answered no isApproved for {id}"))?;
    let is_programmatic = match record.get("isProgrammatic") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => {
            return Err(format!(
                "registry answered an invalid isProgrammatic for {id}"
            ));
        }
    };
    let metadata = record
        .get("metadata")
        .and_then(|m| m.get("thirdParty"))
        .filter(|m| !m.is_null());
    let contracts = match metadata.and_then(|m| m.get("contracts")) {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(contracts)) => contracts
            .iter()
            .map(|c| {
                let address = c.get("address").and_then(Value::as_str);
                let network = c.get("network").and_then(Value::as_str);
                match (address, network) {
                    (Some(address), Some(network))
                        if is_valid_eth_address(address)
                            && !network.is_empty()
                            && network.len() <= 64
                            && network
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') =>
                    {
                        Ok(LinkedContract {
                            address: address.to_string(),
                            network: network.to_string(),
                        })
                    }
                    _ => Err(format!("registry answered an invalid contract for {id}")),
                }
            })
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => return Err(format!("registry answered invalid contracts for {id}")),
    };
    Ok(Provider {
        id: id.to_string(),
        root,
        managers,
        max_items,
        is_approved,
        is_programmatic,
        name: text(metadata.and_then(|m| m.get("name")))?,
        description: text(metadata.and_then(|m| m.get("description")))?,
        contracts,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use axum::Router;
    use axum::routing::post;

    use super::*;
    use crate::IndexSettings;
    use crate::http::Outbound;

    const P1: &str = "urn:decentraland:matic:collections-thirdparty:alpha";
    const P2: &str = "urn:decentraland:amoy:collections-thirdparty:beta";
    const M1: &str = "0x00000000000000000000000000000000000000a1";

    #[derive(Clone, Default)]
    struct Fake {
        records: Arc<Mutex<Option<Vec<Value>>>>,
        receipts: Arc<Mutex<Option<Vec<Value>>>>,
        bodies: Arc<Mutex<Vec<Value>>>,
        hits: Arc<AtomicUsize>,
    }

    async fn answer(
        axum::extract::State(fake): axum::extract::State<Fake>,
        axum::Json(body): axum::Json<Value>,
    ) -> axum::Json<Value> {
        fake.hits.fetch_add(1, Ordering::SeqCst);
        fake.bodies.lock().unwrap().push(body.clone());
        if body["query"]
            .as_str()
            .unwrap_or_default()
            .contains("receipts")
        {
            return axum::Json(match fake.receipts.lock().unwrap().clone() {
                Some(receipts) => json!({ "data": { "receipts": receipts } }),
                None => json!({ "errors": [{ "message": "indexing error" }] }),
            });
        }
        let Some(mut records) = fake.records.lock().unwrap().clone() else {
            return axum::Json(json!({ "errors": [{ "message": "indexing error" }] }));
        };
        records.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        let from = body["variables"]["idFrom"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let first = body["variables"]["first"].as_u64().unwrap_or(100) as usize;
        let page: Vec<Value> = records
            .into_iter()
            .filter(|r| r["id"].as_str().is_none_or(|id| id > from.as_str()))
            .take(first)
            .collect();
        axum::Json(json!({ "data": { "thirdParties": page } }))
    }

    async fn index(fake: &Fake) -> ChainIndex {
        let app = Router::new()
            .route("/registry", post(answer))
            .with_state(fake.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let index = ChainIndex::new(
            Outbound::for_tests(),
            IndexSettings {
                third_party_registry_subgraph_url: Some(format!("http://{addr}/registry")),
                ..IndexSettings::for_tests()
            },
        );
        index.allow_loopback_for_tests();
        index
    }

    fn url(index: &ChainIndex) -> &str {
        index
            .settings
            .third_party_registry_subgraph_url
            .as_deref()
            .unwrap()
    }

    async fn read(fake: &Fake) -> Result<Vec<Provider>, String> {
        let index = index(fake).await;
        registry(&index, url(&index)).await
    }

    fn record(id: &str) -> Value {
        json!({
            "id": id,
            "root": format!("0x{}", "ab".repeat(32)),
            "managers": [M1.to_uppercase().replace("0X", "0x")],
            "maxItems": "100",
            "isApproved": true,
            "isProgrammatic": false,
            "metadata": { "type": "third_party_v2", "thirdParty": {
                "name": "Alpha", "description": "d", "contracts": [{ "address": "0x00000000000000000000000000000000000000c0", "network": "amoy" }]
            } }
        })
    }

    fn set(fake: &Fake, records: Option<Vec<Value>>) {
        *fake.records.lock().unwrap() = records;
    }

    #[tokio::test]
    async fn a_receipt_is_found_by_its_id_and_an_unreadable_answer_fails() {
        let consumed = format!("0x{}", "1f".repeat(32));
        let other = format!("0x{}", "2e".repeat(32));
        let fake = Fake::default();
        let index = index(&fake).await;
        *fake.receipts.lock().unwrap() = Some(vec![json!({ "id": consumed })]);
        assert_eq!(
            receipt_exists(&index, url(&index), &consumed).await,
            Ok(true)
        );
        let body = fake.bodies.lock().unwrap().last().cloned().unwrap();
        assert_eq!(body["variables"], json!({ "id": consumed }));
        assert!(
            body["query"]
                .as_str()
                .unwrap()
                .contains("receipts(first: 1, where: { id: $id })")
        );
        *fake.receipts.lock().unwrap() = Some(vec![]);
        assert_eq!(receipt_exists(&index, url(&index), &other).await, Ok(false));
        for answer in [
            None,
            Some(vec![json!({ "id": other })]),
            Some(vec![json!({ "qty": "1" })]),
            Some(vec![json!({ "id": consumed }), json!({ "id": consumed })]),
        ] {
            *fake.receipts.lock().unwrap() = answer.clone();
            assert!(
                receipt_exists(&index, url(&index), &consumed)
                    .await
                    .is_err(),
                "{answer:?}"
            );
        }
    }

    #[tokio::test]
    async fn every_provider_is_read_with_its_fields_and_lowercased_managers() {
        let fake = Fake::default();
        let mut unapproved = record(P2);
        unapproved["isApproved"] = json!(false);
        unapproved["root"] = Value::Null;
        unapproved["isProgrammatic"] = json!(true);
        unapproved["metadata"] = Value::Null;
        set(&fake, Some(vec![record(P1), unapproved]));
        let index = index(&fake).await;
        let providers = registry(&index, url(&index)).await.unwrap();
        assert_eq!(
            providers,
            vec![
                Provider {
                    id: P2.into(),
                    root: None,
                    managers: vec![M1.into()],
                    max_items: "100".into(),
                    is_approved: false,
                    is_programmatic: true,
                    name: String::new(),
                    description: String::new(),
                    contracts: vec![],
                },
                Provider {
                    id: P1.into(),
                    root: Some(format!("0x{}", "ab".repeat(32))),
                    managers: vec![M1.into()],
                    max_items: "100".into(),
                    is_approved: true,
                    is_programmatic: false,
                    name: "Alpha".into(),
                    description: "d".into(),
                    contracts: vec![LinkedContract {
                        address: "0x00000000000000000000000000000000000000c0".into(),
                        network: "amoy".into(),
                    }],
                },
            ]
        );
        assert!(providers[0].managed_by(&M1.to_uppercase().replace("0X", "0x")));
        let bodies = fake.bodies.lock().unwrap().clone();
        assert_eq!(bodies.len(), 1);
        assert_eq!(
            bodies[0]["variables"],
            json!({ "idFrom": "", "first": 1000 })
        );
        assert!(!bodies[0]["query"].as_str().unwrap().contains(P1));
    }

    #[tokio::test]
    async fn the_walk_pages_by_id_and_stops_at_a_short_page() {
        let fake = Fake::default();
        let records: Vec<Value> = (0..1500)
            .map(|n| {
                record(&format!(
                    "urn:decentraland:matic:collections-thirdparty:p{n:05}"
                ))
            })
            .collect();
        set(&fake, Some(records));
        let index = index(&fake).await;
        let providers = registry(&index, url(&index)).await.unwrap();
        assert_eq!(providers.len(), 1500);
        assert_eq!(fake.hits.load(Ordering::SeqCst), 2);
        assert_eq!(
            fake.bodies.lock().unwrap()[1]["variables"]["idFrom"],
            json!("urn:decentraland:matic:collections-thirdparty:p00999")
        );
    }

    #[tokio::test]
    async fn a_registry_longer_than_the_page_bound_fails() {
        let fake = Fake::default();
        let records: Vec<Value> = (0..(MAX_PAGES * 1000))
            .map(|n| {
                record(&format!(
                    "urn:decentraland:matic:collections-thirdparty:p{n:06}"
                ))
            })
            .collect();
        set(&fake, Some(records));
        let index = index(&fake).await;
        let err = registry(&index, url(&index)).await.unwrap_err();
        assert!(err.contains("longer than"), "{err}");
        assert_eq!(fake.hits.load(Ordering::SeqCst), MAX_PAGES);
    }

    #[tokio::test]
    async fn a_failing_registry_fails_the_read() {
        let fake = Fake::default();
        set(&fake, None);
        let index = index(&fake).await;
        assert!(
            registry(&index, url(&index))
                .await
                .unwrap_err()
                .contains("errors")
        );
    }

    #[tokio::test]
    async fn the_registry_states_the_live_subgraph_answers_are_read() {
        let fake = Fake::default();
        let mut rootless = record(P1);
        rootless["root"] = json!("");
        rootless["isApproved"] = json!(false);
        rootless["metadata"]["type"] = json!("third_party_v1");
        let polygon = record("urn:decentraland:polygon:collections-thirdparty:gamma");
        set(&fake, Some(vec![rootless, polygon, record(P2)]));
        let providers = read(&fake).await.unwrap();
        let ids: Vec<&str> = providers.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                P2,
                P1,
                "urn:decentraland:polygon:collections-thirdparty:gamma"
            ]
        );
        assert_eq!(providers[1].root, None);
        assert_eq!(providers[1].managers, vec![M1.to_string()]);
        assert!(!providers[1].is_approved);
        assert_eq!(providers[2].managers, vec![M1.to_string()]);
    }

    fn nobody_manages(id: &str) -> Provider {
        Provider {
            id: id.into(),
            root: None,
            managers: vec![],
            max_items: "0".into(),
            is_approved: false,
            is_programmatic: false,
            name: String::new(),
            description: String::new(),
            contracts: vec![],
        }
    }

    #[tokio::test]
    async fn a_record_that_breaks_the_shape_stands_for_a_provider_nobody_manages() {
        let mut bad_contract = record(P1);
        bad_contract["metadata"]["thirdParty"]["contracts"] =
            json!([{ "address": "0x1", "network": "amoy" }]);
        let broken: Vec<(&str, Value)> = vec![
            ("root", json!("0x12")),
            ("root", json!(format!("0x{}", "zz".repeat(32)))),
            ("root", json!(7)),
            ("managers", json!(["not-an-address", M1])),
            ("managers", Value::Null),
            ("maxItems", json!("-1")),
            ("maxItems", json!(100)),
            ("isApproved", Value::Null),
            ("isProgrammatic", json!("yes")),
            ("metadata", bad_contract["metadata"].clone()),
        ];
        for (field, value) in broken {
            let fake = Fake::default();
            let mut bad = record(P1);
            bad[field] = value.clone();
            set(&fake, Some(vec![record(P2), bad]));
            let providers = read(&fake).await.unwrap();
            assert_eq!(providers.len(), 2, "{field} = {value}");
            assert_eq!(providers[0].id, P2, "{field} = {value}");
            assert_eq!(providers[0].managers, vec![M1.to_string()]);
            assert_eq!(providers[1], nobody_manages(P1), "{field} = {value}");
        }
        for id in [
            "urn:decentraland:matic:collections-thirdparty:al pha",
            "urn:decentraland:matic:collections-thirdparty:a:b",
            "urn:decentraland:matic:collections-thirdparty:a\u{FEFF}",
            "not a provider",
        ] {
            let fake = Fake::default();
            set(&fake, Some(vec![record(P2), record(id)]));
            let providers = read(&fake).await.unwrap();
            assert_eq!(providers.len(), 2, "{id:?}");
            assert!(providers.contains(&nobody_manages(id)), "{id:?}");
        }
    }

    #[tokio::test]
    async fn a_record_without_a_string_id_is_left_out() {
        for id in [json!(7), Value::Null, json!(["x"])] {
            let fake = Fake::default();
            let mut bad = record(P1);
            bad["id"] = id.clone();
            set(&fake, Some(vec![record(P2), bad]));
            let providers = read(&fake).await.unwrap();
            assert_eq!(providers.len(), 1, "{id}");
            assert_eq!(providers[0].id, P2);
        }
    }

    #[tokio::test]
    async fn an_answer_that_breaks_the_walk_fails_the_whole_read() {
        let fake = Fake::default();
        set(&fake, Some(vec![record(P1), record(P1)]));
        assert!(read(&fake).await.unwrap_err().contains("twice"));
    }

    #[test]
    fn an_unreadable_record_is_reported_once() {
        let id = "urn:decentraland:matic:collections-thirdparty:reported-once";
        assert!(first_report(id));
        assert!(!first_report(id));
        assert!(first_report(
            "urn:decentraland:matic:collections-thirdparty:another"
        ));
    }

    #[test]
    fn a_provider_id_has_builder_servers_shape() {
        for id in [
            P1,
            P2,
            "urn:decentraland:mainnet:collections-thirdparty:x",
            "urn:decentraland:polygon:collections-thirdparty:x",
        ] {
            assert!(is_provider_id(id), "{id}");
        }
        for id in [
            "",
            "urn:decentraland:matic:collections-thirdparty:",
            "urn:decentraland:matic:collections-thirdparty:a:b",
            "urn:decentraland::collections-thirdparty:a",
            "urn:decentraland:Matic:collections-thirdparty:a",
            "urn:decentraland:ma tic:collections-thirdparty:a",
            "urn:decentraland:matic:collections-v2:a",
            "URN:decentraland:matic:collections-thirdparty:a",
            "urn:decentraland:matic:collections-thirdparty:a|b",
            "urn:decentraland:matic:collections-thirdparty:a\u{FEFF}",
            "urn:decentraland:matic:collections-thirdparty:a\n",
        ] {
            assert!(!is_provider_id(id), "{id:?}");
        }
    }
}
