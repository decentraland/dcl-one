#!/usr/bin/env bash
# Answers one question for the CI status writers: which pull request does this workflow_run of
# Bevy Explorer / CI report into?
#
# For a run GitHub tied to a pull request, the payload's pull_requests[0] is the answer. GitHub
# leaves pull_requests empty for some runs of a branch that has an open pull request (a run it
# started before the pull request was opened, a fork's pull request), so those are looked up by
# the commit: the open pull request of this repository whose head is the run's commit. A push run
# on main, or a run of a branch with no open pull request, reports nowhere.
#
# Prints GITHUB_OUTPUT lines on stdout:
#   pr-number   the pull request to write to, or empty when the run belongs to none
#   head-ref    the pull request's head branch
#   head-sha    the commit the run built or tested
#
# A failed lookup resolves to no pull request with a warning rather than failing the caller: the
# writers must never red a run over a lookup blip.
#
# Usage: WORKFLOW_RUN_EVENT_OBJ='<json>' REPO=owner/name bash resolve-comment-pr.sh >> "$GITHUB_OUTPUT"
set -euo pipefail

: "${WORKFLOW_RUN_EVENT_OBJ:?WORKFLOW_RUN_EVENT_OBJ is required}"
: "${REPO:?REPO is required}"

field() { jq -r "$1 // \"\"" <<< "$WORKFLOW_RUN_EVENT_OBJ"; }

number=$(field '.pull_requests[0].number')
head_ref=""
head_sha=$(field '.head_sha')

if [ -n "$number" ]; then
  head_ref=$(field '.pull_requests[0].head.ref')
  head_sha=$(field '.pull_requests[0].head.sha // .head_sha')
else
  branch=$(field '.head_branch')
  if [ "$branch" != "main" ] && [ -n "$head_sha" ]; then
    # The endpoint omits the leading slash so Git Bash (MSYS) does not rewrite it into a Windows
    # path when the script is run locally.
    if pulls=$(gh api "repos/$REPO/commits/$head_sha/pulls?per_page=50"); then
      match=$(jq -c --arg sha "$head_sha" --arg repo "$REPO" '
        first(.[] | select(.state == "open" and .head.sha == $sha and .head.repo.full_name == $repo)) // empty' <<< "$pulls")
      if [ -n "$match" ]; then
        number=$(jq -r '.number' <<< "$match")
        head_ref=$(jq -r '.head.ref' <<< "$match")
        echo "::notice::The run of ${head_sha:0:7} on ${branch} is the head of pull request #${number}; its result reports there." >&2
      fi
    else
      echo "::warning::Could not list the pull requests of ${head_sha:0:7}; reporting nowhere." >&2
    fi
  fi
fi

echo "pr-number=${number}"
echo "head-ref=${head_ref}"
echo "head-sha=${head_sha}"
