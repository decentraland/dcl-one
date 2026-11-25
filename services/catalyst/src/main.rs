use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::{Parser, Subcommand};

use catalyst::config::Config;
use catalyst::{builder, comms, content, db, explore, http, ops, storage};
use catalyst_config::posture;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[cfg(test)]
mod cli_tests;
#[cfg(test)]
mod preflight_tests;

#[derive(Parser)]
#[command(name = "catalyst", version, about = "Decentraland Catalyst node")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Verify the schema, attach the workers and serve HTTP.
    Serve(Box<Config>),
    /// Run chain indexes and marketplace mirrors with a private metrics listener.
    Index(Box<Config>),
    /// Administrative schema operations.
    Schema {
        #[command(subcommand)]
        command: SchemaCommand,
    },
    /// Builder operator commands (the old `catalyrst-builder catalog|seed`).
    Builder {
        #[command(subcommand)]
        command: BuilderCommand,
    },
    /// Content server operator commands.
    Content {
        #[command(subcommand)]
        command: ContentCommand,
    },
    /// Blob store operator commands.
    Storage {
        #[command(subcommand)]
        command: StorageCommand,
    },
    /// Imposter store operator commands (the old `catalyrst-bvimposters
    /// seed|quarantine`), on the store at `BVIMPOSTERS_STORE_ROOT`.
    Bvimposters {
        #[command(subcommand)]
        command: BvimpostersCommand,
    },
    /// Run Pulse (the ENet/UDP + WebTransport realtime layer) in its own
    /// process; reads the `PULSE_*` contract (`comms::pulse::env::ENV_DOCS`)
    /// and installs its own tracing and Prometheus exporter. Deployed as a
    /// second unit beside `serve` (AGENTS.md, "Ownership").
    Pulse,
    /// Community operator commands.
    Communities {
        #[command(subcommand)]
        command: CommunitiesCommand,
    },
    /// Camera reel operator commands.
    CameraReel {
        #[command(subcommand)]
        command: CameraReelCommand,
    },
    /// Shop operator commands.
    Shop {
        #[command(subcommand)]
        command: ShopCommand,
    },
    /// Explore (places, events) operator commands.
    Explore {
        #[command(subcommand)]
        command: ExploreCommand,
    },
    /// The secrets a self-contained host mints for itself (deploy/README.md, "Secrets").
    Secrets {
        #[command(subcommand)]
        command: SecretsCommand,
    },
    /// The node postures `NODE_POSTURE` names.
    Posture {
        #[command(subcommand)]
        command: PostureCommand,
    },
}

#[derive(Subcommand)]
enum PostureCommand {
    /// Print every variable `NODE_POSTURE=<name>` sets and its value, by
    /// family (a variable set in the environment wins over it).
    Show {
        #[arg(value_parser = ["mirror", "island", "journey"])]
        name: String,
    },
    /// Print `KEY=value` for the listed keys (every key when none is listed)
    /// as `NODE_POSTURE=<name>` sets them in an empty environment: the
    /// environment file of a process that reads part of the table without
    /// running this binary (the sites server). A key the posture leaves to
    /// the binary's own default prints nothing.
    Env {
        #[arg(value_parser = ["mirror", "island", "journey"])]
        name: String,
        keys: Vec<String>,
    },
}

#[derive(Subcommand)]
enum ShopCommand {
    /// Move a chain notification producer's cursor (`shop.notification_cursor`)
    /// back, with its replay floor, so its next passes announce every sale,
    /// royalty, rental or transfer after that point again (runbook:
    /// docs/shop-notices.md). A producer no pass has seeded yet is seeded
    /// there and its first pass keeps it: the cutover seed from a legacy
    /// cursor. Never moves a cursor forward. A notice already stored keeps
    /// its id, so it is neither stored nor emailed twice. `--since` resolves
    /// through the logs of the indexes the producer reads and is refused
    /// until they have indexed past it. Prints the outcome as JSON and
    /// records it in `ops.admin_audit` as `--actor`. Not behind
    /// OPS_WRITES_ENABLED: the producers' own switches decide whether
    /// anything is emitted.
    NoticeRewind {
        #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
        database_url: String,
        #[arg(long, value_parser = clap::builder::PossibleValuesParser::new(catalyst::shop::market::notice_cursor::PRODUCERS))]
        producer: String,
        /// Announce the logs of this block (1 or more) and later.
        #[arg(
            long,
            conflicts_with = "since",
            required_unless_present = "since",
            value_parser = clap::value_parser!(i64).range(1..)
        )]
        from_block: Option<i64>,
        /// Announce the logs mined at or after this time (for `rental_ended`,
        /// the periods ending at or after it): RFC 3339, `YYYY-MM-DD` (UTC)
        /// or epoch milliseconds, the legacy cursor's unit.
        #[arg(long, value_parser = catalyst::shop::market::notice_cursor::parse_since)]
        since: Option<i64>,
        /// Resolve and print, with each index's logs after the resulting
        /// cursor (`logs_after`); write nothing.
        #[arg(long)]
        dry_run: bool,
        /// The chain the node's trade-events walk reads on each network, as
        /// it configures it: `--since` counts a trade_events history only
        /// when it was proven on that chain.
        #[arg(long, env = "MARKETPLACE_INDEX_CHAIN_ID_ETHEREUM", default_value_t = 1)]
        marketplace_index_chain_id_ethereum: i64,
        #[arg(long, env = "MARKETPLACE_INDEX_CHAIN_ID_MATIC", default_value_t = 137)]
        marketplace_index_chain_id_matic: i64,
        /// Who ran it, recorded as the `ops.admin_audit` actor: the
        /// operator's name, `$USER` when not given, `cli` when neither is set.
        #[arg(long, env = "USER", hide_env_values = true)]
        actor: Option<String>,
    },
    /// Compare one chain index's logs over a block range as the RPC and a
    /// portal speaking the SQD portal protocol return them, window by
    /// window. Read-only: a read-only database connection for the index's
    /// filters, no lease, no write, no schema bootstrap. The RPC is
    /// `ETH_RPC_URL` or `RPC_ENDPOINT_POLYGON` (or their `_FILE`), the portal
    /// key `CHAIN_LOGS_PORTAL_API_KEY` (or its `_FILE`). The network, the
    /// dataset and the RPC must name one chain, or the run is refused. A
    /// read that fails ends the run with what was compared and the rest
    /// uncovered. Prints one JSON report, whatever happens; exits 4 when
    /// refused, else 1 when anything differed, else 3 when part of the range
    /// or a compared field was not compared, else 0. Without a report, 2 is
    /// a usage error and 1 a process setup error before the command runs (an
    /// invalid NODE_POSTURE): a difference always prints its report.
    CompareSources {
        #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
        database_url: String,
        #[arg(long, value_parser = ["ethereum", "matic"])]
        net: String,
        #[arg(long, value_parser = ["nft", "sales", "trade_events", "committee", "mana_transfers", "rentals", "third_party_registry", "collections"])]
        indexer: String,
        #[arg(long)]
        from: u64,
        #[arg(long)]
        to: u64,
        /// Blocks per window.
        #[arg(long, default_value_t = 10_000)]
        window: u64,
        #[arg(long, env = "CHAIN_LOGS_PORTAL_URL")]
        portal_url: String,
        /// The portal dataset; CHAIN_LOGS_PORTAL_DATASET_<NET> when not
        /// given, else the network's mainnet dataset.
        #[arg(long)]
        dataset: Option<String>,
        #[arg(long, env = "CHAIN_LOGS_PORTAL_MAX_RPS", default_value_t = 1.0)]
        max_rps: f64,
        /// Report the portal as holding every log of its dataset: an
        /// operator assertion, labelled as such.
        #[arg(long, env = "CHAIN_LOGS_PORTAL_ASSERT_FULL_HISTORY", default_value_t = false, action = clap::ArgAction::Set)]
        assert_full_history: bool,
    },
}

