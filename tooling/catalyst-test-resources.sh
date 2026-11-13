#!/usr/bin/env bash
# Source before launching test binaries, including inventory/provisioned checks.
# The 3,000-verifier loopback workload owns both socket ends and retains idle and
# background connections across links; verifier concurrency is not a descriptor cap.
# Only the explicit workflow provisioning step calls this function. Developer
# machines retain the read-only kernel checks in prepare_catalyst_test_resources.
provision_hosted_macos_test_resources() {
  local per_process system value required=65536 system_required=1048576
  if [ "${GITHUB_ACTIONS:-}:${RUNNER_ENVIRONMENT:-}:${RUNNER_OS:-}:$(uname -s)" != true:github-hosted:macOS:Darwin ]; then
    echo 'catalyst-test: hosted descriptor provisioning: requires a GitHub-hosted macOS runner' >&2
    return 1
  fi
  per_process=$(sysctl -n kern.maxfilesperproc) && system=$(sysctl -n kern.maxfiles) || {
    echo 'catalyst-test: hosted descriptor provisioning: cannot read kernel limits' >&2
    return 1
  }
  for value in "$per_process" "$system"; do
    case "$value" in
      ''|*[!0-9]*) echo 'catalyst-test: hosted descriptor provisioning: invalid kernel limits' >&2; return 1 ;;
    esac
  done
  echo "catalyst-test: hosted descriptors before kern.maxfilesperproc=$per_process kern.maxfiles=$system"
  # Sixteen concurrent binaries can each inherit the 65536-descriptor budget.
  # Preserve a higher per-process ceiling and keep the system ceiling above it.
  [ "$per_process" -le "$system_required" ] || system_required="$per_process"
  if [ "$system" -lt "$system_required" ]; then
    sudo sysctl -w "kern.maxfiles=$system_required" || {
      echo 'catalyst-test: hosted descriptor provisioning: cannot raise system limit' >&2
      return 1
    }
  fi
  if [ "$per_process" -lt "$required" ]; then
    sudo sysctl -w "kern.maxfilesperproc=$required" || {
      echo 'catalyst-test: hosted descriptor provisioning: cannot raise per-process limit' >&2
      return 1
    }
  fi
  per_process=$(sysctl -n kern.maxfilesperproc) && system=$(sysctl -n kern.maxfiles) || {
    echo 'catalyst-test: hosted descriptor provisioning: cannot read back kernel limits' >&2
    return 1
  }
  if ! [ "$per_process" -ge "$required" ] 2>/dev/null || ! [ "$system" -ge "$system_required" ] 2>/dev/null; then
    echo "catalyst-test: hosted descriptor provisioning: inadequate readback per_process=$per_process system=$system" >&2
    return 1
  fi
  echo "catalyst-test: hosted descriptors verified kern.maxfilesperproc=$per_process kern.maxfiles=$system"
}

prepare_catalyst_test_resources() {
  local required=65536 soft hard limit kernel
  soft=$(ulimit -Sn) && hard=$(ulimit -Hn) || {
    echo 'catalyst-test: descriptor prerequisite: cannot read inherited limits' >&2
    return 1
  }
  echo "catalyst-test: descriptors before soft=$soft hard=$hard required=$required"
  for limit in "$soft" "$hard"; do
    case "$limit" in
      unlimited) ;;
      ''|*[!0-9]*) echo 'catalyst-test: descriptor prerequisite: invalid limits' >&2; return 1 ;;
    esac
  done
  if [ "$(uname -s)" = Darwin ]; then
    kernel=$(sysctl -n kern.maxfilesperproc) || {
      echo 'catalyst-test: descriptor prerequisite: cannot read kern.maxfilesperproc' >&2
      return 1
    }
    echo "catalyst-test: descriptors kern.maxfilesperproc=$kernel required=$required"
    if ! [ "$kernel" -ge "$required" ] 2>/dev/null; then
      echo "catalyst-test: descriptor prerequisite: kernel per-process limit $kernel is below $required" >&2
      echo 'catalyst-test: the full backend suite includes 3,000 concurrent RPC verifiers; use hosted CI or have an administrator provision and verify adequate kernel and inherited descriptor limits' >&2
      echo 'catalyst-test: this check does not change developer kernel limits; individual Cargo targets can be run separately' >&2
      return 1
    fi
  fi
  if [ "$soft" != unlimited ] && ! [ "$soft" -ge "$required" ] 2>/dev/null; then
    if [ "$hard" != unlimited ] && ! [ "$hard" -ge "$required" ] 2>/dev/null; then
      echo "catalyst-test: descriptor prerequisite: hard limit $hard is below $required" >&2
      return 1
    fi
    # Change only this shell's soft limit; children inherit it. No host sysctl changes.
    if ! ulimit -Sn "$required"; then
      echo "catalyst-test: descriptor prerequisite: cannot raise soft limit to $required; check hard and kernel limits" >&2
      return 1
    fi
  fi
  soft=$(ulimit -Sn) && hard=$(ulimit -Hn) || {
    echo 'catalyst-test: descriptor prerequisite: cannot read back limits' >&2
    return 1
  }
  echo "catalyst-test: descriptors after soft=$soft hard=$hard required=$required"
  if [ "$soft" != unlimited ] && ! [ "$soft" -ge "$required" ] 2>/dev/null; then
    echo "catalyst-test: descriptor prerequisite: verified soft limit $soft is below $required" >&2
    return 1
  fi
}
