"""Only known backend-only PRs may skip frontend units; contracts stay separate."""
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('dev_ci', Path(__file__).parents[1] / 'dev-ci.py')
ci = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ci)


class FrontendSelectionTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        root_patch = patch.object(ci, 'ROOT', self.root)
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
            ['git', '-c', 'user.name=CI', '-c', 'user.email=ci@example.invalid', *args],
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
        return ci.plan(event, base or self.base, head)['run-web-unit-tests']

    def test_backend_implementation_and_explicit_companion_docs_skip_units(self):
        self.write(self.backend, 'changed')
        self.write('services/catalyst/src/serve.rs', 'backend')
        self.write('services/catalyst/tests/storage.rs', 'backend test')
        for name in ci.COMPANION_DOCS:
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
            with self.subTest(event=event), patch.object(ci, 'ensure_commit') as ensure:
                self.assertTrue(ci.plan(event, '', '')['run-web-unit-tests'])
                ensure.assert_not_called()

    def test_missing_or_invalid_comparison_runs_units(self):
        for sha in ('', '0' * 40, 'HEAD', '--help'):
            with self.subTest(sha=sha):
                self.assertTrue(ci.plan('pull_request', sha, self.base)['run-web-unit-tests'])
                self.assertTrue(ci.plan('pull_request', self.base, sha)['run-web-unit-tests'])

    def test_missing_commit_fetch_failure_runs_units(self):
        with patch.object(ci.subprocess, 'run', side_effect=[
            subprocess.CompletedProcess([], 1), subprocess.CalledProcessError(1, ['git', 'fetch'])
        ]) as run:
            self.assertTrue(ci.plan('pull_request', 'a' * 40, self.base)['run-web-unit-tests'])
        self.assertEqual(run.call_args_list[1].args[0],
                         ['git', 'fetch', '--no-tags', '--depth=1', 'origin', 'a' * 40])

    def test_diff_failure_runs_units(self):
        with patch.object(ci.subprocess, 'check_output', side_effect=OSError('git unavailable')):
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
            ci.write_outputs({'run-web-unit-tests': False, 'reason': 'backend only'})
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
                       'library/catalyst-api/backend-source.ts'):
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
                        '"services/" + "catalyst/" + crate + "/src/lib.rs"',
                        '`services/catalyst/${crate}/src/lib.rs`',
                        '"services/catalyst/" + crate + "/src/lib.rs"'):
            with self.subTest(content=content):
                self.write('apps/sites/src/helper.ts', content)
                before = self.commit()
                self.write(self.backend, content)
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
                    ci, 'frontend_backend_references', side_effect=failure):
                self.assertTrue(self.selected(head))

    def test_git_grep_error_is_not_treated_as_no_references(self):
        self.write(self.backend, 'changed')
        head = self.commit()
        run = subprocess.run

        def fail_scan(args, **kwargs):
            if args[:2] == ['git', 'grep']:
                return subprocess.CompletedProcess(args, 2, stdout='', stderr='scan failed')
            return run(args, **kwargs)

        with patch.object(ci.subprocess, 'run', side_effect=fail_scan):
            self.assertTrue(self.selected(head))

    def test_missing_frontend_root_keeps_units(self):
        (self.root / 'apps/sites/src/example.ts').unlink()
        before = self.commit()
        self.write(self.backend, 'changed')
        self.assertTrue(self.selected(self.commit(), before))


if __name__ == '__main__':
    unittest.main()
