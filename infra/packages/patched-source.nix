# One Explorer source under rendering/bevy-explorer/patches/<name>: a fetch of the upstream commit,
# then tooling/patched-sources.py applies the patch series offline. The tool fails when the result
# is not the `tree` of source.toml. The Explorer (bevy-explorer.nix), `cast-bot` (cast-bot.nix) and
# the message router (message-router.nix) build their patched sources, and vendor their crates, here.
{ pkgs }:
let
  inherit (pkgs) lib;
  root = ../..;
  explorerDir = "rendering/bevy-explorer";
  patchesDir = root + "/${explorerDir}/patches";
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
    in pkgs.runCommand "patched-${name}" {
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
    '';
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
  # The lock's crates of a patched tree, in the layout crane vendors a git checkout in: one
  # directory per crate with its manifest's workspace inheritance resolved. A patched tree keeps
  # only some members of its upstream workspace, so Cargo cannot read it as one.
  vendoredCrates = craneUtils: fetched: src: crates: pkgs.runCommand fetched.name {
    nativeBuildInputs = [ pkgs.python3 craneUtils workspaceRootOnly ];
  } "python3 ${gitCrates} ${src} $out ${lib.escapeShellArgs (map (crate: "${crate.name}-${crate.version}") crates)}";
in {
  inherit patchesDir treeDir patched vendoredCrates;
}
