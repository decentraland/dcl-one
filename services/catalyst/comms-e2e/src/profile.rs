//! A minimal profile for a test wallet, deployed to a realm's content server the way the explorer
//! deploys its own (`deploy_entity` and `send_deployment` in the explorer's profile module): an
//! entity file with no files attached, named by its CIDv1, signed by the wallet and sent as a
//! multipart form to `POST {content server}/entities`.
//!
//! A client draws another player only once it has that player's profile, which it reads from a
//! catalyst's lambdas. A test wallet that never deployed one is drawn by no such client, so a
//! visiting bot deploys one first and announces its version in its Pulse handshake.

use std::time::Duration;

use anyhow::{Context as _, Result, anyhow, bail};
use dcl_auth::{AuthLink, AuthLinkType};
use serde::Serialize;
use sha2::{Digest, Sha256};
use url::Url;

use crate::engine::timeline::redact_text;
use crate::identity::Identity;

/// The version of every profile this module deploys; a handshake of its wallet announces it.
pub const PROFILE_VERSION: i32 = 1;

/// The longest `Retry-After` a refused deployment waits for before it is sent once more, as the
/// explorer does; a longer one is not waited for.
pub const MOST_RETRY_AFTER: Duration = Duration::from_secs(60);

/// How long one request may take.
const TIMEOUT: Duration = Duration::from_secs(60);

/// The base avatars, the only wearables every catalog has: a body shape and what it wears.
const LOOKS: [(&str, &[&str]); 2] = [
    (
        "urn:decentraland:off-chain:base-avatars:BaseFemale",
        &[
            "urn:decentraland:off-chain:base-avatars:f_sweater",
            "urn:decentraland:off-chain:base-avatars:f_jeans",
            "urn:decentraland:off-chain:base-avatars:bun_shoes",
            "urn:decentraland:off-chain:base-avatars:standard_hair",
            "urn:decentraland:off-chain:base-avatars:f_eyes_01",
            "urn:decentraland:off-chain:base-avatars:f_eyebrows_00",
            "urn:decentraland:off-chain:base-avatars:f_mouth_00",
        ],
    ),
    (
        "urn:decentraland:off-chain:base-avatars:BaseMale",
        &[
            "urn:decentraland:off-chain:base-avatars:simple_blue_tshirt",
            "urn:decentraland:off-chain:base-avatars:distressed_black_Jeans",
            "urn:decentraland:off-chain:base-avatars:citycomfortableshoes",
            "urn:decentraland:off-chain:base-avatars:standard_hair",
            "urn:decentraland:off-chain:base-avatars:eyes_00",
            "urn:decentraland:off-chain:base-avatars:eyebrows_00",
            "urn:decentraland:off-chain:base-avatars:mouth_00",
        ],
    ),
];

/// The base emotes, by their short names, in the ten slots: owned by everyone.
const EMOTES: [&str; 10] = [
    "handsair",
    "wave",
    "fistpump",
    "dance",
    "raisehand",
    "clap",
    "money",
    "kiss",
    "headexplode",
    "shrug",
];

const SKIN: [[f32; 3]; 5] = [
    [0.95, 0.79, 0.66],
    [0.80, 0.60, 0.46],
    [0.60, 0.42, 0.30],
    [0.49, 0.36, 0.28],
    [0.36, 0.24, 0.17],
];
const HAIR: [[f32; 3]; 5] = [
    [0.10, 0.08, 0.06],
    [0.60, 0.37, 0.22],
    [0.95, 0.80, 0.40],
    [0.55, 0.15, 0.10],
    [0.40, 0.40, 0.45],
];
const EYES: [[f32; 3]; 4] = [
    [0.30, 0.22, 0.99],
    [0.20, 0.50, 0.30],
    [0.35, 0.20, 0.10],
    [0.10, 0.10, 0.10],
];

