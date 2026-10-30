//! The price archive's settings.

use clap::Args;

/// Price: the MANA spot archive.
#[derive(Args, Clone, Debug)]
pub struct PriceConfig {
    // price (src/price, the old catalyrst-price): the MANA spot archive.
    /// Poll CoinGecko into economy.price_snapshots from this process.
    #[arg(long, env = "PRICE_POLL_ENABLED", default_value_t = false, action = clap::ArgAction::Set)]
    pub price_poll_enabled: bool,

    /// The CoinGecko API base the poller reads; empty (the default) names
    /// none, and the poller refuses to start without one.
    #[arg(long, env = "COINGECKO_URL", default_value = "")]
    pub coingecko_url: String,

    #[arg(long, env = "PRICE_POLL_INTERVAL_SECS", default_value_t = 300)]
    pub price_poll_interval_secs: u64,

    /// The exchanges the price poller takes the MANA spot from, in order, when
    /// CoinGecko fails, as `exchange=base` (`coinbase`, `binance`, `kraken`;
    /// the mirror posture names all three); empty = none.
    #[arg(long, env = "PRICE_FALLBACK_SOURCES", default_value = "")]
    pub price_fallback_sources: String,

    /// Bearer of the price override console (the old CATALYRST_PRICE_ADMIN_TOKEN); unset refuses it.
    #[arg(long, env = "PRICE_ADMIN_TOKEN", hide_env_values = true)]
    pub price_admin_token: Option<String>,
}