#[derive(Subcommand)]
enum ExploreCommand {
    /// Import upstream's `user_likes` export as the ground truth of the like
    /// rates: one JSON object a line, as `psql "$OLD_PLACES_DB" -Atc "COPY
    /// (SELECT row_to_json(l) FROM user_likes l) TO STDOUT"` writes it. Each
    /// row becomes a like fact with `source = 'upstream'` and its voting
    /// power at vote time; a wallet that already voted on this node keeps
    /// its local fact. Each entity is settled and recounted in its own
    /// transaction, once: a later export adds nothing to a settled entity, so
    /// export after the old service stops taking votes. A malformed line
    /// imports nothing. Prints the outcome as JSON. Not behind
    /// DATA_WRITES_ENABLED: an import is operator data.
    ImportLikes {
        #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
        database_url: String,
        /// The export; `-` reads standard input.
        #[arg(long)]
        file: String,
    },
}

#[derive(Subcommand)]
enum CameraReelCommand {
    /// Copy camera-reel-service's photos into `camera_reel.images`
    /// (`camera_reel::import`; runbook: docs/camera-reel.md): pages the old
    /// service's `images` table by `(created_at, id)`, fetches each photo and
    /// thumbnail from its bucket (`<bucket-url>/<key>`, the key ending the
    /// row's `url`/`thumbnail_url`), generates the 640x360 thumbnail when the
    /// row has none or the bucket lacks it, stores both blobs by hash and
    /// inserts the row under the same id, owner, document, visibility and
    /// `created_at`, unmoderated, with no ranking count and no photo event,
    /// and without `MAX_IMAGES_PER_USER`. Re-running is safe and resumes: a
    /// photo already imported is not fetched again, only its visibility and
    /// document follow the source while it is unmoderated. A row the local
    /// table refuses or whose objects are missing is skipped and reported;
    /// the run goes on. Prints the outcome as JSON. Not behind
    /// CREATE_WRITES_ENABLED: an import is operator data.
    Import {
        #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
        database_url: String,
        /// camera-reel-service's database (read only).
        #[arg(long, env = "CAMERA_REEL_SOURCE_DATABASE_URL", hide_env_values = true)]
        source_database_url: String,
        /// camera-reel-service's public bucket (its `BUCKET_URL`).
        #[arg(long, env = "CAMERA_REEL_SOURCE_BUCKET_URL", hide_env_values = true)]
        bucket_url: String,
        #[arg(
            long,
            env = "STORAGE_ROOT_FOLDER",
            default_value = "/var/lib/catalyst/content"
        )]
        storage_root_folder: String,
        /// Source rows per page.
        #[arg(long, default_value_t = 500, value_parser = clap::value_parser!(i64).range(1..=10_000))]
        batch: i64,
        /// Photos fetched at once.
        #[arg(long, default_value_t = 8, value_parser = clap::value_parser!(u16).range(1..=256))]
        concurrency: u16,
        /// Fetch and check every photo; write no blob and no row.
        #[arg(long)]
        dry_run: bool,
        /// After the pass, delete the imported photos the source no longer
        /// has (photos uploaded here are never touched).
        #[arg(long)]
        prune: bool,
    },
}

