"""Pull requests run the Rust tests on macOS for build, toolchain and dev-stack inputs; source stays on Linux."""
import contextlib
import importlib.util
import io
import os
from pathlib import Path
import re
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('dev_ci', Path(__file__).parents[1] / 'dev-ci.py')
dev = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(dev)


class DevCiTest(unittest.TestCase):
    def setUp(self):
        # The smoke job runs these tests; they must not write its outputs or summary.
        environment = patch.dict(os.environ)
        environment.start()
        self.addCleanup(environment.stop)
        for key in ('GITHUB_EVENT_NAME', 'GITHUB_OUTPUT', 'GITHUB_STEP_SUMMARY'):
            os.environ.pop(key, None)
        self.ci = dev.load_ci_local()

    def macos(self, *changed):
        return dev.needs_macos(changed, self.ci)[0]

    def test_linux_covered_inputs_skip_macos(self):
        for changed in (['apps/ui3/src/play/Hud.tsx'], ['apps/sites/src/app.ts', 'apps/ui3/package.json'],
                        ['tooling/tests/test_dev.py'], ['tooling/bevy-ci.py'],
                        ['rendering/bevy-explorer/upstream.lock.json'],
                        ['docs/development.md', 'rendering/bevy-explorer/source/react-web/src/App.tsx',
                         'apps/ui3/src/a.ts'],
                        ['services/catalyst/db/src/lib.rs'], ['library/protocol/proto/a.proto'],
                        ['schemas/50_create.sql']):
            with self.subTest(changed=changed):
                self.assertFalse(self.macos(*changed))

    def test_build_and_dev_stack_inputs_run_macos(self):
        for path in ('services/catalyst/Cargo.lock', 'services/catalyst/social/Cargo.toml',
                     'services/catalyst/comms/build.rs', 'services/catalyst/.cargo/config.toml',
                     'library/vendor/rusty_enet/src/lib.rs', '.cargo/config.toml', 'flake.lock', 'Makefile',
                     'infra/dev/shell-hook.sh', 'tooling/dev.py',
                     'tooling/dev-test.sh', 'tooling/dev-ci.py', 'tooling/enet-provenance.py',
                     'tooling/tests/test_dev_regressions.py', 'tooling/tests/test_enet_provenance.py',
                     '.github/workflows/development.yml'):
            with self.subTest(path=path):
                self.assertTrue(self.macos('apps/ui3/src/a.ts', path))

    def test_every_script_the_rust_suite_runs_is_a_macos_input(self):
        script = (dev.ROOT / 'tooling/dev-test.sh').read_text()
        rust = script.split('    rust)', 1)[1].split(';;', 1)[0]
        paths = (['tooling/tests/' + name for name in re.findall(r"-p '([^']+)'", rust)]
                 + re.findall(r'python3 ((?:tooling|library)/\S+)', rust))
        self.assertGreaterEqual(len(paths), 4)
        for path in paths:
            with self.subTest(path=path):
                self.assertTrue(self.macos(path))

    def test_a_missing_plan_runs_macos(self):
        # Push and dispatch skip the plan, and a failed plan leaves no output: both must keep macOS.
        workflow = (dev.ROOT / '.github/workflows/development.yml').read_text()
        self.assertIn("needs.changes.outputs.macos != 'false'", workflow)

    def test_unclassified_or_unfiltered_triggers_run_macos(self):
        with patch.object(self.ci, 'workflow_paths', return_value=['apps/ui3/**', 'services/new/**']):
            self.assertTrue(self.macos('services/new/src/lib.rs'))
            self.assertFalse(self.macos('apps/ui3/src/a.ts'))
        with patch.object(self.ci, 'workflow_paths', return_value=None):
            self.assertTrue(self.macos('docs/development.md'))

    def test_other_events_and_failures_run_macos(self):
        self.assertTrue(dev.plan('push', 'HEAD^1', 'HEAD')[0])
        self.assertFalse(dev.plan('local', 'HEAD', 'HEAD')[0])
        self.assertTrue(dev.plan('pull_request', 'not-a-revision', 'HEAD')[0])
        self.assertTrue(dev.plan('local', 'not-a-revision', 'HEAD')[0])
        with patch.object(dev, 'load_ci_local', side_effect=FileNotFoundError('ci-local.py')):
            self.assertTrue(dev.plan('pull_request', 'HEAD', 'HEAD')[0])

    def run_main(self, summary):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory, 'output')
            os.environ.update(GITHUB_EVENT_NAME='pull_request', GITHUB_OUTPUT=str(output),
                              GITHUB_STEP_SUMMARY=str(summary or Path(directory, 'summary')))
            with patch.object(dev, 'plan', return_value=(False, 'Only Linux-covered inputs changed')), \
                    patch('sys.argv', ['dev-ci.py', 'plan']), contextlib.redirect_stdout(io.StringIO()):
                try:
                    dev.main()
                finally:
                    written = output.read_text() if output.exists() else None
            return written

    def test_writes_the_output_last(self):
        self.assertEqual(self.run_main(None), 'macos=false\n')
        # A failed step must leave no answer, so the Rust job falls back to macOS.
        with tempfile.TemporaryDirectory() as directory, self.assertRaises(OSError):
            self.run_main(Path(directory))


if __name__ == '__main__':
    unittest.main()
