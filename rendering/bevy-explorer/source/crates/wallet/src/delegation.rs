// Orchestrator-minted, short-lived delegations that let a headless authoritative server sign
// requests to one service for one scene without ever holding the authoritative key: World
// storage, and Credits (scene payments). Both share the wire envelope (base64 JSON, mirrors
// hammurabi-headless), its decoding and the delegated signer:
// { "v": 1, "ephemeral": { "privateKey", "publicKey", "address" },
//   "scope": { "payload", "signature" } }
// The scene-scope fields (World/SceneId/Parcel/Expiration) are derived from the signed
// scope.payload lines — the single source of truth — never from unsigned copies.

use std::{collections::HashMap, str::FromStr};

use alloy_signer_local::PrivateKeySigner;
use base64::Engine;
use bevy::prelude::*;
use serde::Deserialize;

use crate::Wallet;

pub const STORAGE_HOSTS: [&str; 2] = ["storage.decentraland.org", "storage.decentraland.zone"];

#[derive(Deserialize)]
struct DelegationEnvelope {
    v: u32,
    ephemeral: DelegationEphemeral,
    scope: DelegationScope,
}

#[derive(Deserialize)]
struct DelegationEphemeral {
    #[serde(rename = "privateKey")]
    private_key: String,
    address: String,
}

#[derive(Deserialize, serde::Serialize, Clone)]
pub struct DelegationScope {
    pub payload: String,
    pub signature: String,
}

/// A decoded envelope whose ephemeral key derives the address the envelope names.
struct Envelope {
    scope: DelegationScope,
    signer: PrivateKeySigner,
    /// lowercased, derived from the key
    address: String,
}

/// Decode a base64 delegation envelope of `kind` and check its version and that its key derives
/// its address. Errors never echo decoded content: it contains the ephemeral private key.
fn decode_envelope(encoded: &str, kind: &str) -> Result<Envelope, anyhow::Error> {
    let json = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|_| anyhow::anyhow!("{kind} delegation is not valid base64"))?;
    let envelope: DelegationEnvelope = serde_json::from_slice(&json)
        .map_err(|_| anyhow::anyhow!("{kind} delegation envelope is malformed"))?;
    if envelope.v != 1 {
        anyhow::bail!("unsupported {kind} delegation version {}", envelope.v);
    }
    let signer =
        PrivateKeySigner::from_str(envelope.ephemeral.private_key.trim_start_matches("0x"))
            .map_err(|_| anyhow::anyhow!("{kind} delegation ephemeral key is invalid"))?;
    let address = format!("{:#x}", signer.address());
    if envelope.ephemeral.address.to_lowercase() != address {
        anyhow::bail!("{kind} delegation ephemeral address does not match its key");
    }
    Ok(Envelope {
        scope: envelope.scope,
        signer,
        address,
    })
}

impl Envelope {
    /// The ephemeral's standalone wallet (authorized only by the scope claim) and the
    /// pre-encoded `x-authoritative-scope` value, base64(json(scope)).
    fn into_signer(self) -> Result<(Wallet, String), anyhow::Error> {
        let scope_header =
            base64::engine::general_purpose::STANDARD.encode(serde_json::to_string(&self.scope)?);
        let mut wallet = Wallet::default();
        wallet.finalize(self.signer.address(), self.signer, Vec::default(), false);
        Ok((wallet, scope_header))
    }
}

/// `meta` with the body's SHA-256 added as `hashPayload` when there is a body, every other
/// claim-derived field kept as the delegation built it.
fn delegated_meta(meta: &str, body_hash: Option<String>) -> Result<String, serde_json::Error> {
    let Some(hash) = body_hash else {
        return Ok(meta.to_owned());
    };
    let mut value: serde_json::Value = serde_json::from_str(meta)?;
    value["hashPayload"] = hash.into();
    Ok(value.to_string())
}

