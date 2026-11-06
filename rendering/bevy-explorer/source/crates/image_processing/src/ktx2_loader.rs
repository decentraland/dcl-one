//! Standalone `.ktx2` textures, as `KHR_texture_basisu` models reference them: Basis Universal
//! transcoded to BC7 on load, by the same `transcode_ktx2` the gltf repair uses. The dds a
//! transcode produces is stashed in `KTX2_RESULTS` only until the image's
//! `LoadedWithDependencies` event: there `engine::check_assets` takes it out (`take_result`)
//! and carries it in `AssetForProcessing::ktx2_dds` on a `ProcessingAssetType::Ktx2Cache`
//! request, and the processor writes it over the cached ktx2 bytes, so the next load reads a
//! dds (`DDS ` magic) and skips the transcode. The queued write holds its own dds, so it can
//! wait behind any number of gltf repairs without losing it. A transcode whose event never
//! comes (the image was dropped before it loaded) is evicted after `RESULT_LIFETIME`.
//!
//! Only `.ktx2` paths reach this loader: it is registered ahead of the NFT image loader, which
//! stays the last image loader and so takes image paths without an extension (see
//! `ImageProcessingPlugin` in the app's plugin list).

use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    time::Duration,
};

use bevy::{
    asset::{io::Reader, AssetLoader, LoadContext},
    image::{CompressedImageFormats, ImageFormat, ImageLoaderSettings, ImageType},
    platform::time::Instant,
    prelude::*,
    render::renderer::RenderCapabilities,
};

/// Transcoded dds bytes by asset path string, with when they were stashed, waiting for the
/// image's `LoadedWithDependencies` event to take them into a cache write request.
type Ktx2Results = HashMap<String, (Instant, Vec<u8>)>;

static KTX2_RESULTS: LazyLock<Mutex<Ktx2Results>> = LazyLock::new(Default::default);

/// A safety net, not a deadline for the cache write: a stash lasts from the load to the image's
/// `LoadedWithDependencies` event, normally a frame or two, and the queued write carries the dds
/// from there however long it waits behind gltf repairs. What this evicts is a transcode whose
/// event never comes, or comes when `check_assets` can't take it (no handle or asset path).
const RESULT_LIFETIME: Duration = Duration::from_secs(60);

/// Takes the transcode stashed for `path` (an asset path string) out of the stash, for the
/// `Ktx2Cache` request that will write it.
pub(crate) fn take_result(path: &str) -> Option<Vec<u8>> {
    KTX2_RESULTS.lock().ok()?.remove(path).map(|(_, dds)| dds)
}

/// Drops the transcodes stashed longer than `RESULT_LIFETIME`.
pub(crate) fn evict_stale_results() {
    if let Ok(mut results) = KTX2_RESULTS.lock() {
        results.retain(|_, (stashed, _)| stashed.elapsed() < RESULT_LIFETIME);
    }
}

const DDS_MAGIC: &[u8] = b"DDS ";

/// The dds to build the image from, and whether it is a fresh transcode to stash for the cache
/// write: a cached dds as it is, or the ktx2 transcoded to a dds.
fn prepare(bytes: Vec<u8>) -> anyhow::Result<(Vec<u8>, bool)> {
    if bytes.starts_with(DDS_MAGIC) {
        // a previous load's transcode, written over the ktx2 bytes in the cache
        return Ok((bytes, false));
    }
    let dds = crate::processor::transcode_ktx2(&bytes)
        .map_err(|e| anyhow::anyhow!("ktx2 transcode failed: {e:?}"))?;
    Ok((dds, true))
}

pub struct Ktx2Loader {
    supported_compressed_formats: CompressedImageFormats,
}

impl FromWorld for Ktx2Loader {
    fn from_world(world: &mut World) -> Self {
        let supported_compressed_formats = match world.get_resource::<RenderCapabilities>() {
            Some(capabilities) => CompressedImageFormats::from_features(capabilities.features()),
            None => CompressedImageFormats::NONE,
        };
        Self {
            supported_compressed_formats,
        }
    }
}

