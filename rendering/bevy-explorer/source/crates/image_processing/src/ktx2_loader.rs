//! Standalone `.ktx2` textures, as `KHR_texture_basisu` models reference them: Basis Universal
//! transcoded to BC7 on load, by the same `transcode_ktx2` the gltf repair uses. The dds a
//! transcode produces is kept in `KTX2_RESULTS` until `engine::check_assets` queues it as a
//! `ProcessingAssetType::Ktx2Cache` and the processor writes it over the cached ktx2 bytes, so
//! the next load reads a dds (`DDS ` magic) and skips the transcode. A ktx2 that already holds
//! BC7 is only unwrapped and not written back: the cache would only grow.

use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};

use bevy::{
    asset::{io::Reader, AssetLoader, LoadContext},
    image::{CompressedImageFormats, ImageFormat, ImageLoaderSettings, ImageType},
    prelude::*,
    render::renderer::RenderCapabilities,
};

/// Transcoded dds bytes by asset path string, waiting to be written over the cached ktx2.
pub(crate) static KTX2_RESULTS: LazyLock<Mutex<HashMap<String, Vec<u8>>>> =
    LazyLock::new(Default::default);

const DDS_MAGIC: &[u8] = b"DDS ";

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

        let dds = if bytes.starts_with(DDS_MAGIC) {
            // a previous load's transcode, written over the ktx2 bytes in the cache
            bytes
        } else {
            // vkFormat, at byte 12 of the header: 145/146 are BC7 already, only unwrapped
            let bc7 = bytes
                .get(12..16)
                .and_then(|f| <[u8; 4]>::try_from(f).ok())
                .is_some_and(|f| matches!(u32::from_le_bytes(f), 145 | 146));
            let dds = crate::processor::transcode_ktx2(&bytes)
                .map_err(|e| anyhow::anyhow!("ktx2 transcode failed: {e:?}"))?;
            if !bc7 {
                if let Ok(mut results) = KTX2_RESULTS.lock() {
                    results.insert(load_context.asset_path().to_string(), dds.clone());
                }
            }
            dds
        };

        Ok(Image::from_buffer(
            #[cfg(debug_assertions)]
            "a ktx2 texture".to_owned(),
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
