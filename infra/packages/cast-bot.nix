# `cast-bot`, the Cast presenter's LiveKit participant (services/catalyst/cast-bot): its own
# workspace and lock, built apart from the node so a libwebrtc problem never breaks `catalyst`
# (docs/adr/2026-11-17-cast-presenter-bot-process.md). It links the Explorer's LiveKit SDK, built
# from the same patched source, and the same prebuilt libwebrtc the Explorer's package takes.
{ pkgs, craneLib }:
let
  inherit (pkgs) lib stdenv;
  root = ../..;
  dir = root + /services/catalyst/cast-bot;
  manifest = builtins.fromTOML (builtins.readFile (dir + /Cargo.toml));

  # The lock names the SDK as git commits of this repository's patched source; the commits live
  # on refs a clone does not fetch. The vendored crates come from the tree the patch series builds
  # (patched-source.nix), checked against that rev, as the Explorer's package vendors them.
  patchedSource = import ./patched-source.nix { inherit pkgs; };
  sdk = patchedSource.patched "client-sdk-rust";
  sdkSource = "git+https://github.com/decentraland/dcl-one?rev=${sdk.rev}#${sdk.rev}";

  # The release the build script of webrtc-sys downloads (WEBRTC_TAG in
  # webrtc-sys/build/src/lib.rs of the patched SDK; preBuild checks it). Each archive is
  # BSD-3-Clause WebRTC with its third-party notices inside (LICENSE.md). The hashes are the
  # digests GitHub lists for these archives in webrtc-a545de4-prefixed, which -prefixed-2 carries
  # unchanged (it rebuilt only the Windows x64 archive).
  webrtcTag = "webrtc-a545de4-prefixed-2";
  webrtcTriple = {
    x86_64-linux = { triple = "linux-x64"; hash = "sha256-b9SHFy2suLTz044/33AwCbQTI2NL8Bnfg13vLTqRZ10="; };
    aarch64-linux = { triple = "linux-arm64"; hash = "sha256-3hPA3mNN0qGGHkT/PEJNSERrEM54a+LNsJu7axfkCuE="; };
    aarch64-darwin = { triple = "mac-arm64"; hash = "sha256-aOnLHLFsjRUeXUZSgF7YotSXwOwu2GRhSdsmfCQRETk="; };
  }.${stdenv.hostPlatform.system} or (throw "cast-bot: no libwebrtc prebuilt for ${stdenv.hostPlatform.system}");
  # On x86_64-linux the archive is the store path the Explorer's `webrtc` (bevy-explorer.nix) fetches.
  webrtc = with webrtcTriple; pkgs.runCommand "livekit-${webrtcTag}" {
    nativeBuildInputs = [ pkgs.unzip ];
    archive = pkgs.fetchurl {
      name = "${webrtcTag}-${triple}-release.zip";
      url = "https://github.com/robtfm/client-sdk-rust/releases/download/${webrtcTag}/webrtc-${triple}-release.zip";
      inherit hash;
      meta.sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
    };
  } ''
    unzip -q $archive
    mv ${triple}-release $out
  '';

  # Linux compiles the C++ bridge with the clang the Explorer's package uses (21), which the
  # hermetic libc++ inside libwebrtc requires.
  crane = if stdenv.hostPlatform.isLinux
    then craneLib.overrideScope (_: _: { stdenvSelector = _: pkgs.llvmPackages_21.stdenv; })
    else craneLib;

  common = {
    src = lib.fileset.toSource {
      root = dir;
      fileset = lib.fileset.unions [ (dir + /Cargo.toml) (dir + /Cargo.lock) (dir + /build.rs) (dir + /src) ];
    };
    pname = manifest.package.name;
    inherit (manifest.package) version;
    strictDeps = true;
    doCheck = false;
    cargoExtraArgs = "--locked --bin cast-bot";
    nativeBuildInputs = [ pkgs.pkg-config ];
    # webrtc-sys reads only glib's headers on Linux; what it would dlopen (X11, DRM) is for desktop
    # capture, which cast-bot does not use.
    buildInputs = lib.optionals stdenv.hostPlatform.isLinux [ pkgs.glib ];
    LK_CUSTOM_WEBRTC = "${webrtc}";
    preBuild = ''
      grep -q 'WEBRTC_TAG: &str = "${webrtcTag}"' ${sdk}/webrtc-sys/build/src/lib.rs \
        || { echo "the patched SDK downloads another libwebrtc than ${webrtcTag}: update cast-bot.nix" >&2; exit 1; }
    '';
  };
  cargoVendorDir = crane.vendorCargoDeps (common // {
    overrideVendorGitCheckout = packages: fetched:
      let source = (lib.head packages).source; in
      lib.throwIf (source != sdkSource) "cast-bot: ${source} is not the patched SDK (${sdkSource})"
        (patchedSource.vendoredCrates crane.craneUtils fetched sdk packages);
  });
  cargoArtifacts = crane.buildDepsOnly (common // { inherit cargoVendorDir; });
  # The test executables, built here with the package's toolchain and libwebrtc and run outside
  # the sandbox, where WebRTC can reach a livekit-server (.github/workflows/cast-presenter.yml):
  # bin/test-cast-bot (unit tests) and bin/test-livekit (tests/livekit.rs; CAST_BOT_BIN names
  # the bot it drives).
  tests = crane.mkCargoDerivation (common // {
    inherit cargoArtifacts cargoVendorDir;
    pname = "${manifest.package.name}-tests";
    src = lib.fileset.toSource {
      root = dir;
      fileset = lib.fileset.unions [ (dir + /Cargo.toml) (dir + /Cargo.lock) (dir + /build.rs) (dir + /src) (dir + /tests) ];
    };
    nativeBuildInputs = common.nativeBuildInputs ++ [ pkgs.jq ];
    doInstallCargoArtifacts = false;
    buildPhaseCargoCommand = ''
      cargo test --locked --release --no-run --message-format json-render-diagnostics > test-build.json
    '';
    installPhaseCommand = ''
      mkdir -p $out/bin
      jq -r 'select(.reason == "compiler-artifact" and .profile.test and .executable != null)
        | "\(.target.name) \(.executable)"' test-build.json \
        | while read -r name exe; do install -m755 "$exe" "$out/bin/test-$name"; done
      test -x $out/bin/test-livekit
      removeReferencesToVendoredSources "$out" "${webrtc}"
    '';
  });
in crane.buildPackage (common // {
  inherit cargoArtifacts cargoVendorDir;
  # The bridge keeps libwebrtc's header paths (__FILE__ in its checks); the 1 GB archive stays out
  # of the runtime closure, as crane keeps the vendored sources out. libwebrtc's notices, which a
  # binary distribution reproduces, go beside the binary.
  postInstall = ''
    removeReferencesToVendoredSources "$out" "${webrtc}"
    install -Dm644 ${webrtc}/LICENSE.md $out/share/doc/cast-bot/libwebrtc-LICENSE.md
  '';
  passthru = { inherit cargoArtifacts cargoVendorDir webrtc sdk tests; };
  meta.mainProgram = "cast-bot";
})
