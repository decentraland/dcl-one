{ pkgs, craneLib }:
let
  inherit (pkgs) lib;
  root = ../..;
  explorerDir = "rendering/bevy-explorer";
  workspaceDir = "${explorerDir}/source";
  patchesDir = root + "/${explorerDir}/patches";
  workspace = root + "/${workspaceDir}";
  manifestText = builtins.readFile (workspace + /Cargo.toml);
  lockText = builtins.readFile (workspace + /Cargo.lock);
  manifest = builtins.fromTOML manifestText;
  lock = builtins.fromTOML lockText;
  # The compiler, the tools, the libraries and the prebuilt V8 and CEF: the dev shell has the same.
  native = import ./bevy-explorer-native.nix { inherit pkgs; };
  inherit (native) binary v8 cef;

  crane = craneLib.overrideScope (_: _: { stdenvSelector = _: native.stdenv; });

  tree = dir: lib.fileset.toSource {
    root = root + "/${dir}";
    fileset = root + "/${dir}";
  };

  # The repository's own files of the build. A layer's source root is these, or
  # their dummy, with the third-party trees laid over them by directory.
  ownFiles = [
    workspace
    (root + /library/protocol/proto)
    (root + /library/vendor/rusty_enet)
    (root + "/${explorerDir}/vendor")
    (root + /infra/gateway/tests/explorer_routes.py)
    (root + /.cargo/config.toml)
  ];
  repoFiles = lib.fileset.toSource { inherit root; fileset = lib.fileset.unions ownFiles; };

  # One derivation per patched source: a fetch of the upstream commit, then the
  # tool applies the patch series offline. The tool fails when the result is not
  # the `tree` of source.toml.
  patchTool = root + /tooling/patched-sources.py;
  checkout = url: rev: hash: pkgs.fetchgit { inherit url rev hash; fetchSubmodules = false; };
  # Where the tool writes the tree of a source, below the repository root.
  treeDir = name: "${explorerDir}/${if name == "bevy" then name else "forks/${name}"}";
  patched = name:
    let
      inputs = patchesDir + "/${name}";
      spec = builtins.fromTOML (builtins.readFile (inputs + /source.toml));
      base = checkout spec.upstream spec.commit spec.nix_hash;
      submodules = spec.submodule or [ ];
    in lib.nameValuePair (treeDir name) (pkgs.runCommand "patched-${name}" {
      nativeBuildInputs = [ pkgs.gitMinimal pkgs.python3 ];
      passthru = { inherit base; inherit (spec) rev; };
    } ''
      mkdir -p repo/tooling repo/${explorerDir}/patches bases
      cp ${patchTool} repo/tooling/patched-sources.py
      cp -r ${inputs} repo/${explorerDir}/patches/${name}
      ${if submodules == [ ] then "ln -s ${base} bases/${name}" else ''
        cp -r ${base} bases/${name}
        chmod -R u+w bases/${name}
        ${lib.concatMapStrings (submodule: ''
          cp -r ${checkout submodule.url submodule.commit submodule.nix_hash}/. bases/${name}/${submodule.path}/
        '') submodules}
      ''}
      python3 repo/tooling/patched-sources.py materialise ${name} --offline --base-dir bases --dest-root dest
      rm dest/${treeDir name}/.patched-stamp.json
      mv dest/${treeDir name} $out
    '');
  patchedSources = lib.listToAttrs (map patched (lib.attrNames (lib.filterAttrs
    (name: type: type == "directory" && builtins.pathExists (patchesDir + "/${name}/source.toml"))
    (builtins.readDir patchesDir))));

  # The third-party trees by repository-relative directory. A tree is a path or
  # a derivation.
  inTree = lib.genAttrs [
    "library/vendor/rusty_enet"
    "${workspaceDir}/vendor/basisu"
    "${explorerDir}/vendor/rpc-rust"
    "${explorerDir}/vendor/bevy_dui"
    "${explorerDir}/vendor/boimp"
  ] tree;
  thirdParty = patchedSources // inTree;

  # The path crates the workspace manifest names, each with its directory below
  # the repository root.
  resolvePath = path: lib.concatStringsSep "/" (lib.foldl' (done: part:
    if part == ".." then lib.init done else done ++ [ part ]) [ ] (lib.splitString "/" path));
  pathCrates = lib.concatMapAttrs
    (name: spec: lib.optionalAttrs (spec ? path) { ${spec.package or name} = resolvePath "${workspaceDir}/${spec.path}"; })
    (lib.foldl' lib.mergeAttrs manifest.workspace.dependencies (lib.attrValues manifest.patch));
  # A package of the lock file. A dependency names its package, with the version
  # when the name has several, and with the source when the version has several:
  # a path crate has no source.
  packageId = package: "${package.name} ${package.version} ${package.source or ""}";
  packagesByName = lib.groupBy (package: package.name) lock.package;
  dependencyId = dependency:
    let
      parts = lib.splitString " " dependency;
      part = lib.elemAt parts;
      named = packagesByName.${part 0};
      versioned = lib.filter (package: package.version == part 1) named;
      source = if lib.length parts > 2 then part 2 else "()";
      sourceLessCommit = package: lib.head (lib.splitString "#" (package.source or ""));
    in packageId (lib.head (
      if lib.length parts == 1 then named
      else if lib.length versioned == 1 then versioned
      else lib.filter (package: "(${sourceLessCommit package})" == source) versioned));
  dependents = lib.zipAttrs (lib.concatMap (package:
    map (dependency: { ${dependencyId dependency} = packageId package; }) (package.dependencies or [ ])) lock.package);
  commitOf = package: lib.last (lib.splitString "#" (package.source or ""));
  cratesOf = tree: map packageId (
    if patchedSources ? ${tree} then lib.filter (package: commitOf package == patchedSources.${tree}.rev) lock.package
    else lib.concatMap (name: lib.filter (package: !(package ? source)) (packagesByName.${name} or [ ]))
      (lib.attrNames (lib.filterAttrs (_: dir: dir == tree || lib.hasPrefix "${tree}/" dir) pathCrates)));
  usesBevy = lib.genAttrs (map (item: item.key) (builtins.genericClosure {
    startSet = map (key: { inherit key; }) (cratesOf (treeDir "bevy"));
    operator = item: map (key: { inherit key; }) (dependents.${item.key} or [ ]);
  })) (_: true);
  # The trees that are Bevy or depend on it, by the lock file. The second layer
  # compiles them; every other third-party tree compiles in the first.
  bevyLayer = lib.filter (tree: lib.any (id: usesBevy ? ${id}) (cratesOf tree)) (lib.attrNames thirdParty);
  late = lib.getAttrs bevyLayer thirdParty;
  early = removeAttrs thirdParty bevyLayer;
  byRev = lib.mapAttrs' (_: source: lib.nameValuePair source.rev source);
  latePatched = builtins.intersectAttrs late patchedSources;
  latePatchedByRev = byRev latePatched;
  inLatePatched = package: latePatchedByRev ? ${commitOf package};
  registryCratesThatUseBevy = lib.filter
    (package: usesBevy ? ${packageId package} && package ? source && !inLatePatched package) lock.package;
  earlyPackages = lib.throwIf (registryCratesThatUseBevy != [ ])
    "the first layer would compile these against a dummy of Bevy: ${lib.concatMapStringsSep ", " packageId registryCratesThatUseBevy}"
    (lib.filter (package: !inLatePatched package) lock.package);

  standInRevs = lib.mapAttrs (dir: _: builtins.hashString "sha1" dir) latePatched;
  withStandInRevs = builtins.replaceStrings
    (map (source: source.rev) (lib.attrValues latePatched)) (lib.attrValues standInRevs);

  # The dummy of a patched tree keeps only its manifests, and the evaluation
  # reads them from the derivation's output (import from derivation). So a
  # patch that changes no manifest leaves the first layer as it is.
  dummy = src: crane.mkDummySrc { inherit src; };
  dummyRepo = crane.mkDummySrc {
    src = repoFiles;
    extraDummyScript = "cp ${workspace + /Cargo.lock} $out/${workspaceDir}/Cargo.lock";
  };
  # Every layer unpacks to the same directory name, so Cargo sees the same paths.
  assemble = base: trees: pkgs.runCommand "source" { } ''
    cp -r ${base} $out
    chmod -R u+w $out
    ${lib.concatStrings (lib.mapAttrsToList (dir: src: ''
      rm -rf $out/${dir}
      mkdir -p $out/${dirOf dir}
      cp -r ${src} $out/${dir}
      chmod -R u+w $out/${dir}
    '') trees)}
  '';
  stableDummyRepo = crane.mkDummySrc {
    src = assemble repoFiles { "${workspaceDir}/Cargo.toml" = builtins.toFile "Cargo.toml" (withStandInRevs manifestText); };
    extraDummyScript = "cp ${builtins.toFile "Cargo.lock" (withStandInRevs lockText)} $out/${workspaceDir}/Cargo.lock";
  };
  depsSrc = assemble stableDummyRepo (builtins.intersectAttrs early inTree);
  bevySrc = assemble dummyRepo inTree;
  src = repoFiles;

  gitCrates = pkgs.writeText "git-crates.py" ''
    import os, pathlib, re, shutil, subprocess, sys, tomllib

    tree, out, *wanted = sys.argv[1:]
    manifests = {path.parent: tomllib.loads(path.read_text()) for path in pathlib.Path(tree).rglob("Cargo.toml")}
    crates = {}
    for crate, manifest in manifests.items():
        if "package" in manifest:
            root = next((above for above in [crate, *crate.parents] if "workspace" in manifests.get(above, {})), crate)
            resolved = subprocess.run(["crane-resolve-workspace-inheritance", crate / "Cargo.toml"], check=True,
                stdout=subprocess.PIPE, text=True, env=os.environ | {"WORKSPACE_ROOT": str(root)}).stdout
            package = tomllib.loads(resolved).get("package", {})
            crates.setdefault(f"{package.get('name')}-{package.get('version', '0.0.0')}", []).append((crate, resolved))
    for name in wanted:
        found = crates.get(name, [])
        if len(found) != 1:
            sys.exit(f"{name}: {len(found)} crates of this name and version in {tree}")
        crate, resolved = found[0]
        target = pathlib.Path(out, name)
        shutil.copytree(crate, target, ignore=lambda directory, names: [
            entry for entry in names if pathlib.Path(directory, entry) in manifests])
        subprocess.run(["chmod", "-R", "u+w", target], check=True)
        library = tomllib.loads(resolved).get("lib", {}).get("path", "")
        inside = re.sub(r"^(\.\./)+", "", library)
        if inside != library:
            shutil.copytree((crate / library).parent, (target / inside).parent, dirs_exist_ok=True)
            resolved = resolved.replace(f'"{library}"', f'"{inside}"')
        (target / "Cargo.toml").write_text(resolved)
        (target / ".cargo-checksum.json").write_text('{"files":{}, "package":null}')
  '';
  workspaceRootOnly = pkgs.writeShellScriptBin "cargo" ''echo "{\"workspace_root\":\"$WORKSPACE_ROOT\"}"'';
  hashOfUnpatchedGitSource."git+https://github.com/gltf-rs/gltf?rev=50d65229#50d65229477fe5f785c2c90df21eb59c93ea2261" =
    "sha256-nSlREMX9EKNpBFAXMA9PUj2FeLEr9Yh8UayEzrJIUBQ=";
  vendor = checkoutsByRev: packages: crane.vendorCargoDeps {
    cargoLockParsed.package = packages;
    outputHashes = hashOfUnpatchedGitSource;
    overrideVendorGitCheckout = crates: fetched:
      let
        package = lib.head crates;
        src = checkoutsByRev.${commitOf package} or null;
      in if src == null then lib.throwIfNot (hashOfUnpatchedGitSource ? ${package.source})
        "${package.name}: ${package.source} is not the rev of a patched source and has no hash in hashOfUnpatchedGitSource" fetched
      else pkgs.runCommand fetched.name {
        nativeBuildInputs = [ pkgs.python3 crane.craneUtils workspaceRootOnly ];
      } "python3 ${gitCrates} ${src} $out ${lib.escapeShellArgs (map (crate: "${crate.name}-${crate.version}") crates)}";
  };
  earlyVendor = vendor (byRev patchedSources) earlyPackages;
  lateVendor = vendor latePatchedByRev (lib.filter inLatePatched lock.package);
  lateDummiesByStandInRev = lib.mapAttrs' (dir: source: lib.nameValuePair standInRevs.${dir} (dummy source)) latePatched;
  lateDummyVendor = vendor lateDummiesByStandInRev (lib.filter
    (package: lateDummiesByStandInRev ? ${commitOf package}) (builtins.fromTOML (withStandInRevs lockText)).package);
  vendorDir = lateDir: pkgs.runCommandLocal "vendor-cargo-deps" { } ''
    for crate in ${earlyVendor}/*/* ${lateDir}/*/*; do
      [ -e "$crate/Cargo.toml" ] || { echo "$crate: crane did not find this crate in its checkout" >&2; exit 1; }
    done
    mkdir $out
    cat ${earlyVendor}/config.toml ${lateDir}/config.toml >$out/config.toml
  '';
  cargoVendorDir = vendorDir lateVendor;

  # What the build scripts download in a cargo build is fetched ahead, because the sandbox has no
  # network: V8 and CEF in bevy-explorer-native.nix, WebRTC here.
  # The release the build script of webrtc-sys downloads, read from the patched
  # tree (import from derivation).
  webrtcTag =
    let
      file = "${patchedSources.${treeDir "client-sdk-rust"}}/webrtc-sys/build/src/lib.rs";
      line = lib.findFirst (lib.hasInfix "const WEBRTC_TAG") (throw "${file} has no WEBRTC_TAG")
        (lib.splitString "\n" (builtins.readFile file));
    in lib.elemAt (lib.splitString "\"" line) 1;
  webrtc = pkgs.runCommand "livekit-${webrtcTag}" {
    nativeBuildInputs = [ pkgs.unzip ];
    archive = binary {
      name = "${webrtcTag}-linux-x64-release.zip";
      url = "https://github.com/robtfm/client-sdk-rust/releases/download/${webrtcTag}/webrtc-linux-x64-release.zip";
      hash = "sha256-b9SHFy2suLTz044/33AwCbQTI2NL8Bnfg13vLTqRZ10=";
    };
  } ''
    unzip -q $archive
    mv linux-x64-release $out
  '';
  # The root build script asks git for the revision and the local changes.
  git = pkgs.writeShellScriptBin "git" ''
    if [ "$1" = rev-parse ]; then echo 0000000000000000000000000000000000000000; fi
  '';

  # The inputs and the environment are the same in every layer: a build script
  # that reads either one stays fresh in the next layer.
  common = {
    pname = "bevy-explorer";
    version = manifest.package.version;
    inherit cargoVendorDir;
    postUnpack = ''sourceRoot="$sourceRoot/${workspaceDir}"'';
    strictDeps = true;
    doCheck = false;
    dontUseCmakeConfigure = true;
    nativeBuildInputs = native.tools ++ [ pkgs.rustPlatform.bindgenHook pkgs.python3 pkgs.jq git ];
    buildInputs = native.libraries;
    # nixpkgs seeds the C compiler flags with the hash of the output, and the
    # bindgen hook copies them. bindgen compiles again when its flags change.
    #
    # Every source file has the same time. For a build script that names no
    # inputs Cargo records the newest file of its package, and among equal times
    # that is the first one the directory lists, which differs from one unpack to
    # the next. One newer file in each such package makes the record the same in
    # every layer. A script that names its inputs keeps the time of the artifacts
    # of the layer before, so it does not run again. The build script of a dummy
    # crate names no inputs and is not beside the manifest, so the mark goes on
    # every manifest that has no build.rs with inputs beside it.
    preBuild = ''
      export BINDGEN_EXTRA_CLANG_ARGS="$(sed 's/-frandom-seed=[^ ]*//' <<<"$BINDGEN_EXTRA_CLANG_ARGS")"
      find "$NIX_BUILD_TOP/source" -name Cargo.toml | while read -r manifest; do
        grep -qs rerun-if "''${manifest%Cargo.toml}build.rs" || touch -d @2 "$manifest"
      done
    '';
    CARGO_PROFILE = "ci-test";
    RUSTY_V8_ARCHIVE = "${v8}";
    LK_CUSTOM_WEBRTC = "${webrtc}";
    CEF_PATH = "${cef}";
  };

  # The two builds of CI: the binaries of the root package, then every test.
  # The two resolve features apart, so part of the dependencies compiles once
  # for each, and every layer runs both. Cargo keeps one record for the root
  # library, and the test step must find the record of the test build, so that
  # build is the last one.
  json = "--message-format json-render-diagnostics";
  cargoBuild = ''
    cargoWithProfile build --locked ${json} >"$TMPDIR/binaries.json"
    cargoWithProfile test --locked --all --no-run ${json} >"$TMPDIR/tests.json"
  '';
  # How many units Cargo reused. A unit of both builds counts once, as built
  # when either build compiled it.
  countUnits = ''
    jq -cs 'map(select(.reason == "compiler-artifact")) | group_by([.package_id, .filenames])
      | map(sort_by(.fresh)[0]) | .[]' "$TMPDIR/binaries.json" "$TMPDIR/tests.json" >"$TMPDIR/units.json"
    jq -rs '"units: \(map(select(.fresh)) | length) fresh, \(map(select(.fresh | not)) | length) built"' "$TMPDIR/units.json"
  '';
  # The packages a layer compiled: none of them is of an earlier layer.
  builtPackages = ''
    jq -rs '[.[] | select(.fresh | not) | .package_id
        | capture("(?<directory>[^/#]+)#((?<name>[^@]+)@)?") | .name // .directory]
      | "built: \(unique | join(" "))"' "$TMPDIR/units.json"
  '';

  # Bevy and every crate that uses it are dummies here, so each compiles as an
  # empty crate and the layer keeps what does not depend on them.
  deps = crane.buildDepsOnly (common // {
    dummySrc = depsSrc;
    cargoVendorDir = vendorDir lateDummyVendor;
    buildPhaseCargoCommand = ''
      ${cargoBuild}
      ${countUnits}
    '';
  });

  bevy = crane.mkCargoDerivation (common // {
    src = bevySrc;
    pnameSuffix = "-bevy";
    cargoArtifacts = deps;
    buildPhaseCargoCommand = ''
      ${cargoBuild}
      ${countUnits}
      ${builtPackages}
    '';
  });

  explorer = crane.mkCargoDerivation (common // {
    inherit src;
    cargoArtifacts = bevy;
    outputs = [ "out" "artifacts" ];
    buildPhaseCargoCommand = ''
      ${cargoBuild}
      ${countUnits}
      ${builtPackages}
    '';
    doInstallCargoArtifacts = false;
    installPhaseCommand = ''
      install -Dt $out/bin target/ci-test/decentra-bevy target/ci-test/decentra-bevy-cef
      prepareAndInstallCargoArtifactsDir "$artifacts"
    '';
    passthru = { inherit deps bevy tests cargoVendorDir patchedSources sources cef webrtc v8; };
    meta.mainProgram = "decentra-bevy";
  });

  # Every patched tree under its directory, to build them without a layer.
  sources = pkgs.linkFarm "bevy-explorer-sources" patchedSources;

  # Tests that need what the build sandbox does not have: a network, /bin/ps.
  sandboxSkips = [
    "test::test_ffmpeg"
    "test::test_async_tls"
    "test::test_livekit"
    "test::test_tls"
    "profile::tests::the_process_list_can_be_read"
    "with_no_stray_on_the_profile_nothing_happens"
  ];

  # Outside its own tests deno_core starts V8 on the protected platform. On a
  # processor with memory protection keys the second test thread that starts V8
  # then ends the process with a segmentation fault, so each test of dcl_deno
  # runs in its own process. The arguments already carry --exact.
  testRunner = pkgs.writeShellScript "explorer-test-runner" ''
    binary=$1
    shift
    case "''${binary##*/}" in
      dcl_deno-*)
        status=0
        for name in $("$binary" --list --format terse | sed -n 's/: test$//p'); do
          "$binary" "$@" "$name" || status=1
        done
        exit $status ;;
      *) exec "$binary" "$@" ;;
    esac
  '';

  # The test step of the CI test job.
  tests = crane.mkCargoDerivation (common // {
    inherit src;
    pnameSuffix = "-tests";
    cargoArtifacts = explorer.artifacts;
    doInstallCargoArtifacts = false;
    buildPhaseCargoCommand = ''
      export HOME="$TMPDIR/home"
      mkdir -p "$HOME"
      export CARGO_TARGET_${pkgs.stdenv.hostPlatform.rust.cargoEnvVarTarget}_RUNNER=${testRunner}
      cargoWithProfile test --locked --all --no-fail-fast -- --test-threads=1 --exact \
        ${lib.concatMapStringsSep " " (name: "--skip ${name}") sandboxSkips}
    '';
  });
in explorer
