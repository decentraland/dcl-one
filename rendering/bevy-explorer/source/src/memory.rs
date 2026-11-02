use bevy::prelude::*;
use common::memory::{MemorySample, MemoryStatus};

pub(crate) struct MemoryStatusPlugin;

impl Plugin for MemoryStatusPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MemoryStatus>()
            .add_systems(PreUpdate, sample_memory);
    }
}

#[cfg(not(target_arch = "wasm32"))]
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
                    std::thread::sleep(std::time::Duration::from_millis(250));
                }
            });
        if let Err(error) = worker {
            warn!("Could not start the memory sampler: {error}");
            return Self { latest: None };
        }
        Self {
            latest: Some(receiver),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn sample_memory(mut sampler: Local<NativeSampler>, status: ResMut<MemoryStatus>) {
    let Some(latest) = &mut sampler.latest else {
        return;
    };
    match latest.has_changed() {
        Ok(true) => MemoryStatus::record(status.into(), *latest.borrow_and_update()),
        Ok(false) => {}
        Err(_) => {
            warn!("Memory sampler stopped; clearing the process memory reading");
            sampler.latest = None;
            MemoryStatus::record(status.into(), None);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn process_memory(system: &mut sysinfo::System) -> Option<MemorySample> {
    use sysinfo::{MemoryRefreshKind, Pid, ProcessRefreshKind, ProcessesToUpdate};

    system.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
    let total = system.total_memory();
    // sysinfo requires a successful RAM refresh before reading cgroup limits.
    if total == 0 {
        return None;
    }
    let capacity = system
        .cgroup_limits()
        .map_or(total, |limits| limits.total_memory);
    let pid = Pid::from_u32(std::process::id());
    let refreshed = system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().without_tasks().with_memory(),
    );
    let used = system.process(pid)?.memory();
    (refreshed > 0 && used > 0).then_some(MemorySample {
        used_bytes: used,
        capacity_bytes: capacity,
    })
}

// Count live engine allocations so frees lower pressure even though WASM pages never shrink.
#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ENGINE_HEAP: cap::Cap<std::alloc::System> = cap::Cap::new(std::alloc::System, usize::MAX);

#[cfg(target_arch = "wasm32")]
fn sample_memory(status: ResMut<MemoryStatus>) {
    // The imported engine memory can grow to 4 GiB (.cargo/config.toml and deploy/web/engine).
    const ENGINE_CAPACITY: u64 = 1 << 32;
    MemoryStatus::record(
        status.into(),
        Some(MemorySample {
            used_bytes: ENGINE_HEAP.allocated() as u64,
            capacity_bytes: ENGINE_CAPACITY,
        }),
    );
}
