#!/usr/bin/env bash
# Read-only sampling. Does not start workers, mutate data or approve activation.
set -euo pipefail
sync_binary="${MIGRATION_SYNC_BIN:-dcl-migration-sync}"
sync_stream="${1:?usage: soak.sh STREAM [SAMPLES] [INTERVAL_SECONDS] [places|events|social-settings]}"
sync_samples="${2:-1440}"
sync_interval="${3:-60}"
sync_source="${4:-places}"
case "$sync_source" in
  places) sync_reconcile=(reconcile) ;;
  events|social-settings) sync_reconcile=(reconcile-domain --source "$sync_source") ;;
  *) echo 'Unsupported source; expected places, events or social-settings' >&2; exit 2 ;;
esac
(( $# <= 4 )) || exit 2
[[ "$sync_stream" =~ ^[A-Za-z0-9_-]{1,64}$ ]] || exit 2
# Bound text before arithmetic: Bash integers can wrap huge inputs into a
# successful zero-iteration run. At most one million samples, one day apart.
[[ "$sync_samples" =~ ^[1-9][0-9]{0,6}$ && "$sync_interval" =~ ^[1-9][0-9]{0,4}$ ]] || exit 2
(( sync_samples <= 1000000 && sync_interval <= 86400 )) || exit 2
for ((sync_sample=1; sync_sample<=sync_samples; sync_sample++)); do
  date -u '+%Y-%m-%dT%H:%M:%SZ'
  "$sync_binary" status --stream "$sync_stream"
  "$sync_binary" metrics --stream "$sync_stream"
  "$sync_binary" check --stream "$sync_stream"
  # A moving boundary makes reconciliation inconclusive and stops this sample.
  # Run during a stable rehearsal window; do not suppress errors as matches.
  "$sync_binary" "${sync_reconcile[@]}" --stream "$sync_stream"
  if ((sync_sample < sync_samples)); then sleep "$sync_interval"; fi
done
