"""Pull requests run the Rust tests on macOS for build, toolchain and dev-stack inputs; source stays on Linux."""
import contextlib
import importlib.util
import io
import os
from pathlib import Path
import re
import subprocess
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
        self.dev = dev.load_ci_local()

    def macos(self, *changed):
        return dev.needs_macos(changed, self.dev)[0]

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
                     'services/catalyst/rust-toolchain.toml', 'library/vendor/rusty_enet/rust-toolchain',
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

    def test_main_runs_are_not_cancelled_by_the_next_push(self):
        # PRs test source on Linux only; finished main runs are its macOS check.
        workflow = (dev.ROOT / '.github/workflows/development.yml').read_text()
        self.assertIn("cancel-in-progress: ${{ github.event_name == 'pull_request' }}", workflow)

    def test_unclassified_or_unfiltered_triggers_run_macos(self):
        with patch.object(self.dev, 'workflow_paths', return_value=['apps/ui3/**', 'services/new/**']):
            self.assertTrue(self.macos('services/new/src/lib.rs'))
            self.assertFalse(self.macos('apps/ui3/src/a.ts'))
        with patch.object(self.dev, 'workflow_paths', return_value=None):
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


class FrontendSelectionTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        root_patch = patch.object(dev, 'ROOT', self.root)
        root_patch.start()
        self.addCleanup(root_patch.stop)
        self.git('init', '-q')
        self.git('config', 'maintenance.auto', 'false')
        self.git('config', 'gc.auto', '0')
        self.git('config', 'commit.gpgsign', 'false')
        self.backend = 'services/catalyst/worlds/src/storage/export.rs'
        self.write(self.backend, 'original')
        self.write('apps/ui3/src/example.test.ts', 'frontend')
        self.write('apps/sites/src/example.ts', 'sites')
        self.write('library/catalyst-api/example.ts', 'shared helper')
        self.base = self.commit()

    def git(self, *args):
        return subprocess.check_output(
            ['git', '-c', 'user.name=CI', '-c', 'user.email=dev@example.invalid', *args],
            cwd=self.root, text=True,
        ).strip()

    def write(self, name, value):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(value)

    def commit(self):
        self.git('add', '.')
        self.git('commit', '-qm', 'fixture', '--allow-empty')
        return self.git('rev-parse', 'HEAD')

    def selected(self, head, base=None, event='pull_request'):
        return dev.web_plan(event, base or self.base, head)['run-web-unit-tests']

    def test_backend_implementation_and_explicit_companion_docs_skip_units(self):
        self.write(self.backend, 'changed')
        self.write('services/catalyst/src/serve.rs', 'backend')
        self.write('services/catalyst/tests/storage.rs', 'backend test')
        for name in dev.COMPANION_DOCS:
            self.write(name, 'documentation')
        self.assertFalse(self.selected(self.commit()))

    def test_shared_frontend_config_manifests_and_unknown_files_keep_units(self):
        for name in ('apps/ui3/src/example.test.ts', 'apps/sites/package.json',
                     'apps/ui3/editor-scene/src/index.ts', 'schemas/storage.sql',
                     'library/catalyst-api/worlds/schema.ts', 'services/catalyst/shared/src/lib.rs',
                     'services/catalyst/config/src/worlds.rs', 'services/catalyst/worlds/Cargo.toml',
                     'services/catalyst/Cargo.lock', 'services/catalyst/build.rs',
                     'services/catalyst/new-crate/src/lib.rs', 'services/catalyst/worlds/schema.sql',
                     'flake.lock', 'Makefile', '.cargo/config.toml',
                     '.github/workflows/development.yml', 'tooling/dev-ci.py',
                     'apps/sites/content/docs/example.md', 'docs/unknown.md'):
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write(self.backend, name)
                self.write(name, 'changed')
                self.assertTrue(self.selected(self.commit(), before))

    def test_empty_and_documentation_only_diffs_keep_units(self):
        self.assertTrue(self.selected(self.base))
        self.write('DATABASE.md', 'documentation')
        self.assertTrue(self.selected(self.commit()))

    def test_backend_deletion_can_skip_but_frontend_deletion_cannot(self):
        (self.root / self.backend).unlink()
        self.assertFalse(self.selected(self.commit()))
        (self.root / 'apps/ui3/src/example.test.ts').unlink()
        self.assertTrue(self.selected(self.commit()))

    def test_rename_from_frontend_into_backend_keeps_units(self):
        source = self.root / 'apps/ui3/src/example.test.ts'
        target = self.root / 'services/catalyst/worlds/src/renamed.rs'
        source.rename(target)
        self.assertTrue(self.selected(self.commit()))

    def test_rename_from_backend_to_unknown_path_keeps_units(self):
        (self.root / self.backend).rename(self.root / 'moved.rs')
        self.assertTrue(self.selected(self.commit()))

    def test_main_manual_and_unknown_events_always_run_units(self):
        for event in ('push', 'workflow_dispatch', 'schedule', '', 'pull_request_target'):
            with self.subTest(event=event), patch.object(dev, 'ensure_commit') as ensure:
                self.assertTrue(dev.web_plan(event, '', '')['run-web-unit-tests'])
                ensure.assert_not_called()

    def test_missing_or_invalid_comparison_runs_units(self):
        for sha in ('', '0' * 40, 'HEAD', '--help'):
            with self.subTest(sha=sha):
                self.assertTrue(dev.web_plan('pull_request', sha, self.base)['run-web-unit-tests'])
                self.assertTrue(dev.web_plan('pull_request', self.base, sha)['run-web-unit-tests'])

    def test_missing_commit_fetch_failure_runs_units(self):
        with patch.object(dev.subprocess, 'run', side_effect=[
            subprocess.CompletedProcess([], 1), subprocess.CalledProcessError(1, ['git', 'fetch'])
        ]) as run:
            self.assertTrue(dev.web_plan('pull_request', 'a' * 40, self.base)['run-web-unit-tests'])
        self.assertEqual(run.call_args_list[1].args[0],
                         ['git', 'fetch', '--no-tags', '--depth=1', 'origin', 'a' * 40])

    def test_diff_failure_runs_units(self):
        with patch.object(dev.subprocess, 'check_output', side_effect=OSError('git unavailable')):
            self.assertTrue(self.selected(self.base))

    def test_compares_supplied_base_to_tested_merge_not_pr_head(self):
        self.write(self.backend, 'PR change')
        pr_head = self.commit()
        self.git('checkout', '-qb', 'base-changes', self.base)
        self.write('apps/ui3/src/example.test.ts', 'base update')
        actual_base = self.commit()
        self.git('checkout', '-q', '--detach', pr_head)
        self.git('merge', '-q', '--no-edit', actual_base)
        tested_merge = self.git('rev-parse', 'HEAD')
        # The base's frontend edit is already in the tested merge, so only the
        # backend PR changes remain when comparing the correct two trees.
        self.assertFalse(self.selected(tested_merge, actual_base))
        self.assertTrue(self.selected(pr_head, actual_base))

    def test_outputs_lowercase_boolean_and_reason(self):
        output = self.root / 'output'
        summary = self.root / 'summary'
        with patch.dict(os.environ, {'GITHUB_OUTPUT': str(output), 'GITHUB_STEP_SUMMARY': str(summary)}):
            dev.write_outputs({'run-web-unit-tests': False, 'reason': 'backend only'})
        self.assertEqual(output.read_text(), 'run-web-unit-tests=false\nreason=backend only\n')
        self.assertIn('backend only', summary.read_text())

    def test_rust_files_read_by_frontend_tests_keep_units(self):
        for source in ('services/catalyst/shop/src/mana/quote.rs',
                       'services/catalyst/explore/src/places/rows.rs',
                       'services/catalyst/explore/src/copies.rs'):
            with self.subTest(source=source):
                self.write('apps/sites/src/source.test.ts',
                           f'readFileSync(new URL("../../../../../{source}", import.meta.url));')
                self.write(source, 'original')
                before = self.commit()
                self.write(source, 'changed')
                self.assertTrue(self.selected(self.commit(), before))

    def test_helper_and_source_references_are_not_limited_to_test_filenames(self):
        for helper in ('apps/ui3/src/helpers/backend-source.ts',
                       'apps/sites/src/backend-source.mjs',
                       'apps/ui3/editor-scene/test/source.cjs',
                       'library/catalyst-api/backend-source.ts',
                       'apps/sites/src/fixtures/backend-source.json'):
            with self.subTest(helper=helper):
                self.write(helper, f'export const source = "{self.backend}";')
                before = self.commit()
                self.write(self.backend, helper)
                self.assertTrue(self.selected(self.commit(), before))

    def test_unrelated_literal_dependency_does_not_select_frontend_units(self):
        self.write('apps/sites/src/helper.ts',
                   'const source = "services/catalyst/shop/src/mana/quote.rs";')
        before = self.commit()
        self.write(self.backend, 'unrelated')
        self.assertFalse(self.selected(self.commit(), before))

    def test_backend_directory_reference_covers_descendant_source(self):
        self.write('apps/sites/src/helper.ts',
                   'const directory = "services/catalyst/worlds/src/storage";')
        before = self.commit()
        self.write(self.backend, 'changed')
        self.assertTrue(self.selected(self.commit(), before))

    def test_computed_or_split_backend_paths_keep_units(self):
        for content in ('join(root, "services", "catalyst", crate, "src/lib.rs")',
                        'const root = new URL("../../../services", import.meta.url); join(root, "catalyst", "worlds/src/storage/export.rs")',
                        '"services/" + "catalyst/" + crate + "/src/lib.rs"',
                        '`services/catalyst/${crate}/src/lib.rs`',
                        '"services/catalyst/" + crate + "/src/lib.rs"'):
            with self.subTest(content=content):
                self.write('apps/sites/src/helper.ts', content)
                before = self.commit()
                self.write(self.backend, content)
                self.assertTrue(self.selected(self.commit(), before))

    def test_sentence_punctuation_does_not_hide_a_referenced_rust_file(self):
        self.write('apps/sites/src/helper.ts', f'// This reads {self.backend}.')
        before = self.commit()
        self.write(self.backend, 'changed')
        self.assertTrue(self.selected(self.commit(), before))

    def test_referenced_backend_deletion_and_rename_keep_units(self):
        self.write('apps/ui3/src/helper.ts', f'const source = "{self.backend}";')
        before = self.commit()
        (self.root / self.backend).rename(self.root / 'services/catalyst/worlds/src/renamed.rs')
        self.assertTrue(self.selected(self.commit(), before))
        self.write(self.backend, 'restored')
        before = self.commit()
        (self.root / self.backend).unlink()
        self.assertTrue(self.selected(self.commit(), before))

    def test_frontend_scan_reads_tested_tree_not_working_files(self):
        self.write('apps/ui3/src/helper.ts', f'const source = "{self.backend}";')
        before = self.commit()
        self.write(self.backend, 'changed')
        head = self.commit()
        (self.root / 'apps/ui3/src/helper.ts').unlink()
        self.assertTrue(self.selected(head, before))

    def test_dependency_scan_error_keeps_units(self):
        self.write(self.backend, 'changed')
        head = self.commit()
        for failure in (OSError('unreadable frontend'), ValueError('ambiguous dependency'),
                        subprocess.CalledProcessError(2, ['git', 'grep'])):
            with self.subTest(failure=failure), patch.object(
                    dev, 'frontend_backend_references', side_effect=failure):
                self.assertTrue(self.selected(head))

    def test_git_grep_error_is_not_treated_as_no_references(self):
        self.write(self.backend, 'changed')
        head = self.commit()
        run = subprocess.run

        def fail_scan(args, **kwargs):
            if args[:2] == ['git', 'grep']:
                return subprocess.CompletedProcess(args, 2, stdout='', stderr='scan failed')
            return run(args, **kwargs)

        with patch.object(dev.subprocess, 'run', side_effect=fail_scan):
            self.assertTrue(self.selected(head))

    def test_missing_frontend_root_keeps_units(self):
        (self.root / 'apps/sites/src/example.ts').unlink()
        before = self.commit()
        self.write(self.backend, 'changed')
        self.assertTrue(self.selected(self.commit(), before))


if __name__ == '__main__':
    unittest.main()
