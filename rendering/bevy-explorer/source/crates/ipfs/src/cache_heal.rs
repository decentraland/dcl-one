//! Drops a url's cached entry whose bytes failed to load, so the next request downloads them again.
//!
//! A load failure whose error says the bytes are bad ([`says_bad_bytes`]) has the cache entry its
//! file reads from dropped, once per entry per session ([`CacheHeals`]), when that entry is filed
//! under a url: those bytes were never checked against anything. An entry under a content hash is
//! left alone, as it was checked against the hash before it was written (`download.rs`). After a
//! drop every asset that failed on the entry is reloaded once, which downloads it. On web the entry
//! is the service worker's under the url's own key (`web_cache::forget`); natively it is the disk
//! cache file.

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "cache_heal_tests.rs"]
mod tests;

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Mutex, PoisonError},
};

use bevy::{
    asset::{AssetLoadError, AssetPath, UntypedAssetLoadFailedEvent},
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use common::util::{TaskCompat, TaskExt};

#[cfg(any(target_arch = "wasm32", test))]
use crate::IpfsContext;
use crate::{
    ipfs_path::{CacheKey, IpfsPath},
    IpfsIo, IpfsResource,
};

/// glTF extensions the asset processor (`image_processing`) repairs a failed load for: such a
/// failure is the processor's to handle, not a sign of bad bytes.
pub const REPAIRABLE_GLTF_EXTENSIONS: [&str; 1] = ["KHR_draco_mesh_compression"];

/// Whether a load error's text names an extension the asset processor repairs.
pub fn names_repairable_extension(text: &str) -> bool {
    REPAIRABLE_GLTF_EXTENSIONS
        .iter()
        .any(|extension| text.contains(extension))
}

/// Most cache entries checked in one session; failures past it are left as they are.
const MAX_CACHE_HEALS: usize = 1024;

/// Whether a failed load says the bytes it read are bad: the loader rejected them or panicked on
/// them. Reader errors (missing, interrupted, refused) and setup errors say nothing about the bytes.
pub fn says_bad_bytes(error: &AssetLoadError) -> bool {
    match error {
        // the loader's own error is opaque: its text is the only way to tell a repairable glTF
        AssetLoadError::AssetLoaderError(_) => !names_repairable_extension(&error.to_string()),
        AssetLoadError::AssetLoaderPanic { .. } => true,
        _ => false,
    }
}

/// Where the check of one cache entry stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Check {
    Checking,
    Kept,
    Dropped,
}

/// The cache entries checked this session, by cache name, and where each check stands.
#[derive(Default)]
pub(crate) struct CacheHeals(Mutex<HashMap<String, Check>>);

impl CacheHeals {
    /// Claims the one check `name` gets this session: false when it had it, or when the session
    /// is past [`MAX_CACHE_HEALS`].
    fn begin(&self, name: &str) -> bool {
        let mut checked = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if checked.contains_key(name) || checked.len() >= MAX_CACHE_HEALS {
            return false;
        }
        checked.insert(name.to_owned(), Check::Checking);
        true
    }

    /// Records what the check of `name` found.
    fn finish(&self, name: &str, dropped: bool) {
        if let Some(check) = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_mut(name)
        {
            *check = if dropped { Check::Dropped } else { Check::Kept };
        }
    }

    fn state(&self, name: &str) -> Option<Check> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(name)
            .copied()
    }

    /// (entries checked, entries dropped) this session.
    pub(crate) fn counts(&self) -> (usize, usize) {
        let checked = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        (
            checked.len(),
            checked.values().filter(|c| **c == Check::Dropped).count(),
        )
    }
}

/// What the check for one failed load did with its cache entry, named by the entry's cache name.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Checked {
    /// Nothing was dropped: there was no entry, or the delete failed.
    Kept(String),
    /// The entry was dropped.
    Dropped(String),
    /// Another failure's check has the entry, under way or finished.
    Elsewhere(String),
}

