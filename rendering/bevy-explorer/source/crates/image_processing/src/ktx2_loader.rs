//! Standalone `.ktx2` textures, as `KHR_texture_basisu` models reference them: Basis Universal
//! transcoded to BC7 on load, by the same `transcode_ktx2` the gltf repair uses. The dds a
//! transcode produces is kept in `KTX2_RESULTS` until `engine::check_assets` queues it as a
//! `ProcessingAssetType::Ktx2Cache` and the processor writes it over the cached ktx2 bytes, so
//! the next load reads a dds (`DDS ` magic) and skips the transcode. A transcode whose write
//! never comes (the image was dropped before it loaded, or it has no cache entry) is evicted
//! after `RESULT_LIFETIME`.
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

/// Transcoded dds bytes by asset path string, with when they were stashed, waiting to be written
/// over the cached ktx2.
type Ktx2Results = HashMap<String, (Instant, Vec<u8>)>;

pub(crate) static KTX2_RESULTS: LazyLock<Mutex<Ktx2Results>> = LazyLock::new(Default::default);

/// Far longer than a stashed transcode waits for its write, which goes ahead of every texture
/// compression in the processor's queue.
const RESULT_LIFETIME: Duration = Duration::from_secs(60);

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
}
