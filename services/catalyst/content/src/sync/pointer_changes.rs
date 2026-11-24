//! One peer's `/pointer-changes` stream: paginated fetch with prefetch,
//! per-delta timestamp plausibility, the overlap each poll re-reads and its
//! suppression, and the confirmed-progress rule for the resume cursor (see
//! [`deploy_entities_from_pointer_changes`]).

use reqwest::Client;
use std::collections::{BTreeSet, HashSet};
use tracing::{debug, info, warn};

use super::repository::LiveDeploymentRepository;

/// A pointer-changes page is at most 500 deltas of entity metadata (a few
/// MB); the cap stops a misbehaving peer from growing the buffer without
/// limit, as every other reader's does.
const MAX_PAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_PAGES_PER_POLL: usize = 10_000;
use super::{SyncDeployment, SyncError, Timestamp};
use crate::batch_deployer::{BatchDeployer, DeploymentReport};

#[derive(Debug, Clone)]
pub struct PointerChangesOptions<'a> {
    pub from_timestamp: Timestamp,
    pub wait_time_ms: u64,
    pub entity_type_filter: Option<&'a HashSet<String>>,
}

/// Where a stream hands its deployments and how their acknowledgement is tracked.
pub struct ScheduleTarget<'a> {
    pub deployer: &'a BatchDeployer,
    pub report: &'a std::sync::Arc<DeploymentReport>,
}

/// Upstream MAX_TIMESTAMP_CLOCK_SKEW_IN_MS: how far ahead of our own clock a remote timestamp
/// may sit and still be adopted as sync state.
pub(crate) const MAX_TIMESTAMP_CLOCK_SKEW_MS: Timestamp = 24 * 60 * 60 * 1000;

/// Bound on rejected-delta log lines per stream, as upstream's MAX_REJECTED_DELTA_LOGS: a server
/// can put a rejectable delta on every page, so per-item logging is attacker-sized.
const MAX_REJECTED_DELTA_LOGS: u64 = 100;

/// Upstream MAX_BOUNDARY_ROWS_TRACKED. Ours only stops *tracking* past the cap (the oldest rows
/// fall back to the DB-side dedup, wasted work but never wrong) where upstream fails the poll,
/// because for us the suppression is purely an optimization: identity below is entity-level, and
/// the deployer already dedups entities durably.
const MAX_DELIVERED_ROWS_TRACKED: usize = 10_000;

/// How long after stamping a deployment's `localTimestamp` a peer may take to make it visible.
/// Upstream stamps `Date.now()` before it stores the files and commits; a v2 flush stamps its
/// transaction's start (`now()`), and several flushes commit in any order. A poll that read past
/// such a row before it committed would pass it for good, so every poll re-reads this window
/// behind the newest row the stream has seen ([`settled_cursor`]).
pub const POINTER_CHANGES_OVERLAP_MS: Timestamp = 5 * 60_000;

/// The confirmed cursor after a poll from `from` whose pages arrived within `span_ms` of its
/// first request, `greatest` being the newest `localTimestamp` the stream has seen. The peer read
/// every page after stamping `greatest - span_ms`, so a row stamped more than the overlap before
/// that was visible when its page was read, and the poll delivered it: every row stamped before
/// the result has been delivered.
fn settled_cursor(from: Timestamp, greatest: Timestamp, span_ms: Timestamp) -> Timestamp {
    from.max(greatest.saturating_sub(POINTER_CHANGES_OVERLAP_MS.saturating_add(span_ms)))
}

/// Upstream `isUsableTimestamp`. These values become the stream's high-water mark, and the persisted
/// resume state only ever moves forward (`advance_sync_frontier` / `advance_server_sync_cursor` are
/// GREATEST-monotonic), so a single bad one is permanent: the node then polls /pointer-changes from a
/// point no real deployment can exceed and silently stops syncing. The deployment parser already
/// checks that timestamps encode whole milliseconds.
pub(crate) fn is_usable_timestamp(ts: Timestamp, now_ms: Timestamp) -> bool {
    ts >= 0 && ts <= now_ms.saturating_add(MAX_TIMESTAMP_CLOCK_SKEW_MS)
}

/// Folds a delta's `localTimestamp` into the tentative high-water mark ONLY when the delta is
/// acceptable -- the upstream-53e9c07 guarantee this function keeps in one place. The mark becomes
/// the confirmed timestamp at the next poll boundary and from there the durable, GREATEST-monotonic
/// frontier, so one hostile delta reaching the max() below would poison the node permanently.
///
/// Malformed and oversized rows are skipped before scheduling or advancing the mark.
fn accept_delta_and_advance_mark(
    item: serde_json::Value,
    from_timestamp: Timestamp,
    now_ms: Timestamp,
    greatest_timestamp: &mut Timestamp,
    rejected_logged: &mut u64,
) -> Option<SyncDeployment> {
    let mut report_rejected = |reason: &str, detail: String| {
        if *rejected_logged < MAX_REJECTED_DELTA_LOGS {
            *rejected_logged += 1;
            warn!(reason, detail, "Rejecting delta from /pointer-changes");
            if *rejected_logged == MAX_REJECTED_DELTA_LOGS {
                warn!(
                    suppressed_after = MAX_REJECTED_DELTA_LOGS,
                    "Too many rejected deltas from /pointer-changes, suppressing further logs"
                );
            }
        }
    };

    let deployment: SyncDeployment = match serde_json::from_value(item) {
        Ok(d) => d,
        Err(e) => {
            report_rejected("unparseable", e.to_string());
            return None;
        }
    };

    if let Err(reason) = deployment.validate_shape() {
        report_rejected("invalid deployment", reason.into());
        return None;
    }
    let Some(local_ts) = deployment.local_timestamp else {
        report_rejected("invalid deployment", "localTimestamp is required".into());
        return None;
    };
    if let Err(reason) = check_delta_limits(&deployment) {
        crate::metrics::OVERSIZED_POINTER_CHANGES
            .handle([])
            .increment(1);
        report_rejected("oversized deployment", reason);
        return None;
    }
    if let Err(reason) = deployment.validate_auth_chain() {
        report_rejected("invalid deployment", reason.into());
        return None;
    }
    if !is_usable_timestamp(deployment.entity_timestamp, now_ms)
        || !is_usable_timestamp(local_ts, now_ms)
    {
        report_rejected(
            "implausible timestamp",
            format!(
                "entity_id={} entityTimestamp={} localTimestamp={:?}",
                deployment.entity_id, deployment.entity_timestamp, deployment.local_timestamp
            ),
        );
        return None;
    }

    if local_ts >= from_timestamp {
        *greatest_timestamp = (*greatest_timestamp).max(local_ts);
    }

    Some(deployment)
}

