//! The LAND authorization index's settings.

use clap::Args;

/// LAND authorization: the indexer over ETH_RPC_URL.
#[derive(Args, Clone, Debug)]
pub struct LandAuthzConfig {
    // land_authz (src/land_authz): the LAND authorization indexer over
    // ETH_RPC_URL for the ETH_NETWORK registries.
    #[arg(long, env = "LAND_AUTHZ_INDEX_ENABLED", default_value_t = false, action = clap::ArgAction::Set)]
    pub land_authz_index_enabled: bool,

    /// Seconds between passes. When the index is enabled, values above 240
    /// are clamped with a warning to leave room for retries.
    #[arg(long, env = "LAND_AUTHZ_INDEX_INTERVAL_SECS", default_value_t = 120)]
    pub land_authz_index_interval_secs: u64,

    /// Log, timestamp-header and legacy-repair RPC budget per pass (minimum 2).
    /// Head probes and the headers that check a range's fork are excluded;
    /// timestamp batches contain at most 100 blocks.
    #[arg(long, env = "LAND_AUTHZ_INDEX_REQUESTS_PER_PASS", default_value_t = 50)]
    pub land_authz_index_requests_per_pass: usize,

    /// Rewind the index durably once, at the first pass able to begin it (the registry start block
    /// for a full repair, which may also switch registries). While it is set, every restart replays
    /// again: unset it once the replay has published.
    #[arg(long, env = "LAND_AUTHZ_REPLAY_FROM_BLOCK")]
    pub land_authz_replay_from_block: Option<u64>,

    /// Development bound: index no further; an index held short of the head is never published.
    #[arg(long, env = "LAND_AUTHZ_STOP_BLOCK")]
    pub land_authz_stop_block: Option<u64>,

    /// Blocks the index stays behind the chain head; a reorg past this depth is
    /// detected by the cursor block's hash and re-indexed.
    #[arg(long, env = "LAND_AUTHZ_CONFIRMATIONS", default_value_t = 12)]
    pub land_authz_confirmations: u64,
}
