//! Who can enter a world.
//!
//! The worlds server keeps one `access` setting per world: anyone, a shared
//! password, or an allow-list of wallets and communities. This is the owner's
//! side of it, spoken in names: an avatar name or a community name resolves to
//! the address or id the server wants, and what the server holds resolves back
//! to names, faces and member counts before it is shown. Profiles and claimed
//! names come from the catalyst (`DCL_SCENE_CLI_CATALYST`), communities from the
//! social API (`DCL_SCENE_CLI_SOCIAL_API`); neither is written to.
//!
//! `edit` hosts a browser page that does the same with a wallet attached: it
//! recognises the connected owner, lists the communities they belong to and
//! the members of any community they pick, and signs one replacement of the
//! whole setting.

use crate::deploy::{
    encode_segment, now_ms, read_server_message, refusal, send_text, with_headers,
};
use crate::linker::{DoneSender, LinkerOptions};
use crate::ux::{self, TrySteps, UserError};
use crate::world::{self, BrowserOptions, WorldAction, signed_fetch_payload_meta};
use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    response::Html,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

pub const DEFAULT_SOCIAL_API: &str = "https://social-api.decentraland.org";
const SOCIAL_ENV: &str = "DCL_SCENE_CLI_SOCIAL_API";

/// The worlds server's own caps on an allow-list; refused here so the wallet
/// is never asked to sign a request the server will bounce.
pub const MAX_WALLETS: usize = 1000;
pub const MAX_COMMUNITIES: usize = 50;

/// Community thumbnails live on the assets CDN keyed by id, not in the
/// community record.
fn community_thumbnail(id: &str) -> String {
    format!("https://assets-cdn.decentraland.org/social/communities/{id}/raw-thumbnail.png")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessSpec {
    Unrestricted,
    SharedSecret {
        secret: String,
    },
    AllowList {
        wallets: Vec<String>,
        communities: Vec<String>,
    },
}

impl AccessSpec {
    /// Lowercased and de-duplicated, first occurrence wins, so the signed
    /// metadata is canonical whatever order the flags came in.
    pub fn allow_list(wallets: &[String], communities: &[String]) -> Self {
        AccessSpec::AllowList {
            wallets: dedup_lower(wallets),
            communities: dedup_lower(communities),
        }
    }

    pub fn validate(&self) -> Result<()> {
        match self {
            AccessSpec::Unrestricted => Ok(()),
            AccessSpec::SharedSecret { secret } => {
                if secret.trim().is_empty() {
                    return Err(UserError::new(
                        "the password is empty",
                        TrySteps::one("pass --password <secret> with at least one character"),
                    )
                    .into());
                }
                Ok(())
            }
            AccessSpec::AllowList {
                wallets,
                communities,
            } => {
                for w in wallets {
                    check_address(w)?;
                }
                for c in communities {
                    check_community_id(c)?;
                }
                if wallets.len() > MAX_WALLETS {
                    return Err(UserError::new(
                        format!(
                            "{} wallets on the list; the worlds server takes at most {MAX_WALLETS}",
                            wallets.len()
                        ),
                        TrySteps::one(
                            "put the crowd in a community and allow the community instead",
                        ),
                    )
                    .into());
                }
                if communities.len() > MAX_COMMUNITIES {
                    return Err(UserError::new(
                        format!(
                            "{} communities on the list; the worlds server takes at most {MAX_COMMUNITIES}",
                            communities.len()
                        ),
                        TrySteps::one("trim the list to the communities that matter"),
                    )
                    .into());
                }
                Ok(())
            }
        }
    }

    /// The signed-fetch metadata, exactly the bytes the wallet signs and the
    /// `x-identity-metadata` header carries.
    pub fn metadata_json(&self) -> String {
        let v = match self {
            AccessSpec::Unrestricted => json!({ "type": "unrestricted" }),
            AccessSpec::SharedSecret { secret } => {
                json!({ "type": "shared-secret", "secret": secret })
            }
            AccessSpec::AllowList {
                wallets,
                communities,
            } => json!({
                "type": "allow-list",
                "wallets": wallets,
                "communities": communities,
            }),
        };
        v.to_string()
    }

    pub fn kind(&self) -> &'static str {
        match self {
            AccessSpec::Unrestricted => "unrestricted",
            AccessSpec::SharedSecret { .. } => "shared-secret",
            AccessSpec::AllowList { .. } => "allow-list",
        }
    }

    pub fn summary(&self) -> String {
        match self {
            AccessSpec::Unrestricted => "open the world to anyone".to_string(),
            AccessSpec::SharedSecret { .. } => "gate the world behind a password".to_string(),
            AccessSpec::AllowList {
                wallets,
                communities,
            } => format!(
                "restrict entry to {} and {}",
                plural(wallets.len(), "person", "people"),
                plural(communities.len(), "community", "communities")
            ),
        }
    }

    pub fn success(&self, name: &str) -> String {
        match self {
            AccessSpec::Unrestricted => format!("{name} is open to anyone"),
            AccessSpec::SharedSecret { .. } => format!("{name} now asks for the password"),
            AccessSpec::AllowList {
                wallets,
                communities,
            } => format!(
                "{name} now admits {} and the members of {}",
                plural(wallets.len(), "person", "people"),
                plural(communities.len(), "community", "communities")
            ),
        }
    }

    /// One line per entry for the signing page.
    pub fn details(&self) -> Vec<String> {
        match self {
            AccessSpec::Unrestricted => vec!["access=unrestricted".to_string()],
            AccessSpec::SharedSecret { .. } => vec![
                "access=shared-secret".to_string(),
                "the password is signed, not shown".to_string(),
            ],
            AccessSpec::AllowList {
                wallets,
                communities,
            } => {
                let mut out = vec!["access=allow-list".to_string()];
                out.extend(wallets.iter().map(|w| format!("wallet={w}")));
                out.extend(communities.iter().map(|c| format!("community={c}")));
                out
            }
        }
    }

    /// What a `GET /world/<name>/permissions` body says the setting is. A
    /// shared secret comes back as its type only (the hash stays on the
    /// server), so a round trip through here cannot re-sign a password.
    pub fn from_permissions(v: &Value) -> Self {
        let access = v
            .pointer("/permissions/access")
            .cloned()
            .unwrap_or_default();
        match access.get("type").and_then(Value::as_str) {
            Some("shared-secret") => AccessSpec::SharedSecret {
                secret: String::new(),
            },
            Some("allow-list") => AccessSpec::allow_list(
                &strings(access.get("wallets")),
                &strings(access.get("communities")),
            ),
            _ => AccessSpec::Unrestricted,
        }
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn dedup_lower(items: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    items
        .iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty() && seen.insert(s.clone()))
        .collect()
}

fn strings(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn check_address(address: &str) -> Result<()> {
    if dcl_auth::is_eth_address(address) {
        return Ok(());
    }
    Err(UserError::new(
        format!("\"{address}\" is not an ethereum address"),
        TrySteps::one("expect 0x + 40 hex chars, or an avatar name the catalyst can resolve"),
    )
    .into())
}

/// Community ids are UUIDs on every social API this tool knows; anything
/// else is a name the caller forgot to resolve.
pub fn check_community_id(id: &str) -> Result<()> {
    if looks_like_uuid(id) {
        return Ok(());
    }
    Err(UserError::new(
        format!("\"{id}\" is not a community id"),
        TrySteps::one(
            "expect a UUID; `world access set --community <name>` resolves a name for you",
        ),
    )
    .into())
}

fn looks_like_uuid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            _ => c.is_ascii_hexdigit(),
        })
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Person {
    pub address: String,
    pub name: Option<String>,
    #[serde(default)]
    pub claimed: bool,
    pub face: Option<String>,
}

