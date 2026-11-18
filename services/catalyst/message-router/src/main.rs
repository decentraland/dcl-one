//! `catalyst-message-router`: the community chat message router's process (crate docs in
//! `lib.rs`). It reads the database, asks the gatekeeper for its token, and serves `/metrics`
//! and `/health/*` on `MESSAGE_ROUTER_METRICS_BIND` until SIGINT or SIGTERM.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use clap::Parser;

use catalyst_message_router::config::{Cli, key_from_env};
use catalyst_message_router::membership::PgMembership;
use catalyst_message_router::metrics::{self, Connection};
use catalyst_message_router::participant::Router;
use catalyst_message_router::rate_limiter::RateLimiter;
use catalyst_message_router::token::{TokenClient, check_url};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("catalyst_message_router=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    let cli = Cli::parse();
    cli.check_posture(&|key| std::env::var(key).ok())?;
    let key = key_from_env()?;
    let token_url = check_url(&cli.token_url)?;

    let handle = metrics::install()?;
    let connection = Connection::default();
    tokio::spawn({
        let handle = handle.clone();
        let connection = connection.clone();
        let bind = cli.metrics_bind;
        async move {
            if let Err(e) = metrics::serve(bind, handle, connection).await {
                tracing::error!(error = %e, "the metrics listener stopped");
                std::process::exit(1);
            }
        }
    });

    let pool = catalyst_db::connect_pool(
        &cli.database_url,
        &catalyst_db::PoolSettings {
            max_connections: cli.db_max_connections,
            acquire_timeout_secs: Some(5),
            statement_timeout_ms: 5_000,
            ..Default::default()
        },
    )
    .await?;
    catalyst_db::bootstrap(&pool).await?;

    let limiter = Arc::new(Mutex::new(RateLimiter::new(cli.limits())));
    tokio::spawn({
        let limiter = limiter.clone();
        let every = cli.cleanup_interval();
        async move {
            let mut tick = tokio::time::interval(every);
            loop {
                tick.tick().await;
                limiter.lock().expect("rate limiter").sweep(Instant::now());
            }
        }
    });

    let router = Arc::new(Router {
        tokens: TokenClient::new(token_url, key, cli.replica)?,
        livekit_url: cli.livekit_url.clone(),
        retry: cli.retry(),
        max_packet_size_bytes: cli.max_packet_size_bytes,
        members: Arc::new(PgMembership::new(pool)),
        limiter,
        connection,
    });
    router.run(shutdown_signal()).await
}

async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut interrupt = signal(SignalKind::interrupt()).expect("installing the SIGINT handler");
    let mut term = signal(SignalKind::terminate()).expect("installing the SIGTERM handler");
    tokio::select! {
        _ = interrupt.recv() => tracing::info!("SIGINT received"),
        _ = term.recv() => tracing::info!("SIGTERM received"),
    }
}
