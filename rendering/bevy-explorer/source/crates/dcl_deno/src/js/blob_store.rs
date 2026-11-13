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

/// One scene's Blob parts, capped at [`MAX_BLOB_BYTES`]. A clone or slice of a part shares
/// its bytes, which count once until the last id holding them goes. Object URLs are not
/// available to scenes, so none are stored.
#[derive(Debug)]
pub struct CappedBlobStore {
    cap: usize,
    inner: Mutex<Parts>,
}

#[derive(Debug)]
struct Entry {
    part: Part,
    /// The address and size of the bytes `part` reads from.
    backing: usize,
    backing_size: usize,
}

#[derive(Debug, Default)]
struct Parts {
    parts: HashMap<Uuid, Entry>,
    /// How many ids hold each backing.
    holders: HashMap<usize, usize>,
    bytes: usize,
    /// `bytes` as last reported to V8.
    reported: usize,
}

fn address(part: &Part) -> usize {
    Arc::as_ptr(part) as *const () as usize
}

impl Parts {
    fn insert(&mut self, part: Part, backing: usize, backing_size: usize) -> Uuid {
        let id = Uuid::new_v4();
        let holders = self.holders.entry(backing).or_default();
        if *holders == 0 {
            self.bytes = self.bytes.saturating_add(backing_size);
        }
        *holders += 1;
        self.parts.insert(
            id,
            Entry {
                part,
                backing,
                backing_size,
            },
        );
        id
    }

    fn insert_new(&mut self, part: Part) -> Uuid {
        let (backing, size) = (address(&part), part.size());
        self.insert(part, backing, size)
    }
}

/// `len` bytes of `part` from `start`, read without a copy.
#[derive(Debug)]
struct SlicedPart {
    part: Part,
    start: usize,
    len: usize,
}

#[async_trait::async_trait]
impl BlobPart for SlicedPart {
    async fn read<'a>(&'a self) -> &'a [u8] {
        let bytes = self.part.read().await;
        &bytes[self.start..self.start + self.len]
    }

    fn size(&self) -> usize {
        self.len
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
        Ok(parts.insert_new(part()))
    }

    /// The change in stored bytes since the last call, for V8's external memory count.
    pub fn take_external_change(&self) -> i64 {
        let mut parts = self.parts();
        let change = parts.bytes as i64 - parts.reported as i64;
        parts.reported = parts.bytes;
        change
    }

    /// Stores the part `id` holds under another id, with its size.
    pub fn insert_clone(&self, id: &Uuid) -> Option<(Uuid, usize)> {
        let mut parts = self.parts();
        let entry = parts.parts.get(id)?;
        let (part, backing, backing_size) = (entry.part.clone(), entry.backing, entry.backing_size);
        let size = part.size();
        Some((parts.insert(part, backing, backing_size), size))
    }

    /// Stores `len` bytes of the part `id` holds, from `start`, sharing its bytes.
    pub fn insert_slice(&self, id: &Uuid, start: usize, len: usize) -> Result<Uuid, AnyError> {
        let mut parts = self.parts();
        let entry = parts
            .parts
            .get(id)
            .ok_or_else(|| deno_core::anyhow::anyhow!("Blob part not found"))?;
        if start
            .checked_add(len)
            .is_none_or(|end| end > entry.part.size())
        {
            deno_core::anyhow::bail!("start + len can not be larger than blob part size");
        }
        let part: Part = Arc::new(SlicedPart {
            part: entry.part.clone(),
            start,
            len,
        });
        let (backing, backing_size) = (entry.backing, entry.backing_size);
        Ok(parts.insert(part, backing, backing_size))
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
    /// Past the cap the part is not stored, so reading the returned id fails.
    fn insert_part(&self, part: Part) -> Uuid {
        let mut parts = self.parts();
        if parts.bytes.saturating_add(part.size()) > self.cap {
            return Uuid::new_v4();
        }
        parts.insert_new(part)
    }

    fn get_part(&self, id: &Uuid) -> Option<Part> {
        self.parts().parts.get(id).map(|entry| entry.part.clone())
    }

    fn remove_part(&self, id: &Uuid) -> Option<Part> {
        let mut parts = self.parts();
        let entry = parts.parts.remove(id)?;
        if let Some(holders) = parts.holders.get_mut(&entry.backing) {
            *holders -= 1;
            if *holders == 0 {
                parts.holders.remove(&entry.backing);
                parts.bytes = parts.bytes.saturating_sub(entry.backing_size);
            }
        }
        Some(entry.part)
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

#[cfg(test)]
mod tests {
    use super::*;
    use deno_web::InMemoryBlobPart;

    #[test]
    fn insert_part_past_the_cap_stores_nothing() {
        let store = CappedBlobStore::with_cap(8);
        let fits = store.insert_part(Arc::new(InMemoryBlobPart::from(vec![0; 8])));
        let past = store.insert_part(Arc::new(InMemoryBlobPart::from(vec![0; 1])));
        assert!(store.get_part(&fits).is_some());
        assert!(store.get_part(&past).is_none());
    }
}
