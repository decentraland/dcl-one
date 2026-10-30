#!/usr/bin/env python3
"""Verify exact vendored bytes and reconstruct the pinned import without network access."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def manifest(name):
    entries = {}
    for line in (ROOT / name).read_text(encoding="utf-8").splitlines():
        checksum, relative = line.split("  ", 1)
        path = Path(relative)
        if path.is_absolute() or ".." in path.parts or path.as_posix() != relative or relative in entries:
            raise SystemExit(f"invalid manifest path: {relative}")
        entries[relative] = checksum
    return entries


def main():
    pin = json.loads((ROOT / "upstream.lock.json").read_text(encoding="utf-8"))
    for name, expected in pin["manifests"].items():
        if digest(ROOT / name) != expected:
            raise SystemExit(f"manifest digest mismatch: {name}")
    upstream = manifest("upstream-files.sha256")
    patched = manifest("checksums.sha256")
    metadata = {"upstream.lock.json", "upstream-files.sha256", "checksums.sha256"}
    # Cargo may build this standalone workspace in its ignored root target directory.
    actual = {p.relative_to(ROOT).as_posix() for p in ROOT.rglob("*")
              if p.is_file() and p.relative_to(ROOT).parts[0] != "target"}
    if actual != set(patched) | metadata:
        raise SystemExit(f"file set mismatch; missing={sorted((set(patched) | metadata) - actual)}, extra={sorted(actual - set(patched) - metadata)}")
    for relative, expected in patched.items():
        if (ROOT / relative).is_symlink() or digest(ROOT / relative) != expected:
            raise SystemExit(f"source digest mismatch: {relative}")
    if digest(ROOT / pin["license"]["path"]) != pin["license"]["sha256"]:
        raise SystemExit("license digest mismatch")
    patches = pin["patches"]
    for patch in patches:
        if digest(ROOT / patch["path"]) != patch["sha256"]:
            raise SystemExit(f"patch digest mismatch: {patch['path']}")
    with tempfile.TemporaryDirectory(prefix="enet-provenance-") as temporary:
        destination = Path(temporary)
        for relative in upstream:
            target = destination / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / relative, target)
        for patch in reversed(patches):
            subprocess.run(["git", "-c", "core.autocrlf=false", "apply", "--reverse", str(ROOT / patch["path"])], cwd=destination, check=True)
        for relative, expected in upstream.items():
            if digest(destination / relative) != expected:
                raise SystemExit(f"patch replay mismatch: {relative}")
    print(f"Verified {len(upstream)} imported files, {len(patched)} shipped files and offline patch replay at {pin['revision']}.")


if __name__ == "__main__":
    main()
