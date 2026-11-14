//! OS process measurements. No Node or V8 collector is inferred from Rust runtime data.
use std::fmt::Write;

#[derive(Default, Clone, Copy)]
pub(super) struct Sample {
    pub cpu: Option<[f64; 2]>,
    start: Option<f64>,
    resident: Option<f64>,
    virtual_memory: Option<f64>,
    heap: Option<f64>,
    open_fds: Option<f64>,
    max_fds: Option<f64>,
}
impl Sample {
    /// The metrics facade's integer counters cannot represent fractional CPU seconds.
    /// Render only these measured families directly, with their actual Prometheus types.
    pub fn append(&self, body: &mut String, origin: &mut Option<[f64; 2]>) {
        if let Some(cpu) = self.cpu {
            let origin = origin.get_or_insert(cpu);
            let user = (cpu[0] - origin[0]).max(0.);
            let system = (cpu[1] - origin[1]).max(0.);
            for (name, help, value) in [
                (
                    "process_cpu_user_seconds_total",
                    "Total user CPU time spent in seconds.",
                    user,
                ),
                (
                    "process_cpu_system_seconds_total",
                    "Total system CPU time spent in seconds.",
                    system,
                ),
                (
                    "process_cpu_seconds_total",
                    "Total user and system CPU time spent in seconds.",
                    user + system,
                ),
            ] {
                metric(body, name, "counter", help, value);
            }
        }
        for (name, help, value) in [
            (
                "process_start_time_seconds",
                "Start time of the process since unix epoch in seconds.",
                self.start,
            ),
            (
                "process_resident_memory_bytes",
                "Resident memory size in bytes.",
                self.resident,
            ),
            (
                "process_virtual_memory_bytes",
                "Virtual memory size in bytes.",
                self.virtual_memory,
            ),
            (
                "process_heap_bytes",
                "Process heap size in bytes.",
                self.heap,
            ),
            (
                "process_open_fds",
                "Number of open file descriptors.",
                self.open_fds,
            ),
            (
                "process_max_fds",
                "Maximum number of open file descriptors.",
                self.max_fds,
            ),
        ] {
            if let Some(value) = value {
                metric(body, name, "gauge", help, value);
            }
        }
    }
}
fn metric(body: &mut String, name: &str, kind: &str, help: &str, value: f64) {
    if value.is_finite() {
        writeln!(
            body,
            "# HELP {name} {help}\n# TYPE {name} {kind}\n{name} {value}\n"
        )
        .unwrap();
    }
}
#[cfg(any(windows, test))]
fn filetime_seconds(low: u32, high: u32) -> f64 {
    (((high as u64) << 32) | low as u64) as f64 / 10_000_000.
}
#[cfg(windows)]
pub(super) fn collect() -> Sample {
    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[repr(C)]
    struct Memory {
        size: u32,
        faults: u32,
        values: [usize; 8],
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn GetProcessTimes(
            process: *mut std::ffi::c_void,
            created: *mut FileTime,
            exited: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
        fn K32GetProcessMemoryInfo(
            process: *mut std::ffi::c_void,
            memory: *mut Memory,
            size: u32,
        ) -> i32;
    }
    let mut result = Sample::default();
    let (mut created, mut exited, mut kernel, mut user) = (
        FileTime::default(),
        FileTime::default(),
        FileTime::default(),
        FileTime::default(),
    );
    // Initialized ABI-sized buffers, borrowed only for these synchronous system calls.
    unsafe {
        let process = GetCurrentProcess();
        if GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) != 0 {
            result.start =
                Some((filetime_seconds(created.low, created.high) - 11_644_473_600.).round());
            result.cpu = Some([
                filetime_seconds(user.low, user.high),
                filetime_seconds(kernel.low, kernel.high),
            ]);
        }
        let mut memory = Memory {
            size: std::mem::size_of::<Memory>() as u32,
            faults: 0,
            values: [0; 8],
        };
        if K32GetProcessMemoryInfo(process, &mut memory, memory.size) != 0 {
            result.resident = Some(memory.values[1] as f64);
        }
    }
    result
}
#[cfg(any(target_os = "linux", test))]
fn proc_cpu(stat: &str, ticks_per_second: f64, boot_seconds: Option<f64>) -> Option<Sample> {
    if !ticks_per_second.is_finite() || ticks_per_second <= 0. {
        return None;
    }
    // comm is parenthesized and may itself contain spaces and parentheses.
    let (_, fields) = stat.rsplit_once(") ")?;
    let fields: Vec<_> = fields.split_whitespace().collect();
    let ticks = |index: usize| {
        fields
            .get(index)?
            .parse::<u64>()
            .ok()
            .map(|n| n as f64 / ticks_per_second)
    };
    Some(Sample {
        cpu: Some([ticks(11)?, ticks(12)?]),
        start: boot_seconds.and_then(|boot| ticks(19).map(|start| (boot + start).round())),
        ..Default::default()
    })
}
#[cfg(any(target_os = "linux", test))]
fn proc_memory(sample: &mut Sample, status: &str) {
    for (field, output) in [
        ("VmRSS:", &mut sample.resident),
        ("VmSize:", &mut sample.virtual_memory),
        ("VmData:", &mut sample.heap),
    ] {
        *output = status.lines().find_map(|line| {
            let mut values = line.strip_prefix(field)?.split_whitespace();
            let kib = values.next()?.parse::<u64>().ok()?;
            (values.next() == Some("kB")).then_some(kib as f64 * 1024.)
        });
    }
}
#[cfg(any(target_os = "linux", test))]
fn proc_max_fds(limits: &str) -> Option<f64> {
    limits.lines().find_map(|line| {
        line.strip_prefix("Max open files")?
            .split_whitespace()
            .next()?
            .parse::<u64>()
            .ok()
            .map(|n| n as f64)
    })
}
#[cfg(any(target_os = "linux", test))]
fn cached_max_fds(
    cache: &std::sync::OnceLock<Option<f64>>,
    read: impl FnOnce() -> Option<String>,
) -> Option<f64> {
    *cache.get_or_init(|| read().and_then(|limits| proc_max_fds(&limits)))
}
#[cfg(target_os = "linux")]
pub(super) fn collect() -> Sample {
    unsafe extern "C" {
        fn sysconf(name: std::ffi::c_int) -> std::ffi::c_long;
    }
    // Linux's _SC_CLK_TCK. Read the configured kernel tick frequency, never assume HZ.
    let ticks = unsafe { sysconf(2) } as f64;
    let boot = std::fs::read_to_string("/proc/stat").ok().and_then(|s| {
        s.lines().find_map(|l| {
            l.strip_prefix("btime ")?
                .parse::<u64>()
                .ok()
                .map(|n| n as f64)
        })
    });
    let mut result = std::fs::read_to_string("/proc/self/stat")
        .ok()
        .and_then(|s| proc_cpu(&s, ticks, boot))
        .unwrap_or_default();
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        proc_memory(&mut result, &status);
    }
    if let Ok(mut entries) = std::fs::read_dir("/proc/self/fd") {
        let count = entries
            .try_fold(0usize, |count, entry| entry.map(|_| count + 1))
            .ok();
        // Exclude the directory descriptor used by read_dir itself.
        result.open_fds = count.and_then(|n| n.checked_sub(1)).map(|n| n as f64);
    }
    static MAX_FDS: std::sync::OnceLock<Option<f64>> = std::sync::OnceLock::new();
    result.max_fds = cached_max_fds(&MAX_FDS, || {
        std::fs::read_to_string("/proc/self/limits").ok()
    });
    result
}
#[cfg(not(any(windows, target_os = "linux")))]
pub(super) fn collect() -> Sample {
    Sample::default()
}

