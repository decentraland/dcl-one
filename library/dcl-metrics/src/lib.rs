//! Metrics recorded by service libraries and exposed once by each host process.
#![deny(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]
use metrics_exporter_prometheus::Matcher;
use std::collections::BTreeMap;
use std::net::SocketAddr;

use anyhow::{Context, ensure};
use axum::http::header;
use axum::response::IntoResponse;
pub use metrics::*;
pub use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle, PrometheusRecorder};

mod family;
pub use family::{CounterFamily, GaugeFamily, HistogramFamily};
pub mod http;
pub mod legacy_http;
pub mod process;
pub mod ws;

pub const CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

/// prom-client's default histogram buckets, for families that keep a legacy Node service's
/// histograms declared without buckets of their own.
pub const PROM_CLIENT_DEFAULT_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// A count or byte size as a metric value.
#[allow(
    clippy::cast_precision_loss,
    reason = "counts and byte sizes are exact below 2^53 and only rounded above"
)]
pub(crate) fn metric_value(n: u64) -> f64 {
    n as f64
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Counter,
    Gauge,
    Histogram(&'static [f64]),
    Summary,
}

/// The stable contract for a service metric. Declare it beside its recording code.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Descriptor {
    pub name: &'static str,
    pub help: &'static str,
    pub unit: Option<Unit>,
    pub label_keys: &'static [&'static str],
    kind: Kind,
}

impl Descriptor {
    pub const fn counter(
        name: &'static str,
        help: &'static str,
        unit: Option<Unit>,
        label_keys: &'static [&'static str],
    ) -> Self {
        Self {
            name,
            help,
            unit,
            label_keys,
            kind: Kind::Counter,
        }
    }

    pub const fn gauge(
        name: &'static str,
        help: &'static str,
        unit: Option<Unit>,
        label_keys: &'static [&'static str],
    ) -> Self {
        Self {
            name,
            help,
            unit,
            label_keys,
            kind: Kind::Gauge,
        }
    }

    pub const fn histogram(
        name: &'static str,
        help: &'static str,
        unit: Option<Unit>,
        label_keys: &'static [&'static str],
        buckets: &'static [f64],
    ) -> Self {
        Self {
            name,
            help,
            unit,
            label_keys,
            kind: Kind::Histogram(buckets),
        }
    }

    /// A legacy distribution exposed with the recorder's existing quantiles.
    pub const fn summary(
        name: &'static str,
        help: &'static str,
        unit: Option<Unit>,
        label_keys: &'static [&'static str],
    ) -> Self {
        Self {
            name,
            help,
            unit,
            label_keys,
            kind: Kind::Summary,
        }
    }

    fn describe(self, recorder: &impl Recorder) {
        let name = self.name.into();
        let help = self.help.into();
        match self.kind {
            Kind::Counter => recorder.describe_counter(name, self.unit, help),
            Kind::Gauge => recorder.describe_gauge(name, self.unit, help),
            Kind::Histogram(_) | Kind::Summary => {
                recorder.describe_histogram(name, self.unit, help)
            }
        }
    }
}

fn valid_identifier(name: &str, allow_colon: bool) -> bool {
    !name.is_empty()
        && name.bytes().enumerate().all(|(index, ch)| {
            ch.is_ascii_alphabetic()
                || ch == b'_'
                || (allow_colon && ch == b':')
                || (index != 0 && ch.is_ascii_digit())
        })
}

/// Composes process-local and shared declarations before installing the process recorder.
pub struct Registry {
    families: BTreeMap<&'static str, Descriptor>,
}

