//! The restore stamps behind every write switch (`WRITES_RESTORE_STAMP_DIR`):
//! deploy/backup/backup.sh writes `backup.json` once a snapshot's manifest is
//! complete, verify-restore.sh writes `drill.json` once a restore drill of
//! that snapshot passed. With the directory set, a switch that is on lets
//! writes through only while the last complete backup's snapshot is under
//! `WRITES_BACKUP_MAX_AGE_HOURS` old, the last passing drill under
//! `WRITES_DRILL_MAX_AGE_DAYS` and the snapshot it restored no more than that
//! behind the backup's. A plane whose writes store blobs (every plane but
//! ops) also needs both stamps to cover the blobs ([`RestoreStamps::check_blobs`]).
//! The check is live: each gated write stats both files and re-reads one
//! whose inode, size or mtime moved (or after [`REREAD`]), so a stamp going stale
//! holds writes and a fresh one releases them without a restart.

use std::fmt;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};

pub const RESTORE_STAMP_DIR: &str = "WRITES_RESTORE_STAMP_DIR";
pub const BACKUP_MAX_AGE_HOURS: &str = "WRITES_BACKUP_MAX_AGE_HOURS";
pub const DRILL_MAX_AGE_DAYS: &str = "WRITES_DRILL_MAX_AGE_DAYS";

/// A parsed stamp is trusted this long while its size and mtime stay put.
pub const REREAD: Duration = Duration::from_secs(60);

/// How far ahead of this node's clock a stamp may be dated (two hosts'
/// clocks); further is a stamp nobody should trust.
const FUTURE_SLACK: Duration = Duration::from_secs(3600);

/// One of the two stamps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stamp {
    Backup,
    Drill,
}

impl Stamp {
    pub const ALL: [Stamp; 2] = [Stamp::Backup, Stamp::Drill];

    pub fn file(self) -> &'static str {
        match self {
            Stamp::Backup => "backup.json",
            Stamp::Drill => "drill.json",
        }
    }

    /// The `kind` the stamp's JSON carries.
    pub fn kind(self) -> &'static str {
        match self {
            Stamp::Backup => "backup",
            Stamp::Drill => "drill",
        }
    }

    fn what(self) -> &'static str {
        match self {
            Stamp::Backup => "the last complete backup",
            Stamp::Drill => "the last passing restore drill",
        }
    }

    fn left_out_by(self) -> &'static str {
        match self {
            Stamp::Backup => "backup.sh --no-blobs",
            Stamp::Drill => "verify-restore.sh --no-blobs",
        }
    }

    fn noun(self) -> &'static str {
        match self {
            Stamp::Backup => "backup stamp",
            Stamp::Drill => "restore-drill stamp",
        }
    }

    fn budget_var(self) -> &'static str {
        match self {
            Stamp::Backup => BACKUP_MAX_AGE_HOURS,
            Stamp::Drill => DRILL_MAX_AGE_DAYS,
        }
    }

    fn budget_value(self, budget: Duration) -> u64 {
        match self {
            Stamp::Backup => budget.as_secs() / 3600,
            Stamp::Drill => budget.as_secs() / 86_400,
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// Why the stamps hold writes right now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hold {
    Missing(Stamp),
    Unreadable(Stamp, String),
    Stale {
        stamp: Stamp,
        age: Duration,
        budget: Duration,
    },
    Future {
        stamp: Stamp,
        ahead: Duration,
    },
    /// The stamp's snapshot or drill left the blobs out (`--no-blobs`): it
    /// does not vouch for the planes that store blobs.
    NoBlobs(Stamp),
    /// The last passing drill restored a snapshot further behind the last
    /// backup's than the drill budget.
    DrillBehind {
        drilled: String,
        backup: String,
        behind: Duration,
        budget: Duration,
    },
}

impl Hold {
    pub fn stamp(&self) -> Stamp {
        match self {
            Hold::Missing(s) | Hold::Unreadable(s, _) | Hold::NoBlobs(s) => *s,
            Hold::Stale { stamp, .. } | Hold::Future { stamp, .. } => *stamp,
            Hold::DrillBehind { .. } => Stamp::Drill,
        }
    }
}

impl fmt::Display for Hold {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Hold::Missing(s) => write!(
                f,
                "there is no {} ({} in {RESTORE_STAMP_DIR})",
                s.noun(),
                s.file()
            ),
            Hold::Unreadable(s, why) => {
                write!(f, "the {} ({}) cannot be read: {why}", s.noun(), s.file())
            }
            Hold::Stale { stamp, age, budget } => write!(
                f,
                "{} ({}) is {} old, past {}={}",
                stamp.what(),
                stamp.file(),
                human(*age),
                stamp.budget_var(),
                stamp.budget_value(*budget)
            ),
            Hold::Future { stamp, ahead } => write!(
                f,
                "the {} ({}) is dated {} ahead of this node's clock",
                stamp.noun(),
                stamp.file(),
                human(*ahead)
            ),
            Hold::NoBlobs(s) => write!(
                f,
                "{} ({}) left the blobs out ({})",
                s.what(),
                s.file(),
                s.left_out_by()
            ),
            Hold::DrillBehind {
                drilled,
                backup,
                behind,
                budget,
            } => write!(
                f,
                "{} ({}) restored snapshot {drilled}, {} behind the last complete backup's {backup}, past {}={}",
                Stamp::Drill.what(),
                Stamp::Drill.file(),
                human(*behind),
                Stamp::Drill.budget_var(),
                Stamp::Drill.budget_value(*budget)
            ),
        }
    }
}

