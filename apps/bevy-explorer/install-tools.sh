#!/usr/bin/env bash
# A package of the flake (infra/packages/tools.nix) for the steps that follow. With the closure
# already in the staging directory (an Actions cache hit) its paths appear under /nix/store as
# links and no Nix is needed. Otherwise Nix builds the package from the lock and the closure is
# copied to the staging directory for the cache to save. bevy-tools goes on PATH, and its protoc
# must share the major of the version install-protoc.py pins for Windows and macOS.
set -euo pipefail
shopt -s nullglob
staging=$1 package=$2
if [ -s "$staging/out" ] && ! command -v nix > /dev/null; then
  paths=("$staging"/store/*)
  if [ ${#paths[@]} -eq 0 ]; then
    echo "$staging/out names a closure but $staging/store holds no paths" >&2
    exit 1
  fi
  if [ ! -e /nix/store ]; then
    sudo install -d -o "$(id -u)" -g "$(id -g)" /nix /nix/store
  fi
  linked=0
  for path in "${paths[@]}"; do
    # -e is false for a dangling link; -sfn replaces one instead of failing on it.
    [ -e "/nix/store/${path##*/}" ] || { ln -sfn "$path" "/nix/store/${path##*/}"; linked=$((linked + 1)); }
  done
  echo "$package: ${#paths[@]} cached store paths, $linked linked into /nix/store"
else
  out=$(cd "$GITHUB_WORKSPACE" && nix build ".#$package" --no-link --print-out-paths)
  if [ ! -s "$staging/out" ]; then
    [ ! -e "$staging/store" ] || { chmod -R u+w "$staging/store"; rm -rf "$staging/store"; }
    mkdir -p "$staging/store"
    for path in $(nix-store -qR "$out"); do cp -a "$path" "$staging/store/"; done
    chmod -R u+w "$staging/store"
    printf '%s\n' "$out" > "$staging/out"
    echo "$package: built $out, $(ls "$staging/store" | wc -l | tr -d ' ') store paths exported for the cache"
  fi
fi
out=$(< "$staging/out")
case $package in
  bevy-tools)
    for tool in protoc taplo wasm-pack; do "$out/bin/$tool" --version; done
    major=$(sed -n "s/^VERSION = '\([0-9]*\)\..*'$/\1/p" "$GITHUB_WORKSPACE/apps/bevy-explorer/install-protoc.py")
    [ -n "$major" ] || { echo 'install-protoc.py no longer pins a VERSION to compare with' >&2; exit 1; }
    case "$("$out/bin/protoc" --version)" in
      "libprotoc $major."*) ;;
      *)
        echo "The flake's $("$out/bin/protoc" --version) is not the $major.x that install-protoc.py pins" >&2
        exit 1 ;;
    esac
    echo "$out/bin" >> "$GITHUB_PATH" ;;
  *) echo "install-tools.sh: no verification for package $package" >&2; exit 1 ;;
esac
