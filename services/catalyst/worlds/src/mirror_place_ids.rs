//! The production Places UUID of an exact WCS scene deployment.
use std::collections::HashMap;

use uuid::Uuid;

use super::*;

const PAGE_SIZE: usize = 100;
const MAX_ROWS: usize = 4_000;
const MAX_PAGE_BYTES: usize = 4 * 1024 * 1024;
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(10);

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
    let mut url = url::Url::parse(places)?.join("/api/places")?;
    url.query_pairs_mut()
        .append_pair("names", world.key())
        .append_pair("include_opted_out", "true")
        .append_pair("limit", &PAGE_SIZE.to_string());
    let mut total = None;
    let mut offset = 0;
    let mut matches = PlacesMatches::new(candidates);
    loop {
        let mut page_url = url.clone();
        page_url
            .query_pairs_mut()
            .append_pair("offset", &offset.to_string());
        let response = client
            .get(page_url.as_str())
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
        for row in &page.data {
            matches.add(world, row);
        }
        offset += page.data.len();
        if offset == page.total {
            return Ok(matches.finish(candidates));
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
) {
    let Some(places) = settings.places_source.as_deref() else {
        return;
    };
    let source = &settings.source;
    let result = async {
        let candidates = scene_identity::pending_public_ids(pool, source, world.key(), active_cids).await?;
        if candidates.is_empty() {
            return Ok(());
        }
        let matches = tokio::time::timeout(LOOKUP_TIMEOUT, fetch_ids(client, places, world, &candidates))
            .await.context("Places lookup timed out")??;
        for (cid, id) in matches {
            let result = async {
                let mut tx = pool.begin().await?;
                let binding = scene_identity::bind_public_id(&mut tx, source, world.key(), cid, id).await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(binding)
            }.await;
            match result {
                Ok(scene_identity::PublicIdBinding::Conflict) => {
                    tracing::warn!(%world, %cid, %id, "Places scene UUID conflicts with an existing identity");
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(%world, %cid, %error, "could not save the Places scene UUID");
                }
            }
        }
        Ok::<_, anyhow::Error>(())
    }.await;
    if let Err(error) = result {
        tracing::warn!(%world, %error, "could not look up Places scene UUIDs");
    }
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
