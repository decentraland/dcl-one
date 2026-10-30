//! The presence archive's settings.

use clap::Args;

/// Presence: the headcount archive.
#[derive(Args, Clone, Debug)]
pub struct PresenceConfig {
    // presence (src/presence, the old catalyrst-presence): the headcount archive.
    /// Seconds between presence collector passes (0 = 300).
    #[arg(long, env = "PRESENCE_SNAPSHOT_INTERVAL_SECS", default_value_t = 300)]
    pub presence_snapshot_interval_secs: u64,

    /// The Genesis realm the presence collector asks comms for.
    #[arg(long, env = "PRESENCE_GENESIS_REALM", default_value = "main")]
    pub presence_genesis_realm: String,

    /// Days of presence snapshots to keep; unset keeps everything.
    #[arg(long, env = "PRESENCE_RETENTION_DAYS")]
    pub presence_retention_days: Option<u32>,

    /// Records presence passes without the rest of the data plane's writes;
    /// unset follows `DATA_WRITES_ENABLED`.
    #[arg(long, env = "PRESENCE_COLLECTOR_ENABLED", action = clap::ArgAction::Set)]
    pub presence_collector_enabled: Option<bool>,
}
