//! Bake-on-miss: a tile the store misses is queued for the engine's `impost`
//! renderer (`engine/src/bin/impost.rs`, shipped in the engine's
//! `batch-tools` package with its `dcl_deno_ipc` sidecar), run under an
//! optional display wrapper with a hard timeout that kills its whole process
//! group. It loads the scenes from this node's own realm and content, and
//! each run gets private XDG data/cache/config dirs inside its staging
//! directory, so its session log and caches go with it.
//! The renderer writes zips for every level up to the tile's under a
//! staging directory; each one is verified against its key (the crc in the
//! name must be the spec's) and landed in the store, then the store is
//! evicted back under its budget.
//!
//! The queue lives beside the store, so every process that serves the store
//! feeds it: a miss leaves a marker `bake/wanted/<level>,<x>,<y>` (at most
//! `queue_depth` of them, a tile once), and the one process holding the
//! `imposters-bake` lease bakes them oldest first, one at a time, holding
//! the tile under `bake/inflight/` meanwhile. An API replica stands the
//! worker down, so its misses reach the baker with no routing rule. A tile
//! still held when a baker starts died with its baker and counts as a failed
//! bake; one held when the lease ends goes back to the queue. Failures feed
//! the bake quarantine (`quarantine`). A bake starts only once the realm
//! answers `/about` with 200 (a bootstrapping node answers 503, and the
//! renderer then never loads a scene); until then the tile is dropped
//! without counting against it, and the next miss asks again. The
//! renderer's output goes to `impost.log` in the staging directory, and a
//! failed bake carries its last lines.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result, anyhow};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::filecache::FileCache;
use crate::imposters::key::{TileKey, parse_zip_request};
use crate::imposters::quarantine::Quarantine;
use crate::imposters::zips;

/// How often the baker looks for markers other processes left.
const POLL: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
pub struct BakeConfig {
    /// `BVIMPOSTERS_BAKE_WRAPPER`: a command prefix giving the renderer its
    /// display (e.g. `xvfb-run -a`); empty runs it directly.
    pub wrapper: String,
    /// `BVIMPOSTERS_IMPOST_BIN`.
    pub bin: String,
    /// `BVIMPOSTERS_IMPOST_SERVER`: the realm the renderer loads.
    pub server: String,
    /// `BVIMPOSTERS_IMPOST_CONTENT_SERVER`; empty omits `--content-server`.
    pub content_server: String,
    /// `BVIMPOSTERS_IMPOST_SDK6_ADAPTION_LAYER`; empty omits
    /// `--sdk6-adaption-layer`, and an SDK6 scene then bakes empty.
    pub sdk6_adaption_layer: String,
    pub timeout: Duration,
    pub queue_depth: usize,
}

pub struct BakeQueue {
    cfg: BakeConfig,
    http: reqwest::Client,
    store: Arc<FileCache>,
    quarantine: Arc<Quarantine>,
    wanted: PathBuf,
    inflight: PathBuf,
    notify: Notify,
}

fn marker_name(tile: &TileKey) -> String {
    format!("{},{},{}", tile.level, tile.x, tile.y)
}

fn parse_marker(name: &str) -> Option<TileKey> {
    let mut parts = name.split(',');
    let level = parts.next()?.parse().ok()?;
    let x = parts.next()?.parse().ok()?;
    let y = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    TileKey::new(level, x, y)
}

/// The markers under `dir`, oldest first; a name that is no tile is dropped.
fn markers(dir: &Path) -> Vec<(TileKey, PathBuf)> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<(SystemTime, TileKey, PathBuf)> = Vec::new();
    for entry in rd.flatten() {
        let path = entry.path();
        let Some(tile) = entry.file_name().to_str().and_then(parse_marker) else {
            let _ = std::fs::remove_file(&path);
            continue;
        };
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        found.push((modified, tile, path));
    }
    found.sort();
    found
        .into_iter()
        .map(|(_, tile, path)| (tile, path))
        .collect()
}

