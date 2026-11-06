pub mod engine;
pub use engine::ImageProcessingPlugin;
mod ktx2_loader;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::wasm_bindgen;

mod processor;

#[cfg(target_arch = "wasm32")]
use std::sync::{Arc, Mutex, OnceLock};
#[cfg(target_arch = "wasm32")]
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use bevy::{asset::AssetPath, prelude::*};

#[derive(Clone, Copy)]
pub enum ProcessingAssetType {
    Gltf,
    // a gltf that failed to load on an extension the loader lacks (draco geometry,
    // basisu textures); unlike `Gltf` the original asset never loaded, so on
    // success it is reloaded in place
    UnloadableGltf,
    Image,
    // the dds a `.ktx2` load produced (`ktx2_loader::KTX2_RESULTS`), to be written over the
    // cached ktx2 bytes so the next load skips the transcode. nothing is reprocessed or reloaded
    Ktx2Cache,
}

#[derive(Event, Clone)]
pub struct AssetForProcessing {
    pub cache_path: String,
    pub base_path: AssetPath<'static>,
    pub ty: ProcessingAssetType,
}

const KTX2_MAGIC: &[u8] = &[
    0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
];

/// [`bevy::gltf::GltfPlugin::with_image_transcoder`]: an embedded KTX2 image (a
/// `KHR_texture_basisu` image in a glb buffer view or a `data:` URI) becomes the BC7 dds a
/// standalone `.ktx2` loads as, through the same `processor::transcode_ktx2`. External `.ktx2`
/// images never get here: the glTF loader loads them by path, through `Ktx2Loader`. Anything
/// else, or a KTX2 the transcoder rejects, is left to the loader as it is.
pub fn transcode_gltf_image(bytes: &[u8], mime_type: &str) -> Option<(Vec<u8>, &'static str)> {
    if mime_type != "image/ktx2" && !bytes.starts_with(KTX2_MAGIC) {
        return None;
    }
    match processor::transcode_ktx2(bytes) {
        Ok(dds) => Some((dds, "image/vnd-ms.dds")),
        Err(e) => {
            warn!("embedded ktx2 transcode failed: {e:?}");
            None
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[allow(clippy::type_complexity)]
static CHANNELS: OnceLock<
    Arc<
        Mutex<(
            Option<(
                UnboundedSender<AssetForProcessing>,
                UnboundedReceiver<Result<AssetForProcessing, ()>>,
            )>,
            Option<(
                UnboundedReceiver<AssetForProcessing>,
                UnboundedSender<Result<AssetForProcessing, ()>>,
            )>,
        )>,
    >,
> = OnceLock::new();

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn image_processor_init() {
    use tokio::sync::mpsc::unbounded_channel;

    let (req_sx, req_rx) = unbounded_channel();
    let (resp_sx, resp_rx) = unbounded_channel();

    CHANNELS
        .set(Arc::new(Mutex::new((
            Some((req_sx, resp_rx)),
            Some((req_rx, resp_sx)),
        ))))
        .unwrap();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn image_processor_run() {
    let (req_rx, resp_sx) = CHANNELS.get().unwrap().lock().unwrap().1.take().unwrap();
    processor::process_events(req_rx, resp_sx).await
}
