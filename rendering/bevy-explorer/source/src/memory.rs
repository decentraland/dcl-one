use bevy::prelude::*;
use common::memory::{MemorySample, MemoryStatus};
use std::time::Duration;

const SAMPLE_INTERVAL: Duration = Duration::from_millis(250);

pub(crate) struct MemoryStatusPlugin;

impl Plugin for MemoryStatusPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(not(target_arch = "wasm32"))]
        app.init_resource::<NativeSampler>();
        app.init_resource::<MemoryStatus>()
            .add_systems(PreUpdate, sample_memory);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource)]
struct NativeSampler {
    latest: Option<tokio::sync::watch::Receiver<Option<MemorySample>>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for NativeSampler {
    fn default() -> Self {
        let (sender, receiver) = tokio::sync::watch::channel(None);
        let worker = std::thread::Builder::new()
            .name("memory-status".into())
            .spawn(move || {
                let mut system = sysinfo::System::new();
                while !sender.is_closed() {
                    let _ = sender.send_replace(process_memory(&mut system));
                    std::thread::sleep(SAMPLE_INTERVAL);
                }
            });
        if let Err(error) = worker {
            error!("Could not start the memory sampler: {error}");
            return Self { latest: None };
        }
        Self {
            latest: Some(receiver),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn sample_memory(mut sampler: ResMut<NativeSampler>, status: ResMut<MemoryStatus>) {
    let Some(latest) = &mut sampler.latest else {
        return;
    };
    match latest.has_changed() {
        Ok(true) => MemoryStatus::record(status.into(), *latest.borrow_and_update()),
        Ok(false) => {}
        Err(_) => {
            error!("Memory sampler stopped; clearing the process memory reading");
            sampler.latest = None;
            MemoryStatus::record(status.into(), None);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn process_memory(system: &mut sysinfo::System) -> Option<MemorySample> {
    use sysinfo::MemoryRefreshKind;

    system.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
    let total = system.total_memory();
    // sysinfo requires a successful RAM refresh before reading cgroup limits.
    if total == 0 {
        return None;
    }
    let capacity = system
        .cgroup_limits()
        .map_or(total, |limits| limits.total_memory);
    #[cfg(target_os = "linux")]
    let used_bytes = linux_process_rss()?;
    #[cfg(not(target_os = "linux"))]
    let used_bytes = process_rss(system)?;
    (used_bytes > 0).then_some(MemorySample {
        used_bytes,
        capacity_bytes: capacity,
    })
}

#[cfg(target_os = "linux")]
fn linux_process_rss() -> Option<u64> {
    // Reading the engine's own status avoids a scan of every pid in /proc.
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let kib = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))?
        .trim()
        .strip_suffix("kB")?
        .trim()
        .parse::<u64>()
        .ok()?;
    kib.checked_mul(1024)
}

#[cfg(not(any(target_os = "linux", target_arch = "wasm32")))]
fn process_rss(system: &mut sysinfo::System) -> Option<u64> {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate};
    let pid = Pid::from_u32(std::process::id());
    let refreshed = system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().without_tasks().with_memory(),
    );
    (refreshed > 0).then_some(system.process(pid)?.memory())
}

// Count live engine allocations so frees lower pressure even though WASM pages never shrink.
#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ENGINE_HEAP: cap::Cap<crate::perf::allocation_counter::CountingAllocator<std::alloc::System>> =
    cap::Cap::new(
        crate::perf::allocation_counter::CountingAllocator(std::alloc::System),
        usize::MAX,
    );

#[cfg(target_arch = "wasm32")]
fn sample_memory(
    time: Res<Time<Real>>,
    mut last_sample: Local<Option<Duration>>,
    status: ResMut<MemoryStatus>,
) {
    // The imported engine memory can grow to 4 GiB (.cargo/config.toml and deploy/web/engine).
    const ENGINE_CAPACITY: u64 = 1 << 32;
    let now = time.elapsed();
    if last_sample.is_some_and(|last| now.saturating_sub(last) < SAMPLE_INTERVAL) {
        return;
    }
    *last_sample = Some(now);
    MemoryStatus::record(
        status.into(),
        Some(MemorySample {
            used_bytes: ENGINE_HEAP.allocated() as u64,
            capacity_bytes: ENGINE_CAPACITY,
        }),
    );
}
