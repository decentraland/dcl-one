//! Drops a cached entry whose bytes failed to load, so the next request downloads them again.
//!
//! A load failure whose error says the bytes are bad ([`says_bad_bytes`]) has the cache entry its
//! file reads from checked, once per entry per session ([`CacheHeals`]). An entry filed under a
//! content hash is dropped only when its bytes do not hash to it: the cache takes nothing else under
//! a hash, so what fails the check is a processed or damaged copy, never the content itself. An
//! entry filed under a url cannot be checked and is dropped. After a drop the asset is reloaded,
//! which downloads it, and the asset processor leaves that file alone for the rest of the session
//! ([`IpfsIo::cache_entry_dropped`]). On web the entry is the service worker's, under both its keys
//! (`web_cache`); natively it is the disk cache file.

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "cache_heal_tests.rs"]
mod tests;

use std::{collections::HashMap, path::Path, sync::Mutex};

use bevy::{
    asset::{AssetLoadError, AssetPath, UntypedAssetLoadFailedEvent},
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use common::util::{TaskCompat, TaskExt};

#[cfg(not(target_arch = "wasm32"))]
use crate::ipfs_path::CacheKey;
use crate::{content_hash, ipfs_path::IpfsPath, IpfsIo, IpfsResource};

/// glTF extensions the asset processor (`image_processing`) repairs a failed load for: such a
/// failure is the processor's to handle, not a sign of bad bytes.
pub const REPAIRABLE_GLTF_EXTENSIONS: [&str; 2] =
    ["KHR_draco_mesh_compression", "KHR_texture_basisu"];

/// Most cache entries checked in one session; failures past it are left as they are.
const MAX_CACHE_HEALS: usize = 1024;

/// Whether a failed load says the bytes it read are bad: the loader rejected them or panicked on
/// them. Reader errors (missing, interrupted, refused) and setup errors say nothing about the bytes.
pub fn says_bad_bytes(error: &AssetLoadError) -> bool {
    match error {
        // the loader's own error is opaque: its text is the only way to tell a repairable glTF
        AssetLoadError::AssetLoaderError(_) => {
            let text = error.to_string();
            !REPAIRABLE_GLTF_EXTENSIONS
                .iter()
                .any(|extension| text.contains(extension))
        }
        AssetLoadError::AssetLoaderPanic { .. } => true,
        _ => false,
    }
}

/// The name a cache entry is filed under.
#[derive(Clone, Copy, Debug)]
pub(crate) enum FiledUnder<'a> {
    /// A content hash: the bytes can be checked against it.
    Hash(&'a str),
    /// A url, or a digest of one: nothing to check the bytes against.
    Url,
}

/// Whether a cached entry's bytes are not what its name vouches for, and so may be dropped.
pub(crate) fn holds_other_bytes(filed: FiledUnder, bytes: &[u8]) -> bool {
    match filed {
        FiledUnder::Hash(hash) => !content_hash::matches_content_hash(hash, bytes),
        FiledUnder::Url => true,
    }
}

/// The cache entries checked this session, by cache name, and whether each was dropped.
#[derive(Default)]
pub(crate) struct CacheHeals(Mutex<HashMap<String, bool>>);

impl CacheHeals {
    /// Claims the one check `name` gets this session: false when it had it, or when the session
    /// is past [`MAX_CACHE_HEALS`].
    fn begin(&self, name: &str) -> bool {
        let mut checked = self.0.lock().unwrap();
        if checked.contains_key(name) || checked.len() >= MAX_CACHE_HEALS {
            return false;
        }
        checked.insert(name.to_owned(), false);
        true
    }

    fn dropped(&self, name: &str) {
        if let Some(dropped) = self.0.lock().unwrap().get_mut(name) {
            *dropped = true;
        }
    }

    fn was_dropped(&self, name: &str) -> bool {
        self.0.lock().unwrap().get(name).copied().unwrap_or(false)
    }

    /// (entries checked, entries dropped) this session.
    pub(crate) fn counts(&self) -> (usize, usize) {
        let checked = self.0.lock().unwrap();
        (checked.len(), checked.values().filter(|d| **d).count())
    }
}

impl IpfsIo {
    /// Whether the cached entry under `name` (a cache key's name: the hash `IpfsPath::hash` gives)
    /// was dropped this session because its bytes failed to load: whatever wrote them must not
    /// write them again.
    pub fn cache_entry_dropped(&self, name: &str) -> bool {
        self.cache_heals.was_dropped(name)
    }

    /// Drops the cached entry the file at `path` reads from, when its bytes are not what the
    /// entry's name vouches for. Once per entry per session. True when an entry was dropped: the
    /// asset should be reloaded.
    pub(crate) async fn drop_bad_cached_entry(&self, path: &Path) -> bool {
        let Ok(Some(ipfs_path)) = IpfsPath::new_from_path(path) else {
            return false;
        };
        let context = self.context.read().await;
        let Some(key) = ipfs_path.cache_key(&context) else {
            return false;
        };
        #[cfg(target_arch = "wasm32")]
        let remote = ipfs_path.to_url(&context);
        drop(context);
        if !self.cache_heals.begin(key.name()) {
            return false;
        }
        #[cfg(not(target_arch = "wasm32"))]
        let dropped = self.drop_bad_disk_entry(&key).await;
        #[cfg(target_arch = "wasm32")]
        let dropped = match remote {
            Ok(remote) => drop_bad_web_entry(&remote).await,
            Err(_) => false,
        };
        if dropped {
            warn!(
                "dropped the cached copy of `{}`: its bytes failed to load",
                path.display()
            );
            self.cache_heals.dropped(key.name());
        }
        dropped
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn drop_bad_disk_entry(&self, key: &CacheKey) -> bool {
        let Ok(Some(bytes)) = self.read_disk_cache(key).await else {
            return false;
        };
        let filed = match key {
            CacheKey::Content(hash) => FiledUnder::Hash(hash),
            CacheKey::Url(_) => FiledUnder::Url,
        };
        if !holds_other_bytes(filed, &bytes) {
            return false;
        }
        match self.forget_disk_cache(key).await {
            Ok(()) => true,
            Err(error) => {
                warn!("failed to drop the cached copy `{}`: {error}", key.name());
                false
            }
        }
    }
}

/// Drops the entry a read of `remote` gets (its shared one first, else its own), when its bytes
/// are not what its key vouches for.
#[cfg(target_arch = "wasm32")]
async fn drop_bad_web_entry(remote: &str) -> bool {
    use crate::web_cache;
    let Ok(cache) = web_cache::open().await else {
        return false;
    };
    let Ok((key, response)) = web_cache::find(&cache, remote).await else {
        return false;
    };
    let Ok(bytes) = web_cache::bytes(&response).await else {
        return false;
    };
    let filed = web_cache::shared_hash(&key).map_or(FiledUnder::Url, FiledUnder::Hash);
    if !holds_other_bytes(filed, &bytes) {
        return false;
    }
    match web_cache::discard(&cache, remote).await {
        Ok(()) => true,
        Err(error) => {
            warn!("failed to drop the cached copy of `{remote}`: {error:?}");
            false
        }
    }
}

/// Checks under way: each yields the asset to reload once its cached entry is gone.
#[derive(Resource, Default)]
pub(crate) struct PendingCacheHeals(Vec<Task<Option<AssetPath<'static>>>>);

/// After the asset plugins exist: an app without them has no load failures.
pub(crate) fn finish(app: &mut App) {
    if app
        .world()
        .contains_resource::<Events<UntypedAssetLoadFailedEvent>>()
    {
        app.init_resource::<PendingCacheHeals>()
            .add_systems(Update, drop_failed_cached_entries);
    }
}

/// Checks the cached entry of each asset whose bytes failed to load, and reloads the asset once a
/// bad entry is gone.
pub(crate) fn drop_failed_cached_entries(
    mut failed: EventReader<UntypedAssetLoadFailedEvent>,
    ipfs: Res<IpfsResource>,
    server: Res<AssetServer>,
    mut pending: ResMut<PendingCacheHeals>,
) {
    for event in failed.read() {
        // a variant's failure is the variants client's (`bevy_variants`), which runs after this
        if !says_bad_bytes(&event.error) || ipfs.served_bevy_variant(event.path.path()) {
            continue;
        }
        let path = event.path.without_label().into_owned();
        let io = ipfs.inner.clone();
        pending.0.push(IoTaskPool::get().spawn_compat(async move {
            let dropped = io.drop_bad_cached_entry(path.path()).await;
            dropped.then_some(path)
        }));
    }
    pending.0.retain_mut(|task| match task.complete() {
        Some(Some(path)) => {
            server.reload(path);
            false
        }
        Some(None) => false,
        None => true,
    });
}
