//! `dcl-scene-cli sync`: a two-way mirror of a project folder and a node's
//! builder project, by content hash. The node keeps the project's file set
//! (`path -> CIDv1`) at a version (`GET/PUT /builder-api/v1/projects/{id}/files`)
//! and the bytes by hash (`PUT .../blobs/{hash}`, read back from
//! `/builder-api/v1/storage/contents/{hash}`); `.dcl-one/sync.json` keeps the
//! set both sides agreed on at the last sync, the base of a three-way
//! comparison per path:
//!
//! - local and node agree: nothing to do;
//! - only the node changed it since the base: written (or deleted) here;
//! - only the folder changed it: uploaded (or dropped) there, only the
//!   bytes the node lacks travelling;
//! - both changed it, differently: a conflict, reported and left alone on
//!   both sides (the base keeps the old hash, so the next sync reports it
//!   again) unless `--prefer local|remote` picks a side. The rest of the
//!   sync still applies, and the command exits non-zero.
//!
//! A node change is written here only while the folder still holds what the
//! scan read at that path, checked again just before the write or removal,
//! never through or over a symlink (or a file where a directory goes), and
//! never under a name the folder lists only in another spelling (a
//! case-insensitive folder holds one of `Scene.ts` and `scene.ts`):
//! otherwise the folder's copy stays, the path is reported, its base keeps
//! the old hash, and the command exits non-zero. Every pulled content is
//! downloaded before the folder changes, each streamed into a temporary
//! under `.dcl-one/` and renamed into place from there, so a failed
//! download changes nothing here. A path the base holds that a symlink or
//! non-directory now hides from the scan is held the same way, never read
//! as deleted, and keeps its base (reported while the node still holds
//! it), so the copy that comes back meets whatever the node did meanwhile.
//! `.dcl-one/` and its state file are never written through a symlink
//! either.
//!
//! A sync that may change anything holds `.dcl-one/sync.lock` from before
//! it reads the state until it ends (the system releases it however the
//! process ends): a second sync of the folder meanwhile is refused, never
//! interleaved. Holding it, a sync first removes the temporaries a killed
//! one left in `.dcl-one/`.
//!
//! The set goes up with the version it was planned against; a node that
//! moved meanwhile answers 409 with its set, and the plan is made again
//! from it. A node missing contents the set names answers 400 with their
//! hashes, and only those are uploaded (bytes another project or path
//! already gave the node never travel). A remembered project that is gone
//! from the node, or was deleted and made again under its id (its
//! `created_at` moved), stops the sync before anything changes: against an
//! empty node set, the base would read as "deleted there" and empty the
//! folder. Every request is signed by `--sign-key` or `DCL_PRIVATE_KEY`
//! over its full path; the project is that wallet's. The folder is walked
//! as the composite walker walks it (dot directories, `node_modules`, `bin`
//! and `dist` pruned), without symlinks, `.env` files and the temporaries a
//! killed sync left; node paths the walk would never list are kept on the
//! node untouched. A name that is not UTF-8 is no node path: it is left out
//! on both sides and reported, and the command exits non-zero.

use crate::storage_remote::Signer;
use crate::ux::{self, TrySteps, UserError};
use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use dcl_auth::{HashV1Writer, hash_bytes_v1};
use dcl_scene_authoring::is_pruned_dir;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The node's per-content cap (`uploaded_contents_size_check`).
pub const MAX_FILE_BYTES: u64 = 20 * 1024 * 1024;
/// The node's per-project cap.
pub const MAX_FILES: usize = 5000;
const MAX_PATH_CHARS: usize = 512;
const STATE_FILE: &str = "sync.json";
const LOCK_FILE: &str = "sync.lock";
const PUT_ATTEMPTS: usize = 3;

#[derive(Args, Debug, Clone)]
pub struct SyncOptions {
    #[arg(long, default_value = ".", help = "Project folder")]
    pub dir: PathBuf,
    #[arg(long, help = "The node's URL (its origin); the last sync's otherwise")]
    pub node: Option<String>,
    #[arg(
        long,
        help = "The node project's id; the last sync's otherwise, or a new project"
    )]
    pub project: Option<String>,
    #[arg(
        long,
        help = "Private key file that signs the requests; DCL_PRIVATE_KEY otherwise"
    )]
    pub sign_key: Option<PathBuf>,
    #[arg(
        long,
        value_enum,
        help = "Resolve paths both sides changed with this side's copy instead of stopping"
    )]
    pub prefer: Option<Side>,
    #[arg(long, help = "Print what would change, change nothing")]
    pub dry_run: bool,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Local,
    Remote,
}

/// `.dcl-one/sync.json`: what the last sync left both sides agreeing on.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SyncState {
    pub node: String,
    pub project: String,
    pub version: i64,
    /// The node project's `created_at` (absent before the node sent it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    pub files: BTreeMap<String, String>,
}

/// The node's file set and its project's `created_at`.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct FileSet {
    pub version: i64,
    #[serde(default)]
    pub created_at: Option<String>,
    pub files: BTreeMap<String, String>,
}

/// Whether the node's project is another than the one `last` synced with:
/// deleted and made again under its id (when both sides know `created_at`).
fn remade(set: &FileSet, last: &SyncState) -> bool {
    matches!((&set.created_at, &last.created_at), (Some(now), Some(then)) if now != then)
}

/// What the node made of a file-set PUT.
enum Put {
    Saved(FileSet),
    /// 409: the node's set moved past the base version.
    Moved(FileSet),
    /// 400: these hashes are no upload on the node yet.
    Missing(Vec<String>),
}

/// A path both sides changed since the base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub path: String,
    pub local: Option<String>,
    pub remote: Option<String>,
}

/// Why a node change was not written here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hold {
    /// The folder's copy changed after the scan: edited, made or deleted
    /// while the sync waited on the node.
    Changed,
    /// This part of the path is a symlink: a write through it could leave
    /// the folder, one over it would replace the link.
    Symlink(String),
    /// This part of the path, which a directory has to be, is something else.
    NotADirectory(String),
    /// The path names something other than a regular file.
    NotAFile(String),
    /// This part of the path answers only to another spelling the folder
    /// lists (`Src/` for `src`): a case- or normalization-insensitive
    /// folder holds one of the two, which the sync tracks as two paths.
    Spelling(String),
}

impl std::fmt::Display for Hold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Hold::Changed => f.write_str("changed here while syncing"),
            Hold::Symlink(p) => write!(f, "{p} is a symlink"),
            Hold::NotADirectory(p) => write!(f, "{p} is not a directory"),
            Hold::NotAFile(p) => write!(f, "{p} is not a regular file"),
            Hold::Spelling(p) => write!(f, "the folder holds {p} under another spelling"),
        }
    }
}

