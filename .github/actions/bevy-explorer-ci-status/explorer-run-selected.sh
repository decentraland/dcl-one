#!/usr/bin/env bash
# Answers one question for Bevy Explorer / CI status: did this Bevy Explorer / CI run select an
# explorer job, so the pull request changes the explorer and gets a CI status comment?
#
# Bevy Explorer / CI also runs for pull requests that change only the backend or the sites (its
# image jobs consume the explorer's web build), and its *Changes* job then selects no explorer
# job: no *Test Suite*, *Headless Smoke*, *Clippy* or *Web engine, HUD and bridge scene*. Such a
# run says nothing about the explorer, and the pull request gets no comment.
#
# The signal is the explorer jobs once *Changes* has completed: one that is not `skipped` means
# the run builds or tests the explorer. A *Changes* job that did not succeed selected nothing. A
# run that completed without *Changes* (cancelled while queued, or the integration guards failed
# first) selected nothing either.
#
# Prints `true` (an explorer job was selected; the sections may be written) or `false` (leave the
# pull request alone) on stdout.
#
# Endpoints omit the leading slash so Git Bash (MSYS) does not rewrite them into Windows paths
# when the script is run locally.
#
# Usage: RUN_ID=<id> REPO=owner/name [WAIT_SECONDS=0] [POLL_SECONDS=5] bash explorer-run-selected.sh
#
# WAIT_SECONDS is for the `requested` caller, which asks before *Changes* has run: 0 (the default)
# answers from the run's current state, which is all a `completed` caller needs. A run whose
# *Changes* job has not completed when the budget runs out answers `false`: the `completed` caller
# writes the sections anyway, so the pull request loses only the pending badges.
set -euo pipefail

: "${RUN_ID:?RUN_ID is required}"
: "${REPO:?REPO is required}"
WAIT_SECONDS="${WAIT_SECONDS:-0}"
POLL_SECONDS="${POLL_SECONDS:-5}"

# The job that selects the explorer jobs. Renaming it in bevy-explorer-ci.yml without updating
# this breaks the probe, so a completed run that has jobs but not this one is a hard error rather
# than a silent `false` that would leave every pull request without a comment.
CHANGES_JOB_NAME='Changes'
# The jobs *Changes* selects, by name; *Test Suite* carries its matrix in parentheses.
EXPLORER_JOB_PATTERN='^(Test Suite( \(.*\))?|Headless Smoke|Clippy|Web engine, HUD and bridge scene)$'

log() { echo "[explorer-run-selected] $*" >&2; }

deadline=$(( $(date +%s) + WAIT_SECONDS ))

while :; do
  jobs_json=$(gh api "repos/$REPO/actions/runs/$RUN_ID/jobs?per_page=100")

  changes=$(jq -c --arg name "$CHANGES_JOB_NAME" \
    'first(.jobs[] | select(.name == $name)) // empty' <<< "$jobs_json")

  if [ -n "$changes" ]; then
    status=$(jq -r '.status' <<< "$changes")
    conclusion=$(jq -r '.conclusion // ""' <<< "$changes")
    log "$CHANGES_JOB_NAME: status=$status conclusion=${conclusion:-null}"

    if [ "$status" = "completed" ]; then
      if [ "$conclusion" != "success" ]; then
        log "$CHANGES_JOB_NAME ended $conclusion, so no explorer job was selected"
        echo false
        exit 0
      fi
      selected=$(jq -r --arg re "$EXPLORER_JOB_PATTERN" \
        '[.jobs[] | select((.name | test($re)) and .conclusion != "skipped") | .name] | join(", ")' <<< "$jobs_json")
      if [ -n "$selected" ]; then
        log "selected: $selected"
        echo true
        exit 0
      fi
      log "every explorer job is skipped: the run does not build or test the explorer"
      echo false
      exit 0
    fi
  else
    job_count=$(jq '.jobs | length' <<< "$jobs_json")
    run_status=$(gh api "repos/$REPO/actions/runs/$RUN_ID" --jq '.status')
    log "no $CHANGES_JOB_NAME job yet (jobs=$job_count, run status=$run_status)"

    if [ "$run_status" = "completed" ]; then
      if [ "$job_count" -gt 0 ]; then
        names=$(jq -r '[.jobs[].name] | join(", ")' <<< "$jobs_json")
        echo "::error::Run $RUN_ID has jobs but none named '$CHANGES_JOB_NAME' (found: $names). Update CHANGES_JOB_NAME in $(basename "${BASH_SOURCE[0]}") to match bevy-explorer-ci.yml." >&2
        exit 2
      fi
      log "run finished without starting a job (cancelled while queued): nothing selected"
      echo false
      exit 0
    fi
  fi

  now=$(date +%s)
  if [ "$now" -ge "$deadline" ]; then
    log "$CHANGES_JOB_NAME has not completed after ${WAIT_SECONDS}s; answering false, the completed run decides"
    echo false
    exit 0
  fi

  sleep "$POLL_SECONDS"
done
