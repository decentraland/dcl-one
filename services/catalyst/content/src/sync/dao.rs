//! The DAO catalyst list: the L1 `Catalyst` registry contract read over
//! `ETH_RPC_URL` (`catalystCount()`, `catalystIds(i)`, `catalystById(id)`),
//! as the reference catalyst's `createDAOSource` reads it through
//! `@dcl/catalyst-contracts`. With `SYNC_FROM_DAO` on, the sync worker reads
//! it at start and every [`REFRESH`], drops this node's own entry and syncs
//! from every other catalyst's `/content`; when a read fails or names no
//! other peer, the list it had (at start, `SYNC_SOURCE`) stays. Registry
//! domains are third-party input: an entry that is not a plain public https
//! name ([`peer_base_url`]) is left out.
//! `/lambdas/contracts/servers` decodes the same records
//! (`catalyst-shop` `market::contracts`).

use std::collections::HashSet;
use std::future::Future;
use std::time::Duration;

use futures::{StreamExt, TryStreamExt};
use serde_json::json;

use crate::http::jsonrpc::{hex_of, parse_envelope, request, result_of};
use crate::http::{
    RetryDecision, is_private_or_internal_host, is_transient_status, read_body_capped,
    retry_with_backoff,
};

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

/// Above any real registry (a few dozen entries): a larger `catalystCount()`
/// is a bad RPC answer, refused before it turns into that many calls.
pub const MAX_CATALYSTS: usize = 1024;

const CALLS_IN_FLIGHT: usize = 16;
/// Tries per `eth_call` on a timeout, connect error, 5xx or 429.
const RPC_ATTEMPTS: u32 = 3;
const RPC_RETRY_BASE_MS: u64 = 500;
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

/// The base URL sync may fetch from, or `None` for an entry it must not
/// dial: only `https://` to a public DNS name on the default port, with no
/// userinfo, query or fragment. IP literals, `localhost`, single-label and
/// internal names are refused (`is_private_or_internal_host`). The host comes
/// back lowercased, the path without a trailing `/`.
pub fn peer_base_url(catalyst: &Catalyst) -> Option<String> {
    let raw = catalyst.base_url()?;
    let url = url::Url::parse(&raw).ok()?;
    let host = match url.host()? {
        url::Host::Domain(host) => host.to_string(),
        url::Host::Ipv4(_) | url::Host::Ipv6(_) => return None,
    };
    let plain = url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && url.query().is_none()
        && url.fragment().is_none();
    if !plain || is_private_or_internal_host(&host) {
        return None;
    }
    Some(format!(
        "https://{host}{}",
        url.path().trim_end_matches('/')
    ))
}

fn word_usize(word: &[u8]) -> Result<usize, String> {
    if word.len() != 32 || word[..24].iter().any(|b| *b != 0) {
        return Err(format!("not a small uint word: 0x{}", hex::encode(word)));
    }
    Ok(word[24..]
        .iter()
        .fold(0usize, |v, b| (v << 8) | usize::from(*b)))
}

/// A `catalystCount()` answer, at most [`MAX_CATALYSTS`].
pub fn decode_count(ret: &[u8]) -> Result<usize, String> {
    let count = ret
        .get(..32)
        .ok_or_else(|| format!("bad catalystCount return: 0x{}", hex::encode(ret)))
        .and_then(word_usize)?;
    if count > MAX_CATALYSTS {
        return Err(format!(
            "catalystCount {count} is above {MAX_CATALYSTS}: not a registry answer"
        ));
    }
    Ok(count)
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
    let start = offset.checked_add(32).ok_or_else(short)?;
    let len = word_usize(ret.get(offset..start).ok_or_else(short)?)?;
    let end = start.checked_add(len).ok_or_else(short)?;
    let bytes = ret.get(start..end).ok_or_else(short)?;
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

/// One `eth_call` at `latest` over the sync client, retried on a timeout,
/// connect error, 5xx or 429 (`retry_with_backoff`, as the shop's `eth_call`).
/// The error never names the endpoint: it commonly carries a gateway key.
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
    let answer = retry_with_backoff("dao-eth-call", RPC_ATTEMPTS, RPC_RETRY_BASE_MS, || {
        let body = &body;
        async move {
            let resp = match client
                .post(rpc_url)
                .timeout(RPC_TIMEOUT)
                .json(body)
                .send()
                .await
            {
                Ok(resp) => resp,
                Err(e) if e.is_connect() || e.is_timeout() => {
                    return Ok(RetryDecision::Retry {
                        after: None,
                        rate_limited: false,
                    });
                }
                Err(e) => return Err(anyhow::anyhow!("{}", e.without_url())),
            };
            let status = resp.status();
            if is_transient_status(status) {
                return Ok(RetryDecision::Retry {
                    after: None,
                    rate_limited: status == reqwest::StatusCode::TOO_MANY_REQUESTS,
                });
            }
            let bytes = read_body_capped(resp, RPC_BODY_CAP).await?;
            Ok(RetryDecision::Done((status.as_u16(), bytes)))
        }
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some((status, bytes)) = answer else {
        return Err("RPC unavailable after retries".to_string());
    };
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

/// Each listed server's `/content`, in registry order: only entries
/// [`peer_base_url`] accepts, without this node (compared case-insensitively
/// against `own_urls`, with or without `/content`, as the reference compares
/// its `CONTENT_SERVER_ADDRESS`; the same identities `SYNC_SOURCE` is checked
/// against) and without repeats.
pub fn content_peers(catalysts: &[Catalyst], own_urls: &[String]) -> Vec<String> {
    let own = super::own_identities(own_urls);
    let mut seen = HashSet::new();
    catalysts
        .iter()
        .filter_map(|catalyst| {
            let base = peer_base_url(catalyst);
            if base.is_none() {
                tracing::warn!(domain = %catalyst.domain, "Skipping a DAO catalyst entry that is not a public https name");
            }
            base
        })
        .filter(|base| {
            let key = base.to_ascii_lowercase();
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
        assert!(
            decode_count(&word(MAX_CATALYSTS + 1)).is_err(),
            "a huge count is refused"
        );
    }

    #[test]
    fn a_domain_offset_or_length_near_usize_max_is_an_error_not_a_panic() {
        let mut bad_offset = record(0xab, 0x11, "peer.example.org");
        bad_offset[64..96].copy_from_slice(&[0u8; 32]);
        bad_offset[88..96].copy_from_slice(&u64::MAX.to_be_bytes());
        assert!(decode_catalyst("ab", &bad_offset).is_err());
        let mut bad_len = record(0xab, 0x11, "peer.example.org");
        bad_len[5 * 32 + 24..6 * 32].copy_from_slice(&u64::MAX.to_be_bytes());
        assert!(decode_catalyst("ab", &bad_len).is_err());
    }

    #[test]
    fn only_public_https_names_are_dialled() {
        for domain in [
            "peer.example.org",
            "https://peer.example.org/",
            "https://Peer.Example.org:443",
        ] {
            assert_eq!(
                peer_base_url(&catalyst(domain)).as_deref(),
                Some("https://peer.example.org"),
                "{domain}"
            );
        }
        for domain in [
            "http://peer.example.org",
            "10.0.0.5",
            "https://127.0.0.1",
            "https://[::1]",
            "localhost",
            "https://metadata",
            "peer.internal",
            "https://user:pass@peer.example.org",
            "https://peer.example.org:8443",
            "https://peer.example.org/?x=1",
            "not a url",
        ] {
            assert_eq!(peer_base_url(&catalyst(domain)), None, "{domain}");
        }
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
            catalyst("192.168.1.10"),
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
