//! Runtime hooks for the statistics and connector HTTP hosts.
use super::{
    content,
    http::{Hooks, Lifecycle, Observation},
};
use crate::archipelago::AppState;
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::Ordering;

#[path = "process_metrics.rs"]
mod process;

struct Registry {
    recorder: metrics_exporter_prometheus::PrometheusRecorder,
    next_reset_ms: i64,
    feed: [u64; 3],
    cpu_origin: Option<[f64; 2]>,
}
/// Independent real registry for one public HTTP host.
pub struct Metrics {
    registry: Mutex<Registry>,
    collect_default: bool,
    reset_at_night: bool,
}
impl Metrics {
    pub fn from_env() -> Arc<Self> {
        Self::configured(
            std::env::var("WKC_METRICS_COLLECT_DEFAULT").as_deref() != Ok("false"),
            std::env::var("WKC_METRICS_RESET_AT_NIGHT").as_deref() == Ok("true"),
        )
    }
    fn configured(collect_default: bool, reset_at_night: bool) -> Arc<Self> {
        Self::configured_with_sampler(collect_default, reset_at_night, process::collect)
    }
    fn configured_with_sampler(
        collect_default: bool,
        reset_at_night: bool,
        sample: impl FnOnce() -> process::Sample,
    ) -> Arc<Self> {
        Arc::new(Self {
            registry: Mutex::new(Registry {
                recorder: recorder(),
                next_reset_ms: next_reset(chrono::Local::now()),
                feed: [0; 3],
                cpu_origin: collect_default.then(sample).and_then(|sample| sample.cpu),
            }),
            collect_default,
            reset_at_night,
        })
    }
    pub fn observe_connector(&self, mut observation: Observation) {
        // Native connector HTTP accepts arbitrary tokens. Preserve its responses
        // while allowing only known method labels into this independent registry.
        if !super::http::NODE_METHODS
            .iter()
            .any(|method| observation.method.eq_ignore_ascii_case(method))
        {
            return;
        }
        observation.method.make_ascii_lowercase();
        self.record(observation, false);
    }
    fn record(&self, observation: Observation, size: bool) {
        let registry = self.registry.lock();
        metrics::with_local_recorder(&registry.recorder, || {
            let labels = [
                ("method", observation.method),
                ("handler", observation.handler),
                ("code", observation.code.to_string()),
            ];
            metrics::counter!("http_requests_total", &labels).increment(1);
            metrics::histogram!("http_request_duration_seconds", &labels)
                .record(observation.duration.as_secs_f64());
            if size {
                metrics::histogram!("http_request_size_bytes", &labels)
                    .record(observation.request_size);
            }
        });
        // Drain raw histogram samples at the recording boundary, including when scrapes
        // are disabled or rejected. This registry owns no detached upkeep task.
        registry.recorder.handle().run_upkeep();
    }
    fn scrape<Tz: chrono::TimeZone>(
        &self,
        clock: impl FnMut() -> chrono::DateTime<Tz>,
        feed: Option<[u64; 3]>,
    ) -> String {
        self.scrape_with_sampler(clock, feed, process::collect)
    }
    fn scrape_with_sampler<Tz: chrono::TimeZone>(
        &self,
        mut clock: impl FnMut() -> chrono::DateTime<Tz>,
        feed: Option<[u64; 3]>,
        sample: impl FnOnce() -> process::Sample,
    ) -> String {
        let process = self.collect_default.then(sample);
        let mut registry = self.registry.lock();
        metrics::with_local_recorder(&registry.recorder, || {
            if let Some(feed) = feed {
                for (index, name) in [
                    "dcl_ws_connector_island_changed_no_session_socket_total",
                    "dcl_ws_connector_island_changed_deduplicated_total",
                    "dcl_ws_connector_island_changed_dropped_close_total",
                ]
                .iter()
                .enumerate()
                {
                    metrics::counter!(*name)
                        .increment(feed[index].saturating_sub(registry.feed[index]));
                }
            }
        });
        if let Some(feed) = feed {
            // Concurrent blocking scrapes can arrive with an older sample.
            // Keep each absolute source baseline monotonic, including across resets.
            for (previous, current) in registry.feed.iter_mut().zip(feed) {
                *previous = (*previous).max(current);
            }
        }
        let mut body = registry.recorder.handle().render();
        declared_metadata(&mut body, feed.is_some());
        if let Some(sample) = process {
            sample.append(&mut body, &mut registry.cpu_origin);
        }
        if self.reset_at_night && clock().timestamp_millis() > registry.next_reset_ms {
            registry.next_reset_ms = next_reset(clock());
            registry.recorder = recorder();
            registry.cpu_origin = process.and_then(|sample| sample.cpu);
        }
        body
    }
}
/// Lifecycle hooks retain the actual connector; metric registries carry no state cycle.
pub struct Runtime {
    connector: AppState,
    metrics: Arc<Metrics>,
    connector_host: bool,
}
impl Runtime {
    pub fn new(connector: AppState) -> Arc<Self> {
        Arc::new(Self {
            connector,
            metrics: Metrics::from_env(),
            connector_host: false,
        })
    }
    pub fn connector(connector: AppState) -> Arc<Self> {
        Arc::new(Self {
            metrics: connector.connector_metrics.clone(),
            connector,
            connector_host: true,
        })
    }
    #[cfg(test)]
    fn configured(connector: AppState, collect: bool, reset: bool) -> Arc<Self> {
        Arc::new(Self {
            connector,
            metrics: Metrics::configured(collect, reset),
            connector_host: false,
        })
    }
    pub fn observe_connector(&self, observation: Observation) {
        self.metrics.observe_connector(observation);
    }
    fn feed(&self) -> Option<[u64; 3]> {
        self.connector_host.then(|| {
            [
                self.connector.feed.no_session_socket_count(),
                self.connector.feed.deduplicated_count(),
                self.connector.feed.dropped_close_count(),
            ]
        })
    }
    #[cfg(test)]
    fn scrape<Tz: chrono::TimeZone>(&self, clock: impl FnMut() -> chrono::DateTime<Tz>) -> String {
        self.metrics.scrape(clock, self.feed())
    }
}

