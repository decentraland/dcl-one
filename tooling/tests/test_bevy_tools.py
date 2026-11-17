import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class BevyToolPackageTests(unittest.TestCase):
    def run_installer(self, package, clang_version):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / 'output'
            binary = output / 'bin'
            binary.mkdir(parents=True)
            tools = {'protoc': 'libprotoc 36.2'}
            if package == 'bevy-tools':
                tools.update({'taplo': 'taplo 0.10.0', 'wasm-pack': 'wasm-pack 0.15.1'})
            if package == 'bevy-web-tools':
                tools['pkg-config'] = '1.2.14'
            if clang_version:
                tools['clang'] = 'clang version ' + clang_version
            for name, version in tools.items():
                path = binary / name
                path.write_text('#!/bin/sh\nprintf "%s\\n" "' + version + '"\n')
                path.chmod(0o755)
            fake_bin = root / 'fake-bin'
            fake_bin.mkdir()
            nix = fake_bin / 'nix'
            nix.write_text('#!/bin/sh\nprintf "%s\\n" "$BEVY_TEST_OUT"\n')
            nix.chmod(0o755)
            staging = root / 'staging'
            staging.mkdir()
            (staging / 'out').write_text(str(output) + '\n')
            exported = root / 'github-path'
            result = subprocess.run(['bash', str(ROOT / 'apps/bevy-explorer/install-tools.sh'),
                                     str(staging), package], capture_output=True, text=True,
                                    env={**os.environ, 'GITHUB_WORKSPACE': str(ROOT),
                                         'GITHUB_PATH': str(exported), 'GITHUB_ENV': str(root / 'github-env'),
                                         'BEVY_TEST_OUT': str(output),
                                         'PATH': str(fake_bin) + os.pathsep + os.environ['PATH']})
            path_exported = exported.read_text() if exported.exists() else ''
            return result, path_exported, str(binary)

    def test_web_tools_require_the_pinned_clang_major(self):
        result, exported, binary = self.run_installer('bevy-web-tools', '21.1.8')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(exported, binary + '\n')

    def test_web_tools_reject_an_old_compiler_before_exporting_path(self):
        result, exported, _ = self.run_installer('bevy-web-tools', '20.1.8')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('must provide clang 21', result.stderr)
        self.assertEqual(exported, '')

    def test_existing_shared_tools_do_not_require_clang(self):
        result, exported, binary = self.run_installer('bevy-tools', None)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(exported, binary + '\n')


if __name__ == '__main__':
    unittest.main()