fn check_delta_limits(deployment: &SyncDeployment) -> Result<(), String> {
    const MAX_POINTERS: usize = 10_000;
    const MAX_AUTH_LINKS: usize = 100;
    const MAX_FIELD_BYTES: usize = 256 * 1024;
    const MAX_ROW_BYTES: usize = 1024 * 1024;

    if deployment.pointers.len() > MAX_POINTERS {
        return Err(format!(
            "pointer-change delta exceeds {MAX_POINTERS} pointers"
        ));
    }
    if deployment.auth_chain.len() > MAX_AUTH_LINKS {
        return Err(format!(
            "pointer-change delta exceeds {MAX_AUTH_LINKS} auth links"
        ));
    }

    let mut bytes = 0;
    let mut add = |len| {
        if len > MAX_FIELD_BYTES {
            return Err(format!(
                "pointer-change delta field exceeds {MAX_FIELD_BYTES} bytes"
            ));
        }
        bytes += len;
        if bytes > MAX_ROW_BYTES {
            return Err(format!(
                "pointer-change delta exceeds {MAX_ROW_BYTES} field bytes"
            ));
        }
        Ok(())
    };
    add(deployment.entity_type.len())?;
    add(deployment.entity_id.len())?;
    for pointer in &deployment.pointers {
        add(pointer.len())?;
    }
    for link in &deployment.auth_chain {
        add(link.link_type.to_string().len())?;
        add(link.payload.len())?;
        add(link.signature.as_deref().map_or(0, str::len))?;
    }
    Ok(())
}

/// Rows already delivered at or after the poll's `from`, so the overlap every poll re-reads costs
/// no `is_entity_deployed` round trip per row; upstream's `boundaryRowFingerprint` map covers the
/// inclusive `from` alone, which is all its polls re-read. Entity-level identity suffices -- the
/// deployer dedups by entity_id anyway -- so this never affects correctness, only wasted work.
#[derive(Default)]
struct DeliveredWindow(BTreeSet<(Timestamp, String, Timestamp)>);

impl DeliveredWindow {
    fn fingerprint(deployment: &SyncDeployment) -> Option<(Timestamp, String, Timestamp)> {
        deployment.local_timestamp.map(|local| {
            (
                local,
                deployment.entity_id.clone(),
                deployment.entity_timestamp,
            )
        })
    }

    /// Forgets the rows before the poll's `from`: the peer does not return them again.
    fn begin_poll(&mut self, from: Timestamp) {
        self.0 = self.0.split_off(&(from, String::new(), Timestamp::MIN));
    }

    fn already_delivered(&self, deployment: &SyncDeployment) -> bool {
        Self::fingerprint(deployment).is_some_and(|fp| self.0.contains(&fp))
    }