/// `45 min`, `31 h 5 min` under two days, `9 d 4 h` from there.
pub fn human(d: Duration) -> String {
    let mins = d.as_secs() / 60;
    if mins < 60 {
        return format!("{mins} min");
    }
    if mins < 48 * 60 {
        let (h, m) = (mins / 60, mins % 60);
        return if m == 0 {
            format!("{h} h")
        } else {
            format!("{h} h {m} min")
        };
    }
    let (days, h) = (mins / 1440, mins / 60 % 24);
    if h == 0 {
        format!("{days} d")
    } else {
        format!("{days} d {h} h")
    }
}

/// One stamp as the last check read it. A backup's age runs from its
/// snapshot's start (`snapshot_at`, the dump's), a drill's from its end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StampState {
    pub stamp: Stamp,
    pub completed_at: Option<DateTime<Utc>>,
    pub snapshot: Option<String>,
    pub blobs: Option<bool>,
    pub age: Option<Duration>,
    pub budget: Duration,
    pub hold: Option<Hold>,
}

/// What a stamp file says.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Parsed {
    completed_at: DateTime<Utc>,
    aged_from: DateTime<Utc>,
    snapshot: Option<String>,
    blobs: bool,
}

/// A snapshot id (`20261002T031500Z`, backup.sh's `stamp`) as an instant.
fn snapshot_time(id: &str) -> Option<DateTime<Utc>> {
    chrono::NaiveDateTime::parse_from_str(id, "%Y%m%dT%H%M%SZ")
        .ok()
        .map(|t| t.and_utc())
}

type Clock = Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>;

/// What a read of one stamp file found, keyed by the file's inode, size and
/// mtime (a stamp renamed over the last one in the same mtime tick and of
/// the same size is still another inode).
struct Read {
    key: (u64, u64, Option<SystemTime>),
    read_at: DateTime<Utc>,
    parsed: Result<Parsed, String>,
}

struct Inner {
    dir: PathBuf,
    budgets: [Duration; 2],
    now: Clock,
    reads: Mutex<[Option<Read>; 2]>,
}

/// The stamp directory and its two budgets; cheap to clone.
#[derive(Clone)]
pub struct RestoreStamps(Arc<Inner>);

impl fmt::Debug for RestoreStamps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RestoreStamps")
            .field("dir", &self.0.dir)
            .field("budgets", &self.0.budgets)
            .finish()
    }
}