/// The profile name of visiting player `index`.
pub fn visit_name(index: usize) -> String {
    format!("ce2e-visit-{index:02}")
}

fn colour(rgb: [f32; 3]) -> serde_json::Value {
    serde_json::json!({ "color": { "r": rgb[0], "g": rgb[1], "b": rgb[2], "a": 1.0 } })
}

/// The avatar of visiting player `index` with wallet `address`: a base avatar, its body shape and
/// colours varied by index, no claimed name and no snapshots.
pub fn visit_avatar(address: &str, index: usize) -> serde_json::Value {
    let (body_shape, wearables) = LOOKS[index % LOOKS.len()];
    let emotes: Vec<serde_json::Value> = EMOTES
        .iter()
        .enumerate()
        .map(|(slot, urn)| serde_json::json!({ "slot": slot, "urn": urn }))
        .collect();
    serde_json::json!({
        "userId": address,
        "ethAddress": address,
        "name": visit_name(index),
        "description": "",
        "hasClaimedName": false,
        "hasConnectedWeb3": false,
        "tutorialStep": 0,
        "version": PROFILE_VERSION,
        "avatar": {
            "bodyShape": body_shape,
            "wearables": wearables,
            "emotes": emotes,
            "skin": colour(SKIN[index % SKIN.len()]),
            "hair": colour(HAIR[(index / 2) % HAIR.len()]),
            "eyes": colour(EYES[(index / 3) % EYES.len()]),
        },
    })
}

/// The CIDv1 of `bytes` as the explorer names an entity: raw codec (0x55), SHA2-256, in
/// lowercase base32 with its multibase prefix `b`.
pub fn cid_v1_raw(bytes: &[u8]) -> String {
    let mut cid = vec![0x01, 0x55, 0x12, 0x20];
    cid.extend_from_slice(&Sha256::digest(bytes));
    format!("b{}", base32_lower(&cid))
}

