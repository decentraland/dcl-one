//! Statistics registry integration tests with deterministic OS samples.

#[cfg(test)]
mod tests {
    use super::super::Metrics;
    use chrono::{DateTime, Duration, Utc};
    use dcl_metrics::process::{Sample, cached_max_fds};

    const CPU_FAMILIES: [&str; 3] = [
        "process_cpu_user_seconds_total",
        "process_cpu_system_seconds_total",
        "process_cpu_seconds_total",
    ];
    const MEMORY_FAMILIES: [&str; 3] = [
        "process_resident_memory_bytes",
        "process_virtual_memory_bytes",
        "process_heap_bytes",
    ];
    const FD_FAMILIES: [&str; 2] = ["process_open_fds", "process_max_fds"];

    fn instant() -> DateTime<Utc> {
        "2024-04-01T00:00:00Z".parse().unwrap()
    }
    fn cpu(user: f64, system: f64) -> Sample {
        Sample {
            cpu: Some([user, system]),
            ..Default::default()
        }
    }
    fn scrape(metrics: &Metrics, now: DateTime<Utc>, sample: Sample) -> String {
        metrics.scrape_with_sampler(|| now, None, || sample)
    }
    fn assert_values(body: &str, names: &[&str], values: &[&str], kind: &str) {
        assert_eq!(names.len(), values.len());
        for (name, value) in names.iter().zip(values) {
            assert!(body.contains(&format!("# TYPE {name} {kind}\n")), "{body}");
            assert_eq!(
                body.lines()
                    .filter(|line| *line == format!("{name} {value}"))
                    .count(),
                1,
                "{body}"
            );
        }
    }
    fn assert_absent(body: &str, names: &[&str]) {
        for name in names {
            assert!(!body.contains(name), "unexpected {name}: {body}");
        }
    }
    fn record_request(metrics: &Metrics) {
        metrics.record(
            super::super::Observation {
                method: "GET".into(),
                handler: "/peers".into(),
                code: 200,
                request_size: 12.,
                duration: std::time::Duration::from_millis(250),
            },
            true,
        );
    }
    fn assert_request(body: &str) {
        for (name, value) in [
            ("http_requests_total", "1"),
            ("http_request_duration_seconds_sum", "0.25"),
            ("http_request_size_bytes_sum", "12"),
        ] {
            assert!(
                body.lines()
                    .any(|line| line.starts_with(&format!("{name}{{"))
                        && line.contains("handler=\"/peers\"")
                        && line.ends_with(&format!(" {value}"))),
                "{body}"
            );
        }
    }

    #[test]
    fn registry_cpu_ordinary_missing_sample_retains_measured_origin_on_recovery() {
        let metrics = Metrics::configured_with_sampler(true, false, || cpu(10., 2.));
        record_request(&metrics);
        let first = scrape(&metrics, instant(), cpu(12., 2.5));
        assert_values(&first, &CPU_FAMILIES, &["2", "0.5", "2.5"], "counter");
        let missing = scrape(&metrics, instant(), Sample::default());
        assert_absent(&missing, &CPU_FAMILIES);
        assert_request(&missing);
        let recovered = scrape(&metrics, instant(), cpu(14., 3.));
        // The unchanged origin covers the entire measured interval, including the failed read.
        assert_values(&recovered, &CPU_FAMILIES, &["4", "1", "5"], "counter");
        assert_request(&recovered);
    }

    #[test]
    fn registry_missing_cpu_at_reset_starts_recovery_at_first_known_sample() {
        let metrics = Metrics::configured_with_sampler(true, true, || cpu(10., 2.));
        metrics.registry.lock().next_reset_ms = instant().timestamp_millis();
        record_request(&metrics);
        let equal = scrape(&metrics, instant(), cpu(12., 2.5));
        assert_values(&equal, &CPU_FAMILIES, &["2", "0.5", "2.5"], "counter");
        assert_request(&equal);
        let old = scrape(
            &metrics,
            instant() + Duration::milliseconds(1),
            Sample::default(),
        );
        assert_absent(&old, &CPU_FAMILIES);
        assert_request(&old);
        let recovered = scrape(&metrics, instant() + Duration::seconds(1), cpu(14., 3.));
        // With no boundary measurement, CPU usage before this recovery cannot be assigned.
        assert_values(&recovered, &CPU_FAMILIES, &["0", "0", "0"], "counter");
        assert!(!recovered.contains("http_requests_total{"), "{recovered}");
        let next = scrape(&metrics, instant() + Duration::seconds(2), cpu(15., 3.5));
        assert_values(&next, &CPU_FAMILIES, &["1", "0.5", "1.5"], "counter");
    }