/// A path this sync left as it is on both sides: a node change not written
/// here, or a base path a symlink or non-directory hides from the scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    pub path: String,
    pub why: Hold,
}

/// One sync's decisions: what the node's set and the folder become (`None`
/// deletes), and the base afterwards.
#[derive(Debug, Default, PartialEq)]
pub struct Plan {
    pub push: BTreeMap<String, Option<String>>,
    pub pull: BTreeMap<String, Option<String>>,
    pub conflicts: Vec<Conflict>,
    pub base: BTreeMap<String, String>,
}

impl Plan {
    /// The node's set with the pushes applied.
    fn remote_after(&self, remote: &BTreeMap<String, String>) -> BTreeMap<String, String> {
        let mut files = remote.clone();
        for (path, hash) in &self.push {
            match hash {
                Some(h) => files.insert(path.clone(), h.clone()),
                None => files.remove(path),
            };
        }
        files
    }
}

/// Whether the walk lists `path`: a valid node path, no pruned directory
/// on it, not a `.env` file nor a sync's temporary, and not the root's
/// `.dcl-one` in any case (a file there would leave the sync no room for
/// its state, on a case-insensitive folder too).
pub fn tracked(path: &str) -> bool {
    let mut segments: Vec<&str> = path.split('/').collect();
    let Some(name) = segments.pop() else {
        return false;
    };
    valid_path(path)
        && !path.eq_ignore_ascii_case(".dcl-one")
        && !segments.iter().any(|s| is_pruned_dir(s))
        && name != ".env"
        && !name.starts_with(".env.")
        && !is_sync_temp(name)
}

/// The name `replace` writes a file under before renaming it into place.
fn sync_temp_name() -> String {
    format!(
        ".dcl-one-sync-{}.tmp",
        hex::encode(rand::random::<[u8; 8]>())
    )
}

/// A temporary a killed sync left: `sync_temp_name`'s, or the
/// `.<target>.dcl-one-sync` older syncs wrote beside a valid target name.
fn is_sync_temp(name: &str) -> bool {
    let random = name
        .strip_prefix(".dcl-one-sync-")
        .and_then(|rest| rest.strip_suffix(".tmp"))
        .is_some_and(|hex| hex.len() == 16 && hex.bytes().all(|b| b.is_ascii_hexdigit()));
    let legacy = name
        .strip_prefix('.')
        .and_then(|rest| rest.strip_suffix(".dcl-one-sync"))
        .is_some_and(valid_path);
    random || legacy
}

/// The node's `project_files_path_check`.
fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && path.chars().count() <= MAX_PATH_CHARS
        && !path.contains(['\\', '\0'])
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// The three-way decision per path over `local`, `remote` and `base`.
pub fn plan(
    local: &BTreeMap<String, String>,
    remote: &BTreeMap<String, String>,
    base: &BTreeMap<String, String>,
    prefer: Option<Side>,
) -> Plan {
    let mut out = Plan::default();
    let paths: BTreeSet<&String> = local
        .keys()
        .chain(remote.keys().filter(|p| tracked(p)))
        .chain(base.keys())
        .collect();
    for path in paths {
        let (l, r, b) = (local.get(path), remote.get(path), base.get(path));
        let side = if l == r {
            None
        } else if l == b {
            Some(Side::Remote)
        } else if r == b {
            Some(Side::Local)
        } else if let Some(side) = prefer {
            Some(side)
        } else {
            out.conflicts.push(Conflict {
                path: path.clone(),
                local: l.cloned(),
                remote: r.cloned(),
            });
            if let Some(b) = b {
                out.base.insert(path.clone(), b.clone());
            }
            continue;
        };
        let agreed = match side {
            None => l,
            Some(Side::Remote) => {
                out.pull.insert(path.clone(), r.cloned());
                r
            }
            Some(Side::Local) => {
                out.push.insert(path.clone(), l.cloned());
                l
            }
        };
        if let Some(h) = agreed {
            out.base.insert(path.clone(), h.clone());
        }
    }
    out
}

/// The folder's tracked files, `path -> CIDv1`, and the entries left out
/// because their names are not UTF-8 (shown lossily); a file over the
/// node's cap or with a path it refuses stops the sync before anything
/// moves.
pub fn scan(root: &Path) -> Result<(BTreeMap<String, String>, Vec<String>)> {
    let mut paths = Vec::new();
    let mut unsynced = Vec::new();
    walk(root, root, &mut paths, &mut unsynced)?;
    unsynced.sort();
    let mut out = BTreeMap::new();
    for rel in paths {
        if !tracked(&rel) {
            let name = rel.rsplit('/').next().unwrap_or(&rel);
            if name.starts_with(".env")
                || is_sync_temp(name)
                || rel.eq_ignore_ascii_case(".dcl-one")
            {
                continue;
            }
            return Err(UserError::new(
                format!("{rel} cannot be synced: the node takes '/'-separated paths of at most {MAX_PATH_CHARS} characters"),
                TrySteps::one("rename it, or move it under a dot directory to keep it local"),
            )
            .into());
        }
        let full = root.join(&rel);
        let size = std::fs::metadata(&full)
            .with_context(|| format!("reading {}", full.display()))?
            .len();
        if size > MAX_FILE_BYTES {
            return Err(UserError::new(
                format!(
                    "{rel} is {size} bytes; the node keeps files of at most {MAX_FILE_BYTES} bytes"
                ),
                TrySteps::one(
                    "shrink or split it, or move it under a dot directory to keep it local",
                ),
            )
            .into());
        }
        let bytes = std::fs::read(&full).with_context(|| format!("reading {}", full.display()))?;
        out.insert(rel, hash_bytes_v1(&bytes));
    }
    if out.len() > MAX_FILES {
        return Err(UserError::new(
            format!(
                "the folder holds {} files; a node project holds at most {MAX_FILES}",
                out.len()
            ),
            TrySteps::one("move generated or vendored files under bin/, dist/ or a dot directory"),
        )
        .into());
    }
    Ok((out, unsynced))
}

/// The files under `dir`, into `out`; an entry whose name is not UTF-8
/// goes into `unsynced` instead: no node path names it, so it is neither
/// synced nor read as deleted.
fn walk(root: &Path, dir: &Path, out: &mut Vec<String>, unsynced: &mut Vec<String>) -> Result<()> {
    let entries = std::fs::read_dir(dir).with_context(|| format!("listing {}", dir.display()))?;
    for entry in entries {
        let entry = entry?;
        let kind = entry.file_type()?;
        let name = entry.file_name();
        if kind.is_symlink() || (kind.is_dir() && is_pruned_dir(&name.to_string_lossy())) {
            continue;
        }
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if name.to_str().is_none() {
            unsynced.push(rel);
        } else if kind.is_dir() {
            walk(root, &path, out, unsynced)?;
        } else {
            out.push(rel);
        }
    }
    Ok(())
}

