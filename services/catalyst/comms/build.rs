use std::io::Result;
use std::path::{Path, PathBuf};

/// The repository root, from the [env] table of the root .cargo/config.toml.
fn root() -> PathBuf {
    PathBuf::from(std::env::var("DCL_ONE_ROOT").expect("DCL_ONE_ROOT"))
}

/// The Pulse and archipelago wire messages, compiled from the one proto source
/// (`library/protocol/proto`) without protoc: protox parses, prost-build emits
/// one module per package into OUT_DIR (`decentraland.common.rs`,
/// `decentraland.pulse.rs`, `decentraland.kernel.comms.v3.rs`), which
/// src/protocol/pulse/mod.rs includes.
fn main() -> Result<()> {
    println!("cargo:rerun-if-env-changed=DCL_ONE_ROOT");
    let include = root().join("library/protocol/proto");
    println!("cargo:rerun-if-changed={}", include.display());
    let files: Vec<PathBuf> = [
        "decentraland/common/auth.proto",
        "decentraland/common/options.proto",
        "decentraland/common/vectors.proto",
        "decentraland/kernel/comms/v3/archipelago.proto",
        "decentraland/pulse/pulse_client.proto",
        "decentraland/pulse/pulse_clusters.proto",
        "decentraland/pulse/pulse_server.proto",
        "decentraland/pulse/pulse_shared.proto",
    ]
    .iter()
    .map(|f| include.join(f))
    .collect();
    let fds = compile_protos(&files, &include)?;
    prost_build::Config::new().compile_fds(fds)
}

fn protox_error(e: protox::Error) -> std::io::Error {
    std::io::Error::other(format!("{e:?}"))
}

/// What `protoc --include_imports -o` produces, without protoc; the google
/// resolver serves `google/protobuf/descriptor.proto` for the field options.
fn compile_protos(files: &[PathBuf], include: &Path) -> Result<prost_types::FileDescriptorSet> {
    let mut resolver = protox::file::ChainFileResolver::new();
    resolver.add(protox::file::IncludeFileResolver::new(
        include.to_path_buf(),
    ));
    resolver.add(protox::file::GoogleFileResolver::new());
    let mut compiler = protox::Compiler::with_file_resolver(resolver);
    compiler.include_imports(true);
    compiler.open_files(files).map_err(protox_error)?;
    Ok(compiler.file_descriptor_set())
}
