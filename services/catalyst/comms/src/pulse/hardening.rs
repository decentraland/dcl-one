#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum DisconnectReason {
    Graceful = 1,

    AuthTimeout = 2,

    AuthFailed = 3,

    DuplicateSession = 4,

    Banned = 5,

    ServerFull = 6,

    PreAuthIpLimitExhausted = 7,

    PreAuthBudgetExhausted = 8,

    InputRateExceeded = 9,

    DiscreteEventRateExceeded = 10,

    InvalidInputField = 11,

    InvalidEmoteField = 12,

    InvalidTeleportField = 13,

    HandshakeReplayRejected = 14,

    InvalidHandshakeField = 15,

    PacketCorrupted = 16,

    IpConnectionLimitExceeded = 17,

    SceneListenerIpLimitExceeded = 18,

    InvalidSceneListenerField = 19,
}

impl DisconnectReason {
    pub fn code(self) -> u32 {
        self as u32
    }
}

#[derive(Default)]
pub struct BanList {
    banned: std::collections::HashSet<String>,
}

impl BanList {
    pub fn new() -> Self {
        Self::default()
    }

    fn normalize(wallet: &str) -> String {
        let trimmed = wallet.trim();
        let with_prefix = if trimmed
            .get(..2)
            .is_some_and(|p| p.eq_ignore_ascii_case("0x"))
        {
            trimmed.to_string()
        } else {
            format!("0x{trimmed}")
        };
        with_prefix.to_lowercase()
    }

    pub fn is_banned(&self, wallet: &str) -> bool {
        self.banned.contains(&Self::normalize(wallet))
    }

    pub fn replace<I, S>(&mut self, addresses: I) -> Vec<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let next: std::collections::HashSet<String> = addresses
            .into_iter()
            .map(|a| Self::normalize(a.as_ref()))
            .collect();
        let newly_banned: Vec<String> = next
            .iter()
            .filter(|w| !self.banned.contains(*w))
            .cloned()
            .collect();
        self.banned = next;
        newly_banned
    }
}

/// Source IPs of trusted infrastructure: the operator's one statement that a host is trusted,
/// which exempts it from the scene-listener parcel budget. Entries and queries share one canonical
/// form, so a dotted entry matches a peer the transport reported as v4-mapped IPv6, and IPv6
/// spellings differing only in case or zero-run compression are one entry. A peer whose address
/// is empty or unparseable never matches.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct IpWhitelist {
    entries: std::collections::HashSet<std::net::IpAddr>,
}

impl IpWhitelist {
    pub fn new() -> Self {
        Self::default()
    }

    /// Comma-separated exact addresses; blank is the empty list and blank entries are dropped.
    /// An entry that is not an address (a CIDR, a hostname, a typo) is an error naming it, since
    /// it could never match and would silently exempt nobody.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let mut entries = std::collections::HashSet::new();
        for entry in raw.split(',').map(str::trim).filter(|e| !e.is_empty()) {
            let ip = entry
                .parse::<std::net::IpAddr>()
                .map_err(|e| format!("`{entry}`: {e}"))?;
            entries.insert(ip.to_canonical());
        }
        Ok(Self { entries })
    }

    pub fn contains(&self, ip: &str) -> bool {
        ip.trim()
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| self.entries.contains(&ip.to_canonical()))
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// The entries in the canonical form they match against, sorted for a stable log line.
    pub fn entries(&self) -> Vec<std::net::IpAddr> {
        let mut entries: Vec<_> = self.entries.iter().copied().collect();
        entries.sort_unstable();
        entries
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayRejection {
    Duplicate,
    Forgotten,
    Capacity,
    SourceQuota,
    FutureDated,
    Storage,
}

/// A live `(wallet, timestamp)` pair: its signed time and, when this process admitted it, the
/// sources ([`replay_sources`]) it counts against. Pairs read back from the journal have none.
struct ReplayEntry {
    signed_ms: i64,
    sources: Vec<String>,
}

type ReplayEntries = std::collections::HashMap<(String, String), ReplayEntry>;

/// Live replay entries one source may hold. Each lives at most two freshness windows, so this is
/// far above what the clients behind one address sign, and one source can take only a small part
/// of the table.
pub const DEFAULT_REPLAY_ENTRIES_PER_SOURCE: usize = 256;

/// Prefix of the IPv6 site a peer is also counted against, so addresses spread over the /64s of
/// one allocation share one quota, as the clients behind one IPv4 address do. A provider that
/// places many subscribers in one /48 therefore shares one quota, like an IPv4 carrier NAT.
pub const IPV6_REPLAY_SITE_PREFIX_BITS: u32 = 48;

/// The sources a peer address counts against: its `client_ip::source_key` (the IPv4 address, or
/// the IPv6 /64) and, for IPv6, its /48 site.
fn replay_sources(ip: &str) -> Vec<String> {
    let mut sources = vec![crate::shared::client_ip::source_key(ip)];
    if let Ok(std::net::IpAddr::V6(v6)) = ip.parse::<std::net::IpAddr>()
        && v6.to_ipv4_mapped().is_none()
    {
        let mask = u128::MAX << (128 - IPV6_REPLAY_SITE_PREFIX_BITS);
        let site = std::net::Ipv6Addr::from(u128::from(v6) & mask);
        sources.push(format!("{site}/{IPV6_REPLAY_SITE_PREFIX_BITS}"));
    }
    sources
}

/// One write to the replay journal, in the order the event loop issued it.
enum JournalJob {
    /// One record line.
    Append(String),
    /// The whole journal, replacing the file: the floor and every live pair.
    Compact(String),
}

/// How far the journal writer has got: every job up to `durable_through` (counted from 1 in
/// issue order) is on disk, and `failed` is set once a write failed, after which it writes
/// nothing more.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JournalProgress {
    pub durable_through: u64,
    pub failed: bool,
}

/// The journal file, owned by the writer thread once the journal is open.
struct ReplayJournalFile {
    path: std::path::PathBuf,
    file: std::fs::File,
    _lock: std::fs::File,
}

impl ReplayJournalFile {
    #[cfg(unix)]
    fn sync_parent(path: &std::path::Path) -> std::io::Result<()> {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        std::fs::File::open(parent)?.sync_all()
    }

    #[cfg(not(unix))]
    fn sync_parent(_path: &std::path::Path) -> std::io::Result<()> {
        Ok(())
    }

