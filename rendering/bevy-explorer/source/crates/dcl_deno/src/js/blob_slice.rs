use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use deno_core::{op2, FromV8, OpDecl, OpState};
use deno_web::{BlobError, BlobPart, BlobStoreTrait};
use uuid::Uuid;

/// deno_web's `op_blob_slice_part`, with slices that are never more than one level deep.
///
/// Upstream wraps the part it is given, so slicing a slice builds a chain, and reading or
/// dropping a long chain recurses once per level until the scene thread's stack overflows.
/// Here a slice of a slice wraps the original part at an absolute offset.
pub fn override_ops() -> Vec<OpDecl> {
    vec![op_blob_slice_part()]
}

type Part = Arc<dyn BlobPart + Send + Sync>;

/// The original part and offset of each live [`Slice`], by the slice's address. The store
/// hands parts back as trait objects, so this is how a part is recognised as one of ours.
#[derive(Clone, Debug, Default)]
struct Slices(Arc<Mutex<HashMap<usize, (Part, usize)>>>);

#[derive(Debug)]
struct Slice {
    root: Part,
    start: usize,
    len: usize,
    slices: Slices,
}

#[async_trait::async_trait]
impl BlobPart for Slice {
    async fn read<'a>(&'a self) -> &'a [u8] {
        &self.root.read().await[self.start..self.start + self.len]
    }

    fn size(&self) -> usize {
        self.len
    }
}

impl Drop for Slice {
    fn drop(&mut self) {
        let entry = self
            .slices
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&(self as *const Self as usize));
        drop(entry);
    }
}

#[derive(FromV8)]
struct SliceOptions {
    start: usize,
    len: usize,
}

#[op2]
fn op_blob_slice_part(
    state: &mut OpState,
    #[scoped] id: Uuid,
    #[scoped] options: SliceOptions,
) -> Result<Uuid, BlobError> {
    let SliceOptions { start, len } = options;
    let part = state
        .borrow::<Arc<dyn BlobStoreTrait>>()
        .get_part(&id)
        .ok_or(BlobError::BlobPartNotFound)?;
    if start.checked_add(len).is_none_or(|end| end > part.size()) {
        return Err(BlobError::SizeLargerThanBlobPart);
    }

    if !state.has::<Slices>() {
        state.put(Slices::default());
    }
    let slices = state.borrow::<Slices>().clone();
    let mut live = slices
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (root, base) = live
        .get(&(Arc::as_ptr(&part) as *const () as usize))
        .cloned()
        .unwrap_or((part, 0));
    let slice = Arc::new(Slice {
        root: root.clone(),
        start: base + start,
        len,
        slices: slices.clone(),
    });
    live.insert(Arc::as_ptr(&slice) as usize, (root, base + start));
    drop(live);

    Ok(state.borrow::<Arc<dyn BlobStoreTrait>>().insert_part(slice))
}