/// Sign a request with a delegation: the signed-fetch chain of its ephemeral `wallet` over
/// `method` and the URI's path, with the delegation's `meta` (plus `hashPayload` for a body), and
/// its claim as `x-authoritative-scope`. Storage and Credits requests are both signed here.
pub async fn sign_delegated(
    method: &str,
    uri: &http::Uri,
    wallet: &Wallet,
    meta: &str,
    body_hash: Option<String>,
    scope_header: &str,
) -> Result<Vec<(String, String)>, anyhow::Error> {
    let meta = delegated_meta(meta, body_hash)?;
    let mut headers = crate::sign_request(method, uri, wallet, meta).await?;
    headers.push(("x-authoritative-scope".to_owned(), scope_header.to_owned()));
    Ok(headers)
}

#[derive(Clone)]
pub struct StorageDelegation {
    /// standalone ephemeral signer (owner = ephemeral address; authorized only by the scope claim)
    pub wallet: Wallet,
    /// pre-encoded `x-authoritative-scope` header value: base64(json(scope))
    pub scope_header: String,
    /// replacement `x-identity-metadata` JSON, built wholesale from the delegation claims
    pub meta: String,
    pub world: String,
    pub scene_id: String,
    pub parcel: String,
    /// unix millis
    pub expiration: i64,
}

impl StorageDelegation {
    /// Decode and validate a base64 delegation. `realm_hostname` is only reported in the
    /// request metadata (the verifier keys on the claim's world/sceneId/parcel).
    /// Errors never echo decoded content — it contains the ephemeral private key.
    pub fn parse(encoded: &str, realm_hostname: &str) -> Result<Self, anyhow::Error> {
        let envelope = decode_envelope(encoded, "storage")?;
        // an audience-bound claim (Credits) never fills a storage slot, whatever fields it shares
        if envelope.scope.payload.lines().next() == Some(AUDIENCE_DELEGATION_PREFIX) {
            anyhow::bail!("storage delegation is an audience-bound claim");
        }
        let value_for = |prefix: &str| -> Option<String> {
            envelope
                .scope
                .payload
                .lines()
                .find(|l| l.starts_with(prefix))
                .map(|l| l[prefix.len()..].trim().to_owned())
        };
        let world = value_for("World:")
            .map(|w| w.to_lowercase())
            .ok_or_else(|| anyhow::anyhow!("delegation claim missing World"))?;
        let scene_id = value_for("SceneId:")
            .ok_or_else(|| anyhow::anyhow!("delegation claim missing SceneId"))?;
        let parcel = value_for("Parcel:")
            .ok_or_else(|| anyhow::anyhow!("delegation claim missing Parcel"))?;
        let expiration_iso = value_for("Expiration:")
            .ok_or_else(|| anyhow::anyhow!("delegation claim missing Expiration"))?;
        let expiration = chrono::DateTime::parse_from_rfc3339(&expiration_iso)
            .map_err(|_| anyhow::anyhow!("delegation claim has unparseable Expiration"))?
            .timestamp_millis();
        let (wallet, scope_header) = envelope.into_signer()?;

        // deliberately the DELEGATION's world/sceneId/parcel — the storage service derives
        // the placeId the scope claim is bound to from these (hammurabi parity, including
        // the historical 'hammurabi-server//' origin embedded in signed payloads)
        let meta = serde_json::json!({
            "origin": "hammurabi-server//",
            "signer": "dcl:authoritative-server",
            "isGuest": false,
            "realm": { "serverName": world, "hostname": realm_hostname },
            "realmName": world,
            "sceneId": scene_id,
            "parcel": parcel,
        })
        .to_string();

        Ok(Self {
            wallet,
            scope_header,
            meta,
            world,
            scene_id,
            parcel,
            expiration,
        })
    }

    pub fn is_expired(&self, now_millis: i64) -> bool {
        now_millis >= self.expiration
    }
}

/// Per-scene world-storage delegations, plus a fallback slot for single-scene
/// (non-orchestrated) runs where the scene hash isn't known at startup.
#[derive(Resource, Default)]
pub struct StorageDelegations {
    pub by_scene: HashMap<String, StorageDelegation>,
    pub fallback: Option<StorageDelegation>,
}