impl AssetLoader for Ktx2Loader {
    type Asset = Image;
    type Settings = ImageLoaderSettings;
    type Error = anyhow::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let (dds, transcoded) = prepare(bytes)?;
        if transcoded {
            if let Ok(mut results) = KTX2_RESULTS.lock() {
                results.insert(
                    load_context.asset_path().to_string(),
                    (Instant::now(), dds.clone()),
                );
            }
        }

        Ok(Image::from_buffer(
            #[cfg(debug_assertions)]
            load_context.path().display().to_string(),
            &dds,
            ImageType::Format(ImageFormat::Dds),
            self.supported_compressed_formats,
            settings.is_srgb,
            settings.sampler.clone(),
            settings.asset_usage,
        )?)
    }

    fn extensions(&self) -> &[&str] {
        &["ktx2"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UASTC_8: &[u8] = include_bytes!("../testdata/uastc_8.ktx2");

    #[test]
    fn ktx2_is_transcoded_and_stashed() {
        let (dds, transcoded) = prepare(UASTC_8.to_vec()).unwrap();
        assert!(dds.starts_with(DDS_MAGIC));
        assert!(transcoded);
    }

    #[test]
    fn cached_dds_passes_through() {
        let (dds, _) = prepare(UASTC_8.to_vec()).unwrap();
        let (bytes, transcoded) = prepare(dds.clone()).unwrap();
        assert_eq!(bytes, dds);
        assert!(!transcoded);
    }

    #[test]
    fn taking_a_result_empties_its_stash_entry() {
        // a key no other test uses: the stash is global and tests run in parallel
        let path = "ktx2-loader-test/taken.ktx2";
        KTX2_RESULTS
            .lock()
            .unwrap()
            .insert(path.to_owned(), (Instant::now(), b"DDS taken".to_vec()));

        assert_eq!(take_result(path).as_deref(), Some(&b"DDS taken"[..]));
        assert!(!KTX2_RESULTS.lock().unwrap().contains_key(path));
        assert!(take_result(path).is_none());
    }

    /// The review case on #53: a write queued behind slow repairs used to find its stash
    /// evicted. With the dds carried by the request, the worker writes it with nothing stashed.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn queued_cache_write_carries_its_dds() {
        use bevy::asset::AssetPath;
        use tokio::sync::mpsc::unbounded_channel;

        use crate::{AssetForProcessing, ProcessingAssetType};

        let path = "ktx2-loader-test/queued.ktx2";
        let (dds, transcoded) = prepare(UASTC_8.to_vec()).unwrap();
        assert!(transcoded);
        KTX2_RESULTS
            .lock()
            .unwrap()
            .insert(path.to_owned(), (Instant::now(), dds.clone()));

        // what `check_assets` does on the image's `LoadedWithDependencies` event
        let ktx2_dds = take_result(path);
        assert!(!KTX2_RESULTS.lock().unwrap().contains_key(path));

        let file = format!("dcl-ktx2-loader-test-{}-queued.ktx2", std::process::id());
        let cache_path = std::env::temp_dir().join(file);
        std::fs::write(&cache_path, UASTC_8).unwrap();

        let (req_sx, req_rx) = unbounded_channel();
        let (resp_sx, mut resp_rx) = unbounded_channel();
        let request = AssetForProcessing {
            cache_path: cache_path.to_string_lossy().into_owned(),
            base_path: AssetPath::from(path),
            ty: ProcessingAssetType::Ktx2Cache,
            ktx2_dds,
        };
        assert!(req_sx.send(request).is_ok());
        // the worker returns once the queue is drained and the sender is gone
        drop(req_sx);
        bevy::tasks::block_on(crate::processor::process_events(req_rx, resp_sx));

        let written = std::fs::read(&cache_path).unwrap();
        let _ = std::fs::remove_file(&cache_path);
        assert_eq!(written, dds);

        let response = resp_rx
            .try_recv()
            .expect("the worker answers the request")
            .expect("the cache write succeeds");
        // the dds is written, not sent back to the main thread
        assert!(response.ktx2_dds.is_none());
    }
}
