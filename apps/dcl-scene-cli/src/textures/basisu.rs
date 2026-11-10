//! The Basis Universal encoder, run in-process: build.rs embeds the official
//! single-threaded WASI build pinned in basisu-release.lock, compiled here
//! once per process and instantiated once per texture, each instance in its
//! own scratch directory, with the command line the basisu tool takes.
//!
//! WebAssembly rather than a native basisu because the wasm module encodes
//! byte-identically wherever it runs, while native builds differ from it (and
//! from each other) on about half of all images through floating-point code
//! generation: a texture's content hash must not depend on the machine that
//! deployed it. NaN canonicalization and deterministic relaxed SIMD keep that
//! true across CPU architectures too. One instance per texture, never basisu's
//! own threading: the threaded encoder is slower here and not deterministic.

use super::{ColorMode, Encoder};
use anyhow::{Context, Result, anyhow, bail};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use wasmtime::{Config, Engine, Linker, Module, Store};
use wasmtime_wasi::p1::{self, WasiP1Ctx};
use wasmtime_wasi::p2::pipe::MemoryOutputPipe;
use wasmtime_wasi::{FsPerms, I32Exit, WasiCtxBuilder};

include!(concat!(env!("OUT_DIR"), "/basisu_embed_data.rs"));

/// UASTC LDR 4x4 in a Zstandard-supercompressed KTX2 (the KTX2 default), RDO
/// on. Level 1 measured within 0.5 dB PSNR of the default level 2 at 44% less
/// encode time; level 0 visibly hurts normal maps.
pub const ARGS: [&str; 7] = [
    "-uastc",
    "-ktx2",
    "-uastc_level",
    "1",
    "-uastc_rdo_l",
    "1.0",
    "-quiet",
];

/// `-linear` writes a linear transfer function and uses linear error metrics;
/// `-normal_map` tunes for normals (linear metrics, no sRGB, no selector RDO).
fn mode_args(mode: ColorMode) -> &'static [&'static str] {
    match mode {
        ColorMode::Srgb => &[],
        ColorMode::Linear => &["-linear"],
        ColorMode::Normal => &["-normal_map", "-linear"],
    }
}

/// The guest path the scratch directory is mounted at.
const GUEST_DIR: &str = "/work";

pub struct WasmBasisu {
    engine: Engine,
    module: Module,
    id: String,
}

fn wasm_err(e: wasmtime::Error) -> anyhow::Error {
    anyhow!("{e:?}")
}

impl WasmBasisu {
    /// The process's one compiled encoder; compiling the 8 MB module is the
    /// expensive part, so it happens once.
    pub fn shared() -> Result<&'static WasmBasisu> {
        static SHARED: OnceLock<std::result::Result<WasmBasisu, String>> = OnceLock::new();
        SHARED
            .get_or_init(|| WasmBasisu::compile().map_err(|e| format!("{e:#}")))
            .as_ref()
            .map_err(|e| anyhow!("the embedded basisu encoder could not be loaded: {e}"))
    }

    fn compile() -> Result<WasmBasisu> {
        use std::io::Read;
        let mut wasm = Vec::with_capacity(RAW_LEN);
        flate2::read::DeflateDecoder::new(PACKED)
            .read_to_end(&mut wasm)
            .context("inflating the embedded basisu module")?;
        let mut config = Config::new();
        config
            .cranelift_nan_canonicalization(true)
            .relaxed_simd_deterministic(true);
        let engine = Engine::new(&config).map_err(wasm_err)?;
        let module = Module::new(&engine, &wasm).map_err(wasm_err)?;
        Ok(WasmBasisu {
            engine,
            module,
            id: encoder_id(),
        })
    }

    fn run(&self, dir: &Path, args: &[String]) -> Result<()> {
        let stderr = MemoryOutputPipe::new(64 * 1024);
        let mut wasi = WasiCtxBuilder::new();
        wasi.args(args)
            .stdout(MemoryOutputPipe::new(64 * 1024))
            .stderr(stderr.clone())
            .allow_blocking_current_thread(true);
        wasi.preopened_dir(dir, GUEST_DIR, FsPerms::ReadWrite)
            .map_err(wasm_err)?;
        let mut store: Store<WasiP1Ctx> = Store::new(&self.engine, wasi.build_p1());
        let mut linker: Linker<WasiP1Ctx> = Linker::new(&self.engine);
        p1::add_to_linker_sync(&mut linker, |cx| cx).map_err(wasm_err)?;
        let instance = linker
            .instantiate(&mut store, &self.module)
            .map_err(wasm_err)?;
        let start = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .map_err(wasm_err)?;
        let code = match start.call(&mut store, ()) {
            Ok(()) => 0,
            Err(e) => match e.downcast_ref::<I32Exit>() {
                Some(exit) => exit.0,
                None => bail!("basisu trapped: {e:?}"),
            },
        };
        if code != 0 {
            let said = String::from_utf8_lossy(&stderr.contents())
                .trim()
                .to_string();
            bail!("basisu exited with {code}: {said}");
        }
        Ok(())
    }
}

