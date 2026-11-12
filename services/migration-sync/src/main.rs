//! Operator entry point for the isolated one-way synchronization rehearsal.
use std::{path::PathBuf, time::Duration};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use dcl_migration_sync::{capture, dispatch, model::Envelope, ops, store};
use sqlx::postgres::PgPoolOptions;
use tokio::io::AsyncReadExt;
use uuid::Uuid;

const MAX_ENVELOPE_BYTES: u64 = 1024 * 1024;

#[derive(Parser)]
#[command(
    version,
    about = "Standalone production-to-dcl-one sync rehearsal worker"
)]
struct Args {
    #[arg(long, env = "MIGRATION_SYNC_DATABASE_URL", hide_env_values = true)]
    database_url: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Fail unless capture is fresh, application caught up, and the stream unpaused.
    Check {
        #[arg(long)]
        stream: String,
        #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..=3600))]
        max_age_seconds: u64,
    },
    /// Emit payload-free Prometheus gauges for a supervised textfile collector.
    Metrics {
        #[arg(long)]
        stream: String,
    },
    /// Revalidate one immutable blocked head after resolving its cause; requires pause.
    RetryBlocked {
        #[arg(long)]
        stream: String,
        #[arg(long)]
        expected_cursor: String,
    },
    /// Explicitly install the source-local capture adapter for reviewed Genesis IDs.
    InstallSource {
        #[arg(long, required = true, num_args = 1..)]
        entity: Vec<Uuid>,
    },
    /// Import the scoped favorite baseline after catalogue migration; starts paused.
    Bootstrap {
        #[arg(long)]
        stream: String,
    },
    /// Copy committed source outbox transactions into the durable destination inbox.
    Capture {
        #[arg(long)]
        stream: String,
        #[arg(long)]
        once: bool,
        #[arg(long, default_value_t = 1000, value_parser = clap::value_parser!(u64).range(100..=60000))]
        poll_ms: u64,
    },
    /// Compare scoped favorite rows at an unchanged, caught-up source boundary.
    Reconcile {
        #[arg(long)]
        stream: String,
    },
    /// Register an immutable snapshot boundary; the new stream starts paused.
    Register {
        #[arg(long)]
        stream: String,
        #[arg(long)]
        generation: Uuid,
        #[arg(long)]
        baseline_lsn: String,
    },
    /// Durably enqueue one trusted, normalized source transaction (not raw WAL).
    Enqueue { file: PathBuf },
    /// Apply ordered transactions to an isolated destination with competing writers off.
    Run {
        #[arg(long)]
        stream: String,
        /// Acknowledge this increment supports isolated rehearsal targets only.
        #[arg(long, required = true)]
        rehearsal: bool,
        #[arg(long)]
        once: bool,
        #[arg(long, default_value_t = 1000, value_parser = clap::value_parser!(u64).range(100..=60000))]
        poll_ms: u64,
    },
    /// Stop application at a transaction boundary while retaining queued input.
    Pause {
        #[arg(long)]
        stream: String,
    },
    /// Resume a paused rehearsal stream; does not skip or clear blocked changes.
    Resume {
        #[arg(long)]
        stream: String,
        #[arg(long, required = true)]
        rehearsal: bool,
    },
    /// Print checkpoints and queue counts without request payloads or credentials.
    Status {
        #[arg(long)]
        stream: String,
    },
}

async fn read_envelope(file: &PathBuf) -> Result<Envelope> {
    let mut bytes = Vec::new();
    tokio::fs::File::open(file)
        .await
        .context("open transaction envelope")?
        .take(MAX_ENVELOPE_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .context("read transaction envelope")?;
    if bytes.len() as u64 > MAX_ENVELOPE_BYTES {
        bail!("transaction envelope exceeds 1 MiB");
    }
    // The serde error may contain source data; expose only its location.
    let envelope: Envelope = serde_json::from_slice(&bytes).map_err(|error| {
        anyhow::anyhow!(
            "invalid envelope JSON at line {}, column {}",
            error.line(),
            error.column()
        )
    })?;
    envelope.validate()?;
    Ok(envelope)
}

async fn shutdown_signal() -> Result<()> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            signal = tokio::signal::ctrl_c() => signal?,
            _ = terminate.recv() => {},
        }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await?;
    Ok(())
}

