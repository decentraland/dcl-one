use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use bytes::Bytes;
use tracing::{debug, warn};

use super::Denylist;

use super::{
    FileIds, KnownShards, StorageError, create_staging_file, ensure_file_path, open_for_read,
    resolve_file_path, staging_path, stat_for_read,
};

static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Read surface contract: `Ok(None)` means provably absent, `Err` a storage fault -- a fault reported
/// as absence makes a broken node advertise itself as empty to peers.
///
/// One instance per root, shared behind an `Arc`, never one per consumer: the instance carries the
/// record of which shard directories it has created or observed ([`KnownShards`]), and that is what
/// tells a destroyed shard from one that never existed. Two instances over the same root disagree
/// about the same damage -- the writer faults, a read-only instance that created nothing reports 404.
///
/// Moderation is the store's, not each route's: every read that hands out bytes or their metadata
/// ([`retrieve`](Self::retrieve), [`retrieve_of`](Self::retrieve_of), [`open`](Self::open),
/// [`read_range`](Self::read_range), [`file_info`](Self::file_info), [`file_path`](Self::file_path))
/// consults the store's [`Denylist`] first and answers a listed hash as a miss, so a route that
/// learns a hash from any table (a camera-reel row, a community thumbnail, a builder upload, a
/// profile's content files) cannot serve it by forgetting the check. The presence probes
/// ([`exist`](Self::exist), [`exist_multiple`](Self::exist_multiple)), the enumeration and the
/// write side see the file: sync and the collector keep their bookkeeping, and that a blob is held
/// is not what the operator withholds. The projection is one per store ([`denylist`](Self::denylist)),
/// which the content state, the builder and the camera reel share.
pub struct Storage {
    root: PathBuf,
    known_shards: KnownShards,
    kind: &'static str,
    denylist: Arc<Denylist>,
}