impl StorageDelegations {
    pub fn get(&self, scene: &str) -> Option<&StorageDelegation> {
        // the fallback is still bound to the scene its claim was minted for (claim SceneId
        // and the lookup key are both the scene entity hash) — it must never become a
        // wildcard credential for co-tenant scenes
        self.by_scene
            .get(scene)
            .or_else(|| self.fallback.as_ref().filter(|d| d.scene_id == scene))
    }
}

/// The audience-bound claim format (`@dcl/authoritative-delegation`), distinct from the storage
/// claim's prefix so neither ever parses as the other.
const AUDIENCE_DELEGATION_PREFIX: &str = "Decentraland Authoritative Delegation";
const AUDIENCE_CLAIM_FIELDS: [&str; 6] = [
    "Audience:",
    "Ephemeral:",
    "World:",
    "SceneId:",
    "Parcel:",
    "Expiration:",
];
const CREDITS_AUDIENCE: &str = "credits";

/// A scene's Credits delegation: an orchestrator-minted ephemeral key and the root-signed
/// `Audience: credits` claim that authorizes it for one scene until it expires. The engine signs
/// only Credits requests to credits-server with it, never a scene's own signed fetch.
#[derive(Clone)]
pub struct CreditsDelegation {
    wallet: Wallet,
    /// pre-encoded `x-authoritative-scope` header value: base64(json(scope))
    scope_header: String,
    /// `x-identity-metadata` JSON before the body digest is added
    meta: String,
    world: String,
    scene_id: String,
    parcel: String,
    /// unix millis
    expiration: i64,
}

impl CreditsDelegation {
    /// Decode a base64 Credits delegation and check its claim's structure: the prefix line, then
    /// exactly the six fields once each with a value free of whitespace, `Audience: credits`, and
    /// the `Ephemeral` the key it came with. The value formats are credits-server's to enforce.
    /// Errors never echo decoded content (it holds the key).
    pub fn parse(encoded: &str) -> Result<Self, anyhow::Error> {
        let envelope = decode_envelope(encoded, "credits")?;

        let mut lines = envelope.scope.payload.split('\n');
        if lines.next() != Some(AUDIENCE_DELEGATION_PREFIX) {
            anyhow::bail!("credits delegation claim has the wrong prefix");
        }
        let mut values: HashMap<&str, &str> = HashMap::new();
        for line in lines {
            let field = AUDIENCE_CLAIM_FIELDS
                .iter()
                .find(|field| {
                    line.strip_prefix(**field)
                        .is_some_and(|rest| rest.starts_with(' '))
                })
                .ok_or_else(|| anyhow::anyhow!("credits delegation claim has an unknown line"))?;
            let value = &line[field.len() + 1..];
            if value.is_empty() || value.contains(char::is_whitespace) {
                anyhow::bail!("credits delegation claim has a malformed {field}");
            }
            if values.insert(*field, value).is_some() {
                anyhow::bail!("credits delegation claim repeats {field}");
            }
        }
        if values.len() != AUDIENCE_CLAIM_FIELDS.len() {
            anyhow::bail!("credits delegation claim is missing a field");
        }
        if values["Audience:"] != CREDITS_AUDIENCE {
            anyhow::bail!("delegation claim is not for the credits audience");
        }
        let expiration = chrono::DateTime::parse_from_rfc3339(values["Expiration:"])
            .map_err(|_| anyhow::anyhow!("credits delegation claim has unparseable Expiration"))?
            .timestamp_millis();

        if values["Ephemeral:"] != envelope.address {
            anyhow::bail!("credits delegation claim does not authorize its own key");
        }
        let world = values["World:"].to_owned();
        let scene_id = values["SceneId:"].to_owned();
        let parcel = values["Parcel:"].to_owned();
        let (wallet, scope_header) = envelope.into_signer()?;
        // credits-server reads only `signer` (a scene's own signer is refused) and the body
        // digest added per request; the location it trusts is the claim's
        let meta = serde_json::json!({
            "signer": "dcl:authoritative-server",
            "isGuest": false,
            "realmName": world,
            "sceneId": scene_id,
            "parcel": parcel,
        })
        .to_string();

        Ok(Self {
            wallet,
            scope_header,
            meta,
            world,
            scene_id,
            parcel,
            expiration,
        })
    }

