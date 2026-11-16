{ pkgs, craneLib }:
let
  inherit (pkgs) lib stdenv;
  root = ../..;
  backend = root + /services/catalyst;
  app = root + /apps/dcl-scene-cli;
  manifest = builtins.fromTOML (builtins.readFile (app + /Cargo.toml));

  # The `key = value` lines of a release lock (abgen-release.lock,
  # livekit-release.lock): the pins build.rs downloads and verifies in a cargo
  # build. Here the same pins are fetched as fixed-output derivations and laid
  # out as the cache build.rs looks in first, so the build stays offline and the
  # binary embeds the same bytes and reports the same versions.
  lockOf = file: lib.listToAttrs (lib.concatMap
    (line:
      let m = builtins.match "([^#=[:space:]][^=[:space:]]*)[[:space:]]*=[[:space:]]*(.*[^[:space:]])[[:space:]]*" line;
      in lib.optional (m != null) (lib.nameValuePair (builtins.elemAt m 0) (builtins.elemAt m 1)))
    (lib.splitString "\n" (builtins.readFile file)));
  fill = vars: template:
    lib.replaceStrings (map (k: "{${k}}") (lib.attrNames vars)) (lib.attrValues vars) template;
  # One pinned archive, unpacked to <out>/<version>/<key>/dist with `exe` at its top.
  embedCache = { name, lock, key, vars, exe }:
    let
      archive = pkgs.fetchurl {
        url = fill ({ inherit (lock) version; } // vars) lock.url;
        sha256 = lock.${key};
        meta.sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
      };
    in pkgs.runCommand "${name}-embed-cache-${lock.version}" { passthru = { inherit archive; }; } ''
      mkdir -p stage "$out/${lock.version}/${key}"
      tar -xzf ${archive} -C stage
      found=$(find stage -type f -name ${exe} | head -n 1)
      test -n "$found"
      chmod 755 "$found"
      mv "$(dirname "$found")" "$out/${lock.version}/${key}/dist"
    '';

  targets = {
    x86_64-linux = { abgen = "x86_64-unknown-linux-gnu"; livekit = "linux_amd64.tar.gz"; };
    aarch64-linux = { abgen = "aarch64-unknown-linux-gnu"; livekit = "linux_arm64.tar.gz"; };
  }.${stdenv.hostPlatform.system};
  abgenLock = lockOf (app + /abgen-release.lock);
  livekitLock = lockOf (app + /livekit-release.lock);
  abgenCache = embedCache {
    name = "abgen";
    lock = abgenLock;
    key = targets.abgen;
    vars.target = targets.abgen;
    exe = "abgen";
  };
  livekitCache = embedCache {
    name = "livekit-server";
    lock = livekitLock;
    key = targets.livekit;
    vars = { asset = targets.livekit; semver = lib.removePrefix "v" livekitLock.version; };
    exe = "livekit-server";
  };

  src = lib.fileset.toSource {
    inherit root;
    fileset = lib.fileset.unions [
      # The workspace: every member's manifest has to resolve; the scene CLI
      # and the library crates it links are what compiles.
      backend
      app
      (root + /library/dcl-auth)
      (root + /library/dcl-http)
      (root + /library/dcl-livekit-token)
      (root + /library/dcl-preview-tunnel)
      (root + /library/dcl-preview-tunnel-protocol)
      (root + /library/dcl-scene-authoring)
      (root + /library/dcl-scene-build)
      (root + /library/protocol/proto)
      # A path dependency of the node's crates: its manifest has to load.
      (root + /library/vendor/rusty_enet)
      # The preview pages' shared sheets (src/start/chrome.rs).
      (root + /apps/ui3/src/atoms/primitives.css)
      (root + /apps/ui3/src/atoms/button.css)
      (root + /apps/ui3/src/components/surface.css)
      (root + /.cargo/config.toml)
    ];
  };
  common = {
    inherit src;
    pname = "dcl-scene-cli";
    version = manifest.package.version;
    cargoToml = app + /Cargo.toml;
    cargoLock = backend + /Cargo.lock;
    postUnpack = ''sourceRoot="$sourceRoot/services/catalyst"'';
    strictDeps = true;
    doCheck = false;
    cargoExtraArgs = "--locked -p dcl-scene-cli --bin dcl-scene-cli";
    nativeBuildInputs = [ pkgs.pkg-config pkgs.cmake pkgs.clang ];
    LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
    ABGEN_EMBED_CACHE = "${abgenCache}";
    LIVEKIT_EMBED_CACHE = "${livekitCache}";
  };
  cargoVendorDir = craneLib.vendorCargoDeps common;
  cargoArtifacts = craneLib.buildDepsOnly (common // { inherit cargoVendorDir; });
in craneLib.buildPackage (common // {
  inherit cargoArtifacts cargoVendorDir;
  nativeBuildInputs = common.nativeBuildInputs ++ [ pkgs.makeWrapper ];
  # Type checking and the scene host run the scene's own JavaScript in node.
  # One already on PATH wins; this one is for a machine that has none.
  postInstall = ''
    wrapProgram "$out/bin/dcl-scene-cli" --suffix PATH : ${lib.makeBinPath [ pkgs.nodejs ]}
  '';
  passthru = { inherit cargoArtifacts cargoVendorDir abgenCache livekitCache; };
  meta.mainProgram = "dcl-scene-cli";
})
