//! Source-clock validation and metrics serialization for the native emitter.

use bevy::prelude::{Component, Entity};
use dcl_types::SceneResourceCounters;

/// The baseline belongs to one execution and is removed with its scene entity.
#[derive(Component, Default)]
pub struct EmittedSceneStats(pub SceneResourceCounters);

/// Build one fresh source observation. Non-Unix clocks cannot measure thread CPU.
pub fn scene_stats_sample(
    scene: &str,
    execution: Entity,
    cur: &SceneResourceCounters,
    base: &SceneResourceCounters,
    thread_cpu_available: bool,
) -> Option<serde_json::Value> {
    let window_s = source_window_seconds(
        cur.sample_seq,
        cur.sample_elapsed_us,
        base.sample_seq,
        base.sample_elapsed_us,
    )?;
    Some(serde_json::json!({
        "scene": scene,
        "window_s": window_s,
        "observed_at_ms": cur.sample_timestamp_ms,
        "sample_seq": cur.sample_seq,
        "execution": execution.to_bits().to_string(),
        "fetch": {
            "started": cur.fetch_started.saturating_sub(base.fetch_started),
            "completed": cur.fetch_completed.saturating_sub(base.fetch_completed),
            "failed": cur.fetch_failed.saturating_sub(base.fetch_failed),
            "bytes_down": cur.fetch_bytes_down.saturating_sub(base.fetch_bytes_down),
            "in_flight": cur.fetch_in_flight,
            "duration_ms": cur.fetch_duration_us.saturating_sub(base.fetch_duration_us) as f64 / 1000.0,
            "status_2xx": cur.fetch_status_2xx.saturating_sub(base.fetch_status_2xx),
            "status_3xx": cur.fetch_status_3xx.saturating_sub(base.fetch_status_3xx),
            "status_4xx": cur.fetch_status_4xx.saturating_sub(base.fetch_status_4xx),
            "status_5xx": cur.fetch_status_5xx.saturating_sub(base.fetch_status_5xx),
        },
        "storage": {
            "requests": cur.storage_requests.saturating_sub(base.storage_requests),
            "completed": cur.storage_completed.saturating_sub(base.storage_completed),
            "failed": cur.storage_failed.saturating_sub(base.storage_failed),
            "unauthorized": cur.storage_unauthorized.saturating_sub(base.storage_unauthorized),
        },
        "ws": {"opened": cur.ws_opened.saturating_sub(base.ws_opened), "open": cur.ws_open},
        "comms": {
            "msgs_in": cur.comms_msgs_in.saturating_sub(base.comms_msgs_in),
            "bytes_in": cur.comms_bytes_in.saturating_sub(base.comms_bytes_in),
            "msgs_out": cur.comms_msgs_out.saturating_sub(base.comms_msgs_out),
            "bytes_out": cur.comms_bytes_out.saturating_sub(base.comms_bytes_out),
        },
        "logs": {
            "dropped": cur.log_dropped.saturating_sub(base.log_dropped),
            "lines": cur.log_lines.saturating_sub(base.log_lines),
            "bytes": cur.log_bytes.saturating_sub(base.log_bytes),
        },
        "crdt": {
            "bytes": cur.crdt_bytes.saturating_sub(base.crdt_bytes),
            "store_bytes": cur.crdt_store_bytes,
            "entities": cur.crdt_entities,
            "components": cur.crdt_components,
        },
        "ipc": {"responses": cur.ipc_responses.saturating_sub(base.ipc_responses)},
        // Non-Unix runtime timing is wall time, not thread CPU.
        "cpu": if thread_cpu_available { serde_json::json!({
            "ticks": cur.tick_count.saturating_sub(base.tick_count),
            "run_ms": cur.run_us.saturating_sub(base.run_us) as f64 / 1000.0,
            "updates_completed": cur.updates_completed.saturating_sub(base.updates_completed),
            "update_errors": cur.update_errors.saturating_sub(base.update_errors),
            "update_le_1ms": cur.update_cpu_le_1ms.saturating_sub(base.update_cpu_le_1ms),
            "update_1_5ms": cur.update_cpu_1_5ms.saturating_sub(base.update_cpu_1_5ms),
            "update_5_16ms": cur.update_cpu_5_16ms.saturating_sub(base.update_cpu_5_16ms),
            "update_16_50ms": cur.update_cpu_16_50ms.saturating_sub(base.update_cpu_16_50ms),
            "update_gt_50ms": cur.update_cpu_gt_50ms.saturating_sub(base.update_cpu_gt_50ms),
        }) } else { serde_json::json!({
            "ticks": cur.tick_count.saturating_sub(base.tick_count),
            "updates_completed": cur.updates_completed.saturating_sub(base.updates_completed),
            "update_errors": cur.update_errors.saturating_sub(base.update_errors),
        }) },
        "mem": {"heap_used": cur.heap_used, "heap_limit": cur.heap_limit, "external_bytes": cur.external_memory},
    }))
}

/// Seconds covered by a new cumulative observation. Baselines belong to a Bevy
/// Entity (one execution), never the deployment hash. A new execution starts at
/// sequence/elapsed zero, regardless of how long the engine has been alive.
pub fn source_window_seconds(
    sequence: u64,
    elapsed_us: u64,
    previous_sequence: u64,
    previous_elapsed_us: u64,
) -> Option<f64> {
    if sequence == 0 || sequence <= previous_sequence || elapsed_us <= previous_elapsed_us {
        return None;
    }
    Some((elapsed_us - previous_elapsed_us) as f64 / 1_000_000.0)
}

#[cfg(test)]
mod tests {
    use super::source_window_seconds;