    fn open(path: impl AsRef<std::path::Path>) -> std::io::Result<(Self, ReplayEntries, i64)> {
        use std::io::BufRead as _;

        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        let lock_path = path.with_extension("replay-journal.lock");
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)?;
        lock.try_lock()?;
        let readable = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)?;
        Self::sync_parent(&path)?;
        let mut seen = std::collections::HashMap::new();
        let mut forgotten_through_ms = i64::MIN;
        for line in std::io::BufReader::new(&readable).lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }
            let fields: Vec<_> = line.split('\t').collect();
            match fields.as_slice() {
                ["F", through] => {
                    let through = through.parse::<i64>().map_err(|_| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "invalid replay-journal floor",
                        )
                    })?;
                    forgotten_through_ms = forgotten_through_ms.max(through);
                }
                ["S", signed_ms, wallet, timestamp]
                    if !wallet.is_empty()
                        && !wallet.contains(char::is_whitespace)
                        && !timestamp.is_empty()
                        && !timestamp.contains(char::is_whitespace) =>
                {
                    let signed_ms = signed_ms.parse::<i64>().map_err(|_| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "invalid replay-journal timestamp",
                        )
                    })?;
                    if timestamp.parse::<i64>().ok() != Some(signed_ms) {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "inconsistent replay-journal timestamp",
                        ));
                    }
                    seen.insert(
                        (wallet.to_string(), timestamp.to_string()),
                        ReplayEntry {
                            signed_ms,
                            sources: Vec::new(),
                        },
                    );
                }
                _ => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "invalid replay-journal record",
                    ));
                }
            }
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        Ok((
            Self {
                path,
                file,
                _lock: lock,
            },
            seen,
            forgotten_through_ms,
        ))
    }

    /// Writes one batch with a single sync. Appends issued before a compaction are already in
    /// the compacted contents, so they are not written separately.
    fn write_batch(&mut self, batch: Vec<JournalJob>) -> std::io::Result<()> {
        use std::io::Write as _;

        let mut appended = String::new();
        for job in batch {
            match job {
                JournalJob::Append(line) => appended.push_str(&line),
                JournalJob::Compact(contents) => {
                    appended.clear();
                    self.compact(&contents)?;
                }
            }
        }
        if !appended.is_empty() {
            self.file.write_all(appended.as_bytes())?;
            self.file.sync_data()?;
        }
        Ok(())
    }

    fn compact(&mut self, contents: &str) -> std::io::Result<()> {
        use std::io::Write as _;

        let temporary = self.path.with_extension("replay-journal.tmp");
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&temporary, &self.path)?;
        Self::sync_parent(&self.path)?;
        self.file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        Ok(())
    }

    /// The writer thread: waits for a job, takes every job queued behind it as one batch, and
    /// writes the batch with one sync, so concurrent handshakes share a sync. `gate`, used by
    /// tests only, must release each batch (an error fails it, a closed gate stops the writer
    /// as a crash would, before the batch is written).
    fn write_jobs(
        mut self,
        jobs: std::sync::mpsc::Receiver<JournalJob>,
        progress: tokio::sync::watch::Sender<JournalProgress>,
        gate: Option<std::sync::mpsc::Receiver<std::io::Result<()>>>,
    ) {
        let mut durable_through = 0u64;
        while let Ok(first) = jobs.recv() {
            let gated = match gate.as_ref().map(std::sync::mpsc::Receiver::recv) {
                None => Ok(()),
                Some(Ok(result)) => result,
                Some(Err(_)) => return,
            };
            let batch: Vec<_> = std::iter::once(first).chain(jobs.try_iter()).collect();
            let issued = batch.len() as u64;
            match gated.and_then(|()| self.write_batch(batch)) {
                Ok(()) => {
                    durable_through += issued;
                    progress.send_replace(JournalProgress {
                        durable_through,
                        failed: false,
                    });
                }
                Err(error) => {
                    tracing::error!(%error, "Pulse replay journal write failed; admission is closed");
                    progress.send_replace(JournalProgress {
                        durable_through,
                        failed: true,
                    });
                    return;
                }
            }
        }
    }
}

fn journal_contents(seen: &ReplayEntries, forgotten_through_ms: i64) -> String {
    use std::fmt::Write as _;

    let mut contents = format!("F\t{forgotten_through_ms}\n");
    let mut entries: Vec<_> = seen.iter().collect();
    entries.sort_by(|left, right| left.0.cmp(right.0));
    for ((wallet, timestamp), entry) in entries {
        let _ = writeln!(contents, "S\t{}\t{wallet}\t{timestamp}", entry.signed_ms);
    }
    contents
}

/// The event loop's side of the journal: it issues jobs without waiting, and a dedicated
/// thread writes them. Dropping it writes every job already issued before it returns.
struct ReplayJournal {
    jobs: Option<std::sync::mpsc::Sender<JournalJob>>,
    progress: tokio::sync::watch::Receiver<JournalProgress>,
    writer: Option<std::thread::JoinHandle<()>>,
    issued: u64,
    records: usize,
}

impl ReplayJournal {
    fn start(
        file: ReplayJournalFile,
        records: usize,
        gate: Option<std::sync::mpsc::Receiver<std::io::Result<()>>>,
    ) -> std::io::Result<Self> {
        let (jobs, queued) = std::sync::mpsc::channel();
        let (sender, progress) = tokio::sync::watch::channel(JournalProgress::default());
        let writer = std::thread::Builder::new()
            .name("pulse-replay-journal".into())
            .spawn(move || file.write_jobs(queued, sender, gate))?;
        Ok(Self {
            jobs: Some(jobs),
            progress,
            writer: Some(writer),
            issued: 0,
            records,
        })
    }

    fn issue(&mut self, job: JournalJob) {
        // A send fails only once the writer has stopped, which `progress` reports as failed.
        if let Some(jobs) = self.jobs.as_ref() {
            let _ = jobs.send(job);
        }
        self.issued += 1;
    }

    fn append_seen(&mut self, wallet: &str, timestamp: &str, signed_ms: i64) {
        self.issue(JournalJob::Append(format!(
            "S\t{signed_ms}\t{wallet}\t{timestamp}\n"
        )));
        self.records = self.records.saturating_add(1);
    }

    fn append_floor(&mut self, through: i64) {
        self.issue(JournalJob::Append(format!("F\t{through}\n")));
        self.records = self.records.saturating_add(1);
    }

    fn compact(&mut self, seen: &ReplayEntries, forgotten_through_ms: i64) {
        self.issue(JournalJob::Compact(journal_contents(
            seen,
            forgotten_through_ms,
        )));
        self.records = seen.len().saturating_add(1);
    }

    fn progress(&self) -> JournalProgress {
        let mut progress = *self.progress.borrow();
        // The writer stops only on a failed write or when this side is dropped.
        progress.failed |= self.progress.has_changed().is_err();
        progress
    }
}

impl Drop for ReplayJournal {
    fn drop(&mut self) {
        self.jobs = None;
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
    }
}

/// Admits each `(wallet, signed timestamp)` at most once. The production constructor journals a
/// valid pair before it is answered, including a clock-ahead pair that is refused for now, so
/// restart cannot turn observed signed bytes into a first use. Journal writes run on a dedicated
/// thread and are group-committed: an admission issues its record and returns, and the caller
/// holds the answer until [`Self::journal_progress`] covers [`Self::journal_issued`] as it was
/// after the admission. A crash before that sync loses only pairs that were never answered. An entry is dropped only after the wall
/// clock has left its freshness window, and every dropped timestamp raises
/// `forgotten_through_ms`, at or below which nothing is admitted. A stepped-back wall clock
/// therefore cannot make a dropped timestamp fresh again, and `forget_before` puts everything
/// signed before this process started under the same floor.
///
/// The table is bounded overall by `max_entries` and per source by `max_entries_per_source`: a
/// source at its quota is refused a new pair, which is then neither recorded nor answered, and
/// no existing entry is dropped early.
pub struct HandshakeReplayPolicy {
    enabled: bool,
    skew_ms: i64,
    max_entries: usize,
    max_entries_per_source: usize,
    forgotten_through_ms: i64,
    seen: ReplayEntries,
    entries_by_source: std::collections::HashMap<String, usize>,
    journal: Option<ReplayJournal>,
}

impl HandshakeReplayPolicy {
    pub fn new(enabled: bool, skew_ms: i64, max_entries: usize) -> Self {
        Self {
            enabled,
            skew_ms,
            max_entries,
            max_entries_per_source: DEFAULT_REPLAY_ENTRIES_PER_SOURCE,
            forgotten_through_ms: i64::MIN,
            seen: std::collections::HashMap::new(),
            entries_by_source: std::collections::HashMap::new(),
            journal: None,
        }
    }

