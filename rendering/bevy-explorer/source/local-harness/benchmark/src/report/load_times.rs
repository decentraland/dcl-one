//! The run's load times, read from its log before the exit line:
//! - time to world: the autopilot's `phase Measuring at <s> s` entry (category `autopilot`), the
//!   app's real time when the run arrived in world; on the web the page load and the wasm
//!   download and compile come before the app starts and are not counted;
//! - first ring ready: the first `player's scene and first ring ready <ms> ms after the scene
//!   spawned` entry (category `scene_timing`), the arrival at spawn, timed from the spawn
//!   scene's spawn while the player waits behind the loading screen.
//!
//! A log without an entry gives no value for it.

use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;

use super::exceptions::{entry_start, EXIT_MARKER};
use super::Lines;

/// The autopilot's log target (`common::autopilot::LOG_TARGET`).
pub const AUTOPILOT_CATEGORY: &str = "autopilot";
/// The scene load timing's log target (`scene_runner::load_timing::LOG_TARGET`).
pub const SCENE_TIMING_CATEGORY: &str = "scene_timing";
/// What the autopilot logs on entering the measured phase, before the seconds since launch.
pub const ARRIVED_IN_WORLD_PREFIX: &str = "phase Measuring at ";
/// What the scene load timing logs when an arrival's scene and first ring are ready, before the
/// milliseconds since the scene spawned (`scene_runner::load_timing::FIRST_RING_READY` and a
/// space).
pub const FIRST_RING_READY_PREFIX: &str = "player's scene and first ring ready ";

/// One run's load times, in milliseconds, in the order of [`super::stats::LOAD_TIME_METRICS`].
pub type RunLoadTimes = [Option<f32>; 2];

pub fn read_load_times(log: &Path) -> io::Result<RunLoadTimes> {
    if !log.is_file() {
        return Ok([None, None]);
    }
    load_times(Lines::new(BufReader::new(File::open(log)?)))
}

fn load_times(lines: impl Iterator<Item = io::Result<String>>) -> io::Result<RunLoadTimes> {
    let mut time_to_world_ms = None;
    let mut first_ring_ms = None;
    for line in lines {
        let line = line?;
        if line.contains(EXIT_MARKER) {
            break;
        }
        let Some((category, text)) = entry_start(&line) else {
            continue;
        };
        if category == AUTOPILOT_CATEGORY && time_to_world_ms.is_none() {
            time_to_world_ms = text
                .strip_prefix(ARRIVED_IN_WORLD_PREFIX)
                .and_then(|rest| rest.strip_suffix(" s"))
                .and_then(|seconds| seconds.parse::<f32>().ok())
                .map(|seconds| seconds * 1000.0);
        } else if category == SCENE_TIMING_CATEGORY && first_ring_ms.is_none() {
            first_ring_ms = text
                .strip_prefix(FIRST_RING_READY_PREFIX)
                .and_then(|rest| rest.split_once(" ms"))
                .and_then(|(ms, _)| ms.parse::<f32>().ok());
        }
    }
    Ok([time_to_world_ms, first_ring_ms])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_arrival_before_the_exit_line_counts() {
        let log = concat!(
            "10:00:00.000 - <color=#DBD6C8>[autopilot]</color>: phase Arriving at 0.0 s\n",
            "10:00:01.000 - <color=#DBD6C8>[SCENE]</color>: phase Measuring at 1.0 s\n",
            "10:00:12.000 - <color=#DBD6C8>[autopilot]</color>: phase Measuring at 12.3 s\n",
            "10:00:14.000 - <color=#DBD6C8>[scene_timing]</color>: scene ready 900 ms after spawn: bafy ring 0 tick 5\n",
            "10:00:15.000 - <color=#DBD6C8>[scene_timing]</color>: player's scene and first ring ready 4321 ms after the scene spawned\n",
            "10:00:30.000 - <color=#DBD6C8>[scene_timing]</color>: player's scene and first ring ready 99 ms after the scene spawned\n",
            "10:01:35.000 - <color=#DBD6C8>[autopilot]</color>: [ExitUtils] Exit requested at 2026-10-28T10:01:35.000Z\n",
        );
        assert_eq!(
            load_times(Lines::new(log.as_bytes())).unwrap(),
            [Some(12_300.0), Some(4321.0)]
        );
    }

    #[test]
    fn entries_after_the_exit_line_or_missing_give_no_value() {
        let log = concat!(
            "10:01:35.000 - <color=#DBD6C8>[autopilot]</color>: [ExitUtils] Exit requested at 2026-10-28T10:01:35.000Z\n",
            "10:01:36.000 - <color=#DBD6C8>[scene_timing]</color>: player's scene and first ring ready 4321 ms after the scene spawned\n",
        );
        assert_eq!(
            load_times(Lines::new(log.as_bytes())).unwrap(),
            [None, None]
        );
        assert_eq!(load_times(Lines::new(&b""[..])).unwrap(), [None, None]);
    }
}
