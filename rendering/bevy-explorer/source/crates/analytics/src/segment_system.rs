use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use chrono::{DateTime, Utc};
use common::{
    rpc::RpcStreamSender,
    structs::{AppConfig, CurrentRealm, PreviewMode},
    util::{is_loopback_url, reqwest_client, TaskCompat, TaskExt},
};
use std::{collections::VecDeque, time::Duration};
use system_bridge::{AnalyticsEvent, SystemApi};
use uuid::Uuid;

use crate::data_definition::{
    build_segment_event_batch_item, SegmentEvent, SegmentEventCommonExplorerFields,
};
pub struct SegmentMetricPlugin;

impl Plugin for SegmentMetricPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SegmentMetricTimer(Timer::new(
            Duration::from_secs(10),
            TimerMode::Repeating,
        )))
        .init_resource::<SegmentMetricsEvents>()
        .add_systems(
            Update,
            (
                subscribe_analytics_listeners,
                mark_local_scene,
                send_segment_metric_event,
            )
                .chain(),
        );
    }
}

#[derive(Resource)]
pub struct SegmentConfig {
    user_id: String,
    write_key: String,
    common: SegmentEventCommonExplorerFields,
    /// The run was asked for a scene being developed (a preview, the editor, a loopback realm):
    /// what `is_local_scene` means until the realm resolves and can say itself.
    boot_local: bool,
}

#[derive(Resource)]
pub struct SegmentMetricTimer(Timer);

/// An event with the moment and the common fields it was queued with, what it carries when it
/// goes out later: behind the consent gate a queue can hold the whole sign-in, and a batch sent
/// after it must not stamp the pre-login screens with the wallet.
struct QueuedEvent {
    event: SegmentEvent,
    queued_at: DateTime<Utc>,
    message_id: String,
    common: SegmentEventCommonExplorerFields,
}

impl QueuedEvent {
    /// When it happened: the page's time for a page event (which can predate the engine), the
    /// queue time for the engine's own.
    fn happened_at(&self) -> DateTime<Utc> {
        match &self.event {
            SegmentEvent::Page { timestamp, .. } => DateTime::parse_from_rfc3339(timestamp)
                .map(|at| at.to_utc())
                .unwrap_or(self.queued_at),
            _ => self.queued_at,
        }
    }
}

/// The batch being sent, to go out again as it is (same messageIds) until it arrives, Segment
/// refuses it or all of it is older than the warehouse reads.
struct InFlight {
    batch: Vec<String>,
    attempts: u32,
    /// The latest moment in the batch: once it is past the horizon, nothing in the batch is young.
    newest: DateTime<Utc>,
    retry_at: DateTime<Utc>,
}

#[derive(Resource, Default)]
pub struct SegmentMetricsEvents {
    events: VecDeque<QueuedEvent>,
    in_flight: Option<InFlight>,
    overflow_logged: bool,
    /// Batches Segment refused, pacing the log: a wrong key refuses every one.
    rejections: u32,
    /// System scenes watching the engine's events as they are queued
    /// (`SystemApi::GetAnalyticsEventStream`).
    listeners: Vec<RpcStreamSender<AnalyticsEvent>>,
}

/// Events held while consent is pending or sends fail; past this the oldest go.
const MAX_QUEUED_EVENTS: usize = 2000;
/// A failed send waits this before the next, twice as long each time, up to the maximum.
const RETRY_DELAY: chrono::Duration = chrono::Duration::seconds(10);
const MAX_RETRY_DELAY: chrono::Duration = chrono::Duration::minutes(10);
/// The warehouse reads two days back: an event held longer (the consent never given, a page
/// that never reached the engine) would land where no model looks.
const MAX_EVENT_AGE: chrono::Duration = chrono::Duration::hours(36);
/// A send that has not answered by then has hung (the web client has no timeout of its own);
/// its batch is retried like any failed one.
const SEND_TIMEOUT: Duration = Duration::from_secs(15);
/// Rejections per error line after the first.
const REJECTIONS_PER_LOG: u32 = 30;

