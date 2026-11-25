{ pkgs, bevy-explorer }:
let
  inherit (pkgs) lib;
  root = ../..;
  source = "rendering/bevy-explorer/source";
  web = "${source}/react-web";
  nodejs = pkgs.nodejs_24;
  pnpm = pkgs.callPackage ./pnpm.nix { };
  inherit (bevy-explorer) cef;
  native = import ./bevy-explorer-native.nix { inherit pkgs; };

  engine = bevy-explorer.overrideAttrs (old: {
    postBuild = (old.postBuild or "") + ''
      cargoWithProfile build --locked --package dcl_deno_ipc
    '';
    postInstall = (old.postInstall or "") + ''
      install -Dt $out/bin target/ci-test/dcl_deno_ipc
    '';
  });

  hudPart = { name, dir, prefixes, hash, build }:
    let
      srcName = "explorer-${name}-source";
      under = path: let rel = lib.removePrefix "${toString root}/" (toString path); in
        !(lib.hasInfix "/node_modules" rel)
        && lib.any (prefix: lib.hasPrefix "${prefix}/" "${rel}/" || lib.hasPrefix "${rel}/" "${prefix}/") prefixes;
      src = builtins.path { name = srcName; path = root; filter = path: _: under path; };
    in pkgs.stdenvNoCC.mkDerivation (finalAttrs: {
      pname = "explorer-${name}";
      version = "0";
      inherit src;
      sourceRoot = "${srcName}/${dir}";
      postUnpack = "chmod -R u+w ${srcName}";
      nativeBuildInputs = [ nodejs pnpm pkgs.pnpmConfigHook ];
      pnpmDeps = pkgs.fetchPnpmDeps {
        inherit (finalAttrs) pname version src sourceRoot postUnpack;
        inherit pnpm hash;
        fetcherVersion = 4;
      };
      buildPhase = ''
        runHook preBuild
        export HOME=$TMPDIR
        ${build}
        runHook postBuild
      '';
      installPhase = "cp -r $TMPDIR/out $out";
    });

  reactHud = hudPart {
    name = "react-hud";
    dir = web;
    prefixes = [ web "${source}/deploy/web" "library" ];
    hash = "sha256-Q5XwbZCp7e6Vn7UpRfpS5H9rcPOpOqU1+j2sQOSC7Qg=";
    build = ''
      node_modules/.bin/tsc --noEmit
      node_modules/.bin/vite build --mode native --base ./ --outDir $TMPDIR/out --emptyOutDir
    '';
  };
  bridgeScene = hudPart {
    name = "bridge-scene";
    dir = "${web}/bridge-scene";
    prefixes = [ "${web}/bridge-scene" "library/hashing" "library/explorer-core" "apps/creator-hub/packages/zip-extract" ];
    hash = "sha256-kIK1Di/JA//WWDqi9CBZY2RQ4BCZNn/IwrjF2ntV1fg=";
    build = ''
      pnpm --filter @dcl/hashing build
      pnpm run export-native
      mv ../../assets/bridge-scene $TMPDIR/out
    '';
  };
in
pkgs.runCommand "explorer-${bevy-explorer.version}" {
  nativeBuildInputs = [ pkgs.makeWrapper ];
  passthru = { inherit engine reactHud bridgeScene; };
  meta.mainProgram = "decentra-bevy";
} ''
  app=$out/lib/explorer
  mkdir -p $app/assets $out/bin
  cp ${engine}/bin/{decentra-bevy,decentra-bevy-cef,dcl_deno_ipc} $app/
  ln -s ${cef}/* $app/
  ln -s ${reactHud} $app/assets/react-hud
  ln -s ${bridgeScene} $app/assets/bridge-scene
  makeWrapper $app/decentra-bevy $out/bin/decentra-bevy \
    --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath native.runtimeLibraries}:/run/opengl-driver/lib
''
