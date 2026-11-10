{ lib, runCommand, python3 }:
let
  root = ../..;
  manifestPath = "rendering/bevy-explorer/source/react-web/shared-inputs.json";
  manifest = builtins.fromJSON (builtins.readFile (root + "/${manifestPath}"));
  safe = path: builtins.isString path && path != "" && !(lib.hasPrefix "/" path)
    && lib.all (part: part != "" && part != "." && part != "..") (lib.splitString "/" path)
    && !(lib.hasInfix "\\" path);
  rows = manifest.controls ++ manifest.files;
  requiredControls = [
    "rendering/bevy-explorer/source/react-web/package.json"
    "rendering/bevy-explorer/source/react-web/package-lock.json"
    "rendering/bevy-explorer/source/react-web/tsconfig.json"
    "rendering/bevy-explorer/source/react-web/vite.config.ts"
    "rendering/bevy-explorer/source/react-web/vitest.config.ts"
    "rendering/bevy-explorer/source/react-web/vite.shared.ts"
    "rendering/bevy-explorer/source/react-web/scripts/shared-inputs.mjs"
    "library/ui/package.json"
    "library/ui/foundations/typography/font-source.json"
  ];
  regular = path:
    let parts = lib.splitString "/" path;
      walk = parent: remaining:
        let name = builtins.head remaining;
          entries = builtins.readDir parent;
          kind = entries.${name} or "missing";
        in if builtins.length remaining == 1 then kind == "regular"
          else kind == "directory" && walk (parent + "/${name}") (builtins.tail remaining);
    in safe path && walk root parts;
  sourceRoots = prefix:
    let entries = builtins.readDir (root + "/${prefix}");
    in lib.concatMap (name:
      let path = "${prefix}/${name}"; kind = entries.${name};
      in if path == "rendering/bevy-explorer/source/react-web/src/engine/generated" || name == "node_modules" then []
        else if kind == "directory" then sourceRoots path
        else assert kind == "regular"; [ path ]) (builtins.attrNames entries);
  rootInventory = builtins.sort builtins.lessThan
    ((sourceRoots "rendering/bevy-explorer/source/react-web/src") ++
      (builtins.filter (path: builtins.pathExists (root + "/${path}")) [
        "rendering/bevy-explorer/source/react-web/app.html"
        "rendering/bevy-explorer/source/react-web/index.html"
      ]));
  valid = row:
    regular row.path
    && (let
      path = root + "/${row.path}";
      bytes = builtins.readFile path;
      # Hash first: readFile rejects binary NUL bytes; the Python verifier also checks payload size.
      payload = builtins.hashFile "sha256" path == row.sha256;
      pointer = if row ? lfs then
        "version https://git-lfs.github.com/spec/v1\noid sha256:${row.lfs.oid}\nsize ${toString row.lfs.size}\n"
        else "";
    in if row ? lfs then row.lfs.oid == row.sha256 &&
      (payload || bytes == pointer || bytes == lib.removeSuffix "\n" pointer)
    else payload);
  resolverControls = file:
    let directory = builtins.dirOf file;
    in (builtins.filter (path: builtins.pathExists (root + "/${path}"))
        (map (name: (if directory == "." then "" else directory + "/") + name) [ "package.json" "tsconfig.json" "jsconfig.json" ]))
      ++ (if directory == "." then [] else resolverControls directory);
  currentResolverControls = lib.unique (lib.concatMap resolverControls
    (manifest.roots ++ map (row: row.path) manifest.files));
  checked = assert manifest.schema == 1;
    assert lib.all valid rows;
    assert lib.all (path: lib.any (row: row.path == path) manifest.controls) currentResolverControls;
    assert lib.all (path: lib.any (row: row.path == path) manifest.controls) requiredControls;
    assert manifest.roots == rootInventory;
    assert lib.all (path: lib.any (row: row.path == path) rows) manifest.roots;
    manifest;
  sharedFiles = map (row: root + "/${row.path}") (checked.controls ++ checked.files);
  supplied = builtins.getEnv "PLAY_WEB_DIR";
  artifact = if supplied == "" then null else builtins.path {
    path = /. + supplied;
    name = "foundation-built-explorer";
  };
  # The flake's Git source contains tracked inputs, never Cargo targets or local
  # build output. Verify the first-party source directly without a snapshot pin.
  contract = lib.fileset.toSource {
    inherit root;
    fileset = lib.fileset.unions (sharedFiles ++ (map (p: lib.fileset.maybeMissing (root + p)) [
      /rendering/bevy-explorer/upstream.lock.json
      /rendering/bevy-explorer/source
      /rendering/bevy-explorer/bevy
      /library/protocol/proto
      /apps/bevy-explorer/build-web.py /apps/bevy-explorer/install-linux-deps.py
      /apps/bevy-explorer/install-web-tools.py
      /apps/bevy-explorer/install-tools.sh /.github/actions/bevy-explorer-tools/action.yml
      /flake.lock /infra/packages/default.nix /infra/packages/tools.nix
      /tooling/play-artifact.py /tooling/hud_shared_inputs.py
      /.github/workflows/bevy-explorer-ci.yml
      /.cargo /rust-toolchain /rust-toolchain.toml
      /apps/ui3/src/play/engine/generated
    ]));
  };
in runCommand "foundation-built-play-web" {
  src = artifact;
  nativeBuildInputs = [ python3 ];
} ''
  if [ -z "''${src:-}" ]; then
    echo 'Sites requires our Bevy artifact. Run tooling/fetch-play.py, then build with PLAY_WEB_DIR and --impure; see infra/containers/README.md.' >&2
    exit 1
  fi
  # A PR build (PLAY_ALLOW_UNOPTIMIZED=1) may carry main's engine when the PR changes no
  # engine input. --trust-inputs accepts only a build fetch-play.py recorded as validated
  # against its producer commit for a tree with exactly these inputs (producer-validated.json);
  # a build of the tree's own inputs, or any other, is still checked as before, and
  # integrity, HUD review and bindings are checked in every mode.
  python ${contract}/tooling/play-artifact.py verify "$src" --root ${contract} \
    ${if builtins.getEnv "PLAY_ALLOW_UNOPTIMIZED" == "1" then "--trust-inputs" else "--production"}
  mkdir -p "$out"
  for part in engine bridge-scene service_worker.js preview_realm.js sites-play.json; do
    cp -r "$src/$part" "$out/$part"
  done
''
