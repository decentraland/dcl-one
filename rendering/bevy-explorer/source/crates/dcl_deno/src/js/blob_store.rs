use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
};

use deno_core::{error::AnyError, url::Url};
use deno_web::{Blob, BlobPart, BlobStoreTrait};
use uuid::Uuid;

/// Blob bytes live outside the V8 heap, so its cap does not bound them.
pub const MAX_BLOB_BYTES: usize = 64 * 1024 * 1024;

type Part = Arc<dyn BlobPart + Send + Sync>;

/// One scene's Blob parts, capped at [`MAX_BLOB_BYTES`]. A part stored under several ids (a
/// structured clone) counts once. Object URLs are not available to scenes, so none are stored.
#[derive(Debug)]
pub struct CappedBlobStore {
    cap: usize,
    inner: Mutex<Parts>,
}

#[derive(Debug, Default)]
struct Parts {
    parts: HashMap<Uuid, Part>,
    /// How many ids hold each distinct part, keyed by its address.
    holders: HashMap<usize, usize>,
    bytes: usize,
    /// `bytes` as last reported to V8.
    reported: usize,
}

fn address(part: &Part) -> usize {
    Arc::as_ptr(part) as *const () as usize
}

impl Parts {
    /// The bytes in use once a part at `part_address` is stored; a stored part adds nothing.
    fn bytes_with(&self, part_address: usize, size: usize) -> usize {
        if self.holders.contains_key(&part_address) {
            self.bytes
        } else {
            self.bytes.saturating_add(size)
        }
    }

    fn insert(&mut self, part: Part) -> Uuid {
        let id = Uuid::new_v4();
        self.bytes = self.bytes_with(address(&part), part.size());
        *self.holders.entry(address(&part)).or_default() += 1;
        self.parts.insert(id, part);
        id
    }
}

impl Default for CappedBlobStore {
    fn default() -> Self {
        Self::with_cap(MAX_BLOB_BYTES)
    }
}

impl CappedBlobStore {
    pub fn with_cap(cap: usize) -> Self {
        Self {
            cap,
            inner: Default::default(),
        }
    }

    fn parts(&self) -> MutexGuard<'_, Parts> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Inserts a new part of `size` bytes, built only once the cap allows it.
    pub fn try_insert_with(
        &self,
        size: usize,
        part: impl FnOnce() -> Part,
    ) -> Result<Uuid, AnyError> {
        let mut parts = self.parts();
        if parts.bytes.saturating_add(size) > self.cap {
            return Err(full(&parts, self.cap));
        }
        Ok(parts.insert(part()))
    }

    /// The change in stored bytes since the last call, for V8's external memory count.
    pub fn take_external_change(&self) -> i64 {
        let mut parts = self.parts();
        let change = parts.bytes as i64 - parts.reported as i64;
        parts.reported = parts.bytes;
        change
    }

    /// Stores `part` under another id; it counts against the cap only if it is not stored yet.
    pub fn try_insert_shared(&self, part: Part) -> Result<Uuid, AnyError> {
        let mut parts = self.parts();
        if parts.bytes_with(address(&part), part.size()) > self.cap {
            return Err(full(&parts, self.cap));
        }
        Ok(parts.insert(part))
    }
}

fn full(parts: &Parts, cap: usize) -> AnyError {
    deno_core::anyhow::anyhow!(
        "scene Blob storage is full ({} of {} bytes in use)",
        parts.bytes,
        cap
    )
}

impl BlobStoreTrait for CappedBlobStore {
    fn insert_part(&self, part: Part) -> Uuid {
        self.parts().insert(part)
    }

    fn get_part(&self, id: &Uuid) -> Option<Part> {
        self.parts().parts.get(id).cloned()
    }

    fn remove_part(&self, id: &Uuid) -> Option<Part> {
        let mut parts = self.parts();
        let part = parts.parts.remove(id)?;
        let part_address = address(&part);
        if let Some(holders) = parts.holders.get_mut(&part_address) {
            *holders -= 1;
            if *holders == 0 {
                parts.holders.remove(&part_address);
                parts.bytes = parts.bytes.saturating_sub(part.size());
            }
        }
        Some(part)
    }

    fn get_object_url(&self, _url: Url) -> Option<Arc<Blob>> {
        None
    }

    fn insert_object_url(&self, _blob: Blob, _maybe_location: Option<Url>) -> Url {
        // unreachable from scenes: op_blob_create_object_url is refused
        Url::parse(&format!("blob:null/{}", Uuid::new_v4())).expect("a UUID blob URL parses")
    }

    fn remove_object_url(&self, _url: &Url) {}

    fn clear(&self) {
        let mut parts = self.parts();
        parts.parts.clear();
        parts.holders.clear();
        parts.bytes = 0;
    }
}
