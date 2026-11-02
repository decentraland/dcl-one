use bevy::prelude::*;
use common::memory::MemoryStatus;

pub(crate) struct MemoryStatusPlugin;

impl Plugin for MemoryStatusPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MemoryStatus>()
            .add_systems(PreUpdate, sample_memory);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct NativeSampler {
    next_sample: std::time::Duration,
    pending: Option<bevy::tasks::Task<Option<(u64, u64)>>>,
}

#[cfg(not(target_arch = "wasm32"))]
fn sample_memory(
    time: Res<Time<Real>>,
    mut sampler: Local<NativeSampler>,
    mut status: ResMut<MemoryStatus>,
) {
    use std::time::Duration;

    use bevy::tasks::IoTaskPool;
    use common::util::TaskExt;

    if let Some(sample) = sampler.pending.as_mut().and_then(TaskExt::complete) {
        status.record(sample);
        sampler.pending = None;
    }
    if sampler.pending.is_none() && time.elapsed() >= sampler.next_sample {
        sampler.next_sample = time.elapsed() + Duration::from_millis(250);
        sampler.pending = Some(IoTaskPool::get().spawn(async { process_memory() }));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn process_memory() -> Option<(u64, u64)> {
    use sysinfo::{MemoryRefreshKind, Pid, ProcessRefreshKind, ProcessesToUpdate, System};

    let pid = Pid::from_u32(std::process::id());
    let mut system = System::new();
    system.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().without_tasks().with_memory(),
    );
    let used = system.process(pid)?.memory();
    // Some platforms return zero when they cannot read this process.
    (used > 0).then_some((used, system.total_memory()))
}

// Count live engine allocations so frees lower pressure even though WASM pages never shrink.
#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ENGINE_HEAP: cap::Cap<std::alloc::System> = cap::Cap::new(std::alloc::System, usize::MAX);

#[cfg(target_arch = "wasm32")]
fn sample_memory(mut status: ResMut<MemoryStatus>) {
    // The imported engine memory can grow to 4 GiB (.cargo/config.toml and deploy/web/engine).
    const ENGINE_CAPACITY: u64 = 1 << 32;
    status.record(Some((ENGINE_HEAP.allocated() as u64, ENGINE_CAPACITY)));
}

#[cfg(all(test, target_arch = "wasm32"))]
#[wasm_bindgen_test::wasm_bindgen_test]
fn web_reading_falls_after_an_engine_allocation_is_freed() {
    let mut app = App::new();
    app.add_plugins(MemoryStatusPlugin);
    app.update();
    let allocation = vec![0u8; 8 << 20];
    std::hint::black_box(&allocation);
    app.update();
    let held = app.world().resource::<MemoryStatus>().reading().unwrap();
    assert_eq!(held.capacity_mib, 4096);
    drop(allocation);
    app.update();
    let freed = app.world().resource::<MemoryStatus>().reading().unwrap();
    assert!(held.used_bytes >= freed.used_bytes + (7 << 20));
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn client_plugin_publishes_a_native_process_sample() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, MemoryStatusPlugin));
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            app.update();
            if let Some(reading) = app.world().resource::<MemoryStatus>().reading() {
                assert!(reading.used_bytes > 0);
                assert!(reading.capacity_mib > 0);
                assert_eq!(reading.cap_mib, reading.capacity_mib);
                break;
            }
            assert!(Instant::now() < deadline, "no native process measurement");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