    #[test]
    fn first_observation_uses_execution_elapsed_time() {
        assert_eq!(source_window_seconds(1, 5_125_000, 0, 0), Some(5.125));
    }

    #[test]
    fn cached_observation_does_not_create_a_new_sample() {
        assert_eq!(source_window_seconds(2, 10_000_000, 2, 10_000_000), None);
    }

    #[test]
    fn skipped_cumulative_snapshots_keep_the_full_window() {
        assert_eq!(
            source_window_seconds(5, 25_000_000, 2, 10_000_000),
            Some(15.0)
        );
    }

    #[test]
    fn new_execution_does_not_inherit_the_previous_baseline() {
        // The emitter keys baselines by Entity. A replacement receives the empty
        // baseline even when it has exactly the same deployment hash.
        assert_eq!(source_window_seconds(1, 5_000_000, 0, 0), Some(5.0));
        assert_eq!(source_window_seconds(1, 5_000_000, 10, 50_000_000), None);
    }

    #[test]
    fn zero_or_regressing_clocks_are_not_divisors() {
        assert_eq!(source_window_seconds(1, 0, 0, 0), None);
        assert_eq!(source_window_seconds(2, 1, 1, 2), None);
    }
}

#[cfg(test)]
mod emission_tests {
    use super::*;
    use bevy::prelude::World;

    fn observation() -> SceneResourceCounters {
        SceneResourceCounters {
            sample_seq: 1,
            sample_elapsed_us: 5_000_000,
            sample_timestamp_ms: 1_000_000,
            fetch_started: 7,
            fetch_duration_us: 1250,
            run_us: 1750,
            heap_used: 100,
            external_memory: 50,
            crdt_store_bytes: 80,
            updates_completed: 1,
            update_cpu_1_5ms: 1,
            ..Default::default()
        }
    }

    #[test]
    fn deltas_preserve_fractional_time_and_gauges_are_instantaneous() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let first = observation();
        let sample =
            scene_stats_sample("scene", entity, &first, &Default::default(), true).unwrap();
        assert_eq!(sample["cpu"]["run_ms"], 1.75);
        assert_eq!(sample["fetch"]["duration_ms"], 1.25);
        assert_eq!(sample["window_s"], 5.0);
        assert_eq!(sample["observed_at_ms"], first.sample_timestamp_ms);
        let mut next = first.clone();
        next.sample_seq = 3;
        next.sample_elapsed_us = 15_000_000;
        next.fetch_started += 2;
        next.run_us += 250;
        next.heap_used = 70;
        let sample = scene_stats_sample("scene", entity, &next, &first, true).unwrap();
        assert_eq!(sample["window_s"], 10.0);
        assert_eq!(sample["sample_seq"], 3);
        assert_eq!(sample["fetch"]["started"], 2);
        assert_eq!(sample["cpu"]["run_ms"], 0.25);
        assert_eq!(sample["mem"]["heap_used"], 70);
        assert_eq!(sample["mem"]["external_bytes"], 50);
        assert_eq!(sample["crdt"]["store_bytes"], 80);
        assert!(scene_stats_sample("scene", entity, &next, &next, true).is_none());
    }

    #[test]
    fn replacement_entity_gets_fresh_baseline_for_same_scene_hash() {
        let mut world = World::new();
        let old = world.spawn(EmittedSceneStats(observation())).id();
        world.despawn(old);
        let new = world.spawn_empty().id();
        let current = observation();
        let baseline = world
            .get::<EmittedSceneStats>(new)
            .map(|b| b.0.clone())
            .unwrap_or_default();
        let sample = scene_stats_sample("same-scene", new, &current, &baseline, true).unwrap();
        assert_ne!(old, new);
        assert_eq!(sample["execution"], new.to_bits().to_string());
        assert_eq!(sample["fetch"]["started"], 7);
        assert_eq!(sample["window_s"], 5.0);
        world
            .entity_mut(new)
            .insert(EmittedSceneStats(current.clone()));
        assert!(scene_stats_sample(
            "same-scene",
            new,
            &current,
            &world.get::<EmittedSceneStats>(new).unwrap().0,
            true
        )
        .is_none());
    }

    #[test]
    fn deferred_baseline_insert_tolerates_despawn() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        world
            .commands()
            .entity(entity)
            .try_insert(EmittedSceneStats(observation()));
        world.despawn(entity);
        world.flush();
        assert!(world.get_entity(entity).is_err());
    }

    #[test]
    fn non_unix_output_has_counts_without_wall_time_disguised_as_cpu() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let sample =
            scene_stats_sample("scene", entity, &observation(), &Default::default(), false)
                .unwrap();
        assert_eq!(
            sample["cpu"],
            serde_json::json!({"ticks": 0, "updates_completed": 1, "update_errors": 0})
        );
    }

    #[test]
    fn update_cpu_bucket_boundaries_are_disjoint_and_exhaustive() {
        for (duration, bucket) in [
            (0, 0),
            (1000, 0),
            (1001, 1),
            (5000, 1),
            (5001, 2),
            (16000, 2),
            (16001, 3),
            (50000, 3),
            (50001, 4),
            (u64::MAX, 4),
        ] {
            let mut counters = SceneResourceCounters::default();
            counters.record_update_cpu(duration);
            let buckets = [
                counters.update_cpu_le_1ms,
                counters.update_cpu_1_5ms,
                counters.update_cpu_5_16ms,
                counters.update_cpu_16_50ms,
                counters.update_cpu_gt_50ms,
            ];
            assert_eq!(buckets.iter().sum::<u64>(), 1);
            assert_eq!(buckets[bucket], 1, "duration {duration}");
        }
    }
}
