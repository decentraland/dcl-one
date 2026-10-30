{ pkgs, craneLib }:
let
  inherit (pkgs) lib;
  root = ../..;
  backend = root + /services/catalyst;
  manifest = builtins.fromTOML (builtins.readFile (backend + /Cargo.toml));
  # The domain crates beside src/; the library and SDK members are listed below.
  crates = map (m: backend + "/${m}") (lib.filter (m: !lib.hasPrefix "../" m) manifest.workspace.members);
  src = lib.fileset.toSource {
    inherit root;
    fileset = lib.fileset.unions ([
      (backend + /Cargo.toml)
      (backend + /Cargo.lock)
      (backend + /src)
      (root + /schemas)
      (root + /apps/dcl-scene-cli/Cargo.toml)
      (root + /apps/dcl-scene-cli/src/main.rs)
      (root + /library)
      (root + /.cargo/config.toml)
    ] ++ crates);
  };
  common = {
    inherit src;
    pname = "foundation-catalyst";
    version = manifest.workspace.package.version;
    cargoToml = backend + /Cargo.toml;
    cargoLock = backend + /Cargo.lock;
    postUnpack = ''sourceRoot="$sourceRoot/services/catalyst"'';
    strictDeps = true;
    doCheck = false;
    cargoExtraArgs = "--locked --bin catalyst";
    nativeBuildInputs = [ pkgs.pkg-config pkgs.cmake pkgs.clang pkgs.protobuf ];
    buildInputs = [ pkgs.openssl ];
    LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
    OPENSSL_NO_VENDOR = "1";
    RUSTY_V8_ARCHIVE = "${pkgs.callPackage ./librusty-v8.nix { }}";
  };
  cargoVendorDir = craneLib.vendorCargoDeps common;
  cargoArtifacts = craneLib.buildDepsOnly (common // { inherit cargoVendorDir; });
in craneLib.buildPackage (common // {
  inherit cargoArtifacts cargoVendorDir;
  passthru = { inherit cargoArtifacts cargoVendorDir; };
  meta.mainProgram = "catalyst";
})
