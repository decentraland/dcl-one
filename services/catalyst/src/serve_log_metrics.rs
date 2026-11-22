//! `wkc_logger_logs_total{logger, level}`, the log count the content server
//! and lamb2 exported (`@well-known-components/logger` 3.1.3): one per event
//! the `RUST_LOG` filter lets through. `logger` is the event's target (its
//! module path), and `level` is the old printer's name for the level:
//! `ERROR`, `WARNING`, `INFO` or `DEBUG` (`TRACE` counts as `DEBUG`).

pub(super) const LOGS: &str = "wkc_logger_logs_total";

pub(super) fn register_series() {
    metrics::describe_counter!(LOGS, "Log amounts per logger and level");
}

pub(super) struct CountLogs;

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for CountLogs {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        let metadata = event.metadata();
        let level = match *metadata.level() {
            tracing::Level::ERROR => "ERROR",
            tracing::Level::WARN => "WARNING",
            tracing::Level::INFO => "INFO",
            _ => "DEBUG",
        };
        metrics::counter!(LOGS, "logger" => metadata.target(), "level" => level).increment(1);
    }
}

#[cfg(test)]
mod tests {
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn printed_events_count_by_target_and_level() {
        let recorder = crate::comms::metrics::prometheus_builder()
            .unwrap()
            .build_recorder();
        let handle = recorder.handle();
        let _recording = metrics::set_default_local_recorder(&recorder);
        super::register_series();
        let subscriber = tracing_subscriber::registry()
            .with(super::CountLogs)
            .with(tracing_subscriber::EnvFilter::new("info"));
        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(target: "sync", "one");
            tracing::warn!(target: "sync", "two");
            tracing::info!(target: "sync", "three");
            tracing::debug!(target: "sync", "filtered out");
        });
        let rendered = handle.render();
        for line in [
            "# HELP wkc_logger_logs_total Log amounts per logger and level",
            "# TYPE wkc_logger_logs_total counter",
            "wkc_logger_logs_total{logger=\"sync\",level=\"WARNING\"} 2",
            "wkc_logger_logs_total{logger=\"sync\",level=\"INFO\"} 1",
        ] {
            assert!(rendered.contains(line), "missing {line}:\n{rendered}");
        }
        assert!(!rendered.contains("DEBUG"), "{rendered}");
    }
}
