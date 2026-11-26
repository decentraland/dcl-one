#!/usr/bin/env bash
# Sets the sections of the pull request's "Creator Hub test builds" comment to the state of this
# run, so that the comment never shows the builds of an earlier commit as current:
#
#   pending  At the start of the run. A section that shows the result of a run, and the desktop
#            section when this run builds the desktop app, show this run as in progress. Without
#            the ci:desktop or ci:linux label, an earlier desktop result changes to "Not built".
#   ended    At the end of the run. A section that still shows this run as in progress shows that
#            the run made no test build.
#
# section.sh writes the result of a build over the pending state.
#
# Environment: STATE (pending | ended), RUN_ID, RUN_URL, HEAD_SHA, DESKTOP (true when this run
# builds the desktop app), INSPECTOR_CHANGED and ASSET_PACKS_CHANGED (ended: the `changed` output
# of the package workflow), and for upsert-ci-status.sh GH_TOKEN, REPO and PR_NUMBER.
set -euo pipefail

: "${STATE:?STATE is required}"
: "${RUN_ID:?RUN_ID is required}"
: "${RUN_URL:?RUN_URL is required}"
: "${HEAD_SHA:?HEAD_SHA is required}"
UPSERT="${UPSERT:-$(dirname "${BASH_SOURCE[0]}")/../bevy-explorer-ci-status/upsert-ci-status.sh}"

# Each result that section.sh writes has this footer, and the states below have it too.
ANY_RUN='<sub>Run summary: ['
FOOTER="<sub>Run summary: [$RUN_ID]($RUN_URL) · Commit: \`${HEAD_SHA:0:7}\`"
IN_PROGRESS="$FOOTER · In progress</sub>"

# write SECTION IF_SECTION_HAS BODY
write() { COMMENT=creator-hub SECTION="$1" IF_SECTION_HAS="$2" SECTION_BODY="$3" bash "$UPSERT"; }

# badge LABEL MESSAGE COLOR LOGO
badge() {
  printf '[![%s](https://img.shields.io/badge/%s-%s-%s?logo=%s&logoColor=white&style=for-the-badge)](%s)' \
    "$1" "${1// /%20}" "${2// /%20}" "$3" "$4" "$RUN_URL"
}

case "$STATE" in
  pending)
    if [ "${DESKTOP:-}" = true ]; then
      write desktop '' "$(badge 'Desktop app' 'Pending!' yellow electron)

This run builds the desktop app. The downloads show here when the builds end.

$IN_PROGRESS"
    else
      write desktop "$ANY_RUN" "$(badge 'Desktop app' 'Not built' lightgrey electron)

_To build the desktop app, add the \`ci:desktop\` label, or \`ci:linux\` for Linux alone, then push or reopen._"
    fi
    write inspector "$ANY_RUN" "$(badge Inspector 'Pending!' yellow npm)

This run builds \`@dcl/inspector\` if it changed. The package shows here when it is published.

$IN_PROGRESS"
    write asset-packs "$ANY_RUN" "$(badge 'Asset packs' 'Pending!' yellow npm)

This run builds \`@dcl/asset-packs\` if it changed. The package shows here when it is published.

$IN_PROGRESS"
    ;;
  ended)
    # not_built SECTION LABEL LOGO PRODUCT CHANGED
    not_built() {
      local why="This run stopped before it published $4."
      [ "$5" = false ] && why="$4 did not change since its last release. This run did not build it."
      write "$1" "$IN_PROGRESS" "$(badge "$2" 'Not built' lightgrey "$3")

$why

$FOOTER</sub>"
    }
    not_built desktop 'Desktop app' electron 'the desktop app' ''
    not_built inspector Inspector npm '`@dcl/inspector`' "${INSPECTOR_CHANGED:-}"
    not_built asset-packs 'Asset packs' npm '`@dcl/asset-packs`' "${ASSET_PACKS_CHANGED:-}"
    ;;
  *) echo "::error::Unknown state '$STATE'." >&2; exit 2 ;;
esac