/// RFC 4648 base32 in lowercase, without padding.
fn base32_lower(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut text = String::with_capacity(bytes.len() * 8 / 5 + 1);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for byte in bytes {
        buffer = ((buffer << 8) | u32::from(*byte)) & 0x1fff;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            text.push(ALPHABET[((buffer >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        text.push(ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    text
}

/// The entity file, its fields in the order the explorer writes them.
#[derive(Serialize)]
struct EntityFile<'a> {
    version: &'a str,
    #[serde(rename = "type")]
    entity_type: &'a str,
    pointers: Vec<String>,
    timestamp: i64,
    metadata: serde_json::Value,
}

/// One signed profile deployment, ready to send.
#[derive(Debug, Clone)]
pub struct Deployment {
    /// The wallet, the entity's one pointer.
    pub address: String,
    /// The CIDv1 of [`Deployment::entity`].
    pub entity_id: String,
    /// The entity file: the exact bytes sent.
    pub entity: Vec<u8>,
    /// The wallet's chain, its last link signing the entity id.
    pub auth_chain: Vec<AuthLink>,
}

/// The deployment of visiting player `index`'s profile for `identity`, stamped `timestamp_ms`.
pub fn profile_deployment(identity: &Identity, index: usize, timestamp_ms: i64) -> Deployment {
    let address = identity.address();
    let entity = serde_json::to_vec(&EntityFile {
        version: "v3",
        entity_type: "profile",
        pointers: vec![address.clone()],
        timestamp: timestamp_ms,
        metadata: serde_json::json!({ "avatars": [visit_avatar(&address, index)] }),
    })
    .expect("an entity file serializes");
    let entity_id = cid_v1_raw(&entity);
    let auth_chain = identity.auth_chain(&entity_id);
    Deployment {
        address,
        entity_id,
        entity,
        auth_chain,
    }
}

impl Deployment {
    /// The form's text fields, in order: `entityId`, then `authChain[i][type]`, `[payload]` and
    /// `[signature]` for each link (empty for the signer, which signs nothing).
    pub fn fields(&self) -> Vec<(String, String)> {
        let mut fields = vec![("entityId".to_owned(), self.entity_id.clone())];
        for (index, link) in self.auth_chain.iter().enumerate() {
            let link_type = match serde_json::to_value(link.link_type) {
                Ok(serde_json::Value::String(name)) => name,
                _ => unreachable!("a link type serializes as its name"),
            };
            fields.push((format!("authChain[{index}][type]"), link_type));
            fields.push((format!("authChain[{index}][payload]"), link.payload.clone()));
            fields.push((
                format!("authChain[{index}][signature]"),
                link.signature.clone().unwrap_or_default(),
            ));
        }
        fields
    }

    /// The boundary of this deployment's form: from the entity id, which no part contains.
    pub fn boundary(&self) -> String {
        format!(
            "ce2e-{}",
            &hex::encode(Sha256::digest(self.entity_id.as_bytes()))[..32]
        )
    }

    /// The `multipart/form-data` body: the text fields, then the entity file as a part named by
    /// the entity id, `application/octet-stream`, as the explorer's form has it.
    pub fn form(&self) -> Vec<u8> {
        let boundary = self.boundary();
        let mut body = Vec::new();
        for (name, value) in self.fields() {
            body.extend_from_slice(
                format!(
                    "\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}"
                )
                .as_bytes(),
            );
        }
        body.extend_from_slice(
            format!(
                "\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"{}\"\r\nContent-Type: application/octet-stream\r\n\r\n",
                self.entity_id
            )
            .as_bytes(),
        );
        body.extend_from_slice(&self.entity);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        body
    }

    /// Whether the last link of the chain signs this deployment's entity id.
    pub fn signs_its_id(&self) -> bool {
        self.auth_chain.last().is_some_and(|link| {
            link.link_type == AuthLinkType::EcdsaSignedEntity && link.payload == self.entity_id
        })
    }
}

/// Where a realm keeps profiles: its content server's deployment route and its lambdas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    /// `{content server}/entities`.
    pub entities: Url,
    /// The lambdas, without a trailing slash: profiles are read at `{lambdas}/profiles/{address}`.
    pub lambdas: Url,
}

impl Endpoints {
    /// The hosts a deployment reaches, an IPv6 address without its brackets.
    pub fn hosts(&self) -> Vec<String> {
        let mut hosts: Vec<String> = [&self.entities, &self.lambdas]
            .iter()
            .map(|url| crate::stack::host_of(url))
            .collect();
        hosts.dedup();
        hosts
    }
}

impl Endpoints {
    /// These endpoints with every address that names `front`, the explorer's front for the API,
    /// dialed at `real`'s host and port instead: a stack that advertises the front relays it
    /// only while an explorer runs. `None` when neither names it.
    pub fn past_front(&self, front: std::net::SocketAddr, real: &Url) -> Option<Endpoints> {
        let names_front = |url: &Url| {
            url.socket_addrs(|| None)
                .is_ok_and(|addresses| addresses.contains(&front))
        };
        if !names_front(&self.entities) && !names_front(&self.lambdas) {
            return None;
        }
        let swap = |url: &Url| {
            let mut swapped = url.clone();
            if names_front(url) {
                let _ = swapped.set_host(real.host_str());
                let _ = swapped.set_port(real.port_or_known_default());
            }
            swapped
        };
        Some(Endpoints {
            entities: swap(&self.entities),
            lambdas: swap(&self.lambdas),
        })
    }
}

/// A client for the content server and the lambdas.
pub fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("no HTTP client")
}

fn no_answer(what: &str, error: &reqwest::Error) -> anyhow::Error {
    anyhow!("{what}: no answer: {}", redact_text(&error.to_string()))
}

/// The public URL a realm description gives under `key` (`content` or `lambdas`), resolved
/// against the description's own address when it is relative.
fn public_url(about: &Url, body: &serde_json::Value, key: &str) -> Result<Url> {
    let text = body
        .pointer(&format!("/{key}/publicUrl"))
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| anyhow!("the realm description names no {key}.publicUrl"))?;
    let url = about
        .join(text.trim())
        .with_context(|| format!("the realm's {key}.publicUrl {text:?} is not a URL"))?;
    if !matches!(url.scheme(), "http" | "https") {
        bail!("the realm's {key}.publicUrl {text:?} is not an HTTP address");
    }
    Ok(url)
}

/// `url` with `path` appended to its path, whatever slash it ends with.
fn under(url: &Url, path: &str) -> Url {
    let mut joined = url.clone();
    let base = url.path().trim_end_matches('/');
    joined.set_path(&format!("{base}/{path}"));
    joined
}

/// The realm's content server and lambdas, from its description at `{api}/about`.
pub async fn endpoints(client: &reqwest::Client, api: &Url) -> Result<Endpoints> {
    let about = api
        .join("about")
        .context("the realm description's address")?;
    let response = client
        .get(about.clone())
        .send()
        .await
        .map_err(|error| no_answer("the realm description", &error))?;
    let status = response.status();
    if !status.is_success() {
        bail!("the realm description answered HTTP {status}");
    }
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|error| anyhow!("the realm description is not JSON: {error}"))?;
    let content = public_url(&about, &body, "content")?;
    let mut lambdas = public_url(&about, &body, "lambdas")?;
    let trimmed = lambdas.path().trim_end_matches('/').to_owned();
    lambdas.set_path(&trimmed);
    Ok(Endpoints {
        entities: under(&content, "entities"),
        lambdas,
    })
}

/// The version of the profile the lambdas serve for `address`, `None` when they serve none.
pub async fn served_version(
    client: &reqwest::Client,
    lambdas: &Url,
    address: &str,
) -> Result<Option<i64>> {
    Ok(served_profile(client, lambdas, address)
        .await?
        .and_then(|body| {
            body.pointer("/avatars/0/version").and_then(|version| {
                version
                    .as_i64()
                    .or_else(|| version.as_f64().map(crate::casts::trunc_i64))
            })
        }))
}

/// What the lambdas serve for `address` (`{ timestamp, avatars }`), `None` when they serve none.
pub async fn served_profile(
    client: &reqwest::Client,
    lambdas: &Url,
    address: &str,
) -> Result<Option<serde_json::Value>> {
    let url = under(lambdas, &format!("profiles/{address}"));
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| no_answer("the lambdas", &error))?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !status.is_success() {
        bail!("the lambdas answered HTTP {status} for the profile of {address}");
    }
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|error| anyhow!("the lambdas' profile of {address} is not JSON: {error}"))?;
    Ok(Some(body))
}

