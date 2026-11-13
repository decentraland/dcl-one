//! KTX2 (`KHR_texture_basisu`) textures: Basis Universal UASTC transcoded to BC7 on load, by the
//! same `transcode_ktx2` the gltf repair uses. [`install_ktx2_transcoder`] sets
//! [`transcode_ktx2_image`] as `bevy_image`'s KTX2 transcoder, so every KTX2 bevy decodes takes
//! this one route: a KTX2 loaded by path through bevy's `ImageLoader` (by its magic, whatever
//! the extension), and an image embedded in a glTF (glb buffer view or `data:` URI) through `Image::from_buffer`.
//!
//! A KTX2 loaded by path is cached, whatever its extension (`bevy_image` takes KTX2 by its
//! magic): the dds its transcode produces is kept in `KTX2_RESULTS` until `engine::check_assets`
//! queues it as a `ProcessingAssetType::Ktx2Cache` and the processor writes it over the cached
//! ktx2 bytes, so the next load reads a dds (`DDS ` magic), which `bevy_image` decodes as a dds
//! without calling the transcoder. A transcode whose write never
//! comes (the image was dropped before it loaded, or it has no cache entry) is evicted after
//! `RESULT_LIFETIME`.
//!
//! An embedded KTX2 arrives at its labelled path (`model.glb#Texture0`). Its transcode is noted
//! in `EMBEDDED_TRANSCODES`, and `engine::check_assets` queues the glTF holding it for a
//! `ProcessingAssetType::Gltf` pass, which transcodes the glTF's KTX2 images and writes it back
//! with dds, so the next load reads dds. One it can't transcode fails to load: glTF content never
//! reaches bevy's own KTX2 decode, which trusts the header's sizes and offsets.

use std::{
    collections::{HashMap, HashSet},
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

/// Labelled paths of embedded KTX2 images transcoded and not yet seen by `check_assets`, which
/// queues their glTF for the pass that writes them back as dds. An image dropped before it
/// loaded leaves its path here until it loads again.
static EMBEDDED_TRANSCODES: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);

/// Far longer than a stashed transcode waits for its write, which goes ahead of every texture
/// compression in the processor's queue.
const RESULT_LIFETIME: Duration = Duration::from_secs(60);

/// Drops the transcodes stashed longer than `RESULT_LIFETIME`.
pub(crate) fn evict_stale_results() {
    if let Ok(mut results) = KTX2_RESULTS.lock() {
        results.retain(|_, (stashed, _)| stashed.elapsed() < RESULT_LIFETIME);
    }
}

/// Whether a transcode for `path` is stashed for a cache write: an unlabelled path, which has a
/// cache entry of its own. A labelled image is part of another file and has none.
fn is_cacheable_path(path: &AssetPath<'_>) -> bool {
    path.label().is_none()
}

/// Whether a transcode of the image loaded from `path` is waiting for its cache write, which is
/// how `check_assets` tells a freshly transcoded KTX2 from an image already compressed.
pub(crate) fn has_waiting_transcode(path: &AssetPath<'_>) -> bool {
    KTX2_RESULTS
        .lock()
        .is_ok_and(|results| results.contains_key(&path.to_string()))
}

/// Whether the image loaded at the labelled `path` is an embedded KTX2 that was transcoded, and
/// so whether its glTF needs the pass that writes it back as dds; the note is taken.
pub(crate) fn take_embedded_transcode(path: &AssetPath<'_>) -> bool {
    path.label().is_some()
        && EMBEDDED_TRANSCODES
            .lock()
            .is_ok_and(|mut noted| noted.remove(&path.to_string()))
}

/// Sets [`transcode_ktx2_image`] as the process's `bevy_image` KTX2 transcoder. A second call
/// keeps the first.
pub(crate) fn install_ktx2_transcoder() {
    bevy::image::set_ktx2_transcoder(Arc::new(transcode_ktx2_image));
}

