#!/usr/bin/env bash
# Create or update the one CI status comment on a pull request, replacing only the given sections
# (build | tests | performance | automation | inworld). The writers call this through the
# bevy-explorer-ci-status composite action: Bevy Explorer / CI status (build, tests) and Bevy Explorer /
# Benchmark (performance). Each writer's bot comment collapses into one.
#
# COMMENT=creator-hub writes the Creator Hub's comment instead, the same way: its own marker and
# heading, and the sections desktop | inspector | asset-packs (the Creator Hub / CI jobs that
# publish a test build of the pull request).
#
# The comment is keyed by the hidden <!-- bevy-explorer-ci-status --> marker and holds the $HEADER
# heading plus one fenced block per section:
#
#   <!-- bevy-explorer-ci-status -->
#   ### 🚦 Bevy Explorer CI
#   <!-- ci:build:start -->       …build…       <!-- ci:build:end -->
#   <!-- ci:tests:start -->       …tests…       <!-- ci:tests:end -->
#   <!-- ci:performance:start --> …performance… <!-- ci:performance:end -->
#   <!-- ci:automation:start -->  …automation…  <!-- ci:automation:end -->
#   <!-- ci:inworld:start -->     …inworld…     <!-- ci:inworld:end -->
#
# The writers run as independent workflows that can fire at the same time, so a plain
# read-modify-write would drop a section or create a duplicate comment. Each attempt collapses any
# duplicates (keeping the oldest), rewrites only its own sections on that comment, then re-reads to
# confirm they landed and no duplicate slipped in, retrying otherwise.
#
# Environment: REPO (owner/name), PR_NUMBER, SECTION, COMMENT (bevy-explorer when unset), and the
# body in SECTION_BODY or, for a body too large to pass comfortably through the environment, in
# the file SECTION_BODY_FILE. SECTIONS_DIR instead of SECTION writes each <section>.md in it, all
# in one read-modify-write. With IF_SECTION_HAS set (SECTION only), the script writes only over a
# section that has that text, and writes nothing when the comment or the section is not there.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/retry.sh"

# Each comment has its marker, its heading, its allowed sections, and the sections a fresh
# comment starts with (SKELETON).
case "${COMMENT:-bevy-explorer}" in
  bevy-explorer)
    MARKER="<!-- bevy-explorer-ci-status -->"
    HEADER='### 🚦 Bevy Explorer CI'
    SECTIONS='build tests performance automation inworld'
    SKELETON='build tests performance'
    ;;
  creator-hub)
    MARKER="<!-- creator-hub-test-builds -->"
    HEADER='### 🧪 Creator Hub test builds'
    SECTIONS='desktop inspector asset-packs'
    SKELETON='desktop'
    ;;
  *) echo "::error::Unknown comment '${COMMENT:-}'."; exit 2 ;;
esac