impl Registry {
    pub fn new(groups: &[&[Descriptor]]) -> anyhow::Result<Self> {
        let mut families = BTreeMap::new();
        for family in http::FAMILIES
            .iter()
            .chain(ws::FAMILIES)
            .chain(groups.iter().flat_map(|group| group.iter()))
        {
            ensure!(
                valid_identifier(family.name, true),
                "invalid Prometheus metric name {}",
                family.name
            );
            ensure!(!family.help.is_empty(), "missing help for {}", family.name);
            for (i, key) in family.label_keys.iter().enumerate() {
                ensure!(
                    valid_identifier(key, false)
                        && !key.starts_with("__")
                        && !(matches!(family.kind, Kind::Histogram(_)) && *key == "le")
                        && !(family.kind == Kind::Summary && *key == "quantile"),
                    "invalid label {key} in {}",
                    family.name
                );
                ensure!(
                    !family.label_keys[..i].contains(key),
                    "duplicate label {key} in {}",
                    family.name
                );
            }
            if let Kind::Histogram(bounds) = family.kind {
                ensure!(
                    !bounds.is_empty()
                        && bounds.iter().all(|b| b.is_finite())
                        && bounds.windows(2).all(|w| w[0] < w[1]),
                    "invalid histogram buckets for {}",
                    family.name
                );
            }
            if let Some(existing) = families.insert(family.name, *family) {
                ensure!(
                    existing == *family,
                    "conflicting metric declarations for {}",
                    family.name
                );
            }
        }
        // The exporter synthesizes samples for distributions; a second declaration with
        // one of these names would produce ambiguous samples in the same scrape.
        for family in families.values() {
            let suffixes: &[&str] = match family.kind {
                Kind::Histogram(_) => &["_bucket", "_sum", "_count"],
                Kind::Summary => &["_sum", "_count"],
                _ => &[],
            };
            for suffix in suffixes {
                let sample = format!("{}{suffix}", family.name);
                ensure!(
                    !families.contains_key(sample.as_str()),
                    "metric name {sample} conflicts with {}",
                    family.name
                );
            }
        }
        Ok(Self { families })
    }

    pub fn builder(&self, mut builder: PrometheusBuilder) -> anyhow::Result<PrometheusBuilder> {
        for family in self.families.values() {
            if let Kind::Histogram(bounds) = family.kind {
                builder =
                    builder.set_buckets_for_metric(Matcher::Full(family.name.into()), bounds)?;
            }
        }
        Ok(builder)
    }

    /// Builds a recorder with the process's descriptors and histogram buckets.
    pub fn build_recorder(
        &self,
        builder: PrometheusBuilder,
    ) -> anyhow::Result<metrics_exporter_prometheus::PrometheusRecorder> {
        let recorder = self.builder(builder)?.build_recorder();
        for family in self.families.values() {
            family.describe(&recorder);
        }
        Ok(recorder)
    }

    /// For binaries that mount metrics alongside their own health routes.
    pub fn install(&self, builder: PrometheusBuilder) -> anyhow::Result<PrometheusHandle> {
        let recorder = self.build_recorder(builder)?;
        let handle = recorder.handle();
        set_global_recorder(recorder)
            .map_err(|error| anyhow::anyhow!("installing prometheus recorder: {error}"))?;
        Ok(handle)
    }

    /// Install during startup, before serving requests or starting jobs.
    pub fn listen(
        &self,
        builder: PrometheusBuilder,
        bind: SocketAddr,
    ) -> anyhow::Result<PrometheusHandle> {
        let runtime = tokio::runtime::Handle::try_current()?;
        let listener = std::net::TcpListener::bind(bind)
            .with_context(|| format!("binding metrics listener on {bind}"))?;
        listener.set_nonblocking(true)?;
        // Keep the bound socket so restarting the server preserves its port (including
        // an OS-assigned one) and clients can queue while a replacement starts.
        let bound_addr = listener.local_addr()?;
        let open_socket = move || {
            listener
                .try_clone()
                .and_then(tokio::net::TcpListener::from_std)
        };
        let mut first = Some(open_socket()?);
        let handle = self.install(builder)?;
        let scrape_handle = handle.clone();
        let app = axum::Router::new()
            .route("/health", axum::routing::any(|| async { "OK" }))
            .route(
                "/metrics",
                axum::routing::get(move || render(scrape_handle.clone())),
            );
        runtime.spawn(upkeep(handle.clone()));
        runtime.spawn(supervise_listener(bound_addr, move || {
            let socket = first.take().map(Ok).unwrap_or_else(&open_socket);
            let app = app.clone();
            async move { axum::serve(socket?, app).await }
        }));
        Ok(handle)
    }
}

