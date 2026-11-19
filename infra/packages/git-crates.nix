{ pkgs }:
# Vendors the crates of a git checkout without loading its workspace. Crane's default git
# vendoring runs `cargo metadata` on the checkout's root workspace, which fails when the commit
# carries only some of the members its root Cargo.toml lists (a patched source such as
# rendering/bevy-explorer/patches/client-sdk-rust, which publishes only its `paths`).
# `checkout` copies the named crates out of `tree`, each manifest with its workspace inheritance
# resolved against the nearest workspace above it, through a `cargo` that answers only the
# workspace root. Use it as crane's `overrideVendorGitCheckout`.
let
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
in {
  # `crates` are crane's lock entries of one git source; `fetched` is crane's checkout of it,
  # whose name the result keeps; it is never built, since building it loads the workspace. `tree`
  # is the source to copy from: the plain fetch (`fetched.src`) or a patched tree. `craneUtils`
  # is the crane instance's (it resolves inheritance).
  checkout = { craneUtils, tree, crates, fetched }: pkgs.runCommand fetched.name {
    nativeBuildInputs = [ pkgs.python3 craneUtils workspaceRootOnly ];
  } "python3 ${gitCrates} ${tree} $out ${pkgs.lib.escapeShellArgs (map (crate: "${crate.name}-${crate.version}") crates)}";
}
