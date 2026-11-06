//! The DAO catalyst list: the L1 `Catalyst` registry contract read over
//! `ETH_RPC_URL` (`catalystCount()`, `catalystIds(i)`, `catalystById(id)`),
//! as the reference catalyst's `createDAOSource` reads it through
//! `@dcl/catalyst-contracts`. With `SYNC_FROM_DAO` on, the sync worker reads
//! it at start and every [`REFRESH`], drops this node's own entry and syncs
//! from every other catalyst's `/content`; when a read fails or names no
//! other peer, the list it had (at start, `SYNC_SOURCE`) stays.
//! `/lambdas/contracts/servers` decodes the same records
//! (`catalyst-shop` `market::contracts`).

use std::collections::HashSet;
use std::future::Future;
use std::time::Duration;

use futures::{StreamExt, TryStreamExt};
use serde_json::json;

use crate::http::jsonrpc::{hex_of, parse_envelope, request, result_of};
use crate::http::read_body_capped;

/// The registry proxy per `ETH_NETWORK`, the addresses `/lambdas/contracts/servers`
/// reads (Decentraland's `addresses.json` `CatalystProxy` on mainnet); another
/// network has no registry.
pub fn registry_address(eth_network: &str) -> Option<&'static str> {
    match eth_network.trim() {
        "mainnet" => Some(MAINNET_REGISTRY),
        "sepolia" => Some(SEPOLIA_REGISTRY),
        _ => None,
    }
}

pub const MAINNET_REGISTRY: &str = "0x4a2f10076101650f40342885b99b6b101d83c486";
pub const SEPOLIA_REGISTRY: &str = "0x9b5091588a4bae0a5ea54a35af3c31f57a68ed37";

/// `catalystCount()`.
pub const SEL_CATALYST_COUNT: &str = "18becc10";
/// `catalystIds(uint256)`.
pub const SEL_CATALYST_IDS: &str = "7b9b4f2c";
/// `catalystById(bytes32)`.
pub const SEL_CATALYST_BY_ID: &str = "c9038ce9";

/// How often the list is read again (the reference's `UPDATE_FROM_DAO_INTERVAL` default).
pub const REFRESH: Duration = Duration::from_secs(30 * 60);

const CALLS_IN_FLIGHT: usize = 16;
const RPC_TIMEOUT: Duration = Duration::from_secs(30);
const RPC_BODY_CAP: usize = 1024 * 1024;

/// One registry entry: the 32-byte id (lowercase hex, no `0x`), the owner and
/// the domain as registered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalyst {
    pub id: String,
    pub owner: [u8; 20],
    pub domain: String,
}

impl Catalyst {
    /// The server's base URL as the reference lists it: an `http://` domain is
    /// dropped, a bare one gets `https://`.
    pub fn base_url(&self) -> Option<String> {
        let domain = self.domain.trim();
        if domain.starts_with("http://") {
            return None;
        }
        Some(if domain.starts_with("https://") {
            domain.to_string()
        } else {
            format!("https://{domain}")
        })
    }
}

fn word_usize(word: &[u8]) -> Result<usize, String> {
    if word.len() != 32 || word[..24].iter().any(|b| *b != 0) {
        return Err(format!("not a small uint word: 0x{}", hex::encode(word)));
    }
    Ok(word[24..]
        .iter()
        .fold(0usize, |v, b| (v << 8) | usize::from(*b)))
}

/// A `catalystCount()` answer.
pub fn decode_count(ret: &[u8]) -> Result<usize, String> {
    ret.get(..32)
        .ok_or_else(|| format!("bad catalystCount return: 0x{}", hex::encode(ret)))
        .and_then(word_usize)
}

/// A `catalystById(id)` answer: `(bytes32 id, address owner, string domain, ...)`;
/// only the first three heads are read.
pub fn decode_catalyst(id: &str, ret: &[u8]) -> Result<Catalyst, String> {
    let short = || format!("catalystById return too short: 0x{}", hex::encode(ret));
    if ret.len() < 96 {
        return Err(short());
    }
    let mut owner = [0u8; 20];
    owner.copy_from_slice(&ret[44..64]);
    let offset = word_usize(&ret[64..96])?;
    let len = word_usize(ret.get(offset..offset + 32).ok_or_else(short)?)?;
    let bytes = ret.get(offset + 32..offset + 32 + len).ok_or_else(short)?;
    let domain =
        String::from_utf8(bytes.to_vec()).map_err(|e| format!("domain is not utf8: {e}"))?;
    Ok(Catalyst {
        id: id.to_string(),
        owner,
        domain,
    })
}