/// `bevy_image`'s KTX2 transcoder (`bevy::image::Ktx2Transcoder`): a UASTC KTX2 becomes a BC7
/// dds, which bevy decodes with the `is_srgb`, sampler and asset usage the KTX2 was loaded with.
///
/// - With a path: a fresh transcode is stashed for the cache write when the path is unlabelled
///   (a file of its own), and noted for its glTF's write-back when it is labelled (embedded in a
///   glTF). A KTX2 that can't be transcoded (not UASTC, malformed) fails to load.
/// - Without one (`Image::from_buffer` callers, bevy's own KTX2 LUTs, which are not UASTC): a
///   KTX2 that can't be transcoded is left to bevy's own decode.
///
/// A BC texture variant (`ipfs::bevy_variants`, BC1/BC5/BC7 in KTX2) is unwrapped to its dds
/// first, never transcoded, stashed or noted: its cache entry is the variant's, not this path's.
/// A BC KTX2 that doesn't unwrap goes on to the transcoder.
pub(crate) fn transcode_ktx2_image(
    bytes: &[u8],
    path: Option<&AssetPath<'_>>,
) -> Option<Ktx2Transcoded> {
    if let Some(Ok(dds)) = ipfs::bevy_variants::ktx2::bc_ktx2_dds(bytes) {
        return Some(Ok((dds, ImageFormat::Dds)));
    }
    match crate::processor::transcode_ktx2(bytes) {
        Ok(dds) => {
            match path {
                Some(path) if is_cacheable_path(path) => {
                    if let Ok(mut results) = KTX2_RESULTS.lock() {
                        results.insert(path.to_string(), (Instant::now(), dds.clone()));
                    }
                }
                Some(path) => {
                    if let Ok(mut noted) = EMBEDDED_TRANSCODES.lock() {
                        noted.insert(path.to_string());
                    }
                }
                None => {}
            }
            Some(Ok((dds, ImageFormat::Dds)))
        }
        Err(e) if path.is_some() => Some(Err(TextureError::TranscodeError(format!(
            "ktx2 transcode failed: {e:?}"
        )))),
        Err(e) => {
            debug!("ktx2 with no asset path not transcoded ({e:?}), left to bevy's decoder");
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
        decode_at(bytes, image_type, is_srgb, None)
    }

    /// [`decode`] at an asset path, as the glTF loader decodes an embedded image.
    fn decode_at(
        bytes: &[u8],
        image_type: ImageType,
        is_srgb: bool,
        path: Option<&AssetPath<'_>>,
    ) -> Result<Image, TextureError> {
        install_ktx2_transcoder();
        Image::from_buffer_at(
            #[cfg(debug_assertions)]
            "test".to_owned(),
            bytes,
            image_type,
            CompressedImageFormats::BC,
            is_srgb,
            ImageSampler::nearest(),
            RenderAssetUsages::RENDER_WORLD,
            path,
        )
    }

    #[test]
    fn ktx2_is_transcoded_and_stashed() {
        let path = AssetPath::from("ktx2_tests/stashed.ktx2");
        let (dds, format) = transcode_ktx2_image(UASTC_8, Some(&path)).unwrap().unwrap();
        assert!(dds.starts_with(DDS_MAGIC));
        assert!(matches!(format, ImageFormat::Dds));
        // the dds the cache write takes is the one bevy decodes
        assert_eq!(unstash("ktx2_tests/stashed.ktx2"), Some(dds));
    }

    #[test]
    fn ktx2_is_stashed_at_any_unlabelled_path() {
        // no extension (an NFT image, a URL texture) or another one: cached all the same
        for path in ["ktx2_tests/no_extension", "ktx2_tests/albedo.png"] {
            let asset_path = AssetPath::from(path);
            assert!(transcode_ktx2_image(UASTC_8, Some(&asset_path)).is_some());
            assert!(has_waiting_transcode(&asset_path));
            assert!(!take_embedded_transcode(&asset_path));
            assert!(unstash(path).is_some());
        }
        // part of another file: no cache entry of its own; noted once for its glTF's write-back
        let path = AssetPath::from("ktx2_tests/model.glb#Texture0");
        assert!(transcode_ktx2_image(UASTC_8, Some(&path)).is_some());
        assert!(!stashed("ktx2_tests/model.glb#Texture0"));
        assert!(take_embedded_transcode(&path));
        assert!(!take_embedded_transcode(&path));
    }

    #[test]
    fn bad_ktx2_fails_with_a_path_and_is_left_to_bevy_without_one() {
        // a file of its own, and one embedded in a glTF: both fail here
        for path in ["ktx2_tests/bad.ktx2", "ktx2_tests/bad.glb#Texture0"] {
            let asset_path = AssetPath::from(path);
            assert!(matches!(
                transcode_ktx2_image(b"not a ktx2", Some(&asset_path)),
                Some(Err(_))
            ));
            assert!(!stashed(path));
            assert!(!take_embedded_transcode(&asset_path));
        }
        // no path (bevy's own LUTs): bevy decodes it
        assert!(transcode_ktx2_image(b"not a ktx2", None).is_none());
    }

    #[test]
    fn cached_dds_passes_through() {
        let (dds, _) = transcode_ktx2_image(UASTC_8, None).unwrap().unwrap();
        // the cache entry read back decodes as a dds whatever its path declares
        for image_type in [
            ImageType::Extension("ktx2"),
            ImageType::Extension("png"),
            ImageType::MimeType("application/octet-stream"),
        ] {
            let image = decode(&dds, image_type, true).unwrap();
            assert_eq!(
                image.texture_descriptor.format,
                TextureFormat::Bc7RgbaUnormSrgb
            );
            assert_eq!((image.width(), image.height()), (8, 8));
            // a warm load keeps the caller's usage, as the cold (ktx2) load does
            assert_eq!(image.asset_usage, RenderAssetUsages::RENDER_WORLD);
        }
    }

    #[test]
    fn embedded_ktx2_is_transcoded_for_the_gltf_loader() {
        // a glb buffer view or `data:` uri, at the labelled path the glTF loader decodes it at
        let path = AssetPath::from("ktx2_tests/embedded.glb#Texture0");
        let image = decode_at(
            UASTC_8,
            ImageType::MimeType("image/ktx2"),
            true,
            Some(&path),
        )
        .unwrap();
        assert!(take_embedded_transcode(&path));
        assert_eq!(
            image.texture_descriptor.format,
            TextureFormat::Bc7RgbaUnormSrgb
        );
        assert_eq!((image.width(), image.height()), (8, 8));
        // the caller's settings hold on the transcoded image
        assert_eq!(image.asset_usage, RenderAssetUsages::RENDER_WORLD);
        assert!(matches!(image.sampler, ImageSampler::Descriptor(_)));
        let linear = decode(UASTC_8, ImageType::MimeType("image/ktx2"), false).unwrap();
        assert_eq!(
            linear.texture_descriptor.format,
            TextureFormat::Bc7RgbaUnorm
        );
        // recognised by its magic when the mime type is missing or wrong
        assert!(decode(
            UASTC_8,
            ImageType::MimeType("application/octet-stream"),
            true
        )
        .is_ok());
        // a ktx2 the transcoder rejects fails as glTF content, never reaching bevy's decoder
        let rejected = decode_at(
            b"\xABKTX 20\xBB\r\n\x1A\nnot uastc",
            ImageType::MimeType("image/ktx2"),
            true,
            Some(&path),
        );
        assert!(matches!(rejected, Err(TextureError::TranscodeError(_))));
    }
}
