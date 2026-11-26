"""Headless packaging keeps its compiler flags and complete per-target payload."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('headless_build',
    Path(__file__).resolve().parents[2] / 'apps/bevy-explorer/build-headless.py')
build = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build)


class HeadlessBuildTests(unittest.TestCase):
    def test_native_and_cross_target_payloads_keep_engine_sidecar_and_license(self):
        for target in (None, 'x86_64-unknown-linux-gnu', 'x86_64-pc-windows-msvc'):
            with self.subTest(target=target), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                source, app = root / 'source', root / 'app'
                release = source / 'target' / (target or '') / 'release'
                release.mkdir(parents=True)
                suffix = '.exe' if target and 'windows' in target else ''
                for name in ('headless', 'dcl_deno_ipc'):
                    (release / (name + suffix)).write_bytes(name.encode())
                (source / 'LICENSE').write_text('license')
                with patch.object(build, 'SOURCE', source), patch.object(build, 'APP', app), \
                        patch.object(build.subprocess, 'run') as cargo, patch.object(build.os, 'name', 'posix'), \
                        patch.dict(build.os.environ, clear=False) as environ:
                    environ.pop('CARGO_MESSAGES', None)
                    build.headless(target)
                commands = [list(call.args[0]) for call in cargo.call_args_list]
                flags = ['--target', target] if target else []
                self.assertEqual(commands, [
                    ['cargo', 'build', '--release', '--locked', *flags, '-p', 'dcl_deno_ipc'],
                    ['cargo', 'build', '--release', '--locked', *flags, '--bin', 'headless',
                     '--no-default-features', '--features', 'headless,livekit'],
                ])
                self.assertTrue(all(call.kwargs == {'cwd': source, 'check': True}
                                    for call in cargo.call_args_list))
                output = app / 'dist/headless' / (target or 'native')
                self.assertEqual({p.name for p in output.iterdir()},
                                 {'headless' + suffix, 'dcl_deno_ipc' + suffix, 'LICENSE'})
                for name in ('headless', 'dcl_deno_ipc'):
                    self.assertEqual((output / (name + suffix)).read_bytes(), name.encode())
                self.assertEqual((output / 'LICENSE').read_text(), 'license')

    def test_cargo_messages_append_both_builds_json_output_to_the_named_file(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, app = root / 'source', root / 'app'
            release = source / 'target/release'
            release.mkdir(parents=True)
            for name in ('headless', 'dcl_deno_ipc'):
                (release / name).write_bytes(name.encode())
            (source / 'LICENSE').write_text('license')
            messages = source / 'target/cargo-messages/headless.jsonl'

            def cargo(args, **kwargs):
                kwargs['stdout'].write(f'{{"reason":"build-finished","args":{len(args)}}}\n')

            with patch.object(build, 'SOURCE', source), patch.object(build, 'APP', app), \
                    patch.object(build.subprocess, 'run', side_effect=cargo) as run, \
                    patch.object(build.os, 'name', 'posix'), patch.dict(build.os.environ, {'CARGO_MESSAGES': str(messages)}):
                build.headless()
            self.assertEqual([list(call.args[0]) for call in run.call_args_list], [
                ['cargo', 'build', '--release', '--locked', '-p', 'dcl_deno_ipc', '--message-format=json-render-diagnostics'],
                ['cargo', 'build', '--release', '--locked', '--bin', 'headless', '--no-default-features',
                 '--features', 'headless,livekit', '--message-format=json-render-diagnostics'],
            ])
            self.assertEqual(len(messages.read_text().splitlines()), 2)


if __name__ == '__main__':
    unittest.main()
