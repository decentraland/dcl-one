# `catalyst-message-router`, the community chat message router's LiveKit participant
# (services/catalyst/message-router, docs/adr/2026-11-18-community-chat-message-router.md). A
# member of the catalyst workspace built apart from `catalyst`, so a libwebrtc problem never
# breaks the node. It takes cast-bot's libwebrtc build (archive, patched SDK, clang 21 crane, tag
# check, notices) and catalyst's workspace source.
{ pkgs, catalyst, cast-bot, ... }:
let
  inherit (pkgs) lib stdenv;
  inherit (cast-bot) webrtc sdk sdkSource;
  inherit (cast-bot.libwebrtc) crane;
  patchedSource = import ./patched-source.nix { inherit pkgs; };

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
    inherit (cast-bot.libwebrtc) preBuild;
  };
  # The lock's git source fetched by catalyst's hash; its crates come from the tree the patch
  # series builds, checked against that rev, as cast-bot vendors them. Assumes the workspace's
  # only git dependency is the patched SDK; another one throws here.
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
  postInstall = cast-bot.libwebrtc.postInstall "catalyst-message-router";
  passthru = { inherit cargoArtifacts cargoVendorDir; };
  meta.mainProgram = "catalyst-message-router";
})
