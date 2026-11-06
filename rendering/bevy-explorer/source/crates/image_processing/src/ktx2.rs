//! KTX2 (`KHR_texture_basisu`) textures: Basis Universal UASTC transcoded to BC7 on load, by the
//! same `transcode_ktx2` the gltf repair uses. [`install_ktx2_transcoder`] sets
//! [`transcode_ktx2_image`] as `bevy_image`'s KTX2 transcoder, so every KTX2 bevy decodes takes
//! this one route: a `.ktx2` loaded by path through bevy's `ImageLoader`, and an image embedded
//! in a glTF (glb buffer view or `data:` URI) through `Image::from_buffer`.
//!
//! A `.ktx2` loaded by path is cached: the dds its transcode produces is kept in `KTX2_RESULTS`
//! until `engine::check_assets` queues it as a `ProcessingAssetType::Ktx2Cache` and the processor
//! writes it over the cached ktx2 bytes, so the next load reads a dds (`DDS ` magic), which
//! `bevy_image` decodes as a dds without calling the transcoder. A transcode whose write never
//! comes (the image was dropped before it loaded, or it has no cache entry) is evicted after
//! `RESULT_LIFETIME`. An embedded KTX2 is transcoded on every load: the glTF holding it is not
//! rewritten in the cache.

use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};