#[cfg(test)]
mod tests {
    use super::super::Metrics;
    use super::*;
    use chrono::{DateTime, Duration, Utc};

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

    #[test]
    fn cpu_seconds_keep_fractions_and_restart_from_the_last_scrape_on_reset() {
        assert_eq!(filetime_seconds(1_250_000, 0), 0.125);
        let sample = Sample {
            cpu: Some([10.125, 2.25]),
            ..Default::default()
        };
        let mut origin = Some([10., 2.]);
        let mut body = String::new();
        sample.append(&mut body, &mut origin);
        assert!(body.contains("process_cpu_user_seconds_total 0.125\n"));
        assert!(body.contains("process_cpu_system_seconds_total 0.25\n"));
        assert!(body.contains("process_cpu_seconds_total 0.375\n"));
        origin = sample.cpu;
        body.clear();
        Sample {
            cpu: Some([10.25, 2.5]),
            ..Default::default()
        }
        .append(&mut body, &mut origin);
        assert!(body.contains("process_cpu_seconds_total 0.375\n"));
        assert!(!body.contains("process_resident_memory_bytes"));
    }
    #[test]
    fn linux_proc_units_and_field_positions_handle_spaces_in_comm() {
        let stat = "123 (name with ) spaces) R 1 2 3 4 5 6 7 8 9 10 123 45 0 0 0 0 0 0 1234";
        let mut sample = proc_cpu(stat, 100., Some(1_000_000.)).unwrap();
        assert_eq!(sample.cpu, Some([1.23, 0.45]));
        assert_eq!(sample.start, Some(1_000_012.));
        proc_memory(&mut sample, "VmRSS:\t17 kB\nVmSize: 19 kB\nVmData: 3 kB\n");
        assert_eq!(sample.resident, Some(17408.));
        assert_eq!(sample.virtual_memory, Some(19456.));
        assert_eq!(sample.heap, Some(3072.));
        assert_eq!(
            proc_max_fds(
                "Max open files            1024                 1048576              files\n"
            ),
            Some(1024.)
        );
    }
    #[test]
    fn missing_or_malformed_proc_data_never_fabricates_zero_gauges() {
        assert!(proc_cpu("bad", 100., None).is_none());
        assert!(proc_cpu("123 (x) R 1", 100., None).is_none());
        assert!(proc_cpu("123 (x) R 1", 0., None).is_none());
        assert!(proc_max_fds("Max open files unlimited unlimited files").is_none());
        let mut sample = Sample::default();
        proc_memory(&mut sample, "VmRSS: nope kB\nVmSize: 20 MB\n");
        let mut body = String::new();
        sample.append(&mut body, &mut None);
        assert!(body.is_empty());
    }
}
