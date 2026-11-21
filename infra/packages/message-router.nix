# `catalyst-message-router`, the community chat message router's LiveKit participant
# (services/catalyst/message-router, docs/adr/2026-11-18-community-chat-message-router.md). A
# member of the catalyst workspace built apart from `catalyst`, so a libwebrtc problem never
# breaks the node. Like cast-bot.nix it compiles the Explorer's patched LiveKit SDK against the
# prebuilt libwebrtc of the SDK's WEBRTC_TAG, with clang 21 on Linux; it takes that archive,
# the patched tree and the tag check from cast-bot, and the workspace source from catalyst.
{ pkgs, craneLib, catalyst, cast-bot }:
let
  inherit (pkgs) lib stdenv;
  inherit (cast-bot) webrtc webrtcTag sdk sdkSource;
  patchedSource = import ./patched-source.nix { inherit pkgs; };
  crane = if stdenv.hostPlatform.isLinux
    then craneLib.overrideScope (_: _: { stdenvSelector = _: pkgs.llvmPackages_21.stdenv; })
    else craneLib;

  common = catalyst.workspace // {
    pname = "catalyst-message-router";
    strictDeps = true;
    doCheck = false;
    # The package and feature here too, so the dependency build compiles the router's
    # dependencies alone, not the node's (V8, OpenSSL).
    cargoExtraArgs = "--locked -p catalyst-message-router --features livekit --bin catalyst-message-router";
    # cmake for aws-lc-sys (the rustls provider of the SDK's HTTP client).
    nativeBuildInputs = [ pkgs.pkg-config pkgs.cmake ];
    buildInputs = lib.optionals stdenv.hostPlatform.isLinux [ pkgs.glib ];
    LK_CUSTOM_WEBRTC = "${webrtc}";
    preBuild = ''
      grep -q 'WEBRTC_TAG: &str = "${webrtcTag}"' ${sdk}/webrtc-sys/build/src/lib.rs \
        || { echo "the patched SDK downloads another libwebrtc than ${webrtcTag}: update cast-bot.nix" >&2; exit 1; }
    '';
  };
  # The lock's git source fetched by catalyst's hash; its crates come from the tree the patch
  # series builds, checked against that rev, as cast-bot vendors them.
  cargoVendorDir = crane.vendorCargoDeps (common // {
    inherit (catalyst.vendorGit) outputHashes;
    overrideVendorGitCheckout = packages: fetched:
      let source = (lib.head packages).source; in
      lib.throwIf (source != sdkSource) "message-router: ${source} is not the patched SDK (${sdkSource})"
        (patchedSource.vendoredCrates crane.craneUtils fetched sdk packages);
  });
  cargoArtifacts = crane.buildDepsOnly (common // { inherit cargoVendorDir; });
in crane.buildPackage (common // {
  inherit cargoArtifacts cargoVendorDir;
  # As cast-bot: libwebrtc stays out of the runtime closure and its notices go beside the binary.
  postInstall = ''
    removeReferencesToVendoredSources "$out" "${webrtc}"
    install -Dm644 ${webrtc}/LICENSE.md $out/share/doc/catalyst-message-router/libwebrtc-LICENSE.md
  '';
  passthru = { inherit cargoArtifacts cargoVendorDir; };
  meta.mainProgram = "catalyst-message-router";
})