    pub fn durable(
        enabled: bool,
        skew_ms: i64,
        max_entries: usize,
        path: impl AsRef<std::path::Path>,
        now_ms: i64,
    ) -> std::io::Result<Self> {
        Self::durable_with_gate(enabled, skew_ms, max_entries, path, now_ms, None)
    }

    fn durable_with_gate(
        enabled: bool,
        skew_ms: i64,
        max_entries: usize,
        path: impl AsRef<std::path::Path>,
        now_ms: i64,
        gate: Option<std::sync::mpsc::Receiver<std::io::Result<()>>>,
    ) -> std::io::Result<Self> {
        let (mut file, seen, forgotten_through_ms) = ReplayJournalFile::open(path)?;
        let mut policy = Self {
            enabled,
            skew_ms,
            max_entries,
            max_entries_per_source: DEFAULT_REPLAY_ENTRIES_PER_SOURCE,
            forgotten_through_ms,
            seen,
            entries_by_source: std::collections::HashMap::new(),
            journal: None,
        };
        policy.sweep_expired_in_memory(now_ms);
        if policy.seen.len() > max_entries {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "replay journal exceeds configured capacity",
            ));
        }
        file.compact(&journal_contents(&policy.seen, policy.forgotten_through_ms))?;
        policy.journal = Some(ReplayJournal::start(
            file,
            policy.seen.len().saturating_add(1),
            gate,
        )?);
        Ok(policy)
    }

    /// [`Self::durable`] whose writer waits for one message from the returned gate before each
    /// batch: `Ok(())` writes it, an error fails it, and dropping the gate stops the writer
    /// before the batch reaches the file, as a crash would.
    #[cfg(test)]
    pub(crate) fn durable_gated(
        skew_ms: i64,
        max_entries: usize,
        path: impl AsRef<std::path::Path>,
        now_ms: i64,
    ) -> (Self, std::sync::mpsc::Sender<std::io::Result<()>>) {
        let (gate, released) = std::sync::mpsc::channel();
        let policy =
            Self::durable_with_gate(true, skew_ms, max_entries, path, now_ms, Some(released))
                .expect("open the gated replay journal");
        (policy, gate)
    }

    /// [`Self::durable`] as a restart inside one test process: the previous
    /// owner has been dropped, but a sibling test that spawned a child in the
    /// meantime can still hold a copy of its lock descriptor until that child
    /// execs, so `WouldBlock` is retried until that copy is gone.
    #[cfg(test)]
    pub(crate) fn durable_after_restart(
        enabled: bool,
        skew_ms: i64,
        max_entries: usize,
        path: impl AsRef<std::path::Path>,
        now_ms: i64,
    ) -> std::io::Result<Self> {
        let mut outcome = None;
        let _ = crate::shared::test_wait::until_blocking(|| {
            let attempt = Self::durable(enabled, skew_ms, max_entries, path.as_ref(), now_ms);
            let settled = !matches!(&attempt, Err(e) if e.kind() == std::io::ErrorKind::WouldBlock);
            outcome = Some(attempt);
            settled
        });
        outcome.expect("the journal was opened at least once")
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Live entries one source may hold; 0 removes the per-source bound.
    pub fn set_max_entries_per_source(&mut self, max: usize) {
        self.max_entries_per_source = max;
    }

    pub fn forget_before(&mut self, wall_ms: i64) {
        let next = self.forgotten_through_ms.max(wall_ms.saturating_sub(1));
        if next == self.forgotten_through_ms {
            return;
        }
        self.forgotten_through_ms = next;
        if let Some(journal) = self.journal.as_mut() {
            journal.append_floor(next);
        }
    }

    /// Journal jobs issued so far; 0 without a journal.
    pub fn journal_issued(&self) -> u64 {
        self.journal.as_ref().map_or(0, |journal| journal.issued)
    }

    /// How far the journal writer has got. Without a journal nothing is ever pending.
    pub fn journal_progress(&self) -> JournalProgress {
        self.journal
            .as_ref()
            .map_or_else(JournalProgress::default, ReplayJournal::progress)
    }

    /// Resolves when the journal writer reports progress, including its last report after it
    /// stopped; never once that report has been seen, nor without a journal.
    pub async fn journal_progressed(&mut self) {
        match self.journal.as_mut() {
            // `changed` yields an unseen value before it reports the writer gone.
            Some(journal) => {
                if journal.progress.changed().await.is_err() {
                    std::future::pending::<()>().await;
                }
            }
            None => std::future::pending().await,
        }
    }

    /// Records a fresh pair from the peer address `ip` (`None` is not counted per source) and
    /// issues its journal record; see the type's documentation for when it may be answered.
    pub fn try_admit(
        &mut self,
        now_ms: i64,
        ip: Option<&str>,
        wallet: &str,
        timestamp: &str,
    ) -> Result<(), ReplayRejection> {
        if !self.enabled {
            return Ok(());
        }
        if self.journal_progress().failed {
            return Err(ReplayRejection::Storage);
        }
        let signed_ms = match timestamp.parse::<i64>() {
            Ok(signed_ms) if signed_ms > self.forgotten_through_ms => signed_ms,
            _ => return Err(ReplayRejection::Forgotten),
        };
        let key = (wallet.to_lowercase(), timestamp.to_string());
        if self.seen.contains_key(&key) {
            return Err(ReplayRejection::Duplicate);
        }

        let sources = match ip {
            Some(ip) if self.max_entries_per_source > 0 => replay_sources(ip),
            _ => Vec::new(),
        };
        let source_full = |policy: &Self| {
            sources.iter().any(|source| {
                policy.entries_by_source.get(source).copied().unwrap_or(0)
                    >= policy.max_entries_per_source
            })
        };
        if self.seen.len() >= self.max_entries / 2 || source_full(self) {
            self.sweep_expired(now_ms);
        }

        if source_full(self) {
            return Err(ReplayRejection::SourceQuota);
        }
        if self.seen.len() >= self.max_entries {
            return Err(ReplayRejection::Capacity);
        }

        if let Some(journal) = self.journal.as_mut() {
            journal.append_seen(&key.0, &key.1, signed_ms);
        }
        for source in &sources {
            *self.entries_by_source.entry(source.clone()).or_default() += 1;
        }
        self.seen.insert(key, ReplayEntry { signed_ms, sources });
        let compaction_due = self.max_entries.saturating_mul(4).max(4);
        if let Some(journal) = self.journal.as_mut()
            && journal.records > compaction_due
        {
            journal.compact(&self.seen, self.forgotten_through_ms);
        }
        if signed_ms > now_ms {
            Err(ReplayRejection::FutureDated)
        } else {
            Ok(())
        }
    }

    fn sweep_expired(&mut self, now_ms: i64) {
        let old_floor = self.forgotten_through_ms;
        self.sweep_expired_in_memory(now_ms);
        if self.forgotten_through_ms != old_floor
            && let Some(journal) = self.journal.as_mut()
        {
            journal.append_floor(self.forgotten_through_ms);
        }
    }

    fn sweep_expired_in_memory(&mut self, now_ms: i64) {
        let oldest_fresh_ms = now_ms.saturating_sub(self.skew_ms);
        let mut forgotten_through_ms = self.forgotten_through_ms;
        let entries_by_source = &mut self.entries_by_source;
        self.seen.retain(|_, entry| {
            if entry.signed_ms >= oldest_fresh_ms {
                return true;
            }
            forgotten_through_ms = forgotten_through_ms.max(entry.signed_ms);
            for source in &entry.sources {
                if let Some(count) = entries_by_source.get_mut(source) {
                    *count -= 1;
                    if *count == 0 {
                        entries_by_source.remove(source);
                    }
                }
            }
            false
        });
        self.forgotten_through_ms = forgotten_through_ms;
    }
}

