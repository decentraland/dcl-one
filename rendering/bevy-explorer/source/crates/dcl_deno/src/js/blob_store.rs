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

/// One scene's Blob parts, capped at [`MAX_BLOB_BYTES`]. Object URLs are not available to
/// scenes, so none are stored.
#[derive(Debug)]
pub struct CappedBlobStore {
    cap: usize,
    inner: Mutex<Parts>,
}

#[derive(Debug, Default)]
struct Parts {
    parts: HashMap<Uuid, Part>,
    bytes: usize,
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

    /// Inserts a part of `size` bytes built only once the cap allows it.
    pub fn try_insert_with(
        &self,
        size: usize,
        part: impl FnOnce() -> Part,
    ) -> Result<Uuid, AnyError> {
        let mut parts = self.parts();
        let total = parts.bytes.saturating_add(size);
        if total > self.cap {
            deno_core::anyhow::bail!(
                "scene Blob storage is full ({} of {} bytes in use)",
                parts.bytes,
                self.cap
            );
        }
        let id = Uuid::new_v4();
        parts.parts.insert(id, part());
        parts.bytes = total;
        Ok(id)
    }
}

impl BlobStoreTrait for CappedBlobStore {
    fn insert_part(&self, part: Part) -> Uuid {
        let id = Uuid::new_v4();
        let mut parts = self.parts();
        parts.bytes = parts.bytes.saturating_add(part.size());
        parts.parts.insert(id, part);
        id
    }

    fn get_part(&self, id: &Uuid) -> Option<Part> {
        self.parts().parts.get(id).cloned()
    }

    fn remove_part(&self, id: &Uuid) -> Option<Part> {
        let mut parts = self.parts();
        let part = parts.parts.remove(id)?;
        parts.bytes = parts.bytes.saturating_sub(part.size());
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
        parts.bytes = 0;
    }
}