#[derive(Subcommand)]
enum CommunitiesCommand {
    /// One pass of the upstream communities mirror (the `communities-mirror`
    /// job of `COMMUNITIES_MIRROR_UPSTREAM`, on demand): pages the upstream
    /// public listing into `social.communities` as read-only mirrored rows
    /// (no local roles; writes on them answer 403), resuming an interrupted
    /// pass, and prints the outcome as JSON. A complete pass deactivates the
    /// mirrored communities not listed for `--grace-secs`. Not behind
    /// SOCIAL_WRITES_ENABLED: a mirror is not local authority.
    Mirror {
        #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
        database_url: String,
        #[arg(
            long,
            env = "COMMUNITIES_UPSTREAM_URL",
            default_value = "https://social-api.decentraland.org"
        )]
        upstream_url: String,
        #[arg(
            long,
            env = "COMMUNITIES_MIRROR_THUMBNAIL_URL",
            default_value = "https://assets-cdn.decentraland.org"
        )]
        thumbnail_url: String,
        #[arg(long, default_value_t = 7200)]
        grace_secs: u64,
    },
}

#[derive(Subcommand)]
enum SecretsCommand {
    /// Fill every slot `--serve-file` lacks (the LiveKit API key pair the SFU
    /// on this host shares, `PULSE_ROOM_AUTHORITY_KEY`, `ENCRYPTION_KEY`, the
    /// authoritative scene-host wallet with `AUTHORITATIVE_SERVER_ADDRESS`, the
    /// admin bearers; `AB_REGISTRY_ADMIN_TOKEN` with `--with-abgen`), keep every
    /// present line as it is, and derive `--pulse-file` from the result. Files
    /// land 0400 in a 0700 directory; nothing is rewritten when nothing changed.
    /// Prints the slots it minted. The `catalyst-secrets-init` oneshot runs it
    /// before serve, pulse and the SFU on a host with `[secrets].generated`.
    Init {
        #[arg(long)]
        serve_file: PathBuf,
        #[arg(long)]
        pulse_file: Option<PathBuf>,
        #[arg(long)]
        with_abgen: bool,
    },
}

#[derive(Subcommand)]
enum BuilderCommand {
    /// Walk an `@dcl/asset-packs` tree, store every asset file in the blob
    /// store and upsert the one `builder.asset_catalog` row `/v1/assetPacks`
    /// serves (503 with this command's hint until it runs). Given the npm
    /// package's `catalog.json` instead, upsert the row from it and store
    /// nothing: the files are pulled from the builder bucket on first read.
    Catalog {
        packs_dir: String,
        #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
        database_url: String,
        #[arg(
            long,
            env = "STORAGE_ROOT_FOLDER",
            default_value = "/var/lib/catalyst/content"
        )]
        storage_root_folder: String,
    },
    /// Pull the named asset-pack files from a bucket into the blob store.
    Seed {
        base_url: String,
        hashes: Vec<String>,
        #[arg(
            long,
            env = "STORAGE_ROOT_FOLDER",
            default_value = "/var/lib/catalyst/content"
        )]
        storage_root_folder: String,
    },
}

#[derive(Subcommand)]
enum ContentCommand {
    /// Record the avatar renders active profiles name in
    /// `metadata.avatars[].avatar.snapshots` but not in `content[]`
    /// (`content::profile_snapshots::repair`; DATABASE.md, "Profile snapshot
    /// repair workload"): pages the active profiles by id, fetches each
    /// render the store lacks from `SYNC_SOURCE` and inserts its
    /// `content.content_files` row (`{name}.png`) once the blob is stored, so
    /// the blob collector and the history prune keep it. Re-running is safe:
    /// present rows are kept, and a render no source served is tried again.
    /// Prints the outcome as JSON.
    RepairProfileSnapshots {
        #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
        database_url: String,
        #[arg(
            long,
            env = "STORAGE_ROOT_FOLDER",
            default_value = "/var/lib/catalyst/content"
        )]
        storage_root_folder: String,
        /// The peers asked for a missing render (comma-separated).
        #[arg(long, env = "SYNC_SOURCE")]
        sync_source: String,
        /// Profiles per batch.
        #[arg(long, default_value_t = 500, value_parser = clap::value_parser!(i64).range(1..=10_000))]
        batch: i64,
    },
    /// List every entity id and content hash of the old content server's
    /// denylist file (`<STORAGE_ROOT_FOLDER>/<DENYLIST_FILE_NAME>` on that
    /// server, `denylist.txt` by default) in `content.denylist`, as console
    /// rows added by `--added-by` (`content::denylist::import_file`). One id
    /// a line; blank lines and lines starting with `#` are skipped, and so
    /// are lines that are no entity id or hash (pointers), counted with
    /// their line numbers. A listed id is left alone, so a re-run adds
    /// nothing. A malformed line imports nothing and names itself. Prints
    /// the counts as JSON; a running `serve` hides the new ids within about
    /// 65 seconds (its 60 s denylist reload, then up to 5 s of cached history
    /// answers). Not behind OPS_WRITES_ENABLED: an import is operator data.
    ImportDenylist {
        #[arg(
            long,
            env = "DATABASE_URL",
            hide_env_values = true,
            required_unless_present = "print_ids"
        )]
        database_url: Option<String>,
        /// The file; `-` reads standard input.
        #[arg(long)]
        file: String,
        /// The operator wallet recorded as each new row's `added_by` (the
        /// console records the signed-in wallet); the audit actor is
        /// `cli:<wallet>`.
        #[arg(long, value_parser = wallet, required_unless_present = "print_ids")]
        added_by: Option<String>,
        /// Print the ids the import would list, one a line, and write
        /// nothing.
        #[arg(long)]
        print_ids: bool,
    },
}

