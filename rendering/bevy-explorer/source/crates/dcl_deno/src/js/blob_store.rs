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

/// A stored part: `len` bytes of `root` from `offset`. Slices and clones keep the root, so a
/// part is never more than one view deep however often it is sliced.
#[derive(Debug)]
struct Entry {
    root: Part,
    offset: usize,
    len: usize,
}

impl Entry {
    fn whole(root: Part) -> Self {
        let len = root.size();
        Self {
            root,
            offset: 0,
            len,
        }
    }

    fn view(&self) -> Part {
        if self.offset == 0 && self.len == self.root.size() {
            return self.root.clone();
        }
        Arc::new(SlicedPart {
            part: self.root.clone(),
            start: self.offset,
            len: self.len,
        })
    }
}

#[derive(Debug, Default)]
struct Parts {
    parts: HashMap<Uuid, Entry>,
    /// How many ids hold each root, by its address.
    holders: HashMap<usize, usize>,
    bytes: usize,
    /// `bytes` as last reported to V8.
    reported: usize,
}

fn address(part: &Part) -> usize {
    Arc::as_ptr(part) as *const () as usize
}

impl Parts {
    fn insert(&mut self, entry: Entry) -> Uuid {
        let id = Uuid::new_v4();
        let holders = self.holders.entry(address(&entry.root)).or_default();
        if *holders == 0 {
            self.bytes = self.bytes.saturating_add(entry.root.size());
        }
        *holders += 1;
        self.parts.insert(id, entry);
        id
    }
}

/// `len` bytes of a root part from `start`, read without a copy.
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
        Ok(parts.insert(Entry::whole(part())))
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
        let clone = Entry {
            root: entry.root.clone(),
            offset: entry.offset,
            len: entry.len,
        };
        let size = clone.len;
        Some((parts.insert(clone), size))
    }

    /// Stores `len` bytes of the part `id` holds, from `start`, sharing its bytes.
    pub fn insert_slice(&self, id: &Uuid, start: usize, len: usize) -> Result<Uuid, AnyError> {
        let mut parts = self.parts();
        let entry = parts
            .parts
            .get(id)
            .ok_or_else(|| deno_core::anyhow::anyhow!("Blob part not found"))?;
        if start.checked_add(len).is_none_or(|end| end > entry.len) {
            deno_core::anyhow::bail!("start + len can not be larger than blob part size");
        }
        let slice = Entry {
            root: entry.root.clone(),
            offset: entry.offset + start,
            len,
        };
        Ok(parts.insert(slice))
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
        parts.insert(Entry::whole(part))
    }

    fn get_part(&self, id: &Uuid) -> Option<Part> {
        self.parts().parts.get(id).map(Entry::view)
    }

    fn remove_part(&self, id: &Uuid) -> Option<Part> {
        let mut parts = self.parts();
        let entry = parts.parts.remove(id)?;
        let root = address(&entry.root);
        if let Some(holders) = parts.holders.get_mut(&root) {
            *holders -= 1;
            if *holders == 0 {
                parts.holders.remove(&root);
                parts.bytes = parts.bytes.saturating_sub(entry.root.size());
            }
        }
        Some(entry.view())
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

    fn read(part: Part) -> Vec<u8> {
        deno_core::futures::FutureExt::now_or_never(part.read())
            .unwrap()
            .to_vec()
    }

    #[test]
    fn slices_and_clones_keep_their_offsets_one_view_deep() {
        let store = CappedBlobStore::default();
        let root = store.insert_part(Arc::new(InMemoryBlobPart::from(
            (0..10).collect::<Vec<u8>>(),
        )));
        let slice = store.insert_slice(&root, 2, 6).unwrap();
        let inner = store.insert_slice(&slice, 1, 3).unwrap();
        let (clone, size) = store.insert_clone(&inner).unwrap();
        assert_eq!(size, 3);
        assert_eq!(read(store.get_part(&slice).unwrap()), [2, 3, 4, 5, 6, 7]);
        assert_eq!(read(store.get_part(&inner).unwrap()), [3, 4, 5]);
        assert_eq!(read(store.get_part(&clone).unwrap()), [3, 4, 5]);
        assert!(store.insert_slice(&inner, 2, 2).is_err());
    }

    #[test]
    fn a_slice_of_a_full_store_still_fits() {
        let store = CappedBlobStore::with_cap(10);
        let root = store.insert_part(Arc::new(InMemoryBlobPart::from(vec![1u8; 10])));
        let slice = store.insert_slice(&root, 2, 3).unwrap();
        assert_eq!(read(store.get_part(&slice).unwrap()), [1; 3]);
        assert!(store.try_insert_with(1, || unreachable!()).is_err());
    }

    #[test]
    fn a_surviving_slice_keeps_its_root_counted() {
        let store = CappedBlobStore::with_cap(10);
        let part = || Arc::new(InMemoryBlobPart::from(vec![0u8; 3])) as Part;
        let root = store.insert_part(Arc::new(InMemoryBlobPart::from(vec![1u8; 8])));
        let slice = store.insert_slice(&root, 0, 1).unwrap();
        store.remove_part(&root);
        assert!(store.try_insert_with(3, part).is_err());
        assert_eq!(read(store.get_part(&slice).unwrap()), [1]);
        store.remove_part(&slice);
        assert!(store.try_insert_with(3, part).is_ok());
    }

    #[test]
    fn a_long_chain_of_slices_reads_and_drops() {
        let store = CappedBlobStore::default();
        let mut id = store.insert_part(Arc::new(InMemoryBlobPart::from(vec![7u8; 4])));
        for _ in 0..100_000 {
            let next = store.insert_slice(&id, 0, 4).unwrap();
            store.remove_part(&id);
            id = next;
        }
        assert_eq!(read(store.get_part(&id).unwrap()), [7; 4]);
        store.remove_part(&id);
        assert_eq!(store.take_external_change(), 0);
    }
}