/// Whether `served`, what the lambdas serve for visiting player `index`'s wallet `address`, is
/// the profile this module deploys for it: one avatar, equal to [`visit_avatar`] in every field
/// but those the lambdas set themselves (`userId` and `ethAddress`, the pointer, and the
/// avatar's `snapshots`), with no field it does not set. Numbers compare by value ([`same`]), so
/// a server that writes `1.0` as `1` serves the same profile.
pub fn serves_visit_profile(served: &serde_json::Value, address: &str, index: usize) -> bool {
    let without_what_the_lambdas_set = |avatar: &serde_json::Value| {
        let mut avatar = avatar.clone();
        if let Some(fields) = avatar.as_object_mut() {
            fields.remove("userId");
            fields.remove("ethAddress");
        }
        if let Some(looks) = avatar
            .get_mut("avatar")
            .and_then(serde_json::Value::as_object_mut)
        {
            looks.remove("snapshots");
        }
        avatar
    };
    match served.get("avatars").and_then(serde_json::Value::as_array) {
        Some(avatars) if avatars.len() == 1 => same(
            &without_what_the_lambdas_set(&avatars[0]),
            &without_what_the_lambdas_set(&visit_avatar(address, index)),
        ),
        _ => false,
    }
}

/// Whether `a` and `b` are the same JSON: numbers by value (`1` and `1.0` alike), everything else
/// exactly, objects with the same keys.
pub fn same(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    use serde_json::Value;
    match (a, b) {
        // two integers exactly, anything else by its value as a float
        (Value::Number(a), Value::Number(b)) => {
            match (a.as_i64(), b.as_i64(), a.as_u64(), b.as_u64()) {
                (Some(a), Some(b), _, _) => a == b,
                (_, _, Some(a), Some(b)) => a == b,
                _ => a.as_f64() == b.as_f64(),
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(key, a)| b.get(key).is_some_and(|b| same(a, b)))
        }
        _ => a == b,
    }
}