NAMES=()
if [ -n "${SECTIONS_DIR:-}" ]; then
  if [ -n "${SECTION:-}${SECTION_BODY:-}${SECTION_BODY_FILE:-}" ]; then
    echo "::error::Give SECTION with its body, or SECTIONS_DIR, not both."
    exit 2
  fi
  if [ -n "${IF_SECTION_HAS:-}" ]; then
    echo "::error::IF_SECTION_HAS works with SECTION only, not with SECTIONS_DIR."
    exit 2
  fi
  for file in "$SECTIONS_DIR"/*.md; do
    if [ -e "$file" ]; then
      NAMES+=("$(basename "$file" .md)")
    fi
  done
  if [ "${#NAMES[@]}" -eq 0 ]; then
    echo "::error::No section files in '$SECTIONS_DIR'."
    exit 2
  fi
else
  NAMES=("${SECTION:-}")
fi

# Fail fast on a section name outside the fence set: an unknown name would append a dead fence
# to the shared comment and then wedge the survive check for 5 attempts, burning about 15 API
# calls per write from then on.
for name in "${NAMES[@]}"; do
  known=false
  for allowed in $SECTIONS; do
    if [ "$allowed" = "$name" ]; then
      known=true
    fi
  done
  if [ "$known" != true ]; then
    echo "::error::Unknown section '$name'."
    exit 2
  fi
done
LABEL="$(printf "'%s', " "${NAMES[@]}")"
LABEL="${LABEL%, }"

BOT="github-actions[bot]"

# Neutral placeholder for a section that has not reported yet. Used only when seeding a brand-new
# comment or appending a missing fence; a real run always overwrites its own.
section_default() {
  case "$1" in
    build) printf '![Build](https://img.shields.io/badge/Build-Waiting-lightgrey?logo=webassembly&logoColor=white&style=for-the-badge)\n\n_Waiting for Bevy Explorer / CI to build the explorer…_' ;;
    tests) printf '![Tests](https://img.shields.io/badge/Tests-Waiting-lightgrey?logo=rust&logoColor=white&style=for-the-badge)\n\n_Waiting for the test suite…_' ;;
    performance) printf '![Performance](https://img.shields.io/badge/Performance-Waiting-lightgrey?logo=speedtest&logoColor=white&style=for-the-badge)\n\n_Waiting for a passed CI run with the web app built: the benchmark then measures it against main in Chrome on the laptop fleet, and the report lands here._' ;;
    automation) printf '![Automation](https://img.shields.io/badge/Automation-Waiting-lightgrey?logo=github&logoColor=white&style=for-the-badge)\n\n_Waiting for the automation suite…_' ;;
    inworld) printf '![InWorld](https://img.shields.io/badge/InWorld-Waiting-lightgrey?logo=github&logoColor=white&style=for-the-badge)\n\n_Waiting for the InWorld suite…_' ;;
    desktop) printf '![Desktop app](https://img.shields.io/badge/Desktop%%20app-Not%%20built-lightgrey?logo=electron&logoColor=white&style=for-the-badge)\n\n_To build the desktop app, add the `ci:desktop` label, or `ci:linux` for Linux alone, then push or reopen._' ;;
    inspector) printf '![Inspector](https://img.shields.io/badge/Inspector-Waiting-lightgrey?logo=npm&logoColor=white&style=for-the-badge)\n\n_Waiting for the inspector package…_' ;;
    asset-packs) printf '![Asset packs](https://img.shields.io/badge/Asset%%20packs-Waiting-lightgrey?logo=npm&logoColor=white&style=for-the-badge)\n\n_Waiting for the asset packs package…_' ;;
  esac
}

# One section, fenced by its start/end markers.
wrap_section() { printf '<!-- ci:%s:start -->\n%s\n<!-- ci:%s:end -->' "$1" "$2" "$1"; }

# A fresh comment with every always-present section defaulted to waiting. automation and inworld
# are deliberately absent: no suite writes them yet, and a permanent waiting row on every pull
# request would be noise. The append-missing-fence path below adds one the first time it reports.
# The Creator Hub's package sections are absent the same way: a pull request that changes no
# package never writes them.
skeleton() {
  local section
  printf '%s\n%s\n' "$MARKER" "$HEADER"
  for section in $SKELETON; do
    printf '\n%s\n' "$(wrap_section "$section" "$(section_default "$section")")"
  done
}

# GitHub caps an issue comment at 65536 chars across every section; keep one writer from
# consuming the whole budget and failing an unrelated section's PATCH with an opaque 422.
# Truncation is fine for a status section that already links out to the full report.
#
# The cap is per section and blind to the others, so it only bounds the total if
# (section count × CAP) stays under 65536 with headroom for the header and the fences. A comment
# has at most 5 allowed sections (see above): 5 × 12000 = 60000, leaving about 5.5k. Re-derive
# this when a section is added, or the ${#NEW_BODY} guard below is the only thing left between a big comment
# and a 422.
CAP=12000

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

# Section $1's body $2 as it lands in the comment, in a file so awk splices it verbatim.
prepare_body() {
  local name="$1" body="$2" note cut truncated opens closes
  # Everything below matches markers as whole lines, which CRLF endings defeat, and GitHub's web
  # editor resubmits an edited comment with \r\n. Normalize the body here and every API read below,
  # so one manual edit cannot make each writer append a duplicate fence beneath a stale, still
  # rendering one.
  body="${body//$'\r'/}"
  if [ "${#body}" -gt "$CAP" ]; then
    echo "::warning::Section '$name' body is ${#body} chars; truncating to $CAP."
    note=$'\n\n'"_…truncated; see the linked run for the full report._"
    # Cut with headroom for the note, then close constructs the cut may have severed: an
    # unterminated code fence or <details> makes GitHub render everything after it in this comment
    # inside the open block, visually eating the neighbouring sections. The appended closers count
    # against the cap too, so re-cut and re-balance until the finished body fits inside it.
    cut=$((CAP - ${#note}))
    while :; do
      truncated="${body:0:cut}"
      if [ $(( $(grep -c '^```' <<< "$truncated") % 2 )) -ne 0 ]; then
        truncated="$truncated"$'\n''```'
      fi
      opens=$(grep -oi '<details' <<< "$truncated" | wc -l || true)
      closes=$(grep -oi '</details' <<< "$truncated" | wc -l || true)
      while [ "${opens:-0}" -gt "${closes:-0}" ]; do
        truncated="$truncated"$'\n</details>'
        closes=$((closes + 1))
      done
      truncated="$truncated$note"
      if [ "${#truncated}" -le "$CAP" ]; then
        break
      fi
      cut=$((cut - (${#truncated} - CAP)))
      if [ "$cut" -lt 0 ]; then
        cut=0
      fi
    done
    body="$truncated"
  fi
  # Parts of a body (job names, report notes) originate in the pull request's own run, so drop
  # any line shaped like a section marker: a body line must never open or close a section fence,
  # or it would scramble the comment structure and wedge the survive check below.
  printf '%s\n' "$body" \
    | grep -vE '^[[:space:]]*<!-- (ci:[^>]*|bevy-explorer-ci-status|creator-hub-test-builds) -->[[:space:]]*$' > "$WORK_DIR/$name.md" || true
}

# Read each body in its own assignment: set -e ignores a failed cat inside function arguments.
if [ -n "${SECTIONS_DIR:-}" ]; then
  for name in "${NAMES[@]}"; do
    body="$(cat "$SECTIONS_DIR/$name.md")"
    prepare_body "$name" "$body"
  done
elif [ -n "${SECTION_BODY_FILE:-}" ]; then
  body="$(cat "$SECTION_BODY_FILE")"
  prepare_body "$SECTION" "$body"
else
  prepare_body "$SECTION" "${SECTION_BODY:-}"
fi

# Replace the content of section $2 in $1 with its prepared body. Hand edits are repaired: a
# fence missing its end marker is closed before the next section's start marker or at the end of
# the comment, and a repeated start marker of the section is dropped with what follows it.
replace_section() {
  awk -v s="<!-- ci:$2:start -->" -v e="<!-- ci:$2:end -->" -v f="$WORK_DIR/$2.md" '
    $0==s && mode    { next }
    $0==s && !seen++ { print; while ((getline line < f) > 0) print line; close(f); mode="keep"; next }
    $0==s            { mode="drop"; next }
    $0==e            { if (mode != "drop") print; mode=""; next }
    mode && /^<!-- ci:[^>]*:start -->$/ { if (mode == "keep") print e; mode="" }
    mode             { next }
                     { print }
    END              { if (mode == "keep") print e }
  ' <<< "$1"
}

# Trimmed content currently in section $2 of $1 (for the survive check).
extract_section() {
  awk -v s="<!-- ci:$2:start -->" -v e="<!-- ci:$2:end -->" '
    $0==s { grab=1; next }
    $0==e { grab=0; next }
    grab  { print }
  ' <<< "$1"
}

# Normalise the comment list to a flat array, whether `--paginate --slurp` hands back a flat
# array of comments or an array of per-page arrays.
flatten_pages() { jq -c '[.[] | if type=="array" then .[] else . end]' <<< "$1"; }

# IDs of every marker-bearing bot comment on the PR, oldest first. Flattened first so sort_by
# orders globally rather than only within a page.
marker_ids() {
  jq -r --arg m "$MARKER" --arg bot "$BOT" \
    '[.[] | select(.user.login==$bot and (.body|contains($m)))] | sort_by(.id) | .[].id' <<< "$(flatten_pages "$1")"
}

COMMENT_WRITE_ATTEMPTS=5
COMMENT_WRITE_MAX_DELAY_SECONDS=8

for ((attempt=1; attempt<=COMMENT_WRITE_ATTEMPTS; attempt++)); do
  COMMENTS=$(gh api "/repos/$REPO/issues/$PR_NUMBER/comments" --paginate --slurp)
  IDS=()
  while IFS= read -r line; do [ -n "$line" ] && IDS+=("$line"); done <<< "$(marker_ids "$COMMENTS")"
  COMMENT_ID="${IDS[0]:-}"

  # Collapse accidental duplicates from a create race: keep the oldest, drop the rest.
  if [ "${#IDS[@]}" -gt 1 ]; then
    for extra in "${IDS[@]:1}"; do
      echo "Deleting duplicate CI status comment $extra."
      gh api -X DELETE "/repos/$REPO/issues/comments/$extra" >/dev/null || true
    done
  fi

  if [ -n "$COMMENT_ID" ]; then
    CURRENT_BODY=$(jq -r --arg id "$COMMENT_ID" '.[] | select(.id==($id|tonumber)) | .body' <<< "$(flatten_pages "$COMMENTS")")
    CURRENT_BODY="${CURRENT_BODY//$'\r'/}"
  else
    CURRENT_BODY=""
  fi

  if [ -n "${IF_SECTION_HAS:-}" ] && ! grep -qF -- "$IF_SECTION_HAS" <<< "$(extract_section "$CURRENT_BODY" "$SECTION")"; then
    echo "Section '$SECTION' does not have '$IF_SECTION_HAS'; not written."
    exit 0
  fi

  # No comment yet: start from the full skeleton. A comment that lacks one of our fences predates
  # that section, or the section is not part of the skeleton (automation, inworld): append an
  # empty fence for just that section instead of resetting the whole comment and wiping the other
  # sections' state. Two independent checks, not if/elif: a fresh skeleton needs the fence
  # appended too when the section is a non-skeleton one, or replace_section would find no fence
  # and the survive check would burn all 5 attempts.
  if [ -z "$CURRENT_BODY" ]; then
    CURRENT_BODY="$(skeleton)"
  fi
  NEW_BODY="$CURRENT_BODY"
  for name in "${NAMES[@]}"; do
    # -x: whole-line, matching replace_section/extract_section's $0==s exactly. A substring hit on
    # a marker embedded in a body line (which the strip filter deliberately lets through) would
    # skip fence creation here while the awk matchers see nothing, leaving the section permanently
    # unwritable.
    if ! grep -qxF "<!-- ci:$name:start -->" <<< "$NEW_BODY"; then
      NEW_BODY="$NEW_BODY"$'\n\n'"$(wrap_section "$name" "$(section_default "$name")")"
    fi
    NEW_BODY="$(replace_section "$NEW_BODY" "$name")"
  done

  # Near GitHub's 65536-char comment cap the write starts 422ing; the per-section CAP cannot see
  # the other sections, so at least say why.
  if [ "${#NEW_BODY}" -gt 65000 ]; then
    echo "::warning::The CI status comment is ${#NEW_BODY} chars, at or over GitHub's 65536 cap; a section needs a tighter cap."
  fi

  # A failed write must land in the retry loop, not kill the script under set -e: that would fail
  # this section's job and strand the section stale.
  if [ -z "$COMMENT_ID" ]; then
    if ! RESULT=$(jq -n --arg b "$NEW_BODY" '{body:$b}' \
      | gh api -X POST "/repos/$REPO/issues/$PR_NUMBER/comments" --input -); then
      echo "Create failed (attempt $attempt); retrying."
      retry_after_attempt "$attempt" "$COMMENT_WRITE_ATTEMPTS" "$COMMENT_WRITE_MAX_DELAY_SECONDS"
      continue
    fi
    COMMENT_ID=$(jq -r '.id' <<< "$RESULT")
  else
    if ! jq -n --arg b "$NEW_BODY" '{body:$b}' \
      | gh api -X PATCH "/repos/$REPO/issues/comments/$COMMENT_ID" --input - >/dev/null; then
      echo "Update failed (attempt $attempt); retrying."
      retry_after_attempt "$attempt" "$COMMENT_WRITE_ATTEMPTS" "$COMMENT_WRITE_MAX_DELAY_SECONDS"
      continue
    fi
  fi

  # Re-read and confirm our sections landed on the surviving comment, and that no concurrent
  # writer left a duplicate behind. A stale read falls through to the retry below.
  RECHECK=$(gh api "/repos/$REPO/issues/$PR_NUMBER/comments" --paginate --slurp)
  RIDS=()
  while IFS= read -r line; do [ -n "$line" ] && RIDS+=("$line"); done <<< "$(marker_ids "$RECHECK")"
  LIVE_BODY=$(jq -r --arg id "$COMMENT_ID" '.[] | select(.id==($id|tonumber)) | .body' <<< "$(flatten_pages "$RECHECK")")
  LIVE_BODY="${LIVE_BODY//$'\r'/}"

  # A write that landed on a younger duplicate is doomed: GC keeps the oldest, so these sections'
  # content would vanish with the duplicate. Retry on the survivor instead of declaring success
  # on a comment about to be deleted.
  if [ -n "${RIDS[0]:-}" ] && [ "$COMMENT_ID" != "${RIDS[0]}" ]; then
    echo "Comment $COMMENT_ID lost the create race to ${RIDS[0]}; retrying on the survivor."
    retry_after_attempt "$attempt" "$COMMENT_WRITE_ATTEMPTS" "$COMMENT_WRITE_MAX_DELAY_SECONDS"
    continue
  fi

  # Success means every one of our sections landed on the comment we wrote, nothing more.
  # Duplicate collapsing is best-effort cleanup (the DELETE above may lack permission); a duplicate
  # we could not remove must not block reporting that already succeeded, or every run would burn
  # all 5 attempts and warn forever.
  settled=true
  for name in "${NAMES[@]}"; do
    if [ "$(extract_section "$LIVE_BODY" "$name")" != "$(cat "$WORK_DIR/$name.md")" ]; then
      settled=false
    fi
  done
  if [ "$settled" = true ]; then
    echo "CI status $LABEL updated (attempt $attempt)."
    if [ "${#RIDS[@]}" -gt 1 ]; then
      echo "A duplicate CI status comment remains (could not be deleted); it will be retried next run."
    fi
    exit 0
  fi

  echo "CI status $LABEL not settled (attempt $attempt); retrying."
  retry_after_attempt "$attempt" "$COMMENT_WRITE_ATTEMPTS" "$COMMENT_WRITE_MAX_DELAY_SECONDS"
done

echo "::warning::Could not confirm CI status $LABEL after $COMMENT_WRITE_ATTEMPTS attempts."
exit 0
