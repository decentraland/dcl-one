#!/usr/bin/env bash
# Writes the build and tests sections of the CI status comment for a completed Bevy Explorer / CI
# run, from the run's job and artifact lists (the trusted Actions API; the run's own code reaches
# this only as job names and artifact names, which render inside table cells).
#
# Environment: REPO (owner/name), RUN_ID, RUN_URL, HEAD_SHA (the commit the run built), JOBS_FILE
# (the run's /jobs response), ARTIFACTS_FILE (its /artifacts response) and OUT_DIR (where build.md
# and tests.md are written; the working directory by default). PREVIEW_VERSION, when set, is the
# version of @dcl-regenesislabs/bevy-explorer-web this run published under the pull request's pr-<n>
# tag; the web app row then links its CDN preview, on the zone services.
#
# The build section covers the products: the web app (*Web engine, HUD and bridge scene*) and the
# headless engine (*Headless Smoke*), with their artifacts' download links, and the `benchmark`
# runners the *Test Suite* uploads. The tests section covers *Test Suite* on each platform and
# *Headless Smoke*. A job *Changes* did not select (`skipped`) is reported as not selected; a
# section none of whose jobs ran says its inputs are unchanged.
#
# Usage: REPO=o/r RUN_ID=1 RUN_URL=... HEAD_SHA=... JOBS_FILE=jobs.json ARTIFACTS_FILE=artifacts.json \
#          bash explorer-ci-sections.sh
set -euo pipefail

: "${REPO:?REPO is required}"
: "${RUN_ID:?RUN_ID is required}"
: "${RUN_URL:?RUN_URL is required}"
: "${HEAD_SHA:?HEAD_SHA is required}"
: "${JOBS_FILE:?JOBS_FILE is required}"
: "${ARTIFACTS_FILE:?ARTIFACTS_FILE is required}"
OUT_DIR="${OUT_DIR:-.}"

WEB_JOB='Web engine, HUD and bridge scene'
HEADLESS_JOB='Headless Smoke'
TEST_JOB_PREFIX='Test Suite'
BADGE_STYLE='logoColor=white&style=for-the-badge'

# Same h/m/s convention as the benchmark report's durations.
fmt_secs() {
  awk -v s="$1" 'BEGIN { s=int(s+0.5); h=int(s/3600); m=int(s%3600/60); r=s%60;
    if (h>0) printf("%dh %dm", h, m); else if (m>0) printf("%dm %ds", m, r); else printf("%ds", r) }'
}

# A job's fields by exact name: conclusion, html_url and wall time in seconds, tab-separated;
# empty when the run has no such job. jq's output loses any carriage return, so a Windows jq
# works too.
job_fields() {
  local conclusion url started completed secs=""
  IFS=$'	' read -r conclusion url started completed <<< "$(jq -r --arg n "$1" '
    first(.jobs[] | select(.name == $n)) // empty
    | [(.conclusion // ""), (.html_url // ""), (.started_at // ""), (.completed_at // "")] | @tsv' "$JOBS_FILE" | tr -d '\r')" || true
  [ -n "${url:-}${conclusion:-}" ] || return 0
  if [ -n "$started" ] && [ -n "$completed" ]; then
    secs=$(( $(date -u -d "$completed" +%s) - $(date -u -d "$started" +%s) ))
  fi
  printf '%s	%s	%s' "$conclusion" "$url" "$secs"
}

# The names of the jobs starting with $1, sorted.
job_names_with_prefix() {
  jq -r --arg p "$1" '[.jobs[] | select(.name | startswith($p)) | .name] | sort | .[]' "$JOBS_FILE" | tr -d '\r'
}

# The result cell of a job conclusion. $2 names the success wording.
result_cell() {
  case "$1" in
    success) printf '✅ %s' "$2" ;;
    failure) printf '❌ Failed' ;;
    cancelled) printf '⚠️ Cancelled' ;;
    timed_out) printf '⚠️ Timed out' ;;
    skipped) printf '— Not selected' ;;
    '') printf '🕓 In progress' ;;
    *) printf '⚠️ `%s`' "$1" ;;
  esac
}

# Folds a job conclusion into a section state: failed beats cancelled beats passed; skipped and
# absent jobs leave it as it is.
fold_state() {
  local state="$1" conclusion="$2"
  case "$conclusion" in
    success) [ "$state" = none ] && state=passed ;;
    skipped|'') ;;
    cancelled) [ "$state" != failed ] && state=cancelled ;;
    *) state=failed ;;
  esac
  printf '%s' "$state"
}

