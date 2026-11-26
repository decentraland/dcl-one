//! The production Places UUID of an exact WCS scene deployment.
use std::cmp::Reverse;
use std::collections::HashMap;
use std::sync::LazyLock;

use crate::content::read_through::{Fills, MAX_CONCURRENT_FILLS};
use futures::FutureExt;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::*;

const PAGE_SIZE: usize = 100;
const MAX_ROWS: usize = 4_000;
const MAX_PAGE_BYTES: usize = 4 * 1024 * 1024;
const PAGE_TIMEOUT: Duration = Duration::from_secs(10);
const SCAN_TIMEOUT: Duration = Duration::from_secs(90);
static SCHEDULED_RETRIES: LazyLock<Fills> =
    LazyLock::new(|| Fills::new(MAX_CONCURRENT_FILLS, Duration::ZERO));

#[derive(Deserialize)]
struct Page {
    data: Vec<Value>,
    total: usize,
}

async fn fetch_ids<'a>(
    client: &crate::http::Outbound,
    places: &str,
    world: &WorldName,
    candidates: &'a [String],
) -> Result<Vec<(&'a str, Uuid)>> {
    let scan_once = || tokio::time::timeout(SCAN_TIMEOUT, scan(client, places, world, candidates));
    let (matches, digest) = scan_once().await.context("Places scan timed out")??;
    if let Some(digest) = digest.filter(|_| !matches.is_empty()) {
        let (_, confirmation) = scan_once()
            .await
            .context("Places confirmation timed out")??;
        if confirmation != Some(digest) {
            bail!("Places pagination changed between reads for {world}");
        }
    }
    Ok(matches)
}

// Upstream places@bbfc4b5 sorts named opted-out queries by disabled, then
// deployed_at DESC NULLS LAST, then the requested updated_at DESC NULLS LAST.
// It has no unique tie-breaker or snapshot cursor. Keep the identity fields of
// the response stable across two scans and refuse ties at a page boundary.
type OrderKey = (
    bool,
    Reverse<Option<chrono::DateTime<chrono::FixedOffset>>>,
    Reverse<Option<chrono::DateTime<chrono::FixedOffset>>>,
);

fn order_key(row: &Value) -> Option<OrderKey> {
    let timestamp = |key: &str| {
        let field = row.get(key)?;
        if field.is_null() {
            Some(Reverse(None))
        } else {
            chrono::DateTime::parse_from_rfc3339(field.as_str()?)
                .ok()
                .map(|date| Reverse(Some(date)))
        }
    };
    Some((
        row["disabled"].as_bool()?,
        timestamp("deployed_at")?,
        timestamp("updated_at")?,
    ))
}

type PlacesScan<'a> = (Vec<(&'a str, Uuid)>, Option<[u8; 32]>);

async fn scan<'a>(
    client: &crate::http::Outbound,
    places: &str,
    world: &WorldName,
    candidates: &'a [String],
) -> Result<PlacesScan<'a>> {
    let mut url = url::Url::parse(places)?.join("/api/places")?;
    url.query_pairs_mut()
        .append_pair("names", world.key())
        .append_pair("include_opted_out", "true")
        .append_pair("order_by", "updated_at")
        .append_pair("order", "desc")
        .append_pair("limit", &PAGE_SIZE.to_string());
    let mut total = None;
    let mut offset = 0;
    let mut matches = PlacesMatches::new(candidates);
    let mut digest = Sha256::new();
    let mut seen = HashSet::new();
    let mut previous_key = None;
    loop {
        let mut page_url = url.clone();
        page_url
            .query_pairs_mut()
            .append_pair("offset", &offset.to_string());
        let response = client
            .get(page_url.as_str())
            .timeout(PAGE_TIMEOUT)
            .send()
            .await
            .with_context(|| format!("GET {page_url}"))?;
        if !response.status().is_success() {
            bail!("GET {page_url} answered {}", response.status());
        }
        let page: Page = serde_json::from_slice(&read_body_capped(response, MAX_PAGE_BYTES).await?)
            .with_context(|| format!("parsing {page_url}"))?;
        if page.total > MAX_ROWS
            || total.is_some_and(|expected| page.total != expected)
            || offset > page.total
            || page.data.len() != PAGE_SIZE.min(page.total - offset)
        {
            bail!("inconsistent or over-limit Places pagination for {world}");
        }
        let multipage = page.total > PAGE_SIZE;
        for (index, row) in page.data.iter().enumerate() {
            if multipage {
                let key = order_key(row).context("missing Places row sort key")?;
                if previous_key
                    .is_some_and(|previous| previous > key || (index == 0 && previous == key))
                {
                    bail!("unstable or tied Places pagination boundary for {world}");
                }
                previous_key = Some(key);
                let id = row["id"]
                    .as_str()
                    .and_then(|id| Uuid::parse_str(id).ok())
                    .filter(|id| !id.is_nil())
                    .context("missing Places row UUID")?;
                if !seen.insert(id) {
                    bail!("duplicate Places row {id} across pages for {world}");
                }
                digest.update(serde_json::to_vec(&[
                    &row["id"],
                    &row["world"],
                    &row["world_name"],
                    &row["deployment_id"],
                    &row["disabled"],
                    &row["disabled_reason"],
                    &row["deployed_at"],
                    &row["updated_at"],
                ])?);
            }
            matches.add(world, row);
        }
        offset += page.data.len();
        if offset == page.total {
            return Ok((
                matches.finish(candidates),
                multipage.then(|| digest.finalize().into()),
            ));
        }
        total = Some(page.total);
    }
}

