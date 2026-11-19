"""Plan the standard CI target selection before linking integration test executables."""
import json
import os
from pathlib import Path
import subprocess
import sys


def candidates(arguments):
    # Delegate custom package and target selections to the ordinary Cargo build.
    if len(arguments) % 2 != 1 or arguments[0] != "--workspace":
        return None
    if any(arg.startswith("-") for arg in arguments[2::2]):
        return None
    if set(arguments[1::2]) - {"--exclude", "--features"}:
        return None
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"]))
    tree = subprocess.check_output(
        ["cargo", "tree", "--locked", *arguments, "--depth", "0", "--prefix", "none", "--color", "never",
         "--format", "{p}|{f}"], text=True)
    active = {}
    for line in tree.splitlines():
        if line.strip():
            package, enabled = line.rsplit("|", 1)
            active[package.split()[0]] = set(enabled.split(","))
    rows = []
    for package in metadata["packages"]:
        if package["name"] not in active:
            continue
        for target in package["targets"]:
            required = target.get("required-features", [])
            # Cargo also compiles examples during an unfiltered test build. Its package
            # feature list does not describe required features of dependencies.
            if target["kind"] == ["example"] or any("/" in feature for feature in required):
                return None
            if not target["test"]:
                if target["kind"] in (["bin"], ["lib"]):
                    return None
                continue
            if len(target["kind"]) != 1:
                return None
            kind, = target["kind"]
            if set(required) - active[package["name"]]:
                continue
            if kind not in ("test", "lib", "bin"):
                return None
            rows.append((os.path.relpath(target["src_path"]), kind, f"{kind}/{target['name']}"))
    if len({label for _, _, label in rows}) != len(rows):
        return None
    return rows


def resolve(plan, inventory, artifacts):
    def entries(filename):
        return [line.split("\t") for line in Path(filename).read_text().splitlines()]
    paths = {(src, kind): path for src, kind, path in entries(artifacts)}
    keys = {name: (src, kind) for src, kind, name in entries(inventory)}
    if extra := paths.keys() - set(keys.values()):
        raise ValueError(f"unexpected compiled targets: {sorted(extra)}")
    for name in Path(plan).read_text().splitlines():
        print(paths[keys[name]])


if __name__ == "__main__":
    if sys.argv[1] == "plan":
        for row in candidates(sys.argv[2:]) or ():
            print(*row, sep="\t")
    elif sys.argv[1] == "resolve":
        resolve(*sys.argv[2:])
    else:
        sys.exit("expected plan or resolve")
