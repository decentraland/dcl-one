"""Check the inherited descriptor prerequisite without running Rust workloads."""
import os
from pathlib import Path
import shutil
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[2]
BASH = shutil.which("bash")


class DescriptorPrerequisiteTests(unittest.TestCase):
    def setUp(self):
        self.assertIsNotNone(BASH, "bash is required by the test harness")

    def run_provision(self, per_process="10240", system="12288", *, hosted="github-hosted",
                      apply=True, write_fails=False):
        script = r'''
mock_per_process="$2"; mock_system="$3"; apply="$4"; write_fails="$5"
uname() { printf 'Darwin\n'; }
sysctl() {
  case "$*" in
    '-n kern.maxfilesperproc') printf '%s\n' "$mock_per_process" ;;
    '-n kern.maxfiles') printf '%s\n' "$mock_system" ;;
    *) return 2 ;;
  esac
}
sudo() {
  [ "$1 $2" = 'sysctl -w' ] || return 2
  [ "$write_fails" = false ] || return 1
  printf 'WRITE %s\n' "$3"
  if [ "$apply" = true ]; then
    case "$3" in
      kern.maxfilesperproc=*) mock_per_process="${3#*=}" ;;
      kern.maxfiles=*) mock_system="${3#*=}" ;;
      *) return 2 ;;
    esac
  fi
}
source "$1"
provision_hosted_macos_test_resources
'''
        return subprocess.run(
            [BASH, "-c", script, "_", (ROOT / "tooling/catalyst-test-resources.sh").as_posix(),
             per_process, system, str(apply).lower(), str(write_fails).lower()],
            env={**os.environ, "GITHUB_ACTIONS": "true", "RUNNER_OS": "macOS",
                 "RUNNER_ENVIRONMENT": hosted},
            capture_output=True, text=True, check=False,
        )

    def test_hosted_provision_raises_system_before_per_process_and_reads_back(self):
        result = self.run_provision()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertLess(result.stdout.index("WRITE kern.maxfiles=1048576"),
                        result.stdout.index("WRITE kern.maxfilesperproc=65536"))
        self.assertIn("verified kern.maxfilesperproc=65536 kern.maxfiles=1048576", result.stdout)

    def test_hosted_provision_preserves_higher_limits(self):
        result = self.run_provision("131072", "2097152")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("WRITE", result.stdout)

    def test_hosted_provision_refuses_other_hosts_and_failed_or_unverified_writes(self):
        for options in [{"hosted": "self-hosted"}, {"hosted": ""}, {"apply": False},
                        {"write_fails": True}, {"per_process": "unknown"}]:
            with self.subTest(options=options):
                result = self.run_provision(**options)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("catalyst-test: hosted descriptor provisioning:", result.stderr)
                if "hosted" in options:
                    self.assertNotIn("WRITE", result.stdout)

    def run_prerequisite(self, soft, hard, *, raise_fails=False, readback=None,
                         platform="Linux", kernel="65536"):
        script = r'''
mock_soft="$2"; mock_hard="$3"; raise_fails="$4"; readback="$5"; platform="$6"; mock_kernel="$7"
uname() { printf '%s\n' "$platform"; }
sysctl() { [ "$*" = '-n kern.maxfilesperproc' ] || return 2; printf '%s\n' "$mock_kernel"; }
ulimit() {
  case "$*" in
    -Sn) printf '%s\n' "$mock_soft" ;;
    -Hn) printf '%s\n' "$mock_hard" ;;
    '-Sn 65536')
      [ "$raise_fails" = false ] || return 1
      mock_soft="${readback:-65536}" ;;
    *) return 2 ;;
  esac
}
source "$1"
prepare_catalyst_test_resources || exit 1
printf 'LAUNCHED soft=%s hard=%s\n' "$(ulimit -Sn)" "$(ulimit -Hn)"
'''
        return subprocess.run(
            [BASH, "-c", script, "_", (ROOT / "tooling/catalyst-test-resources.sh").as_posix(),
             soft, hard, str(raise_fails).lower(), readback or "", platform, kernel],
            capture_output=True, text=True, check=False,
        )

    def test_raises_only_soft_limit_and_reports_verified_limits(self):
        result = self.run_prerequisite("10240", "unlimited")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("before soft=10240 hard=unlimited required=65536", result.stdout)
        self.assertIn("after soft=65536 hard=unlimited required=65536", result.stdout)
        self.assertIn("LAUNCHED soft=65536 hard=unlimited", result.stdout)

    def test_preserves_sufficient_and_unlimited_soft_limits(self):
        for soft in ["65536", "1048576", "unlimited"]:
            with self.subTest(soft=soft):
                result = self.run_prerequisite(soft, "unlimited", raise_fails=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn(f"LAUNCHED soft={soft}", result.stdout)

    def test_macos_checks_kernel_ceiling_even_with_unlimited_rlimits(self):
        for kernel in ["10240", "unknown"]:
            with self.subTest(kernel=kernel):
                result = self.run_prerequisite("unlimited", "unlimited",
                                               platform="Darwin", kernel=kernel)
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn("LAUNCHED", result.stdout)
                self.assertIn("kernel per-process limit", result.stderr)
        result = self.run_prerequisite("10240", "unlimited", platform="Darwin")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("kern.maxfilesperproc=65536", result.stdout)

    def test_inadequate_hard_limit_failed_raise_and_bad_readback_prevent_launch(self):
        for options in [
            {"hard": "10240"},
            {"hard": "unlimited", "raise_fails": True},
            {"hard": "unlimited", "readback": "10240"},
            {"hard": "unknown"},
            {"hard": "unlimited", "readback": "unknown"},
        ]:
            with self.subTest(options=options):
                result = self.run_prerequisite("10240", **options)
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn("LAUNCHED", result.stdout)
                self.assertIn("catalyst-test: descriptor prerequisite", result.stderr)

    def test_real_child_inherits_or_inadequate_host_fails_before_launch(self):
        script = r'''
set -euo pipefail
source "$1"
platform=$(uname -s)
hard=$(ulimit -Hn)
case "$hard" in unlimited) ;; ''|*[!0-9]*) exit 90 ;; esac
expected=''
if [ "$hard" != unlimited ] && [ "$hard" -lt 65536 ]; then
  expected="hard limit $hard is below 65536"
fi
if [ "$platform" = Darwin ]; then
  kernel=$(sysctl -n kern.maxfilesperproc)
  case "$kernel" in ''|*[!0-9]*) exit 91 ;; esac
  if [ "$kernel" -lt 65536 ]; then
    expected="kernel per-process limit $kernel is below 65536"
  fi
fi
if [ -n "$expected" ]; then
  if output=$(prepare_catalyst_test_resources 2>&1); then
    echo 'An inadequate host unexpectedly passed the workload prerequisite' >&2
    exit 92
  fi
  case "$output" in *"$expected"*) ;; *) printf '%s\n' "$output" >&2; exit 93 ;; esac
  printf 'RESOURCE_SELFTEST insufficient_host_failed_before_child: %s\n' "$expected"
else
  prepare_catalyst_test_resources
  bash -c 'set -euo pipefail; value=$(ulimit -Sn); [ "$value" = unlimited ] || [ "$value" -ge 65536 ]; printf "RESOURCE_SELFTEST child_inherited soft=%s\n" "$value"'
fi
'''
        result = subprocess.run(
            [BASH, "-c", script, "_", (ROOT / "tooling/catalyst-test-resources.sh").as_posix()],
            capture_output=True, text=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(result.stdout.count("RESOURCE_SELFTEST "), 1, result.stdout)
        print(result.stdout.strip())


if __name__ == "__main__":
    unittest.main()
