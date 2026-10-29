#!/usr/bin/env bash
# The GitHub Actions policy of this repository, applied with the gh CLI by a repository admin:
#   tooling/repo-actions-policy.sh decentraland/dcl-one
# Idempotent; prints the resulting settings. The workflow files carry the rest of the policy
# (declared permissions, commit-pinned actions, source-repository checks on workflow_run).
#
# - GITHUB_TOKEN is read-only unless a workflow asks for more, and can never approve a PR.
# - Only GitHub's own actions, verified creators' actions and the third-party actions the
#   workflows pin may run; a new action needs a line here before a workflow can use it.
# - A workflow run of a pull request from outside the repository waits for a maintainer's
#   approval every time, not only for first-time contributors.
set -euo pipefail
repo=${1:?usage: $0 owner/repo}

gh api -X PUT "repos/$repo/actions/permissions/workflow" \
  -f default_workflow_permissions=read -F can_approve_pull_request_reviews=false
gh api -X PUT "repos/$repo/actions/permissions" -F enabled=true -f allowed_actions=selected
gh api -X PUT "repos/$repo/actions/permissions/selected-actions" \
  -F github_owned_allowed=true -F verified_allowed=true \
  -f 'patterns_allowed[]=decentraland/*' \
  -f 'patterns_allowed[]=dtolnay/rust-toolchain@*' \
  -f 'patterns_allowed[]=Swatinem/rust-cache@*' \
  -f 'patterns_allowed[]=cachix/install-nix-action@*' \
  -f 'patterns_allowed[]=Mozilla-Actions/sccache-action@*' \
  -f 'patterns_allowed[]=docker/setup-buildx-action@*'
# GitHub only accepts this setting for a public repository: on a private one the call is skipped,
# and the script is run again once the repository is public.
if [ "$(gh api "repos/$repo" --jq .private)" = false ]; then
  gh api -X PUT "repos/$repo/actions/permissions/fork-pr-contributor-approval" \
    -f approval_policy=all_external_contributors
else
  echo "fork PR approval: skipped, the repository is private; run this script again after it goes public"
fi

echo "workflow token:   $(gh api "repos/$repo/actions/permissions/workflow" --jq '"\(.default_workflow_permissions), approve PRs: \(.can_approve_pull_request_reviews)"')"
echo "allowed actions:  $(gh api "repos/$repo/actions/permissions" --jq .allowed_actions) $(gh api "repos/$repo/actions/permissions/selected-actions" --jq '"github-owned=\(.github_owned_allowed) verified=\(.verified_allowed) patterns=\(.patterns_allowed | join(" "))"')"
[ "$(gh api "repos/$repo" --jq .private)" = false ] && echo "fork PR approval: $(gh api "repos/$repo/actions/permissions/fork-pr-contributor-approval" --jq .approval_policy)" || true
