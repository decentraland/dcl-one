"""The local hooks select the same workflows GitHub would for a change."""
import importlib.util
from pathlib import Path
import re
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('ci_local', Path(__file__).parents[1] / 'ci-local.py')
ci = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ci)


class CiLocalTest(unittest.TestCase):
    def setUp(self):
        # Standalone runs must inspect this checkout, not a stale hook worktree.
        tree = patch.object(ci, 'TREE', ci.ROOT)
        tree.start()
        self.addCleanup(tree.stop)

    def test_reads_pull_request_path_filters(self):
        self.assertIn('services/catalyst/**', ci.workflow_paths('development.yml'))
        self.assertIn('tooling/image-*', ci.workflow_paths('apis-sites-images.yml'))
        self.assertIsNone(ci.workflow_paths('structure.yml'))

    def test_globs_follow_github_semantics(self):
        self.assertTrue(ci.glob_regex('services/catalyst/**').match('services/catalyst/db/src/lib.rs'))
        self.assertTrue(ci.glob_regex('tooling/dev*').match('tooling/dev-smoke.sh'))
        self.assertFalse(ci.glob_regex('tooling/dev*').match('tooling/dev/x.sh'))
        self.assertFalse(ci.glob_regex('flake.*').match('apps/flake.nix'))

    def test_selects_triggered_workflows(self):
        self.assertTrue(ci.triggered('development.yml', {'schemas/001.sql'}))
        self.assertFalse(ci.triggered('development.yml', {'docs/README.md'}))
        self.assertTrue(ci.triggered('apis-sites-images.yml', {'tooling/image-smoke.py'}))
        self.assertTrue(ci.triggered('structure.yml', {'docs/README.md'}))
        names = [s.name for s in ci.pre_push_steps({'docs/README.md'})]
        self.assertEqual(names, ['ownership catalogue', 'module layers'])

    def test_engine_edits_select_the_engine_and_its_sites_consumer(self):
        changed = {'rendering/bevy-explorer/source/src/lib.rs'}
        self.assertTrue(ci.triggered('bevy-explorer-ci.yml', changed))
        self.assertTrue(ci.triggered('apis-sites-images.yml', changed))
        self.assertFalse(ci.triggered('development.yml', changed))

    def test_root_cargo_and_toolchain_edits_select_sites_and_its_web_producer(self):
        for path in ('.cargo/config.toml', 'rust-toolchain', 'rust-toolchain.toml'):
            with self.subTest(path=path):
                self.assertTrue(ci.triggered('apis-sites-images.yml', {path}))
                self.assertTrue(ci.triggered('bevy-explorer-ci.yml', {path}))

    def test_cargo_only_push_and_pr_select_sites_and_its_web_producer(self):
        for workflow in ('bevy-explorer-ci.yml',):
            source = (ci.ROOT / '.github/workflows' / workflow).read_text()
            for event in ('push', 'pull_request'):
                with self.subTest(workflow=workflow, event=event):
                    section = re.search(rf'(?ms)^  {event}:\n(.*?)(?=^  \S|\Z)', source)
                    self.assertIsNotNone(section)
                    paths = [line.strip()[2:].strip('\'"') for line in
                             section.group(1).split('    paths:\n', 1)[1].splitlines()
                             if line.strip().startswith('- ')]
                    self.assertIn('.cargo/**', paths)
                    self.assertTrue(ci.triggered(workflow, {'apps/dcl-scene-cli/Cargo.toml'}, event=event))
                    self.assertFalse(ci.triggered(workflow, {'apps/dcl-scene-cli/src/main.rs'}, event=event))

    def test_bevy_selection_changes_run_their_guards_locally_and_in_ci(self):
        for name in ('tooling/bevy-ci.py', 'tooling/tests/test_bevy_ci.py', 'tooling/tests/test_bevy_protoc.py'):
            changed = {name}
            self.assertTrue(ci.triggered('bevy-explorer-ci.yml', changed))
            self.assertIn(ci.TOOLING_TESTS, ci.pre_push_steps(changed))

    def test_native_build_inputs_publish_headless_packages(self):
        for name in ('rendering/bevy-explorer/source/.cargo/config.toml', 'rendering/bevy-explorer/source/build.rs',
                     'rendering/bevy-explorer/source/crates/scene_runner/src/lib.rs',
                     'rendering/bevy-explorer/source/Cargo.lock', 'rendering/bevy-explorer/source/deploy/headless/launcher/install.js'):
            self.assertTrue(ci.triggered('bevy-explorer-headless.yml', {name}, event='push'), name)
        for name in ('rendering/bevy-explorer/source/react-web/src/App.tsx', 'rendering/bevy-explorer/source/deploy/web/engine/boot.js',
                     'apps/bevy-explorer/Dockerfile.web'):
            self.assertFalse(ci.triggered('bevy-explorer-headless.yml', {name}, event='push'), name)

    def test_gateway_edits_do_not_rebuild_the_applications(self):
        changed = {'infra/gateway/gateway.py'}
        self.assertTrue(ci.triggered('gateway-images.yml', changed))
        self.assertFalse(ci.triggered('apis-sites-images.yml', changed))
        self.assertFalse(ci.triggered('development.yml', changed))
        self.assertFalse(ci.triggered('bevy-explorer-ci.yml', changed))
        self.assertIn('gateway configuration', [step.name for step in ci.pre_commit_steps(changed)])


if __name__ == '__main__':
    unittest.main()