/// Axum retries TCP accept errors internally. If a serve attempt itself terminates,
/// retry on the retained socket without interrupting recorder upkeep.
async fn supervise_listener<F, Fut>(bind: SocketAddr, mut serve: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = std::io::Result<()>>,
{
    loop {
        match serve().await {
            Ok(()) => {
                tracing::error!(%bind, "metrics listener exited unexpectedly; restarting in one second")
            }
            Err(error) => {
                tracing::error!(%bind, %error, "metrics listener failed; restarting in one second")
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

/// Drain histogram observations even if nobody scrapes the process.
pub async fn upkeep(handle: PrometheusHandle) {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(5));
    loop {
        tick.tick().await;
        handle.run_upkeep();
    }
}

/// Produce Prometheus text with one OS-lifetime sample for this process.
pub async fn render(handle: PrometheusHandle) -> axum::response::Response {
    let body = tokio::task::spawn_blocking(move || {
        let mut body = handle.render();
        process::collect().append(&mut body);
        body
    })
    .await;
    match body {
        Ok(body) => ([(header::CONTENT_TYPE, CONTENT_TYPE)], body).into_response(),
        Err(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub fn loopback_bind(setting: &str, raw: &str) -> anyhow::Result<SocketAddr> {
    let bind: SocketAddr = raw.parse().with_context(|| format!("{setting} `{raw}`"))?;
    ensure!(
        bind.ip().is_loopback(),
        "{setting} must use a loopback address"
    );
    Ok(bind)
}

#[cfg(test)]
mod registry_tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn listener_retries_errors_and_unexpected_clean_exits_with_backoff() {
        let start = tokio::time::Instant::now();
        let (timestamps, mut attempts) = tokio::sync::mpsc::unbounded_channel();
        let mut outcomes = [
            Err(std::io::Error::other("injected listener failure")),
            Ok(()),
        ]
        .into_iter();
        let task = tokio::spawn(supervise_listener(
            "127.0.0.1:5006".parse().unwrap(),
            move || {
                timestamps.send(tokio::time::Instant::now()).unwrap();
                let result = outcomes.next();
                async move {
                    match result {
                        Some(result) => result,
                        None => std::future::pending().await,
                    }
                }
            },
        ));
        for seconds in 0..3 {
            assert_eq!(
                attempts.recv().await.unwrap() - start,
                std::time::Duration::from_secs(seconds)
            );
        }
        task.abort();
    }

    const LAG: HistogramFamily<1> = HistogramFamily::new(
        "consumer_lag_seconds",
        "Consumer delay in seconds.",
        Some(Unit::Seconds),
        &["source"],
        &[0.1, 1.0, 10.0],
    );

    #[test]
    fn compatible_repeated_declarations_share_one_histogram() {
        let family = [LAG.descriptor];
        let registry = Registry::new(&[&family, &family]).unwrap();
        let recorder = registry
            .builder(PrometheusBuilder::new())
            .unwrap()
            .build_recorder();
        let handle = recorder.handle();
        with_local_recorder(&recorder, || {
            LAG.descriptor.describe(&recorder);
            LAG.handle(["replica"]).record(0.5);
        });
        let rendered = handle.render();
        assert_eq!(rendered.matches("# HELP consumer_lag_seconds ").count(), 1);
        assert!(rendered.contains("# TYPE consumer_lag_seconds histogram"));
        assert!(rendered.contains("consumer_lag_seconds_bucket{source=\"replica\",le=\"1\"} 1"));
    }

    #[test]
    fn conflicting_contracts_fail_before_installation() {
        let family = [LAG.descriptor];
        for conflict in [
            Descriptor::counter(
                "consumer_lag_seconds",
                "Consumer delay in seconds.",
                Some(Unit::Seconds),
                &["source"],
            ),
            Descriptor::histogram(
                "consumer_lag_seconds",
                "Consumer delay in seconds.",
                Some(Unit::Seconds),
                &["source"],
                &[1.0, 10.0],
            ),
            Descriptor::histogram(
                "consumer_lag_seconds",
                "Consumer delay in seconds.",
                Some(Unit::Seconds),
                &[],
                &[0.1, 1.0, 10.0],
            ),
        ] {
            assert!(Registry::new(&[&family, &[conflict]]).is_err());
        }
    }

    #[test]
    fn malformed_buckets_and_duplicate_labels_fail_registration() {
        for bad in [
            Descriptor::counter("9illegal", "Invalid name.", None, &[]),
            Descriptor::gauge("healthy", "Invalid label.", None, &["x/y"]),
            Descriptor::histogram("timings", "Exporter uses le.", None, &["le"], &[1.]),
        ] {
            assert!(Registry::new(&[&[bad]]).is_err());
        }
        let colliding =
            Descriptor::gauge("consumer_lag_seconds_sum", "Duplicate sample.", None, &[]);
        assert!(Registry::new(&[&[LAG.descriptor, colliding]]).is_err());
        let repeated = Descriptor::counter("jobs_total", "Jobs.", None, &["job", "job"]);
        assert!(Registry::new(&[&[repeated]]).is_err());
        for bounds in [
            &[][..],
            &[1.0, 1.0],
            &[5.0, 1.0],
            &[f64::NAN],
            &[f64::INFINITY],
        ] {
            let descriptor = Descriptor::histogram("lag", "Lag.", None, &[], bounds);
            assert!(Registry::new(&[&[descriptor]]).is_err());
        }
    }
}