impl Storage {
    /// `root` is `STORAGE_ROOT_FOLDER`; blobs live under `<root>/contents/<shard>/<cid>`, the tree
    /// the nginx `X-Accel-Redirect` alias serves from. Creates the tree and kicks off the orphaned
    /// staging-file sweep in the background.
    pub async fn from_config(root: &Path) -> Result<Self, StorageError> {
        let (root, kind) = (root.join("contents"), "content");
        tokio::fs::create_dir_all(&root).await?;
        let sweep_root = root.clone();
        tokio::spawn(async move {
            super::sweep_stale_staging(&sweep_root, kind).await;
        });
        debug!(kind, root = %root.display(), "storage initialized");
        Ok(Self {
            known_shards: KnownShards::new(root.clone()),
            root,
            kind,
            denylist: Arc::new(Denylist::default()),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The moderation projection every byte read consults: the operator's `content.denylist` and
    /// the camera reel's withheld blobs (`content::Denylist`). The content state loads and reloads
    /// it, the reel adds its verdicts; hand this `Arc` on rather than a second projection.
    pub fn denylist(&self) -> &Arc<Denylist> {
        &self.denylist
    }

    /// Whether the store withholds `id` (a hash or an entity id): the byte reads below answer a
    /// listed hash as absent, and a route that also knows the entity refuses on it too.
    pub fn withholds(&self, id: &str) -> bool {
        self.denylist.contains(id)
    }

    pub async fn store(&self, hash: &str, data: Bytes) -> Result<(), StorageError> {
        use tokio::io::AsyncWriteExt;

        let path = ensure_file_path(&self.root, hash, &self.known_shards).await?;

        let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let (mut file, mut staging) =
            create_staging_file(staging_path(&path, self.kind, seq)).await?;
        file.write_all(&data).await?;
        file.sync_all().await?;
        drop(file);

        tokio::fs::rename(staging.path(), &path).await?;
        staging.disarm();

        if let Some(parent) = path.parent()
            && let Ok(dir) = tokio::fs::File::open(parent).await
        {
            let _ = dir.sync_all().await;
        }

        debug!(kind = self.kind, hash, bytes = data.len(), "stored");
        Ok(())
    }

    pub async fn retrieve(&self, hash: &str) -> Result<Option<Bytes>, StorageError> {
        if self.withholds(hash) {
            return Ok(None);
        }
        let path = resolve_file_path(&self.root, hash)?;

        if stat_for_read(&self.known_shards, &path).await?.is_some() {
            let data = tokio::fs::read(&path).await?;
            return Ok(Some(Bytes::from(data)));
        }

        Ok(None)
    }

    /// [`retrieve`](Self::retrieve) for a blob read on behalf of the entity that names it (a
    /// profile's snapshot file): a listed entity id withholds its files as a listed hash does.
    pub async fn retrieve_of(
        &self,
        entity_id: &str,
        hash: &str,
    ) -> Result<Option<Bytes>, StorageError> {
        if self.withholds(entity_id) {
            return Ok(None);
        }
        self.retrieve(hash).await
    }

    pub async fn exist(&self, hash: &str) -> Result<bool, StorageError> {
        let path = resolve_file_path(&self.root, hash)?;
        Ok(stat_for_read(&self.known_shards, &path).await?.is_some())
    }

    /// Batch existence probe for `/available-content`: a provably-invalid id is a per-id miss that
    /// never poisons the batch; only real storage faults abort it.
    pub async fn exist_multiple<S: AsRef<str>>(
        &self,
        hashes: &[S],
    ) -> Result<HashMap<String, bool>, StorageError> {
        use futures::future::BoxFuture;
        use futures::stream::{self, StreamExt};

        let futs: Vec<BoxFuture<'_, (&str, Result<bool, StorageError>)>> = hashes
            .iter()
            .map(|hash| {
                let hash = hash.as_ref();
                Box::pin(async move { (hash, self.exist(hash).await) }) as _
            })
            .collect();
        let mut st = stream::iter(futs).buffered(32);

        let mut results = HashMap::with_capacity(hashes.len());
        while let Some((hash, res)) = st.next().await {
            let exists = match res {
                Ok(exists) => exists,
                Err(StorageError::InvalidId(_)) | Err(StorageError::PathTraversal(_)) => false,
                Err(e) => return Err(e),
            };
            results.insert(hash.to_owned(), exists);
        }
        Ok(results)
    }

    /// Best-effort delete: unlink failures other than already-gone are only logged; use
    /// [`delete_strict`](Self::delete_strict) to observe the outcome.
    pub async fn delete(&self, hash: &str) -> Result<(), StorageError> {
        let path = resolve_file_path(&self.root, hash)?;

        if let Err(e) = tokio::fs::remove_file(&path).await
            && e.kind() != std::io::ErrorKind::NotFound
        {
            warn!(kind = self.kind, hash, error = %e, "failed to delete file");
        }

        debug!(kind = self.kind, hash, "deleted");
        Ok(())
    }

    /// `Ok(())` proves the path no longer holds the content; already-gone counts as success.
    pub async fn delete_strict(&self, hash: &str) -> Result<(), StorageError> {
        let path = resolve_file_path(&self.root, hash)?;

        match tokio::fs::remove_file(&path).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }

        debug!(kind = self.kind, hash, "deleted");
        Ok(())
    }

    /// One open answers the whole `GET /contents/{hash}` request: size, sniff window, range and body
    /// all come off the same descriptor, so there is no window between the check and the read in
    /// which a destroyed shard could be reported as a legitimate 404.
    ///
    /// A non-canonical id is a miss here (the HTTP layer answers 404 for it), where the point
    /// methods above reject it as [`StorageError::InvalidId`].
    pub async fn open(&self, hash: &str) -> Result<Option<OpenedContent>, StorageError> {
        if self.withholds(hash) {
            return Ok(None);
        }
        let path = match resolve_file_path(&self.root, hash) {
            Ok(path) => path,
            Err(StorageError::InvalidId(_)) | Err(StorageError::PathTraversal(_)) => {
                return Ok(None);
            }
            Err(e) => return Err(e),
        };

        Ok(open_for_read(&self.known_shards, &path)
            .await?
            .map(|(file, meta)| OpenedContent {
                size: meta.len(),
                file,
            }))
    }

    /// The bytes in `start..=end`, or `None` when the id is absent or the window starts past its end.
    ///
    /// Reads the WINDOW, not the file: slicing a range out of a whole-file read means a 30 MB model
    /// resident to answer a 32-byte sniff, once per concurrent request.
    pub async fn read_range(
        &self,
        hash: &str,
        start: u64,
        end: u64,
    ) -> Result<Option<Bytes>, StorageError> {
        if self.withholds(hash) {
            return Ok(None);
        }
        let path = resolve_file_path(&self.root, hash)?;
        let Some((file, meta)) = open_for_read(&self.known_shards, &path).await? else {
            return Ok(None);
        };
        let mut opened = OpenedContent {
            size: meta.len(),
            file,
        };
        if start >= opened.size {
            return Ok(None);
        }
        opened.read_range(start, end).await.map(Some)
    }

    pub async fn file_info(&self, hash: &str) -> Result<Option<FileInfo>, StorageError> {
        if self.withholds(hash) {
            return Ok(None);
        }
        let path = resolve_file_path(&self.root, hash)?;

        Ok(stat_for_read(&self.known_shards, &path)
            .await?
            .map(|meta| FileInfo { size: meta.len() }))
    }

    /// The on-disk path of a present id, under the same miss-vs-fault rules as [`exist`](Self::exist).
    pub async fn file_path(&self, hash: &str) -> Result<Option<PathBuf>, StorageError> {
        if self.withholds(hash) {
            return Ok(None);
        }
        let path = resolve_file_path(&self.root, hash)?;

        Ok(stat_for_read(&self.known_shards, &path)
            .await?
            .map(|_| path))
    }

    /// The CIDv1 the stored bytes actually hash to, or `None` when the id is absent.
    ///
    /// A key in this store is a claim about its own content, and nothing enforces the claim at write
    /// time for content that arrived from anywhere but [`store`](Self::store); `exist()` answers the
    /// much weaker "is there a file here". Streams the file, so a snapshot of hundreds of megabytes
    /// costs a buffer, not its length.
    pub async fn stored_content_hash(&self, hash: &str) -> Result<Option<String>, StorageError> {
        use tokio::io::AsyncReadExt;

        let path = resolve_file_path(&self.root, hash)?;
        let Some((mut file, _)) = open_for_read(&self.known_shards, &path).await? else {
            return Ok(None);
        };

        let mut writer = crate::auth::hash::HashV1Writer::new();
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            let n = file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            writer.update(&buf[..n]);
        }

        Ok(Some(writer.finish()))
    }

    /// Moves stored content from one id to another, reporting whether there was anything to move.
    ///
    /// For content whose key turned out to misdescribe it: the bytes are worth keeping and the key is
    /// not. A rename settles that without re-reading a file that can run to hundreds of megabytes, and
    /// it retires the wrong key in the same step -- leaving it in place would keep serving bytes under
    /// a CID they do not hash to.
    pub async fn rekey(&self, from: &str, to: &str) -> Result<bool, StorageError> {
        let src = resolve_file_path(&self.root, from)?;
        let dst = ensure_file_path(&self.root, to, &self.known_shards).await?;

        match tokio::fs::rename(&src, &dst).await {
            Ok(()) => {
                if let Some(parent) = dst.parent()
                    && let Ok(dir) = tokio::fs::File::open(parent).await
                {
                    let _ = dir.sync_all().await;
                }
                debug!(kind = self.kind, from, to, "re-keyed");
                Ok(true)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// See [`FileIds`] for the contract. The round-trip filter matters for both trees: a staging
    /// file left by a cancelled store is named `<id>.<pid>.<seq>.tmp`, which no read can resolve,
    /// and offering it as an id makes a consumer that syncs from this list act on content that does
    /// not exist.
    pub fn all_file_ids<'a>(&'a self, prefix: Option<&'a str>) -> FileIds<'a> {
        FileIds::new(&self.root, &self.known_shards, self.kind, prefix)
    }
}

/// The local backend stores raw bytes only, so there is no `encoding`: a `.gzip` companion next to
/// a blob is never consulted (an unverified sibling would let anyone plant bytes under a CID).
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub size: u64,
}

pub struct OpenedContent {
    pub size: u64,
    file: tokio::fs::File,
}

impl OpenedContent {
    /// The bytes in `start..=end` clamped to the file; the reader is left positioned after them.
    pub async fn read_range(&mut self, start: u64, end: u64) -> Result<Bytes, StorageError> {
        use tokio::io::{AsyncReadExt, AsyncSeekExt};

        if start >= self.size {
            return Ok(Bytes::new());
        }
        let len = (end.min(self.size - 1) - start + 1) as usize;
        self.file.seek(std::io::SeekFrom::Start(start)).await?;
        let mut buf = vec![0u8; len];
        let mut filled = 0;
        while filled < len {
            let n = self.file.read(&mut buf[filled..]).await?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        buf.truncate(filled);
        Ok(Bytes::from(buf))
    }

    /// Streams the file from the start; `consumed` is whatever a preceding `read_range(0, ..)`
    /// took off the front, replayed instead of re-read.
    pub fn into_body(self, consumed: Bytes) -> axum::body::Body {
        use futures::StreamExt;

        let rest = tokio_util::io::ReaderStream::with_capacity(self.file, 64 * 1024);
        axum::body::Body::from_stream(
            futures::stream::iter([Ok::<_, std::io::Error>(consumed)]).chain(rest),
        )
    }
}

#[cfg(test)]
mod tests;
