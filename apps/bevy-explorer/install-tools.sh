#!/usr/bin/env bash
# protoc, taplo and wasm-pack as the flake's bevy-tools package builds them. With the closure
# already in the staging directory (an Actions cache hit) its paths are linked under /nix/store
# and no Nix is needed; otherwise Nix builds the package from the lock and the closure is
# copied there for the cache to save. Either way the package's bin directory goes on PATH, and
# its protoc must share the major of the version install-protoc.py pins for Windows and macOS.
set -euo pipefail
shopt -s nullglob
staging=$1
if [ -s "$staging/out" ] && ! command -v nix > /dev/null; then
  paths=("$staging"/store/*)
  if [ ${#paths[@]} -eq 0 ]; then
    echo "$staging/out names a closure but $staging/store holds no paths" >&2
    exit 1
  fi
  if [ ! -e /nix/store ]; then
    sudo install -d -o "$(id -u)" -g "$(id -g)" /nix /nix/store
  fi
  for path in "${paths[@]}"; do
    # -e is false for a dangling link; -sfn replaces one instead of failing on it.
    [ -e "/nix/store/${path##*/}" ] || ln -sfn "$path" "/nix/store/${path##*/}"
  done
else
  out=$(cd "$GITHUB_WORKSPACE" && nix build .#bevy-tools --no-link --print-out-paths)
  if [ ! -s "$staging/out" ]; then
    [ ! -e "$staging/store" ] || { chmod -R u+w "$staging/store"; rm -rf "$staging/store"; }
    mkdir -p "$staging/store"
    for path in $(nix-store -qR "$out"); do cp -a "$path" "$staging/store/"; done
    chmod -R u+w "$staging/store"
    printf '%s\n' "$out" > "$staging/out"
  fi
fi
out=$(< "$staging/out")
for tool in protoc taplo wasm-pack; do "$out/bin/$tool" --version; done
major=$(sed -n "s/^VERSION = '\([0-9]*\)\..*'$/\1/p" "$GITHUB_WORKSPACE/apps/bevy-explorer/install-protoc.py")
[ -n "$major" ] || { echo 'install-protoc.py no longer pins a VERSION to compare with' >&2; exit 1; }
case "$("$out/bin/protoc" --version)" in
  "libprotoc $major."*) ;;
  *)
    echo "The flake's $("$out/bin/protoc" --version) is not the $major.x that install-protoc.py pins" >&2
    exit 1 ;;
esac
echo "$out/bin" >> "$GITHUB_PATH"
