#!/usr/bin/env bash
# protoc, taplo and wasm-pack as the flake's bevy-tools package builds them. With the closure
# already in the staging directory (an Actions cache hit) its paths are linked under /nix/store
# and no Nix is needed; otherwise Nix builds the package from the lock and the closure is
# copied there for the cache to save. Either way the package's bin directory goes on PATH.
set -euo pipefail
staging=$1
if [ -s "$staging/out" ] && ! command -v nix > /dev/null; then
  if [ ! -e /nix/store ]; then
    sudo install -d -o "$(id -u)" -g "$(id -g)" /nix /nix/store
  fi
  for path in "$staging"/store/*; do
    [ -e "/nix/store/${path##*/}" ] || ln -s "$path" "/nix/store/${path##*/}"
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
echo "$out/bin" >> "$GITHUB_PATH"