impl RestoreStamps {
    pub fn new(dir: impl Into<PathBuf>, backup_max_age: Duration, drill_max_age: Duration) -> Self {
        Self(Arc::new(Inner {
            dir: dir.into(),
            budgets: [backup_max_age, drill_max_age],
            now: Arc::new(Utc::now),
            reads: Mutex::new([None, None]),
        }))
    }

    /// From the three settings; a zero budget is refused (it would hold
    /// every write forever, which is what turning the switch off is for).
    pub fn from_settings(
        dir: &str,
        backup_max_age_hours: u64,
        drill_max_age_days: u64,
    ) -> Result<Self, String> {
        if dir.trim().is_empty() {
            return Err(format!("{RESTORE_STAMP_DIR} is empty"));
        }
        if backup_max_age_hours == 0 {
            return Err(format!("{BACKUP_MAX_AGE_HOURS}=0 would hold every write"));
        }
        if drill_max_age_days == 0 {
            return Err(format!("{DRILL_MAX_AGE_DAYS}=0 would hold every write"));
        }
        Ok(Self::new(
            dir.trim(),
            Duration::from_secs(backup_max_age_hours * 3600),
            Duration::from_secs(drill_max_age_days * 86_400),
        ))
    }

    /// The same stamps judged against `now` instead of the wall clock.
    pub fn with_clock(self, now: impl Fn() -> DateTime<Utc> + Send + Sync + 'static) -> Self {
        Self(Arc::new(Inner {
            dir: self.0.dir.clone(),
            budgets: self.0.budgets,
            now: Arc::new(now),
            reads: Mutex::new([None, None]),
        }))
    }

    pub fn dir(&self) -> &Path {
        &self.0.dir
    }

    /// `Ok` while both stamps vouch for this node; the backup's hold first.
    pub fn check(&self) -> Result<(), Hold> {
        self.verdict(false)
    }

    /// [`check`](Self::check) for a plane whose writes store blobs: a
    /// backup or drill that left the blobs out holds it as well.
    pub fn check_blobs(&self) -> Result<(), Hold> {
        self.verdict(true)
    }

    fn verdict(&self, blobs: bool) -> Result<(), Hold> {
        let [backup, drill] = self.states();
        for state in [&backup, &drill] {
            if let Some(hold) = &state.hold {
                return Err(hold.clone());
            }
        }
        if let (Some(b), Some(d)) = (&backup.snapshot, &drill.snapshot)
            && let (Some(bt), Some(dt)) = (snapshot_time(b), snapshot_time(d))
            && let Ok(behind) = (bt - dt).to_std()
            && behind > drill.budget
        {
            return Err(Hold::DrillBehind {
                drilled: d.clone(),
                backup: b.clone(),
                behind,
                budget: drill.budget,
            });
        }
        if blobs {
            for state in [&backup, &drill] {
                if state.blobs == Some(false) {
                    return Err(Hold::NoBlobs(state.stamp));
                }
            }
        }
        Ok(())
    }

    /// Both stamps as they read now.
    pub fn states(&self) -> [StampState; 2] {
        Stamp::ALL.map(|s| self.state(s))
    }

    fn state(&self, stamp: Stamp) -> StampState {
        let budget = self.0.budgets[stamp.index()];
        let now = (self.0.now)();
        let mut out = StampState {
            stamp,
            completed_at: None,
            snapshot: None,
            blobs: None,
            age: None,
            budget,
            hold: None,
        };
        let parsed = match self.parsed(stamp, now) {
            Ok(p) => p,
            Err(hold) => {
                out.hold = Some(hold);
                return out;
            }
        };
        out.completed_at = Some(parsed.completed_at);
        out.snapshot = parsed.snapshot;
        out.blobs = Some(parsed.blobs);
        let completed_at = parsed.aged_from;
        match (now - completed_at).to_std() {
            Ok(age) => {
                out.age = Some(age);
                if age > budget {
                    out.hold = Some(Hold::Stale { stamp, age, budget });
                }
            }
            Err(_) => {
                let ahead = (completed_at - now).to_std().unwrap_or_default();
                out.age = Some(Duration::ZERO);
                if ahead > FUTURE_SLACK {
                    out.hold = Some(Hold::Future { stamp, ahead });
                }
            }
        }
        out
    }

