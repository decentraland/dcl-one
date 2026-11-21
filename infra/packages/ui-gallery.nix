{ lib, stdenv, fetchPnpmDeps, pnpmConfigHook, pnpm, nodejs-slim_24 }:
let
  # Node 24, the major the Shared UI library run builds the scanned gallery with (setup-node in ui-library.yml): the
  # publish receipt compares both builds byte for byte, so the two move together.
  pnpm' = pnpm.override { nodejs-slim = nodejs-slim_24; };
  root = ../..;
  ui = root + /library/ui;
  version = (builtins.fromJSON (builtins.readFile (ui + /package.json))).version;
  depsSrc = lib.fileset.toSource {
    root = ui;
    fileset = lib.fileset.unions (map (p: ui + p) [ /package.json /pnpm-lock.yaml /pnpm-workspace.yaml ]);
  };
  pnpmDepsArgs = {
    pnpm = pnpm';
    src = depsSrc;
    fetcherVersion = 4;
    hash = "sha256-OpH+YlxUnKeck0km/qgNTB57wmBDcVMk1+WG/rFX/RY=";
  };
  # A changed lockfile renames the fixed-output derivation, so a stale hash fails instead of reusing old modules.
  pnpmDepsKey = builtins.substring 0 12
    (builtins.hashString "sha256" (fetchPnpmDeps (pnpmDepsArgs // { pname = "foundation-ui-gallery"; })).drvPath);
  pnpmDeps = fetchPnpmDeps (pnpmDepsArgs // { pname = "foundation-ui-gallery-${pnpmDepsKey}"; });
  # What the hosted gallery compiles: the package's runtime sources, the HUD it draws and the Explorer core the HUD
  # imports. library/ui's figma/, test/, templates/ and apps/sites stay out, so changes there keep this build's
  # identity. The directories it takes whole (components, foundations, apps/hud, explorer-core) hold their own tests,
  # so a change to one of those tests still changes it.
  src = lib.fileset.toSource {
    inherit root;
    fileset = lib.fileset.unions ((map (p: ui + p) [
      /package.json /pnpm-lock.yaml /pnpm-workspace.yaml /tsconfig.json
      /index.ts /css-modules.d.ts /content.helpers.ts /link.helpers.ts /text-styles.ts
      /components /foundations /apps/hud
      /gallery/index.html /gallery/frame.html /gallery/hud.html /gallery/vite.config.ts
      /gallery/tsconfig.json /gallery/src /gallery/scripts/hosted-digest.mjs
    ]) ++ [ (root + /library/explorer-core) ]);
  };
in stdenv.mkDerivation {
  pname = "foundation-ui-gallery";
  inherit version src pnpmDeps;
  sourceRoot = "${src.name}/library/ui";
  nativeBuildInputs = [ nodejs-slim_24 pnpmConfigHook pnpm' ];
  # Static files only: the Sites image serves them under /ui-gallery/.
  allowedReferences = [ ];
  buildPhase = ''
    runHook preBuild
    if grep -rIl '^version https://git-lfs.github.com/spec/v1$' foundations/typography/fonts; then
      echo "The shared fonts contain Git LFS pointers. Run git lfs pull and build git+file:.?lfs=1#image-sites." >&2
      exit 1
    else
      status=$?
      if [ "$status" -ne 1 ]; then
        echo "Unable to inspect the shared fonts." >&2
        exit "$status"
      fi
    fi
    pnpm exec vite build --config gallery/vite.config.ts --mode hosted
    runHook postBuild
  '';
  installPhase = ''
    runHook preInstall
    # Fails on a hidden path, which the scanned artifact would not carry; prints the digest the publish receipt compares.
    node gallery/scripts/hosted-digest.mjs gallery/dist
    cp -r gallery/dist "$out"
    for page in index frame hud; do
      grep -qF 'name="robots" content="noindex"' "$out/$page.html" || { echo "$page.html is indexable" >&2; exit 1; }
    done
    runHook postInstall
  '';
}