fn next_reset<Tz: chrono::TimeZone>(now: chrono::DateTime<Tz>) -> i64 {
    use chrono::{Duration, Offset, TimeZone};
    let timezone = now.timezone();
    let midnight = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
    if let Some(start) = timezone.from_local_datetime(&midnight).earliest() {
        return start.timestamp_millis() + 86_400_000;
    }
    // A local clock gap uses the offset immediately before the jump, retaining
    // midnight's position within the gap even when it began the previous evening.
    // Whole-second offsets are strictly within ±24h, bounding a gap below 48h.
    for seconds in 1..=172_800 {
        let Some(local) = midnight.checked_sub_signed(Duration::seconds(seconds)) else {
            break;
        };
        let Some(before) = timezone.from_local_datetime(&local).latest() else {
            continue;
        };
        let pre_offset = before.offset().fix();
        let candidate = pre_offset
            .from_local_datetime(&midnight)
            .unwrap()
            .with_timezone(&timezone);
        let after = timezone.from_utc_datetime(&(before.naive_utc() + Duration::seconds(1)));
        let shift = i64::from(after.offset().fix().local_minus_utc())
            - i64::from(pre_offset.local_minus_utc());
        if shift > 0
            && after.naive_local() > midnight
            && candidate.offset().fix() == after.offset().fix()
            && candidate.naive_local() == midnight + Duration::seconds(shift)
        {
            return candidate.timestamp_millis() + 86_400_000;
        }
        break;
    }
    // A failed timezone lookup is not evidence of a clock gap. Retain the
    // operational fallback when no forward transition can be established.
    now.timestamp_millis() + 86_400_000
}