    fn parsed(&self, stamp: Stamp, now: DateTime<Utc>) -> Result<Parsed, Hold> {
        let path = self.0.dir.join(stamp.file());
        let meta = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Hold::Missing(stamp)),
            Err(e) => return Err(Hold::Unreadable(stamp, e.to_string())),
        };
        let key = (file_id(&meta), meta.len(), meta.modified().ok());
        let mut reads = self.0.reads.lock().unwrap_or_else(|p| p.into_inner());
        let slot = &mut reads[stamp.index()];
        let fresh = slot.as_ref().is_some_and(|r| {
            r.key == key && (now - r.read_at).to_std().is_ok_and(|since| since < REREAD)
        });
        if !fresh {
            *slot = Some(Read {
                key,
                read_at: now,
                parsed: read_stamp(&path, stamp),
            });
        }
        let read = slot.as_ref().expect("filled above");
        read.parsed
            .clone()
            .map_err(|why| Hold::Unreadable(stamp, why))
    }
}

/// The file's inode; platforms without one fall back to `0`, leaving size and
/// mtime as the change key.
#[cfg(unix)]
fn file_id(meta: &std::fs::Metadata) -> u64 {
    meta.ino()
}

#[cfg(not(unix))]
fn file_id(_meta: &std::fs::Metadata) -> u64 {
    0
}

/// A stamp file whose `kind` names `stamp`. A stamp that does not say its
/// blobs were included counts as one that left them out.
fn read_stamp(path: &Path, stamp: Stamp) -> Result<Parsed, String> {
    let raw = std::fs::read(path).map_err(|e| e.to_string())?;
    let v: serde_json::Value =
        serde_json::from_slice(&raw).map_err(|e| format!("not JSON: {e}"))?;
    let kind = v.get("kind").and_then(|k| k.as_str()).unwrap_or("");
    if kind != stamp.kind() {
        return Err(format!("kind is {kind:?}, not {:?}", stamp.kind()));
    }
    let instant = |field: &str| -> Result<Option<DateTime<Utc>>, String> {
        v.get(field)
            .and_then(|c| c.as_str())
            .map(|at| {
                DateTime::parse_from_rfc3339(at)
                    .map(|t| t.with_timezone(&Utc))
                    .map_err(|e| format!("{field} {at:?}: {e}"))
            })
            .transpose()
    };
    let completed_at = instant("completed_at")?.ok_or("no completed_at")?;
    let snapshot = v
        .get("snapshot")
        .and_then(|s| s.as_str())
        .map(str::to_owned);
    if let Some(id) = &snapshot
        && snapshot_time(id).is_none()
    {
        return Err(format!("snapshot {id:?} is not a snapshot id"));
    }
    let aged_from = match stamp {
        Stamp::Backup => instant("snapshot_at")?.unwrap_or(completed_at),
        Stamp::Drill => completed_at,
    };
    let blobs = v
        .pointer("/blobs/included")
        .and_then(|b| b.as_bool())
        .unwrap_or(false);
    Ok(Parsed {
        completed_at,
        aged_from,
        snapshot,
        blobs,
    })
}

static INSTALLED: OnceLock<RestoreStamps> = OnceLock::new();

#[cfg(any(test, feature = "test-support"))]
tokio::task_local! {
    static SCOPED: RestoreStamps;
}

/// Makes `stamps` the process's restore stamps (`serve`, once at startup);
/// `false` if some were installed already.
pub fn install(stamps: RestoreStamps) -> bool {
    INSTALLED.set(stamps).is_ok()
}