/// Require one row claiming the exact world and deployment, and one occurrence of its UUID
/// across the whole answer. A repeated row can indicate that pagination shifted under us.
struct PlacesMatches<'a> {
    wanted: HashSet<&'a str>,
    by_cid: HashMap<&'a str, Option<Uuid>>,
    id_counts: HashMap<Uuid, usize>,
}

impl<'a> PlacesMatches<'a> {
    fn new(candidates: &'a [String]) -> Self {
        Self {
            wanted: candidates.iter().map(String::as_str).collect(),
            by_cid: HashMap::new(),
            id_counts: HashMap::new(),
        }
    }

    fn add(&mut self, world: &WorldName, row: &Value) {
        let id = row["id"]
            .as_str()
            .and_then(|id| Uuid::parse_str(id).ok())
            .filter(|id| !id.is_nil());
        if let Some(id) = id {
            *self.id_counts.entry(id).or_insert(0) += 1;
        }
        let Some(&cid) = row["deployment_id"]
            .as_str()
            .and_then(|cid| self.wanted.get(cid))
        else {
            return;
        };
        let name = row["world_name"]
            .as_str()
            .and_then(|name| WorldName::parse(name).ok());
        if row["world"] != true || !name.is_some_and(|name| name.key() == world.key()) {
            return;
        }
        let eligible = row["disabled"] == false
            || (row["disabled"] == true && row["disabled_reason"] == "opt_out");
        self.by_cid
            .entry(cid)
            .and_modify(|matched| *matched = None)
            .or_insert(id.filter(|_| eligible));
    }

    fn finish(&self, candidates: &'a [String]) -> Vec<(&'a str, Uuid)> {
        candidates
            .iter()
            .filter_map(|cid| {
                let id = self.by_cid.get(cid.as_str()).copied().flatten()?;
                (self.id_counts.get(&id) == Some(&1)).then_some((cid.as_str(), id))
            })
            .collect()
    }
}

/// Content has already committed. A missing answer leaves the stored fallback eligible for the
/// next pass, including when there is no change to the WCS deployments.
pub(super) async fn refresh(
    client: &crate::http::Outbound,
    pool: &PgPool,
    settings: &MirrorSettings,
    world: &WorldName,
    active_cids: &[&str],
) -> bool {
    let Some(places) = settings.places_source.as_deref() else {
        return false;
    };
    let source = &settings.source;
    let result = async {
        let candidates = scene_identity::pending_public_ids(pool, source, world.key(), active_cids).await?;
        if candidates.is_empty() {
            return Ok(false);
        }
        let matches = fetch_ids(client, places, world, &candidates).await?;
        let mut retry = matches.len() < candidates.len();
        for (cid, id) in matches {
            let result = async {
                let mut tx = pool.begin().await?;
                let binding = scene_identity::bind_public_id(&mut tx, source, world.key(), cid, id).await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(binding)
            }.await;
            match result {
                Ok(scene_identity::PublicIdBinding::Conflict) => {
                    retry = true;
                    tracing::warn!(%world, %cid, %id, "Places scene UUID conflicts with an existing identity");
                }
                Ok(_) => {}
                Err(error) => {
                    retry = true;
                    tracing::warn!(%world, %cid, %error, "could not save the Places scene UUID");
                }
            }
        }
        Ok::<_, anyhow::Error>(retry)
    }.await;
    match result {
        Ok(retry) => retry,
        Err(error) => {
            tracing::warn!(%world, %error, "could not look up Places scene UUIDs");
            true
        }
    }
}