impl IpfsIo {
    /// Drops the url-keyed cache entry the failed load of `path` read, once per entry per session.
    /// None when there is nothing to check: no entry under a url, or the session is past
    /// [`MAX_CACHE_HEALS`].
    pub(crate) async fn drop_bad_cached_entry(&self, path: &Path) -> Option<Checked> {
        let Ok(Some(ipfs_path)) = IpfsPath::new_from_path(path) else {
            return None;
        };
        let context = self.context.read().await;
        // an entry under a content hash was checked against it before it was written
        let Some(CacheKey::Url(name)) = ipfs_path.cache_key(&context) else {
            return None;
        };
        #[cfg(target_arch = "wasm32")]
        let remote = web_lookup_url(&ipfs_path, &context)?;
        drop(context);
        if !self.cache_heals.begin(&name) {
            return self
                .cache_heals
                .state(&name)
                .is_some()
                .then_some(Checked::Elsewhere(name));
        }
        #[cfg(not(target_arch = "wasm32"))]
        let forgotten = self
            .forget_disk_cache(&CacheKey::Url(name.clone()))
            .await
            .map_err(|e| e.to_string());
        #[cfg(target_arch = "wasm32")]
        let forgotten = crate::web_cache::forget(&remote)
            .await
            .map_err(|e| format!("{e:?}"));
        let dropped = forgotten.unwrap_or_else(|error| {
            warn!(
                "failed to drop the cached copy of `{}`: {error}",
                path.display()
            );
            false
        });
        if dropped {
            warn!(
                "dropped the cached copy of `{}`: its bytes failed to load",
                path.display()
            );
        }
        self.cache_heals.finish(&name, dropped);
        Some(if dropped {
            Checked::Dropped(name)
        } else {
            Checked::Kept(name)
        })
    }
}

/// The url whose web cache entry a read of `ipfs_path` gets, if a read looks in the cache at all:
/// loopback content is local already, and a preview server's is mutable, so it is never cached.
#[cfg(any(target_arch = "wasm32", test))]
fn web_lookup_url(ipfs_path: &IpfsPath, context: &IpfsContext) -> Option<String> {
    let remote = ipfs_path.to_url(context).ok()?;
    (!common::util::is_loopback_url(&remote)).then_some(remote)
}

/// Checks under way, and the assets that failed on an entry until its check has finished.
#[derive(Resource, Default)]
pub(crate) struct PendingCacheHeals {
    // each yields the failed asset and what its check did
    checks: Vec<Task<(AssetPath<'static>, Option<Checked>)>>,
    // failed assets whose entry another failure is still checking, with that entry's name
    waiting: Vec<(String, AssetPath<'static>)>,
    // assets reloaded after their entry was dropped, with its name: each is reloaded once
    reloaded: HashSet<(String, AssetPath<'static>)>,
}

impl PendingCacheHeals {
    /// The assets to reload now that the check for the failed load of `path` reported `checked`.
    /// Only a check found under way elsewhere reads `heals`, to learn whether it has finished.
    fn settle(
        &mut self,
        heals: &CacheHeals,
        path: AssetPath<'static>,
        checked: Option<Checked>,
    ) -> Vec<AssetPath<'static>> {
        let (name, dropped) = match checked {
            None => return Vec::new(),
            Some(Checked::Kept(name)) => (name, false),
            Some(Checked::Dropped(name)) => (name, true),
            Some(Checked::Elsewhere(name)) => match heals.state(&name) {
                Some(Check::Checking) => {
                    let entry = (name, path);
                    if self.waiting.len() + self.reloaded.len() < MAX_CACHE_HEALS
                        && !self.waiting.contains(&entry)
                    {
                        self.waiting.push(entry);
                    }
                    return Vec::new();
                }
                state => (name, state == Some(Check::Dropped)),
            },
        };
        if !dropped {
            self.waiting.retain(|(waiting, _)| *waiting != name);
            return Vec::new();
        }
        let mut assets = vec![path];
        self.waiting.retain(|(waiting, asset)| {
            let ours = *waiting == name;
            if ours {
                assets.push(asset.clone());
            }
            !ours
        });
        assets.retain(|asset| {
            self.reloaded.len() < MAX_CACHE_HEALS
                && self.reloaded.insert((name.clone(), asset.clone()))
        });
        assets
    }
}

pub(crate) struct CacheHealPlugin;

impl Plugin for CacheHealPlugin {
    fn build(&self, _app: &mut App) {}

    /// After the asset plugins exist: an app without them has no load failures.
    fn finish(&self, app: &mut App) {
        if app
            .world()
            .contains_resource::<Events<UntypedAssetLoadFailedEvent>>()
        {
            app.init_resource::<PendingCacheHeals>()
                .add_systems(Update, drop_failed_cached_entries);
        }
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
        pending
            .checks
            .push(IoTaskPool::get().spawn_compat(async move {
                let checked = io.drop_bad_cached_entry(path.path()).await;
                (path, checked)
            }));
    }
    let mut done = Vec::new();
    pending.checks.retain_mut(|task| match task.complete() {
        Some(result) => {
            done.push(result);
            false
        }
        None => true,
    });
    for (path, checked) in done {
        for asset in pending.settle(&ipfs.cache_heals, path, checked) {
            server.reload(asset);
        }
    }
}