/// `--added-by`: a wallet address, lowercased.
fn wallet(value: &str) -> Result<String, String> {
    catalyst::auth::normalize_eth_address(value)
        .ok_or_else(|| "must be a wallet address (0x and 40 hex digits)".to_string())
}

#[derive(Subcommand)]
enum StorageCommand {
    /// The store-wide collector (`storage::gc`, the worlds `/admin/gc`
    /// without the HTTP switch): reports the aged files no domain names and
    /// deletes them only with `--delete`.
    Gc {
        #[arg(long)]
        delete: bool,
        /// Raise the age floor (one hour at least).
        #[arg(long, default_value_t = 3600)]
        min_age_secs: u64,
        #[arg(long)]
        allow_empty_reference_set: bool,
        #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
        database_url: String,
        #[arg(
            long,
            env = "STORAGE_ROOT_FOLDER",
            default_value = "/var/lib/catalyst/content"
        )]
        storage_root_folder: String,
        #[arg(long, env = "GLOBAL_SCENES_URN")]
        global_scenes_urn: Option<String>,
    },
}

#[derive(Subcommand)]
enum BvimpostersCommand {
    /// Import a renderer's imposter cache (`{level}/{x},{y}-spec.json` +
    /// `.boimp` + `-floor.boimp`) into the store as stored zips.
    Seed {
        realm_cache_dir: String,
        #[arg(
            long,
            env = "BVIMPOSTERS_STORE_ROOT",
            default_value = "/var/lib/catalyst/bvimposters"
        )]
        store_root: String,
    },
    /// Move every stored zip the list names (`{level}/{x},{y}.{crc}[.zip]`
    /// per line) out of the store; the list defaults to
    /// `BVIMPOSTERS_QUARANTINE_LIST`, else `<store root>/readthrough-quarantine.txt`.
    Quarantine {
        list: Option<String>,
        #[arg(
            long,
            env = "BVIMPOSTERS_STORE_ROOT",
            default_value = "/var/lib/catalyst/bvimposters"
        )]
        store_root: String,
        #[arg(long, env = "BVIMPOSTERS_QUARANTINE_LIST")]
        default_list: Option<String>,
    },
}

#[derive(Subcommand)]
enum SchemaCommand {
    /// Apply the schema to an empty database; refuses if meta.schema exists.
    Apply {
        #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
        database_url: String,
    },
    /// Bring a database stamped by an earlier binary of the same schema
    /// identity and version up to this binary's layout, in one transaction:
    /// the additive steps of schemas/migrations/ it has not recorded in
    /// meta.schema_steps (a column an existing table gained, a constraint, a
    /// backfill), then every object of schema.sql it lacks (tables, indexes,
    /// types, functions, triggers, with the seeds and sequence ownership of
    /// the tables it creates). It never alters or drops anything a step does
    /// not spell out, refuses a step that would (drop, rename, retype,
    /// delete), a recorded step whose text changed, another identity or
    /// version, and a column still missing after the steps; a refusal
    /// applies nothing. The administrative role runs it; the schema oneshot
    /// does on every activation.
    Migrate {
        #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
        database_url: String,
        /// On a database one schema version behind, first run the embedded
        /// release (schemas/releases/v3-private-voice-pending.sql) in the
        /// same transaction. Refused while any other session is connected
        /// to the database: stop every writer first. No effect on a database
        /// already at this binary's version.
        #[arg(long)]
        release: bool,
    },
    /// Compare a database with this binary's layout without changing it:
    /// the meta.schema stamp against this binary's identity and version, the
    /// steps of schemas/migrations/ pending or drifted, the objects and
    /// columns missing. Exit 0 when serve can run on it as it is, 3 when
    /// `schema migrate` brings it up to date, 4 when it cannot (no stamp,
    /// another identity or version, a drifted or refused step, a missing
    /// column no step adds), 5 when it is one version behind and
    /// `schema migrate --release` takes it up once every writer is stopped.
    Status {
        #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
        database_url: String,
        /// Write the exit code to this file and exit 0 instead, for a
        /// container without a shell whose next step reads the verdict.
        #[arg(long)]
        exit_code_file: Option<PathBuf>,
    },
    /// Print the schema this binary applies (schema/*.sql rendered in name
    /// order), for `psql -f` on a lab database or a diff against another
    /// build. `--migrations` prints the steps of schemas/migrations/ instead.
    Print {
        #[arg(long)]
        migrations: bool,
    },
    /// Print what this binary migrates a database to, one line each: the
    /// identity and version it stamps, the SHA-256 of its schema.sql and
    /// each step of schemas/migrations/ with its SHA-256. Two builds that
    /// print the same layout migrate a database alike; local.sh compares the
    /// dev build's with the commit build's before the dev build migrates the
    /// database the two share.
    Layout,
    /// Print this binary's content and admission functions as idempotent
    /// `CREATE OR REPLACE FUNCTION` statements, for an administrative role
    /// to apply to a database created before them:
    /// `catalyst schema functions | psql "$ADMIN_DSN"`.
    Functions,
}

/// `catalyst schema layout`.
fn schema_layout() -> String {
    catalyst::schema_migrate::layout(
        db::SCHEMA_IDENTITY,
        db::SCHEMA_VERSION,
        db::SCHEMA_SQL,
        db::MIGRATIONS_SQL,
    )
}

