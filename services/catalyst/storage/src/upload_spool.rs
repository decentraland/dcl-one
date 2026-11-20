//! Multipart file parts spooled to disk instead of memory: one directory per
//! request under a route's spool root (by default beside the blob store, in
//! [`Storage::scratch_dir`](crate::Storage::scratch_dir), so storing a part
//! is a hard link), removed when the request ends however it ends.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use tokio::io::AsyncWriteExt;

use dcl_auth::HashV1Writer;

const WRITE_BUFFER_BYTES: usize = 256 * 1024;

/// Spool mappings live at once in the process, an eighth of Linux's
/// default `vm.max_map_count` (65530), which the allocator and every other
/// mapping share.
pub const MAX_LIVE_MAPS: usize = 8192;

static LIVE_MAPS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// One of the [`MAX_LIVE_MAPS`], given back on drop.
struct MapSlot;

impl MapSlot {
    fn take() -> std::io::Result<Self> {
        use std::sync::atomic::Ordering;
        LIVE_MAPS
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < MAX_LIVE_MAPS).then_some(n + 1)
            })
            .map(|_| MapSlot)
            .map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    "too many spooled files are mapped at once",
                )
            })
    }
}

impl Drop for MapSlot {
    fn drop(&mut self) {
        LIVE_MAPS.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    }
}

/// A mapping holding its slot.
struct CountedMap {
    map: memmap2::Mmap,
    _slot: MapSlot,
}

impl AsRef<[u8]> for CountedMap {
    fn as_ref(&self) -> &[u8] {
        &self.map
    }
}

/// One request's spool directory; dropping it removes the directory and every part in it.
pub struct UploadSpool {
    dir: PathBuf,
    next: usize,
    /// The route label of the cleanup-failure counter.
    route: &'static str,
}

/// A spooled file part: its form name, the CIDv1 of its bytes, size and path inside the spool.
#[derive(Debug, Clone)]
pub struct SpooledFile {
    pub name: String,
    pub hash: String,
    pub len: usize,
    pub path: PathBuf,
}

impl SpooledFile {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub async fn read(&self) -> std::io::Result<Vec<u8>> {
        tokio::fs::read(&self.path).await
    }

    /// The part's bytes, memory-mapped: read through the page cache on
    /// demand, never copied onto the heap. The mapping keeps the bytes
    /// readable after the spool directory is removed (the unlinked file
    /// lives until the last mapping goes). At most [`MAX_LIVE_MAPS`]
    /// mappings live at once in the process; past it this fails with
    /// `WouldBlock`, so the kernel's per-process map limit is never reached
    /// whatever the budgets allow.
    pub async fn map(&self) -> std::io::Result<bytes::Bytes> {
        if self.len == 0 {
            return Ok(bytes::Bytes::new());
        }
        let slot = MapSlot::take()?;
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || {
            let file = std::fs::File::open(&path)?;
            // SAFETY: the file is this request's own spool part, in a
            // directory only its `UploadSpool` writes; it was written and
            // flushed before this call and is never written, truncated or
            // replaced afterwards, only unlinked, which keeps the mapped
            // inode intact.
            let map = unsafe { memmap2::Mmap::map(&file)? };
            Ok(bytes::Bytes::from_owner(CountedMap { map, _slot: slot }))
        })
        .await
        .map_err(std::io::Error::other)?
    }
}