/// The module's sha256 and every argument: a different module or setting is
/// a different encoder, with its own cache entries.
fn encoder_id() -> String {
    format!(
        "basisu {VERSION} wasm sha256:{WASM_SHA256} args:{} srgb:[] linear:[{}] normal:[{}]",
        ARGS.join(" "),
        mode_args(ColorMode::Linear).join(" "),
        mode_args(ColorMode::Normal).join(" "),
    )
}

/// A per-encode scratch directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Scratch> {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir()
            .join("dcl-scene-cli-basisu")
            .join(format!(
                "{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        Ok(Scratch(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl Encoder for WasmBasisu {
    fn id(&self) -> &str {
        &self.id
    }

    fn encode(&self, png: &[u8], mode: ColorMode) -> Result<Vec<u8>> {
        let scratch = Scratch::new()?;
        std::fs::write(scratch.0.join("in.png"), png).context("writing the encoder input")?;
        let mut args: Vec<String> = vec!["basisu".into()];
        args.extend(ARGS.iter().map(|a| a.to_string()));
        args.extend(mode_args(mode).iter().map(|a| a.to_string()));
        args.extend([
            "-file".to_string(),
            format!("{GUEST_DIR}/in.png"),
            "-output_file".to_string(),
            format!("{GUEST_DIR}/out.ktx2"),
        ]);
        self.run(&scratch.0, &args)?;
        let out = std::fs::read(scratch.0.join("out.ktx2")).context("basisu wrote no KTX2 file")?;
        if !out.starts_with(b"\xABKTX 20\xBB\r\n\x1A\n") {
            bail!("basisu wrote something that is not a KTX2 file");
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_module_is_the_pinned_one() {
        use std::io::Read;
        let mut wasm = Vec::new();
        flate2::read::DeflateDecoder::new(PACKED)
            .read_to_end(&mut wasm)
            .unwrap();
        assert_eq!(wasm.len(), RAW_LEN);
        assert_eq!(super::super::encode::sha256_hex(&wasm), WASM_SHA256);
        assert!(encoder_id().contains(WASM_SHA256));
    }

    /// Runs the real module: slow (it compiles 8 MB of wasm), so ignored by
    /// default. `cargo test -p dcl-scene-cli textures::basisu -- --ignored`.
    #[test]
    #[ignore]
    fn encodes_byte_identically_run_to_run() {
        let img = image::RgbaImage::from_fn(64, 32, |x, y| {
            image::Rgba([(x * 4) as u8, (y * 8) as u8, ((x ^ y) * 3) as u8, 255])
        });
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let png = png.into_inner();
        let encoder = WasmBasisu::shared().unwrap();
        for mode in [ColorMode::Srgb, ColorMode::Linear, ColorMode::Normal] {
            let a = encoder.encode(&png, mode).unwrap();
            let b = encoder.encode(&png, mode).unwrap();
            assert_eq!(a, b, "{mode:?}");
            assert!(a.starts_with(b"\xABKTX 20\xBB"));
        }
    }
}