/// The delay a `Retry-After` value asks for, at `now`: whole seconds, or an HTTP date.
pub fn retry_after(value: &str, now: chrono::DateTime<chrono::Utc>) -> Option<Duration> {
    let value = value.trim();
    if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Some(Duration::from_secs(value.parse().unwrap_or(u64::MAX)));
    }
    let date = chrono::DateTime::parse_from_rfc2822(value).ok()?;
    Some(
        (date.with_timezone(&chrono::Utc) - now)
            .to_std()
            .unwrap_or_default(),
    )
}

/// How a deployment went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deployed {
    /// Accepted at once.
    Accepted,
    /// Accepted when sent again, after a refusal that said when to come back.
    AcceptedOnRetry,
}

async fn post(
    client: &reqwest::Client,
    entities: &Url,
    deployment: &Deployment,
) -> Result<reqwest::Response> {
    client
        .post(entities.clone())
        .header(
            reqwest::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", deployment.boundary()),
        )
        .body(deployment.form())
        .send()
        .await
        .map_err(|error| no_answer("the content server", &error))
}

/// Sends `deployment` to `entities`. A 429 or 503 whose `Retry-After` asks for at most
/// [`MOST_RETRY_AFTER`] is sent once more after that delay; any answer but 200 after that is an
/// error naming its status and what the server said.
pub async fn deploy(
    client: &reqwest::Client,
    entities: &Url,
    deployment: &Deployment,
) -> Result<Deployed> {
    let mut response = post(client, entities, deployment).await?;
    let mut deployed = Deployed::Accepted;
    let refused_for_now = matches!(
        response.status(),
        reqwest::StatusCode::TOO_MANY_REQUESTS | reqwest::StatusCode::SERVICE_UNAVAILABLE
    );
    let delay = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| retry_after(value, chrono::Utc::now()))
        .filter(|delay| refused_for_now && *delay <= MOST_RETRY_AFTER);
    if let Some(delay) = delay {
        tokio::time::sleep(delay).await;
        response = post(client, entities, deployment).await?;
        deployed = Deployed::AcceptedOnRetry;
    }
    let status = response.status();
    if status == reqwest::StatusCode::OK {
        return Ok(deployed);
    }
    let said = response.text().await.unwrap_or_default();
    let said: String = said.chars().take(400).collect();
    bail!(
        "the content server refused the profile of {}{}: HTTP {status}: {said}",
        deployment.address,
        if deployed == Deployed::AcceptedOnRetry {
            " when it was sent again"
        } else {
            ""
        }
    )
}

/// What [`ensure`] did for one wallet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ensured {
    /// The lambdas already served this profile: nothing was deployed.
    AlreadyServed,
    /// A profile was deployed where the lambdas served none.
    Deployed(Deployed),
    /// The lambdas served another profile for the wallet: this one was deployed over it.
    Replaced(Deployed),
}

