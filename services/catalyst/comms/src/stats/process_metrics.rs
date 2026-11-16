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
    result.max_fds = *MAX_FDS.get_or_init(|| {
        std::fs::read_to_string("/proc/self/limits")
            .ok()
            .and_then(|s| proc_max_fds(&s))
    });
    result
}
#[cfg(not(any(windows, target_os = "linux")))]
pub(super) fn collect() -> Sample {
    Sample::default()
}

#[cfg(test)]
mod tests {
    use super::*;
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