/// The stamps this process checks: none unless [`install`]ed (a test's
/// [`scoped`] ones first).
pub fn installed() -> Option<RestoreStamps> {
    #[cfg(any(test, feature = "test-support"))]
    if let Ok(stamps) = SCOPED.try_with(Clone::clone) {
        return Some(stamps);
    }
    INSTALLED.get().cloned()
}

/// Why the restore stamps hold writes right now; `None` without stamps.
pub fn hold() -> Option<Hold> {
    installed().and_then(|s| s.check().err())
}

/// [`hold`] for a plane whose writes store blobs.
pub fn blob_hold() -> Option<Hold> {
    installed().and_then(|s| s.check_blobs().err())
}

/// Runs `f` with `stamps` as the restore stamps every gate it reaches sees,
/// in place of the process's: one test's stamps never leak into another's.
#[cfg(any(test, feature = "test-support"))]
pub async fn scoped<F: std::future::Future>(stamps: RestoreStamps, f: F) -> F::Output {
    SCOPED.scope(stamps, f).await
}

/// A stamp directory for tests, judged on a clock the test moves.
#[cfg(any(test, feature = "test-support"))]
pub mod lab {
    use super::*;
    use std::sync::atomic::{AtomicI64, Ordering};

    pub const HOUR: Duration = Duration::from_secs(3600);
    pub const DAY: Duration = Duration::from_secs(86_400);

    /// A temporary stamp directory (removed on drop) under the default
    /// budgets (26 h, 8 d), at a fixed instant [`StampLab::advance`] moves.
    pub struct StampLab {
        pub dir: PathBuf,
        pub stamps: RestoreStamps,
        now: Arc<AtomicI64>,
        last_len: AtomicI64,
    }

