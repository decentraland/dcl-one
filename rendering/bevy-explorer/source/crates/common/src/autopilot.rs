//! The benchmark autopilot's shared state: what run control (`src/autopilot.rs`) and the
//! measurement output (the CSV writer and the log layer) agree on, so neither reads the other's
//! internals. The benchmark (`docs/guidelines/benchmark.md`) runs a client with `--autopilot`
//! (`?autopilot` on the web), and its report parses the CSV and the log byte for byte
//! (`crates/perf_output`).
//!
//! Run control moves [`AutopilotPhase`] forward; the CSV writer follows it:
//! - `Arriving`: signing in and loading; nothing is recorded.
//! - `Measuring`: from the first frame in this phase, every frame is one CSV row; the writer
//!   creates its destination (header first) on entering it, so a run that never arrives
//!   leaves no CSV.
//! - `Ending`: the writer flushes and closes its destination in this frame; run control logs
//!   [`EXIT_MARKER`] and writes the `AppExit` in a later one.
//!
//! A destination the writer cannot create, write or flush is [`CsvFailed`]: run control ends the
//! run with a non-zero code.

use bevy::prelude::*;

use crate::structs::{AppConfig, PermissionType, PermissionValue};

/// Real-time seconds the autopilot stands at spawn while measuring.
pub const MEASURE_SECONDS: f64 = 90.0;

/// Real-time seconds from launch to arriving in world before the run fails.
pub const ARRIVE_TIMEOUT_SECONDS: f64 = 300.0;

/// The target run control logs under. The log layer knows the run's own exit line by it, and no
/// text from a scene or a peer chooses a target.
pub const LOG_TARGET: &str = "autopilot";

/// The harness stops counting exceptions at the first log line containing this. Run control
/// logs it once, at info level under [`LOG_TARGET`], as
/// `[ExitUtils] Exit requested at <RFC 3339 UTC time>`. The log layer breaks the string in every
/// other message, so no text can end the count early.
pub const EXIT_MARKER: &str = "[ExitUtils] Exit requested";

/// Present only under `--autopilot` (native) or `?autopilot` (web); the app assembly inserts it.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Autopilot {
    /// A CSV destination was given (`--csv`, or the web `perfStream` with `perfCsv`): stand at spawn
    /// for [`MEASURE_SECONDS`]. Without one the run ends on arriving in world; that is the
    /// harness's warm-up launch, which also creates the stored guest on a new machine.
    pub measure: bool,
}

/// Where the autopilot's run stands; inserted with [`Autopilot`].
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum AutopilotPhase {
    #[default]
    Arriving,
    Measuring,
    /// The measured time is over (`exit_code` 0) or the run failed (non-zero).
    Ending {
        exit_code: u8,
    },
}

/// Inserted by the CSV writer when its destination cannot be created, written or flushed; it logs
/// why. The CSV is then missing or cut, which is no measurement: run control ends a `Measuring`
/// run with exit code 1, and a run whose last flush failed exits with 1 although its time was up.
#[derive(Resource, Clone, Copy, Debug)]
pub struct CsvFailed;

/// The answer to a scene's permission request under the autopilot, in place of asking a person
/// and of every rule stored on the machine, so each laptop runs one policy: scenes fetch and open
/// sockets, which their workload needs, and nothing that would move the player off spawn, leave
/// the realm or touch the machine is allowed. The types that never ask keep their
/// default. Written without a wildcard: a new permission type must be placed here.
pub const fn permission(ty: PermissionType) -> PermissionValue {
    use PermissionType::*;
    match ty {
        Fetch | Websocket => PermissionValue::Allow,
        Teleport | ChangeRealm | SpawnPortable | KillPortables | Web3 | CopyToClipboard
        | OpenUrl => PermissionValue::Deny,
        MovePlayer | ForceCamera | PlayEmote | SetLocomotion | HideAvatarsNametags
        | DisableVoice | OpenExplorerUi => AppConfig::default_permission(ty),
    }
}

#[cfg(test)]
mod tests {
    use strum::IntoEnumIterator;

    use super::*;

    #[test]
    fn the_policy_never_asks_and_follows_the_table() {
        use PermissionType::*;
        for ty in PermissionType::iter() {
            let expected = match ty {
                Fetch | Websocket => PermissionValue::Allow,
                Teleport | ChangeRealm | SpawnPortable | KillPortables | Web3 | CopyToClipboard
                | OpenUrl => PermissionValue::Deny,
                _ => PermissionValue::Allow,
            };
            assert_eq!(permission(ty), expected, "{ty:?}");
        }
    }

    #[test]
    fn the_types_that_never_ask_keep_their_default() {
        for ty in PermissionType::iter() {
            if AppConfig::default_permission(ty) == PermissionValue::Allow {
                assert_eq!(permission(ty), PermissionValue::Allow, "{ty:?}");
            }
        }
    }
}
