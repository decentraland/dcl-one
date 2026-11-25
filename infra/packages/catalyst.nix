{ pkgs, craneLib }:
let
  inherit (pkgs) lib;
  root = ../..;
  backend = root + /services/catalyst;
  manifest = builtins.fromTOML (builtins.readFile (backend + /Cargo.toml));
  # The domain crates beside src/; the library and SDK members are listed below.
  crates = map (m: backend + "/${m}") (lib.filter (m: !lib.hasPrefix "../" m) manifest.workspace.members);
  library = root + /library;
  # The workspace member glob `../../library/dcl-*`.
  libraryCrates = map (name: library + "/${name}") (builtins.attrNames
    (lib.filterAttrs (name: type: type == "directory" && lib.hasPrefix "dcl-" name) (builtins.readDir library)));
  src = lib.fileset.toSource {
    inherit root;
    fileset = lib.fileset.unions ([
      (backend + /Cargo.toml)
      (backend + /Cargo.lock)
      (backend + /src)
      (root + /schemas)
      # Embedded by the events fallback-image route.
      (root + /apps/ui3/public/assets/event-default.webp)
      (root + /apps/dcl-scene-cli/Cargo.toml)
      (root + /apps/dcl-scene-cli/src/main.rs)
      # All that the build reads under library/, beside the crates. A change to any other library
      # file keeps this derivation, so the image job does not compile the backend again. A new
      # compile-time read of another library file fails this build: add its path here.
      (library + /vendor/rusty_enet)
      (library + /protocol/proto)
      (lib.fileset.fileFilter (file: file.hasExt "json") (library + /catalyst-api/openapi))
      (root + /.cargo/config.toml)
    ] ++ crates ++ libraryCrates);
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
  # The lock's one git source: the message router's LiveKit SDK, the commit of this repository
  # that rendering/bevy-explorer/patches/client-sdk-rust/source.toml names. With its hash crane
  # fetches that commit alone instead of every ref of the repository. The commit holds only the
  # patched source's `paths` while its root Cargo.toml still lists upstream's members, so its
  # crates are vendored without loading that workspace (patched-source.nix `vendoredCrates`, as
  # bevy-explorer.nix and cast-bot.nix do for the same commit). They are copied from the plain git fetch (`fetched.src`): building
  # crane's own checkout (`fetched`) runs `cargo metadata` on that workspace and fails.
  # `catalyst` does not compile them; the vendor directory only has to hold them. scene-cli.nix
  # vendors the same lock with these arguments.
  patchedSource = import ./patched-source.nix { inherit pkgs; };
  vendorGit = {
    outputHashes."git+https://github.com/decentraland/dcl-one?rev=b2f8bc84000333ea843578959c9d11caccaa9474#b2f8bc84000333ea843578959c9d11caccaa9474" =
      "sha256-pu+EoXyqMEQ/N5Wx9r4x2K/3o6HGBhZtDcg7GTP26Vk=";
    overrideVendorGitCheckout = crates: fetched:
      patchedSource.vendoredCrates craneLib.craneUtils fetched fetched.src crates;
  };
  cargoVendorDir = craneLib.vendorCargoDeps (common // vendorGit);
  cargoArtifacts = craneLib.buildDepsOnly (common // { inherit cargoVendorDir; });
in craneLib.buildPackage (common // {
  inherit cargoArtifacts cargoVendorDir;
  # `workspace`: what message-router.nix builds its member of this workspace from.
  passthru = {
    inherit cargoArtifacts cargoVendorDir vendorGit;
    workspace = { inherit (common) src version cargoToml cargoLock postUnpack; };
  };
  meta.mainProgram = "catalyst";
})