    fn is_expired(&self, now_millis: i64) -> bool {
        now_millis >= self.expiration
    }

    /// Sign a request to credits-server with this delegation (see [`sign_delegated`]).
    pub async fn sign(
        &self,
        method: &str,
        uri: &http::Uri,
        body_hash: Option<String>,
    ) -> Result<Vec<(String, String)>, anyhow::Error> {
        sign_delegated(
            method,
            uri,
            &self.wallet,
            &self.meta,
            body_hash,
            &self.scope_header,
        )
        .await
    }
}

/// A scene's Credits assignment: the orchestrator's id for this add-scene, and the delegation it
/// holds, if one has arrived. The id ties every renewal to the assignment that asked for it.
struct CreditsSlot {
    assignment: String,
    delegation: Option<CreditsDelegation>,
}

/// Why a Credits delegation was not stored. Never echoes decoded content.
#[derive(Debug, PartialEq, Eq)]
pub enum CreditsRefusal {
    /// no current assignment with that id: the scene was removed, re-added or never assigned
    NotAssigned,
    /// the claim names another scene, or another world or parcel than the slot's delegation
    WrongScene,
    Malformed(String),
}

impl std::fmt::Display for CreditsRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAssigned => f.write_str("no current assignment with that id"),
            Self::WrongScene => f.write_str("the claim names another scene"),
            Self::Malformed(reason) => write!(f, "malformed: {reason}"),
        }
    }
}

/// Per-scene Credits assignments, held only by an orchestrated headless server. There is no
/// fallback slot: a process-wide Credits credential would act for every co-tenant scene.
///
/// A slot exists only from an accepted add-scene to the scene's removal or next add-scene. A
/// renewal can fill or refresh the slot of its own assignment; it can never create one, so a
/// late answer for a removed or replaced scene changes nothing.
#[derive(Resource, Default)]
pub struct CreditsDelegations {
    slots: HashMap<String, CreditsSlot>,
}

impl CreditsDelegations {
    fn checked(scene: &str, encoded: &str) -> Result<CreditsDelegation, CreditsRefusal> {
        let delegation = CreditsDelegation::parse(encoded)
            .map_err(|e| CreditsRefusal::Malformed(e.to_string()))?;
        if delegation.scene_id != scene {
            return Err(CreditsRefusal::WrongScene);
        }
        Ok(delegation)
    }

    /// Open the scene's slot for a newly accepted add-scene, replacing any earlier assignment.
    /// A delegation that does not check out leaves the slot open but empty, to be filled by a
    /// renewal for this assignment.
    pub fn assign(
        &mut self,
        scene: &str,
        assignment: String,
        encoded: Option<&str>,
    ) -> Result<(), CreditsRefusal> {
        let delegation = encoded.map(|encoded| Self::checked(scene, encoded));
        let (delegation, result) = match delegation {
            Some(Ok(delegation)) => (Some(delegation), Ok(())),
            Some(Err(refusal)) => (None, Err(refusal)),
            None => (None, Ok(())),
        };
        self.slots.insert(
            scene.to_owned(),
            CreditsSlot {
                assignment,
                delegation,
            },
        );
        result
    }

    /// Store a renewal for the scene's current assignment. Its claim must name the scene, and the
    /// world and parcel of the delegation already held, if any.
    pub fn renew(
        &mut self,
        scene: &str,
        assignment: &str,
        encoded: &str,
    ) -> Result<(), CreditsRefusal> {
        let Some(slot) = self
            .slots
            .get_mut(scene)
            .filter(|slot| slot.assignment == assignment)
        else {
            return Err(CreditsRefusal::NotAssigned);
        };
        let delegation = Self::checked(scene, encoded)?;
        if slot.delegation.as_ref().is_some_and(|current| {
            current.world != delegation.world || current.parcel != delegation.parcel
        }) {
            return Err(CreditsRefusal::WrongScene);
        }
        // renewals can arrive out of order: keep whichever expires later
        if slot
            .delegation
            .as_ref()
            .is_none_or(|current| delegation.expiration >= current.expiration)
        {
            slot.delegation = Some(delegation);
        }
        Ok(())
    }