fn main() -> anyhow::Result<()> {
    // The posture writes the environment clap reads, so it expands first,
    // while this is the only thread.
    posture::apply_to_process_env()?;
    // The complete clap tree exceeds the default Windows debug stack. This
    // parser-only thread returns plain values and finishes before V8 or runtime
    // initialization.
    let parsed = std::thread::Builder::new()
        .name("cli-parser".into())
        .stack_size(64 << 20)
        .spawn(|| -> Result<(Cli, clap::ArgMatches), clap::Error> {
            let matches = <Cli as clap::CommandFactory>::command().try_get_matches()?;
            let cli = <Cli as clap::FromArgMatches>::from_arg_matches(&matches)?;
            Ok((cli, matches))
        })
        .context("creating CLI parser thread")?
        .join()
        .map_err(|_| anyhow::anyhow!("CLI parser thread panicked"))?;
    let (mut cli, matches) = parsed.unwrap_or_else(|error| error.exit());
    match &mut cli.command {
        Command::Serve(config) => {
            if let Some(arguments) = matches.subcommand_matches("serve") {
                config.refuse_argv_secrets(arguments)?;
            }
            serve_preflight(config, &|key: &str| std::env::var(key).ok())?;
            catalyst::worlds::scene_state::jsruntime::ensure_v8_initialized();
        }
        Command::Index(config) => {
            if let Some(arguments) = matches.subcommand_matches("index") {
                config.refuse_argv_secrets(arguments)?;
            }
            check_posture(config)?;
        }
        _ => {}
    }
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("building the tokio runtime")?
        .block_on(run(cli))
}

/// The refusals of [`posture::check`] against this process's environment.
fn check_posture(config: &Config) -> anyhow::Result<()> {
    posture::check(config, &|key: &str| std::env::var(key).ok())
}