/// After a full content reconciliation, give Places' asynchronous indexing
/// one short retry without keeping a mirror slot open. Unchanged worlds use
/// the normal pass so old pending rows do not fill the short-retry slots.
pub(super) async fn refresh_after_sync(
    client: &crate::http::Outbound,
    pool: &PgPool,
    settings: &MirrorSettings,
    world: &WorldName,
    active_cids: &[&str],
) {
    let retry = refresh(client, pool, settings, world, active_cids).await;
    let Some(delay) = settings.places_retry_delay.filter(|_| retry) else {
        return;
    };
    let key = format!("{}:{}", settings.source, world.key());
    let (client, pool, settings, world) = (
        client.clone(),
        pool.clone(),
        settings.clone(),
        world.clone(),
    );
    let cids: Vec<_> = active_cids.iter().map(|cid| (*cid).to_owned()).collect();
    let _ = SCHEDULED_RETRIES
        .fill(vec![key], move |_| async move {
            tokio::time::sleep(delay).await;
            let cids: Vec<_> = cids.iter().map(String::as_str).collect();
            refresh(&client, &pool, &settings, &world, &cids).await;
        })
        .now_or_never();
}

/// Probe a held world whose content was read without making the reader wait.
pub(crate) async fn refresh_held(
    client: &crate::http::Outbound,
    pool: &PgPool,
    settings: &MirrorSettings,
    world: &WorldName,
) {
    let cids = sqlx::query_scalar::<_, String>(
        "SELECT s.entity_id FROM worlds.world_scenes s \
         JOIN worlds.worlds w ON w.name = s.world_name \
         WHERE w.name = $1 AND w.mirror_source = $2 AND s.mirror_source = $2 \
           AND w.blocked_since IS NULL",
    )
    .bind(world.key())
    .bind(&settings.source)
    .fetch_all(pool)
    .await;
    match cids {
        Ok(cids) if !cids.is_empty() => {
            let cids: Vec<_> = cids.iter().map(String::as_str).collect();
            refresh(client, pool, settings, world, &cids).await;
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%world, %error, "could not read the held world for Places"),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn only_unique_exact_world_deployments_with_valid_place_ids_match() {
        let world = WorldName::parse("Scene.DCL.ETH").expect("world name");
        let candidates =
            ["exact", "ambiguous", "reused", "missing", "nil", "disabled"].map(String::from);
        let row = |deployment, name, id: Uuid| {
            json!({
                "world": true, "world_name": name, "deployment_id": deployment,
                "id": id, "disabled": false,
            })
        };
        let exact = Uuid::new_v4();
        let reused = Uuid::new_v4();
        let mut disabled = row("disabled", "scene.dcl.eth", Uuid::new_v4());
        disabled["disabled"] = json!(true);
        disabled["disabled_reason"] = json!("moderation");
        let rows = [
            row("exact", "SCENE.dcl.eth", exact),
            row("exact", "another.dcl.eth", Uuid::new_v4()),
            row("old", "scene.dcl.eth", Uuid::new_v4()),
            row("ambiguous", "scene.dcl.eth", Uuid::new_v4()),
            row("ambiguous", "scene.dcl.eth", Uuid::new_v4()),
            row("reused", "scene.dcl.eth", reused),
            row("old", "scene.dcl.eth", reused),
            row("nil", "scene.dcl.eth", Uuid::nil()),
            disabled,
            json!({"world": true, "world_name": "scene.dcl.eth", "id": Uuid::new_v4(),
                   "entity_id": "missing", "disabled": false}),
        ];
        let mut matches = PlacesMatches::new(&candidates);
        for row in &rows {
            matches.add(&world, row);
        }
        assert_eq!(matches.finish(&candidates), [("exact", exact)]);
    }
}