    pub fn remove(&mut self, scene: &str) {
        self.slots.remove(scene);
    }

    /// Whether the scene has a Credits assignment, filled or not.
    pub fn is_assigned(&self, scene: &str) -> bool {
        self.slots.contains_key(scene)
    }

    /// The scene's delegation while it is valid, and only when its claim names that scene.
    pub fn valid_for(&self, scene: &str, now_millis: i64) -> Option<&CreditsDelegation> {
        self.slots
            .get(scene)
            .and_then(|slot| slot.delegation.as_ref())
            .filter(|d| d.scene_id == scene && !d.is_expired(now_millis))
    }

    /// `(scene, assignment)` for every slot that is empty or expires within `buffer_millis`.
    pub fn needing_renewal(
        &self,
        now_millis: i64,
        buffer_millis: i64,
    ) -> impl Iterator<Item = (&str, &str)> {
        self.slots.iter().filter_map(move |(scene, slot)| {
            slot.delegation
                .as_ref()
                .is_none_or(|d| now_millis >= d.expiration - buffer_millis)
                .then_some((scene.as_str(), slot.assignment.as_str()))
        })
    }
}

/// True when a signed fetch to `uri` must be signed with a storage delegation: exact-match
/// world-storage hosts, or the resolved storage service (host and port; an explicit override
/// chooses them). https only either way — the claim must never go out in cleartext, so an http
/// storage override is simply never signed for.
pub fn is_storage_request(uri: &http::Uri) -> bool {
    fn https_origin(u: &http::Uri) -> Option<(String, u16)> {
        if u.scheme_str() != Some("https") {
            return None;
        }
        Some((u.host()?.to_lowercase(), u.port_u16().unwrap_or(443)))
    }
    let Some((host, port)) = https_origin(uri) else {
        return false;
    };
    STORAGE_HOSTS.contains(&host.as_str())
        || common::base_domain::service(common::base_domain::Service::Storage)
            .parse::<http::Uri>()
            .ok()
            .and_then(|storage| https_origin(&storage))
            == Some((host, port))
}

#[cfg(test)]
mod test {
    use super::*;

    fn mint(expiration: &str) -> String {
        // throwaway key
        let payload = format!(
            "Decentraland Authoritative Storage Delegation\nEphemeral: 0x63f9a92d8d61b48a9fff8d58080425a3012d05c8\nWorld: Boedo.dcl.eth\nSceneId: bafkreitest\nParcel: 0,0\nExpiration: {expiration}"
        );
        let envelope = serde_json::json!({
            "v": 1,
            "ephemeral": {
                "privateKey": "0000000000000000000000000000000000000000000000000000000000000001",
                "publicKey": "unused",
                "address": "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf",
            },
            "scope": { "payload": payload, "signature": "0xsig" },
        });
        base64::engine::general_purpose::STANDARD.encode(envelope.to_string())
    }

    #[test]
    fn parses_and_lowercases_world() {
        let d = StorageDelegation::parse(&mint("2030-01-01T00:00:00Z"), "https://realm").unwrap();
        assert_eq!(d.world, "boedo.dcl.eth");
        assert_eq!(d.scene_id, "bafkreitest");
        assert_eq!(d.parcel, "0,0");
        assert!(!d.is_expired(chrono::Utc::now().timestamp_millis()));
        assert!(d.meta.contains("\"signer\":\"dcl:authoritative-server\""));
    }

    #[test]
    fn rejects_mismatched_ephemeral_address() {
        let payload = "Decentraland Authoritative Storage Delegation\nWorld: w\nSceneId: s\nParcel: 0,0\nExpiration: 2030-01-01T00:00:00Z";
        let envelope = serde_json::json!({
            "v": 1,
            "ephemeral": {
                "privateKey": "0000000000000000000000000000000000000000000000000000000000000001",
                "publicKey": "unused",
                "address": "0x0000000000000000000000000000000000000dead",
            },
            "scope": { "payload": payload, "signature": "0xsig" },
        });
        let encoded = base64::engine::general_purpose::STANDARD.encode(envelope.to_string());
        assert!(StorageDelegation::parse(&encoded, "").is_err());
    }