fn declared_metadata(body: &mut String, connector: bool) {
    for (name, kind, help) in [
        (
            "http_request_duration_seconds",
            "histogram",
            "Request duration in seconds.",
        ),
        (
            "http_requests_total",
            "counter",
            "Total number of HTTP requests",
        ),
        (
            "http_request_size_bytes",
            "histogram",
            if connector {
                "Size of HTTP requests in bytes"
            } else {
                "Duration of HTTP requests size in bytes"
            },
        ),
        (
            "wkc_logger_logs_total",
            "counter",
            "Log amounts per logger and level",
        ),
    ] {
        body.push_str(&format!("# HELP {name} {help}\n"));
        if !body.contains(&format!("# TYPE {name} ")) {
            body.push_str(&format!("# TYPE {name} {kind}\n"));
        }
    }
    if connector {
        for (name, help) in [
            (
                "dcl_ws_connector_island_changed_no_session_socket_total",
                "Total session-addressed island_changed messages for a wallet this replica holds, but not under that session",
            ),
            (
                "dcl_ws_connector_island_changed_deduplicated_total",
                "Total island_changed messages dropped because the same island was forwarded to the same socket within ISLAND_CHANGED_DEDUP_MS",
            ),
            (
                "dcl_ws_connector_island_changed_dropped_close_total",
                "Total sockets closed with 1013 because µWebSockets dropped an island_changed frame under backpressure; each one reconnects and re-authenticates",
            ),
        ] {
            body.push_str(&format!("# HELP {name} {help}\n"));
        }
    }
}