    pub fn t0() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-02T03:00:00Z")
            .expect("a literal instant")
            .with_timezone(&Utc)
    }

    impl StampLab {
        pub fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "restore-stamps-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4().simple()
            ));
            std::fs::create_dir_all(&dir).expect("a temporary directory");
            let now = Arc::new(AtomicI64::new(t0().timestamp()));
            let clock = now.clone();
            let stamps = RestoreStamps::new(&dir, 26 * HOUR, 8 * DAY).with_clock(move || {
                DateTime::from_timestamp(clock.load(Ordering::SeqCst), 0).expect("in range")
            });
            Self {
                dir,
                stamps,
                now,
                last_len: AtomicI64::new(0),
            }
        }

        /// Both stamps an hour old: writes go through.
        pub fn fresh() -> Self {
            let lab = Self::new();
            lab.write_aged(Stamp::Backup, HOUR);
            lab.write_aged(Stamp::Drill, HOUR);
            lab
        }

        pub fn now(&self) -> DateTime<Utc> {
            DateTime::from_timestamp(self.now.load(Ordering::SeqCst), 0).expect("in range")
        }

        pub fn advance(&self, d: Duration) {
            self.now.fetch_add(d.as_secs() as i64, Ordering::SeqCst);
        }

        /// The stamp as the scripts write it (a temporary file renamed over
        /// it), completed `age` before the lab's now.
        pub fn write_aged(&self, stamp: Stamp, age: Duration) {
            let at = self.now() - chrono::Duration::from_std(age).expect("in range");
            self.write(stamp, at, "ab");
        }

        /// The stamp the scripts write for a snapshot taken at
        /// `completed_at` (so a backup ages from it), blobs included.
        pub fn write(&self, stamp: Stamp, completed_at: DateTime<Utc>, digest: &str) {
            let at = completed_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            self.write_json(
                stamp,
                serde_json::json!({
                    "kind": stamp.kind(),
                    "completed_at": at,
                    "snapshot": completed_at.format("%Y%m%dT%H%M%SZ").to_string(),
                    "snapshot_at": at,
                    "manifest_sha256": digest,
                    "blobs": { "included": true },
                }),
            );
        }

        /// `body` as `stamp`'s file, padded longer than the write before
        /// it (a body may shrink, `false` to `true`): two writes in one
        /// tick of the file system's clock still differ in size.
        pub fn write_json(&self, stamp: Stamp, mut body: serde_json::Value) {
            body["pad"] = serde_json::Value::String(String::new());
            let bare = body.to_string().len() as i64;
            let longer = self.last_len.load(Ordering::SeqCst) + 1 - bare;
            body["pad"] = serde_json::Value::String("x".repeat(longer.max(0) as usize));
            let text = body.to_string();
            self.last_len.store(text.len() as i64, Ordering::SeqCst);
            let tmp = self.dir.join(format!("{}.tmp", stamp.file()));
            std::fs::write(&tmp, text).expect("writing a stamp");
            std::fs::rename(&tmp, self.dir.join(stamp.file())).expect("renaming a stamp");
        }
    }

    impl Default for StampLab {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Drop for StampLab {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::lab::{DAY, HOUR, StampLab, t0};
    use super::*;

    #[test]
    fn missing_stamps_hold_and_name_the_file() {
        let lab = StampLab::new();
        let hold = lab.stamps.check().unwrap_err();
        assert_eq!(hold, Hold::Missing(Stamp::Backup));
        assert_eq!(
            hold.to_string(),
            "there is no backup stamp (backup.json in WRITES_RESTORE_STAMP_DIR)"
        );
        lab.write(Stamp::Backup, t0(), "ab");
        let hold = lab.stamps.check().unwrap_err();
        assert_eq!(hold, Hold::Missing(Stamp::Drill));
        assert!(hold.to_string().contains("drill.json"), "{hold}");
        lab.write(Stamp::Drill, t0(), "ab");
        assert_eq!(lab.stamps.check(), Ok(()));
    }

    #[test]
    fn a_stale_backup_holds_live_and_names_its_age() {
        let lab = StampLab::new();
        lab.write(Stamp::Backup, t0(), "ab");
        lab.write(Stamp::Drill, t0(), "ab");
        lab.advance(26 * HOUR);
        assert_eq!(lab.stamps.check(), Ok(()), "26 h is within the budget");
        lab.advance(5 * HOUR + Duration::from_secs(5 * 60));
        let hold = lab.stamps.check().unwrap_err();
        assert_eq!(
            hold.to_string(),
            "the last complete backup (backup.json) is 31 h 5 min old, past WRITES_BACKUP_MAX_AGE_HOURS=26"
        );
        // the next backup's stamp releases writes on the next check
        lab.write_aged(Stamp::Backup, Duration::ZERO);
        assert_eq!(lab.stamps.check(), Ok(()));
    }

    #[test]
    fn a_stale_drill_holds_and_names_its_age_in_days() {
        let lab = StampLab::new();
        lab.write(Stamp::Drill, t0(), "ab");
        lab.advance(9 * DAY + 4 * HOUR);
        lab.write_aged(Stamp::Backup, 4 * HOUR);
        let hold = lab.stamps.check().unwrap_err();
        assert_eq!(
            hold.to_string(),
            "the last passing restore drill (drill.json) is 9 d 4 h old, past WRITES_DRILL_MAX_AGE_DAYS=8"
        );
        assert_eq!(hold.stamp(), Stamp::Drill);
    }

    #[test]
    fn the_lab_grows_each_write_even_when_the_body_shrinks() {
        let lab = StampLab::new();
        let path = lab.dir.join(Stamp::Backup.file());
        let sizes: Vec<u64> = [false, true, false, true]
            .into_iter()
            .map(|included| {
                let body = serde_json::json!({ "blobs": { "included": included } });
                lab.write_json(Stamp::Backup, body);
                std::fs::metadata(&path).unwrap().len()
            })
            .collect();
        assert!(sizes.windows(2).all(|w| w[1] > w[0]), "{sizes:?}");
    }

    #[test]
    fn a_rewrite_is_seen_at_once_and_an_unchanged_file_is_reread_after_a_minute() {
        let lab = StampLab::fresh();
        assert_eq!(lab.stamps.check(), Ok(()));
        // corrupt the stamp in place, keeping its size and mtime: the cached
        // read stands until REREAD has passed
        let path = lab.dir.join("backup.json");
        let meta = std::fs::metadata(&path).unwrap();
        std::fs::write(&path, vec![b'x'; meta.len() as usize]).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(meta.modified().unwrap())
            .unwrap();
        assert_eq!(lab.stamps.check(), Ok(()), "cached within a minute");
        lab.advance(REREAD);
        assert!(matches!(
            lab.stamps.check(),
            Err(Hold::Unreadable(Stamp::Backup, _))
        ));
        // a stamp renamed over it has another size or mtime: read at once
        lab.write(Stamp::Backup, lab.now(), "abcd");
        assert_eq!(lab.stamps.check(), Ok(()));
    }

    // unix only: Windows has no inode (`file_id` is 0), so a renamed stamp keys the same.
    #[cfg(unix)]
    #[test]
    fn a_stamp_renamed_over_with_the_same_size_and_mtime_is_read_at_once() {
        let lab = StampLab::fresh();
        assert_eq!(lab.stamps.check(), Ok(()));
        let path = lab.dir.join("backup.json");
        let meta = std::fs::metadata(&path).unwrap();
        let tmp = lab.dir.join("backup.json.tmp");
        std::fs::write(&tmp, vec![b'x'; meta.len() as usize]).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&tmp)
            .unwrap()
            .set_modified(meta.modified().unwrap())
            .unwrap();
        std::fs::rename(&tmp, &path).unwrap();
        assert!(
            matches!(lab.stamps.check(), Err(Hold::Unreadable(Stamp::Backup, _))),
            "another inode: read at once, not served from the cache"
        );
    }

    #[test]
    fn a_stamp_of_the_wrong_kind_or_from_the_future_holds() {
        let lab = StampLab::new();
        lab.write(Stamp::Drill, t0(), "ab");
        std::fs::copy(lab.dir.join("drill.json"), lab.dir.join("backup.json")).unwrap();
        let hold = lab.stamps.check().unwrap_err();
        assert_eq!(
            hold.to_string(),
            "the backup stamp (backup.json) cannot be read: kind is \"drill\", not \"backup\""
        );
        lab.write(Stamp::Backup, t0() + chrono::Duration::minutes(30), "ab");
        assert_eq!(
            lab.stamps.check(),
            Ok(()),
            "half an hour of clock skew passes"
        );
        lab.write(Stamp::Backup, t0() + chrono::Duration::hours(3), "ab");
        assert_eq!(
            lab.stamps.check().unwrap_err().to_string(),
            "the backup stamp (backup.json) is dated 3 h ahead of this node's clock"
        );
    }

    #[test]
    fn zero_budgets_and_an_empty_dir_are_refused() {
        assert!(RestoreStamps::from_settings("", 26, 8).is_err());
        assert!(RestoreStamps::from_settings("/x", 0, 8).is_err());
        assert!(RestoreStamps::from_settings("/x", 26, 0).is_err());
        let s = RestoreStamps::from_settings("/x", 26, 8).unwrap();
        assert_eq!(s.0.budgets, [26 * HOUR, 8 * DAY]);
    }

    #[tokio::test]
    async fn scoped_stamps_are_seen_only_inside_their_scope() {
        let lab = StampLab::new();
        assert_eq!(hold(), None, "nothing installed in a test process");
        let inside = scoped(lab.stamps.clone(), async { hold() }).await;
        assert_eq!(inside, Some(Hold::Missing(Stamp::Backup)));
        assert_eq!(hold(), None);
    }

    fn stamp_json(stamp: Stamp, completed_at: DateTime<Utc>) -> serde_json::Value {
        serde_json::json!({
            "kind": stamp.kind(),
            "completed_at": completed_at.to_rfc3339(),
            "snapshot": completed_at.format("%Y%m%dT%H%M%SZ").to_string(),
            "manifest_sha256": "ab",
            "blobs": { "included": true },
        })
    }

    #[test]
    fn a_backup_ages_from_its_snapshot_start_not_its_completion() {
        let lab = StampLab::fresh();
        let started = lab.now() - chrono::Duration::hours(27);
        let mut body = stamp_json(Stamp::Backup, lab.now() - chrono::Duration::hours(1));
        body["snapshot_at"] = serde_json::json!(started.to_rfc3339());
        body["snapshot"] = serde_json::json!(started.format("%Y%m%dT%H%M%SZ").to_string());
        lab.write_json(Stamp::Backup, body);
        assert_eq!(
            lab.stamps.check().unwrap_err().to_string(),
            "the last complete backup (backup.json) is 27 h old, past WRITES_BACKUP_MAX_AGE_HOURS=26",
            "a dump that took 26 h to ship is 27 h behind"
        );
    }

    #[test]
    fn a_drill_of_a_snapshot_far_behind_the_backup_holds_and_names_both() {
        let lab = StampLab::fresh();
        let mut drill = stamp_json(Stamp::Drill, lab.now() - chrono::Duration::hours(1));
        drill["snapshot"] = serde_json::json!("20260923T030000Z");
        lab.write_json(Stamp::Drill, drill);
        let hold = lab.stamps.check().unwrap_err();
        assert_eq!(hold.stamp(), Stamp::Drill);
        assert_eq!(
            hold.to_string(),
            "the last passing restore drill (drill.json) restored snapshot 20260923T030000Z, 8 d 23 h behind the last complete backup's 20261002T020000Z, past WRITES_DRILL_MAX_AGE_DAYS=8"
        );
        let mut drill = stamp_json(Stamp::Drill, lab.now() - chrono::Duration::hours(1));
        drill["snapshot"] = serde_json::json!("20260925T030000Z");
        lab.write_json(Stamp::Drill, drill);
        assert_eq!(
            lab.stamps.check(),
            Ok(()),
            "6 d 23 h behind is within the budget"
        );
        let mut drill = stamp_json(Stamp::Drill, lab.now() - chrono::Duration::hours(1));
        drill["snapshot"] = serde_json::json!("latest");
        lab.write_json(Stamp::Drill, drill);
        assert!(matches!(
            lab.stamps.check(),
            Err(Hold::Unreadable(Stamp::Drill, _))
        ));
    }

    #[test]
    fn a_stamp_without_blobs_holds_only_the_planes_that_store_them() {
        let lab = StampLab::fresh();
        let mut backup = stamp_json(Stamp::Backup, lab.now() - chrono::Duration::hours(1));
        backup["blobs"]["included"] = serde_json::json!(false);
        lab.write_json(Stamp::Backup, backup);
        assert_eq!(lab.stamps.check(), Ok(()));
        let hold = lab.stamps.check_blobs().unwrap_err();
        assert_eq!(
            hold.to_string(),
            "the last complete backup (backup.json) left the blobs out (backup.sh --no-blobs)"
        );
        lab.write_aged(Stamp::Backup, HOUR);
        assert_eq!(lab.stamps.check_blobs(), Ok(()));
        let mut drill = stamp_json(Stamp::Drill, lab.now() - chrono::Duration::hours(1));
        drill.as_object_mut().unwrap().remove("blobs");
        lab.write_json(Stamp::Drill, drill);
        assert_eq!(
            lab.stamps.check_blobs().unwrap_err().to_string(),
            "the last passing restore drill (drill.json) left the blobs out (verify-restore.sh --no-blobs)",
            "a stamp that does not say counts as one without blobs"
        );
    }

    #[test]
    fn ages_print_their_two_largest_units() {
        assert_eq!(human(Duration::from_secs(59)), "0 min");
        assert_eq!(human(Duration::from_secs(12 * 60)), "12 min");
        assert_eq!(human(27 * HOUR), "27 h");
        assert_eq!(human(27 * HOUR + Duration::from_secs(60)), "27 h 1 min");
        assert_eq!(human(8 * DAY), "8 d");
        assert_eq!(
            human(8 * DAY + 3 * HOUR + Duration::from_secs(600)),
            "8 d 3 h"
        );
    }
}