    #[test]
    fn fallback_only_serves_its_own_scene() {
        let delegations = StorageDelegations {
            fallback: Some(
                StorageDelegation::parse(&mint("2030-01-01T00:00:00Z"), "https://realm").unwrap(),
            ),
            ..Default::default()
        };
        assert!(delegations.get("bafkreitest").is_some());
        assert!(delegations.get("bafkreiother").is_none());
    }

    #[test]
    fn storage_host_matching() {
        assert!(is_storage_request(
            &"https://storage.decentraland.org/values/x".parse().unwrap()
        ));
        assert!(!is_storage_request(
            &"http://storage.decentraland.org/values/x".parse().unwrap()
        ));
        assert!(!is_storage_request(
            &"https://storage.decentraland.org.evil.com/values"
                .parse()
                .unwrap()
        ));
    }

    // NOTE: latches the process-wide storage override — the only test in this crate that may.
    #[test]
    fn storage_override_matching() {
        common::base_domain::set_services([(
            common::base_domain::Service::Storage,
            "https://storage.example:8443/",
        )])
        .unwrap();
        assert!(is_storage_request(
            &"https://storage.example:8443/values/x".parse().unwrap()
        ));
        // the known hosts stay signed for alongside an override
        assert!(is_storage_request(
            &"https://storage.decentraland.org/values/x".parse().unwrap()
        ));
        // port is part of the origin (default 443 when implicit), and https is not negotiable
        assert!(!is_storage_request(
            &"https://storage.example/values/x".parse().unwrap()
        ));
        assert!(!is_storage_request(
            &"http://storage.example:8443/values/x".parse().unwrap()
        ));
    }

    // throwaway key 1, whose address is 0x7e5f…5bdf
    const KEY_ONE: &str = "0000000000000000000000000000000000000000000000000000000000000001";
    const KEY_ONE_ADDRESS: &str = "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf";

    fn credits_claim(lines: &[&str]) -> String {
        let payload = lines.join("\n");
        let envelope = serde_json::json!({
            "v": 1,
            "ephemeral": { "privateKey": KEY_ONE, "publicKey": "unused", "address": KEY_ONE_ADDRESS },
            "scope": { "payload": payload, "signature": "0xsig" },
        });
        base64::engine::general_purpose::STANDARD.encode(envelope.to_string())
    }

    fn credits_lines(expiration: &str) -> Vec<String> {
        vec![
            "Decentraland Authoritative Delegation".to_owned(),
            "Audience: credits".to_owned(),
            format!("Ephemeral: {KEY_ONE_ADDRESS}"),
            "World: main".to_owned(),
            "SceneId: bafkreiplaza".to_owned(),
            "Parcel: -3,-2".to_owned(),
            format!("Expiration: {expiration}"),
        ]
    }

    fn credits_claim_for(lines: &[String]) -> String {
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        credits_claim(&lines)
    }

    fn parse_credits(lines: &[String]) -> Result<CreditsDelegation, anyhow::Error> {
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        CreditsDelegation::parse(&credits_claim(&lines))
    }

    #[test]
    fn parses_a_credits_claim_and_binds_its_location() {
        let d = parse_credits(&credits_lines("2030-01-01T00:00:00.000Z")).unwrap();
        assert_eq!(
            (d.world.as_str(), d.scene_id.as_str(), d.parcel.as_str()),
            ("main", "bafkreiplaza", "-3,-2")
        );
        assert_eq!(d.expiration, 1_893_456_000_000);
        assert!(d.meta.contains("\"signer\":\"dcl:authoritative-server\""));
        let scope: serde_json::Value = serde_json::from_slice(
            &base64::engine::general_purpose::STANDARD
                .decode(&d.scope_header)
                .unwrap(),
        )
        .unwrap();
        assert!(scope["payload"]
            .as_str()
            .unwrap()
            .contains("Audience: credits"));
        assert_eq!(scope["signature"], "0xsig");
    }