/// The Segment source this run reports to, `prod` or `dev`: prod only against Decentraland's
/// production site. Natively that is the resolved base domain (`--base-domain`, `decentraland.org`
/// by default) in a release build, so a debug run and a build pointed at the zone staging domain
/// are dev. The web engine is always a release build and one npm version serves every site, so
/// there the page's host decides.
pub fn environment() -> &'static str {
    #[cfg(not(target_arch = "wasm32"))]
    let production = !cfg!(debug_assertions) && is_production_site(common::base_domain::get());
    #[cfg(target_arch = "wasm32")]
    let production = page_host().is_some_and(|host| is_production_site(&host));
    if production {
        "prod"
    } else {
        "dev"
    }
}

/// Decentraland's production domain or a host under it, keyed as react-web/src/lib/baseDomain.ts
/// `hostBaseDomain` keys the deployments.
fn is_production_site(host: &str) -> bool {
    host == "decentraland.org" || host.ends_with(".decentraland.org")
}

/// The host of the page running the engine, from the engine worker's own location.
#[cfg(target_arch = "wasm32")]
fn page_host() -> Option<String> {
    let location = js_sys::Reflect::get(&js_sys::global(), &"location".into()).ok()?;
    js_sys::Reflect::get(&location, &"hostname".into())
        .ok()?
        .as_string()
}

const SHARED_WRITE_KEY: &str = "EAdAcIyGP6lIQAfpFF2BXpNzpj7XNWMm";
// Both sources' keys are compiled in (apps/bevy-explorer/README.md); the environment picks.
const PROD_WRITE_KEY: Option<&str> = option_env!("BEVY_SEGMENT_WRITE_KEY_PROD");
const DEV_WRITE_KEY: Option<&str> = option_env!("BEVY_SEGMENT_WRITE_KEY_DEV");

fn write_key() -> &'static str {
    configured_key(environment(), PROD_WRITE_KEY, DEV_WRITE_KEY)
}

/// The environment's configured key, else the key every environment shared before the sources
/// were split. Empty counts as unset: CI exports an undefined variable as "".
fn configured_key(
    environment: &str,
    prod: Option<&'static str>,
    dev: Option<&'static str>,
) -> &'static str {
    let configured = if environment == "prod" { prod } else { dev };
    configured
        .filter(|key| !key.is_empty())
        .unwrap_or(SHARED_WRITE_KEY)
}

// The browser build goes through the first-party proxy, which ad blockers leave alone.
#[cfg(target_arch = "wasm32")]
pub(crate) const SEGMENT_BATCH_URL: &str = "https://api.e.decentraland.org/v1/batch";
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const SEGMENT_BATCH_URL: &str = "https://api.segment.io/v1/batch";
const SEGMENT_EVENT_SIZE_LIMIT_BYTES: usize = 32000;
const SEGMENT_BATCH_SIZE_LIMIT_BYTES: usize = 500000;

impl SegmentMetricsEvents {
    pub fn add_event(&mut self, event: SegmentEvent, common: &SegmentEventCommonExplorerFields) {
        self.add_event_with_id(event, common, Uuid::new_v4().to_string());
    }

    /// Queue an event under the messageId its sender minted: the page's own events keep theirs
    /// across a hand-over it has to repeat.
    pub(crate) fn add_event_with_id(
        &mut self,
        event: SegmentEvent,
        common: &SegmentEventCommonExplorerFields,
        message_id: String,
    ) {
        if self.events.len() >= MAX_QUEUED_EVENTS {
            self.events.pop_front();
            if !self.overflow_logged {
                warn!("segment queue exceeded {MAX_QUEUED_EVENTS} events; dropping the oldest");
                self.overflow_logged = true;
            }
        }
        if !self.listeners.is_empty() {
            if let Some((event, properties)) = event.name_and_properties() {
                self.listeners.retain(|l| !l.is_closed());
                for listener in &self.listeners {
                    let _ = listener.send(AnalyticsEvent {
                        event: event.clone(),
                        properties: properties.clone(),
                    });
                }
            }
        }
        self.events.push_back(QueuedEvent {
            event,
            queued_at: Utc::now(),
            message_id,
            common: common.clone(),
        });
    }

