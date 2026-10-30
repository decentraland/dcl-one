//! Standalone `.ktx2` textures, as `KHR_texture_basisu` models reference them: Basis Universal
//! transcoded to BC7 on load, by the same `transcode_ktx2` the gltf repair uses. Unlike a PNG's
//! BC7, the result isn't written back to the cache, so every load transcodes.

use bevy::{
    asset::{io::Reader, AssetLoader, LoadContext},
    image::{CompressedImageFormats, ImageFormat, ImageLoaderSettings, ImageType},
    prelude::*,
    render::renderer::RenderCapabilities,
};

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
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let dds = crate::processor::transcode_ktx2(&bytes)
            .map_err(|e| anyhow::anyhow!("ktx2 transcode failed: {e:?}"))?;
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