impl BakeQueue {
    pub fn new(cfg: BakeConfig, store: Arc<FileCache>, quarantine: Arc<Quarantine>) -> Arc<Self> {
        let wanted = store.root().join("bake/wanted");
        let inflight = store.root().join("bake/inflight");
        for dir in [&wanted, &inflight] {
            if let Err(e) = std::fs::create_dir_all(dir) {
                tracing::warn!(dir = %dir.display(), error = %e, "creating the bake queue");
            }
        }
        Arc::new(Self {
            cfg,
            http: realm_client(),
            store,
            quarantine,
            wanted,
            inflight,
            notify: Notify::new(),
        })
    }

    /// The worker, a fleet-wide job: it bakes in the process holding the
    /// `imposters-bake` lease and stands down in an API replica.
    pub fn spawn_worker(self: &Arc<Self>, jobs: &crate::jobs::Jobs) {
        let this = self.clone();
        jobs.spawn_leased("imposters-bake", move |ctx| {
            let this = this.clone();
            async move {
                this.worker(ctx.cancel).await;
                Ok(())
            }
        });
    }

    pub fn enqueue(&self, tile: TileKey) -> bool {
        let name = marker_name(&tile);
        if self.inflight.join(&name).exists() {
            return false;
        }
        if markers(&self.wanted).len() >= self.cfg.queue_depth {
            return false;
        }
        let created = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.wanted.join(&name));
        if created.is_err() {
            return false;
        }
        self.notify.notify_one();
        true
    }

    pub fn snapshot(&self) -> (Vec<TileKey>, Option<TileKey>) {
        let queue = markers(&self.wanted).into_iter().map(|(t, _)| t).collect();
        let inflight = markers(&self.inflight).into_iter().map(|(t, _)| t).next();
        (queue, inflight)
    }

    async fn worker(self: Arc<Self>, cancel: CancellationToken) {
        for (tile, held) in markers(&self.inflight) {
            tracing::warn!(tile = %tile.label(), "bake failed, its baker died during it");
            self.quarantine.record_failure(&tile);
            let _ = std::fs::remove_file(held);
        }
        loop {
            let Some((tile, wanted)) = markers(&self.wanted).into_iter().next() else {
                tokio::select! {
                    _ = self.notify.notified() => {}
                    _ = tokio::time::sleep(POLL) => {}
                    _ = cancel.cancelled() => return,
                }
                continue;
            };
            let held = self.inflight.join(marker_name(&tile));
            if let Err(e) = std::fs::rename(&wanted, &held) {
                tracing::warn!(tile = %tile.label(), error = %e, "bake marker unmovable, dropped");
                let _ = std::fs::remove_file(&wanted);
                continue;
            }
            if self.quarantine.is_quarantined(&tile) {
                tracing::info!(tile = %tile.label(), "bake suppressed, tile quarantined");
            } else if let Err(reason) = realm_ready(&self.http, &self.cfg.server).await {
                tracing::info!(tile = %tile.label(), %reason, "bake deferred, realm not ready");
            } else {
                let outcome = tokio::select! {
                    result = run_bake(&self.cfg, self.store.clone(), tile) => result,
                    _ = cancel.cancelled() => {
                        let _ = std::fs::rename(&held, &wanted);
                        return;
                    }
                };
                match outcome {
                    Ok(count) => {
                        tracing::info!(tile = %tile.label(), count, "bake harvested");
                        self.quarantine.record_success(&tile);
                    }
                    Err(e) => {
                        tracing::warn!(tile = %tile.label(), error = %format!("{e:#}"), "bake failed");
                        self.quarantine.record_failure(&tile);
                    }
                }
            }
            let _ = std::fs::remove_file(&held);
        }
    }
}

/// The client that asks the realm whether it is ready: built as every
/// upstream client is (`dcl_http::HttpClientCfg`), so a reserved name
/// such as `realm.localhost` never reaches the system resolver.
fn realm_client() -> reqwest::Client {
    dcl_http::http_client(
        "imposters-realm",
        &dcl_http::HttpClientCfg::default()
            .with_total_timeout(Duration::from_secs(10))
            .following_redirects(10),
    )
}