/// Makes sure the realm serves visiting player `index`'s profile for `identity`: asks the lambdas
/// first, and deploys it unless they serve exactly that profile ([`serves_visit_profile`]). The
/// content server keeps the newer of two profiles, so a deployment over another one is stamped
/// after it: now, or just after the profile served when that is later.
pub async fn ensure(
    client: &reqwest::Client,
    endpoints: &Endpoints,
    identity: &Identity,
    index: usize,
) -> Result<Ensured> {
    let address = identity.address();
    let served = served_profile(client, &endpoints.lambdas, &address).await?;
    if served
        .as_ref()
        .is_some_and(|served| serves_visit_profile(served, &address, index))
    {
        return Ok(Ensured::AlreadyServed);
    }
    let served_at = served
        .as_ref()
        .and_then(|served| served.get("timestamp"))
        .and_then(serde_json::Value::as_i64);
    let now = crate::identity::now_ms();
    let timestamp = served_at.map_or(now, |served_at| now.max(served_at + 1));
    let deployment = profile_deployment(identity, index, timestamp);
    let deployed = deploy(client, &endpoints.entities, &deployment).await?;
    Ok(match served {
        Some(_) => Ensured::Replaced(deployed),
        None => Ensured::Deployed(deployed),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entity_id_is_the_cid_v1_of_the_bytes() {
        // computed apart from this code: SHA-256, the prefix 01 55 12 20, RFC 4648 base32
        assert_eq!(
            cid_v1_raw(b""),
            "bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku"
        );
        assert_eq!(
            cid_v1_raw(b"comms-e2e profile"),
            "bafkreihserwni6zs3nepabl5aulv4l3k42yypbzafqrs4pnbfuhghkqrki"
        );
        // and as the content server names an uploaded file
        let deployment = profile_deployment(&Identity::visitor("unit", 2), 2, 1_700_000_000_000);
        assert_eq!(deployment.entity_id, cid_v1_raw(&deployment.entity));
        assert_eq!(
            deployment.entity_id,
            dcl_auth::hash_bytes_v1(&deployment.entity)
        );
    }

    #[test]
    fn the_entity_file_is_a_profile_of_the_wallet_alone() {
        let identity = Identity::visitor("unit", 7);
        let deployment = profile_deployment(&identity, 7, 1_700_000_000_123);
        let entity: serde_json::Value = serde_json::from_slice(&deployment.entity).unwrap();
        let text = String::from_utf8(deployment.entity.clone()).unwrap();
        // the explorer's field order
        let order: Vec<usize> = [
            "\"version\"",
            "\"type\"",
            "\"pointers\"",
            "\"timestamp\"",
            "\"metadata\"",
        ]
        .iter()
        .map(|key| text.find(key).unwrap())
        .collect();
        assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{text}");
        assert_eq!(entity["version"], "v3");
        assert_eq!(entity["type"], "profile");
        assert_eq!(entity["pointers"], serde_json::json!([identity.address()]));
        assert_eq!(entity["timestamp"], 1_700_000_000_123i64);
        assert!(entity.get("content").is_none());
        let avatars = entity["metadata"]["avatars"].as_array().unwrap();
        assert_eq!(avatars.len(), 1);
        let avatar = &avatars[0];
        assert_eq!(avatar["name"], "ce2e-visit-07");
        assert_eq!(avatar["hasClaimedName"], false);
        assert_eq!(avatar["version"], PROFILE_VERSION);
        assert_eq!(avatar["ethAddress"], identity.address());
        assert!(avatar["avatar"].get("snapshots").is_none());
        for wearable in avatar["avatar"]["wearables"].as_array().unwrap() {
            assert!(
                wearable
                    .as_str()
                    .unwrap()
                    .starts_with("urn:decentraland:off-chain:base-avatars:")
            );
        }
    }

    #[test]
    fn looks_vary_by_index() {
        let looks: std::collections::BTreeSet<String> = (0..20)
            .map(|index| {
                let avatar = visit_avatar("0x0000000000000000000000000000000000000001", index);
                format!(
                    "{}{}{}",
                    avatar["avatar"]["bodyShape"],
                    avatar["avatar"]["skin"],
                    avatar["avatar"]["hair"]
                )
            })
            .collect();
        assert!(looks.len() >= 10, "{looks:?}");
        let shapes: std::collections::BTreeSet<String> = (0..2)
            .map(|index| visit_avatar("0x01", index)["avatar"]["bodyShape"].to_string())
            .collect();
        assert_eq!(shapes.len(), 2);
    }

    #[test]
    fn the_form_carries_the_id_the_chain_and_the_file_named_by_the_cid() {
        let deployment = profile_deployment(&Identity::visitor("unit", 0), 0, 1_700_000_000_000);
        let fields = deployment.fields();
        assert_eq!(
            fields[0],
            ("entityId".to_owned(), deployment.entity_id.clone())
        );
        let names: Vec<&str> = fields.iter().map(|(name, _)| name.as_str()).collect();
        for index in 0..3 {
            for part in ["type", "payload", "signature"] {
                assert!(names.contains(&format!("authChain[{index}][{part}]").as_str()));
            }
        }
        assert!(fields.contains(&("authChain[0][type]".to_owned(), "SIGNER".to_owned())));
        assert!(fields.contains(&("authChain[0][signature]".to_owned(), String::new())));
        assert!(fields.contains(&(
            "authChain[2][payload]".to_owned(),
            deployment.entity_id.clone()
        )));
        assert!(deployment.signs_its_id());
        let form = deployment.form();
        let boundary = deployment.boundary();
        let text = String::from_utf8(form).unwrap();
        assert!(text.contains(&format!(
            "name=\"{}\"\r\nContent-Type: application/octet-stream\r\n\r\n{}\r\n--{boundary}--",
            deployment.entity_id,
            String::from_utf8(deployment.entity.clone()).unwrap()
        )));
        assert_eq!(
            text.matches(&format!("--{boundary}\r\n")).count(),
            fields.len() + 1
        );
    }

    #[test]
    fn an_address_of_the_explorers_front_is_dialed_at_the_api() {
        let endpoints = Endpoints {
            entities: "http://127.0.0.1:6210/content/entities".parse().unwrap(),
            lambdas: "http://127.0.0.1:6210/lambdas".parse().unwrap(),
        };
        let real: Url = "http://127.0.0.1:5210/".parse().unwrap();
        let past = endpoints
            .past_front("127.0.0.1:6210".parse().unwrap(), &real)
            .unwrap();
        assert_eq!(
            past.entities.as_str(),
            "http://127.0.0.1:5210/content/entities"
        );
        assert_eq!(past.lambdas.as_str(), "http://127.0.0.1:5210/lambdas");
        assert_eq!(
            endpoints.past_front("127.0.0.1:7000".parse().unwrap(), &real),
            None
        );
    }

    #[test]
    fn an_ipv6_host_is_named_without_its_brackets() {
        let endpoints = Endpoints {
            entities: "http://[::1]:5210/content/entities".parse().unwrap(),
            lambdas: "http://[::1]:5210/lambdas".parse().unwrap(),
        };
        assert_eq!(endpoints.hosts(), vec!["::1".to_owned()]);
        assert!(crate::stack::is_loopback_host(&endpoints.hosts()[0]));
    }

    #[test]
    fn retry_after_takes_seconds_or_a_date() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-11-23T10:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert_eq!(retry_after("2", now), Some(Duration::from_secs(2)));
        assert_eq!(
            retry_after("Mon, 23 Nov 2026 10:00:05 GMT", now),
            Some(Duration::from_secs(5))
        );
        assert_eq!(
            retry_after("Mon, 23 Nov 2026 09:00:00 GMT", now),
            Some(Duration::ZERO)
        );
        assert_eq!(retry_after("soon", now), None);
        assert_eq!(retry_after("", now), None);
    }
}