    pub fn add_listener(&mut self, listener: RpcStreamSender<AnalyticsEvent>) {
        self.listeners.push(listener);
    }

    /// The next batch to send: the failed one again once its wait is over (nothing while it
    /// waits), else the oldest queued events that fit Segment's limit, each serialized with the
    /// fields it was queued with. One older than the warehouse looks back is dropped here, as
    /// late as it can be.
    pub(crate) fn next_batch(&mut self, user_id: &str, now: DateTime<Utc>) -> Vec<String> {
        if let Some(in_flight) = &self.in_flight {
            return if now >= in_flight.retry_at {
                in_flight.batch.clone()
            } else {
                Vec::new()
            };
        }
        let mut batch = Vec::new();
        let mut size = 0;
        let mut newest = DateTime::<Utc>::MIN_UTC;
        while let Some(queued) = self.events.front() {
            let happened_at = queued.happened_at();
            if now - happened_at > MAX_EVENT_AGE {
                self.events.pop_front();
                continue;
            }
            let item = build_segment_event_batch_item(
                user_id.to_owned(),
                &queued.common,
                &queued.event,
                queued.queued_at,
                queued.message_id.clone(),
            );
            let json = serde_json::to_string(&item).expect("Failed to serialize event body");
            if json.len() > SEGMENT_EVENT_SIZE_LIMIT_BYTES {
                error!("Event too large: {}", json.len());
                self.events.pop_front();
                continue;
            }
            if size + json.len() > SEGMENT_BATCH_SIZE_LIMIT_BYTES {
                break;
            }
            size += json.len();
            newest = newest.max(happened_at);
            batch.push(json);
            self.events.pop_front();
        }
        if !batch.is_empty() {
            self.in_flight = Some(InFlight {
                batch: batch.clone(),
                attempts: 0,
                newest,
                retry_at: now,
            });
        }
        batch
    }

    fn sent(&mut self) {
        self.in_flight = None;
    }

    /// Segment refused the batch: it would never get better, the next one goes out instead.
    fn send_rejected(&mut self) {
        self.in_flight = None;
        self.rejections += 1;
    }

    /// The send failed: the batch goes out again after a wait that doubles each time, while any
    /// of it is still young enough to be read.
    fn send_failed(&mut self, now: DateTime<Utc>) {
        let Some(in_flight) = self.in_flight.as_mut() else {
            return;
        };
        if now - in_flight.newest > MAX_EVENT_AGE {
            warn!(
                "dropping a segment batch of {} events after {} failed sends: older than the \
                 warehouse reads",
                in_flight.batch.len(),
                in_flight.attempts + 1
            );
            self.in_flight = None;
            return;
        }
        in_flight.attempts += 1;
        let doublings = (in_flight.attempts - 1).min(16);
        in_flight.retry_at = now + (RETRY_DELAY * (1 << doublings)).min(MAX_RETRY_DELAY);
    }
}

fn subscribe_analytics_listeners(
    mut requests: EventReader<SystemApi>,
    mut metrics: ResMut<SegmentMetricsEvents>,
) {
    for request in requests.read() {
        if let SystemApi::GetAnalyticsEventStream(sender) = request {
            metrics.add_listener(sender.clone());
        }
    }
}

impl SegmentConfig {
    pub fn new(
        user_id: String,
        session_id: String,
        version: String,
        campaign_anon_user_id: Option<String>,
        boot_local: bool,
    ) -> Self {
        let mut common =
            SegmentEventCommonExplorerFields::new(session_id, version, campaign_anon_user_id);
        common.is_local_scene = boot_local;
        Self {
            user_id,
            common,
            write_key: write_key().into(),
            boot_local,
        }
    }

