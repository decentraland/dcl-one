{ lib, stdenv, runCommand, linkFarm, fetchPnpmDeps, pnpmConfigHook, pnpm, nodejs_26, nodejs-slim_26, makeWrapper, playWeb }:
let
  nodejs = nodejs_26;
  pnpm' = pnpm.override { nodejs-slim = nodejs-slim_26; };
  # Keep every output together in the binary cache. Restoring only slim's runtime
  # forces a partial Node rebuild whose libv8 output check still references out.
  toolchain = linkFarm "foundation-sites-toolchain" (lib.concatMap (package:
    map (output: {
      name = "${package.pname}-${output}";
      path = package.${output};
    }) package.outputs
  ) [ nodejs nodejs-slim_26 ]);
  root = ../..;
  sites = root + /apps/sites;
  ui = root + /apps/ui3;
  version = (builtins.fromJSON (builtins.readFile (sites + /package.json))).version;
  depsSrc = lib.fileset.toSource {
    root = sites;
    fileset = lib.fileset.unions (map (p: sites + p) [ /package.json /pnpm-lock.yaml /.npmrc /pnpm-workspace.yaml ]);
  };
  pnpmDeps = fetchPnpmDeps {
    pname = "foundation-sites";
    pnpm = pnpm';
    src = depsSrc;
    fetcherVersion = 4;
    hash = "sha256-XIPJ6YXQstlI6H+DRcScQteTGoxHi95jFmttSp6RucQ=";
  };
  runtimeDeps = stdenv.mkDerivation {
    pname = "foundation-sites-runtime-deps";
    inherit version pnpmDeps;
    src = depsSrc;
    nativeBuildInputs = [ nodejs-slim_26 pnpmConfigHook pnpm' ];
    pnpmInstallFlags = [ "--prod" ];
    dontBuild = true;
    installPhase = ''
      test -e node_modules/.bin/react-router-serve
      mkdir -p "$out"
      cp -a node_modules "$out/node_modules"
    '';
  };
  docsContent = runCommand "foundation-sites-docs" {
    src = lib.fileset.toSource { root = sites + /content/docs; fileset = sites + /content/docs; };
  } ''
    if grep -rIl '^version https://git-lfs.github.com/spec/v1$' "$src"; then
      echo "Docs media contains Git LFS pointers. Run git lfs pull and build git+file:.?lfs=1#image-sites." >&2
      exit 1
    fi
    test -s "$src/manifest.json"
    mkdir -p "$out"
    cp -a "$src/." "$out/"
  '';
  src = lib.fileset.toSource {
    inherit root;
    fileset = lib.fileset.unions [
      (sites + /src) (sites + /scripts)
      (sites + /package.json) (sites + /pnpm-lock.yaml) (sites + /.npmrc) (sites + /pnpm-workspace.yaml)
      (sites + /vite.config.ts) (sites + /react-router.config.ts) (sites + /tsconfig.json)
      (ui + /src) (ui + /public) (ui + /scripts/gen-error-pages.mts)
      (ui + /vite.validate.js) (ui + /package.json)
      (root + /library/catalyst-api)
    ];
  };
in stdenv.mkDerivation {
  pname = "foundation-sites";
  inherit version src pnpmDeps;
  sourceRoot = "${src.name}/apps/sites";
  nativeBuildInputs = [ nodejs pnpmConfigHook pnpm' makeWrapper ];
  env.NODE_OPTIONS = "--max-old-space-size=4096";
  buildPhase = ''
    runHook preBuild
    chmod -R u+w ..
    ln -s "$PWD/node_modules" ../node_modules
    pnpm run build
    runHook postBuild
  '';
  installPhase = ''
    runHook preInstall
    mkdir -p "$out/bin" "$out/content" "$out/src/data" "$out/src/features"
    cp -r build "$out/build"
    # /play/engine, /play/bridge-scene and /play/service_worker.js are static files of the client
    # build; the /play documents are React Router routes.
    mkdir -p "$out/build/client/play"
    cp -a ${playWeb}/. "$out/build/client/play/"
    cp package.json "$out/package.json"
    ln -s ${runtimeDeps}/node_modules "$out/node_modules"
    ln -s ${docsContent} "$out/content/docs"
    cp -r src/data/fixtures "$out/src/data/fixtures"
    makeWrapper ${nodejs-slim_26}/bin/node "$out/bin/sites-server" \
      --add-flags "$out/node_modules/@react-router/serve/bin.cjs $out/build/server/index.js" \
      --set-default SITES_STATE_DIR /data \
      --set PLAY_WEB_DIR "$out/build/client/play" \
      --chdir "$out"
    test -s "$out/build/server/index.js"
    test -s "$out/content/docs/manifest.json"
    runHook postInstall
  '';
  passthru = { inherit pnpmDeps runtimeDeps docsContent playWeb toolchain; };
  meta.mainProgram = "sites-server";
}