/// Every registry entry, in registry order, through `call` (calldata hex
/// without `0x` -> return bytes); any failed call fails the read.
pub async fn read_registry<F, Fut>(call: F) -> Result<Vec<Catalyst>, String>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, String>>,
{
    let count = decode_count(&call(SEL_CATALYST_COUNT.to_string()).await?)?;
    let ids: Vec<String> = futures::stream::iter(0..count)
        .map(|i| {
            let ret = call(format!("{SEL_CATALYST_IDS}{i:064x}"));
            async move {
                let word = ret.await?;
                word.get(..32)
                    .map(hex::encode)
                    .ok_or_else(|| format!("bad catalystIds return: 0x{}", hex::encode(&word)))
            }
        })
        .buffered(CALLS_IN_FLIGHT)
        .try_collect()
        .await?;
    futures::stream::iter(ids)
        .map(|id| {
            let ret = call(format!("{SEL_CATALYST_BY_ID}{id}"));
            async move { decode_catalyst(&id, &ret.await?) }
        })
        .buffered(CALLS_IN_FLIGHT)
        .try_collect()
        .await
}

/// One `eth_call` at `latest` over the sync client. The error never names the
/// endpoint: it commonly carries a gateway key.
async fn eth_call(
    client: &reqwest::Client,
    rpc_url: &str,
    to: &str,
    data_hex: &str,
) -> Result<Vec<u8>, String> {
    let body = request(
        1,
        "eth_call",
        json!([{ "to": to, "data": format!("0x{data_hex}") }, "latest"]),
    );
    let resp = client
        .post(rpc_url)
        .timeout(RPC_TIMEOUT)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.without_url().to_string())?;
    let status = resp.status().as_u16();
    let bytes = read_body_capped(resp, RPC_BODY_CAP)
        .await
        .map_err(|e| e.to_string())?;
    let hex = parse_envelope(status, &bytes)
        .and_then(result_of)
        .and_then(hex_of)
        .map_err(|e| e.to_string())?;
    hex::decode(hex.trim_start_matches("0x")).map_err(|e| format!("bad eth_call hex: {e}"))
}

/// Where the list comes from and what this node is called: built by
/// `SyncSettings::from_config` with `SYNC_FROM_DAO` on.
#[derive(Debug, Clone)]
pub struct DaoSource {
    pub rpc_url: String,
    pub registry: &'static str,
    /// `PUBLIC_URL`, `CONTENT_URL` and the bind address: an entry naming one is this node.
    pub own_urls: Vec<String>,
}

impl DaoSource {
    /// The other catalysts' content URLs, from the registry.
    pub async fn peers(&self, client: &reqwest::Client) -> Result<Vec<String>, String> {
        let (rpc, registry) = (self.rpc_url.as_str(), self.registry);
        let catalysts =
            read_registry(
                |data: String| async move { eth_call(client, rpc, registry, &data).await },
            )
            .await?;
        let peers = content_peers(&catalysts, &self.own_urls);
        if peers.is_empty() {
            return Err(format!(
                "the registry lists {} catalysts and none but this node",
                catalysts.len()
            ));
        }
        Ok(peers)
    }
}

fn normalized(url: &str) -> String {
    url.trim().trim_end_matches('/').to_ascii_lowercase()
}