    pub fn common(&self) -> &SegmentEventCommonExplorerFields {
        &self.common
    }

    pub(crate) fn user_id(&self) -> &str {
        &self.user_id
    }

    pub(crate) fn write_key(&self) -> &str {
        &self.write_key
    }

    pub fn update_realm(&mut self, realm: String) {
        self.common.realm = realm;
    }

    pub fn update_identity(&mut self, dcl_eth_address: String, dcl_is_guest: bool) {
        self.common.dcl_eth_address = dcl_eth_address;
        self.common.dcl_is_guest = dcl_is_guest;
    }

    /// Signed out: what comes next is nobody's, as before the sign-in.
    pub fn reset_identity(&mut self) {
        self.common.reset_identity();
    }

    pub fn update_position(&mut self, position: String) {
        self.common.position = position;
    }
}

/// A scene being developed: the sdk's or an editor's preview (`preview=true`, `--preview`), or a
/// local realm (a loopback address, a dev server listing its parcels in `about`); until the realm
/// resolves, what the run was asked for.
fn mark_local_scene(
    preview: Res<PreviewMode>,
    realm: Option<Res<CurrentRealm>>,
    mut config: ResMut<SegmentConfig>,
) {
    if !preview.is_changed() && !realm.as_ref().is_some_and(|realm| realm.is_changed()) {
        return;
    }
    // the resource exists from the start; its about url is empty until a realm resolves
    let local = preview.is_preview
        || match realm.filter(|realm| !realm.about_url.is_empty()) {
            Some(realm) => realm.is_local() || is_loopback_url(&realm.about_url),
            None => config.boot_local,
        };
    if config.common.is_local_scene != local {
        config.common.is_local_scene = local;
    }
}

fn send_segment_metric_event(
    time: Res<Time>,
    mut timer: ResMut<SegmentMetricTimer>,
    mut metrics: ResMut<SegmentMetricsEvents>,
    config: Res<SegmentConfig>,
    app_config: Res<AppConfig>,
    mut send_task: Local<Option<Task<Result<(), SendError>>>>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }
    let now = Utc::now();
    if let Some(mut t) = send_task.take() {
        match t.complete() {
            Some(Ok(_)) => metrics.sent(),
            Some(Err(SendError::Rejected { status, body })) => {
                metrics.send_rejected();
                // Segment's answer names the problem (a wrong key refuses every batch), once and
                // then now and again rather than every tick
                if metrics.rejections == 1 || metrics.rejections.is_multiple_of(REJECTIONS_PER_LOG)
                {
                    error!(
                        "segment rejected a batch with status {status} ({} rejected so far); \
                         dropping it: {body}",
                        metrics.rejections
                    );
                }
            }
            Some(Err(SendError::Transport(e))) => {
                error!("send_segment_metric_event task error: {e}");
                metrics.send_failed(now);
            }
            None => {
                *send_task = Some(t);
                return;
            }
        }
    }

    // nothing leaves before the user accepted the terms; events keep queuing meanwhile
    if !app_config.terms_accepted() {
        return;
    }

    let batch = metrics.next_batch(config.user_id(), now);
    if batch.is_empty() {
        return;
    }
    let write_key = config.write_key.clone();
    *send_task = Some(
        IoTaskPool::get().spawn_compat(async move { send_segment_batch(&write_key, &batch).await }),
    );
}

pub(crate) fn batch_body(write_key: &str, events: &[String]) -> String {
    format!(
        "{{\"writeKey\":\"{}\",\"batch\":[{}]}}",
        write_key,
        events.join(",")
    )
}

/// Why a batch did not arrive: the transport failed (worth another send), or Segment refused it,
/// with the start of its answer: a bad key or body never gets better.
enum SendError {
    Transport(anyhow::Error),
    Rejected { status: u16, body: String },
}

