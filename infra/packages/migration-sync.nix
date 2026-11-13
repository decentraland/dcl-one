{ pkgs, craneLib, catalyst }:
let
  root = ../..;
  backend = root + /services/catalyst;
  manifest = builtins.fromTOML (builtins.readFile (backend + /Cargo.toml));
  common = {
    # Share the workspace source, but build no Catalyst runtime or domain crate.
    src = catalyst.src;
    pname = "foundation-migration-sync";
    version = manifest.workspace.package.version;
    cargoToml = backend + /Cargo.toml;
    cargoLock = backend + /Cargo.lock;
    postUnpack = ''sourceRoot="$sourceRoot/services/catalyst"'';
    strictDeps = true;
    doCheck = false;
    cargoExtraArgs = "--locked -p dcl-migration-sync --bin dcl-migration-sync";
    nativeBuildInputs = [ pkgs.pkg-config ];
    buildInputs = [ pkgs.openssl ];
    OPENSSL_NO_VENDOR = "1";
  };
  cargoVendorDir = craneLib.vendorCargoDeps common;
  cargoArtifacts = craneLib.buildDepsOnly (common // { inherit cargoVendorDir; });
in craneLib.buildPackage (common // {
  inherit cargoArtifacts cargoVendorDir;
  meta.mainProgram = "dcl-migration-sync";
})