impl UploadSpool {
    pub async fn create(root: &Path, route: &'static str) -> std::io::Result<Self> {
        let dir = root.join(format!(
            "{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        tokio::fs::create_dir_all(&dir).await?;
        Ok(Self {
            dir,
            next: 0,
            route,
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// A new part file, written and hashed chunk by chunk through [`SpoolWriter`].
    pub async fn part(&mut self) -> std::io::Result<SpoolWriter> {
        let path = self.dir.join(self.next.to_string());
        self.next += 1;
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await?;
        Ok(SpoolWriter {
            file: tokio::io::BufWriter::with_capacity(WRITE_BUFFER_BYTES, file),
            hasher: HashV1Writer::new(),
            len: 0,
            path,
        })
    }
}

impl Drop for UploadSpool {
    fn drop(&mut self) {
        let dir = std::mem::take(&mut self.dir);
        let route = self.route;
        let remove = move || {
            if let Err(e) = std::fs::remove_dir_all(&dir)
                && e.kind() != std::io::ErrorKind::NotFound
            {
                metrics::counter!("multipart_upload_cleanup_failures", "route" => route)
                    .increment(1);
                tracing::warn!(dir = %dir.display(), error = %e, "could not remove an upload spool directory");
            }
        };
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                handle.spawn_blocking(remove);
            }
            Err(_) => remove(),
        }
    }
}

pub struct SpoolWriter {
    file: tokio::io::BufWriter<tokio::fs::File>,
    hasher: HashV1Writer,
    len: usize,
    path: PathBuf,
}

impl SpoolWriter {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub async fn write(&mut self, chunk: &[u8]) -> std::io::Result<()> {
        self.hasher.update(chunk);
        self.len += chunk.len();
        self.file.write_all(chunk).await
    }

    /// The written part, filed under its form `name`.
    pub async fn finish(mut self, name: String) -> std::io::Result<SpooledFile> {
        self.file.flush().await?;
        Ok(SpooledFile {
            name,
            hash: self.hasher.finish(),
            len: self.len,
            path: self.path,
        })
    }
}

/// Whether `name` is one [`UploadSpool::create`] makes (`<pid>-<uuid>`): the
/// sweep reclaims only those, so a spool root shared with anything else
/// (an operator's `UPLOAD_SPOOL_FOLDER`) keeps the rest.
fn is_spool_dir_name(name: &str) -> bool {
    name.split_once('-').is_some_and(|(pid, id)| {
        !pid.is_empty()
            && pid.bytes().all(|b| b.is_ascii_digit())
            && id.len() == 32
            && id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// Removes spool directories a crashed process left behind, once older than
/// `min_age`; only the ones [`UploadSpool::create`] names.
pub async fn sweep_stale(root: &Path, min_age: Duration) -> usize {
    let Ok(mut entries) = tokio::fs::read_dir(root).await else {
        return 0;
    };
    let now = SystemTime::now();
    let mut swept = 0;
    while let Ok(Some(entry)) = entries.next_entry().await {
        if !entry.file_name().to_str().is_some_and(is_spool_dir_name) {
            continue;
        }
        let stale = entry
            .metadata()
            .await
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|m| now.duration_since(m).ok())
            .is_some_and(|age| age >= min_age);
        if stale && tokio::fs::remove_dir_all(entry.path()).await.is_ok() {
            swept += 1;
        }
    }
    if swept > 0 {
        tracing::info!(swept, root = %root.display(), "removed stale upload spool directories");
    }
    swept
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn wait_gone(dir: &Path) {
        for _ in 0..200 {
            if !dir.exists() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("{} was not removed", dir.display());
    }

    #[tokio::test]
    async fn parts_hash_like_the_buffered_path_and_the_directory_goes_on_drop() {
        let root = std::env::temp_dir().join(format!("catalyst-spool-{}", uuid::Uuid::new_v4()));
        let mut spool = UploadSpool::create(&root, "test").await.unwrap();
        let dir = spool.dir().to_path_buf();
        let mut part = spool.part().await.unwrap();
        let bytes: Vec<u8> = (0..600_000u32).map(|i| (i % 251) as u8).collect();
        for chunk in bytes.chunks(65_536) {
            part.write(chunk).await.unwrap();
        }
        let file = part.finish("f".to_string()).await.unwrap();
        assert_eq!(file.hash, dcl_auth::hash_bytes_v1(&bytes));
        assert_eq!(file.len(), bytes.len());
        assert_eq!(file.read().await.unwrap(), bytes);
        let mapped = file.map().await.unwrap();
        assert_eq!(&mapped[..], &bytes[..]);
        drop(spool);
        wait_gone(&dir).await;
        assert_eq!(&mapped[..], &bytes[..], "a mapping outlives the spool");
        let live = LIVE_MAPS.load(std::sync::atomic::Ordering::Acquire);
        drop(mapped);
        assert_eq!(
            LIVE_MAPS.load(std::sync::atomic::Ordering::Acquire),
            live - 1,
            "a dropped mapping gives its slot back"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn the_sweep_removes_only_aged_spool_directories() {
        let root = std::env::temp_dir().join(format!("catalyst-spool-{}", uuid::Uuid::new_v4()));
        let name = |n: u32| format!("{n}-{}", uuid::Uuid::new_v4().simple());
        let old = root.join(name(1));
        let fresh = root.join(name(2));
        let foreign = root.join("operator-data");
        for dir in [&old, &fresh, &foreign] {
            std::fs::create_dir_all(dir).unwrap();
        }
        for dir in [&old, &foreign] {
            std::fs::File::open(dir)
                .unwrap()
                .set_modified(SystemTime::now() - Duration::from_secs(7200))
                .unwrap();
        }
        assert_eq!(sweep_stale(&root, Duration::from_secs(3600)).await, 1);
        assert!(!old.exists() && fresh.exists());
        assert!(
            foreign.exists(),
            "a directory the spool did not make is kept"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