fn recorder() -> metrics_exporter_prometheus::PrometheusRecorder {
    crate::metrics::prometheus_builder()
        .expect("static metric bucket configuration")
        .build_recorder()
}
#[async_trait::async_trait]
impl Hooks for Runtime {
    async fn probe(&self, startup: bool) -> Lifecycle {
        let state = &self.connector;
        Lifecycle {
            initialized: true,
            live: !state.restart.is_cancelled(),
            started: state.started.load(Ordering::Acquire),
            // Original server.startupProbe succeeds after construction; readiness is listening.
            checks: vec![(
                "server".into(),
                startup || (state.started.load(Ordering::Acquire) && !state.restart.is_cancelled()),
            )],
        }
    }
    async fn metrics(&self) -> Result<String, content::Error> {
        let metrics = self.metrics.clone();
        let feed = self.feed();
        tokio::task::spawn_blocking(move || metrics.scrape(chrono::Local::now, feed))
            .await
            .map_err(|error| {
                tracing::warn!(%error, "statistics metrics collection failed");
                content::Error
            })
    }
    async fn diagnostics(&self) -> Result<String, content::Error> {
        Ok(crate::archipelago::handlers::diagnostics(&self.connector))
    }
    fn observe(&self, observation: Observation) {
        self.metrics.record(observation, true);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{
        DateTime, Duration, FixedOffset, LocalResult, NaiveDate, NaiveDateTime, Offset, TimeZone,
        Utc,
    };
    use std::sync::atomic::AtomicUsize;

    #[derive(Clone, Debug)]
    struct TransitionZone {
        transition: NaiveDateTime,
        before: i32,
        after: i32,
        unavailable: bool,
        missing_midnight: bool,
        local_queries: Arc<AtomicUsize>,
        utc_queries: Arc<AtomicUsize>,
    }
    #[derive(Clone, Debug)]
    struct TransitionOffset {
        zone: TransitionZone,
        seconds: i32,
    }
    impl Offset for TransitionOffset {
        fn fix(&self) -> FixedOffset {
            FixedOffset::east_opt(self.seconds).unwrap()
        }
    }
    impl TransitionZone {
        fn new(transition: NaiveDateTime, before: i32, after: i32) -> Self {
            Self {
                transition,
                before,
                after,
                unavailable: false,
                missing_midnight: false,
                local_queries: Arc::new(AtomicUsize::new(0)),
                utc_queries: Arc::new(AtomicUsize::new(0)),
            }
        }
        fn offset(&self, seconds: i32) -> TransitionOffset {
            TransitionOffset {
                zone: self.clone(),
                seconds,
            }
        }
    }
    impl TimeZone for TransitionZone {
        type Offset = TransitionOffset;
        fn from_offset(offset: &Self::Offset) -> Self {
            offset.zone.clone()
        }
        fn offset_from_local_date(&self, date: &NaiveDate) -> LocalResult<Self::Offset> {
            self.offset_from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
        }
        fn offset_from_utc_date(&self, date: &NaiveDate) -> Self::Offset {
            self.offset_from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
        }
        fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> LocalResult<Self::Offset> {
            self.local_queries.fetch_add(1, Ordering::Relaxed);
            if self.unavailable || (self.missing_midnight && local.time() == chrono::NaiveTime::MIN)
            {
                return LocalResult::None;
            }
            let before = *local - Duration::seconds(i64::from(self.before)) < self.transition;
            let after = *local - Duration::seconds(i64::from(self.after)) >= self.transition;
            match (before, after) {
                (true, true) => {
                    LocalResult::Ambiguous(self.offset(self.before), self.offset(self.after))
                }
                (true, false) => LocalResult::Single(self.offset(self.before)),
                (false, true) => LocalResult::Single(self.offset(self.after)),
                (false, false) => LocalResult::None,
            }
        }
        fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> Self::Offset {
            self.utc_queries.fetch_add(1, Ordering::Relaxed);
            self.offset(if *utc < self.transition {
                self.before
            } else {
                self.after
            })
        }
    }
    fn instant(value: &str) -> DateTime<Utc> {
        value.parse().unwrap()
    }

    #[test]
    fn midnight_reset_resolves_gaps_with_pre_gap_offset_and_ambiguity_with_earlier_instant() {
        for (transition, before, after, now, midnight, queries) in [
            (
                "2024-04-01T00:00:00Z",
                0,
                3600,
                "2024-04-01T11:00:00Z",
                "2024-04-01T00:00:00Z",
                2,
            ),
            (
                "2024-03-31T23:30:00Z",
                0,
                3600,
                "2024-04-01T11:00:00Z",
                "2024-04-01T00:00:00Z",
                1802,
            ),
            (
                "2024-04-01T00:00:00Z",
                3600,
                0,
                "2024-04-01T12:00:00Z",
                "2024-03-31T23:00:00Z",
                1,
            ),
            (
                "2024-04-01T00:00:00Z",
                19800,
                19800,
                "2024-04-01T06:30:00Z",
                "2024-03-31T18:30:00Z",
                1,
            ),
            (
                "2024-04-01T00:00:00Z",
                0,
                0,
                "2024-04-01T12:00:00Z",
                "2024-04-01T00:00:00Z",
                1,
            ),
        ] {
            let zone = TransitionZone::new(instant(transition).naive_utc(), before, after);
            let now = instant(now).with_timezone(&zone);
            zone.utc_queries.store(0, Ordering::Relaxed);
            let started = std::time::Instant::now();
            let threshold = next_reset(now);
            let elapsed = started.elapsed();
            let local_queries = zone.local_queries.load(Ordering::Relaxed);
            let utc_queries = zone.utc_queries.load(Ordering::Relaxed);
            eprintln!(
                "midnight transition={transition} offsets={before}/{after}: local={local_queries} utc={utc_queries} elapsed={elapsed:?}"
            );
            assert_eq!(threshold, instant(midnight).timestamp_millis() + 86_400_000);
            assert_eq!(local_queries, queries);
        }
    }

    #[test]
    fn midnight_reset_bounds_unresolved_timezone_lookups_without_claiming_a_gap() {
        let mut zone = TransitionZone::new(instant("2024-04-01T00:00:00Z").naive_utc(), 0, 0);
        zone.unavailable = true;
        let now = instant("2024-04-01T12:00:00Z").with_timezone(&zone);
        let expected = now.timestamp_millis() + 86_400_000;
        let started = std::time::Instant::now();
        assert_eq!(next_reset(now), expected);
        let queries = zone.local_queries.load(Ordering::Relaxed);
        eprintln!(
            "unresolved timezone local={queries} elapsed={:?}",
            started.elapsed()
        );
        assert_eq!(queries, 172_801);
        // An isolated lookup failure with no UTC offset jump must also retain the
        // operational fallback, rather than being interpreted as a midnight gap.
        zone.unavailable = false;
        zone.missing_midnight = true;
        zone.local_queries.store(0, Ordering::Relaxed);
        assert_eq!(
            next_reset(instant("2024-04-01T12:00:00Z").with_timezone(&zone)),
            expected
        );
        assert_eq!(zone.local_queries.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn connector_observer_bounds_method_labels_and_preserves_known_lowercase_labels() {
        let metrics = Metrics::configured(false, false);
        let observe = |method: &str| {
            metrics.observe_connector(Observation {
                method: method.into(),
                handler: "/fixture".into(),
                code: 404,
                request_size: 0.,
                duration: std::time::Duration::ZERO,
            });
        };
        for index in 0..128 {
            observe(&format!("FOO{index}"));
        }
        assert!(
            !metrics
                .scrape(chrono::Local::now, None)
                .contains("http_requests_total{")
        );
        for method in ["GET", "get", "pRoPfInD", "CONNECT", "TRACE", "QUERY"] {
            observe(method);
        }
        let body = metrics.scrape(chrono::Local::now, None);
        let counters = body
            .lines()
            .filter(|line| line.starts_with("http_requests_total{"))
            .collect::<Vec<_>>();
        assert_eq!(counters.len(), 5, "{body}");
        for (method, count) in [
            ("get", 2),
            ("propfind", 1),
            ("connect", 1),
            ("trace", 1),
            ("query", 1),
        ] {
            assert!(
                counters
                    .iter()
                    .any(|line| line.contains(&format!("method=\"{method}\""))
                        && line.ends_with(&format!(" {count}"))),
                "{body}"
            );
        }
        assert!(!body.contains("foo"), "{body}");
        assert!(
            !body.contains("http_request_size_bytes{"),
            "connector observations have no size histogram"
        );
    }

    #[test]
    fn scrape_reads_clocks_after_render_and_uses_fresh_reset_date() {
        let metrics = Metrics::configured(false, true);
        let boundary = instant("2024-04-01T00:00:00Z");
        let counter = {
            let mut registry = metrics.registry.lock();
            registry.next_reset_ms = boundary.timestamp_millis();
            metrics::with_local_recorder(&registry.recorder, || {
                metrics::counter!("render_clock_probe_total")
            })
        };
        counter.increment(1);
        let second = instant("2024-04-03T00:00:01Z");
        let mut clocks = [boundary + Duration::milliseconds(1), second].into_iter();
        let body = metrics.scrape(
            || {
                counter.increment(1);
                clocks
                    .next()
                    .expect("only comparison and reset-date clocks are read")
            },
            None,
        );
        assert!(
            body.lines()
                .any(|line| line == "render_clock_probe_total 1"),
            "{body}"
        );
        assert!(clocks.next().is_none(), "reset reads a fresh local date");
        assert_eq!(
            metrics.registry.lock().next_reset_ms,
            instant("2024-04-04T00:00:00Z").timestamp_millis()
        );
        assert!(
            !metrics
                .scrape(|| boundary, None)
                .contains("render_clock_probe_total")
        );
    }

    #[test]
    fn scrape_clock_history_keeps_strict_boundary_and_feed_baseline_with_one_delayed_reset() {
        let metrics = Metrics::configured(false, true);
        let boundary = instant("2024-04-01T00:00:00Z");
        metrics.registry.lock().next_reset_ms = boundary.timestamp_millis();
        metrics.record(
            Observation {
                method: "GET".into(),
                handler: "/peers".into(),
                code: 200,
                request_size: 0.,
                duration: std::time::Duration::ZERO,
            },
            true,
        );
        for now in [boundary, boundary - Duration::days(2)] {
            let mut reads = 0;
            let body = metrics.scrape(
                || {
                    reads += 1;
                    now
                },
                Some([3, 5, 7]),
            );
            assert!(body.contains("http_requests_total{"));
            assert_eq!(reads, 1);
            assert_eq!(
                metrics.registry.lock().next_reset_ms,
                boundary.timestamp_millis()
            );
        }
        let delayed = boundary + Duration::days(5) + Duration::hours(12);
        let mut reads = 0;
        let old = metrics.scrape(
            || {
                reads += 1;
                delayed
            },
            Some([3, 5, 7]),
        );
        assert!(old.contains("http_requests_total{"));
        assert_eq!(
            reads, 2,
            "a delayed scrape resets once, without daily catch-up"
        );
        assert_eq!(metrics.registry.lock().feed, [3, 5, 7]);
        assert_eq!(
            metrics.registry.lock().next_reset_ms,
            instant("2024-04-07T00:00:00Z").timestamp_millis()
        );
        let new = metrics.scrape(|| delayed, Some([4, 7, 10]));
        assert!(!new.contains("http_requests_total{"));
        for (name, value) in [
            ("no_session_socket", 1),
            ("deduplicated", 2),
            ("dropped_close", 3),
        ] {
            assert!(
                new.lines()
                    .any(|line| line
                        == format!("dcl_ws_connector_island_changed_{name}_total {value}")),
                "{new}"
            );
        }
        // A delayed scrape may contain older values for only some source counters.
        for (sample, baseline, totals) in [
            ([3, 8, 9], [4, 8, 10], [1, 3, 3]),
            ([5, 9, 11], [5, 9, 11], [2, 4, 4]),
        ] {
            let body = metrics.scrape(|| delayed, Some(sample));
            assert_eq!(metrics.registry.lock().feed, baseline);
            for (name, total) in ["no_session_socket", "deduplicated", "dropped_close"]
                .into_iter()
                .zip(totals)
            {
                assert!(
                    body.lines().any(|line| line
                        == format!("dcl_ws_connector_island_changed_{name}_total {total}")),
                    "{body}"
                );
            }
        }
    }

    #[test]
    fn scrape_without_rotation_does_not_read_reset_clocks() {
        let metrics = Metrics::configured(false, false);
        let body = metrics.scrape(|| -> DateTime<Utc> { panic!("rotation is disabled") }, None);
        assert!(body.contains("# TYPE http_requests_total counter"));
    }

    #[tokio::test]
    async fn rejected_scrape_keeps_reset_boundary_and_success_is_observed_after_old_body() {
        use axum::{
            body::{Body, to_bytes},
            http::Request,
        };
        use tower::ServiceExt;
        let state = crate::archipelago::test_support::state_over(
            crate::archipelago::Settings::for_tests(),
            Arc::new(crate::gatekeeper::bus::DisabledBus),
        )
        .await;
        let hooks = Runtime::configured(state.clone(), false, true);
        hooks.metrics.registry.lock().next_reset_ms = i64::MIN;
        hooks.observe(Observation {
            method: "GET".into(),
            handler: "/peers".into(),
            code: 200,
            request_size: 0.,
            duration: std::time::Duration::ZERO,
        });
        let app = crate::stats::http::standalone(crate::stats::http::State::new(
            state.statistics.clone(),
            Arc::new(content::Content {
                pool: state.gatekeeper.pool.clone(),
                denylist: Arc::new(crate::content::Denylist::default()),
            }),
            hooks.clone(),
            crate::stats::http::Settings {
                metrics_bearer_token: Some("fixture".into()),
                ..Default::default()
            },
            crate::stats::http::test_support::matcher(),
        ));
        let unauthorized = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthorized.status().as_u16(), 401);
        assert_eq!(hooks.metrics.registry.lock().next_reset_ms, i64::MIN);
        let mut bodies = Vec::new();
        for _ in 0..2 {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/metrics")
                        .header("authorization", "Bearer fixture")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), 200);
            bodies.push(
                String::from_utf8(
                    to_bytes(response.into_body(), usize::MAX)
                        .await
                        .unwrap()
                        .to_vec(),
                )
                .unwrap(),
            );
        }
        assert!(bodies[0].contains("handler=\"/peers\""));
        assert!(bodies[0].contains("code=\"401\""));
        assert!(!bodies[1].contains("handler=\"/peers\""));
        assert!(!bodies[1].contains("code=\"401\""));
        assert!(
            bodies[1]
                .lines()
                .any(|line| line.starts_with("http_requests_total{")
                    && line.contains("handler=\"/metrics\"")
                    && line.contains("code=\"200\"")
                    && line.ends_with(" 1"))
        );
    }
    #[tokio::test]
    async fn rejected_scrapes_do_not_control_histogram_maintenance() {
        use tower::ServiceExt;
        let state = crate::archipelago::test_support::state_over(
            crate::archipelago::Settings::for_tests(),
            Arc::new(crate::gatekeeper::bus::DisabledBus),
        )
        .await;
        let hooks = Runtime::configured(state.clone(), false, false);
        let app = crate::stats::http::standalone(crate::stats::http::State::new(
            state.statistics.clone(),
            Arc::new(content::Content {
                pool: state.gatekeeper.pool.clone(),
                denylist: Arc::new(crate::content::Denylist::default()),
            }),
            hooks.clone(),
            crate::stats::http::Settings {
                metrics_bearer_token: Some("fixture".into()),
                ..Default::default()
            },
            crate::stats::http::test_support::matcher(),
        ));
        for _ in 0..250 {
            for (path, status) in [("/peers", 200), ("/metrics", 401)] {
                let response = app
                    .clone()
                    .oneshot(
                        axum::http::Request::builder()
                            .uri(path)
                            .body(axum::body::Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(response.status().as_u16(), status);
            }
        }
        let body = hooks.scrape(chrono::Local::now);
        for code in ["200", "401"] {
            assert!(body.lines().any(
                |line| line.starts_with("http_request_duration_seconds_count{")
                    && line.contains(&format!("code=\"{code}\""))
                    && line.ends_with(" 250")
            ));
        }
    }
    #[test]
    fn repeated_observations_are_aggregated_without_a_scrape_and_reset_strictly_after_threshold() {
        let metrics = Metrics::configured(false, true);
        for _ in 0..10_000 {
            metrics.record(
                Observation {
                    method: "GET".into(),
                    handler: "/peers".into(),
                    code: 200,
                    request_size: 12.,
                    duration: std::time::Duration::from_millis(250),
                },
                true,
            );
        }
        use chrono::TimeZone;
        let threshold = metrics.registry.lock().next_reset_ms;
        let boundary = chrono::Local.timestamp_millis_opt(threshold).unwrap();
        let equal = metrics.scrape(|| boundary, None);
        assert!(equal.lines().any(
            |line| line.starts_with("http_request_duration_seconds_count{")
                && line.ends_with(" 10000")
        ));
        assert!(equal.lines().any(
            |line| line.starts_with("http_request_duration_seconds_sum{")
                && line.ends_with(" 2500")
        ));
        assert!(
            equal
                .lines()
                .any(|line| line.starts_with("http_request_size_bytes_sum{")
                    && line.ends_with(" 120000"))
        );
        assert!(
            metrics
                .scrape(|| boundary, None)
                .contains("http_requests_total{"),
            "equality does not reset"
        );
        assert!(
            metrics
                .scrape(|| boundary + chrono::Duration::milliseconds(1), None)
                .contains("http_requests_total{")
        );
        assert!(
            !metrics
                .scrape(|| boundary, None)
                .contains("http_requests_total{")
        );
    }
    #[test]
    fn declared_metric_metadata_exists_before_any_request_or_scrape() {
        let metrics = Metrics::configured(false, false);
        let body = metrics.scrape(chrono::Local::now, None);
        assert!(body.contains("# HELP http_requests_total Total number of HTTP requests"));
        assert!(body.contains("# TYPE http_request_duration_seconds histogram"));
        assert!(body.contains("# HELP wkc_logger_logs_total Log amounts per logger and level"));
        assert!(!body.contains("wkc_logger_logs_total{"));
    }

    #[tokio::test]
    async fn registry_observes_real_requests_and_resets_only_after_boundary_scrape() {
        let state = crate::archipelago::test_support::state_over(
            crate::archipelago::Settings::for_tests(),
            Arc::new(crate::gatekeeper::bus::DisabledBus),
        )
        .await;
        let hooks = Runtime::configured(state, false, true);
        hooks.observe(Observation {
            method: "GET".into(),
            handler: "/peers".into(),
            code: 200,
            request_size: 12.,
            duration: std::time::Duration::from_millis(250),
        });
        let tomorrow = chrono::Local::now() + chrono::Duration::days(2);
        let before = hooks.scrape(|| tomorrow);
        assert!(
            before.contains("http_requests_total"),
            "real HTTP observation must reach registry"
        );
        assert!(before.contains("handler=\"/peers\""));
        assert!(before.contains("http_request_duration_seconds_sum"));
        assert!(before.contains("http_request_size_bytes_sum"));
        assert!(!before.contains("process_start_time_seconds"));
        assert!(!hooks.scrape(|| tomorrow).contains("http_requests_total{"));
    }
    #[tokio::test]
    async fn lifecycle_reports_real_listener_start_and_shutdown_without_dependency_probes() {
        let state = crate::archipelago::test_support::state_over(
            crate::archipelago::Settings::for_tests(),
            Arc::new(crate::gatekeeper::bus::DisabledBus),
        )
        .await;
        let hooks = Runtime::configured(state.clone(), true, false);
        let probe = hooks.probe(false).await;
        assert!(!probe.started);
        assert!(probe.live);
        assert_eq!(probe.checks, vec![("server".into(), false)]);
        state.started.store(true, Ordering::Release);
        assert!(hooks.probe(true).await.started);
        state.restart.cancel();
        assert!(!hooks.probe(false).await.live);
        #[cfg(any(windows, target_os = "linux"))]
        assert!(
            hooks
                .metrics()
                .await
                .unwrap()
                .contains("process_resident_memory_bytes")
        );
    }
    #[cfg(any(windows, target_os = "linux"))]
    #[tokio::test]
    async fn default_registry_has_generic_process_cpu_start_and_memory_families() {
        let state = crate::archipelago::test_support::state_over(
            crate::archipelago::Settings::for_tests(),
            Arc::new(crate::gatekeeper::bus::DisabledBus),
        )
        .await;
        let hooks = Runtime::configured(state, true, false);
        let body = hooks.metrics().await.unwrap();
        for (name, kind) in [
            ("process_cpu_user_seconds_total", "counter"),
            ("process_cpu_system_seconds_total", "counter"),
            ("process_cpu_seconds_total", "counter"),
            ("process_start_time_seconds", "gauge"),
            ("process_resident_memory_bytes", "gauge"),
        ] {
            assert!(
                body.contains(&format!("# TYPE {name} {kind}")),
                "missing {name}: {body}"
            );
            assert!(
                body.lines()
                    .any(|line| line.starts_with(&format!("{name} ")))
            );
        }
        #[cfg(target_os = "linux")]
        for name in [
            "process_virtual_memory_bytes",
            "process_heap_bytes",
            "process_open_fds",
            "process_max_fds",
        ] {
            assert!(
                body.contains(&format!("# TYPE {name} gauge")),
                "missing {name}"
            );
        }
    }
}