    #[test]
    fn refuses_a_claim_for_another_audience() {
        let mut lines = credits_lines("2030-01-01T00:00:00.000Z");
        lines[1] = "Audience: badges".to_owned();
        assert!(parse_credits(&lines).is_err());
    }

    #[test]
    fn refuses_the_storage_claim_format() {
        let lines = [
            "Decentraland Authoritative Storage Delegation",
            &format!("Ephemeral: {KEY_ONE_ADDRESS}"),
            "World: main",
            "SceneId: bafkreiplaza",
            "Parcel: -3,-2",
            "Expiration: 2030-01-01T00:00:00.000Z",
        ];
        assert!(CreditsDelegation::parse(&credits_claim(&lines)).is_err());
    }

    #[test]
    fn refuses_a_claim_naming_another_ephemeral() {
        let mut lines = credits_lines("2030-01-01T00:00:00.000Z");
        lines[2] = "Ephemeral: 0x0000000000000000000000000000000000000dead".to_owned();
        assert!(parse_credits(&lines).is_err());
    }

    #[test]
    fn refuses_a_repeated_missing_or_unknown_field() {
        let mut repeated = credits_lines("2030-01-01T00:00:00.000Z");
        repeated.push("World: other.dcl.eth".to_owned());
        let mut missing = credits_lines("2030-01-01T00:00:00.000Z");
        missing.remove(4);
        let mut unknown = credits_lines("2030-01-01T00:00:00.000Z");
        unknown.push("Realm: main".to_owned());
        let mut spaced = credits_lines("2030-01-01T00:00:00.000Z");
        spaced[5] = "Parcel: -3, -2".to_owned();
        for lines in [repeated, missing, unknown, spaced] {
            assert!(parse_credits(&lines).is_err());
        }
    }

    #[test]
    fn a_credits_claim_is_never_parsed_as_a_storage_delegation() {
        let encoded = credits_claim_for(&credits_lines("2030-01-01T00:00:00.000Z"));
        assert!(CreditsDelegation::parse(&encoded).is_ok());
        assert!(StorageDelegation::parse(&encoded, "https://realm").is_err());
    }

    #[test]
    fn a_credits_delegation_serves_only_its_scene_until_it_expires() {
        let mut delegations = CreditsDelegations::default();
        let encoded = credits_claim_for(&credits_lines("2030-01-01T00:00:00.000Z"));
        delegations
            .assign("bafkreiplaza", "a1".into(), Some(&encoded))
            .unwrap();
        let before = 1_893_455_999_999;
        assert!(delegations.valid_for("bafkreiplaza", before).is_some());
        assert!(delegations.valid_for("bafkreiplaza", before + 1).is_none());
        // a claim for the plaza is never stored for another scene
        assert_eq!(
            delegations.assign("bafkreiother", "a2".into(), Some(&encoded)),
            Err(CreditsRefusal::WrongScene)
        );
        assert!(delegations.is_assigned("bafkreiother"));
        assert!(delegations.valid_for("bafkreiother", before).is_none());
    }

    #[test]
    fn a_renewal_never_creates_an_assignment() {
        let mut delegations = CreditsDelegations::default();
        let encoded = credits_claim_for(&credits_lines("2030-01-01T00:00:00.000Z"));
        assert_eq!(
            delegations.renew("bafkreiplaza", "a1", &encoded),
            Err(CreditsRefusal::NotAssigned)
        );
        assert!(!delegations.is_assigned("bafkreiplaza"));
    }

    #[test]
    fn a_late_renewal_after_removal_changes_nothing() {
        let mut delegations = CreditsDelegations::default();
        let encoded = credits_claim_for(&credits_lines("2030-01-01T00:00:00.000Z"));
        delegations
            .assign("bafkreiplaza", "a1".into(), Some(&encoded))
            .unwrap();
        delegations.remove("bafkreiplaza");
        assert_eq!(
            delegations.renew("bafkreiplaza", "a1", &encoded),
            Err(CreditsRefusal::NotAssigned)
        );
        assert!(!delegations.is_assigned("bafkreiplaza"));
    }