/// The renderer gives up on a realm whose `/about` is not a 200.
async fn realm_ready(http: &reqwest::Client, server: &str) -> Result<(), String> {
    let url = format!("{}/about", server.trim_end_matches('/'));
    match http.get(&url).send().await {
        Ok(resp) if resp.status() == reqwest::StatusCode::OK => Ok(()),
        Ok(resp) => Err(format!("{url} answered {}", resp.status())),
        Err(e) => Err(format!("{url}: {e}")),
    }
}

pub fn bake_args(tile: TileKey, cfg: &BakeConfig, staging: &Path) -> Vec<String> {
    let extent = 1i32 << tile.level;
    let half = extent / 2;
    let mut args = vec!["--server".to_string(), cfg.server.clone()];
    if !cfg.content_server.is_empty() {
        args.push("--content-server".to_string());
        args.push(cfg.content_server.clone());
    }
    if !cfg.sdk6_adaption_layer.is_empty() {
        args.push("--sdk6-adaption-layer".to_string());
        args.push(cfg.sdk6_adaption_layer.clone());
    }
    // the renderer resolves no tile without an imposter store to key its cache by; with
    // --no-download it never fetches from it, so this node's own store names it
    args.extend([
        "--imposter-url-base".to_string(),
        format!("{}/bvimposters", cfg.server.trim_end_matches('/')),
        "--location".to_string(),
        format!("{},{}", tile.x + half, tile.y + half),
        "--range".to_string(),
        half.max(1).to_string(),
        "--levels".to_string(),
        tile.level.to_string(),
        "--threads".to_string(),
        "4".to_string(),
        "--no-download".to_string(),
        "--zip-output".to_string(),
        staging.display().to_string(),
    ]);
    args
}

pub async fn run_bake(cfg: &BakeConfig, store: Arc<FileCache>, tile: TileKey) -> Result<usize> {
    let staging = store
        .tmp_dir()
        .join(format!("bake-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&staging)
        .await
        .with_context(|| format!("creating {}", staging.display()))?;

    let mut argv: Vec<String> = cfg.wrapper.split_whitespace().map(String::from).collect();
    argv.push(cfg.bin.clone());
    argv.extend(bake_args(tile, cfg, &staging));
    let program = argv.remove(0);

    tracing::info!(tile = %tile.label(), %program, args = ?argv, "bake starting");
    let log_path = staging.join("impost.log");
    let log = std::fs::File::create(&log_path)
        .with_context(|| format!("creating {}", log_path.display()))?;
    let log_err = log.try_clone().context("cloning the bake log handle")?;
    let mut cmd = tokio::process::Command::new(&program);
    cmd.args(&argv)
        .env("XDG_DATA_HOME", staging.join("xdg/data"))
        .env("XDG_CACHE_HOME", staging.join("xdg/cache"))
        .env("XDG_CONFIG_HOME", staging.join("xdg/config"))
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err));
    #[cfg(unix)]
    cmd.process_group(0);
    cmd.kill_on_drop(true);
    let spawned = cmd.spawn();
    // The command holds the log handles until it drops; close them here as before.
    drop(cmd);
    let mut child = match spawned {
        Ok(child) => child,
        Err(e) => {
            let _ = tokio::fs::remove_dir_all(&staging).await;
            return Err(e).with_context(|| format!("spawning {program}"));
        }
    };
    #[cfg(unix)]
    let pid = child.id();

    let status = match tokio::time::timeout(cfg.timeout, child.wait()).await {
        Ok(waited) => waited.context("waiting for bake process")?,
        Err(_) => {
            #[cfg(unix)]
            if let Some(pid) = pid {
                // SAFETY: killpg on the process group the child leads
                // (`process_group(0)`); no memory is touched.
                unsafe {
                    libc::killpg(pid as i32, libc::SIGKILL);
                }
            }
            // No process groups here: kill the child itself.
            #[cfg(not(unix))]
            let _ = child.kill().await;
            let _ = child.wait().await;
            let tail = log_tail(&log_path).await;
            let _ = tokio::fs::remove_dir_all(&staging).await;
            return Err(anyhow!("bake timed out after {:?}{tail}", cfg.timeout));
        }
    };
    if !status.success() {
        let tail = log_tail(&log_path).await;
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(anyhow!("bake exited with {status}{tail}"));
    }

    let harvested = {
        let store = store.clone();
        let staging = staging.clone();
        tokio::task::spawn_blocking(move || harvest(&store, &staging, tile.level))
            .await
            .context("harvest task")??
    };
    let _ = tokio::fs::remove_dir_all(&staging).await;
    if harvested == 0 {
        return Err(anyhow!("bake produced no harvestable zips"));
    }
    let _ = tokio::task::spawn_blocking(move || store.evict_pass()).await;
    Ok(harvested)
}

