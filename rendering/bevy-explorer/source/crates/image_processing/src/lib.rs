#![forbid(unsafe_code)]

pub mod engine;
pub use engine::ImageProcessingPlugin;
mod ktx2;
mod satellite_tile;
pub use satellite_tile::{satellite_tile_jpeg, validate_satellite_tile_path};
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
    // the dds a ktx2 load by path produced (`ktx2::KTX2_RESULTS`), to be written over the
    // cached ktx2 bytes so the next load skips the transcode. nothing is reprocessed or reloaded
    Ktx2Cache,
}

#[derive(Event, Clone)]
pub struct AssetForProcessing {
    pub cache_path: String,
    pub base_path: AssetPath<'static>,
    pub ty: ProcessingAssetType,
}

/// What the image processor thread takes from the engine.
pub(crate) enum ProcessorRequest {
    Asset(AssetForProcessing),
    /// A satellite tile's KTX2 bytes, answered with its JPEG. The web engine has no compute
    /// threads, so this runs on the processor worker; the native engine converts tiles on its
    /// compute pool instead.
    #[cfg(target_arch = "wasm32")]
    SatelliteTile(
        Vec<u8>,
        tokio::sync::oneshot::Sender<Result<Vec<u8>, &'static str>>,
    ),
}

#[cfg(target_arch = "wasm32")]
static CHANNELS: OnceLock<
    Arc<
        Mutex<(
            Option<(
                UnboundedSender<ProcessorRequest>,
                UnboundedReceiver<Result<AssetForProcessing, ()>>,
            )>,
            Option<(
                UnboundedReceiver<ProcessorRequest>,
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
