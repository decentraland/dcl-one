use std::io::Result;
use std::path::PathBuf;

/// The rfc4 comms packet (`decentraland.kernel.comms.rfc4`), compiled from the one proto source
/// (`library/protocol/proto`) without protoc, as the comms crate compiles its own; src/proto.rs
/// includes it from OUT_DIR.
fn main() -> Result<()> {
    // libwebrtc's Objective-C codec factories are categories the macOS linker drops unless every
    // object of the archive is loaded (the Explorer passes the same flag in its .cargo/config.toml).
    if std::env::var("CARGO_FEATURE_LIVEKIT").is_ok()
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos")
    {
        println!("cargo:rustc-link-arg=-ObjC");
    }
    println!("cargo:rerun-if-env-changed=DCL_ONE_ROOT");
    let root = PathBuf::from(std::env::var("DCL_ONE_ROOT").expect("DCL_ONE_ROOT"));
    let include = root.join("library/protocol/proto");
    let file = include.join("decentraland/kernel/comms/rfc4/comms.proto");
    println!("cargo:rerun-if-changed={}", file.display());
    let mut compiler = protox::Compiler::new([&include]).map_err(std::io::Error::other)?;
    compiler.include_imports(true);
    compiler
        .open_file(&file)
        .map_err(|e| std::io::Error::other(format!("{e:?}")))?;
    prost_build::Config::new().compile_fds(compiler.file_descriptor_set())
}