fn state_path(root: &Path) -> PathBuf {
    root.join(".dcl-one").join(STATE_FILE)
}

pub fn load_state(root: &Path) -> Option<SyncState> {
    let raw = std::fs::read(state_path(root)).ok()?;
    serde_json::from_slice(&raw).ok()
}

fn state_rel() -> String {
    format!(".dcl-one/{STATE_FILE}")
}

fn state_refused(hold: Hold) -> anyhow::Error {
    UserError::new(
        format!("{hold}: sync keeps its state only in a real .dcl-one directory and file"),
        TrySteps::one("move aside (or rename) what stands there, then sync again"),
    )
    .into()
}

/// Refuses a `.dcl-one/` or state file that is a symlink (or not a
/// directory and file), before anything is read or changed.
fn check_state(root: &Path) -> Result<()> {
    match holds(root, &state_rel(), &Listings::default())? {
        Ok(_) => Ok(()),
        Err(hold) => Err(state_refused(hold)),
    }
}

/// `.dcl-one/`, made when missing (and kept out of git like
/// `scene::work_dir` keeps it), never through a link.
fn state_dir(root: &Path) -> Result<PathBuf> {
    let dir = match parent_dir(root, &state_rel(), true, &Listings::default())? {
        Ok(dir) => dir.context("making .dcl-one/")?,
        Err(hold) => return Err(state_refused(hold)),
    };
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(".gitignore"))
    {
        Ok(mut file) => std::io::Write::write_all(&mut file, b"*\n")?,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e).context("writing .dcl-one/.gitignore"),
    }
    Ok(dir)
}

fn save_state(root: &Path, state: &SyncState) -> Result<()> {
    let dir = state_dir(root)?;
    let ready = || Ok(holds(root, &state_rel(), &Listings::default())?.map(|_| false));
    let bytes = serde_json::to_vec_pretty(state)?;
    replace(&dir, STATE_FILE, bytes.as_slice(), ready)?.map_err(state_refused)
}

/// A folder's sync lock, held until dropped. Its descriptor lives on a thread
/// with a descriptor table of its own: in the shared table, a child another
/// thread forks during the sync would hold the lock until it execs, past the
/// sync's end, and refuse the next sync of the folder.
struct FolderLock {
    release: Option<std::sync::mpsc::Sender<()>>,
    holder: Option<std::thread::JoinHandle<()>>,
}

impl Drop for FolderLock {
    fn drop(&mut self) {
        // The holder closes the lock once its sender is gone; joined, so the
        // folder is free again by the time this returns.
        self.release.take();
        if let Some(holder) = self.holder.take() {
            let _ = holder.join();
        }
    }
}

fn take_lock(path: &Path, rel: &str) -> Result<std::fs::File> {
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)
        .with_context(|| format!("opening {rel}"))?;
    match lock.try_lock() {
        Ok(()) => Ok(lock),
        Err(std::fs::TryLockError::WouldBlock) => Err(UserError::new(
            "another sync of this folder is running",
            TrySteps::one("wait for it to finish, then sync again"),
        )
        .into()),
        Err(std::fs::TryLockError::Error(e)) => Err(e).with_context(|| format!("locking {rel}")),
    }
}

/// Takes the folder's sync lock, refused while another sync holds it, then
/// removes the temporaries a dead sync left in `.dcl-one/` (with the lock
/// held, none is a live sync's).
fn lock_folder(root: &Path) -> Result<FolderLock> {
    let dir = state_dir(root)?;
    let rel = format!(".dcl-one/{LOCK_FILE}");
    if let Err(hold) = holds(root, &rel, &Listings::default())? {
        return Err(state_refused(hold));
    }
    let path = dir.join(LOCK_FILE);
    let (locked_tx, locked) = std::sync::mpsc::channel::<Result<()>>();
    let (release, released) = std::sync::mpsc::channel::<()>();
    let holder = std::thread::Builder::new()
        .name("sync-folder-lock".into())
        .spawn(move || {
            crate::exec_file::private_descriptor_table();
            match take_lock(&path, &rel) {
                Ok(lock) => {
                    let _ = locked_tx.send(Ok(()));
                    let _ = released.recv();
                    drop(lock);
                }
                Err(e) => {
                    let _ = locked_tx.send(Err(e));
                }
            }
        })
        .context("starting the folder lock's thread")?;
    let lock = FolderLock {
        release: Some(release),
        holder: Some(holder),
    };
    locked
        .recv()
        .context("the folder lock's thread ended without an answer")??;
    for listed in std::fs::read_dir(&dir).context("listing .dcl-one/")? {
        let listed = listed?;
        if listed.file_name().to_str().is_some_and(is_sync_temp) {
            let _ = std::fs::remove_file(listed.path());
        }
    }
    Ok(lock)
}

