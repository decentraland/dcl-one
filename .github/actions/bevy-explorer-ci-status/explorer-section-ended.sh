#!/usr/bin/env bash
# Answers one question for Bevy Explorer / CI status: have the jobs of one section (SECTION=tests
# or build) of this Bevy Explorer / CI run ended while the run itself still goes on, so that
# section can be written now?
#
# The run ends only when its slowest job ends: the image jobs often outlast the tests, and the
# preview publication outlasts the build. The `requested` caller asks this after it wrote the
# pending sections; on `true` it writes the section from the run's current job list, and the
# `completed` caller later writes both sections as before.
#
# tests: *Headless Smoke* and every *Test Suite* job. The matrix of *Test Suite* is listed in one
# step, so one listed platform means all are listed.
# build: the products, *Web engine, HUD and bridge scene*, *Web package for the CDN* and *Headless
# Smoke*. *Publish the web package* must be listed too, so the section knows whether a preview
# comes, but it may still run.
# The jobs have ended when all of them are listed and completed.
#
# Prints `true` (the jobs ended, one of them ran, and the run has not ended) or `false` on stdout.
# `false` covers: every job skipped (a draft, or unchanged inputs), the run ended first
# (cancelled, or a failed guard), and the wait budget ran out. In each case the `completed` caller
# reports, so the pull request loses only the earlier result.
#
# Usage: SECTION=tests|build RUN_ID=<id> REPO=owner/name [WAIT_SECONDS=0] [POLL_SECONDS=120] \
#          bash explorer-section-ended.sh
#
# WAIT_SECONDS is the wait budget; 0 (the default) answers from the run's current state.
# POLL_SECONDS is the longest pause between two reads; the pauses start at 60 seconds and double.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/retry.sh"

: "${SECTION:?SECTION is required (tests or build)}"
: "${RUN_ID:?RUN_ID is required}"
: "${REPO:?REPO is required}"
WAIT_SECONDS="${WAIT_SECONDS:-0}"
# Every read spends the repository's GITHUB_TOKEN budget of 1,000 requests an hour.
POLL_SECONDS="${POLL_SECONDS:-120}"

# The same names as the sections of explorer-ci-sections.sh, one pattern per job (a matrix is
# one pattern). ENDED: listed and completed; LISTED: listed only.
case "$SECTION" in
  tests)
    ENDED='["^Headless Smoke$", "^Test Suite( \\(.*\\))?$"]'
    LISTED='[]' ;;
  build)
    ENDED='["^Web engine, HUD and bridge scene$", "^Web package for the CDN$", "^Headless Smoke$"]'
    LISTED='["^Publish the web package$"]' ;;
  *) echo "SECTION must be tests or build, not '$SECTION'" >&2; exit 64 ;;
esac

log() { echo "[explorer-section-ended $SECTION] $*" >&2; }

run_status() { gh api "repos/$REPO/actions/runs/$RUN_ID" --jq '.status'; }

deadline=$(( $(date +%s) + WAIT_SECONDS ))
attempt=1

while :; do
  jobs_json=$(gh api "repos/$REPO/actions/runs/$RUN_ID/jobs?per_page=100")

  # listed: every pattern names a job in the list. open: ENDED jobs not completed. ran: ENDED jobs
  # that completed and were not skipped. idle: no job of the run is open.
  IFS=$'\t' read -r listed open ran idle <<< "$(jq -r --argjson ended "$ENDED" --argjson listed "$LISTED" '
    .jobs as $jobs
    | [$jobs[] | .name as $n | select(any($ended[]; . as $re | $n | test($re)))] as $section
    | [(all(($ended + $listed)[]; . as $re | any($jobs[]; .name | test($re))) | tostring),
       ($section | map(select(.status != "completed")) | length),
       ($section | map(select(.status == "completed" and .conclusion != "skipped")) | length),
       (all($jobs[]; .status == "completed") | tostring)] | @tsv' <<< "$jobs_json" | tr -d '\r')"
  log "jobs: listed=$listed open=$open ran=$ran idle=$idle"

  if [ "$listed" = true ] && [ "$open" -eq 0 ]; then
    if [ "$ran" -eq 0 ]; then
      log "every job is skipped: the completed run reports"
      echo false
      exit 0
    fi
    run_state=in_progress
    if [ "$idle" = true ]; then run_state=$(run_status); fi
    if [ "$run_state" = completed ]; then
      log "the run ended with these jobs: the completed run reports"
      echo false
      exit 0
    fi
    log "the jobs ended and the run goes on"
    echo true
    exit 0
  fi

  # A run with an open job has not ended; one that ended starts no more jobs.
  run_state=in_progress
  if [ "$listed" != true ] && [ "$idle" = true ]; then run_state=$(run_status); fi
  if [ "$run_state" = completed ]; then
    log "the run ended before its jobs were listed: the completed run reports"
    echo false
    exit 0
  fi

  now=$(date +%s)
  if [ "$now" -ge "$deadline" ]; then
    log "the jobs have not ended after ${WAIT_SECONDS}s: the completed run reports"
    echo false
    exit 0
  fi

  remaining=$((deadline - now))
  delay=$(backoff_delay "$attempt" "$POLL_SECONDS" 60)
  sleep_for "$delay" "$remaining"
  attempt=$((attempt + 1))
done