# The download link of the newest unexpired artifact whose name starts with $1, labelled $2;
# empty when the run has none.
artifact_link() {
  local id
  id=$(jq -r --arg p "$1" '[.artifacts[] | select(.expired == false and (.name | startswith($p)))]
    | max_by(.created_at) | .id // empty' "$ARTIFACTS_FILE")
  [ -n "$id" ] && printf '[%s](https://github.com/%s/actions/runs/%s/artifacts/%s)' "$2" "$REPO" "$RUN_ID" "$id"
  return 0
}

job_time() { if [[ "$1" =~ ^[0-9]+$ ]]; then fmt_secs "$1"; else printf '—'; fi; }
job_cell() { if [ -n "$2" ]; then printf '[%s](%s)' "$1" "$2"; else printf '%s' "$1"; fi; }

SHORT_SHA="${HEAD_SHA:0:7}"
FOOTER="<sub>[Run summary]($RUN_URL) · commit \`$SHORT_SHA\`</sub>"

# --- build -----------------------------------------------------------------------------------
state=none
rows=""
IFS=$'\t' read -r conclusion url secs <<< "$(job_fields "$WEB_JOB")" || true
if [ -n "${url:-}${conclusion:-}" ]; then
  state=$(fold_state "$state" "$conclusion")
  downloads=$(artifact_link 'bevy-explorer-web-' 'web app')
  if [ "$conclusion" = success ] && [[ "${PREVIEW_VERSION:-}" =~ ^[0-9A-Za-z.-]+$ ]]; then
    preview="[preview](https://cdn.decentraland.org/@dcl-regenesislabs/bevy-explorer-web/$PREVIEW_VERSION/index.html?position=0%2C-2&baseDomain=decentraland.zone)"
    downloads="${downloads:+$downloads · }$preview"
  fi
  rows="$rows| $(job_cell 'Web app' "$url") | $(result_cell "$conclusion" Built) | $(job_time "$secs") | ${downloads:-—} |"$'\n'
fi
IFS=$'\t' read -r conclusion url secs <<< "$(job_fields "$HEADLESS_JOB")" || true
if [ -n "${url:-}${conclusion:-}" ]; then
  state=$(fold_state "$state" "$conclusion")
  downloads=""
  link=$(artifact_link 'bevy-headless-linux-x64-' 'npm package'); [ -n "$link" ] && downloads="$downloads · $link"
  link=$(artifact_link 'bevy-explorer-headless-linux-amd64-' 'engine'); [ -n "$link" ] && downloads="$downloads · $link"
  downloads="${downloads# · }"
  rows="$rows| $(job_cell 'Headless engine (Linux)' "$url") | $(result_cell "$conclusion" Built) | $(job_time "$secs") | ${downloads:-—} |"$'\n'
fi

runners=""
while IFS=$'\t' read -r name id; do
  [ -n "$name" ] || continue
  label=$(sed -E 's/^bevy-explorer-benchmark-([a-z]+-[a-z0-9]+)-[0-9a-f]{40}$/\1/' <<< "$name")
  runners="$runners · [$label](https://github.com/$REPO/actions/runs/$RUN_ID/artifacts/$id)"
done <<< "$(jq -r '[.artifacts[] | select(.expired == false and (.name | startswith("bevy-explorer-benchmark-")))]
  | sort_by(.name) | .[] | [.name, (.id | tostring)] | @tsv' "$ARTIFACTS_FILE" | tr -d '\r')"
runners="${runners# · }"

case "$state" in
  passed) badge="Build-Success!-3fb950"; headline="The explorer built." ;;
  failed) badge="Build-Failed!-ff0000"; headline="A build failed. Check the [run]($RUN_URL) to see what went wrong." ;;
  cancelled) badge="Build-Cancelled-lightgrey"; headline="The run was cancelled. Push a new commit or re-run the workflow to refresh this section." ;;
  *) badge="Build-Not%20selected-lightgrey"; headline="No explorer build was selected: the web and headless inputs are unchanged, so the consumers reuse main's builds." ;;
esac
{
  echo "[![Build](https://img.shields.io/badge/$badge?logo=webassembly&$BADGE_STYLE)]($RUN_URL)"
  echo
  echo "$headline"
  if [ -n "$rows" ]; then
    echo
    echo "| Product | Result | Job time | Download |"
    echo "| --- | --- | ---: | --- |"
    printf '%s' "$rows"
  fi
  echo
  if [ -n "$runners" ]; then
    echo "<sub>Benchmark runner: $runners</sub>"
    echo
  fi
  echo "$FOOTER"
} > "$OUT_DIR/build.md"

# --- tests -----------------------------------------------------------------------------------
state=none
rows=""
while IFS= read -r name; do
  [ -n "$name" ] || continue
  IFS=$'\t' read -r conclusion url secs <<< "$(job_fields "$name")" || true
  state=$(fold_state "$state" "$conclusion")
  case "$name" in
    *windows*) display='Test Suite (Windows)' ;;
    *ubuntu*) display='Test Suite (Linux)' ;;
    *macos*) display='Test Suite (macOS)' ;;
    *) display="$name" ;;
  esac
  # A job name comes from the workflow file, but a pull request's own copy of it writes the name,
  # so it renders as text: no pipes, no backticks.
  display=$(tr -d '|`' <<< "$display")
  rows="$rows| $(job_cell "$display" "$url") | $(result_cell "$conclusion" Passed) | $(job_time "$secs") |"$'\n'
done <<< "$(job_names_with_prefix "$TEST_JOB_PREFIX")"
IFS=$'\t' read -r conclusion url secs <<< "$(job_fields "$HEADLESS_JOB")" || true
if [ -n "${url:-}${conclusion:-}" ]; then
  state=$(fold_state "$state" "$conclusion")
  rows="$rows| $(job_cell "$HEADLESS_JOB" "$url") | $(result_cell "$conclusion" Passed) | $(job_time "$secs") |"$'\n'
fi

case "$state" in
  passed) badge="Tests-Passed!-3fb950"; headline="All explorer tests passed ✅" ;;
  failed) badge="Tests-Failed!-ff0000"; headline="Some explorer tests failed ❌ Check the [run]($RUN_URL) for the failing tests." ;;
  cancelled) badge="Tests-Cancelled-lightgrey"; headline="The run was cancelled. Push a new commit or re-run the workflow to refresh this section." ;;
  *) badge="Tests-Not%20selected-lightgrey"; headline="No test job was selected: the test inputs are unchanged." ;;
esac
{
  echo "[![Tests](https://img.shields.io/badge/$badge?logo=rust&$BADGE_STYLE)]($RUN_URL)"
  echo
  echo "$headline"
  if [ -n "$rows" ]; then
    echo
    echo "| Suite | Result | Job time |"
    echo "| --- | --- | ---: |"
    printf '%s' "$rows"
  fi
  echo
  echo "$FOOTER"
} > "$OUT_DIR/tests.md"