/// A status worth another send: a timeout or a rate limit (408, 429) and anything that is not
/// Segment refusing the request itself (another 4xx).
fn is_transient(status: u16) -> bool {
    !(400..500).contains(&status) || matches!(status, 408 | 429)
}

async fn send_segment_batch(write_key: &str, events: &[String]) -> Result<(), SendError> {
    let json_body = batch_body(write_key, events);

    let response = reqwest_client()
        .post(SEGMENT_BATCH_URL)
        .header("Content-Type", "application/json")
        .body(json_body.clone())
        .timeout(SEND_TIMEOUT)
        .send()
        .await
        .map_err(|e| SendError::Transport(e.into()))?;

    let status = response.status();
    if status.is_success() {
        debug!("successfully sent segment event, status: {status}, payload  {json_body}");
        Ok(())
    } else if is_transient(status.as_u16()) {
        Err(SendError::Transport(anyhow::anyhow!(
            "Failed to send segment event: status {status}"
        )))
    } else {
        let body = response.text().await.unwrap_or_default();
        Err(SendError::Rejected {
            status: status.as_u16(),
            body: body.chars().take(300).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_definition::{SegmentEventFirstMoveInWorld, SegmentEventTimeSpentInWorld};

    // A listening system scene sees the engine's events as they are queued, by their Segment
    // name, but not the page's own events coming back through `/track`.
    #[test]
    fn listeners_get_engine_events_only() {
        let common = common();
        let mut metrics = SegmentMetricsEvents::default();
        let (sender, mut receiver) = RpcStreamSender::channel();
        metrics.add_listener(sender);
        metrics.add_event(
            SegmentEvent::Page {
                name: "Screen Viewed".into(),
                properties: Default::default(),
                timestamp: "2026-07-02T13:38:08.243Z".into(),
            },
            &common,
        );
        metrics.add_event(
            SegmentEvent::FirstMoveInWorld(SegmentEventFirstMoveInWorld {}),
            &common,
        );
        let got = receiver.try_recv().unwrap();
        assert_eq!(got.event, "First Move In World");
        assert_eq!(got.properties, serde_json::json!({}));
        assert!(receiver.try_recv().is_err());
        assert_eq!(metrics.events.len(), 2);

        // a listener whose scene has gone is dropped on the next engine event
        drop(receiver);
        metrics.add_event(
            SegmentEvent::FirstMoveInWorld(SegmentEventFirstMoveInWorld {}),
            &common,
        );
        assert!(metrics.listeners.is_empty());
    }

    fn common() -> SegmentEventCommonExplorerFields {
        SegmentEventCommonExplorerFields::new("s".into(), "1.0".into(), None)
    }

    fn batch_items(
        metrics: &mut SegmentMetricsEvents,
        now: DateTime<Utc>,
    ) -> Vec<serde_json::Value> {
        metrics
            .next_batch("user", now)
            .iter()
            .map(|item| serde_json::from_str(item).unwrap())
            .collect()
    }

    fn first_move(metrics: &mut SegmentMetricsEvents, common: &SegmentEventCommonExplorerFields) {
        metrics.add_event(
            SegmentEvent::FirstMoveInWorld(SegmentEventFirstMoveInWorld {}),
            common,
        );
    }

    // The sign-in funnel is queued anonymous and leaves after the login: each event goes out with
    // the identity and realm of its own moment, under a messageId of its own.
    #[test]
    fn a_queued_event_keeps_the_fields_of_its_moment() {
        let mut common = common();
        let mut metrics = SegmentMetricsEvents::default();
        first_move(&mut metrics, &common);
        common.dcl_eth_address = "0xabc".into();
        common.realm = "https://peer.decentraland.org/".into();
        first_move(&mut metrics, &common);

        let batch = batch_items(&mut metrics, Utc::now());
        assert_eq!(batch[0]["properties"]["dcl_eth_address"], "unauthenticated");
        assert_eq!(batch[0]["properties"]["realm"], "no-realm");
        assert_eq!(batch[1]["properties"]["dcl_eth_address"], "0xabc");
        assert_eq!(
            batch[1]["properties"]["realm"],
            "https://peer.decentraland.org/"
        );
        assert!(matches!(
            batch[0]["properties"]["environment"].as_str(),
            Some("prod" | "dev")
        ));
        assert!(Uuid::parse_str(batch[0]["messageId"].as_str().unwrap()).is_ok());
        assert_ne!(batch[0]["messageId"], batch[1]["messageId"]);
        metrics.sent();
        assert!(metrics.next_batch("user", Utc::now()).is_empty());
    }

    // After a sign-out the events are nobody's again, as before the sign-in.
    #[test]
    fn the_signed_out_wallet_is_not_on_later_events() {
        let mut config = SegmentConfig::new("user".into(), "s".into(), "1.0".into(), None, false);
        let mut metrics = SegmentMetricsEvents::default();
        config.update_identity("0xabc".into(), false);
        first_move(&mut metrics, config.common());
        config.reset_identity();
        first_move(&mut metrics, config.common());

        let batch = batch_items(&mut metrics, Utc::now());
        assert_eq!(batch[0]["properties"]["dcl_eth_address"], "0xabc");
        assert_eq!(batch[1]["properties"]["dcl_eth_address"], "unauthenticated");
        assert_eq!(batch[1]["properties"]["dcl_is_guest"], false);
    }

    // A boot asked for a scene being developed says so before the realm resolves (the realm
    // resource exists, empty, from the first frame); once it does, the realm says.
    #[test]
    fn a_loopback_boot_is_a_local_scene_until_the_realm_says() {
        let mut app = App::new();
        app.init_resource::<PreviewMode>()
            .init_resource::<CurrentRealm>()
            .insert_resource(SegmentConfig::new(
                "user".into(),
                "s".into(),
                "1.0".into(),
                None,
                true,
            ))
            .add_systems(Update, mark_local_scene);
        let local = |app: &App| {
            app.world()
                .resource::<SegmentConfig>()
                .common()
                .is_local_scene
        };
        app.update();
        assert!(local(&app));
        app.world_mut().resource_mut::<CurrentRealm>().about_url =
            "http://127.0.0.1:8000/about".into();
        app.update();
        assert!(local(&app));
        app.world_mut().resource_mut::<CurrentRealm>().about_url =
            "https://peer.decentraland.org/about".into();
        app.update();
        assert!(!local(&app));
    }

    // The environment's key when the build has one, the shared key otherwise; an empty value is
    // what CI exports for a variable nobody set.
    #[test]
    fn the_environment_picks_its_configured_key() {
        assert_eq!(configured_key("prod", Some("p"), Some("d")), "p");
        assert_eq!(configured_key("dev", Some("p"), Some("d")), "d");
        assert_eq!(
            configured_key("prod", Some(""), Some("d")),
            SHARED_WRITE_KEY
        );
        assert_eq!(configured_key("dev", None, None), SHARED_WRITE_KEY);
    }

    // Only Decentraland's production site is prod: the page's host on the web, the resolved base
    // domain natively.
    #[test]
    fn only_the_production_site_is_prod() {
        assert!(is_production_site("decentraland.org"));
        assert!(is_production_site("play.decentraland.org"));
        assert!(!is_production_site("play.decentraland.zone"));
        assert!(!is_production_site("localhost"));
        assert!(!is_production_site("decentraland.org.example.com"));
    }

    // A batch Segment refused is dropped, and the next one is not held behind it.
    #[test]
    fn a_rejected_batch_is_dropped_and_the_next_goes_out() {
        let common = common();
        let mut metrics = SegmentMetricsEvents::default();
        first_move(&mut metrics, &common);
        let rejected = metrics.next_batch("user", Utc::now());
        metrics.send_rejected();
        first_move(&mut metrics, &common);
        let next = metrics.next_batch("user", Utc::now());
        assert_eq!(next.len(), 1);
        assert_ne!(next, rejected);
    }

    // Only Segment refusing the request itself (a 4xx) is final; a timeout, a rate limit or a
    // server error is sent again.
    #[test]
    fn a_timeout_a_rate_limit_and_a_server_error_are_retried_and_another_4xx_is_not() {
        for status in [408, 429, 500, 502, 503] {
            assert!(is_transient(status), "{status}");
        }
        for status in [400, 401, 403, 404, 413] {
            assert!(!is_transient(status), "{status}");
        }
    }

    // A send that failed goes out again as it was, same messageIds, after a wait that doubles
    // from 10 s up to 10 min; once all of it is older than the warehouse reads it is dropped and
    // what queued behind it goes out.
    #[test]
    fn a_failed_batch_is_resent_unchanged_after_a_growing_wait_until_it_is_stale() {
        let common = common();
        let mut metrics = SegmentMetricsEvents::default();
        let t0 = Utc::now();
        first_move(&mut metrics, &common);
        let batch = metrics.next_batch("user", t0);
        assert_eq!(batch.len(), 1);

        let second = chrono::Duration::seconds(1);
        let mut now = t0;
        for wait in [10, 20, 40, 80, 160, 320, 600, 600] {
            metrics.send_failed(now);
            first_move(&mut metrics, &common);
            let wait = chrono::Duration::seconds(wait);
            assert!(metrics.next_batch("user", now + wait - second).is_empty());
            now += wait;
            assert_eq!(metrics.next_batch("user", now), batch);
        }

        let late = t0 + MAX_EVENT_AGE + second;
        metrics.add_event(
            SegmentEvent::Page {
                name: "Screen Viewed".into(),
                properties: Default::default(),
                timestamp: late.to_rfc3339(),
            },
            &common,
        );
        metrics.send_failed(late);
        let next = batch_items(&mut metrics, late);
        assert_eq!(next.len(), 1);
        assert_eq!(next[0]["event"], "Screen Viewed");
    }

    // Held past what the warehouse reads, an event is dropped rather than sent late: the engine's
    // by its queue time, the page's by the time it reported.
    #[test]
    fn an_event_older_than_the_warehouse_reads_is_dropped() {
        let common = common();
        let mut metrics = SegmentMetricsEvents::default();
        let now = Utc::now();
        let page = |hours_ago: i64| SegmentEvent::Page {
            name: "Screen Viewed".into(),
            properties: Default::default(),
            timestamp: (now - chrono::Duration::hours(hours_ago)).to_rfc3339(),
        };
        metrics.add_event(page(37), &common);
        metrics.add_event(page(35), &common);
        first_move(&mut metrics, &common);
        let sent = batch_items(&mut metrics, now);
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0]["event"], "Screen Viewed");
        assert_eq!(sent[1]["event"], "First Move In World");

        metrics.sent();
        first_move(&mut metrics, &common);
        assert!(metrics
            .next_batch("user", now + chrono::Duration::hours(37))
            .is_empty());
    }

    // Held events go out oldest first; past the cap the oldest are the ones dropped.
    #[test]
    fn queue_is_fifo_and_capped() {
        let common = common();
        let mut metrics = SegmentMetricsEvents::default();
        for i in 0..=MAX_QUEUED_EVENTS {
            metrics.add_event(
                SegmentEvent::TimeSpentInWorld(SegmentEventTimeSpentInWorld {
                    time_spent: i as f32,
                    realm_name: "main".into(),
                }),
                &common,
            );
        }
        assert_eq!(metrics.events.len(), MAX_QUEUED_EVENTS);
        let spent = |q: &QueuedEvent| match &q.event {
            SegmentEvent::TimeSpentInWorld(e) => e.time_spent,
            _ => unreachable!(),
        };
        assert_eq!(spent(metrics.events.front().unwrap()), 1.0);
        assert_eq!(
            spent(metrics.events.back().unwrap()),
            MAX_QUEUED_EVENTS as f32
        );
        assert!(metrics.overflow_logged);
    }
}