    #[test]
    fn registry_measured_cpu_reset_serializes_old_interval_before_starting_new_interval() {
        let metrics = Metrics::configured_with_sampler(true, true, || cpu(10., 2.));
        metrics.registry.lock().next_reset_ms = instant().timestamp_millis();
        let equal = scrape(&metrics, instant(), cpu(12., 2.5));
        assert_values(&equal, &CPU_FAMILIES, &["2", "0.5", "2.5"], "counter");
        let old = scrape(
            &metrics,
            instant() + Duration::milliseconds(1),
            cpu(14., 3.),
        );
        assert_values(&old, &CPU_FAMILIES, &["4", "1", "5"], "counter");
        let new = scrape(&metrics, instant() + Duration::seconds(1), cpu(15., 3.5));
        assert_values(&new, &CPU_FAMILIES, &["1", "0.5", "1.5"], "counter");
    }

    #[test]
    fn registry_unavailable_initial_cpu_sample_waits_for_measured_origin() {
        let metrics = Metrics::configured_with_sampler(true, false, Sample::default);
        let missing = scrape(&metrics, instant(), Sample::default());
        assert_absent(&missing, &CPU_FAMILIES);
        let first = scrape(&metrics, instant(), cpu(14., 3.));
        assert_values(&first, &CPU_FAMILIES, &["0", "0", "0"], "counter");
        let next = scrape(&metrics, instant(), cpu(15., 3.5));
        assert_values(&next, &CPU_FAMILIES, &["1", "0.5", "1.5"], "counter");
    }

    #[test]
    fn registry_memory_and_fd_failures_omit_only_unavailable_families_and_recover() {
        let metrics = Metrics::configured_with_sampler(true, true, || cpu(10., 2.));
        metrics.registry.lock().next_reset_ms = instant().timestamp_millis();
        record_request(&metrics);
        let available = Sample {
            start: Some(1_000_000.),
            resident: Some(1024.),
            virtual_memory: Some(4096.),
            heap: Some(512.),
            open_fds: Some(7.),
            max_fds: Some(1024.),
            ..cpu(12., 2.5)
        };
        let first = scrape(&metrics, instant(), available);
        assert_values(&first, &MEMORY_FAMILIES, &["1024", "4096", "512"], "gauge");
        assert_values(&first, &FD_FAMILIES, &["7", "1024"], "gauge");
        let memory_missing = scrape(
            &metrics,
            instant(),
            Sample {
                resident: None,
                virtual_memory: None,
                heap: None,
                ..available
            },
        );
        assert_absent(&memory_missing, &MEMORY_FAMILIES);
        assert_values(&memory_missing, &FD_FAMILIES, &["7", "1024"], "gauge");
        assert_request(&memory_missing);
        let fd_missing = scrape(
            &metrics,
            instant(),
            Sample {
                open_fds: None,
                ..available
            },
        );
        assert_absent(&fd_missing, &["process_open_fds"]);
        assert_values(&fd_missing, &["process_max_fds"], &["1024"], "gauge");
        assert_values(
            &fd_missing,
            &MEMORY_FAMILIES,
            &["1024", "4096", "512"],
            "gauge",
        );
        assert_values(&fd_missing, &CPU_FAMILIES, &["2", "0.5", "2.5"], "counter");
        assert_values(
            &fd_missing,
            &["process_start_time_seconds"],
            &["1000000"],
            "gauge",
        );
        assert_request(&fd_missing);
        let unavailable = Sample {
            max_fds: available.max_fds,
            ..Default::default()
        };
        let old = scrape(&metrics, instant() + Duration::milliseconds(1), unavailable);
        assert_absent(&old, &CPU_FAMILIES);
        assert_absent(&old, &MEMORY_FAMILIES);
        assert_absent(&old, &["process_open_fds", "process_start_time_seconds"]);
        assert_values(&old, &["process_max_fds"], &["1024"], "gauge");
        assert_request(&old);
        let recovered = scrape(
            &metrics,
            instant() + Duration::seconds(1),
            Sample {
                resident: Some(2048.),
                virtual_memory: Some(8192.),
                heap: Some(1024.),
                open_fds: Some(9.),
                ..available
            },
        );
        assert_values(
            &recovered,
            &MEMORY_FAMILIES,
            &["2048", "8192", "1024"],
            "gauge",
        );
        assert_values(&recovered, &FD_FAMILIES, &["9", "1024"], "gauge");
        assert_values(&recovered, &CPU_FAMILIES, &["0", "0", "0"], "counter");
        assert!(!recovered.contains("http_requests_total{"), "{recovered}");
        record_request(&metrics);
        let missing_again = scrape(&metrics, instant() + Duration::seconds(2), unavailable);
        assert_absent(&missing_again, &CPU_FAMILIES);
        assert_absent(&missing_again, &MEMORY_FAMILIES);
        assert_absent(
            &missing_again,
            &["process_open_fds", "process_start_time_seconds"],
        );
        assert_values(&missing_again, &["process_max_fds"], &["1024"], "gauge");
        assert_request(&missing_again);
    }