    #[test]
    fn a_renewal_for_a_replaced_assignment_is_refused() {
        let mut delegations = CreditsDelegations::default();
        let old = credits_claim_for(&credits_lines("2030-01-01T00:00:00.000Z"));
        delegations
            .assign("bafkreiplaza", "a1".into(), Some(&old))
            .unwrap();
        // the same scene hash re-added: a new assignment, still waiting for its delegation
        delegations
            .assign("bafkreiplaza", "a2".into(), None)
            .unwrap();
        assert_eq!(
            delegations.renew("bafkreiplaza", "a1", &old),
            Err(CreditsRefusal::NotAssigned)
        );
        assert!(delegations.valid_for("bafkreiplaza", 0).is_none());
        // its own assignment's answer fills it
        delegations.renew("bafkreiplaza", "a2", &old).unwrap();
        assert!(delegations.valid_for("bafkreiplaza", 0).is_some());
    }

    #[test]
    fn a_renewal_cannot_move_the_scene() {
        let mut delegations = CreditsDelegations::default();
        let held = credits_claim_for(&credits_lines("2030-01-01T00:00:00.000Z"));
        delegations
            .assign("bafkreiplaza", "a1".into(), Some(&held))
            .unwrap();
        let mut moved = credits_lines("2030-01-01T00:00:00.000Z");
        moved[5] = "Parcel: 0,0".to_owned();
        assert_eq!(
            delegations.renew("bafkreiplaza", "a1", &credits_claim_for(&moved)),
            Err(CreditsRefusal::WrongScene)
        );
        let mut other_scene = credits_lines("2030-01-01T00:00:00.000Z");
        other_scene[4] = "SceneId: bafkreiother".to_owned();
        assert_eq!(
            delegations.renew("bafkreiplaza", "a1", &credits_claim_for(&other_scene)),
            Err(CreditsRefusal::WrongScene)
        );
    }

    #[test]
    fn delegation_claims_are_preserved_when_body_is_bound() {
        let original = r#"{"signer":"dcl:authoritative-server","sceneId":"Mixed","parcel":"1,2"}"#;
        let digest = "44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a";
        assert_eq!(delegated_meta(original, None).unwrap(), original);
        let meta: serde_json::Value =
            serde_json::from_str(&delegated_meta(original, Some(digest.to_owned())).unwrap())
                .unwrap();
        assert_eq!(meta["sceneId"], "Mixed");
        assert_eq!(meta["signer"], "dcl:authoritative-server");
        assert_eq!(meta["parcel"], "1,2");
        assert_eq!(meta["hashPayload"], digest);
    }

    #[test]
    fn a_renewal_that_arrives_late_keeps_the_later_expiry() {
        let mut delegations = CreditsDelegations::default();
        let earlier = credits_claim_for(&credits_lines("2030-01-01T00:00:00.000Z"));
        let later = credits_claim_for(&credits_lines("2030-01-01T00:10:00.000Z"));
        delegations
            .assign("bafkreiplaza", "a1".into(), Some(&earlier))
            .unwrap();
        delegations.renew("bafkreiplaza", "a1", &later).unwrap();
        delegations.renew("bafkreiplaza", "a1", &earlier).unwrap();
        let expiry = 1_893_456_000_000;
        assert!(delegations.valid_for("bafkreiplaza", expiry).is_some());
    }

    #[test]
    fn empty_and_expiring_slots_ask_for_renewal() {
        let mut delegations = CreditsDelegations::default();
        let encoded = credits_claim_for(&credits_lines("2030-01-01T00:00:00.000Z"));
        delegations
            .assign("bafkreiplaza", "a1".into(), Some(&encoded))
            .unwrap();
        delegations
            .assign("bafkreiempty", "a2".into(), None)
            .unwrap();
        let expiry = 1_893_456_000_000;
        let due = |now| {
            let mut due: Vec<_> = delegations.needing_renewal(now, 300_000).collect();
            due.sort();
            due
        };
        assert_eq!(due(expiry - 300_001), vec![("bafkreiempty", "a2")]);
        assert_eq!(
            due(expiry - 300_000),
            vec![("bafkreiempty", "a2"), ("bafkreiplaza", "a1")]
        );
    }
}
