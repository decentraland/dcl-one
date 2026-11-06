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
    // the dds a `.ktx2` load produced (carried in `AssetForProcessing::ktx2_dds`), to be
    // written over the cached ktx2 bytes so the next load skips the transcode. nothing is
    // reprocessed or reloaded
    Ktx2Cache,
}

#[derive(Event, Clone)]
pub struct AssetForProcessing {
    pub cache_path: String,
    pub base_path: AssetPath<'static>,
    pub ty: ProcessingAssetType,
    /// For `Ktx2Cache`, the dds to write, taken out of `ktx2_loader::KTX2_RESULTS` when the
    /// request is queued so it can't be evicted while the write waits behind repairs. `None`
    /// for every other type, and once the processor has taken it for the write.
    pub ktx2_dds: Option<Vec<u8>>,
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
