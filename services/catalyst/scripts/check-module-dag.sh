#!/usr/bin/env bash
# Keeps the application's domain modules a layered DAG (docs/module-layers.toml).
# An edge A -> B is one reference to B in A's source (see refs_of). It must point to
# a lower layer. The known back-edges are listed in [allow] with their counts. The check
# fails on a back-edge that is not listed, and on a listed count that grows.
# It prints each listed edge that shrank, so its allowance can be tightened.
#   scripts/check-module-dag.sh [ROOT]      check
#   scripts/check-module-dag.sh --edges     print every edge with its count
set -euo pipefail
mode=check
[[ "${1:-}" == --edges ]] && { mode=edges; shift; }
ROOT="$(cd "${1:-$(dirname "${BASH_SOURCE[0]}")/..}" && pwd)"
cd "$ROOT"
LAYERS=../../docs/module-layers.toml
command -v rg >/dev/null || { echo "check-module-dag: ripgrep (rg) is required" >&2; exit 2; }

declare -A layer allow=()
section=
while IFS= read -r line; do
  line="${line%%#*}"
  [[ "$line" =~ ^[[:space:]]*$ ]] && continue
  if [[ "$line" =~ ^\[([a-z]+)\] ]]; then section="${BASH_REMATCH[1]}"; continue; fi
  if [[ "$section" == layers && "$line" =~ ^[[:space:]]*([a-z_]+)[[:space:]]*=[[:space:]]*([0-9]+) ]]; then
    layer["${BASH_REMATCH[1]}"]="${BASH_REMATCH[2]}"
  elif [[ "$section" == allow && "$line" =~ ^[[:space:]]*\"([a-z_]+)\ -\>\ ([a-z_]+)\"[[:space:]]*=[[:space:]]*([0-9]+) ]]; then
    allow["${BASH_REMATCH[1]} -> ${BASH_REMATCH[2]}"]="${BASH_REMATCH[3]}"
  fi
done <"$LAYERS"

# The modules module $1 references, one line per reference. A module of the root
# crate (src/<m>, src/<m>.rs) names another as `crate::<m>` (src/lib.rs re-exports
# each crate under its module name). An extracted crate (<m>/src, package
# catalyst-<m>) is the module itself, so `crate::<x>` there is its own item unless its lib.rs
# aliases a dependency as <x> (`use catalyst_<d> as <x>;`). Either may also name a
# dependency in full (`catalyst_<d>::`).
refs_of() {
  local m="$1" p dir="${m//_/-}/src"
  for p in "src/$m" "src/$m.rs"; do
    [[ -e "$p" ]] || continue
    { rg --no-filename -o '\bcrate::[a-z_]+' "$p" || true; } | sed -E 's/^crate:://'
    { rg --no-filename -o '\bcatalyst_[a-z_]+::' "$p" || true; } | sed -E 's/^catalyst_//; s/::$//'
  done
  [[ -f "${m//_/-}/Cargo.toml" ]] || return 0
  {
    rg --no-filename -o -r 'alias $2$3 $1' '\buse catalyst_([a-z_]+)(?: as ([a-z_]+)|::([a-z_]+));' "$dir/lib.rs" || true
    rg --no-filename -o -r 'ref $1' '\bcrate::([a-z_]+)' "$dir" || true
    # (an alias line `use catalyst_<d>::<x>;` is already the alias above)
    rg --no-filename '\bcatalyst_[a-z_]+::' "$dir" | rg -v '^\s*pub\(crate\) use catalyst_[a-z_]+::[a-z_]+;' |
      rg -o -r 'dep $1' '\bcatalyst_([a-z_]+)::' || true
  } | awk '$1 == "alias" { a[$2] = $3; next } $1 == "ref" { if ($2 in a) print a[$2]; next } { print $2 }'
}

edges=$(
  for m in "${!layer[@]}"; do
    refs_of "$m" |
      awk -v m="$m" '$0 != m { n[$0]++ } END { for (t in n) print m, t, n[t] }'
  done | sort
)

if [[ $mode == edges ]]; then
  awk '{ print $1 " -> " $2 " " $3 }' <<<"$edges" | sort -k4,4nr
  exit 0
fi

fail=0 shrank=0
declare -A seen=()
while read -r from to n; do
  [[ -z "$from" ]] && continue
  [[ -n "${layer[$to]:-}" ]] || continue # not a module (crate:: of a leaf alias etc.)
  key="$from -> $to"
  seen["$key"]=1
  if ((layer[$to] < layer[$from])); then continue; fi
  max="${allow[$key]:-}"
  if [[ -z "$max" ]]; then
    echo "check-module-dag: new back-edge $key ($n refs; L${layer[$from]} -> L${layer[$to]})"
    fail=1
  elif ((n > max)); then
    echo "check-module-dag: back-edge $key grew to $n (allowed $max)"
    fail=1
  elif ((n < max)); then
    echo "check-module-dag: back-edge $key shrank to $n (allowed $max); tighten $LAYERS"
    shrank=1
  fi
done <<<"$edges"
for key in ${allow[@]+"${!allow[@]}"}; do
  [[ -n "${seen[$key]:-}" ]] || { echo "check-module-dag: back-edge $key is gone; remove it from $LAYERS"; shrank=1; }
done
# The shared crates in library/ sit below the application: their build and normal
# dependencies may name other library crates, never a service or an app crate
# (catalyst-*, dcl-scene-cli), by path or through the workspace.
for manifest in ../../library/*/Cargo.toml; do
  while IFS= read -r dep; do
    echo "check-module-dag: ${manifest#../../} depends on ${dep%% *}, outside library/"
    fail=1
  done < <(awk '/^\[/ { on = ($0 == "[dependencies]" || $0 == "[build-dependencies]") } on' "$manifest" |
    rg '^(catalyst-[a-z-]+|dcl-scene-cli)\b|path *= *"[^"]*\b(services|apps)/' || true)
done
((fail)) && exit 1
n_allowed=0
[[ ${allow[@]+set} ]] && n_allowed=${#allow[@]}
echo "check-module-dag: ok ($n_allowed allowed back-edges$( ((shrank)) && echo ', some can be tightened'))"