async fn source_pool() -> Result<sqlx::PgPool> {
    let url = std::env::var("MIGRATION_SYNC_SOURCE_DATABASE_URL")
        .context("MIGRATION_SYNC_SOURCE_DATABASE_URL is required for source operations")?;
    PgPoolOptions::new()
        .max_connections(3)
        .acquire_timeout(Duration::from_secs(10))
        .after_connect(|connection, _| {
            Box::pin(async move {
                sqlx::raw_sql("SET statement_timeout = '30s'; SET lock_timeout = '5s'")
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .context("connect to capture source")
}

fn dispatch_config() -> Result<Option<(String, String)>> {
    match (
        std::env::var("MIGRATION_SYNC_FOLLOWER_URL").ok(),
        std::env::var("MIGRATION_FOLLOWER_TOKEN").ok(),
    ) {
        (None, None) => Ok(None),
        (Some(url), Some(token)) => {
            let parsed =
                reqwest::Url::parse(&url).map_err(|_| anyhow::anyhow!("invalid follower URL"))?;
            let loopback = matches!(parsed.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
            if !(parsed.scheme() == "https" || parsed.scheme() == "http" && loopback)
                || !parsed.username().is_empty()
                || parsed.password().is_some()
                || parsed.query().is_some()
                || parsed.fragment().is_some()
                || parsed.path() != "/internal/migration/favorites/replay"
                || token.len() < 32
            {
                bail!(
                    "follower requires HTTPS (or loopback HTTP), the exact replay path, and a token of at least 32 bytes"
                );
            }
            Ok(Some((url, token)))
        }
        _ => bail!("follower URL and token must be configured together"),
    }
}

async fn execute(args: Args) -> Result<()> {
    // Validate/size-bound input before opening any connection.
    let envelope = match &args.command {
        Command::Enqueue { file } => Some(read_envelope(file).await?),
        _ => None,
    };
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .acquire_timeout(Duration::from_secs(10))
        .after_connect(|connection, _| {
            Box::pin(async move {
                sqlx::raw_sql("SET statement_timeout = '30s'; SET lock_timeout = '5s'")
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&args.database_url)
        .await
        .context("connect to migration sync destination")?;
    match args.command {
        Command::Check {
            stream,
            max_age_seconds,
        } => {
            ops::check_health(&ops::status(&pool, &stream).await?, max_age_seconds)?;
            println!("stream healthy at observed capture boundary");
        }
        Command::Metrics { stream } => {
            print!("{}", ops::metrics(&ops::status(&pool, &stream).await?)?)
        }
        Command::RetryBlocked {
            stream,
            expected_cursor,
        } => ops::retry_blocked(&pool, &stream, &expected_cursor).await?,
        Command::InstallSource { entity } => {
            let source = source_pool().await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&capture::install(&source, &entity).await?)?
            );
            source.close().await;
        }
        Command::Bootstrap { stream } => {
            let source = source_pool().await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&capture::bootstrap(&source, &pool, &stream).await?)?
            );
            source.close().await;
        }
        Command::Reconcile { stream } => {
            let source = source_pool().await?;
            let report = capture::reconcile(&source, &pool, &stream).await?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            source.close().await;
            if report["matches"] != true {
                bail!("favorite reconciliation found a mismatch");
            }
        }
        Command::Capture {
            stream,
            once,
            poll_ms,
        } => {
            let source = source_pool().await?;
            let run = async {
                loop {
                    let captured = capture::pump_once(&source, &pool, &stream).await?;
                    if once {
                        return Ok::<(), anyhow::Error>(());
                    }
                    if !captured {
                        tokio::time::sleep(Duration::from_millis(poll_ms)).await;
                    }
                }
            };
            tokio::select! {
                result = run => result?,
                result = shutdown_signal() => { result?; tracing::info!("capture stopping"); }
            }
            source.close().await;
        }
        Command::Register {
            stream,
            generation,
            baseline_lsn,
        } => {
            store::register(&pool, &stream, generation, &baseline_lsn).await?;
            println!("stream registered; resume explicitly before application");
        }
        Command::Enqueue { .. } => {
            let envelope = envelope.context("missing validated envelope")?;
            let inserted = store::enqueue(&pool, &envelope).await?;
            println!("{}", if inserted { "queued" } else { "already queued" });
        }
        Command::Pause { stream } => store::pause(&pool, &stream, true).await?,
        Command::Resume { stream, .. } => store::pause(&pool, &stream, false).await?,
        Command::Status { stream } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&ops::status(&pool, &stream).await?)?
            );
        }
        Command::Run {
            stream,
            once,
            poll_ms,
            ..
        } => {
            let config = dispatch_config()?;
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .redirect(reqwest::redirect::Policy::none())
                .build()?;
            let run = async {
                loop {
                    let outcome = match &config {
                        Some((endpoint, token)) => {
                            dispatch::process_next(&pool, &stream, &client, endpoint, token).await?
                        }
                        None => store::process_next(&pool, &stream).await?,
                    };
                    match outcome {
                        store::Outcome::Applied => tracing::info!("source transaction applied"),
                        store::Outcome::Blocked => {
                            bail!("stream blocked; inspect status and reconcile before proceeding")
                        }
                        store::Outcome::AwaitingCommand => {
                            if config.is_none() {
                                bail!(
                                    "head requires native replay; configure follower URL and token"
                                );
                            }
                            tracing::info!(
                                "native receipt pending; retaining ordered head for retry"
                            );
                            if !once {
                                tokio::time::sleep(Duration::from_millis(poll_ms)).await;
                            }
                        }
                        store::Outcome::Idle => {
                            if !once {
                                tokio::time::sleep(Duration::from_millis(poll_ms)).await;
                            }
                        }
                    }
                    if once {
                        return Ok::<(), anyhow::Error>(());
                    }
                }
            };
            tokio::select! {
                result = run => result?,
                result = shutdown_signal() => { result?; tracing::info!("sync worker stopping"); }
            }
        }
    }
    pool.close().await;
    Ok(())
}

fn operator_error(error: &anyhow::Error) -> String {
    if error.chain().any(|cause| cause.is::<sqlx::Error>()) {
        "database operation failed; inspect stream status and database diagnostics".into()
    } else {
        error.to_string()
    }
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();
    match execute(Args::parse()).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(message = %operator_error(&error), "sync worker failed");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serving_mutations_requires_explicit_rehearsal_mode() {
        assert!(
            Args::try_parse_from([
                "sync",
                "--database-url",
                "postgres://test/db",
                "run",
                "--stream",
                "places"
            ])
            .is_err()
        );
        assert!(
            Args::try_parse_from([
                "sync",
                "--database-url",
                "postgres://test/db",
                "run",
                "--stream",
                "places",
                "--rehearsal",
                "--once"
            ])
            .is_ok()
        );
        assert!(
            Args::try_parse_from([
                "sync",
                "--database-url",
                "postgres://test/db",
                "resume",
                "--stream",
                "places"
            ])
            .is_err()
        );
    }

    #[test]
    fn polling_is_bounded() {
        assert!(
            Args::try_parse_from([
                "sync",
                "--database-url",
                "postgres://test/db",
                "run",
                "--stream",
                "places",
                "--rehearsal",
                "--poll-ms",
                "0"
            ])
            .is_err()
        );
    }

    struct InputFile(PathBuf);

    impl InputFile {
        fn new(bytes: &[u8]) -> Self {
            let path =
                std::env::temp_dir().join(format!("migration-sync-input-{}.json", Uuid::new_v4()));
            std::fs::write(&path, bytes).expect("write input fixture");
            Self(path)
        }
    }

    impl Drop for InputFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn envelope_json() -> serde_json::Value {
        serde_json::json!({
            "version": 1,
            "source": "places",
            "stream": "places-favorites",
            "generation": "10000000-0000-4000-8000-000000000001",
            "previous_lsn": "0/0",
            "commit_lsn": "0/10",
            "mode": "source_only",
            "changes": []
        })
    }

    #[tokio::test]
    async fn envelope_byte_limit_is_inclusive() {
        let mut bytes = serde_json::to_vec(&envelope_json()).unwrap();
        bytes.resize(MAX_ENVELOPE_BYTES as usize, b' ');
        let file = InputFile::new(&bytes);
        let envelope = read_envelope(&file.0)
            .await
            .expect("exact-limit valid JSON");
        assert_eq!(envelope.commit_lsn, "0/10");
    }

    #[tokio::test]
    async fn oversized_envelope_is_rejected_before_parsing() {
        let mut bytes = serde_json::to_vec(&envelope_json()).unwrap();
        bytes.resize(MAX_ENVELOPE_BYTES as usize + 1, b' ');
        let file = InputFile::new(&bytes);
        let error = read_envelope(&file.0).await.expect_err("oversized input");
        assert_eq!(error.to_string(), "transaction envelope exceeds 1 MiB");
    }

    #[tokio::test]
    async fn malformed_and_unknown_fields_do_not_leak_payload_values() {
        const CANARY: &str = "sensitive-payload-canary";
        let mut unknown_mode = envelope_json();
        unknown_mode["mode"] = serde_json::json!(CANARY);
        let mut unknown_field = envelope_json();
        unknown_field[CANARY] = serde_json::json!("private");
        let invalid_json = format!("{{\"{CANARY}\": ");
        for bytes in [
            serde_json::to_vec(&unknown_mode).unwrap(),
            serde_json::to_vec(&unknown_field).unwrap(),
            invalid_json.into_bytes(),
            Vec::new(),
            vec![0xff, 0xfe],
        ] {
            let file = InputFile::new(&bytes);
            let error = read_envelope(&file.0)
                .await
                .expect_err("invalid JSON contract");
            let diagnostic = format!("{error:?}");
            assert!(diagnostic.starts_with("invalid envelope JSON at line "));
            assert!(!diagnostic.contains(CANARY));
            assert!(!diagnostic.contains("private"));
        }
    }

    #[tokio::test]
    async fn semantic_validation_does_not_leak_source_values() {
        let mut payload = envelope_json();
        payload["source"] = serde_json::json!("sensitive-source-canary");
        let file = InputFile::new(&serde_json::to_vec(&payload).unwrap());
        let error = read_envelope(&file.0)
            .await
            .expect_err("unsupported source");
        assert_eq!(format!("{error:?}"), "unsupported source");
    }

    #[tokio::test]
    async fn invalid_input_is_rejected_before_database_connection() {
        let file = InputFile::new(b"not JSON");
        let args = Args {
            database_url: "not-a-database-url-with-secret-canary".to_owned(),
            command: Command::Enqueue {
                file: file.0.clone(),
            },
        };
        let error = execute(args).await.expect_err("input rejected first");
        let diagnostic = format!("{error:?}");
        assert!(diagnostic.starts_with("invalid envelope JSON at line "));
        assert!(!diagnostic.contains("secret-canary"));
        assert!(!diagnostic.contains("connect"));
    }

    #[test]
    fn database_errors_are_redacted_through_context_chains() {
        let database_error = sqlx::Error::Configuration(Box::new(std::io::Error::other(
            "postgres://user:password-canary@example.invalid/private",
        )));
        let error = anyhow::Error::new(database_error).context("destination initialization");
        let diagnostic = operator_error(&error);
        assert!(!diagnostic.contains("password-canary"));
        assert!(!diagnostic.contains("example.invalid"));
        assert!(!diagnostic.contains("private"));
        assert!(!diagnostic.is_empty());
    }
}