    #[test]
    fn registry_disabled_default_collection_never_samples_before_or_after_reset() {
        let metrics =
            Metrics::configured_with_sampler(false, true, || panic!("collector disabled"));
        metrics.registry.lock().next_reset_ms = instant().timestamp_millis();
        record_request(&metrics);
        for now in [instant(), instant() + Duration::milliseconds(1)] {
            let body = metrics.scrape_with_sampler(|| now, None, || panic!("collector disabled"));
            assert!(!body.contains("process_"), "{body}");
            assert_request(&body);
        }
        let new = metrics.scrape_with_sampler(
            || instant() + Duration::seconds(1),
            None,
            || panic!("collector disabled"),
        );
        assert!(!new.contains("process_"), "{new}");
        assert!(!new.contains("http_requests_total{"), "{new}");
    }

    #[test]
    fn registry_unavailable_initial_soft_limit_stays_omitted_in_its_isolated_cache() {
        // Local caches verify first-read policy; this does not exercise the production static.
        let cache = std::sync::OnceLock::new();
        let metrics = Metrics::configured_with_sampler(true, false, || Sample {
            max_fds: cached_max_fds(&cache, || None),
            ..Default::default()
        });
        let missing = scrape(
            &metrics,
            instant(),
            Sample {
                open_fds: Some(7.),
                max_fds: cached_max_fds(&cache, || panic!("first failure is cached")),
                ..Default::default()
            },
        );
        assert_absent(&missing, &["process_max_fds"]);
        assert_values(&missing, &["process_open_fds"], &["7"], "gauge");
        let independent_cache = std::sync::OnceLock::new();
        let limits = "Max open files            2048                 1048576              files\n";
        let available = scrape(
            &metrics,
            instant(),
            Sample {
                open_fds: Some(9.),
                max_fds: cached_max_fds(&independent_cache, || Some(limits.into())),
                ..Default::default()
            },
        );
        assert_values(&available, &FD_FAMILIES, &["9", "2048"], "gauge");
        let cached = scrape(
            &metrics,
            instant(),
            Sample {
                max_fds: cached_max_fds(&independent_cache, || panic!("successful read is cached")),
                ..Default::default()
            },
        );
        assert_values(&cached, &["process_max_fds"], &["2048"], "gauge");
        assert_absent(&cached, &["process_open_fds"]);
    }
}
