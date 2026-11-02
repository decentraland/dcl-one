use bevy::prelude::*;
use common::memory::MemoryStatus;

use crate::memory::MemoryStatusPlugin;

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen_test::wasm_bindgen_test]
fn web_reading_falls_after_an_engine_allocation_is_freed() {
    use bevy::time::{TimePlugin, TimeUpdateStrategy};
    use std::time::Duration;

    let mut app = App::new();
    app.add_plugins((TimePlugin, MemoryStatusPlugin));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        125,
    )));
    app.update();
    let initial = app.world().resource::<MemoryStatus>().reading().unwrap();
    let allocation = vec![0u8; 8 << 20];
    std::hint::black_box(&allocation);
    app.update();
    assert_eq!(
        app.world()
            .resource::<MemoryStatus>()
            .reading()
            .unwrap()
            .used_bytes,
        initial.used_bytes
    );
    app.update();
    let held = app.world().resource::<MemoryStatus>().reading().unwrap();
    assert_eq!(held.capacity_mib, 4096);
    drop(allocation);
    app.update();
    app.update();
    let freed = app.world().resource::<MemoryStatus>().reading().unwrap();
    assert!(held.used_bytes >= freed.used_bytes + (7 << 20));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn client_plugin_publishes_a_native_process_sample() {
    use std::time::{Duration, Instant};

    let mut app = App::new();
    app.add_plugins(MemoryStatusPlugin);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        if let Some(reading) = app.world().resource::<MemoryStatus>().reading() {
            assert!(reading.used_bytes > 0);
            assert!(reading.capacity_mib > 0);
            assert_eq!(reading.cap_mib, reading.capacity_mib);
            #[cfg(target_os = "linux")]
            for path in [
                "/sys/fs/cgroup/memory.max",
                "/sys/fs/cgroup/memory/memory.limit_in_bytes",
            ] {
                if let Some(limit) = std::fs::read_to_string(path)
                    .ok()
                    .and_then(|text| text.trim().parse::<u64>().ok())
                {
                    assert!(reading.capacity_mib <= limit / (1 << 20));
                }
            }
            break;
        }
        assert!(Instant::now() < deadline, "no native process measurement");
        std::thread::sleep(Duration::from_millis(5));
    }
}