impl Person {
    pub fn label(&self) -> String {
        match &self.name {
            Some(n) if self.claimed => n.clone(),
            Some(n) => format!("{n} (unclaimed name)"),
            None => "(no profile)".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Community {
    pub id: String,
    pub name: String,
    pub members: Option<u64>,
    pub privacy: Option<String>,
    pub owner_name: Option<String>,
    /// The connected wallet's role in it, when the listing said.
    pub role: Option<String>,
    pub thumbnail: String,
}

impl Community {
    fn from_json(v: &Value) -> Option<Self> {
        let id = v.get("id").and_then(Value::as_str)?.to_string();
        Some(Community {
            name: v
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("(unnamed)")
                .to_string(),
            members: v.get("membersCount").and_then(Value::as_u64),
            privacy: v.get("privacy").and_then(Value::as_str).map(str::to_string),
            owner_name: v
                .get("ownerName")
                .and_then(Value::as_str)
                .map(str::to_string),
            role: v.get("role").and_then(Value::as_str).map(str::to_string),
            thumbnail: community_thumbnail(&id),
            id,
        })
    }
}

pub fn social_base() -> String {
    match std::env::var(SOCIAL_ENV) {
        Ok(v) if !v.trim().is_empty() => v.trim().trim_end_matches('/').to_string(),
        _ => DEFAULT_SOCIAL_API.to_string(),
    }
}

#[derive(Clone)]
pub struct Directory {
    catalyst: String,
    social: String,
    client: reqwest::Client,
}

impl Directory {
    pub fn from_env() -> Result<Self> {
        Self::new(&crate::start::proxy::catalyst_base(), &social_base())
    }

    pub fn new(catalyst: &str, social: &str) -> Result<Self> {
        Ok(Directory {
            catalyst: catalyst.trim_end_matches('/').to_string(),
            social: social.trim_end_matches('/').to_string(),
            client: crate::deploy::client(Duration::from_secs(10), Duration::from_secs(20))?,
        })
    }

    pub fn catalyst(&self) -> &str {
        &self.catalyst
    }

    pub fn social(&self) -> &str {
        &self.social
    }

    async fn get_json(&self, url: &str, headers: Vec<(String, String)>) -> Result<Option<Value>> {
        let (status, body) = send_text(with_headers(self.client.get(url), headers))
            .await
            .with_context(|| format!("GET {url}"))?;
        if status == 404 {
            return Ok(None);
        }
        anyhow::ensure!(
            (200..300).contains(&status),
            "GET {url} answered HTTP {status}: {}",
            body.trim()
        );
        Ok(Some(
            serde_json::from_str(&body).with_context(|| format!("parsing GET {url}"))?,
        ))
    }

    /// Never an error: a wallet without a profile is still a valid entry, it
    /// just has nothing to show beside the address.
    pub async fn profile(&self, address: &str) -> Person {
        let address = address.to_lowercase();
        let url = format!(
            "{}/lambdas/profiles/{}",
            self.catalyst,
            encode_segment(&address)
        );
        let mut person = Person {
            address: address.clone(),
            ..Default::default()
        };
        if let Ok(Some(v)) = self.get_json(&url, vec![]).await
            && let Some(a) = v.pointer("/avatars/0")
        {
            person.name = a
                .get("name")
                .and_then(Value::as_str)
                .filter(|n| !n.trim().is_empty())
                .map(str::to_string);
            person.claimed = a
                .get("hasClaimedName")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            person.face = a
                .pointer("/avatar/snapshots/face256")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        person
    }

    /// The wallet that owns a claimed name (`.dcl.eth` optional).
    pub async fn name_owner(&self, name: &str) -> Result<Option<String>> {
        let name = name.trim().trim_start_matches('@');
        let bare = name.strip_suffix(".dcl.eth").unwrap_or(name).to_lowercase();
        if bare.is_empty() {
            return Ok(None);
        }
        let url = format!(
            "{}/lambdas/names/{}/owner",
            self.catalyst,
            encode_segment(&bare)
        );
        Ok(self.get_json(&url, vec![]).await?.and_then(|v| {
            v.get("owner")
                .and_then(Value::as_str)
                .map(|s| s.to_lowercase())
        }))
    }

    /// An address or an avatar name, whichever was typed.
    pub async fn person(&self, query: &str) -> Result<Option<Person>> {
        let q = query.trim();
        if dcl_auth::is_eth_address(q) {
            return Ok(Some(self.profile(q).await));
        }
        match self.name_owner(q).await? {
            Some(address) => Ok(Some(self.profile(&address).await)),
            None => Ok(None),
        }
    }

    pub async fn community(&self, id: &str) -> Result<Option<Community>> {
        let url = format!("{}/v1/communities/{}", self.social, encode_segment(id));
        Ok(self
            .get_json(&url, vec![])
            .await?
            .and_then(|v| v.get("data").and_then(Community::from_json)))
    }

    pub async fn search_communities(&self, query: &str, limit: usize) -> Result<Vec<Community>> {
        let url = format!(
            "{}/v1/communities?limit={limit}&search={}",
            self.social,
            encode_segment(query.trim())
        );
        Ok(results(self.get_json(&url, vec![]).await?))
    }

    /// The communities a wallet belongs to. Unsigned, the social API lists
    /// the public ones; with that wallet's own signed-fetch headers it lists
    /// them all.
    pub async fn member_communities(
        &self,
        address: &str,
        headers: Vec<(String, String)>,
    ) -> Result<Vec<Community>> {
        let url = format!(
            "{}{}?limit=100",
            self.social,
            member_communities_path(address)
        );
        Ok(results(self.get_json(&url, headers).await?))
    }

    pub async fn community_members(&self, id: &str, offset: u32) -> Result<(Vec<Person>, u64)> {
        let url = format!(
            "{}/v1/communities/{}/members?limit=100&offset={offset}",
            self.social,
            encode_segment(id)
        );
        let Some(v) = self.get_json(&url, vec![]).await? else {
            return Ok((vec![], 0));
        };
        let total = v
            .pointer("/data/total")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let people = v
            .pointer("/data/results")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|m| {
                let address = m.get("memberAddress").and_then(Value::as_str)?;
                Some(Person {
                    address: address.to_lowercase(),
                    name: m.get("name").and_then(Value::as_str).map(str::to_string),
                    claimed: m
                        .get("hasClaimedName")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    face: m
                        .get("profilePictureUrl")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                })
            })
            .collect();
        Ok((people, total))
    }

    pub async fn people(&self, addresses: &[String]) -> Vec<Person> {
        futures::future::join_all(addresses.iter().map(|a| self.profile(a))).await
    }

    /// Ids the social API does not know stay as bare ids rather than vanish:
    /// the server will still enforce them.
    pub async fn communities(&self, ids: &[String]) -> Vec<Community> {
        futures::future::join_all(ids.iter().map(|id| async move {
            match self.community(id).await {
                Ok(Some(c)) => c,
                _ => Community {
                    id: id.clone(),
                    name: "(unknown community)".to_string(),
                    thumbnail: community_thumbnail(id),
                    ..Default::default()
                },
            }
        }))
        .await
    }
}

fn member_communities_path(address: &str) -> String {
    format!("/v1/members/{}/communities", address.to_lowercase())
}

fn results(v: Option<Value>) -> Vec<Community> {
    v.as_ref()
        .and_then(|v| v.pointer("/data/results"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Community::from_json)
        .collect()
}

/// `--user` values: addresses pass through, names resolve through the catalyst.
pub async fn resolve_people(dir: &Directory, queries: &[String]) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for q in queries {
        out.push(resolve_person(dir, q).await?);
    }
    Ok(out)
}

pub async fn resolve_person(dir: &Directory, query: &str) -> Result<String> {
    let q = query.trim();
    if dcl_auth::is_eth_address(q) {
        return Ok(q.to_lowercase());
    }
    match dir.name_owner(q).await? {
        Some(address) => {
            ux::note(format!("{q} → {address}"));
            Ok(address)
        }
        None => Err(UserError::new(
            format!("no avatar named \"{q}\""),
            TrySteps::one("check the spelling, or pass the wallet address (0x…) instead").and(
                format!(
                    "names are resolved by {} — DCL_SCENE_CLI_CATALYST points elsewhere",
                    dir.catalyst()
                ),
            ),
        )
        .into()),
    }
}

/// `--community` values: ids pass through, names must match one community.
pub async fn resolve_communities(dir: &Directory, queries: &[String]) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for q in queries {
        out.push(resolve_community(dir, q).await?);
    }
    Ok(out)
}

pub async fn resolve_community(dir: &Directory, query: &str) -> Result<String> {
    let q = query.trim();
    if looks_like_uuid(q) {
        return Ok(q.to_lowercase());
    }
    let found = dir.search_communities(q, 20).await?;
    let exact: Vec<&Community> = found
        .iter()
        .filter(|c| c.name.eq_ignore_ascii_case(q))
        .collect();
    let pick = match (exact.len(), found.len()) {
        (1, _) => exact[0],
        (0, 1) => &found[0],
        (0, 0) => {
            return Err(UserError::new(
                format!("no community named \"{q}\""),
                TrySteps::one("check the spelling, or pass the community id (a UUID)").and(
                    format!(
                        "communities are looked up on {} — {SOCIAL_ENV} points elsewhere",
                        dir.social()
                    ),
                ),
            )
            .into());
        }
        _ => {
            let mut steps = TrySteps::one("pass the id of the one you mean:");
            for c in found.iter().take(8) {
                steps = steps.and(format!(
                    "  {}  {}{}",
                    c.id,
                    c.name,
                    c.members
                        .map(|m| format!(" ({m} members)"))
                        .unwrap_or_default()
                ));
            }
            return Err(UserError::new(
                format!("\"{q}\" matches {} communities", found.len()),
                steps,
            )
            .into());
        }
    };
    ux::note(format!("{} → {} ({})", q, pick.name, pick.id));
    Ok(pick.id.to_lowercase())
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct AccessView {
    pub kind: String,
    pub people: Vec<Person>,
    pub communities: Vec<Community>,
}

pub async fn describe(dir: &Directory, permissions: &Value) -> AccessView {
    let spec = AccessSpec::from_permissions(permissions);
    let (people, communities) = match &spec {
        AccessSpec::AllowList {
            wallets,
            communities,
        } => (
            dir.people(wallets).await,
            dir.communities(communities).await,
        ),
        _ => (vec![], vec![]),
    };
    AccessView {
        kind: spec.kind().to_string(),
        people,
        communities,
    }
}

pub fn render_view(name: &str, owner: Option<&Person>, view: &AccessView) -> String {
    let mut lines = vec![format!("world: {name}")];
    match owner {
        Some(o) => lines.push(format!("owner: {}  {}", o.address, o.label())),
        None => lines.push("owner: (unknown)".to_string()),
    }
    match view.kind.as_str() {
        "unrestricted" => lines.push("access: anyone can enter".to_string()),
        "shared-secret" => {
            lines.push("access: password (the secret is not readable back)".to_string())
        }
        "nft-ownership" => lines.push("access: nft-ownership".to_string()),
        _ => {
            lines.push(format!(
                "access: only {} and the members of {} (the owner always can)",
                plural(view.people.len(), "person", "people"),
                plural(view.communities.len(), "community", "communities")
            ));
            if !view.people.is_empty() {
                lines.push("people:".to_string());
                for p in &view.people {
                    lines.push(format!("  - {}  {}", p.address, p.label()));
                }
            }
            if !view.communities.is_empty() {
                lines.push("communities:".to_string());
                for c in &view.communities {
                    lines.push(format!(
                        "  - {}  {}{}",
                        c.id,
                        c.name,
                        c.members
                            .map(|m| format!(" ({m} members)"))
                            .unwrap_or_default()
                    ));
                }
            }
        }
    }
    lines.join("\n")
}

async fn fetch_permissions(base: &str, name: &str) -> Result<Value> {
    let url = format!("{base}/world/{}/permissions", encode_segment(name));
    let (status, body) = send_text(
        crate::deploy::client(Duration::from_secs(30), Duration::from_secs(30))?.get(&url),
    )
    .await
    .map_err(|e| {
        UserError::new(
            "could not reach the worlds server",
            TrySteps::one("check the server is running and the URL is right")
                .and("pass --target-server <worlds-content-server-url>"),
        )
        .why(format!("request failed: {url}"))
        .caused_by(e)
    })?;
    if !(200..300).contains(&status) {
        return Err(refusal(
            UserError::new(
                format!(
                    "the worlds server refused to list the permissions of {name} (HTTP {status})"
                ),
                read_server_message(),
            ),
            &body,
        ));
    }
    serde_json::from_str(&body).context("parsing the permissions response")
}

pub async fn show(name: &str, target_content: Option<&str>) -> Result<()> {
    let base = world::resolve_target(target_content)?;
    let dir = Directory::from_env()?;
    let perms = fetch_permissions(&base, name).await?;
    let owner = match perms.get("owner").and_then(Value::as_str) {
        Some(o) => Some(dir.profile(o).await),
        None => None,
    };
    let view = describe(&dir, &perms).await;
    let mut steps = ux::Steps::new(1);
    println!("{}", render_view(name, owner.as_ref(), &view));
    steps.done(format!("Access fetched for {name}"));
    Ok(())
}

struct EditorState {
    base: String,
    name: String,
    dir: Directory,
    /// Payloads minted by /api/prepare, keyed to the setting they sign.
    pending: Mutex<HashMap<String, AccessSpec>>,
    /// Payloads minted for a signed read of the wallet's own communities.
    reads: Mutex<HashSet<String>>,
    done: Mutex<Option<DoneSender>>,
}

fn editor_state(
    base: String,
    name: String,
    dir: Directory,
) -> (
    Arc<EditorState>,
    tokio::sync::oneshot::Receiver<Result<String>>,
) {
    let (tx, rx) = tokio::sync::oneshot::channel();
    (
        Arc::new(EditorState {
            base,
            name,
            dir,
            pending: Mutex::new(HashMap::new()),
            reads: Mutex::new(HashSet::new()),
            done: Mutex::new(Some(tx)),
        }),
        rx,
    )
}

fn editor_router(state: Arc<EditorState>) -> Router {
    Router::new()
        .route("/", get(|| async { Html(PAGE) }))
        .route("/api/state", get(api_state))
        .route("/api/lookup", get(api_lookup))
        .route("/api/communities/search", get(api_search))
        .route("/api/communities/mine", get(api_mine))
        .route("/api/communities/mine/payload", get(api_mine_payload))
        .route("/api/communities/{id}/members", get(api_members))
        .route("/api/prepare", post(api_prepare))
        .route("/api/sign", post(api_sign))
        .with_state(state)
}

fn api_error(e: impl std::fmt::Display) -> Json<Value> {
    Json(json!({ "error": e.to_string() }))
}

async fn api_state(State(st): State<Arc<EditorState>>) -> Json<Value> {
    let perms = match fetch_permissions(&st.base, &st.name).await {
        Ok(p) => p,
        Err(e) => return api_error(format!("{e:#}")),
    };
    let owner_address = perms
        .get("owner")
        .and_then(Value::as_str)
        .map(|s| s.to_lowercase());
    let owner = match &owner_address {
        Some(o) => Some(st.dir.profile(o).await),
        None => None,
    };
    let view = describe(&st.dir, &perms).await;
    Json(json!({
        "world": st.name,
        "server": st.base,
        "catalyst": st.dir.catalyst(),
        "social": st.dir.social(),
        "owner": owner,
        "access": view,
        "limits": { "wallets": MAX_WALLETS, "communities": MAX_COMMUNITIES },
    }))
}

#[derive(Deserialize)]
struct LookupQuery {
    #[serde(default)]
    q: String,
}

async fn api_lookup(
    State(st): State<Arc<EditorState>>,
    Query(q): Query<LookupQuery>,
) -> Json<Value> {
    if q.q.trim().is_empty() || q.q.len() > 200 {
        return api_error("type an avatar name or a 0x address");
    }
    match st.dir.person(&q.q).await {
        Ok(Some(p)) => Json(json!({ "person": p })),
        Ok(None) => api_error(format!("no avatar named \"{}\"", q.q.trim())),
        Err(e) => api_error(format!("lookup failed: {e:#}")),
    }
}

async fn api_search(
    State(st): State<Arc<EditorState>>,
    Query(q): Query<LookupQuery>,
) -> Json<Value> {
    if q.q.len() > 200 {
        return api_error("search text too long");
    }
    match st.dir.search_communities(&q.q, 12).await {
        Ok(c) => Json(json!({ "communities": c })),
        Err(e) => api_error(format!("search failed: {e:#}")),
    }
}

#[derive(Deserialize)]
struct MineQuery {
    address: String,
    payload: Option<String>,
    signature: Option<String>,
}

async fn api_mine_payload(
    State(st): State<Arc<EditorState>>,
    Query(q): Query<MineQuery>,
) -> Json<Value> {
    if !dcl_auth::is_eth_address(&q.address) {
        return api_error("not an address");
    }
    let payload =
        signed_fetch_payload_meta("get", &member_communities_path(&q.address), now_ms(), "{}");
    let mut reads = st.reads.lock().unwrap_or_else(PoisonError::into_inner);
    if reads.len() > 32 {
        reads.clear();
    }
    reads.insert(payload.clone());
    Json(json!({ "payload": payload }))
}

async fn api_mine(State(st): State<Arc<EditorState>>, Query(q): Query<MineQuery>) -> Json<Value> {
    if !dcl_auth::is_eth_address(&q.address) {
        return api_error("not an address");
    }
    let headers = match (&q.payload, &q.signature) {
        (Some(payload), Some(signature)) => {
            let known = st
                .reads
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(payload);
            if !known {
                return api_error("stale read request — try again");
            }
            world::browser_headers(&q.address, payload, signature)
        }
        _ => vec![],
    };
    let signed = !headers.is_empty();
    match st.dir.member_communities(&q.address, headers).await {
        Ok(c) => Json(json!({ "communities": c, "signed": signed })),
        Err(e) => api_error(format!("could not list communities: {e:#}")),
    }
}

#[derive(Deserialize)]
struct MembersQuery {
    #[serde(default)]
    offset: u32,
}

async fn api_members(
    State(st): State<Arc<EditorState>>,
    Path(id): Path<String>,
    Query(q): Query<MembersQuery>,
) -> Json<Value> {
    if check_community_id(&id).is_err() {
        return api_error("not a community id");
    }
    match st.dir.community_members(&id, q.offset).await {
        Ok((people, total)) => Json(json!({ "people": people, "total": total })),
        Err(e) => api_error(format!("could not list members: {e:#}")),
    }
}

#[derive(Deserialize)]
struct PrepareReq {
    kind: String,
    #[serde(default)]
    wallets: Vec<String>,
    #[serde(default)]
    communities: Vec<String>,
    #[serde(default)]
    secret: String,
}

async fn api_prepare(
    State(st): State<Arc<EditorState>>,
    Json(req): Json<PrepareReq>,
) -> Json<Value> {
    let spec = match req.kind.as_str() {
        "unrestricted" => AccessSpec::Unrestricted,
        "shared-secret" => AccessSpec::SharedSecret { secret: req.secret },
        "allow-list" => AccessSpec::allow_list(&req.wallets, &req.communities),
        other => return api_error(format!("unknown access type {other}")),
    };
    if let Err(e) = spec.validate() {
        return api_error(format!("{e:#}"));
    }
    let action = WorldAction::AccessSet(spec.clone());
    let path = action.path(&st.name);
    let payload = signed_fetch_payload_meta(action.method(), &path, now_ms(), &action.metadata());
    {
        let mut pending = st.pending.lock().unwrap_or_else(PoisonError::into_inner);
        if pending.len() > 32 {
            pending.clear();
        }
        pending.insert(payload.clone(), spec.clone());
    }
    Json(json!({
        "payload": payload,
        "method": action.method().to_uppercase(),
        "path": path,
        "summary": spec.summary(),
        "details": spec.details(),
    }))
}

#[derive(Deserialize)]
struct SignReq {
    address: String,
    signature: String,
    payload: String,
}

async fn api_sign(State(st): State<Arc<EditorState>>, Json(req): Json<SignReq>) -> Json<Value> {
    let spec = st
        .pending
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&req.payload);
    let Some(spec) = spec else {
        return Json(crate::world_linker::stale_payload());
    };
    let action = WorldAction::AccessSet(spec);
    Json(
        crate::world_linker::submit(
            &st.done,
            &st.base,
            &st.name,
            &action,
            &req.address,
            &req.payload,
            &req.signature,
        )
        .await,
    )
}

pub async fn edit(name: &str, target_content: Option<&str>, browser: BrowserOptions) -> Result<()> {
    let base = world::resolve_target(target_content)?;
    let dir = Directory::from_env()?;
    let (state, rx) = editor_state(base, name.to_string(), dir);
    let app = editor_router(state);
    let opts = LinkerOptions {
        port: browser.port,
        open_browser: !browser.no_browser && !browser.ci,
        timeout: crate::linker::linker_timeout(),
        host: None,
    };
    let message =
        crate::world_linker::serve(app, rx, name, "change who can enter the world", opts).await?;
    ux::Steps::new(1).done(message);
    Ok(())
}

const PAGE: &str = r##"<!doctype html>
<html lang="en"><head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>dcl-scene-cli world — who can enter</title>
<style>
  :root{color-scheme:dark;--bg:#0b0e16;--card:#12151e;--line:#24314d;--ink:#e6ebf2;--dim:#8ea1c0;--soft:#b9c6dd;--acc:#22e6d0;--acc-ink:#04121a;--well:#0e1220}
  body{margin:0;background:var(--bg);color:var(--ink);font:15px/1.5 system-ui,sans-serif;display:flex;min-height:100vh;align-items:flex-start;justify-content:center}
  .card{width:min(820px,94vw);background:var(--card);border:1px solid var(--line);border-radius:16px;padding:26px 28px;box-shadow:0 20px 60px #0008;margin:24px 0}
  h1{margin:0 0 4px;font-size:20px}
  h1 .t{color:var(--acc)}
  h2{margin:22px 0 8px;font-size:13px;letter-spacing:.06em;text-transform:uppercase;color:var(--dim);font-weight:600}
  .sub{color:var(--dim);margin:0 0 14px;font-size:13px}
  code{font-family:ui-monospace,monospace;word-break:break-all;color:#cfe;font-size:12px}
  .now{background:var(--well);border:1px solid var(--line);border-radius:10px;padding:10px 14px;font-size:13px;color:var(--soft)}
  .me{display:flex;align-items:center;gap:12px;margin-top:12px;padding:10px 14px;background:var(--well);border:1px solid var(--line);border-radius:10px}
  .me .who{flex:1;min-width:0}
  .me .who b{display:block}
  .face{width:40px;height:40px;border-radius:50%;background:#1a2032;object-fit:cover;flex:none}
  .thumb{width:40px;height:40px;border-radius:8px;background:#1a2032;object-fit:cover;flex:none}
  .badge{font-size:11px;padding:2px 8px;border-radius:999px;border:1px solid var(--line);color:var(--soft);white-space:nowrap}
  .badge.ok{border-color:#1f6b46;color:#8ff0bf}
  .badge.warn{border-color:#6b4a26;color:#ffcf8a}
  .modes{display:grid;gap:8px;margin-top:6px}
  .mode{display:flex;gap:12px;align-items:flex-start;padding:12px 14px;border:1px solid var(--line);border-radius:10px;cursor:pointer;background:var(--well)}
  .mode:has(input:checked){border-color:var(--acc);box-shadow:inset 0 0 0 1px var(--acc)}
  .mode input{margin-top:4px}
  .mode b{display:block}
  .mode span{font-size:13px;color:var(--soft)}
  .panel{display:none;margin-top:6px}
  .panel.on{display:block}
  .row{display:flex;gap:8px;margin-top:8px}
  input[type=text],input[type=password]{flex:1;padding:10px 12px;border-radius:8px;border:1px solid var(--line);background:#0b0e16;color:var(--ink);font:inherit}
  input:focus{outline:2px solid var(--acc);outline-offset:-1px}
  .list{display:flex;flex-direction:column;gap:6px;margin-top:8px}
  .item{display:flex;align-items:center;gap:10px;padding:8px 10px;border:1px solid var(--line);border-radius:10px;background:var(--well)}
  .item .who{flex:1;min-width:0}
  .item .who b{display:block;font-size:14px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .item .who small{color:var(--dim);font-size:12px}
  .item button{margin:0;width:auto;padding:6px 10px;font-size:12px}
  .item button.x{background:transparent;color:var(--dim);border:1px solid var(--line)}
  .item button.x:hover{color:#ff9a9a;border-color:#6b2626}
  .empty{color:var(--dim);font-size:13px;padding:8px 2px}
  .chips{display:flex;flex-wrap:wrap;gap:6px;margin-top:8px}
  .chip{display:inline-flex;align-items:center;gap:6px;padding:4px 10px 4px 4px;border-radius:999px;border:1px solid var(--line);background:var(--well);cursor:pointer;font-size:13px}
  .chip img{width:22px;height:22px;border-radius:50%}
  .chip:hover{border-color:var(--acc)}
  .chip.in{opacity:.45;cursor:default}
  button{margin-top:18px;width:100%;padding:13px;border:0;border-radius:10px;cursor:pointer;font-size:15px;font-weight:600;background:var(--acc);color:var(--acc-ink)}
  button.ghost{background:transparent;color:var(--acc);border:1px solid var(--line);font-weight:500;font-size:13px;padding:8px 12px;width:auto;margin-top:8px}
  button:disabled{opacity:.5;cursor:default}
  #status{margin-top:16px;padding:12px 14px;border-radius:10px;font-size:13px;white-space:pre-wrap;display:none}
  .ok{background:#0e2a1e;border:1px solid #1f6b46;color:#8ff0bf}
  .err{background:#2a1414;border:1px solid #6b2626;color:#ff9a9a}
  .info{background:#101a2c;border:1px solid var(--line);color:#a9c0e6}
  .summary{margin-top:18px;padding:12px 14px;border-radius:10px;background:var(--well);border:1px solid var(--line);font-size:14px}
  details{margin-top:8px}
  details summary{cursor:pointer;color:var(--dim);font-size:13px}
  .hint{font-size:12px;color:var(--dim);margin-top:6px}
  a{color:var(--acc)}
</style></head>
<body><div class="card">
  <h1>Who can enter <span class="t" id="world">…</span></h1>
  <p class="sub">Change the access rule of this world and sign it with the wallet that owns it. The command line waits until you finish here.</p>
  <div class="now" id="now">Loading the current rule…</div>

  <div class="me" id="me" style="display:none">
    <img class="face" id="me-face" alt=""/>
    <div class="who"><b id="me-name">…</b><code id="me-addr"></code></div>
    <span class="badge" id="me-badge"></span>
  </div>
  <button class="ghost" id="connect">Connect wallet</button>

  <h2>Rule</h2>
  <div class="modes">
    <label class="mode"><input type="radio" name="kind" value="unrestricted"/><div><b>Anyone</b><span>Everyone with the link can walk in.</span></div></label>
    <label class="mode"><input type="radio" name="kind" value="allow-list"/><div><b>Only people and communities on a list</b><span>Named avatars, wallet addresses, and every member of the communities you choose. You, the owner, always get in.</span></div></label>
    <label class="mode"><input type="radio" name="kind" value="shared-secret"/><div><b>Anyone with a password</b><span>Visitors type it at the door. The current one cannot be read back, only replaced.</span></div></label>
  </div>

  <div class="panel" id="p-list">
    <h2>People</h2>
    <div class="list" id="people"></div>
    <div class="row"><input type="text" id="who" placeholder="Add by avatar name or 0x address, then Enter" autocomplete="off"/><button class="ghost" id="who-go" style="margin-top:0">Add</button></div>
    <div class="hint" id="who-hint"></div>

    <h2>Communities</h2>
    <div class="list" id="communities"></div>
    <div class="row"><input type="text" id="cq" placeholder="Find a community by name" autocomplete="off"/></div>
    <div class="chips" id="cq-out"></div>
    <div id="mine-wrap" style="display:none">
      <div class="hint" id="mine-title">Communities you belong to</div>
      <div class="chips" id="mine"></div>
      <button class="ghost" id="mine-all">Include private ones (signs a read request)</button>
    </div>
  </div>

  <div class="panel" id="p-secret">
    <div class="row"><input type="password" id="secret" placeholder="New password" autocomplete="new-password"/></div>
    <div class="row"><input type="password" id="secret2" placeholder="Repeat it" autocomplete="new-password"/></div>
    <div class="hint">Share it with visitors out of band; the server keeps only a hash.</div>
  </div>

  <div class="summary" id="summary">…</div>
  <button id="go" disabled>Connect wallet &amp; sign</button>
  <div id="status"></div>
</div>
<script>
const $=id=>document.getElementById(id);
const S={info:null,kind:"unrestricted",people:new Map(),communities:new Map(),me:null,isOwner:false,mine:[],limits:{wallets:1000,communities:50}};
function show(cls,msg){const s=$("status");s.className=cls;s.style.display="block";s.textContent=msg;}
function hide(){$("status").style.display="none";}
function short(a){return a.slice(0,6)+"…"+a.slice(-4);}
function el(tag,cls,text){const e=document.createElement(tag);if(cls)e.className=cls;if(text!=null)e.textContent=text;return e;}
function img(cls,src){const i=el("img",cls);i.alt="";i.loading="lazy";if(src){i.src=src;i.onerror=()=>{i.style.visibility="hidden";};}else if(!cls){i.style.display="none";}return i;}
function personLabel(p){return p.name?(p.claimed?p.name:p.name+" (unclaimed)"):"(no profile)";}
function describeNow(a){
  if(!a)return "";
  if(a.kind==="unrestricted")return "Right now: anyone can enter.";
  if(a.kind==="shared-secret")return "Right now: a password is required.";
  if(a.kind==="allow-list")return "Right now: only "+a.people.length+" "+(a.people.length===1?"person":"people")+" and the members of "+a.communities.length+" "+(a.communities.length===1?"community":"communities")+".";
  return "Right now: "+a.kind+".";
}
function renderPeople(){
  const box=$("people");box.replaceChildren();
  if(!S.people.size){box.append(el("div","empty","Nobody yet — add avatars below, or pick members of a community."));return;}
  for(const p of S.people.values()){
    const it=el("div","item");it.append(img("face",p.face));
    const who=el("div","who");who.append(el("b",null,personLabel(p)));who.append(el("small",null,p.address));it.append(who);
    const x=el("button","x","Remove");x.onclick=()=>{S.people.delete(p.address);render();};it.append(x);box.append(it);
  }
}
function renderCommunities(){
  const box=$("communities");box.replaceChildren();
  if(!S.communities.size){box.append(el("div","empty","No communities yet — search below, or pick one of yours."));}
  for(const c of S.communities.values()){
    const it=el("div","item");it.append(img("thumb",c.thumbnail));
    const who=el("div","who");who.append(el("b",null,c.name));
    who.append(el("small",null,[c.members!=null?c.members+" members":null,c.privacy,c.owner_name?"by "+c.owner_name:null].filter(Boolean).join(" · ")));it.append(who);
    const pick=el("button","ghost","Pick members…");pick.style.marginTop="0";pick.onclick=()=>pickMembers(c,it);it.append(pick);
    const x=el("button","x","Remove");x.onclick=()=>{S.communities.delete(c.id);render();};it.append(x);box.append(it);
  }
  renderMine();
}
function chip(c,onclick){
  const ch=el("span","chip"+(S.communities.has(c.id)?" in":""));ch.append(img("",c.thumbnail));
  ch.append(document.createTextNode(c.name+(c.members!=null?" · "+c.members:"")+(c.role?" · "+c.role:"")));
  if(!S.communities.has(c.id))ch.onclick=onclick;return ch;
}
function renderMine(){
  const box=$("mine");box.replaceChildren();
  $("mine-wrap").style.display=S.me?"block":"none";
  if(!S.mine.length){box.append(el("div","empty",S.mineSigned?"You belong to no communities.":"No public communities on this wallet."));return;}
  for(const c of S.mine)box.append(chip(c,()=>{S.communities.set(c.id,c);render();}));
}
async function pickMembers(c,anchor){
  let list=anchor.nextSibling;
  if(list&&list.dataset&&list.dataset.members===c.id){list.remove();return;}
  list=el("div","chips");list.dataset.members=c.id;list.append(el("div","empty","Loading members…"));anchor.after(list);
  const r=await (await fetch("api/communities/"+encodeURIComponent(c.id)+"/members")).json();
  list.replaceChildren();
  if(r.error){list.append(el("div","empty",r.error));return;}
  if(!r.people.length){list.append(el("div","empty","No members listed."));return;}
  for(const p of r.people){
    const ch=el("span","chip"+(S.people.has(p.address)?" in":""));ch.append(img("",p.face));ch.append(document.createTextNode(personLabel(p)+(p.name?"":" "+short(p.address))));
    if(!S.people.has(p.address))ch.onclick=()=>{S.people.set(p.address,p);render();};
    list.append(ch);
  }
  if(r.total>r.people.length)list.append(el("div","empty","Showing "+r.people.length+" of "+r.total+". Allow the whole community instead of listing everyone."));
}
function summary(){
  if(S.kind==="unrestricted")return "Anyone will be able to enter "+S.info.world+".";
  if(S.kind==="shared-secret")return "Visitors will need the password to enter "+S.info.world+".";
  const p=S.people.size,c=S.communities.size;
  if(!p&&!c)return "Nobody but you would be able to enter "+S.info.world+". Add at least one person or community, or choose another rule.";
  return "Only "+p+" "+(p===1?"person":"people")+" and the members of "+c+" "+(c===1?"community":"communities")+" will be able to enter "+S.info.world+". You always can.";
}
function render(){
  $("p-list").classList.toggle("on",S.kind==="allow-list");
  $("p-secret").classList.toggle("on",S.kind==="shared-secret");
  renderPeople();renderCommunities();
  $("summary").textContent=S.info?summary():"…";
  const over=S.people.size>S.limits.wallets||S.communities.size>S.limits.communities;
  $("go").disabled=!S.info||over;
  $("go").textContent=S.me?"Sign & save":"Connect wallet & sign";
}
function spec(){
  if(S.kind==="allow-list")return {kind:S.kind,wallets:[...S.people.keys()],communities:[...S.communities.keys()]};
  if(S.kind==="shared-secret")return {kind:S.kind,secret:$("secret").value};
  return {kind:S.kind};
}
async function load(){
  try{
    const info=await (await fetch("api/state")).json();
    if(info.error){show("err",info.error);$("now").textContent="Could not read the current rule.";return;}
    S.info=info;S.limits=info.limits||S.limits;
    $("world").textContent=info.world;
    $("now").textContent=describeNow(info.access)+"  ·  "+info.server;
    S.kind=["unrestricted","allow-list","shared-secret"].includes(info.access.kind)?info.access.kind:"allow-list";
    for(const p of info.access.people)S.people.set(p.address,p);
    for(const c of info.access.communities)S.communities.set(c.id,c);
    document.querySelector('input[name=kind][value="'+S.kind+'"]').checked=true;
    render();
  }catch(e){show("err","Could not load the request: "+e);}
}
document.querySelectorAll('input[name=kind]').forEach(r=>r.onchange=()=>{S.kind=r.value;render();});
async function addPerson(){
  const q=$("who").value.trim();if(!q)return;
  $("who-hint").textContent="Looking up "+q+"…";
  const r=await (await fetch("api/lookup?q="+encodeURIComponent(q))).json();
  if(r.error){$("who-hint").textContent=r.error;return;}
  S.people.set(r.person.address,r.person);$("who").value="";$("who-hint").textContent="Added "+personLabel(r.person)+" ("+short(r.person.address)+").";render();
}
$("who-go").onclick=addPerson;
$("who").addEventListener("keydown",e=>{if(e.key==="Enter"){e.preventDefault();addPerson();}});
let cqTimer=null;
$("cq").addEventListener("input",()=>{
  clearTimeout(cqTimer);const q=$("cq").value.trim();
  if(q.length<2){$("cq-out").replaceChildren();return;}
  cqTimer=setTimeout(async()=>{
    const r=await (await fetch("api/communities/search?q="+encodeURIComponent(q))).json();
    const out=$("cq-out");out.replaceChildren();
    if(r.error){out.append(el("div","empty",r.error));return;}
    if(!r.communities.length){out.append(el("div","empty","No community matches "+q+"."));return;}
    for(const c of r.communities)out.append(chip(c,()=>{S.communities.set(c.id,c);$("cq").value="";out.replaceChildren();render();}));
  },250);
});
async function loadMine(signed){
  let url="api/communities/mine?address="+S.me.address;
  if(signed){
    const {payload}=await (await fetch("api/communities/mine/payload?address="+S.me.address)).json();
    const signature=await window.ethereum.request({method:"personal_sign",params:[payload,S.me.address]});
    url+="&payload="+encodeURIComponent(payload)+"&signature="+encodeURIComponent(signature);
  }
  const r=await (await fetch(url)).json();
  if(r.error){$("mine-title").textContent=r.error;return;}
  S.mine=r.communities;S.mineSigned=r.signed;
  $("mine-title").textContent=r.signed?"All communities you belong to":"Public communities you belong to";
  $("mine-all").style.display=r.signed?"none":"inline-block";
  renderMine();
}
$("mine-all").onclick=async()=>{try{await loadMine(true);}catch(e){show("err","✗ "+(e&&e.message?e.message:e));}};
async function connect(){
  if(!window.ethereum){show("err","No wallet found. Open this page in a browser with MetaMask (or another EIP-1193 wallet).");return null;}
  const accounts=await window.ethereum.request({method:"eth_requestAccounts"});
  const address=accounts[0].toLowerCase();
  if(S.me&&S.me.address===address)return S.me;
  show("info","Reading the profile of "+short(address)+"…");
  const r=await (await fetch("api/lookup?q="+address)).json();
  S.me=r.person||{address,name:null,claimed:false,face:null};
  S.isOwner=!!(S.info&&S.info.owner&&S.info.owner.address===address);
  $("me").style.display="flex";$("connect").style.display="none";
  $("me-face").src=S.me.face||"";$("me-name").textContent=personLabel(S.me);$("me-addr").textContent=address;
  const b=$("me-badge");
  if(S.isOwner){b.className="badge ok";b.textContent="owner of "+S.info.world;}
  else if(S.info&&S.info.owner){b.className="badge warn";b.textContent="not the owner — "+(S.info.owner.name||short(S.info.owner.address))+" is";}
  else{b.className="badge";b.textContent="owner unknown";}
  hide();render();
  loadMine(false).catch(()=>{});
  return S.me;
}
$("connect").onclick=()=>connect().catch(e=>show("err","✗ "+(e&&e.message?e.message:e)));
// The server accepts a signed payload for only ~60s, and a wallet prompt can
// easily outlast that: mint and sign back-to-back, and re-mint on expiry.
const MAX_SIGN_ATTEMPTS=3;
async function attempt(address,left){
  const prep=await (await fetch("api/prepare",{method:"POST",headers:{"content-type":"application/json"},body:JSON.stringify(spec())})).json();
  if(prep.error){show("err","✗ "+prep.error);return false;}
  show("info","Signing with "+address+" to "+prep.summary+" … (approve promptly — the signature is valid for about a minute)");
  const signature=await window.ethereum.request({method:"personal_sign",params:[prep.payload,address]});
  show("info","Sending to "+S.info.server+" …");
  const r=await (await fetch("api/sign",{method:"POST",headers:{"content-type":"application/json"},body:JSON.stringify({address,signature,payload:prep.payload})})).json();
  if(r.ok){show("ok","✓ "+r.message+"\n\nYou can close this tab; the command line has finished.");$("now").textContent="Saved: "+summary();return true;}
  if(r.expired&&left>0){show("info","That signature expired before it landed. Signing again with a fresh timestamp — please approve as soon as the prompt appears.");return attempt(address,left-1);}
  if(r.expired){show("err","✗ "+r.error+"\n\nGave up after "+MAX_SIGN_ATTEMPTS+" attempts. Press the button and approve the wallet prompt immediately.");return false;}
  if(r.fatal){show("err","✗ "+r.error+"\n\nThe command line exited with this error — fix it and re-run.");return false;}
  show("err","✗ "+r.error+"\n\nSwitch to the wallet that owns this world and press the button again.");
  return false;
}
$("go").onclick=async()=>{
  const btn=$("go");
  try{
    if(S.kind==="shared-secret"){
      if(!$("secret").value){show("err","Type the new password first.");return;}
      if($("secret").value!==$("secret2").value){show("err","The two passwords differ.");return;}
    }
    if(S.kind==="allow-list"&&!S.people.size&&!S.communities.size&&!confirm("The list is empty: nobody but you could enter. Save anyway?"))return;
    btn.disabled=true;
    const me=await connect();if(!me){btn.disabled=false;return;}
    const ok=await attempt(me.address,MAX_SIGN_ATTEMPTS-1);
    if(!ok)btn.disabled=false;
  }catch(e){show("err","✗ "+(e&&e.message?e.message:e));btn.disabled=false;}
};
load();
</script></body></html>
"##;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;
    use axum::routing::get as aget;

    fn list(w: &[&str], c: &[&str]) -> AccessSpec {
        AccessSpec::allow_list(
            &w.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            &c.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        )
    }

    const A: &str = "0xAAAA111111111111111111111111111111111111";
    const B: &str = "0xbbbb222222222222222222222222222222222222";
    const CID: &str = "E99471AA-31C4-4952-ABF6-99905445F43B";

    #[test]
    fn allow_list_is_canonical_and_lowercase() {
        let spec = list(&[A, " ", B, A], &[CID, CID]);
        assert_eq!(
            spec,
            AccessSpec::AllowList {
                wallets: vec![A.to_lowercase(), B.to_string()],
                communities: vec![CID.to_lowercase()],
            }
        );
        let meta = spec.metadata_json();
        assert_eq!(meta, meta.to_lowercase());
        let v: Value = serde_json::from_str(&meta).unwrap();
        assert_eq!(v["type"], "allow-list");
        assert_eq!(v["wallets"].as_array().unwrap().len(), 2);
        assert_eq!(v["communities"][0], CID.to_lowercase());
        assert!(spec.validate().is_ok());
    }

    #[test]
    fn validation_mirrors_the_server_caps_and_shapes() {
        assert!(list(&["0x123"], &[]).validate().is_err());
        assert!(list(&[], &["Decentraland Foundation"]).validate().is_err());
        assert!(
            AccessSpec::SharedSecret {
                secret: "  ".into()
            }
            .validate()
            .is_err()
        );
        let many: Vec<String> = (0..=MAX_COMMUNITIES)
            .map(|i| format!("{i:08x}-0000-4000-8000-000000000000"))
            .collect();
        assert!(AccessSpec::allow_list(&[], &many).validate().is_err());
        assert!(AccessSpec::Unrestricted.validate().is_ok());
        assert!(check_community_id("e99471aa-31c4-4952-abf6-99905445f43b").is_ok());
    }

    #[test]
    fn the_action_posts_the_setting_as_metadata() {
        let action = WorldAction::AccessSet(list(&[A], &[CID]));
        assert_eq!(action.method(), "post");
        assert_eq!(
            action.path("My-World.dcl.eth"),
            "/world/My-World.dcl.eth/permissions/access"
        );
        assert_eq!(action.metadata(), list(&[A], &[CID]).metadata_json());
        assert!(action.validate().is_ok());
        assert_eq!(
            action.summary(),
            "restrict entry to 1 person and 1 community"
        );

        let community = WorldAction::AccessCommunity {
            id: CID.to_string(),
            revoke: false,
        };
        assert_eq!(community.method(), "put");
        assert_eq!(
            community.path("w.dcl.eth"),
            format!(
                "/world/w.dcl.eth/permissions/access/communities/{}",
                CID.to_lowercase()
            )
        );
        assert_eq!(community.metadata(), "{}");
        assert!(
            WorldAction::AccessCommunity {
                id: "nope".into(),
                revoke: true
            }
            .validate()
            .is_err()
        );
    }

    #[tokio::test]
    async fn metadata_survives_signing_and_the_shared_validator_reads_it_back() {
        use crate::signed_fetch::verify_signed_fetch_meta;

        let signer = crate::random_test_wallet();
        let action = WorldAction::AccessSet(list(&[A, B], &[CID]));
        let path = action.path("Test.dcl.eth");
        let headers =
            world::signed_headers_meta(&signer, action.method(), &path, &action.metadata())
                .unwrap();
        let mut map = HeaderMap::new();
        for (k, v) in &headers {
            map.insert(
                axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                axum::http::HeaderValue::from_str(v).unwrap(),
            );
        }
        assert_eq!(
            map["x-identity-metadata"].to_str().unwrap(),
            action.metadata()
        );
        let (who, meta) = verify_signed_fetch_meta(&map, "post", &path, 300)
            .await
            .expect("metadata-carrying headers must verify");
        assert_eq!(who, signer.address().to_lowercase());
        assert_eq!(meta["type"], "allow-list");
        assert_eq!(meta["wallets"][1], B);
        assert_eq!(meta["communities"][0], CID.to_lowercase());

        // The browser path re-derives the same headers from the signed string.
        let payload = signed_fetch_payload_meta("post", &path, now_ms(), &action.metadata());
        let sig = signer.sign_message(payload.as_bytes()).unwrap();
        let browser = world::browser_headers(&signer.address(), &payload, &sig);
        let meta_header = browser
            .iter()
            .find(|(k, _)| k == "x-identity-metadata")
            .map(|(_, v)| v.clone())
            .unwrap();
        assert_eq!(meta_header, action.metadata());
    }

    #[test]
    fn a_password_keeps_its_case_in_the_signed_bytes() {
        let spec = AccessSpec::SharedSecret {
            secret: "Open Sesame 42".into(),
        };
        let payload = signed_fetch_payload_meta(
            "POST",
            "/world/W.dcl.eth/permissions/access",
            7,
            &spec.metadata_json(),
        );
        assert!(payload.starts_with("post:/world/w.dcl.eth/permissions/access:7:{"));
        assert!(payload.contains("Open Sesame 42"));
        let chain = json!([]);
        let headers = world::headers_from_chain(&payload, &chain);
        assert_eq!(
            headers[0],
            ("x-identity-timestamp".to_string(), "7".to_string())
        );
        assert_eq!(headers[1].1, spec.metadata_json());
    }

    #[test]
    fn the_current_setting_is_read_from_a_permissions_body() {
        let body = json!({
            "owner": "0xabc",
            "permissions": {
                "deployment": { "type": "allow-list", "wallets": [] },
                "streaming": { "type": "allow-list", "wallets": [] },
                "access": { "type": "allow-list", "wallets": [A], "communities": [CID] }
            }
        });
        assert_eq!(AccessSpec::from_permissions(&body), list(&[A], &[CID]));
        let secret = json!({ "permissions": { "access": { "type": "shared-secret" } } });
        assert!(matches!(
            AccessSpec::from_permissions(&secret),
            AccessSpec::SharedSecret { .. }
        ));
        assert_eq!(
            AccessSpec::from_permissions(&json!({})),
            AccessSpec::Unrestricted
        );
    }

    /// A catalyst and a social API in one router: profiles, a name owner,
    /// community search/read/members, and a member listing that only shows
    /// the private community to a signed request.
    async fn directory_fixture() -> (Directory, tokio::task::JoinHandle<()>) {
        let app = Router::new()
            .route(
                "/lambdas/profiles/{address}",
                aget(|Path(address): Path<String>| async move {
                    if address == A.to_lowercase() {
                        Json(json!({"avatars":[{"name":"Alice","hasClaimedName":true,
                            "avatar":{"snapshots":{"face256":"https://img/alice.png"}}}]}))
                        .into_response()
                    } else {
                        axum::http::StatusCode::NOT_FOUND.into_response()
                    }
                }),
            )
            .route(
                "/lambdas/names/{name}/owner",
                aget(|Path(name): Path<String>| async move {
                    if name == "alice" {
                        Json(json!({"owner": A})).into_response()
                    } else {
                        axum::http::StatusCode::NOT_FOUND.into_response()
                    }
                }),
            )
            .route(
                "/v1/communities",
                aget(|Query(q): Query<HashMap<String, String>>| async move {
                    let s = q.get("search").cloned().unwrap_or_default();
                    let all = vec![
                        json!({"id": CID.to_lowercase(), "name":"Decentraland Foundation","membersCount":1301,"privacy":"public","ownerName":"DCLOfficial"}),
                        json!({"id":"3ca51536-f8aa-4f94-b172-d8ff9a6abfe4","name":"ABC Decentraland","membersCount":40,"privacy":"public"}),
                    ];
                    let hits: Vec<Value> = all
                        .into_iter()
                        .filter(|c| c["name"].as_str().unwrap().to_lowercase().contains(&s.to_lowercase()))
                        .collect();
                    Json(json!({"data":{"results":hits}}))
                }),
            )
            .route(
                "/v1/communities/{id}",
                aget(|Path(id): Path<String>| async move {
                    if id == CID.to_lowercase() {
                        Json(json!({"data":{"id":id,"name":"Decentraland Foundation","membersCount":1301}})).into_response()
                    } else {
                        axum::http::StatusCode::NOT_FOUND.into_response()
                    }
                }),
            )
            .route(
                "/v1/communities/{id}/members",
                aget(|Path(_id): Path<String>| async move {
                    Json(json!({"data":{"results":[
                        {"memberAddress":A,"name":"Alice","hasClaimedName":true,"profilePictureUrl":"https://img/alice.png","role":"owner"},
                        {"memberAddress":B,"name":"bob#1234","hasClaimedName":false}
                    ],"total":2}}))
                }),
            )
            .route(
                "/v1/members/{address}/communities",
                aget(|headers: HeaderMap| async move {
                    let signed = headers.contains_key("x-identity-auth-chain-0");
                    let mut out = vec![json!({"id": CID.to_lowercase(), "name":"Decentraland Foundation","role":"member"})];
                    if signed {
                        out.push(json!({"id":"3ca51536-f8aa-4f94-b172-d8ff9a6abfe4","name":"Secret Club","role":"owner"}));
                    }
                    Json(json!({"data":{"results":out}}))
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (Directory::new(&base, &base).unwrap(), handle)
    }

    use axum::response::IntoResponse;

    #[tokio::test]
    async fn names_and_ids_resolve_both_ways() {
        let (dir, handle) = directory_fixture().await;

        let alice = dir.person("Alice.dcl.eth").await.unwrap().unwrap();
        assert_eq!(alice.address, A.to_lowercase());
        assert_eq!(alice.label(), "Alice");
        assert_eq!(alice.face.as_deref(), Some("https://img/alice.png"));
        assert!(dir.person("nobody").await.unwrap().is_none());
        let bob = dir.person(B).await.unwrap().unwrap();
        assert_eq!(bob.label(), "(no profile)");

        assert_eq!(
            resolve_person(&dir, "alice").await.unwrap(),
            A.to_lowercase()
        );
        assert!(resolve_person(&dir, "carol").await.is_err());
        assert_eq!(resolve_person(&dir, A).await.unwrap(), A.to_lowercase());

        assert_eq!(
            resolve_community(&dir, "decentraland foundation")
                .await
                .unwrap(),
            CID.to_lowercase()
        );
        assert_eq!(
            resolve_community(&dir, CID).await.unwrap(),
            CID.to_lowercase()
        );
        let ambiguous = resolve_community(&dir, "decentraland").await.unwrap_err();
        assert!(format!("{ambiguous:#}").contains("matches 2 communities"));
        assert!(resolve_community(&dir, "zzz").await.is_err());

        let (members, total) = dir.community_members(&CID.to_lowercase(), 0).await.unwrap();
        assert_eq!(total, 2);
        assert_eq!(members[1].label(), "bob#1234 (unclaimed name)");

        let public = dir.member_communities(A, vec![]).await.unwrap();
        assert_eq!(public.len(), 1);
        let signer = crate::random_test_wallet();
        let headers = world::signed_headers(&signer, "get", &member_communities_path(A)).unwrap();
        let all = dir.member_communities(A, headers).await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[1].role.as_deref(), Some("owner"));
        handle.abort();
    }

    #[tokio::test]
    async fn the_view_names_what_the_server_holds() {
        let (dir, handle) = directory_fixture().await;
        let perms = json!({"owner": A, "permissions": {"access": {
            "type":"allow-list","wallets":[A, B],"communities":[CID.to_lowercase(), "00000000-0000-4000-8000-000000000000"]}}});
        let view = describe(&dir, &perms).await;
        let owner = dir.profile(A).await;
        let out = render_view("w.dcl.eth", Some(&owner), &view);
        assert_eq!(
            out,
            format!(
                "world: w.dcl.eth\nowner: {a}  Alice\naccess: only 2 people and the members of 2 communities (the owner always can)\npeople:\n  - {a}  Alice\n  - {b}  (no profile)\ncommunities:\n  - {c}  Decentraland Foundation (1301 members)\n  - 00000000-0000-4000-8000-000000000000  (unknown community)",
                a = A.to_lowercase(),
                b = B,
                c = CID.to_lowercase()
            )
        );
        let open = describe(&dir, &json!({})).await;
        assert_eq!(
            render_view("w.dcl.eth", None, &open),
            "world: w.dcl.eth\nowner: (unknown)\naccess: anyone can enter"
        );
        handle.abort();
    }

    /// The editor end to end against a worlds server that checks the signed
    /// metadata: state → prepare → sign, with a stale payload refused first.
    #[tokio::test]
    async fn the_editor_signs_one_replacement_of_the_setting() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let (dir, dir_handle) = directory_fixture().await;
        let posted = Arc::new(Mutex::new(None::<Value>));
        let gets = Arc::new(AtomicUsize::new(0));
        let (p2, g2) = (posted.clone(), gets.clone());
        let worlds = Router::new()
            .route(
                "/world/gate.dcl.eth/permissions",
                aget(move || {
                    g2.fetch_add(1, Ordering::SeqCst);
                    async move {
                        Json(json!({"owner": A, "permissions": {"access": {"type":"allow-list","wallets":[B],"communities":[]}}}))
                    }
                }),
            )
            .route(
                "/world/gate.dcl.eth/permissions/access",
                post(move |headers: HeaderMap| {
                    let posted = p2.clone();
                    async move {
                        let (_, meta) = crate::signed_fetch::verify_signed_fetch_meta(
                            &headers,
                            "post",
                            "/world/gate.dcl.eth/permissions/access",
                            60,
                        )
                        .await
                        .unwrap();
                        *posted.lock().unwrap() = Some(meta);
                        axum::http::StatusCode::NO_CONTENT
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let worlds_base = format!("http://{}", listener.local_addr().unwrap());
        let worlds_handle = tokio::spawn(async move {
            let _ = axum::serve(listener, worlds).await;
        });

        let (state, rx) = editor_state(worlds_base, "gate.dcl.eth".into(), dir);
        let app = editor_router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let editor_handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let http = reqwest::Client::new();

        let page = http
            .get(format!("{base}/"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(page.contains("personal_sign") && page.contains("api/prepare"));

        let state: Value = http
            .get(format!("{base}/api/state"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(state["owner"]["name"], "Alice");
        assert_eq!(state["access"]["kind"], "allow-list");
        assert_eq!(state["access"]["people"][0]["address"], B);

        let lookup: Value = http
            .get(format!("{base}/api/lookup?q=alice"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(lookup["person"]["address"], A.to_lowercase());
        let search: Value = http
            .get(format!("{base}/api/communities/search?q=abc"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(search["communities"][0]["name"], "ABC Decentraland");
        let members: Value = http
            .get(format!(
                "{base}/api/communities/{}/members",
                CID.to_lowercase()
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(members["total"], 2);

        // A signed read of "my" communities goes through the minted payload.
        let signer = crate::random_test_wallet();
        let me = signer.address().to_lowercase();
        let minted: Value = http
            .get(format!("{base}/api/communities/mine/payload?address={me}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let read_payload = minted["payload"].as_str().unwrap().to_string();
        let read_sig = signer.sign_message(read_payload.as_bytes()).unwrap();
        let mine: Value = http
            .get(format!("{base}/api/communities/mine"))
            .query(&[
                ("address", me.as_str()),
                ("payload", read_payload.as_str()),
                ("signature", read_sig.as_str()),
            ])
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(mine["signed"], true);
        assert_eq!(mine["communities"].as_array().unwrap().len(), 2);

        let bad: Value = http
            .post(format!("{base}/api/prepare"))
            .json(&json!({"kind":"allow-list","wallets":["0x1"]}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(
            bad["error"]
                .as_str()
                .unwrap()
                .contains("not an ethereum address")
        );

        let prep: Value = http
            .post(format!("{base}/api/prepare"))
            .json(&json!({"kind":"allow-list","wallets":[A],"communities":[CID]}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(prep["method"], "POST");
        let payload = prep["payload"].as_str().unwrap().to_string();
        assert!(payload.starts_with("post:/world/gate.dcl.eth/permissions/access:"));
        assert!(payload.ends_with(&list(&[A], &[CID]).metadata_json()));

        let stale: Value = http
            .post(format!("{base}/api/sign"))
            .json(&json!({"address":me,"signature":"0x0","payload":"post:/nope:1:{}"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(stale["fatal"], false);

        let sig = signer.sign_message(payload.as_bytes()).unwrap();
        let done: Value = http
            .post(format!("{base}/api/sign"))
            .json(&json!({"address":me,"signature":sig,"payload":payload}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(done["ok"], true, "{done}");
        assert_eq!(
            done["message"],
            "gate.dcl.eth now admits 1 person and the members of 1 community"
        );
        let meta = posted.lock().unwrap().clone().unwrap();
        assert_eq!(meta["wallets"][0], A.to_lowercase());
        assert_eq!(meta["communities"][0], CID.to_lowercase());
        assert_eq!(
            rx.await.unwrap().unwrap(),
            "gate.dcl.eth now admits 1 person and the members of 1 community"
        );
        assert!(gets.load(Ordering::SeqCst) >= 1);

        editor_handle.abort();
        worlds_handle.abort();
        dir_handle.abort();
    }
}