    fn record_delivered(&mut self, deployment: &SyncDeployment) {
        if let Some(fp) = Self::fingerprint(deployment) {
            self.0.insert(fp);
            if self.0.len() > MAX_DELIVERED_ROWS_TRACKED {
                self.0.pop_first();
            }
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct PointerChangesPage {
    deltas: Vec<serde_json::Value>,
    #[serde(default)]
    pagination: Option<PaginationInfo>,
}

#[derive(Debug, serde::Deserialize)]
struct PaginationInfo {
    #[serde(default, rename = "moreData")]
    more_data: bool,
    next: Option<String>,
}

/// One page, and whether the peer holds more rows after it.
pub(super) async fn fetch_page(
    client: &Client,
    url: &str,
) -> Result<(Vec<serde_json::Value>, bool), SyncError> {
    let resp = client.get(url).send().await?.error_for_status()?;
    let resp = crate::sync_types::refuse_redirect(resp, &format!("pointer-changes page {url}"))?;
    let body = crate::http::read_peer_body_capped(resp, MAX_PAGE_BYTES)
        .await
        .map_err(|e| SyncError::Other(format!("pointer-changes page {url}: {e}")))?;
    let page: PointerChangesPage = serde_json::from_slice(&body)?;
    let more = page
        .pagination
        .is_some_and(|p| p.more_data || p.next.is_some());
    Ok((page.deltas, more))
}

/// The ascending page after `page`: the keyset of its last row, `localTimestamp` passed through
/// as the peer wrote it. The peer's own `next` link carries no `sortingOrder` (upstream's does
/// not either), so it reads the same rows again downwards: following it, a poll never got past
/// its first page, and a page that one flush filled with a single timestamp stalled the stream
/// for good.
fn keyset_next_url(server: &str, page: &[serde_json::Value], now_ms: Timestamp) -> Option<String> {
    let (from, last_id) = page.iter().rev().find_map(|delta| {
        let from = crate::sync_types::timestamp_number(delta.get("localTimestamp")?.as_number()?)?;
        if !is_usable_timestamp(from, now_ms) {
            return None;
        }
        Some((from, delta.get("entityId")?.as_str()?))
    })?;
    let mut url = url::Url::parse(&format!("{server}/pointer-changes")).ok()?;
    url.query_pairs_mut()
        .append_pair("sortingOrder", "ASC")
        .append_pair("sortingField", "local_timestamp")
        .append_pair("from", &from.to_string())
        .append_pair("lastId", last_id);
    Some(url.to_string())
}

fn page_entity_ids(page: &[serde_json::Value]) -> HashSet<String> {
    page.iter()
        .filter_map(|delta| delta.get("entityId")?.as_str().map(str::to_owned))
        .collect()
}

/// The resume cursor (`progress`, the return value, and the persisted frontier) only ever reflects
/// CONFIRMED progress: the high-water timestamp stays tentative while entities are merely scheduled
/// and commits at each poll boundary after the deployer has
/// drained and every deployment this stream scheduled has been acknowledged (deployed, or durably
/// recorded in failed_deployments). A boundary where something remains unacknowledged fails the
/// stream instead, so the caller reconnects from the last confirmed timestamp. Committing per entity,
/// as this used to, let a later confirmed entity carry the cursor past an earlier one still in flight
/// -- a crash in that window skipped it forever. The cursor a boundary confirms is the
/// [`settled_cursor`], the overlap behind the high-water mark. A batch that reaches the page cap
/// checkpoints its acknowledged prefix and resumes at its exact keyset within this run.
/// `settled_polls` counts the boundaries this run settled.
#[allow(clippy::too_many_arguments)]
pub async fn deploy_entities_from_pointer_changes<S>(
    client: &Client,
    server: &str,
    options: &PointerChangesOptions<'_>,
    target: ScheduleTarget<'_>,
    heartbeat_repo: Option<std::sync::Arc<LiveDeploymentRepository>>,
    progress: &std::sync::atomic::AtomicI64,
    settled_polls: &std::sync::atomic::AtomicU64,
    should_stop: S,
) -> Result<Timestamp, SyncError>
where
    S: Fn() -> bool,
{
    deploy_pointer_pages::<_, MAX_PAGES_PER_POLL>(
        client,
        server,
        options,
        target,
        heartbeat_repo,
        progress,
        settled_polls,
        should_stop,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn deploy_pointer_pages<S, const PAGE_LIMIT: usize>(
    client: &Client,
    server: &str,
    options: &PointerChangesOptions<'_>,
    target: ScheduleTarget<'_>,
    heartbeat_repo: Option<std::sync::Arc<LiveDeploymentRepository>>,
    progress: &std::sync::atomic::AtomicI64,
    settled_polls: &std::sync::atomic::AtomicU64,
    should_stop: S,
) -> Result<Timestamp, SyncError>
where
    S: Fn() -> bool,
{
    let ScheduleTarget { deployer, report } = target;
    let advertisers = [server.to_owned()];
    let entity_type_filter = options.entity_type_filter;
    let mut last_beat: Option<std::time::Instant> = None;
    let mut last_persisted = options.from_timestamp;
    info!(
        server,
        from_timestamp = options.from_timestamp,
        has_type_filter = entity_type_filter.is_some(),
        "Starting pointer-changes stream"
    );

    let mut greatest_timestamp = options.from_timestamp;
    let mut confirmed_timestamp = options.from_timestamp;
    let mut rejected_logged: u64 = 0;
    let mut delivered = DeliveredWindow::default();
    let first_page = |from| {
        format!(
            "{server}/pointer-changes?sortingOrder=ASC&sortingField=local_timestamp&from={from}"
        )
    };
    let spawn_fetch = |in_flight: &mut tokio::task::JoinSet<_>, u: String| {
        let c = client.clone();
        in_flight.spawn(async move { fetch_page(&c, &u).await });
    };

    let mut poll_started = std::time::Instant::now();
    // Dropping the stream also cancels its page fetch.
    let mut in_flight = tokio::task::JoinSet::new();
    spawn_fetch(&mut in_flight, first_page(options.from_timestamp));
    let mut previous_page: HashSet<String> = HashSet::new();
    let mut pages_fetched = 0;

    loop {
        if should_stop() {
            return Ok(confirmed_timestamp);
        }

        let (items, more) = in_flight
            .join_next()
            .await
            .expect("one pointer-changes page is queued on every iteration")
            .map_err(|e| SyncError::Other(format!("pointer-changes prefetch join: {e}")))??;
        pages_fetched += 1;
        let span = poll_started.elapsed();

        if let Some(repo) = &heartbeat_repo
            && last_beat.is_none_or(|t| t.elapsed() >= std::time::Duration::from_secs(10))
        {
            let _ = repo
                .set_sync_heartbeat(chrono::Utc::now().timestamp_millis())
                .await;
            last_beat = Some(std::time::Instant::now());
        }

        if items.is_empty() {
            debug!(server, from = confirmed_timestamp, "No new pointer-changes");
        }

        // A peer that ignores `lastId` on ascending pages (a v2 node from before the keyset
        // bound; upstream honours it) answers the continuation with rows it already sent;
        // ending the chain there hands the rest to the next poll instead of looping on one page.
        let page_ids = page_entity_ids(&items);
        let repeated = !page_ids.is_empty() && page_ids.is_subset(&previous_page);
        if repeated {
            warn!(
                server,
                from = confirmed_timestamp,
                "Peer answered a keyset continuation with the page before it; ending this poll"
            );
        }
        previous_page = page_ids;
        let now_ms = chrono::Utc::now().timestamp_millis();
        let next = if more && !repeated {
            keyset_next_url(server, &items, now_ms)
        } else {
            None
        };

        let reached_page_limit = pages_fetched == PAGE_LIMIT;
        if let Some(next) = &next
            && !reached_page_limit
        {
            spawn_fetch(&mut in_flight, next.clone());
        }

        for item in items {
            if should_stop() {
                return Ok(confirmed_timestamp);
            }

            let Some(deployment) = accept_delta_and_advance_mark(
                item,
                options.from_timestamp,
                now_ms,
                &mut greatest_timestamp,
                &mut rejected_logged,
            ) else {
                continue;
            };

            if delivered.already_delivered(&deployment) {
                continue;
            }

            if let Some(filter) = entity_type_filter
                && !filter.contains(&deployment.entity_type)
            {
                continue;
            }

            delivered.record_delivered(&deployment);
            deployer
                .schedule_entity_deployment(deployment, &advertisers, Some(report))
                .await?;
        }

        if next.is_none() || reached_page_limit {
            deployer.on_idle().await?;
            if !report.is_complete() {
                return Err(SyncError::Other(format!(
                    "pointer-changes deployments for {} not fully acknowledged ({} of {}); \
                     reconnecting from the last confirmed timestamp",
                    server,
                    report.acknowledged(),
                    report.scheduled()
                )));
            }
            if report.lost() > 0 {
                return Err(SyncError::Other(format!(
                    "deployer reported silently lost entities while syncing {}; \
                     reconnecting from the last confirmed timestamp",
                    server
                )));
            }
            let span_ms = Timestamp::try_from(span.as_millis()).unwrap_or(Timestamp::MAX);
            confirmed_timestamp = settled_cursor(confirmed_timestamp, greatest_timestamp, span_ms);
            progress.fetch_max(confirmed_timestamp, std::sync::atomic::Ordering::Relaxed);
            settled_polls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if let Some(repo) = &heartbeat_repo
                && confirmed_timestamp > last_persisted
            {
                let _ = repo.advance_sync_frontier(confirmed_timestamp).await;
                let _ = repo
                    .advance_server_sync_cursor(server, confirmed_timestamp)
                    .await;
                last_persisted = confirmed_timestamp;
            }

            if next.is_none() && options.wait_time_ms == 0 {
                break;
            }
            if options.wait_time_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(options.wait_time_ms)).await;
            }
            pages_fetched = 0;
            if let Some(next) = next {
                // Keep the chain's full span for cursor safety. The keyset still moves
                // forward when the overlap keeps the durable timestamp unchanged.
                spawn_fetch(&mut in_flight, next);
            } else {
                delivered.begin_poll(confirmed_timestamp);
                previous_page.clear();
                poll_started = std::time::Instant::now();
                spawn_fetch(&mut in_flight, first_page(confirmed_timestamp));
            }
        }
    }

    info!(server, confirmed_timestamp, "Pointer-changes stream ended");
    Ok(confirmed_timestamp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_pointer_changes_page_is_never_followed_through_a_redirect() {
        let (port, hits) = crate::sync_test_support::redirecting_peer("/pointer-changes", |port| {
            format!("http://localhost:{port}/target")
        })
        .await;
        let server = format!("http://localhost:{port}");
        let config = crate::config::Config::parse_for_tests(&[
            "x",
            "--database-url",
            "postgres:///catalyst",
            "--sync-source",
            &server,
        ])
        .unwrap();
        let client = crate::sync::sync_http_client(&config).unwrap();
        let page = fetch_page(&client, &format!("{server}/pointer-changes")).await;
        assert!(
            matches!(&page, Err(SyncError::Other(e)) if e.contains("refusing to follow")),
            "{page:?}"
        );
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    fn delta(local_timestamp: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "entityId": crate::auth::hash_bytes_v1(b"delta test"),
            "entityType": "scene",
            "pointers": ["0,0"],
            "authChain": [{"type": "SIGNER", "payload": "0x0000000000000000000000000000000000000001"}],
            "entityTimestamp": 1_700_000_000_000i64,
            "localTimestamp": local_timestamp,
        })
    }

    const NOW_MS: i64 = 1_750_000_000_000;
    const FROM_TS: i64 = 1_600_000_000_000;

    #[test]
    fn a_keyset_uses_the_last_usable_timestamp_even_if_its_row_cannot_deploy() {
        let rows = [
            serde_json::from_str(r#"{"entityId": "usable-boundary", "localTimestamp": 1.7e12}"#)
                .unwrap(),
            serde_json::json!({"entityId": "too-far", "localTimestamp": i64::MAX}),
        ];
        let next = keyset_next_url("http://localhost", &rows, NOW_MS).unwrap();
        let next = url::Url::parse(&next).unwrap();
        let params: std::collections::HashMap<_, _> = next.query_pairs().collect();
        assert_eq!(params["from"], "1700000000000");
        assert_eq!(params["lastId"], "usable-boundary");
    }

    #[test]
    fn far_future_local_timestamp_is_rejected_and_never_moves_the_mark() {
        let mut mark = FROM_TS;
        let mut logged = 0;
        let rejected = accept_delta_and_advance_mark(
            delta(serde_json::json!(253_402_300_799_000i64)),
            FROM_TS,
            NOW_MS,
            &mut mark,
            &mut logged,
        );
        assert!(rejected.is_none(), "far-future delta must be rejected");
        assert_eq!(mark, FROM_TS, "a rejected delta must not move the mark");

        let negative = accept_delta_and_advance_mark(
            delta(serde_json::json!(-1)),
            FROM_TS,
            NOW_MS,
            &mut mark,
            &mut logged,
        );
        assert!(negative.is_none(), "negative delta must be rejected");
        assert_eq!(mark, FROM_TS);
    }

    #[test]
    fn json_1e999_local_timestamp_cannot_poison_the_mark() {
        let raw = r#"{
            "entityId": "QmPoisonPoisonPoisonPoisonPoisonPoisonPoison1",
            "entityType": "scene",
            "pointers": ["0,0"],
            "authChain": [],
            "entityTimestamp": 1700000000000,
            "localTimestamp": 1e999
        }"#;
        let Ok(item) = serde_json::from_str::<serde_json::Value>(raw) else {
            return;
        };
        let mut mark = FROM_TS;
        let mut logged = 0;
        let rejected = accept_delta_and_advance_mark(item, FROM_TS, NOW_MS, &mut mark, &mut logged);
        assert!(rejected.is_none(), "overflowed timestamp must be rejected");
        assert_eq!(mark, FROM_TS, "a rejected delta must not move the mark");
    }

    #[test]
    fn implausible_entity_timestamp_rejects_the_delta_too() {
        let mut item = delta(serde_json::json!(FROM_TS + 5));
        item["entityTimestamp"] = serde_json::json!(253_402_300_799_000i64);
        let mut mark = FROM_TS;
        let mut logged = 0;
        let rejected = accept_delta_and_advance_mark(item, FROM_TS, NOW_MS, &mut mark, &mut logged);
        assert!(rejected.is_none());
        assert_eq!(mark, FROM_TS);
    }

    #[test]
    fn plausible_delta_is_accepted_and_moves_the_mark() {
        let mut mark = FROM_TS;
        let mut logged = 0;
        let accepted = accept_delta_and_advance_mark(
            delta(serde_json::json!(FROM_TS + 1000)),
            FROM_TS,
            NOW_MS,
            &mut mark,
            &mut logged,
        );
        assert!(accepted.is_some());
        assert_eq!(mark, FROM_TS + 1000);
        assert_eq!(logged, 0);

        let skewed = accept_delta_and_advance_mark(
            delta(serde_json::json!(NOW_MS + MAX_TIMESTAMP_CLOCK_SKEW_MS - 1)),
            FROM_TS,
            NOW_MS,
            &mut mark,
            &mut logged,
        );
        assert!(skewed.is_some());
        assert_eq!(mark, NOW_MS + MAX_TIMESTAMP_CLOCK_SKEW_MS - 1);
    }

    #[test]
    fn delta_without_local_timestamp_is_rejected_and_never_moves_the_mark() {
        let mut item = delta(serde_json::json!(0));
        item.as_object_mut().unwrap().remove("localTimestamp");
        let mut mark = FROM_TS;
        let mut logged = 0;
        let accepted = accept_delta_and_advance_mark(item, FROM_TS, NOW_MS, &mut mark, &mut logged);
        assert!(accepted.is_none());
        assert_eq!(mark, FROM_TS);
    }

    #[test]
    fn malformed_deltas_are_skipped_before_scheduling_or_advancing() {
        let mut mark = FROM_TS;
        let mut logged = 0;
        for (field, value) in [
            ("entityId", serde_json::json!("not-a-cid")),
            ("pointers", serde_json::json!([])),
            ("authChain", serde_json::json!([])),
            (
                "authChain",
                serde_json::json!([{"type": "ECDSA_SIGNED_ENTITY", "payload": "id"}]),
            ),
            ("localTimestamp", serde_json::Value::Null),
            ("localTimestamp", serde_json::json!(FROM_TS as f64 + 0.5)),
        ] {
            let mut item = delta(serde_json::json!(FROM_TS + 1_000));
            item[field] = value;
            assert!(
                accept_delta_and_advance_mark(item, FROM_TS, NOW_MS, &mut mark, &mut logged)
                    .is_none(),
                "{field}"
            );
        }
        assert_eq!(mark, FROM_TS);
        assert_eq!(logged, 6);
    }

    /// The stream never sends `includeAuthChain`, and `SyncDeployment` parses
    /// `authChain` as required: a peer that dropped it when the flag is off
    /// would land every delta here, and the mark would never advance.
    /// That is why `queries::get_pointer_changes` emits `auth_chain` for
    /// both flag values, as peer.decentraland.org does.
    #[test]
    fn delta_without_auth_chain_is_rejected_and_never_moves_the_mark() {
        let mut item = delta(serde_json::json!(FROM_TS + 1000));
        item.as_object_mut().unwrap().remove("authChain");
        let mut mark = FROM_TS;
        let mut logged = 0;
        let accepted = accept_delta_and_advance_mark(item, FROM_TS, NOW_MS, &mut mark, &mut logged);
        assert!(accepted.is_none());
        assert_eq!(mark, FROM_TS);
        assert_eq!(logged, 1);
    }

    #[test]
    fn oversized_deltas_are_skipped_before_advancing_the_mark() {
        let recorder = dcl_metrics::PrometheusBuilder::new().build_recorder();
        let _recording = dcl_metrics::set_default_local_recorder(&recorder);
        let mut mark = FROM_TS;
        let mut logged = 0;
        for pointers in [
            vec!["0,0".to_string(); 10_001],
            vec!["a".repeat(256 * 1024 + 1)],
            vec!["a".repeat(256 * 1024); 4],
        ] {
            let mut item = delta(serde_json::json!(FROM_TS + 1_000));
            item["pointers"] = serde_json::json!(pointers);
            assert!(
                accept_delta_and_advance_mark(item, FROM_TS, NOW_MS, &mut mark, &mut logged)
                    .is_none()
            );
        }
        let mut item = delta(serde_json::json!(FROM_TS + 1_000));
        let mut links = item["authChain"].as_array().unwrap().clone();
        links.extend(std::iter::repeat_n(
            serde_json::json!({"type": "ECDSA_EPHEMERAL", "payload": "payload", "signature": "signature"}),
            100,
        ));
        item["authChain"] = serde_json::json!(links);
        assert!(
            accept_delta_and_advance_mark(item, FROM_TS, NOW_MS, &mut mark, &mut logged).is_none()
        );
        assert_eq!(mark, FROM_TS);
        assert_eq!(logged, 4);
        assert!(
            recorder
                .handle()
                .render()
                .lines()
                .any(|line| line == "content_sync_oversized_pointer_changes_total 4")
        );
    }

    fn deployment_at(entity_id: &str, local_ts: Timestamp) -> SyncDeployment {
        serde_json::from_value(serde_json::json!({
            "entityId": entity_id,
            "entityType": "scene",
            "pointers": ["0,0"],
            "authChain": [],
            "entityTimestamp": 1_700_000_000_000i64,
            "localTimestamp": local_ts,
        }))
        .expect("test deployment must deserialize")
    }

    #[test]
    fn the_window_suppresses_rows_delivered_at_or_after_the_poll_from() {
        let mut window = DeliveredWindow::default();
        let (a, b) = (deployment_at("QmA", 200), deployment_at("QmB", 300));
        assert!(!window.already_delivered(&a));
        window.record_delivered(&a);
        window.record_delivered(&b);

        window.begin_poll(200);
        assert!(window.already_delivered(&a), "a replayed row is skipped");
        assert!(window.already_delivered(&b), "a replayed row is skipped");
        assert!(
            !window.already_delivered(&deployment_at("QmC", 250)),
            "a row that committed after the last poll read past it passes"
        );

        window.begin_poll(250);
        assert!(
            !window.already_delivered(&a),
            "rows before the poll's from are forgotten"
        );
        assert!(window.already_delivered(&b));
    }

    /// An upstream-style peer: `(localTimestamp, lower(entityId))` order, two rows a page, the
    /// keyset honoured on the `from` bound of ascending pages and on the `to` bound of
    /// descending ones, and a `next` link without `sortingOrder`, as upstream writes it. The
    /// `late` rows commit once it has answered one `/pointer-changes` request, whatever their
    /// `localTimestamp`; the counter is the requests it answered.
    async fn spawn_peer(
        rows: Vec<serde_json::Value>,
        late: Vec<serde_json::Value>,
        bodies: std::collections::HashMap<String, Vec<u8>>,
    ) -> (
        String,
        tokio::task::JoinHandle<()>,
        std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) {
        use axum::extract::{Path, Query};
        use std::collections::HashMap;
        use std::sync::atomic::{AtomicUsize, Ordering};
        const PAGE: usize = 2;
        let key = |d: &serde_json::Value| {
            (
                d["localTimestamp"].as_i64().unwrap(),
                d["entityId"].as_str().unwrap().to_lowercase(),
            )
        };
        let late_ids: HashSet<String> = late
            .iter()
            .map(|d| d["entityId"].as_str().unwrap().to_owned())
            .collect();
        let mut rows = rows;
        rows.extend(late);
        rows.sort_by_key(key);
        let rows = std::sync::Arc::new(rows);
        let served = std::sync::Arc::new(AtomicUsize::new(0));
        let answered = served.clone();
        let changes = move |Query(q): Query<HashMap<String, String>>| {
            let rows = rows.clone();
            let committed = served.fetch_add(1, Ordering::SeqCst) > 0;
            let late_ids = late_ids.clone();
            async move {
                let num = |k: &str| q.get(k).map(|v| v.parse::<i64>().unwrap());
                let last = q.get("lastId").map(|s| s.to_lowercase());
                let asc = q.get("sortingOrder").map(String::as_str) == Some("ASC");
                let mut hits: Vec<&serde_json::Value> = rows
                    .iter()
                    .filter(|d| committed || !late_ids.contains(d["entityId"].as_str().unwrap()))
                    .filter(|d| {
                        let k = key(d);
                        let from_ok = match (num("from"), &last) {
                            (Some(f), Some(l)) if asc => k > (f, l.clone()),
                            (Some(f), _) => k.0 >= f,
                            (None, _) => true,
                        };
                        let to_ok = match (num("to"), &last) {
                            (Some(t), Some(l)) if !asc => k < (t, l.clone()),
                            (Some(t), _) => k.0 <= t,
                            (None, _) => true,
                        };
                        from_ok && to_ok
                    })
                    .collect();
                if !asc {
                    hits.reverse();
                }
                let more = hits.len() > PAGE;
                hits.truncate(PAGE);
                let next = (more && !hits.is_empty()).then(|| {
                    let last = hits.last().unwrap();
                    let from = num("from")
                        .map(|f| format!("from={f}&"))
                        .unwrap_or_default();
                    format!(
                        "?{from}to={}&limit={PAGE}&lastId={}",
                        last["localTimestamp"],
                        last["entityId"].as_str().unwrap()
                    )
                });
                axum::Json(serde_json::json!({
                    "deltas": hits,
                    "pagination": {"moreData": more, "next": next},
                }))
            }
        };
        let contents = move |Path(hash): Path<String>| {
            let body = bodies.get(&hash).cloned();
            async move { body.ok_or(axum::http::StatusCode::NOT_FOUND) }
        };
        let app = axum::Router::new()
            .route("/pointer-changes", axum::routing::get(changes))
            .route("/contents/{hash}", axum::routing::get(contents));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{addr}"), handle, answered)
    }

    /// One signed scene per `localTimestamp`, each on its own pointer `{prefix}:{first + n}`:
    /// the peer's row, the entity id, and the entity file the peer serves for it.
    async fn scenes(
        prefix: &str,
        first: usize,
        stamps: &[Timestamp],
    ) -> Vec<(serde_json::Value, String, Vec<u8>)> {
        let mut out = Vec::new();
        for (n, local_ts) in stamps.iter().enumerate() {
            let pointer = format!("{prefix}:{}", first + n);
            let body = serde_json::to_vec(&serde_json::json!({
                "version": "v3", "type": "scene", "pointers": [pointer],
                "timestamp": 1_000, "content": [], "metadata": {}
            }))
            .unwrap();
            let id = crate::auth::hash_bytes_v1(&body);
            let row = serde_json::json!({
                "entityId": id, "entityType": "scene", "pointers": [pointer],
                "authChain": crate::sync_test_support::signed_chain(&id).await,
                "entityTimestamp": 1_000, "localTimestamp": local_ts,
            });
            out.push((row, id, body));
        }
        out
    }

    /// Streams from `base` into the scratch database from cursor 0 and returns the confirmed
    /// cursor and the report.
    async fn stream_into(
        pool: &sqlx::PgPool,
        base: &str,
        wait_time_ms: u64,
        should_stop: impl Fn() -> bool,
    ) -> (Timestamp, std::sync::Arc<DeploymentReport>) {
        use crate::batch_deployer::BatchDeployerConfig;
        use crate::bloom_filter::BloomFilter;
        use crate::failed_deployments_store::LiveFailedDeploymentsStore;
        use crate::sync_deployer::LiveSyncDeployer;
        use std::sync::Arc;

        let (storage, tmp) = crate::sync_test_support::temp_storage("pointer-changes").await;
        let batch = BatchDeployer::new(
            BatchDeployerConfig::default(),
            crate::sync::test_sync_client(reqwest::Client::new()),
            storage.clone(),
            pool.clone(),
            Arc::new(LiveSyncDeployer::new(
                pool.clone(),
                storage.clone(),
                2,
                crate::sync_deployer::SyncValidation::AuthChainOnly,
            )),
            Arc::new(LiveFailedDeploymentsStore::new(pool.clone())),
            BloomFilter::with_capacity(100),
        );
        batch.set_peers(vec![base.to_owned()]).await;
        let report = Arc::new(DeploymentReport::default());
        let progress = std::sync::atomic::AtomicI64::new(0);
        let confirmed = deploy_entities_from_pointer_changes(
            &reqwest::Client::new(),
            base,
            &PointerChangesOptions {
                from_timestamp: 0,
                wait_time_ms,
                entity_type_filter: None,
            },
            ScheduleTarget {
                deployer: &batch,
                report: &report,
            },
            None,
            &progress,
            &std::sync::atomic::AtomicU64::new(0),
            should_stop,
        )
        .await
        .unwrap();
        let _ = tokio::fs::remove_dir_all(&tmp).await;
        (confirmed, report)
    }

    async fn missing(pool: &sqlx::PgPool, ids: &[String]) -> Vec<String> {
        let mut missing = Vec::new();
        for id in ids {
            if !crate::sync_deployer::is_entity_deployed(pool, id, 0)
                .await
                .unwrap()
            {
                missing.push(id.clone());
            }
        }
        missing
    }

    async fn forget(pool: sqlx::PgPool, prefix: &str) {
        for sql in [
            "DELETE FROM content.active_pointers WHERE pointer LIKE $1",
            "DELETE FROM content.deployments WHERE entity_pointers[1] LIKE $1",
        ] {
            sqlx::query(sql)
                .bind(format!("{prefix}%"))
                .execute(&pool)
                .await
                .unwrap();
        }
        pool.close().await;
    }

    /// One v2 flush stamps up to 500 rows with one `now()` and a page holds 500: a poll that
    /// followed the peer's `next` link re-read its first page downwards and stopped, so rows
    /// past that page waited for later polls, and a full page at one timestamp stalled the
    /// stream for good. The keyset continuation walks
    /// every row in one poll.
    #[tokio::test]
    async fn one_poll_walks_every_page_past_a_group_sharing_one_timestamp() {
        let Some(pool) = crate::sync_test_support::test_pool(3).await else {
            return;
        };
        let prefix = crate::sync_test_support::unique();
        let rows = scenes(
            &prefix,
            0,
            &[1_000, 1_000, 1_000, 1_000, 1_000, 2_000, 2_000],
        )
        .await;
        let ids: Vec<String> = rows.iter().map(|(_, id, _)| id.clone()).collect();
        let bodies = rows
            .iter()
            .map(|(_, id, b)| (id.clone(), b.clone()))
            .collect();
        let rows = rows.into_iter().map(|(row, _, _)| row).collect();
        let (base, server, _) = spawn_peer(rows, Vec::new(), bodies).await;

        let (confirmed, report) = stream_into(&pool, &base, 0, || false).await;

        server.abort();
        let missing = missing(&pool, &ids).await;
        forget(pool, &prefix).await;
        assert!(missing.is_empty(), "never delivered: {missing:?}");
        assert_eq!(
            confirmed, 0,
            "the overlap keeps the cursor behind rows still committing"
        );
        assert_eq!((report.scheduled(), report.lost()), (7, 0));
    }

    /// A peer row that commits after a poll has read past its `localTimestamp` -- an upstream
    /// upload stamps before it stores the files, and v2 flushes commit out of order -- reaches
    /// the next poll, which re-reads the overlap behind the newest row seen. Polls from the
    /// newest row seen passed it for good.
    #[tokio::test]
    async fn a_row_committed_after_a_poll_read_past_it_reaches_the_next_poll() {
        use std::sync::atomic::Ordering;
        let Some(pool) = crate::sync_test_support::test_pool(3).await else {
            return;
        };
        let prefix = crate::sync_test_support::unique();
        let early = scenes(&prefix, 0, &[1_000_000]).await;
        let late = scenes(&prefix, 1, &[900_000]).await;
        let ids: Vec<String> = early
            .iter()
            .chain(&late)
            .map(|(_, id, _)| id.clone())
            .collect();
        let bodies = early
            .iter()
            .chain(&late)
            .map(|(_, id, b)| (id.clone(), b.clone()))
            .collect();
        let (base, server, served) = spawn_peer(
            early.into_iter().map(|(row, _, _)| row).collect(),
            late.into_iter().map(|(row, _, _)| row).collect(),
            bodies,
        )
        .await;

        let (confirmed, report) =
            stream_into(&pool, &base, 10, || served.load(Ordering::SeqCst) >= 3).await;

        server.abort();
        let missing = missing(&pool, &ids).await;
        forget(pool, &prefix).await;
        assert!(missing.is_empty(), "never delivered: {missing:?}");
        assert!(
            (600_000..=1_000_000 - POINTER_CHANGES_OVERLAP_MS).contains(&confirmed),
            "the cursor settles the overlap behind the newest row: {confirmed}"
        );
        assert_eq!((report.scheduled(), report.lost()), (2, 0));
    }

    #[tokio::test]
    async fn a_peer_repeating_an_oversized_delta_can_still_deliver_valid_rows() {
        use std::sync::atomic::Ordering;

        let Some(pool) = crate::sync_test_support::test_pool(3).await else {
            return;
        };
        let prefix = crate::sync_test_support::unique();
        let mut entities = scenes(&prefix, 0, &[900_000, 1_000_000]).await.into_iter();
        let (mut oversized, rejected, rejected_body) = entities.next().unwrap();
        let (valid, accepted, accepted_body) = entities.next().unwrap();
        oversized["pointers"] = serde_json::json!(vec!["0,0"; 10_001]);
        let (base, server, served) = spawn_peer(
            vec![oversized, valid],
            Vec::new(),
            std::collections::HashMap::from([
                (rejected.clone(), rejected_body),
                (accepted.clone(), accepted_body),
            ]),
        )
        .await;

        let (confirmed, report) =
            stream_into(&pool, &base, 10, || served.load(Ordering::SeqCst) >= 3).await;
        server.abort();
        let missing = missing(&pool, &[rejected.clone(), accepted]).await;
        forget(pool, &prefix).await;

        assert_eq!(missing, [rejected]);
        assert!((600_000..=700_000).contains(&confirmed), "{confirmed}");
        assert_eq!((report.scheduled(), report.lost()), (1, 0));
    }

    #[tokio::test]
    async fn the_page_cap_confirms_progress_then_continues_inside_a_shared_timestamp() {
        use crate::batch_deployer::BatchDeployerConfig;
        use crate::bloom_filter::BloomFilter;
        use crate::failed_deployments_store::LiveFailedDeploymentsStore;
        use crate::sync_deployer::LiveSyncDeployer;
        use axum::extract::Query;
        use std::collections::HashMap;
        use std::sync::{
            Arc, Mutex,
            atomic::{AtomicI64, AtomicU64, Ordering},
        };

        let rows: Vec<_> = scenes("page-cap", 0, &[1_000_000, 1_000_000, 1_000_000, 1_200_000])
            .await
            .into_iter()
            .map(|(row, _, _)| row)
            .collect();
        let cap_last_id = rows[1]["entityId"].as_str().unwrap().to_owned();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let observed = requests.clone();
        let progress = Arc::new(AtomicI64::new(0));
        let observed_progress = progress.clone();
        let app = axum::Router::new().route(
            "/pointer-changes",
            axum::routing::get(move |Query(query): Query<HashMap<String, String>>| {
                let mut requests = observed.lock().unwrap();
                let page = requests.len();
                requests.push((
                    query["from"].clone(),
                    query.get("lastId").cloned(),
                    observed_progress.load(Ordering::Relaxed),
                ));
                let row = rows.get(page).cloned();
                let more = page + 1 < rows.len();
                async move {
                    axum::Json(serde_json::json!({
                        "deltas": row.into_iter().collect::<Vec<_>>(),
                        "pagination": {"moreData": more}
                    }))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://localhost/postgres")
            .unwrap();
        let (storage, tmp) = crate::sync_test_support::temp_storage("pointer-page-cap").await;
        let _cleanup = crate::sync_test_support::RemoveOnDrop(tmp);
        let client = reqwest::Client::new();
        let batch = BatchDeployer::new(
            BatchDeployerConfig::default(),
            crate::sync::test_sync_client(client.clone()),
            storage.clone(),
            pool.clone(),
            Arc::new(LiveSyncDeployer::new(
                pool.clone(),
                storage,
                2,
                crate::sync_deployer::SyncValidation::AuthChainOnly,
            )),
            Arc::new(LiveFailedDeploymentsStore::new(pool)),
            BloomFilter::with_capacity(10),
        );
        let settled_polls = AtomicU64::new(0);
        let result = deploy_pointer_pages::<_, 2>(
            &client,
            &base,
            &PointerChangesOptions {
                from_timestamp: 0,
                wait_time_ms: 0,
                entity_type_filter: Some(&HashSet::new()),
            },
            ScheduleTarget {
                deployer: &batch,
                report: &Arc::new(DeploymentReport::default()),
            },
            None,
            &progress,
            &settled_polls,
            || false,
        )
        .await
        .unwrap();

        server.abort();
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        let (from, last_id, checkpoint) = &requests[2];
        assert_eq!(from, "1000000");
        assert_eq!(last_id.as_deref(), Some(cap_last_id.as_str()));
        assert!((600_000..=700_000).contains(checkpoint), "{checkpoint}");
        assert!(result > *checkpoint);
        assert_eq!(progress.load(Ordering::Relaxed), result);
        assert_eq!(settled_polls.load(Ordering::Relaxed), 2);
    }
}