/// Each listed server's `/content`, in registry order, without this node
/// (compared case-insensitively against `own_urls`, with or without
/// `/content`, as the reference compares its `CONTENT_SERVER_ADDRESS`) and
/// without repeats.
pub fn content_peers(catalysts: &[Catalyst], own_urls: &[String]) -> Vec<String> {
    let mut own = HashSet::new();
    for url in own_urls.iter().map(|u| normalized(u)) {
        if url.is_empty() {
            continue;
        }
        own.insert(url.trim_end_matches("/content").to_string());
        own.insert(url);
    }
    let mut seen = HashSet::new();
    catalysts
        .iter()
        .filter_map(Catalyst::base_url)
        .map(|base| base.trim_end_matches('/').to_string())
        .filter(|base| {
            let key = normalized(base);
            !own.contains(&key) && seen.insert(key)
        })
        .map(|base| format!("{base}/content"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(n: usize) -> Vec<u8> {
        let mut w = vec![0u8; 32];
        w[24..].copy_from_slice(&(n as u64).to_be_bytes());
        w
    }

    /// A `catalystById` return as the contract ABI-encodes it: id, owner,
    /// the domain's offset, start and end times, then the string.
    fn record(id: u8, owner: u8, domain: &str) -> Vec<u8> {
        let mut out = vec![id; 32];
        out.extend(vec![0u8; 12]);
        out.extend(vec![owner; 20]);
        out.extend(word(5 * 32));
        out.extend(word(1));
        out.extend(word(0));
        out.extend(word(domain.len()));
        let mut data = domain.as_bytes().to_vec();
        data.resize(domain.len().div_ceil(32) * 32, 0);
        out.extend(data);
        out
    }

    fn catalyst(domain: &str) -> Catalyst {
        Catalyst {
            id: "00".repeat(32),
            owner: [0; 20],
            domain: domain.to_string(),
        }
    }

    #[test]
    fn the_selectors_are_the_registry_functions() {
        for (sel, sig) in [
            (SEL_CATALYST_COUNT, "catalystCount()"),
            (SEL_CATALYST_IDS, "catalystIds(uint256)"),
            (SEL_CATALYST_BY_ID, "catalystById(bytes32)"),
        ] {
            let hash = alloy_primitives::keccak256(sig.as_bytes());
            assert_eq!(sel, hex::encode(&hash[..4]), "{sig}");
        }
    }

    #[test]
    fn a_catalyst_record_decodes() {
        let got = decode_catalyst("ab", &record(0xab, 0x11, "peer.example.org")).unwrap();
        assert_eq!(got.owner, [0x11; 20]);
        assert_eq!(got.domain, "peer.example.org");
        assert_eq!(got.base_url().as_deref(), Some("https://peer.example.org"));
        assert!(decode_catalyst("ab", &[0u8; 40]).is_err());
        let mut truncated = record(0xab, 0x11, "peer.example.org");
        truncated.truncate(5 * 32 + 40);
        assert!(decode_catalyst("ab", &truncated).is_err());
        assert_eq!(decode_count(&word(7)).unwrap(), 7);
        assert!(decode_count(&[0u8; 8]).is_err());
    }

    #[tokio::test]
    async fn the_registry_is_read_count_then_ids_then_records() {
        let domains = ["peer-a.example.org", "https://peer-b.example.org/"];
        let call = |data: String| {
            let out = if data == SEL_CATALYST_COUNT {
                Ok(word(domains.len()))
            } else if let Some(i) = data.strip_prefix(SEL_CATALYST_IDS) {
                let i = usize::from_str_radix(i, 16).unwrap();
                Ok(vec![i as u8 + 1; 32])
            } else if let Some(id) = data.strip_prefix(SEL_CATALYST_BY_ID) {
                let i = hex::decode(id).unwrap()[0] as usize - 1;
                Ok(record(i as u8 + 1, 0x22, domains[i]))
            } else {
                Err(format!("unexpected call {data}"))
            };
            async move { out }
        };
        let got = read_registry(call).await.unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].id, "01".repeat(32));
        assert_eq!(got[1].domain, "https://peer-b.example.org/");
        assert_eq!(
            content_peers(&got, &[]),
            [
                "https://peer-a.example.org/content",
                "https://peer-b.example.org/content"
            ]
        );
        let failing = |_: String| async { Err::<Vec<u8>, _>("rpc down".to_string()) };
        assert_eq!(read_registry(failing).await.unwrap_err(), "rpc down");
    }

    #[test]
    fn this_node_http_entries_and_repeats_are_left_out() {
        let list = [
            catalyst("peer.example.org"),
            catalyst("http://plain.example.org"),
            catalyst("https://Me.Example.org"),
            catalyst("peer.example.org/"),
            catalyst("other.example.org"),
        ];
        let own = ["https://me.example.org/content/".to_string()];
        assert_eq!(
            content_peers(&list, &own),
            [
                "https://peer.example.org/content",
                "https://other.example.org/content"
            ]
        );
        let own = ["https://ME.example.org".to_string()];
        assert!(
            !content_peers(&list, &own)
                .iter()
                .any(|p| p.to_ascii_lowercase().contains("me.example.org"))
        );
    }
}
