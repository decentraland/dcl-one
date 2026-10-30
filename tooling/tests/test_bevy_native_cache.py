"""Unused runner compilers must not change the native Rust cache identity."""
import importlib.util
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    'bevy_native_cache', Path(__file__).resolve().parents[2] / 'apps/bevy-explorer/prepare-native-cache.py')
cache = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(cache)


class NativeCacheTest(unittest.TestCase):
    def setUp(self):
        self.environment = patch.dict(cache.os.environ, {'RUNNER_ENVIRONMENT': 'github-hosted'})
        self.environment.start()
        self.addCleanup(self.environment.stop)

    def test_retains_active_compiler_with_targets_and_components_on_every_native_platform(self):
        for host in ('x86_64-unknown-linux-gnu', 'x86_64-pc-windows-msvc',
                     'aarch64-apple-darwin', 'x86_64-apple-darwin'):
            with self.subTest(host=host):
                active = f'1.99.1-{host}'
                stable, nightly = f'stable-{host}', f'nightly-{host}'
                compiler = f'rustc 1.99.1\nrelease: 1.99.1\nhost: {host}'
                installed = {active: {'rustc', 'clippy', 'rustfmt', 'x86_64-apple-darwin'},
                             stable: {'rustc'}, nightly: {'rustc'}}
                expected = installed[active].copy()

                def run(args, **kwargs):
                    self.assertTrue(kwargs['check'])
                    self.assertEqual(args[:3], ['rustup', 'toolchain', 'uninstall'])
                    del installed[args[3]]

                with patch.object(cache, 'output', side_effect=[
                        active + ' (default)', compiler, '\n'.join(installed)]), \
                        patch.object(cache.subprocess, 'run', side_effect=run):
                    cache.prepare()
                self.assertEqual(installed, {active: expected})

    def test_already_prepared_runner_does_not_mutate_toolchains(self):
        active = '1.99.1-x86_64-unknown-linux-gnu'
        with patch.object(cache, 'output', side_effect=[active, 'release: 1.99.1\nhost: x86_64-unknown-linux-gnu', active]), \
                patch.object(cache.subprocess, 'run') as run:
            cache.prepare()
        run.assert_not_called()

    def test_developer_and_self_hosted_environments_are_never_modified(self):
        for environment in ('', 'self-hosted'):
            with self.subTest(environment=environment), \
                    patch.dict(cache.os.environ, {'RUNNER_ENVIRONMENT': environment}), \
                    patch.object(cache, 'output') as output, patch.object(cache.subprocess, 'run') as run:
                with self.assertRaisesRegex(RuntimeError, 'disposable GitHub-hosted'):
                    cache.prepare()
                output.assert_not_called()
                run.assert_not_called()

    def test_rolling_or_mismatched_compiler_is_rejected_before_removing_anything(self):
        for active, release in [('stable-x86_64-unknown-linux-gnu', '1.99.1'),
                                ('1.99.1-x86_64-unknown-linux-gnu', '1.99.0'),
                                ('nightly-x86_64-unknown-linux-gnu', '1.100.0-nightly')]:
            with self.subTest(active=active, release=release), \
                    patch.object(cache, 'output', side_effect=[active, f'release: {release}\nhost: x86_64-unknown-linux-gnu']), \
                    patch.object(cache.subprocess, 'run') as run:
                with self.assertRaisesRegex(RuntimeError, 'numbered native Rust toolchain'):
                    cache.prepare()
                run.assert_not_called()

    def test_active_compiler_must_be_in_the_inventory_before_removing_anything(self):
        with patch.object(cache, 'output', side_effect=[
                '1.99.1-x86_64-unknown-linux-gnu', 'release: 1.99.1\nhost: x86_64-unknown-linux-gnu',
                'stable-x86_64-unknown-linux-gnu']), patch.object(cache.subprocess, 'run') as run:
            with self.assertRaisesRegex(RuntimeError, 'absent'):
                cache.prepare()
            run.assert_not_called()

    def test_failed_cleanup_stops_before_cache_restoration(self):
        active = '1.99.1-x86_64-unknown-linux-gnu'
        with patch.object(cache, 'output', side_effect=[
                active, 'release: 1.99.1\nhost: x86_64-unknown-linux-gnu', active + '\nstable-x86_64-unknown-linux-gnu']), \
                patch.object(cache.subprocess, 'run', side_effect=subprocess.CalledProcessError(1, 'rustup')):
            with self.assertRaises(subprocess.CalledProcessError):
                cache.prepare()


if __name__ == '__main__':
    unittest.main()