/// `serve`'s config refusals: the shared posture checks, then the DCL NAME
/// reader that world writes need (`index` takes no world writes).
fn serve_preflight(config: &Config, env: &posture::Lookup) -> anyhow::Result<()> {
    posture::check(config, env)?;
    posture::worlds_names_reader(config)?;
    Ok(())
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Serve(config) => catalyst::serve::serve(*config).await,
        Command::Index(config) => catalyst::index::run(*config).await,
        Command::Schema {
            command: SchemaCommand::Apply { database_url },
        } => {
            tracing_subscriber::fmt().with_target(false).init();
            let pool = db::connect_pool(
                &database_url,
                &db::PoolSettings {
                    max_connections: 1,
                    ..Default::default()
                },
            )
            .await
            .context("connecting to DATABASE_URL")?;
            db::apply_schema(&pool).await?;
            Ok(())
        }
        Command::Schema {
            command:
                SchemaCommand::Migrate {
                    database_url,
                    release,
                },
        } => {
            tracing_subscriber::fmt().with_target(false).init();
            let pool = db::connect_pool(
                &database_url,
                &db::PoolSettings {
                    max_connections: 1,
                    ..Default::default()
                },
            )
            .await
            .context("connecting to DATABASE_URL")?;
            let done = if release {
                catalyst::schema_migrate::migrate_with_release(
                    &pool,
                    db::SCHEMA_SQL,
                    db::MIGRATIONS_SQL,
                    db::RELEASE_SQL,
                )
                .await?
            } else {
                catalyst::schema_migrate::migrate(&pool, db::SCHEMA_SQL, db::MIGRATIONS_SQL).await?
            };
            if let Some(version) = done.released {
                println!("schema migrate: released the database to version {version}");
            }
            if done.is_empty() {
                println!("schema migrate: the database has every object of this binary's schema");
            } else {
                if !done.steps.is_empty() {
                    println!("schema migrate: applied {} step(s):", done.steps.len());
                    for s in &done.steps {
                        println!("  {s}");
                    }
                }
                if !done.created.is_empty() {
                    println!("schema migrate: created {} object(s):", done.created.len());
                    for o in &done.created {
                        println!("  {o}");
                    }
                }
            }
            Ok(())
        }
        Command::Schema {
            command:
                SchemaCommand::Status {
                    database_url,
                    exit_code_file,
                },
        } => {
            let pool = db::connect_pool(
                &database_url,
                &db::PoolSettings {
                    max_connections: 1,
                    ..Default::default()
                },
            )
            .await
            .context("connecting to DATABASE_URL")?;
            let status =
                catalyst::schema_migrate::status(&pool, db::SCHEMA_SQL, db::MIGRATIONS_SQL).await?;
            for line in status.lines() {
                println!("schema status: {line}");
            }
            if let Some(path) = exit_code_file {
                std::fs::write(&path, format!("{}\n", status.exit_code()))
                    .with_context(|| format!("writing {}", path.display()))?;
                return Ok(());
            }
            std::process::exit(status.exit_code());
        }
        Command::Schema {
            command: SchemaCommand::Print { migrations },
        } => {
            print!(
                "{}",
                if migrations {
                    db::MIGRATIONS_SQL
                } else {
                    db::SCHEMA_SQL
                }
            );
            Ok(())
        }
        Command::Schema {
            command: SchemaCommand::Layout,
        } => {
            print!("{}", schema_layout());
            Ok(())
        }
        Command::Schema {
            command: SchemaCommand::Functions,
        } => {
            print!("{}", content::land_publish::content_functions_sql(true));
            print!(
                "{}",
                comms::gatekeeper::admission::functions::social_functions_sql(true)
            );
            Ok(())
        }
        Command::Builder {
            command:
                BuilderCommand::Catalog {
                    packs_dir,
                    database_url,
                    storage_root_folder,
                },
        } => {
            tracing_subscriber::fmt().with_target(false).init();
            let pool = db::connect_pool(
                &database_url,
                &db::PoolSettings {
                    max_connections: 2,
                    ..Default::default()
                },
            )
            .await
            .context("connecting to DATABASE_URL")?;
            db::bootstrap(&pool).await?;
            let storage = storage::Storage::from_config(Path::new(&storage_root_folder))
                .await
                .with_context(|| format!("opening STORAGE_ROOT_FOLDER {storage_root_folder}"))?;
            if Path::new(&packs_dir).is_file() {
                builder::catalog::import(&pool, &packs_dir).await
            } else {
                builder::catalog::build(&pool, &storage, &packs_dir).await
            }
        }
        Command::Pulse => catalyst::pulse_generation::run_from_env().await,
        Command::Explore {
            command: ExploreCommand::ImportLikes { database_url, file },
        } => {
            use std::io::BufRead;
            tracing_subscriber::fmt().with_target(false).init();
            let pool = db::connect_pool(
                &database_url,
                &db::PoolSettings {
                    max_connections: 2,
                    ..Default::default()
                },
            )
            .await
            .context("connecting to DATABASE_URL")?;
            db::bootstrap(&pool).await?;
            let reader: Box<dyn BufRead> = if file == "-" {
                Box::new(std::io::BufReader::new(std::io::stdin()))
            } else {
                Box::new(std::io::BufReader::new(
                    std::fs::File::open(&file).with_context(|| format!("opening {file}"))?,
                ))
            };
            let out =
                explore::places::interactions::import_upstream_likes(&pool, reader.lines()).await?;
            println!("{}", serde_json::to_string(&out)?);
            Ok(())
        }
        Command::Content {
            command:
                ContentCommand::ImportDenylist {
                    database_url,
                    file,
                    added_by,
                    print_ids,
                },
        } => {
            let bytes = if file == "-" {
                content::denylist::read_file(std::io::stdin().lock())?
            } else {
                let opened =
                    std::fs::File::open(&file).with_context(|| format!("reading {file}"))?;
                content::denylist::read_file(opened).with_context(|| format!("reading {file}"))?
            };
            if print_ids {
                for id in content::denylist::checked_file(&bytes)?.ids {
                    println!("{id}");
                }
                return Ok(());
            }
            let (Some(database_url), Some(added_by)) = (database_url, added_by) else {
                anyhow::bail!("--database-url and --added-by are required");
            };
            tracing_subscriber::fmt().with_target(false).init();
            let pool = db::connect_pool(
                &database_url,
                &db::PoolSettings {
                    max_connections: 1,
                    ..Default::default()
                },
            )
            .await
            .context("connecting to DATABASE_URL")?;
            db::bootstrap(&pool).await?;
            let out = content::denylist::import_file(&pool, &bytes, &added_by).await?;
            println!("{}", serde_json::to_string(&out)?);
            Ok(())
        }
        Command::Shop {
            command:
                ShopCommand::NoticeRewind {
                    database_url,
                    producer,
                    from_block,
                    since,
                    dry_run,
                    marketplace_index_chain_id_ethereum,
                    marketplace_index_chain_id_matic,
                    actor,
                },
        } => {
            use catalyst::shop::market::notice_cursor::{RewindFrom, rewind};
            tracing_subscriber::fmt().with_target(false).init();
            let pool = db::connect_pool(
                &database_url,
                &db::PoolSettings {
                    max_connections: 1,
                    // A date far back walks every log since it.
                    statement_timeout_ms: 900_000,
                    ..Default::default()
                },
            )
            .await
            .context("connecting to DATABASE_URL")?;
            db::bootstrap(&pool).await?;
            let from = match (from_block, since) {
                (Some(b), _) => RewindFrom::Block(b),
                (None, Some(t)) => RewindFrom::Time(t),
                (None, None) => anyhow::bail!("give --from-block or --since"),
            };
            let actor = actor
                .as_deref()
                .map(str::trim)
                .filter(|a| !a.is_empty())
                .unwrap_or("cli");
            let trade_chains = (
                marketplace_index_chain_id_ethereum,
                marketplace_index_chain_id_matic,
            );
            let out = rewind(&pool, &producer, from, trade_chains, actor, dry_run).await?;
            println!("{}", serde_json::to_string(&out)?);
            Ok(())
        }
        Command::Shop {
            command:
                ShopCommand::CompareSources {
                    database_url,
                    net,
                    indexer,
                    from,
                    to,
                    window,
                    portal_url,
                    dataset,
                    max_rps,
                    assert_full_history,
                },
        } => {
            use catalyst::shop::market::chain_walk::ChainNet;
            use catalyst::shop::market::compare_sources;
            tracing_subscriber::fmt()
                .with_target(false)
                .with_writer(std::io::stderr)
                .init();
            // The dataset as `serve` names it: the flag, else
            // CHAIN_LOGS_PORTAL_DATASET_<NET>, else the network's mainnet.
            let dataset = dataset.unwrap_or_else(|| {
                let (var, default) = match ChainNet::parse(&net) {
                    Some(ChainNet::Ethereum) => {
                        ("CHAIN_LOGS_PORTAL_DATASET_ETHEREUM", "ethereum-mainnet")
                    }
                    _ => ("CHAIN_LOGS_PORTAL_DATASET_MATIC", "polygon-mainnet"),
                };
                std::env::var(var)
                    .ok()
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| default.to_string())
            });
            // Every outcome prints a report: a setup that cannot run is
            // refused, never raised (an error exit would read as a difference).
            let setup = || -> anyhow::Result<(ChainNet, String, catalyst::chain_index::portal::PortalReader)> {
                let net = ChainNet::parse(&net).context("--net")?;
                anyhow::ensure!(
                    max_rps.is_finite() && max_rps > 0.0,
                    "--max-rps (CHAIN_LOGS_PORTAL_MAX_RPS) must be above 0"
                );
                let rpc_var = match net {
                    ChainNet::Ethereum => "ETH_RPC_URL",
                    ChainNet::Matic => "RPC_ENDPOINT_POLYGON",
                };
                let rpc_url = catalyst::config::Config::secret_from_env(rpc_var)?
                    .with_context(|| format!("set {rpc_var} (or {rpc_var}_FILE)"))?;
                let key = catalyst::config::Config::secret_from_env("CHAIN_LOGS_PORTAL_API_KEY")?
                    .and_then(catalyst::chain_index::portal::Secret::new);
                let reader = catalyst::chain_index::portal::PortalReader::new(
                    &portal_url,
                    &dataset,
                    key,
                    max_rps,
                )
                .map_err(anyhow::Error::msg)?;
                Ok((net, rpc_url, reader))
            };
            let report = match setup() {
                Err(e) => compare_sources::Report::refused(
                    &net,
                    &indexer,
                    &dataset,
                    from,
                    to,
                    format!("{e:#}"),
                ),
                Ok((net_v, rpc_url, reader)) => match catalyst::http::Outbound::new(
                    catalyst::http::HttpClientCfg::default(),
                ) {
                    Err(e) => compare_sources::Report::refused(
                        &net,
                        &indexer,
                        &dataset,
                        from,
                        to,
                        format!("the outbound client: {}", e.without_url()),
                    ),
                    Ok(http) => compare_sources::command(
                        &database_url,
                        &http.allowing_configured_provider_hosts(),
                        compare_sources::Request {
                            net: net_v,
                            indexer: indexer.clone(),
                            from,
                            to,
                            window,
                            rpc_url,
                            reader,
                            history: if assert_full_history {
                                catalyst::chain_index::portal::AddressHistory::AssertedByOperator
                            } else {
                                catalyst::chain_index::portal::AddressHistory::Unknown
                            },
                        },
                    )
                    .await,
                },
            };
            println!("{}", serde_json::to_string(&report)?);
            match report.exit_code() {
                0 => Ok(()),
                code => std::process::exit(code),
            }
        }
        Command::Communities {
            command:
                CommunitiesCommand::Mirror {
                    database_url,
                    upstream_url,
                    thumbnail_url,
                    grace_secs,
                },
        } => {
            use catalyst::social::communities::mirror;
            tracing_subscriber::fmt().with_target(false).init();
            let cfg = mirror::MirrorConfig {
                source: explore::MirrorSource::checked(&upstream_url, &[])
                    .context("COMMUNITIES_UPSTREAM_URL")?,
                thumbnail_base: explore::MirrorSource::checked(&thumbnail_url, &[])
                    .context("COMMUNITIES_MIRROR_THUMBNAIL_URL")?,
                interval: std::time::Duration::from_secs(grace_secs),
            };
            let pool = db::connect_pool(
                &database_url,
                &db::PoolSettings {
                    max_connections: 2,
                    ..Default::default()
                },
            )
            .await
            .context("connecting to DATABASE_URL")?;
            db::bootstrap(&pool).await?;
            let client = http::Outbound::new(http::HttpClientCfg::default())
                .context("building the HTTP client")?;
            let cancel = tokio_util::sync::CancellationToken::new();
            let on_signal = cancel.clone();
            tokio::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    on_signal.cancel();
                }
            });
            let out = mirror::run_pass(
                &client,
                &pool,
                &cfg,
                std::time::Duration::from_secs(grace_secs),
                &cancel,
            )
            .await?;
            println!("{}", serde_json::to_string(&out)?);
            if !out.complete {
                anyhow::bail!("communities mirror pass incomplete (the next run resumes it)");
            }
            Ok(())
        }
        Command::CameraReel {
            command:
                CameraReelCommand::Import {
                    database_url,
                    source_database_url,
                    bucket_url,
                    storage_root_folder,
                    batch,
                    concurrency,
                    dry_run,
                    prune,
                },
        } => {
            use catalyst::camera_reel::import;
            tracing_subscriber::fmt().with_target(false).init();
            // Neither URL is echoed: either may carry credentials.
            let bucket_url = url::Url::parse(&bucket_url)
                .ok()
                .filter(|u| matches!(u.scheme(), "http" | "https") && !u.cannot_be_a_base())
                .context("CAMERA_REEL_SOURCE_BUCKET_URL is not an http(s) URL")?;
            let concurrency = usize::from(concurrency);
            let target = db::connect_pool(
                &database_url,
                &db::PoolSettings {
                    max_connections: concurrency.min(16) as u32 + 2,
                    ..Default::default()
                },
            )
            .await
            .context("connecting to DATABASE_URL")?;
            db::bootstrap(&target).await?;
            let source = db::connect_pool(
                &source_database_url,
                &db::PoolSettings {
                    max_connections: 1,
                    ..Default::default()
                },
            )
            .await
            .context("connecting to CAMERA_REEL_SOURCE_DATABASE_URL")?;
            let storage = storage::Storage::from_config(Path::new(&storage_root_folder))
                .await
                .with_context(|| format!("opening STORAGE_ROOT_FOLDER {storage_root_folder}"))?;
            let client = http::Outbound::new(
                http::HttpClientCfg::default().with_total_timeout(import::FETCH_TIMEOUT),
            )
            .context("building the HTTP client")?;
            let options = import::ImportOptions {
                bucket_url,
                batch,
                concurrency,
                dry_run,
                prune,
            };
            let report = import::run(&source, &target, &storage, &client, &options).await?;
            println!("{}", serde_json::to_string(&report)?);
            Ok(())
        }
        Command::Posture {
            command: PostureCommand::Show { name },
        } => {
            print!("{}", posture::show(posture::Posture::parse(&name)?));
            Ok(())
        }
        Command::Posture {
            command: PostureCommand::Env { name, keys },
        } => {
            print!(
                "{}",
                posture::env_file(posture::Posture::parse(&name)?, &keys)?
            );
            Ok(())
        }
        Command::Secrets {
            command:
                SecretsCommand::Init {
                    serve_file,
                    pulse_file,
                    with_abgen,
                },
        } => {
            let outcome =
                ops::generated_secrets::init(&serve_file, pulse_file.as_deref(), with_abgen)?;
            if outcome.generated.is_empty() {
                println!(
                    "secrets init: every slot of {} present; nothing written",
                    serve_file.display()
                );
            } else {
                println!(
                    "secrets init: minted {} in {}",
                    outcome.generated.join(", "),
                    serve_file.display()
                );
            }
            if outcome.pulse_written {
                println!(
                    "secrets init: wrote {}",
                    pulse_file
                        .as_deref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default()
                );
            }
            Ok(())
        }
        Command::Content {
            command:
                ContentCommand::RepairProfileSnapshots {
                    database_url,
                    storage_root_folder,
                    sync_source,
                    batch,
                },
        } => {
            tracing_subscriber::fmt().with_target(false).init();
            let servers = content::sync::parse_sync_sources(&sync_source, &[])
                .context("reading SYNC_SOURCE")?;
            let client = content::sync::sources_http_client(&servers)?;
            let pool = db::connect_pool(
                &database_url,
                &db::PoolSettings {
                    max_connections: 2,
                    ..Default::default()
                },
            )
            .await
            .context("connecting to DATABASE_URL")?;
            let storage = storage::Storage::from_config(Path::new(&storage_root_folder))
                .await
                .with_context(|| format!("opening STORAGE_ROOT_FOLDER {storage_root_folder}"))?;
            let report =
                content::profile_snapshots::repair(&pool, &storage, &client, &servers, batch)
                    .await?;
            println!("{}", serde_json::to_string(&report)?);
            Ok(())
        }
        Command::Storage {
            command:
                StorageCommand::Gc {
                    delete,
                    min_age_secs,
                    allow_empty_reference_set,
                    database_url,
                    storage_root_folder,
                    global_scenes_urn,
                },
        } => {
            tracing_subscriber::fmt().with_target(false).init();
            let pool = db::connect_pool(
                &database_url,
                &db::PoolSettings {
                    max_connections: 2,
                    ..Default::default()
                },
            )
            .await
            .context("connecting to DATABASE_URL")?;
            let storage = storage::Storage::from_config(Path::new(&storage_root_folder))
                .await
                .with_context(|| format!("opening STORAGE_ROOT_FOLDER {storage_root_folder}"))?;
            let globals: Vec<String> = global_scenes_urn
                .map(|r| r.split_whitespace().map(str::to_string).collect())
                .unwrap_or_default();
            let options = storage::gc::GcOptions {
                dry_run: !delete,
                min_age: std::time::Duration::from_secs(min_age_secs)
                    .max(storage::gc::MIN_ORPHAN_AGE),
                allow_empty_reference_set,
            };
            let report = storage::gc::collect(&pool, &storage, &globals, options).await?;
            println!("{}", serde_json::to_string(&report)?);
            Ok(())
        }
        Command::Bvimposters {
            command:
                BvimpostersCommand::Seed {
                    realm_cache_dir,
                    store_root,
                },
        } => {
            tracing_subscriber::fmt().with_target(false).init();
            let store =
                explore::filecache::FileCache::new(Path::new(&store_root).to_path_buf(), u64::MAX);
            store
                .init()
                .with_context(|| format!("opening the imposter store at {store_root}"))?;
            let counts = explore::imposters::seed::seed_into(&store, Path::new(&realm_cache_dir))?;
            println!(
                "imported {} skipped {} crc0 {} incomplete {}",
                counts.imported, counts.skipped, counts.crc0, counts.incomplete
            );
            Ok(())
        }
        Command::Bvimposters {
            command:
                BvimpostersCommand::Quarantine {
                    list,
                    store_root,
                    default_list,
                },
        } => {
            tracing_subscriber::fmt().with_target(false).init();
            let store =
                explore::filecache::FileCache::new(Path::new(&store_root).to_path_buf(), u64::MAX);
            store
                .init()
                .with_context(|| format!("opening the imposter store at {store_root}"))?;
            let path = list
                .or(default_list)
                .filter(|p| !p.trim().is_empty())
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| Path::new(&store_root).join("readthrough-quarantine.txt"));
            let quarantine =
                explore::imposters::quarantine_list::QuarantineList::load(path.clone());
            let counts = explore::imposters::quarantine_list::apply(&store, &quarantine);
            println!(
                "list {} keys {} renamed {} absent {} errors {}",
                path.display(),
                quarantine.len(),
                counts.renamed,
                counts.absent,
                counts.errors
            );
            Ok(())
        }
        Command::Builder {
            command:
                BuilderCommand::Seed {
                    base_url,
                    hashes,
                    storage_root_folder,
                },
        } => {
            tracing_subscriber::fmt().with_target(false).init();
            let storage = storage::Storage::from_config(Path::new(&storage_root_folder))
                .await
                .with_context(|| format!("opening STORAGE_ROOT_FOLDER {storage_root_folder}"))?;
            let http = http::Outbound::new(http::HttpClientCfg::default())
                .context("building the HTTP client")?;
            let stored = builder::catalog::seed(&storage, &http, &base_url, &hashes).await?;
            println!("seed: {stored} of {} files stored", hashes.len());
            Ok(())
        }
    }
}