/// A uuid v4, as the node keys projects.
fn new_project_id() -> String {
    let mut b: [u8; 16] = rand::random();
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = hex::encode(b);
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

fn is_project_id(raw: &str) -> bool {
    let parts: Vec<&str> = raw.split('-').collect();
    parts.iter().map(|p| p.len()).eq([8, 4, 4, 4, 12])
        && parts
            .iter()
            .all(|p| p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// The new project's title and layout: scene.json's `display.title` (the
/// folder's name otherwise) and the bounding box of `scene.parcels`.
fn project_shape(root: &Path) -> (String, i64, i64) {
    let scene: Value = std::fs::read(root.join("scene.json"))
        .ok()
        .and_then(|raw| serde_json::from_slice(&raw).ok())
        .unwrap_or(Value::Null);
    let title = scene
        .pointer("/display/title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .or_else(|| {
            dunce::canonicalize(root)
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        })
        .unwrap_or_else(|| "Scene".into());
    let parcels: Vec<(i64, i64)> = scene
        .pointer("/scene/parcels")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(|p| {
            let (x, y) = p.split_once(',')?;
            Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
        })
        .collect();
    let span = |f: fn(&(i64, i64)) -> i64| {
        let min = parcels.iter().map(f).min().unwrap_or(0);
        let max = parcels.iter().map(f).max().unwrap_or(0);
        (max - min + 1).clamp(1, 1000)
    };
    (
        title.chars().take(512).collect(),
        span(|p| p.0),
        span(|p| p.1),
    )
}

/// The node's builder API, signed.
struct Node {
    client: reqwest::Client,
    origin: String,
    signer: Signer,
}

struct Answer {
    status: u16,
    body: Value,
}

impl Answer {
    async fn of(response: reqwest::Response) -> Result<Self> {
        let status = response.status().as_u16();
        let raw = response.bytes().await?;
        Ok(Answer {
            status,
            body: serde_json::from_slice(&raw).unwrap_or(Value::Null),
        })
    }

    fn message(&self) -> String {
        self.body
            .get("error")
            .or_else(|| self.body.get("message"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("HTTP {}", self.status))
    }
}

impl Node {
    fn url(&self, route: &str) -> String {
        format!("{}/builder-api{route}", self.origin)
    }

    async fn send(
        &self,
        method: reqwest::Method,
        route: &str,
        body: Option<(Vec<u8>, &str)>,
        signed: bool,
    ) -> Result<Answer> {
        Answer::of(self.respond(method, route, body, signed).await?).await
    }

    /// The node's response, its body not yet read.
    async fn respond(
        &self,
        method: reqwest::Method,
        route: &str,
        body: Option<(Vec<u8>, &str)>,
        signed: bool,
    ) -> Result<reqwest::Response> {
        let url = self.url(route);
        let mut request = self.client.request(method.clone(), &url);
        if signed {
            let pathname = url::Url::parse(&url)
                .with_context(|| format!("{url} is not a URL"))?
                .path()
                .to_string();
            for (name, value) in
                self.signer
                    .headers(&method.as_str().to_lowercase(), &pathname, "{}")?
            {
                request = request.header(name, value);
            }
        }
        if let Some((bytes, content_type)) = body {
            request = request.header("content-type", content_type).body(bytes);
        }
        request.send().await.map_err(|e| {
            UserError::new(
                format!("could not reach the node at {}", self.origin),
                TrySteps::one("check --node and that the node is running"),
            )
            .caused_by(e)
            .into()
        })
    }

    async fn json(&self, method: reqwest::Method, route: &str, body: &Value) -> Result<Answer> {
        self.send(
            method,
            route,
            Some((serde_json::to_vec(body)?, "application/json")),
            true,
        )
        .await
    }

    fn refused(&self, what: &str, answer: &Answer) -> anyhow::Error {
        let steps = match answer.status {
            401 => TrySteps::one(
                "sign with the wallet that owns the project (--sign-key or DCL_PRIVATE_KEY)",
            ),
            503 => TrySteps::one("the node's builder writes are off (CREATE_WRITES_ENABLED)"),
            _ => TrySteps::one("check the node's logs"),
        };
        UserError::new(
            format!("{what}: the node answered {}", answer.status),
            steps,
        )
        .why(answer.message())
        .into()
    }

    async fn files(&self, project: &str) -> Result<Option<FileSet>> {
        let answer = self
            .send(
                reqwest::Method::GET,
                &format!("/v1/projects/{project}/files"),
                None,
                true,
            )
            .await?;
        match answer.status {
            200 => Ok(Some(serde_json::from_value(answer.body["data"].clone())?)),
            404 => Ok(None),
            _ => Err(self.refused(&format!("reading project {project}"), &answer)),
        }
    }

    /// The project made (or a deleted one made again: empty, its
    /// `files_version` moved on), as its empty set.
    async fn create(&self, project: &str, root: &Path) -> Result<FileSet> {
        let (title, cols, rows) = project_shape(root);
        let body = json!({ "project": {
            "id": project, "title": title, "description": "", "scene_id": project,
            "cols": cols, "rows": rows,
        } });
        let answer = self
            .json(
                reqwest::Method::PUT,
                &format!("/v1/projects/{project}"),
                &body,
            )
            .await?;
        if answer.status != 200 {
            return Err(self.refused(&format!("creating project {project}"), &answer));
        }
        let data = &answer.body["data"];
        Ok(FileSet {
            version: data["files_version"].as_i64().unwrap_or(0),
            created_at: data["created_at"].as_str().map(str::to_string),
            files: BTreeMap::new(),
        })
    }

    async fn upload(&self, project: &str, hash: &str, bytes: Vec<u8>) -> Result<()> {
        let answer = self
            .send(
                reqwest::Method::PUT,
                &format!("/v1/projects/{project}/blobs/{hash}"),
                Some((bytes, "application/octet-stream")),
                true,
            )
            .await?;
        if answer.status != 200 {
            return Err(self.refused(&format!("uploading {hash}"), &answer));
        }
        Ok(())
    }

    async fn put_files(
        &self,
        project: &str,
        base_version: i64,
        files: &BTreeMap<String, String>,
    ) -> Result<Put> {
        let answer = self
            .json(
                reqwest::Method::PUT,
                &format!("/v1/projects/{project}/files"),
                &json!({ "base_version": base_version, "files": files }),
            )
            .await?;
        let data = &answer.body["data"];
        match answer.status {
            200 => Ok(Put::Saved(serde_json::from_value(data.clone())?)),
            409 => Ok(Put::Moved(serde_json::from_value(data.clone())?)),
            400 if data["missing"].is_array() => Ok(Put::Missing(serde_json::from_value(
                data["missing"].clone(),
            )?)),
            _ => Err(self.refused("saving the file set", &answer)),
        }
    }

    /// The content `hash` names, written to `file` as it arrives (never
    /// whole in memory) and hashed on the way.
    async fn download(&self, hash: &str, file: &mut std::fs::File) -> Result<()> {
        let route = format!("/v1/storage/contents/{hash}");
        let mut response = self
            .respond(reqwest::Method::GET, &route, None, false)
            .await?;
        if response.status() != reqwest::StatusCode::OK {
            let answer = Answer::of(response).await?;
            return Err(self.refused(&format!("downloading {hash}"), &answer));
        }
        let mut hashed = HashV1Writer::new();
        while let Some(piece) = response.chunk().await? {
            hashed.update(&piece);
            std::io::Write::write_all(file, &piece)
                .with_context(|| format!("keeping {hash} in .dcl-one/"))?;
        }
        if hashed.finish() != hash {
            anyhow::bail!("the node's bytes for {hash} do not hash to it");
        }
        Ok(())
    }
}

/// What one sync did (or, dry, would do).
#[derive(Debug, Default)]
pub struct SyncReport {
    pub node: String,
    pub project: String,
    pub created: bool,
    pub version: i64,
    pub plan: Plan,
    /// Contents uploaded (each hash once).
    pub uploaded: usize,
    /// Paths left as they are on both sides.
    pub kept: Vec<Kept>,
    /// Folder entries named in no UTF-8 (shown lossily), left out.
    pub unsynced: Vec<String>,
}

/// `Ok(Err(hold))`: the folder's state refuses the change.
type Guarded<T> = Result<std::result::Result<T, Hold>>;

#[cfg(test)]
thread_local! {
    /// How many names this thread's syncs read from directory listings.
    static NAMES_READ: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The names directories list, each listing read once and kept for one
/// pass over the folder (never across a wait on the node): reading it
/// again for every name looked up in it costs a directory of n files n²
/// reads. The pass's own removals and new directories update it; a name
/// it lacks reads the listing again, so an entry made meanwhile by anyone
/// else is still found.
#[derive(Default)]
struct Listings(std::cell::RefCell<HashMap<PathBuf, HashSet<std::ffi::OsString>>>);

impl Listings {
    /// Whether `dir` lists exactly `name`.
    fn lists(&self, dir: &Path, name: &str) -> Result<bool> {
        let name = std::ffi::OsStr::new(name);
        if self
            .0
            .borrow()
            .get(dir)
            .is_some_and(|names| names.contains(name))
        {
            return Ok(true);
        }
        let mut names = HashSet::new();
        for listed in
            std::fs::read_dir(dir).with_context(|| format!("listing {}", dir.display()))?
        {
            names.insert(listed?.file_name());
        }
        #[cfg(test)]
        NAMES_READ.set(NAMES_READ.get() + names.len());
        let lists = names.contains(name);
        self.0.borrow_mut().insert(dir.to_path_buf(), names);
        Ok(lists)
    }

    /// The pass made the directory `name` in `dir`.
    fn made(&self, dir: &Path, name: &str) {
        if let Some(names) = self.0.borrow_mut().get_mut(dir) {
            names.insert(name.into());
        }
    }

    /// The pass removed `path`.
    fn removed(&self, path: &Path) {
        let mut listings = self.0.borrow_mut();
        listings.remove(path);
        if let (Some(dir), Some(name)) = (path.parent(), path.file_name())
            && let Some(names) = listings.get_mut(dir)
        {
            names.remove(name);
        }
    }
}

/// What `dir` holds under `name` (`walked` from the root), never following
/// a link: `None` when nothing answers to the name. Something that does
/// must be listed under exactly that name, byte for byte: a case- or
/// normalization-insensitive folder also answers `src` with its `Src/`,
/// which the walk lists, and the sync tracks, as another path.
fn entry(
    dir: &Path,
    name: &str,
    walked: &str,
    listings: &Listings,
) -> Guarded<Option<std::fs::Metadata>> {
    let found = match std::fs::symlink_metadata(dir.join(name)) {
        Ok(found) => found,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Ok(None)),
        Err(e) => return Err(e).with_context(|| format!("reading {walked}")),
    };
    if listings.lists(dir, name)? {
        Ok(Ok(Some(found)))
    } else {
        Ok(Err(Hold::Spelling(walked.into())))
    }
}

/// The directory `rel` sits in under `root`, walked down one component at a
/// time without following a symlink: each must be a real directory, under
/// exactly its name. A missing one is made when `make` (`create_dir` never
/// follows a link either), and answers `None` otherwise.
fn parent_dir(root: &Path, rel: &str, make: bool, listings: &Listings) -> Guarded<Option<PathBuf>> {
    let mut dir = root.to_path_buf();
    let mut walked = String::new();
    let parts: Vec<&str> = rel.split('/').collect();
    for part in &parts[..parts.len() - 1] {
        if !walked.is_empty() {
            walked.push('/');
        }
        walked.push_str(part);
        let found = match entry(&dir, part, &walked, listings)? {
            Ok(None) if make => match std::fs::create_dir(dir.join(part)) {
                Ok(()) => {
                    listings.made(&dir, part);
                    dir.push(part);
                    continue;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    entry(&dir, part, &walked, listings)?
                }
                Err(e) => return Err(e).with_context(|| format!("making {walked}")),
            },
            found => found,
        };
        match found {
            Ok(Some(m)) if m.is_dir() => dir.push(part),
            Ok(Some(m)) if m.file_type().is_symlink() => return Ok(Err(Hold::Symlink(walked))),
            Ok(Some(_)) => return Ok(Err(Hold::NotADirectory(walked))),
            Ok(None) => return Ok(Ok(None)),
            Err(hold) => return Ok(Err(hold)),
        }
    }
    Ok(Ok(Some(dir)))
}

/// What the folder holds at `rel` now, as the scan reads it: a regular
/// file's CIDv1 under exactly that name, `None` when absent; never read
/// through a symlink.
fn holds(root: &Path, rel: &str, listings: &Listings) -> Guarded<Option<String>> {
    let dir = match parent_dir(root, rel, false, listings)? {
        Ok(Some(dir)) => dir,
        Ok(None) => return Ok(Ok(None)),
        Err(hold) => return Ok(Err(hold)),
    };
    let name = rel.rsplit('/').next().unwrap_or(rel);
    match entry(&dir, name, rel, listings)? {
        Ok(None) => Ok(Ok(None)),
        Ok(Some(m)) if m.file_type().is_symlink() => Ok(Err(Hold::Symlink(rel.into()))),
        Ok(Some(m)) if !m.is_file() => Ok(Err(Hold::NotAFile(rel.into()))),
        Ok(Some(_)) => {
            let bytes = std::fs::read(dir.join(name)).with_context(|| format!("reading {rel}"))?;
            Ok(Ok(Some(hash_bytes_v1(&bytes))))
        }
        Err(hold) => Ok(Err(hold)),
    }
}

/// A fresh file in `dir`: its name is random and `create_new` claims it,
/// so it is never an existing file or link.
fn temp_file(dir: &Path) -> Result<(PathBuf, std::fs::File)> {
    let mut attempts = 0;
    loop {
        let tmp = dir.join(sync_temp_name());
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
        {
            Ok(file) => return Ok((tmp, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && attempts < 8 => {
                attempts += 1
            }
            Err(e) => return Err(e).with_context(|| format!("creating {}", tmp.display())),
        }
    }
}

/// `contents` copied to a `temp_file` in `dir`, renamed over `name` once
/// `ready` still agrees (`Ok(true)`: nothing left to write). The temporary
/// does not outlive a refusal or failure.
fn replace(
    dir: &Path,
    name: &str,
    mut contents: impl std::io::Read,
    ready: impl FnOnce() -> Guarded<bool>,
) -> Guarded<()> {
    let (tmp, mut file) = temp_file(dir)?;
    let written = std::io::copy(&mut contents, &mut file);
    drop(file);
    let mut renamed = false;
    let result = match written.map_err(anyhow::Error::from).and_then(|_| ready()) {
        Ok(Ok(false)) => match std::fs::rename(&tmp, dir.join(name)) {
            Ok(()) => {
                renamed = true;
                Ok(Ok(()))
            }
            Err(e) => Err(e.into()),
        },
        Ok(Ok(true)) => Ok(Ok(())),
        Ok(Err(hold)) => Ok(Err(hold)),
        Err(e) => Err(e),
    };
    if !renamed {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// One pull: `rel` becomes the content waiting in `want`'s temporary
/// (`None` deletes it), only while the folder still holds what the scan
/// read there (`scanned`), checked again right before the rename or
/// removal. The temporary itself is renamed into place when no later pull
/// needs it (`want`'s flag), and copied otherwise. Every directory below
/// `root` on the way must be a real one (a missing one is made a component
/// at a time) and the path itself no link, each listed under exactly its
/// name (a write never lands in, or over, another spelling's directory or
/// file). Std has no openat/renameat, so a directory swapped for a link
/// in the instant between that check and the call is outside this guard;
/// any link already there is refused. So is an edit saved in the instant
/// between the last hash and the rename or removal: it is replaced or
/// removed (no filesystem call compares and swaps contents). An edit after
/// it is caught by the check before the state is saved, which keeps that
/// path's old base.
fn apply_pull(
    root: &Path,
    rel: &str,
    scanned: Option<&String>,
    want: Option<(&String, &Path, bool)>,
    listings: &Listings,
) -> Guarded<()> {
    let wanted = want.map(|(hash, ..)| hash);
    // `Ok(true)`: the folder already holds what the node does.
    let check = || -> Guarded<bool> {
        Ok(match holds(root, rel, listings)? {
            Err(hold) => Err(hold),
            Ok(now) if now.as_ref() == wanted => Ok(true),
            Ok(now) if now.as_ref() == scanned => Ok(false),
            Ok(_) => Err(Hold::Changed),
        })
    };
    match check()? {
        Err(hold) => return Ok(Err(hold)),
        Ok(true) => return Ok(Ok(())),
        Ok(false) => {}
    }
    let Some((_, staged, last)) = want else {
        remove_pruning(root, &root.join(rel), listings)?;
        return Ok(Ok(()));
    };
    let dir = match parent_dir(root, rel, true, listings)? {
        Ok(dir) => dir.with_context(|| format!("making the directory of {rel}"))?,
        Err(hold) => return Ok(Err(hold)),
    };
    let name = rel.rsplit('/').next().unwrap_or(rel);
    if last {
        match check()? {
            Err(hold) => return Ok(Err(hold)),
            Ok(true) => return Ok(Ok(())),
            Ok(false) => match std::fs::rename(staged, dir.join(name)) {
                Ok(()) => return Ok(Ok(())),
                // `.dcl-one/` is on another filesystem: copied below.
                Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {}
                Err(e) => return Err(e).with_context(|| format!("moving {rel} into place")),
            },
        }
    }
    let contents =
        std::fs::File::open(staged).with_context(|| format!("reading {}", staged.display()))?;
    replace(&dir, name, contents, check)
}

/// Removes `path` and the directories it leaves empty, up to `root`.
fn remove_pruning(root: &Path, path: &Path, listings: &Listings) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => listings.removed(path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e).with_context(|| format!("deleting {}", path.display())),
    }
    let mut dir = path.parent();
    while let Some(d) = dir {
        if d == root || std::fs::remove_dir(d).is_err() {
            break;
        }
        listings.removed(d);
        dir = d.parent();
    }
    Ok(())
}

/// Pulled contents by hash, each in a temporary under `.dcl-one/` with the
/// number of pulls still to place it; dropped, it removes what is left.
#[derive(Default)]
struct Fetched(BTreeMap<String, (PathBuf, usize)>);

impl Fetched {
    /// The temporary holding `hash` for one pull, and whether that pull is
    /// the last to need it.
    fn take(&mut self, hash: &str) -> (&Path, bool) {
        let (path, left) = self
            .0
            .get_mut(hash)
            .expect("every pulled content is fetched");
        *left -= 1;
        (path, *left == 0)
    }
}

impl Drop for Fetched {
    fn drop(&mut self) {
        for (path, _) in self.0.values() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// The node and project `opts` sync with: the flags, else what `last`
/// remembers (its project only for the same node).
fn target(opts: &SyncOptions, last: Option<&SyncState>) -> Result<(String, Option<String>)> {
    let node = opts
        .node
        .clone()
        .or_else(|| last.map(|s| s.node.clone()))
        .map(|n| n.trim().trim_end_matches('/').to_string())
        .ok_or_else(|| {
            UserError::new(
                "no node to sync with",
                TrySteps::one("pass --node <url> (the next sync remembers it)"),
            )
        })?;
    if !url::Url::parse(&node).is_ok_and(|u| matches!(u.scheme(), "http" | "https")) {
        return Err(UserError::new(
            format!("{node} is not an http(s) URL"),
            TrySteps::one("pass --node https://<node>"),
        )
        .into());
    }
    let project = opts
        .project
        .clone()
        .or_else(|| last.filter(|s| s.node == node).map(|s| s.project.clone()));
    if let Some(p) = &project
        && !is_project_id(p)
    {
        return Err(UserError::new(
            format!("{p} is not a project id"),
            TrySteps::one("pass the project's uuid to --project"),
        )
        .into());
    }
    Ok((node, project))
}

/// The sync with `signer` (the command's key, a test's wallet).
pub async fn sync(opts: &SyncOptions, signer: Signer) -> Result<SyncReport> {
    let root = dunce::canonicalize(&opts.dir).map_err(|e| {
        UserError::new(
            format!("the directory {} does not exist", opts.dir.display()),
            TrySteps::one("check --dir"),
        )
        .caused_by(e)
    })?;
    check_state(&root)?;
    // The arguments are checked against a peek at the state first, so a
    // sync refused for them leaves nothing behind.
    target(opts, load_state(&root).as_ref())?;
    // Held until the sync returns, however it returns; the state the sync
    // plans from is read under it.
    let _lock = if opts.dry_run {
        None
    } else {
        Some(lock_folder(&root)?)
    };
    let last = load_state(&root);
    let (node, project) = target(opts, last.as_ref())?;
    let remembered = last
        .as_ref()
        .filter(|s| s.node == node && Some(&s.project) == project.as_ref());
    let mut base = remembered.map(|s| s.files.clone()).unwrap_or_default();
    let node_api = Node {
        client: crate::deploy::client(Duration::from_secs(10), Duration::from_secs(300))?,
        origin: node.clone(),
        signer,
    };

    let (mut local, unsynced) = scan(&root)?;
    let mut report = SyncReport {
        node: node.clone(),
        unsynced,
        ..SyncReport::default()
    };
    // A base path the scan no longer sees because a symlink or a
    // non-directory now stands in its way is held, not read as deleted
    // here: planned as unchanged (so nothing deletes it on the node), never
    // written, its base kept, reported while the node still holds it.
    //
    // A base an older build saved may claim two spellings of one folder
    // entry (src/a.ts beside the Src/a.ts the folder lists): the one only
    // the other answers to is dropped, never read as deleted here (which
    // would delete the node's copy). A case-only rename here leaves the
    // base no listed spelling, and still goes up.
    let listed: HashSet<String> = base
        .keys()
        .filter(|p| local.contains_key(*p))
        .map(|p| p.to_lowercase())
        .collect();
    let listings = Listings::default();
    let mut held = BTreeMap::new();
    let mut respelled = Vec::new();
    for path in base.keys().filter(|p| !local.contains_key(*p)) {
        match holds(&root, path, &listings)? {
            Err(why @ (Hold::Symlink(_) | Hold::NotADirectory(_))) => {
                held.insert(path.clone(), why);
            }
            Err(Hold::Spelling(_)) if listed.contains(&path.to_lowercase()) => {
                respelled.push(path.clone());
            }
            _ => {}
        }
    }
    for path in respelled {
        base.remove(&path);
    }
    for path in held.keys() {
        local.insert(path.clone(), base[path].clone());
    }
    let (project, mut remote) = match project {
        Some(p) => match (node_api.files(&p).await?, remembered) {
            (Some(set), Some(last)) if remade(&set, last) => {
                return Err(UserError::new(
                    format!(
                        "project {p} on {node} was deleted and made again since this folder's last sync; nothing was changed"
                    ),
                    TrySteps::one(format!(
                        "merge this folder into the new project as a first sync: remove .dcl-one/{STATE_FILE}, then sync --project {p}"
                    ))
                    .and(format!(
                        "or push this folder as a new project: remove .dcl-one/{STATE_FILE} and sync"
                    )),
                )
                .into());
            }
            (Some(set), _) => (p, set),
            (None, Some(_)) => {
                return Err(UserError::new(
                    format!(
                        "project {p} is no longer on {node} (deleted there since this folder's last sync); nothing was changed"
                    ),
                    TrySteps::one(format!(
                        "push this folder as a new project: remove .dcl-one/{STATE_FILE} and sync"
                    ))
                    .and("or sync with another project: --project <uuid>"),
                )
                .into());
            }
            (None, None) => {
                report.created = true;
                (p, FileSet::default())
            }
        },
        None => {
            report.created = true;
            (new_project_id(), FileSet::default())
        }
    };
    report.project = project.clone();
    if report.created && !opts.dry_run {
        remote = node_api.create(&project, &root).await?;
    }

    let mut uploaded: HashSet<String> = HashSet::new();
    let mut attempt = 0;
    let mut plan = loop {
        attempt += 1;
        let plan = plan(&local, &remote.files, &base, opts.prefer);
        let next = plan.remote_after(&remote.files);
        if opts.dry_run || next == remote.files {
            break plan;
        }
        let mut answer = node_api.put_files(&project, remote.version, &next).await?;
        if let Put::Missing(missing) = &answer {
            for hash in missing {
                let Some(path) = plan
                    .push
                    .iter()
                    .find_map(|(path, h)| (h.as_ref() == Some(hash)).then_some(path))
                else {
                    anyhow::bail!("the node lacks {hash}, which this sync does not upload");
                };
                let bytes =
                    std::fs::read(root.join(path)).with_context(|| format!("reading {path}"))?;
                if hash_bytes_v1(&bytes) != *hash {
                    anyhow::bail!("{path} changed while syncing; run sync again");
                }
                node_api.upload(&project, hash, bytes).await?;
                uploaded.insert(hash.clone());
            }
            answer = node_api.put_files(&project, remote.version, &next).await?;
        }
        match answer {
            Put::Saved(saved) => {
                remote = saved;
                break plan;
            }
            Put::Moved(moved) if attempt < PUT_ATTEMPTS => remote = moved,
            Put::Moved(_) => {
                return Err(UserError::new(
                    "the node's project kept changing while syncing",
                    TrySteps::one("run sync again"),
                )
                .into());
            }
            Put::Missing(missing) => {
                anyhow::bail!(
                    "the node still lacks {} after the upload",
                    missing.join(", ")
                )
            }
        }
    };
    report.uploaded = uploaded.len();
    report.version = remote.version;
    for (path, why) in held {
        plan.pull.remove(&path);
        // The base keeps it even once the node dropped it (then quietly):
        // the hidden copy that comes back meets that deletion, never reads
        // as a new file to push.
        plan.base.insert(path.clone(), base[&path].clone());
        if remote.files.contains_key(&path) {
            report.kept.push(Kept { path, why });
        }
    }

    if !opts.dry_run {
        // Every write's bytes are in hand (each hash-checked, each content
        // fetched once) before the folder changes: a failed download stops
        // the sync with the folder as it was, never after a deletion whose
        // file a write was bringing back under another name. Each streams
        // into a temporary under .dcl-one/ (never whole in memory), and
        // `fetched` removes what no pull moved into place, however the sync
        // ends.
        let staging = state_dir(&root)?;
        let mut fetched = Fetched::default();
        for hash in plan.pull.values().flatten() {
            if let Some((_, pulls)) = fetched.0.get_mut(hash) {
                *pulls += 1;
                continue;
            }
            let (path, mut file) = temp_file(&staging)?;
            fetched.0.insert(hash.clone(), (path, 1));
            node_api.download(hash, &mut file).await?;
        }
        // The base takes a pulled path's new hash only once the folder holds
        // it: a pull left unwritten keeps the path's old base, so the next
        // sync plans it again from both sides' copies (a conflict when the
        // folder's copy changed). An error part-way saves no state at all:
        // the old base stays, and the next sync finds every path already
        // written (or pushed) equal on both sides.
        let mut agreed = plan.base.clone();
        let mut applied = Vec::new();
        let listings = Listings::default();
        // Deletions go first: on a case- or normalization-insensitive folder
        // a node rename (scene.ts -> Scene.ts) pulls a write the old name
        // still answers for, and deleting the old name after it would
        // remove the only copy.
        let deletions = plan.pull.iter().filter(|(_, hash)| hash.is_none());
        let writes = plan.pull.iter().filter(|(_, hash)| hash.is_some());
        for (path, hash) in deletions.chain(writes) {
            let want = hash.as_ref().map(|h| {
                let (staged, last) = fetched.take(h);
                (h, staged, last)
            });
            match apply_pull(&root, path, local.get(path), want, &listings)
                .with_context(|| format!("writing {path}"))?
            {
                Ok(()) => applied.push((path, hash)),
                Err(why) => {
                    match base.get(path) {
                        Some(old) => agreed.insert(path.clone(), old.clone()),
                        None => agreed.remove(path),
                    };
                    report.kept.push(Kept {
                        path: path.clone(),
                        why,
                    });
                }
            }
        }
        #[cfg(test)]
        if let Some(meanwhile) = AFTER_PULLS.take() {
            meanwhile();
        }
        settle(&root, &applied, &base, &mut agreed, &listings)?;
        save_state(
            &root,
            &SyncState {
                node,
                project,
                version: remote.version,
                created_at: remote.created_at.clone(),
                files: agreed,
            },
        )?;
    }
    report.plan = plan;
    Ok(report)
}

#[cfg(test)]
thread_local! {
    /// A test's change to the folder after the pulls, before `settle`.
    static AFTER_PULLS: std::cell::Cell<Option<Box<dyn FnOnce()>>> =
        const { std::cell::Cell::new(None) };
}

/// The base after the pulls: `agreed` keeps an `applied` pull's new entry
/// only where the folder holds it now, under exactly its name; anything
/// else (an edit right after the write, a name only another spelling
/// answers to) takes back the old `base` entry, and the next sync compares
/// again.
fn settle(
    root: &Path,
    applied: &[(&String, &Option<String>)],
    base: &BTreeMap<String, String>,
    agreed: &mut BTreeMap<String, String>,
    listings: &Listings,
) -> Result<()> {
    for &(path, hash) in applied {
        let now = match holds(root, path, listings)? {
            Ok(now) => Some(now),
            // Nothing is under a file, nor under a name the folder lists
            // only in another spelling: a deletion there holds.
            Err(Hold::NotADirectory(_) | Hold::Spelling(_)) => Some(None),
            Err(_) => None,
        };
        if now.as_ref() != Some(hash) {
            match base.get(path) {
                Some(old) => agreed.insert(path.clone(), old.clone()),
                None => agreed.remove(path),
            };
        }
    }
    Ok(())
}

fn short(hash: &Option<String>) -> String {
    match hash {
        Some(h) => h
            .chars()
            .rev()
            .take(8)
            .collect::<String>()
            .chars()
            .rev()
            .collect(),
        None => "deleted".into(),
    }
}

/// `dcl-scene-cli sync`.
pub async fn run(opts: SyncOptions) -> Result<()> {
    let signer = crate::deploy::load_signer(opts.sign_key.as_deref())?.ok_or_else(|| {
        UserError::new(
            "sync needs a signing key: the node keeps the project for the wallet that signs",
            TrySteps::one("set DCL_PRIVATE_KEY or pass --sign-key <file>"),
        )
    })?;
    let report = sync(&opts, Signer::Key(signer.private_key_hex())).await?;
    let dry = if opts.dry_run { " (dry run)" } else { "" };
    for (path, hash) in &report.plan.push {
        ux::note(format!("node  <- {path} ({}){dry}", short(hash)));
    }
    let kept: HashSet<&str> = report.kept.iter().map(|k| k.path.as_str()).collect();
    for (path, hash) in &report.plan.pull {
        if !kept.contains(path.as_str()) {
            ux::note(format!("local <- {path} ({}){dry}", short(hash)));
        }
    }
    let created = if report.created { ", created" } else { "" };
    ux::note(format!(
        "project {} on {}{created}: version {}, {} pushed ({} uploaded), {} pulled{dry}",
        report.project,
        report.node,
        report.version,
        report.plan.push.len(),
        report.uploaded,
        report
            .plan
            .pull
            .keys()
            .filter(|p| !kept.contains(p.as_str()))
            .count(),
    ));
    let mut what = Vec::new();
    let mut steps = Vec::new();
    if !report.plan.conflicts.is_empty() {
        let listed: Vec<String> = report
            .plan
            .conflicts
            .iter()
            .map(|c| {
                format!(
                    "{} (local {}, node {})",
                    c.path,
                    short(&c.local),
                    short(&c.remote)
                )
            })
            .collect();
        what.push(format!(
            "{} path(s) changed on both sides since the last sync and were left alone: {}",
            listed.len(),
            listed.join(", ")
        ));
        steps.push("keep one side: sync --prefer local, or sync --prefer remote");
        steps.push("or make both copies equal by hand and sync again");
    }
    if !report.kept.is_empty() {
        let listed: Vec<String> = report
            .kept
            .iter()
            .map(|k| format!("{} ({})", k.path, k.why))
            .collect();
        what.push(format!(
            "{} path(s) were left as they are on both sides: {}",
            listed.len(),
            listed.join(", ")
        ));
        if report.kept.iter().any(|k| k.why == Hold::Changed) {
            steps.push("sync again: a path changed here meanwhile is then a conflict to resolve");
        }
        if report
            .kept
            .iter()
            .any(|k| matches!(k.why, Hold::Spelling(_)))
        {
            steps.push(
                "this folder holds names that differ only in case as one: give the folder's copy the node's spelling, then sync again",
            );
            steps.push("or, where the node holds both spellings, rename one of them there");
        }
        // A held base path is out of the plan's pulls; a refused pull is in.
        let (hidden, refused): (Vec<&Kept>, Vec<&Kept>) = report
            .kept
            .iter()
            .filter(|k| !matches!(k.why, Hold::Changed | Hold::Spelling(_)))
            .partition(|k| !report.plan.pull.contains_key(&k.path));
        if !refused.is_empty() {
            steps.push(
                "to take the node's copy, replace the symlink or file in the way with a real directory or file, then sync again",
            );
        }
        if !hidden.is_empty() {
            steps.push(
                "the node still holds the files a symlink or file here now hides: to sync them again, make it a real directory",
            );
            steps.push(
                "if the symlink or file is deliberate: move it aside, sync once (the node then drops what it hid), and put it back",
            );
        }
    }
    if !report.unsynced.is_empty() {
        what.push(format!(
            "{} name(s) here are not UTF-8, which every node path is, and were not synced: {}",
            report.unsynced.len(),
            report.unsynced.join(", ")
        ));
        steps.push("rename them, or move them under a dot directory to keep them local");
    }
    let Some((first, rest)) = steps.split_first() else {
        return Ok(());
    };
    let try_next = rest.iter().fold(TrySteps::one(*first), |t, s| t.and(*s));
    Err(UserError::new(what.join("; "), try_next).into())
}

#[cfg(test)]
#[path = "project_sync_tests_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "project_sync_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "project_sync_tests_hold.rs"]
mod tests_hold;
