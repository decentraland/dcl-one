"""The UUAV workflow builds from the cached Nix store and the pins stay hash-locked."""
import calendar
import json
from pathlib import Path
import re
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / '.github/workflows/uuav.yml'
UUAV = ROOT / 'library/uuav'


def jobs():
    source = WORKFLOW.read_text().split('\njobs:\n', 1)[1]
    return dict(re.findall(r'(?ms)^  (\w+):\n(.*?)(?=^  \w+:|\Z)', source))


def steps(job):
    return re.split(r'(?m)^      - ', job)[1:]


class UuavWorkflowTest(unittest.TestCase):
    def test_the_nix_job_restores_the_store_snapshot_and_saves_it_from_main_only(self):
        nix = jobs()['nix']
        self.assertRegex(nix, r'os: \[ubuntu-26\.04, macos-26\]')
        install, snapshot = [i for i, s in enumerate(steps(nix)) if s.startswith(('uses: cachix/install-nix-action@', 'uses: nix-community/cache-nix-action@'))]
        self.assertEqual(snapshot, install + 1)
        snapshot = steps(nix)[snapshot]
        key = 'nix-store-${{ runner.os }}-${{ runner.arch }}-uuav-'
        self.assertIn('primary-key: ' + key + "${{ hashFiles('flake.nix', 'flake.lock', 'infra/packages/uuav.nix', 'library/uuav/Cargo.lock', 'library/uuav/rust-toolchain.toml', 'tooling/uuav/ffmpeg-windows.json') }}", snapshot)
        self.assertIn('restore-prefixes-first-match: ' + key + '\n', snapshot)
        self.assertIn("save: ${{ github.ref == 'refs/heads/main' && github.run_attempt == '1' }}", snapshot)
        self.assertIn('gc-max-store-size: 0', snapshot)
        for installable in ('.#uuav-dist', '.#uuav-smoke', '.#uuav-checks'):
            self.assertIn(f'nix build {installable} -o .cache/nix-roots/', nix)
        roots = 'nix build .#uuav-ffmpeg.inputDerivation .#uuav-helper.inputDerivation '
        self.assertIn(roots + "${{ runner.os == 'Linux' && '.#uuav-helper-windows.inputDerivation' || '' }} -o .cache/nix-roots/inputs", nix)
        self.assertLess(nix.index('nix build .#uuav-dist-windows '), nix.index(roots))
        self.assertLess(nix.index(roots), nix.index('UUAV_FORCE_SW_DECODE'))
        self.assertNotIn('nix develop', nix)

    def test_every_action_is_allowed_by_the_repository_policy(self):
        policy = (ROOT / 'tooling/repo-actions-policy.sh').read_text()
        allowed = re.findall(r"patterns_allowed\[\]=([^']+)'", policy)
        patterns = [re.escape(p).replace(r'\*', '.*') for p in allowed]
        for action in re.findall(r'uses: (\S+)', WORKFLOW.read_text()):
            self.assertTrue(action.startswith('actions/') or any(re.fullmatch(p, action) for p in patterns), action)
            self.assertRegex(action, r'@[0-9a-f]{40}$')

    def test_the_workflow_runs_on_every_input_of_the_runtime_set(self):
        source = WORKFLOW.read_text()
        for path in ('library/uuav/**', 'tooling/uuav/**', 'infra/packages/uuav.nix', 'flake.lock', '.github/workflows/uuav.yml'):
            self.assertEqual(source.count(f'      - {path}\n'), 2, path)

    def test_the_windows_job_checks_the_crates_and_smokes_the_cross_set(self):
        windows = jobs()['windows']
        self.assertIn('needs: nix', windows)
        self.assertIn('working-directory: library/uuav', windows)
        self.assertIn('fetch-ffmpeg-windows.sh', windows)
        self.assertIn('workspaces: library/uuav -> .target', windows)
        self.assertIn("save-if: ${{ github.ref == 'refs/heads/main' && github.run_attempt == '1' }}", windows)
        self.assertIn('bindgen-clang-args.sh', windows)
        self.assertIn('cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings', windows)
        self.assertIn('name: uuav-windows-x86_64\n', windows)
        self.assertIn('--example smoke --target x86_64-pc-windows-gnu', windows)
        self.assertIn('UUAV_FORCE_SW_DECODE=1 timeout 300 ./smoke.exe', windows)
        self.assertNotIn('build.sh', windows)
        self.assertEqual(jobs()['nix'].count('name: uuav-windows-x86_64\n'), 1)

    def test_the_windows_ffmpeg_pin_is_a_month_end_release_with_a_hash(self):
        pin = json.loads((ROOT / 'tooling/uuav/ffmpeg-windows.json').read_text())
        year, month, day = map(int, re.fullmatch(r'autobuild-(\d{4})-(\d{2})-(\d{2})-\d{2}-\d{2}', pin['release_tag']).groups())
        self.assertEqual(day, calendar.monthrange(year, month)[1])
        self.assertRegex(pin['asset'], r'^ffmpeg-n8\.1(\.\d+)?(-\d+-g[0-9a-f]+)?-win64-lgpl-shared-8\.1\.zip$')
        self.assertEqual(pin['url'], f"{pin['repo']}/releases/download/{pin['release_tag']}/{pin['asset']}")
        self.assertRegex(pin['sha256'], r'^[0-9a-f]{64}$')

    def test_nothing_in_the_workspace_depends_on_uuav_client(self):
        workspace = tomllib.loads((UUAV / 'Cargo.toml').read_text())
        for member in [UUAV] + [UUAV / m for m in workspace['workspace']['members']]:
            manifest = tomllib.loads((member / 'Cargo.toml').read_text())
            if manifest['package']['name'] == 'uuav-client':
                continue
            dependencies = set(manifest.get('dependencies', {}))
            for target in manifest.get('target', {}).values():
                dependencies |= set(target.get('dependencies', {}))
            self.assertNotIn('uuav-client', dependencies, manifest['package']['name'])

    def test_the_toolchain_pin_names_only_the_windows_target(self):
        toolchain = tomllib.loads((UUAV / 'rust-toolchain.toml').read_text())['toolchain']
        self.assertRegex(toolchain['channel'], r'^\d+\.\d+\.\d+$')
        self.assertEqual(toolchain['targets'], ['x86_64-pc-windows-gnu'])
        self.assertEqual(sorted(toolchain['components']), ['clippy', 'rustfmt'])


if __name__ == '__main__':
    unittest.main()