use bevy::{
    asset::AssetPath,
    image::{ImageFormat, Ktx2Transcoded, TextureError},
    platform::time::Instant,
    prelude::*,
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

/// Whether `check_assets` takes an image loaded from `path` as a `.ktx2` to cache: an unlabelled
/// `.ktx2` path. A transcode for any other path is not stashed, as no cache write would take it.
pub(crate) fn is_cached_ktx2_path(path: &AssetPath<'_>) -> bool {
    path.label().is_none()
        && path
            .path()
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ktx2"))
}

/// Sets [`transcode_ktx2_image`] as the process's `bevy_image` KTX2 transcoder. A second call
/// keeps the first.
pub(crate) fn install_ktx2_transcoder() {
    bevy::image::set_ktx2_transcoder(Arc::new(transcode_ktx2_image));
}

/// `bevy_image`'s KTX2 transcoder (`bevy::image::Ktx2Transcoder`): a UASTC KTX2 becomes a BC7
/// dds, which bevy decodes with the `is_srgb`, sampler and asset usage the KTX2 was loaded with.
///
/// - Loaded by path (`path` is `Some`): a fresh transcode is stashed for the cache write when
///   the path is a `.ktx2` one. A KTX2 that can't be transcoded (not UASTC, malformed) fails to
///   load, as it did with the former `Ktx2Loader`.
/// - Embedded, or any other `Image::from_buffer` caller (`path` is `None`): a KTX2 that can't be
///   transcoded is left to bevy's own KTX2 decode, as the former glTF image transcoder left it.
///   bevy's own KTX2 LUTs, which are not UASTC, keep loading that way.
pub(crate) fn transcode_ktx2_image(
    bytes: &[u8],
    path: Option<&AssetPath<'_>>,
) -> Option<Ktx2Transcoded> {
    match crate::processor::transcode_ktx2(bytes) {
        Ok(dds) => {
            if let Some(path) = path.filter(|path| is_cached_ktx2_path(path)) {
                if let Ok(mut results) = KTX2_RESULTS.lock() {
                    results.insert(path.to_string(), (Instant::now(), dds.clone()));
                }
            }
            Some(Ok((dds, ImageFormat::Dds)))
        }
        Err(e) if path.is_some() => Some(Err(TextureError::TranscodeError(format!(
            "ktx2 transcode failed: {e:?}"
        )))),
        Err(e) => {
            debug!("embedded ktx2 not transcoded ({e:?}), left to bevy's decoder");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        asset::RenderAssetUsages,
        image::{CompressedImageFormats, ImageSampler, ImageType},
        render::render_resource::TextureFormat,
    };

    use super::*;

    const UASTC_8: &[u8] = include_bytes!("../testdata/uastc_8.ktx2");
    const DDS_MAGIC: &[u8] = b"DDS ";

    fn stashed(path: &str) -> bool {
        KTX2_RESULTS.lock().unwrap().contains_key(path)
    }

    fn unstash(path: &str) -> Option<Vec<u8>> {
        KTX2_RESULTS
            .lock()
            .unwrap()
            .remove(path)
            .map(|(_, dds)| dds)
    }

    /// `bytes` decoded by bevy as `image_type`, through the installed transcoder.
    fn decode(bytes: &[u8], image_type: ImageType, is_srgb: bool) -> Result<Image, TextureError> {
        install_ktx2_transcoder();
        Image::from_buffer(
            #[cfg(debug_assertions)]
            "test".to_owned(),
            bytes,
            image_type,
            CompressedImageFormats::BC,
            is_srgb,
            ImageSampler::nearest(),
            RenderAssetUsages::RENDER_WORLD,
        )
    }

    #[test]
    fn ktx2_is_transcoded_and_stashed() {
        let path = AssetPath::from("ktx2_tests/stashed.ktx2");
        let (dds, format) = transcode_ktx2_image(UASTC_8, Some(&path))
            .unwrap()
            .unwrap();
        assert!(dds.starts_with(DDS_MAGIC));
        assert!(matches!(format, ImageFormat::Dds));
        // the dds the cache write takes is the one bevy decodes
        assert_eq!(unstash("ktx2_tests/stashed.ktx2"), Some(dds));
    }

    #[test]
    fn only_cacheable_ktx2_paths_are_stashed() {
        let path = AssetPath::from("ktx2_tests/no_extension");
        assert!(transcode_ktx2_image(UASTC_8, Some(&path)).is_some());
        assert!(!stashed("ktx2_tests/no_extension"));
        let path = AssetPath::from("ktx2_tests/model.glb#Texture0");
        assert!(transcode_ktx2_image(UASTC_8, Some(&path)).is_some());
        assert!(!stashed("ktx2_tests/model.glb#Texture0"));
    }

    #[test]
    fn bad_ktx2_fails_by_path_and_is_left_to_bevy_embedded() {
        let path = AssetPath::from("ktx2_tests/bad.ktx2");
        assert!(matches!(
            transcode_ktx2_image(b"not a ktx2", Some(&path)),
            Some(Err(_))
        ));
        assert!(!stashed("ktx2_tests/bad.ktx2"));
        assert!(transcode_ktx2_image(b"not a ktx2", None).is_none());
    }

    #[test]
    fn cached_dds_passes_through() {
        let (dds, _) = transcode_ktx2_image(UASTC_8, None).unwrap().unwrap();
        // a dds where a ktx2 was declared (the cache entry of a `.ktx2` path) decodes as a dds
        let image = decode(&dds, ImageType::Extension("ktx2"), true).unwrap();
        assert_eq!(
            image.texture_descriptor.format,
            TextureFormat::Bc7RgbaUnormSrgb
        );
        assert_eq!((image.width(), image.height()), (8, 8));
    }

    #[test]
    fn embedded_ktx2_is_transcoded_for_the_gltf_loader() {
        // a glb buffer view or `data:` uri, as the glTF loader passes it to `Image::from_buffer`
        let image = decode(UASTC_8, ImageType::MimeType("image/ktx2"), true).unwrap();
        assert_eq!(
            image.texture_descriptor.format,
            TextureFormat::Bc7RgbaUnormSrgb
        );
        assert_eq!((image.width(), image.height()), (8, 8));
        // the caller's settings hold on the transcoded image
        assert_eq!(image.asset_usage, RenderAssetUsages::RENDER_WORLD);
        assert!(matches!(image.sampler, ImageSampler::Descriptor(_)));
        let linear = decode(UASTC_8, ImageType::MimeType("image/ktx2"), false).unwrap();
        assert_eq!(linear.texture_descriptor.format, TextureFormat::Bc7RgbaUnorm);
        // recognised by its magic when the mime type is missing or wrong
        assert!(decode(UASTC_8, ImageType::MimeType("application/octet-stream"), true).is_ok());
        // a ktx2 the transcoder rejects goes to bevy's decoder, which can't read it either
        assert!(decode(b"not a ktx2", ImageType::MimeType("image/ktx2"), true).is_err());
    }
}
