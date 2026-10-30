"""Cache identity and disk reservation must not weaken image publication."""
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location('image_cache', Path(__file__).parents[1] / 'image-cache.py')
cache = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(cache)


class CachePlanTest(unittest.TestCase):
    def setUp(self):
        self.derivations = {
            package: f'/nix/store/abc123-{package}.drv'
            for packages in cache.PACKAGES.values() for package in packages
        }

    def plan(self, event='pull_request', ref='refs/pull/93/merge', arch='arm64'):
        return cache.cache_plan(arch, event, ref, self.derivations)

    def test_pull_requests_can_save_but_never_share_the_main_write_key(self):
        pr = self.plan()
        main = self.plan('push', 'refs/heads/main')
        self.assertEqual(pr['save'], 'true')
        self.assertEqual(main['save'], 'true')
        for group in cache.PACKAGES:
            self.assertNotEqual(pr[f'{group}-key'], main[f'{group}-key'])
            self.assertEqual(pr[f'{group}-main-key'], main[f'{group}-key'])
            self.assertNotEqual(pr[f'{group}-key'], self.plan(ref='refs/pull/94/merge')[f'{group}-key'])

    def test_tags_and_other_branches_only_read_main(self):
        for event, ref in [('push', 'refs/tags/v1'), ('workflow_dispatch', 'refs/heads/feature'),
                           ('pull_request', 'refs/heads/feature')]:
            with self.subTest(event=event, ref=ref):
                plan = self.plan(event, ref)
                self.assertEqual(plan['save'], 'false')
                self.assertEqual(plan['catalyst-key'], plan['catalyst-main-key'])

    def test_dependency_groups_change_independently_from_application_source(self):
        before = self.plan()
        self.derivations['catalyst'] = '/nix/store/changed-catalyst.drv'
        application = self.plan()
        self.assertNotEqual(before['apis-key'], application['apis-key'])
        self.assertEqual(before['catalyst-key'], application['catalyst-key'])
        self.assertEqual(before['sites-key'], application['sites-key'])
        self.derivations['catalyst-deps'] = '/nix/store/changed-catalyst-deps.drv'
        dependencies = self.plan()
        self.assertNotEqual(application['catalyst-key'], dependencies['catalyst-key'])
        self.assertEqual(application['sites-key'], dependencies['sites-key'])
        self.derivations['sites-toolchain'] = '/nix/store/changed-sites-toolchain.drv'
        self.assertNotEqual(dependencies['sites-key'], self.plan()['sites-key'])
        self.assertNotEqual(self.plan()['sites-key'], self.plan(arch='amd64')['sites-key'])

    def test_rejects_malformed_derivation_identity(self):
        self.derivations['catalyst'] = '/nix/store/abc-catalyst.drv\ninjected-key=anything'
        with self.assertRaises(ValueError):
            self.plan()

    def test_command_evaluates_once_and_emits_scoped_outputs(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'outputs'
            env = {'GITHUB_EVENT_NAME': 'pull_request', 'GITHUB_REF': 'refs/pull/93/merge',
                   'GITHUB_OUTPUT': str(output)}
            with patch.dict(cache.os.environ, env, clear=True), \
                    patch('sys.argv', ['image-cache.py', 'keys', '--arch', 'arm64']), \
                    patch.object(cache.subprocess, 'check_output', return_value=json.dumps(self.derivations)) as run, \
                    patch('sys.stdout', new_callable=io.StringIO):
                cache.main()
            run.assert_called_once()
            self.assertEqual(run.call_args.args[0][:4],
                             ['nix', 'eval', '--json', '.#packages.aarch64-linux'])
            values = dict(line.split('=', 1) for line in output.read_text().splitlines())
            self.assertEqual(values, self.plan())


class WorkflowDiskTest(unittest.TestCase):
    def test_full_sdk_cleanup_precedes_lfs_checkout_without_repository_helpers(self):
        workflow = (Path(__file__).parents[2] / '.github/workflows/apis-sites-images.yml').read_text()
        build = workflow.split('  build:\n', 1)[1].split('  production-ready:\n', 1)[0]
        cleanup = build.index('      - name: Reserve disk space for native builds')
        checkout = build.index('      - uses: actions/checkout@')
        self.assertLess(cleanup, checkout)
        step = build[cleanup:checkout]
        self.assertIn('sudo rm -rf /usr/local/lib/android /usr/share/dotnet /usr/share/swift '
                      '/opt/ghc /usr/local/.ghcup', step)
        self.assertEqual(step.count('df -h /'), 2)
        self.assertIn('${SECONDS}', step)
        self.assertNotIn('tooling/', step)
        self.assertNotIn('if ', step)


if __name__ == '__main__':
    unittest.main()