/// Lands every verified zip under `staging/imposters/realms/*/{0..=max_level}/`.
/// The renderer's last lines, for the failure it caused.
async fn log_tail(path: &Path) -> String {
    let Ok(text) = tokio::fs::read_to_string(path).await else {
        return String::new();
    };
    let lines: Vec<&str> = text.lines().rev().take(3).collect();
    if lines.is_empty() {
        return String::new();
    }
    let lines: Vec<&str> = lines.into_iter().rev().collect();
    format!("; impost: {}", lines.join(" | "))
}

fn harvest(store: &FileCache, staging: &Path, max_level: u8) -> Result<usize> {
    let mut count = 0;
    let realms_root = staging.join("imposters").join("realms");
    let Ok(realms) = std::fs::read_dir(&realms_root) else {
        return Ok(0);
    };
    for realm in realms.flatten() {
        if !realm.path().is_dir() {
            continue;
        }
        for level in 0..=max_level {
            let Ok(rd) = std::fs::read_dir(realm.path().join(level.to_string())) else {
                continue;
            };
            for entry in rd.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let Some(key) = parse_zip_request(&level.to_string(), &name) else {
                    continue;
                };
                let Ok(bytes) = std::fs::read(entry.path()) else {
                    continue;
                };
                if let Err(e) = zips::verify_zip(&bytes, &key) {
                    tracing::warn!(file = %name, error = %e, "harvest zip failed verification");
                    continue;
                }
                let Some(target) = store.path(&key.rel()) else {
                    continue;
                };
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::rename(entry.path(), &target)?;
                count += 1;
            }
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filecache::tests::{TempDir, tempdir};
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Mutex;

    fn test_cfg(bin: &str) -> BakeConfig {
        BakeConfig {
            // Through sh, not exec: exec of the just-written script fails with
            // ETXTBSY while a sibling test's forked child still holds its write fd.
            wrapper: "/bin/sh".to_string(),
            bin: bin.to_string(),
            server: "https://catalyst.example".to_string(),
            content_server: "http://localhost:5141".to_string(),
            sdk6_adaption_layer: String::new(),
            timeout: Duration::from_secs(30),
            queue_depth: 1,
        }
    }

    #[cfg(unix)]
    fn write_script(dir: &Path, name: &str, body: &str) -> String {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.display().to_string()
    }

    fn store(dir: &TempDir) -> Arc<FileCache> {
        let store = Arc::new(FileCache::new(dir.0.join("root"), u64::MAX));
        store.init().unwrap();
        store
    }

    fn staging_entries(store: &FileCache) -> usize {
        std::fs::read_dir(store.tmp_dir())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("bake-"))
            .count()
    }

    #[test]
    fn args_for_level_zero() {
        let cfg = test_cfg("/bin/impost");
        let tile = TileKey::new(0, 7, -3).unwrap();
        let args = bake_args(tile, &cfg, Path::new("/s/j1"));
        assert_eq!(
            args,
            vec![
                "--server",
                "https://catalyst.example",
                "--content-server",
                "http://localhost:5141",
                "--imposter-url-base",
                "https://catalyst.example/bvimposters",
                "--location",
                "7,-3",
                "--range",
                "1",
                "--levels",
                "0",
                "--threads",
                "4",
                "--no-download",
                "--zip-output",
                "/s/j1"
            ]
        );
    }

    #[test]
    fn empty_content_server_is_omitted() {
        let mut cfg = test_cfg("/bin/impost");
        cfg.content_server = String::new();
        let tile = TileKey::new(0, 7, -3).unwrap();
        let args = bake_args(tile, &cfg, Path::new("/s/j1"));
        assert!(!args.contains(&"--content-server".to_string()));
        assert!(!args.contains(&"--sdk6-adaption-layer".to_string()));
        assert_eq!(args[2], "--imposter-url-base");
    }

    #[test]
    fn the_adaption_layer_is_passed_when_set() {
        let mut cfg = test_cfg("/bin/impost");
        cfg.sdk6_adaption_layer = "http://127.0.0.1:5235/layer.js".to_string();
        let args = bake_args(TileKey::new(0, 7, -3).unwrap(), &cfg, Path::new("/s/j1"));
        assert_eq!(
            args[4..6],
            ["--sdk6-adaption-layer", "http://127.0.0.1:5235/layer.js"]
        );
    }

    #[test]
    fn args_for_level_two() {
        let cfg = test_cfg("/bin/impost");
        let tile = TileKey::new(2, -64, -128).unwrap();
        let args = bake_args(tile, &cfg, Path::new("/s/j2"));
        assert_eq!(args[7], "-62,-126");
        assert_eq!(args[9], "2");
        assert_eq!(args[11], "2");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn fake_bake_harvests_all_levels() {
        let dir = tempdir("bake-harvest");
        let store = store(&dir);
        let fixtures = dir.0.join("fixtures");
        std::fs::create_dir_all(&fixtures).unwrap();
        std::fs::write(
            fixtures.join("2,4.777.zip"),
            zips::test_zip_bytes(2, 4, 777),
        )
        .unwrap();
        std::fs::write(
            fixtures.join("2,4.888.zip"),
            zips::test_zip_bytes(2, 4, 888),
        )
        .unwrap();
        std::fs::write(
            fixtures.join("3,5.999.zip"),
            zips::test_zip_bytes(3, 5, 999),
        )
        .unwrap();
        let body = format!(
            "out=\"\"\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"--zip-output\" ]; then out=\"$2\"; fi\n  shift\ndone\nmkdir -p \"$out/imposters/realms/enc/0\" \"$out/imposters/realms/enc/1\"\ncp '{f}/2,4.777.zip' \"$out/imposters/realms/enc/1/\"\ncp '{f}/2,4.888.zip' \"$out/imposters/realms/enc/0/\"\ncp '{f}/3,5.999.zip' \"$out/imposters/realms/enc/0/\"",
            f = fixtures.display()
        );
        let script = write_script(&dir.0, "fake-impost.sh", &body);
        let tile = TileKey::new(1, 2, 4).unwrap();
        let harvested = run_bake(&test_cfg(&script), store.clone(), tile)
            .await
            .unwrap();
        assert_eq!(harvested, 3);
        assert!(store.store_dir().join("1").join("2,4.777.zip").exists());
        assert!(store.store_dir().join("0").join("2,4.888.zip").exists());
        assert!(store.store_dir().join("0").join("3,5.999.zip").exists());
        assert_eq!(staging_entries(&store), 0);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failing_bin_is_an_error() {
        let dir = tempdir("bake-fail");
        let store = store(&dir);
        let script = write_script(&dir.0, "fail.sh", "echo no realm >&2; exit 1");
        let err = run_bake(
            &test_cfg(&script),
            store.clone(),
            TileKey::new(0, 0, 0).unwrap(),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().ends_with("; impost: no realm"), "{err}");
        assert_eq!(staging_entries(&store), 0);
    }

    #[tokio::test]
    async fn missing_bin_is_an_error_and_cleans_staging() {
        let dir = tempdir("bake-missing");
        let store = store(&dir);
        let cfg = test_cfg(&dir.0.join("no-such-bin").display().to_string());
        assert!(
            run_bake(&cfg, store.clone(), TileKey::new(0, 0, 0).unwrap())
                .await
                .is_err()
        );
        assert_eq!(staging_entries(&store), 0);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn timeout_kills_the_bake_and_cleans_staging() {
        let dir = tempdir("bake-timeout");
        let store = store(&dir);
        let script = write_script(&dir.0, "hang.sh", "exec sleep 60");
        let mut cfg = test_cfg(&script);
        cfg.timeout = Duration::from_millis(200);
        let err = run_bake(&cfg, store.clone(), TileKey::new(0, 0, 0).unwrap())
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("timed out"),
            "expected the timeout error, got: {err:#}"
        );
        assert_eq!(staging_entries(&store), 0);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn empty_harvest_is_an_error() {
        let dir = tempdir("bake-empty");
        let store = store(&dir);
        let script = write_script(&dir.0, "noop.sh", "exit 0");
        assert!(
            run_bake(
                &test_cfg(&script),
                store.clone(),
                TileKey::new(0, 0, 0).unwrap()
            )
            .await
            .is_err()
        );
        assert_eq!(staging_entries(&store), 0);
    }

    #[tokio::test]
    async fn queue_coalesces_and_bounds() {
        let dir = tempdir("bake-queue");
        let store = store(&dir);
        let quarantine = Arc::new(Quarantine::load(dir.0.join("quarantine.json"), 3, 86400));
        let queue = BakeQueue::new(test_cfg("/bin/true"), store, quarantine);
        let a = TileKey::new(0, 0, 0).unwrap();
        let b = TileKey::new(0, 1, 0).unwrap();
        assert!(queue.enqueue(a));
        assert!(!queue.enqueue(a));
        assert!(!queue.enqueue(b));
        let (pending, inflight) = queue.snapshot();
        assert_eq!(pending, vec![a]);
        assert_eq!(inflight, None);
    }

    #[tokio::test]
    async fn a_tile_its_baker_died_holding_counts_as_a_failure() {
        let dir = tempdir("bake-dead");
        let store = store(&dir);
        let quarantine = Arc::new(Quarantine::load(dir.0.join("quarantine.json"), 1, 86400));
        let queue = BakeQueue::new(test_cfg("/bin/true"), store, quarantine.clone());
        let tile = TileKey::new(1, 2, -4).unwrap();
        std::fs::write(queue.inflight.join(marker_name(&tile)), b"").unwrap();
        assert_eq!(queue.snapshot().1, Some(tile));
        let jobs = crate::jobs::Jobs::new();
        queue.spawn_worker(&jobs);
        assert!(
            crate::shared::test_wait::until(async || quarantine.is_quarantined(&tile)).await,
            "the dead baker's tile was never quarantined"
        );
        assert_eq!(queue.snapshot(), (Vec::new(), None));
    }

    #[test]
    fn markers_name_their_tile() {
        let tile = TileKey::new(2, -4, 8).unwrap();
        assert_eq!(parse_marker(&marker_name(&tile)), Some(tile));
        assert_eq!(parse_marker("2,-4"), None);
        assert_eq!(parse_marker("1,1,0"), None, "not level-aligned");
    }

    #[tokio::test]
    async fn a_realm_that_is_not_ready_is_not_baked_against() {
        let status = Arc::new(Mutex::new(axum::http::StatusCode::SERVICE_UNAVAILABLE));
        let answer = status.clone();
        let app = axum::Router::new().route(
            "/about",
            axum::routing::get(move || {
                let answer = answer.clone();
                async move { *answer.lock().unwrap() }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server = format!(
            "http://realm.localhost:{}",
            listener.local_addr().unwrap().port()
        );
        tokio::spawn(async move { axum::serve(listener, app).await });
        let http = realm_client();
        let reason = realm_ready(&http, &server).await.unwrap_err();
        assert!(reason.contains("503"), "{reason}");
        *status.lock().unwrap() = axum::http::StatusCode::OK;
        realm_ready(&http, &format!("{server}/")).await.unwrap();
    }

    /// The realm probe answers a reserved name itself, as every built client
    /// does: an island's `.invalid` realm never becomes a DNS query.
    #[tokio::test]
    async fn the_realm_probe_resolves_no_reserved_name_through_dns() {
        let err = realm_client()
            .get("http://realm.invalid/about")
            .send()
            .await
            .unwrap_err();
        let mut chain = Vec::new();
        let mut source: Option<&dyn std::error::Error> = Some(&err);
        while let Some(e) = source {
            chain.push(e.to_string());
            source = e.source();
        }
        assert!(chain.iter().any(|e| e.contains("RFC 6761")), "{chain:?}");
    }
}