pub struct HandshakeAttemptPolicy {
    max_attempts: u8,
}

impl HandshakeAttemptPolicy {
    pub fn new(max_attempts: u8) -> Self {
        Self { max_attempts }
    }

    pub fn is_enabled(&self) -> bool {
        self.max_attempts > 0
    }

    pub fn try_record_attempt(&self, attempts: u8) -> Option<u8> {
        if !self.is_enabled() {
            return Some(attempts);
        }
        if attempts >= self.max_attempts {
            return None;
        }
        Some(attempts + 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmitResult {
    Ok,
    IpLimitExhausted,
    BudgetExhausted,
}

#[derive(Default)]
pub struct PreAuthAdmission {
    per_ip_cap: i64,
    global_budget: i64,
    per_ip_counts: std::collections::HashMap<String, i64>,
    ip_by_pending_peer: std::collections::HashMap<u32, String>,
    in_flight: i64,
}

impl PreAuthAdmission {
    pub fn new(per_ip_cap: i64, global_budget: i64) -> Self {
        Self {
            per_ip_cap,
            global_budget,
            ..Default::default()
        }
    }

    pub fn in_flight(&self) -> i64 {
        self.in_flight
    }

    pub fn try_admit(&mut self, peer_index: u32, ip: &str) -> AdmitResult {
        let per_ip = *self.per_ip_counts.get(ip).unwrap_or(&0);

        if self.per_ip_cap > 0 && per_ip >= self.per_ip_cap {
            return AdmitResult::IpLimitExhausted;
        }
        if self.global_budget > 0 && self.in_flight >= self.global_budget {
            return AdmitResult::BudgetExhausted;
        }

        self.per_ip_counts.insert(ip.to_string(), per_ip + 1);
        self.ip_by_pending_peer.insert(peer_index, ip.to_string());
        self.in_flight += 1;
        AdmitResult::Ok
    }

    pub fn release_on_promotion(&mut self, peer_index: u32) {
        self.release_internal(peer_index);
    }

    pub fn release_on_disconnect(&mut self, peer_index: u32) {
        self.release_internal(peer_index);
    }

    fn release_internal(&mut self, peer_index: u32) {
        let Some(ip) = self.ip_by_pending_peer.remove(&peer_index) else {
            return;
        };
        if let Some(c) = self.per_ip_counts.get_mut(&ip) {
            if *c <= 1 {
                self.per_ip_counts.remove(&ip);
            } else {
                *c -= 1;
            }
        }
        self.in_flight -= 1;
    }
}

pub fn pre_auth_refusal_reason(result: AdmitResult) -> Option<DisconnectReason> {
    match result {
        AdmitResult::Ok => None,
        AdmitResult::IpLimitExhausted => Some(DisconnectReason::PreAuthIpLimitExhausted),
        AdmitResult::BudgetExhausted => Some(DisconnectReason::PreAuthBudgetExhausted),
    }
}

pub const DEFAULT_PRE_AUTH_BUDGET: i64 = 512;

pub const DEFAULT_PRE_AUTH_BUDGET_WT: i64 = 512;

pub const DEFAULT_MAX_CONCURRENT_PRE_AUTH_PER_IP: i64 = 32;

pub const DEFAULT_MAX_HANDSHAKE_ATTEMPTS: u8 = 2;

pub const DEFAULT_MAX_EMOTE_DURATION_MS: u32 = 60_000;

pub const DEFAULT_MAX_REALM_LENGTH: usize = 255;

pub const DEFAULT_SCENE_LISTENER_MAX_PARCELS: usize = 4096;

pub const SCENE_LISTENER_REALM_BUDGET_COST: usize = 4;

/// Largest encoded AoI a whitelisted source IP announces unpriced: upstream's 4096-byte
/// `Transport:BufferSize`, the one ceiling left once the budget is waived (an 8-byte rect names
/// the whole world). ENet here admits far larger frames, so the ceiling is applied to the AoI
/// itself, and a larger announcement is priced like anyone else's.
pub const SCENE_LISTENER_EXEMPT_MAX_AOI_BYTES: usize = 4096;

pub const DEFAULT_CORRUPT_MAX_PER_MINUTE: u32 = 5;

pub const DEFAULT_CORRUPT_BURST: u32 = 5;

pub struct CorruptedPacketLimiter {
    peer_buckets: std::collections::HashMap<u32, CorruptBucket>,
    burst_capacity: u8,
    refill_interval_ms: u32,
}

#[derive(Clone, Copy)]
struct CorruptBucket {
    tokens: u8,
    last_refill_ms: u32,
}

impl CorruptedPacketLimiter {
    pub fn new(max_per_minute: u32, burst_capacity: u32) -> Self {
        Self {
            peer_buckets: std::collections::HashMap::new(),
            burst_capacity: burst_capacity.min(u8::MAX as u32) as u8,
            refill_interval_ms: 60_000u32.checked_div(max_per_minute).unwrap_or(0),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.refill_interval_ms > 0 && self.burst_capacity > 0
    }

    pub fn register_and_check_exhausted(&mut self, peer: u32, now_ms: u32) -> bool {
        if !self.is_enabled() {
            return false;
        }
        let cap = self.burst_capacity;
        let interval = self.refill_interval_ms;
        let mut bucket = match self.peer_buckets.get(&peer) {
            Some(b) if b.last_refill_ms != 0 => {
                let mut b = *b;
                let refills = now_ms.wrapping_sub(b.last_refill_ms) / interval;
                if refills > 0 {
                    b.tokens = (b.tokens as u32).saturating_add(refills).min(cap as u32) as u8;
                    b.last_refill_ms = b.last_refill_ms.wrapping_add(refills * interval);
                }
                b
            }

            _ => CorruptBucket {
                tokens: cap,
                last_refill_ms: if now_ms == 0 { 1 } else { now_ms },
            },
        };
        if bucket.tokens == 0 {
            self.peer_buckets.insert(peer, bucket);
            return true;
        }
        bucket.tokens -= 1;
        self.peer_buckets.insert(peer, bucket);
        false
    }

    pub fn release(&mut self, peer: u32) {
        self.peer_buckets.remove(&peer);
    }
}

pub const DEFAULT_INPUT_MAX_HZ: u32 = 20;

pub const DEFAULT_INPUT_BURST: u32 = 16;

pub const DEFAULT_DISCRETE_RATE_PER_SEC: u32 = 20;

pub const DEFAULT_DISCRETE_BURST: u32 = 16;

#[derive(Clone, Copy, Default)]
struct GameplayBucket {
    tokens: u8,
    last_refill_ms: u32,
}

struct BucketParams {
    burst: u8,
    refill_interval_ms: u32,
}

impl BucketParams {
    fn new(rate_per_sec: u32, burst: u32) -> Self {
        Self {
            burst: burst.min(u8::MAX as u32) as u8,
            refill_interval_ms: 1000u32
                .checked_div(rate_per_sec)
                .map(|v| v.max(1))
                .unwrap_or(0),
        }
    }

    fn is_enabled(&self) -> bool {
        self.refill_interval_ms > 0 && self.burst > 0
    }
}

#[derive(Clone, Copy, Default)]
struct PeerBuckets {
    input: GameplayBucket,
    discrete: GameplayBucket,
}

pub struct GameplayRateLimiter {
    input: BucketParams,
    discrete: BucketParams,
    peers: std::collections::HashMap<u32, PeerBuckets>,
}

impl GameplayRateLimiter {
    pub fn new(
        input_max_hz: u32,
        input_burst: u32,
        discrete_rate_per_sec: u32,
        discrete_burst: u32,
    ) -> Self {
        Self {
            input: BucketParams::new(input_max_hz, input_burst),
            discrete: BucketParams::new(discrete_rate_per_sec, discrete_burst),
            peers: std::collections::HashMap::new(),
        }
    }

    pub fn try_accept_input(&mut self, peer: u32, now_ms: u32) -> bool {
        if !self.input.is_enabled() {
            return true;
        }
        let bucket = &mut self.peers.entry(peer).or_default().input;
        Self::charge(bucket, &self.input, now_ms)
    }

    pub fn try_accept_discrete(&mut self, peer: u32, now_ms: u32) -> bool {
        if !self.discrete.is_enabled() {
            return true;
        }
        let bucket = &mut self.peers.entry(peer).or_default().discrete;
        Self::charge(bucket, &self.discrete, now_ms)
    }

    pub fn release(&mut self, peer: u32) {
        self.peers.remove(&peer);
    }

    fn charge(bucket: &mut GameplayBucket, p: &BucketParams, now_ms: u32) -> bool {
        let cap = p.burst;
        let interval = p.refill_interval_ms;
        if bucket.last_refill_ms == 0 {
            *bucket = GameplayBucket {
                tokens: cap,
                last_refill_ms: if now_ms == 0 { 1 } else { now_ms },
            };
        } else {
            let refills = now_ms.wrapping_sub(bucket.last_refill_ms) / interval;
            if refills > 0 {
                bucket.tokens = (bucket.tokens as u32)
                    .saturating_add(refills)
                    .min(cap as u32) as u8;
                bucket.last_refill_ms = bucket.last_refill_ms.wrapping_add(refills * interval);
            }
        }
        if bucket.tokens == 0 {
            return false;
        }
        bucket.tokens -= 1;
        true
    }
}

#[cfg(test)]
mod limiter_tests {
    use super::{
        CorruptedPacketLimiter, DEFAULT_DISCRETE_BURST, DEFAULT_DISCRETE_RATE_PER_SEC,
        DEFAULT_INPUT_BURST, DEFAULT_INPUT_MAX_HZ, GameplayRateLimiter,
    };

    #[test]
    fn gameplay_defaults_track_upstream_appsettings() {
        assert_eq!(
            (DEFAULT_INPUT_MAX_HZ, DEFAULT_INPUT_BURST),
            (20, 16),
            "upstream appsettings Messaging:Hardening:MovementInput MaxHz / BurstCapacity"
        );
        assert_eq!(
            (DEFAULT_DISCRETE_RATE_PER_SEC, DEFAULT_DISCRETE_BURST),
            (20, 16),
            "upstream appsettings Messaging:Hardening:DiscreteEvent RatePerSecond / \
             BurstCapacity: the registered values, not the 5/10 option-class defaults"
        );
    }

    #[test]
    fn tolerates_burst_then_exhausts() {
        let mut l = CorruptedPacketLimiter::new(5, 5);
        assert!(l.is_enabled());
        for _ in 0..5 {
            assert!(!l.register_and_check_exhausted(1, 1000));
        }
        assert!(l.register_and_check_exhausted(1, 1000));
    }

    #[test]
    fn refills_over_time() {
        let mut l = CorruptedPacketLimiter::new(5, 5);
        for _ in 0..6 {
            l.register_and_check_exhausted(1, 1000);
        }

        assert!(!l.register_and_check_exhausted(1, 13_000));
        assert!(l.register_and_check_exhausted(1, 13_000));
    }

    #[test]
    fn disabled_passes_everything() {
        let mut l = CorruptedPacketLimiter::new(0, 5);
        assert!(!l.is_enabled());
        for _ in 0..100 {
            assert!(!l.register_and_check_exhausted(1, 0));
        }
    }

    #[test]
    fn release_resets_the_bucket() {
        let mut l = CorruptedPacketLimiter::new(5, 5);
        for _ in 0..6 {
            l.register_and_check_exhausted(1, 1000);
        }
        l.release(1);
        assert!(!l.register_and_check_exhausted(1, 1000));
    }

    #[test]
    fn gameplay_release_resets_buckets() {
        let mut l = GameplayRateLimiter::new(20, 16, 5, 10);
        for _ in 0..16 {
            assert!(l.try_accept_input(1, 1000));
        }
        assert!(!l.try_accept_input(1, 1000));
        l.release(1);
        assert!(l.try_accept_input(1, 1000));
    }

    #[test]
    fn gameplay_disabled_passes_everything() {
        let mut l = GameplayRateLimiter::new(0, 16, 0, 10);
        for _ in 0..100 {
            assert!(l.try_accept_input(1, 0));
            assert!(l.try_accept_discrete(1, 0));
        }
    }

    #[test]
    fn gameplay_refills_over_time() {
        let mut l = GameplayRateLimiter::new(20, 16, 5, 10);
        for _ in 0..16 {
            assert!(l.try_accept_input(1, 1000));
        }
        assert!(!l.try_accept_input(1, 1000));
        assert!(l.try_accept_input(1, 1050));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ban_list_normalizes_prefix_and_case() {
        let mut bans = BanList::new();
        bans.replace(["0xABC", "DEF"]);
        assert!(bans.is_banned("0xabc"));
        assert!(bans.is_banned("0xABC"), "case-insensitive");
        assert!(
            bans.is_banned("0xdef"),
            "missing-prefix entry gets 0x prefix"
        );
        assert!(bans.is_banned(" 0xabc \n"), "whitespace trimmed on query");
        assert!(!bans.is_banned("0x999"));
    }

    #[test]
    fn a_ban_entry_starting_with_a_multibyte_character_is_kept_as_is() {
        let mut bans = BanList::new();
        bans.replace(["€abc"]);
        assert!(bans.is_banned("0x€abc"));
        assert!(!bans.is_banned("0xabc"));
    }

    #[test]
    fn ban_list_replace_returns_newly_banned() {
        let mut bans = BanList::new();
        assert_eq!(bans.replace(["0x1"]), vec!["0x1".to_string()]);

        let newly = bans.replace(["0x1", "0x2"]);
        assert_eq!(newly, vec!["0x2".to_string()]);

        assert!(bans.is_banned("0x1"));
        assert!(bans.is_banned("0x2"));

        let newly = bans.replace(["0x2"]);
        assert!(
            newly.is_empty(),
            "0x2 was already banned, nothing newly added"
        );
        assert!(!bans.is_banned("0x1"));
        assert!(bans.is_banned("0x2"));
    }

    #[test]
    fn ip_whitelist_matches_one_canonical_form_across_transport_spellings() {
        let list = IpWhitelist::parse(" 10.0.0.7 , ,::ffff:10.0.0.8,2001:DB8:0:0::1,").unwrap();
        assert_eq!(list.len(), 3, "blank entries are dropped");
        assert!(list.contains("10.0.0.7"));
        assert!(
            list.contains("::ffff:10.0.0.7"),
            "a dotted entry covers a peer reported as v4-mapped IPv6"
        );
        assert!(
            list.contains("10.0.0.8"),
            "a v4-mapped entry covers the dotted peer"
        );
        assert!(
            list.contains("2001:db8::1"),
            "IPv6 case and zero-run compression are one address"
        );
        assert!(!list.contains("10.0.0.9"));
        assert!(!list.contains(""), "an unattributable peer is never exempt");
        assert!(!list.contains("not-an-ip"));
        assert_eq!(
            list.entries(),
            vec![
                "10.0.0.7".parse::<std::net::IpAddr>().unwrap(),
                "10.0.0.8".parse().unwrap(),
                "2001:db8::1".parse().unwrap(),
            ]
        );
    }

    #[test]
    fn ip_whitelist_is_empty_when_blank_and_refuses_what_could_never_match() {
        assert!(IpWhitelist::parse("").unwrap().is_empty());
        assert!(IpWhitelist::parse(" , ").unwrap().is_empty());
        assert!(!IpWhitelist::new().contains("127.0.0.1"));
        for bad in ["10.0.0.0/8", "relay.example", "10.0.0.256", "10.0.0.1:9000"] {
            let err = IpWhitelist::parse(&format!("10.0.0.1, {bad}")).unwrap_err();
            assert!(err.starts_with(&format!("`{bad}`: ")), "{bad}: {err}");
        }
    }

    const SKEW: i64 = crate::pulse::handshake::MAX_TIMESTAMP_SKEW_MS;
    const SIGNED: i64 = 1_700_000_000_000;

    struct ReplayPath(std::path::PathBuf);

    impl ReplayPath {
        fn new(label: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Self(std::env::temp_dir().join(format!(
                "catalyrst-pulse-{label}-{}-{id}.tsv",
                std::process::id()
            )))
        }
    }

    impl Drop for ReplayPath {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
            let _ = std::fs::remove_file(self.0.with_extension("replay-journal.tmp"));
            let _ = std::fs::remove_file(self.0.with_extension("replay-journal.lock"));
        }
    }

    #[test]
    fn replay_first_admit_succeeds_then_dup_rejected() {
        let mut p = HandshakeReplayPolicy::new(true, SKEW, 4096);
        assert_eq!(p.try_admit(SIGNED, None, "0xabc", "1700000000000"), Ok(()));

        assert_eq!(
            p.try_admit(SIGNED, None, "0xabc", "1700000000000"),
            Err(ReplayRejection::Duplicate)
        );

        assert_eq!(
            p.try_admit(SIGNED, None, "0xABC", "1700000000000"),
            Err(ReplayRejection::Duplicate)
        );
    }

    #[test]
    fn replay_same_pair_is_never_admitted_again_after_its_entry_is_dropped() {
        let mut p = HandshakeReplayPolicy::new(true, SKEW, 2);
        let used = SIGNED.to_string();
        assert_eq!(p.try_admit(SIGNED + SKEW, None, "0xabc", &used), Ok(()));

        let later = SIGNED + 2 * SKEW + 1;
        assert_eq!(
            p.try_admit(later, None, "0xdef", &later.to_string()),
            Ok(())
        );
        assert_eq!(p.seen.len(), 1, "the expired entry was dropped");
        for stepped_back in [later, SIGNED + SKEW, SIGNED, SIGNED - SKEW] {
            assert_eq!(
                p.try_admit(stepped_back, None, "0xabc", &used),
                Err(ReplayRejection::Forgotten)
            );
        }
    }

    #[test]
    fn replay_floor_never_reaches_a_timestamp_inside_the_freshness_window() {
        let mut p = HandshakeReplayPolicy::new(true, SKEW, 2);
        let future_dated = (SIGNED + SKEW).to_string();
        assert_eq!(
            p.try_admit(SIGNED, None, "0xabc", &future_dated),
            Err(ReplayRejection::FutureDated)
        );
        let later = SIGNED + 2 * SKEW + 1;
        assert_eq!(p.try_admit(later, None, "0x1", &later.to_string()), Ok(()));
        assert_eq!(p.seen.len(), 1);
        let slowest_fresh = (later - SKEW).to_string();
        assert_eq!(p.try_admit(later, None, "0x2", &slowest_fresh), Ok(()));
    }

    #[test]
    fn replay_forget_before_refuses_everything_signed_before_the_process_started() {
        let mut p = HandshakeReplayPolicy::new(true, SKEW, 4096);
        p.forget_before(SIGNED);
        assert_eq!(
            p.try_admit(SIGNED, None, "0xabc", &(SIGNED - 1).to_string()),
            Err(ReplayRejection::Forgotten)
        );
        assert_eq!(
            p.try_admit(SIGNED, None, "0xabc", &SIGNED.to_string()),
            Ok(())
        );
        p.forget_before(SIGNED - SKEW);
        assert_eq!(
            p.try_admit(SIGNED, None, "0xdef", &(SIGNED - 1).to_string()),
            Err(ReplayRejection::Forgotten),
            "the floor never moves back"
        );
        assert_eq!(
            p.try_admit(SIGNED, None, "0xdef", "not-a-number"),
            Err(ReplayRejection::Forgotten)
        );
    }

    #[test]
    fn replay_different_timestamp_or_wallet_accepted() {
        let mut p = HandshakeReplayPolicy::new(true, SKEW, 4096);
        assert_eq!(
            p.try_admit(SIGNED + 2, None, "0xabc", "1700000000001"),
            Ok(())
        );
        assert!(
            p.try_admit(SIGNED + 2, None, "0xabc", "1700000000002")
                .is_ok(),
            "fresh timestamp = legit reconnect"
        );
        assert!(
            p.try_admit(SIGNED + 2, None, "0xdef", "1700000000001")
                .is_ok(),
            "different wallet, same ts"
        );
    }

    #[test]
    fn replay_disabled_admits_everything() {
        let mut p = HandshakeReplayPolicy::new(false, SKEW, 4096);
        for _ in 0..100 {
            assert_eq!(p.try_admit(SIGNED, None, "0xabc", "ts"), Ok(()));
        }
    }

    #[test]
    fn replay_capacity_rejects_new_admission_without_forgetting_live_entries() {
        let mut p = HandshakeReplayPolicy::new(true, SKEW, 2);
        assert_eq!(
            p.try_admit(SIGNED + 3, None, "0x1", "1700000000001"),
            Ok(())
        );
        assert_eq!(
            p.try_admit(SIGNED + 3, None, "0x2", "1700000000002"),
            Ok(())
        );
        assert_eq!(
            p.try_admit(SIGNED + 3, None, "0x3", "1700000000003"),
            Err(ReplayRejection::Capacity)
        );
        assert_eq!(
            p.try_admit(SIGNED + 1, None, "0x1", "1700000000001"),
            Err(ReplayRejection::Duplicate)
        );
        let later = SIGNED + SKEW + 3;
        assert_eq!(p.try_admit(later, None, "0x3", &later.to_string()), Ok(()));
        assert_eq!(p.seen.len(), 1);
    }

    #[test]
    fn replay_retention_includes_entire_freshness_window() {
        let mut p = HandshakeReplayPolicy::new(true, SKEW, 1);
        let future_dated = (SIGNED + SKEW).to_string();
        assert_eq!(
            p.try_admit(SIGNED, None, "wallet", &future_dated),
            Err(ReplayRejection::FutureDated)
        );
        for delta in [7_000, 31_000, 60_000, 120_000] {
            assert_eq!(
                p.try_admit(SIGNED + delta, None, "wallet", &future_dated),
                Err(ReplayRejection::Duplicate)
            );
            assert_eq!(
                p.try_admit(SIGNED + delta, None, "other", &(SIGNED + delta).to_string()),
                Err(ReplayRejection::Capacity)
            );
        }
        let later = SIGNED + 2 * SKEW + 1;
        assert_eq!(
            p.try_admit(later, None, "other", &later.to_string()),
            Ok(())
        );
    }

    #[test]
    fn durable_replay_journal_consumes_future_dated_bytes_across_restart() {
        let path = ReplayPath::new("future-restart");
        let future = SIGNED + SKEW;
        {
            let mut first = HandshakeReplayPolicy::durable(true, SKEW, 4, &path.0, SIGNED).unwrap();
            assert_eq!(
                first.try_admit(SIGNED, None, "0xabc", &future.to_string()),
                Err(ReplayRejection::FutureDated)
            );
        }
        let mut restarted =
            HandshakeReplayPolicy::durable_after_restart(true, SKEW, 4, &path.0, SIGNED + 1)
                .unwrap();
        restarted.forget_before(SIGNED + 1);
        assert_eq!(
            restarted.try_admit(future, None, "0xabc", &future.to_string()),
            Err(ReplayRejection::Duplicate)
        );
    }

    #[test]
    fn malformed_replay_journal_fails_startup_closed() {
        let path = ReplayPath::new("malformed");
        std::fs::write(&path.0, b"not a replay record\n").unwrap();
        assert!(HandshakeReplayPolicy::durable(true, SKEW, 4, &path.0, SIGNED).is_err());
    }

    #[test]
    fn durable_replay_journal_has_exactly_one_writer() {
        let path = ReplayPath::new("single-writer");
        let first = HandshakeReplayPolicy::durable(true, SKEW, 4, &path.0, SIGNED).unwrap();
        assert!(
            HandshakeReplayPolicy::durable(true, SKEW, 4, &path.0, SIGNED).is_err(),
            "two processes must not race appends or compaction through one journal path"
        );
        drop(first);
        assert!(
            HandshakeReplayPolicy::durable_after_restart(true, SKEW, 4, &path.0, SIGNED).is_ok(),
            "the next process must acquire the journal after the owner exits"
        );
    }

    async fn settle(
        policy: &mut HandshakeReplayPolicy,
        done: impl Fn(JournalProgress) -> bool,
    ) -> JournalProgress {
        tokio::time::timeout(crate::shared::test_wait::HANG_GUARD, async {
            loop {
                let progress = policy.journal_progress();
                if done(progress) {
                    return progress;
                }
                policy.journal_progressed().await;
            }
        })
        .await
        .expect("the journal writer reports progress")
    }

    #[tokio::test]
    async fn baseline_input_boundaries_durable_storage_failure_latches_closed() {
        let path = ReplayPath::new("storage-failure");
        let (mut policy, gate) = HandshakeReplayPolicy::durable_gated(SKEW, 4, &path.0, SIGNED);
        assert_eq!(
            policy.try_admit(SIGNED, None, "wallet", &SIGNED.to_string()),
            Ok(())
        );
        let issued = policy.journal_issued();
        gate.send(Err(std::io::Error::other("injected write failure")))
            .unwrap();
        let progress = settle(&mut policy, |progress| progress.failed).await;
        assert!(
            progress.durable_through < issued,
            "failed persistence cannot grant admission"
        );
        // The failure is latched: nothing reopens admission inside the failed process.
        let _ = gate.send(Ok(()));
        assert_eq!(
            policy.try_admit(SIGNED + 1, None, "other", &(SIGNED + 1).to_string()),
            Err(ReplayRejection::Storage)
        );
    }

    #[tokio::test]
    async fn a_writer_that_failed_and_stopped_still_wakes_the_event_loop() {
        let path = ReplayPath::new("failed-wakes");
        let (mut policy, gate) = HandshakeReplayPolicy::durable_gated(SKEW, 4, &path.0, SIGNED);
        assert_eq!(
            policy.try_admit(SIGNED, None, "wallet", &SIGNED.to_string()),
            Ok(())
        );
        gate.send(Err(std::io::Error::other("injected write failure")))
            .unwrap();
        // The writer reports the failure and exits before the event loop looks.
        assert!(
            crate::shared::test_wait::until(async || {
                policy
                    .journal
                    .as_ref()
                    .is_some_and(|journal| journal.progress.has_changed().is_err())
            })
            .await
        );
        tokio::time::timeout(
            crate::shared::test_wait::HANG_GUARD,
            policy.journal_progressed(),
        )
        .await
        .expect("the failure wakes the event loop so held handshakes are refused");
        assert!(policy.journal_progress().failed);
    }

    #[tokio::test]
    async fn journal_group_commits_every_record_queued_behind_one_sync() {
        let path = ReplayPath::new("group-commit");
        let (mut policy, gate) = HandshakeReplayPolicy::durable_gated(SKEW, 16, &path.0, SIGNED);
        let ts = SIGNED.to_string();
        for wallet in ["0x1", "0x2", "0x3"] {
            assert_eq!(policy.try_admit(SIGNED, None, wallet, &ts), Ok(()));
        }
        let issued = policy.journal_issued();
        assert_eq!(issued, 3);
        assert_eq!(
            policy.journal_progress(),
            JournalProgress::default(),
            "nothing is durable before the sync"
        );
        // The writer needs one release per batch: one release makes all three durable.
        gate.send(Ok(())).unwrap();
        let progress = settle(&mut policy, |progress| progress.durable_through >= issued).await;
        assert_eq!(
            progress,
            JournalProgress {
                durable_through: issued,
                failed: false
            }
        );
        let journal = std::fs::read_to_string(&path.0).unwrap();
        for wallet in ["0x1", "0x2", "0x3"] {
            assert!(
                journal.contains(&format!("\t{wallet}\t{ts}\n")),
                "{journal}"
            );
        }
    }

    #[tokio::test]
    async fn a_crash_before_the_sync_loses_only_unanswered_pairs() {
        let path = ReplayPath::new("crash");
        let ts = SIGNED.to_string();
        {
            let (mut policy, gate) =
                HandshakeReplayPolicy::durable_gated(SKEW, 16, &path.0, SIGNED);
            assert_eq!(policy.try_admit(SIGNED, None, "0xanswered", &ts), Ok(()));
            let answerable = policy.journal_issued();
            gate.send(Ok(())).unwrap();
            settle(&mut policy, |progress| {
                progress.durable_through >= answerable
            })
            .await;

            assert_eq!(policy.try_admit(SIGNED, None, "0xunanswered", &ts), Ok(()));
            assert!(
                policy.journal_progress().durable_through < policy.journal_issued(),
                "its answer is still held"
            );
            // Crash: the writer stops before the batch reaches the file.
            drop(gate);
        }
        let mut restarted =
            HandshakeReplayPolicy::durable_after_restart(true, SKEW, 16, &path.0, SIGNED).unwrap();
        assert_eq!(
            restarted.try_admit(SIGNED, None, "0xanswered", &ts),
            Err(ReplayRejection::Duplicate),
            "a pair that could have been answered is never admitted again"
        );
        assert_eq!(
            restarted.try_admit(SIGNED, None, "0xunanswered", &ts),
            Ok(()),
            "a pair that was never answered may be lost"
        );
    }

    #[test]
    fn zero_capacity_fails_closed() {
        let mut p = HandshakeReplayPolicy::new(true, SKEW, 0);
        assert_eq!(
            p.try_admit(SIGNED, None, "wallet", "1700000000000"),
            Err(ReplayRejection::Capacity)
        );
        assert!(p.seen.is_empty());
    }

    #[test]
    fn source_quota_refuses_only_that_source_and_keeps_its_entries() {
        let mut p = HandshakeReplayPolicy::new(true, SKEW, 4096);
        p.set_max_entries_per_source(2);
        let ts = SIGNED.to_string();
        assert_eq!(p.try_admit(SIGNED, Some("192.0.2.1"), "0x1", &ts), Ok(()));
        assert_eq!(p.try_admit(SIGNED, Some("192.0.2.1"), "0x2", &ts), Ok(()));
        assert_eq!(
            p.try_admit(SIGNED, Some("192.0.2.1"), "0x3", &ts),
            Err(ReplayRejection::SourceQuota)
        );
        // The refused pair was not recorded, and the source's entries were not dropped.
        assert_eq!(p.seen.len(), 2);
        assert_eq!(
            p.try_admit(SIGNED, Some("192.0.2.1"), "0x1", &ts),
            Err(ReplayRejection::Duplicate)
        );
        // Another source, and a pair with no source, still have room.
        assert_eq!(p.try_admit(SIGNED, Some("192.0.2.2"), "0x3", &ts), Ok(()));
        assert_eq!(p.try_admit(SIGNED, None, "0x4", &ts), Ok(()));
        // Once the source's entries leave the freshness window, its quota is free again.
        let later = SIGNED + SKEW + 1;
        assert_eq!(
            p.try_admit(later, Some("192.0.2.1"), "0x5", &later.to_string()),
            Ok(())
        );
        assert_eq!(p.entries_by_source.get("192.0.2.1"), Some(&1));
        assert_eq!(
            p.try_admit(later, Some("192.0.2.1"), "0x1", &ts),
            Err(ReplayRejection::Forgotten)
        );
    }

    #[test]
    fn ipv6_peers_share_the_quota_of_their_slash_48_site() {
        let mut p = HandshakeReplayPolicy::new(true, SKEW, 4096);
        p.set_max_entries_per_source(2);
        let ts = SIGNED.to_string();
        // Two /64s of one /48 site take its whole quota.
        assert_eq!(
            p.try_admit(SIGNED, Some("2001:db8:1:1::1"), "0x1", &ts),
            Ok(())
        );
        assert_eq!(
            p.try_admit(SIGNED, Some("2001:db8:1:2::1"), "0x2", &ts),
            Ok(())
        );
        assert_eq!(
            p.try_admit(SIGNED, Some("2001:db8:1:3::1"), "0x3", &ts),
            Err(ReplayRejection::SourceQuota)
        );
        // Another /48 has its own quota, and IPv4 (also v4-mapped) is counted by address only.
        assert_eq!(
            p.try_admit(SIGNED, Some("2001:db8:2:1::1"), "0x3", &ts),
            Ok(())
        );
        assert_eq!(
            p.try_admit(SIGNED, Some("::ffff:192.0.2.1"), "0x4", &ts),
            Ok(())
        );
        assert_eq!(p.entries_by_source.get("192.0.2.1"), Some(&1));
        // Expiry releases both the /64 and the /48 counts.
        let later = SIGNED + SKEW + 1;
        assert_eq!(
            p.try_admit(later, Some("2001:db8:1:3::1"), "0x5", &later.to_string()),
            Ok(())
        );
        assert_eq!(p.entries_by_source.get("2001:db8:1::/48"), Some(&1));
        assert_eq!(p.entries_by_source.get("2001:db8:1:1::/64"), None);
    }

    #[test]
    fn source_quota_refusal_journals_nothing_and_admitted_pairs_survive_restart() {
        let path = ReplayPath::new("source-quota");
        let ts = SIGNED.to_string();
        {
            let mut first = HandshakeReplayPolicy::durable(true, SKEW, 4, &path.0, SIGNED).unwrap();
            first.set_max_entries_per_source(1);
            assert_eq!(
                first.try_admit(SIGNED, Some("192.0.2.1"), "0x1", &ts),
                Ok(())
            );
            assert_eq!(
                first.try_admit(SIGNED, Some("192.0.2.1"), "0x2", &ts),
                Err(ReplayRejection::SourceQuota)
            );
        }
        let mut restarted =
            HandshakeReplayPolicy::durable_after_restart(true, SKEW, 4, &path.0, SIGNED + 1)
                .unwrap();
        restarted.set_max_entries_per_source(1);
        assert_eq!(
            restarted.try_admit(SIGNED + 1, Some("192.0.2.2"), "0x1", &ts),
            Err(ReplayRejection::Duplicate)
        );
        // Restored pairs carry no source, so they do not count against one after restart.
        assert_eq!(
            restarted.try_admit(SIGNED + 1, Some("192.0.2.1"), "0x2", &ts),
            Ok(())
        );
    }

    #[test]
    fn attempt_throttle_rejects_after_max() {
        let p = HandshakeAttemptPolicy::new(2);

        assert_eq!(p.try_record_attempt(0), Some(1));
        assert_eq!(p.try_record_attempt(1), Some(2));
        assert_eq!(p.try_record_attempt(2), None);
    }

    #[test]
    fn attempt_throttle_disabled_when_max_zero() {
        let p = HandshakeAttemptPolicy::new(0);
        assert_eq!(p.try_record_attempt(255), Some(255));
    }

    #[test]
    fn pre_auth_budget_caps_global_in_flight() {
        let mut a = PreAuthAdmission::new(0, 2);
        assert_eq!(a.try_admit(1, "1.1.1.1"), AdmitResult::Ok);
        assert_eq!(a.try_admit(2, "2.2.2.2"), AdmitResult::Ok);
        assert_eq!(a.try_admit(3, "3.3.3.3"), AdmitResult::BudgetExhausted);
        assert_eq!(a.in_flight(), 2);

        a.release_on_promotion(1);
        assert_eq!(a.in_flight(), 1);
        assert_eq!(a.try_admit(3, "3.3.3.3"), AdmitResult::Ok);
    }

    #[test]
    fn pre_auth_per_ip_cap_isolates_one_ip() {
        let mut a = PreAuthAdmission::new(2, 0);
        assert_eq!(a.try_admit(1, "1.1.1.1"), AdmitResult::Ok);
        assert_eq!(a.try_admit(2, "1.1.1.1"), AdmitResult::Ok);
        assert_eq!(a.try_admit(3, "1.1.1.1"), AdmitResult::IpLimitExhausted);

        assert_eq!(a.try_admit(4, "2.2.2.2"), AdmitResult::Ok);

        a.release_on_disconnect(1);
        assert_eq!(a.try_admit(3, "1.1.1.1"), AdmitResult::Ok);
    }

    #[test]
    fn pre_auth_release_is_idempotent() {
        let mut a = PreAuthAdmission::new(0, 4);
        a.try_admit(1, "1.1.1.1");
        a.release_on_promotion(1);

        a.release_on_disconnect(1);
        assert_eq!(a.in_flight(), 0);
    }

    #[test]
    fn pre_auth_refusal_reason_maps_codes() {
        assert_eq!(pre_auth_refusal_reason(AdmitResult::Ok), None);
        assert_eq!(
            pre_auth_refusal_reason(AdmitResult::IpLimitExhausted),
            Some(DisconnectReason::PreAuthIpLimitExhausted)
        );
        assert_eq!(
            pre_auth_refusal_reason(AdmitResult::BudgetExhausted),
            Some(DisconnectReason::PreAuthBudgetExhausted)
        );
    }

    #[test]
    fn disconnect_reason_codes_match_upstream() {
        assert_eq!(DisconnectReason::AuthTimeout.code(), 2);
        assert_eq!(DisconnectReason::DuplicateSession.code(), 4);
        assert_eq!(DisconnectReason::Banned.code(), 5);
        assert_eq!(DisconnectReason::HandshakeReplayRejected.code(), 14);
        assert_eq!(DisconnectReason::InvalidEmoteField.code(), 12);
        assert_eq!(DisconnectReason::InvalidHandshakeField.code(), 15);
        assert_eq!(DisconnectReason::InvalidSceneListenerField.code(), 19);
    }
}
