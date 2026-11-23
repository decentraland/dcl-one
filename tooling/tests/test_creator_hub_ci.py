"""Creator Hub e2e selection: what a pull request's changes require, and when the label runs it."""
import importlib.util
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('creator_hub_ci', ROOT / 'tooling/creator-hub-ci.py')
ci = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ci)

INSPECTOR = 'apps/creator-hub/packages/inspector/'
E2E = INSPECTOR + 'test/e2e/'
CH = 'apps/creator-hub/packages/creator-hub/'
HIERARCHY_TSX = INSPECTOR + 'src/components/Hierarchy/Hierarchy.tsx'
HIERARCHY_SPECS = ['test/e2e/Hierarchy.spec.ts', 'test/e2e/HierarchyAutoScroll.spec.ts']
ALL_SPECS = sorted('test/e2e/' + path.name for path in (ROOT / E2E).glob('*.spec.ts'))


def resolve(*changed, impact=None):
    return ci.resolve(list(changed), ci.load_selection(), impact, ci.e2e_importers())


def pull_request(*changed, labels=(), action='opened', attempt=1, impact=None):
    return ci.plan('pull_request', action, attempt, list(changed), list(labels), ci.load_selection(), impact,
                   ci.e2e_importers())


class SelectionTest(unittest.TestCase):

    def test_an_inspector_stylesheet_requires_no_e2e(self):
        got = resolve(INSPECTOR + 'src/components/Hierarchy/Hierarchy.css')
        self.assertEqual((got.inspector, got.offline, got.live), (set(), False, False))

    def test_a_creator_hub_stylesheet_requires_the_offline_suite(self):
        got = resolve(CH + 'renderer/src/components/SignInCard/styles.css')
        self.assertEqual((got.inspector, got.offline, got.live), (set(), True, False))

    def test_unit_tests_and_docs_require_no_e2e(self):
        got = resolve(INSPECTOR + 'src/lib/sdk/engine.spec.ts', CH + 'renderer/src/lib/deploy.spec.ts',
                      'apps/creator-hub/docs/testing.md')
        self.assertEqual((got.inspector, got.offline, got.live), (set(), False, False))

    def test_inspector_source_runs_the_specs_the_impact_map_names(self):
        got = resolve(HIERARCHY_TSX, impact={HIERARCHY_TSX: HIERARCHY_SPECS})
        self.assertEqual((got.inspector, got.offline), (set(HIERARCHY_SPECS), False))

    def test_inspector_source_no_spec_executes_requires_no_inspector_e2e(self):
        path = INSPECTOR + 'src/components/WearablePreview/WearablePreview.tsx'
        self.assertEqual(resolve(path, impact={path: []}).inspector, set())

    def test_inspector_source_missing_from_the_map_runs_the_whole_suite(self):
        self.assertEqual(resolve(HIERARCHY_TSX, impact={}).inspector, ci.ALL)
        self.assertEqual(resolve(HIERARCHY_TSX, impact=None).inspector, ci.ALL)

    def test_a_map_naming_a_deleted_spec_runs_the_whole_suite(self):
        got = resolve(HIERARCHY_TSX, impact={HIERARCHY_TSX: ['test/e2e/Gone.spec.ts']})
        self.assertEqual(got.inspector, ci.ALL)

    def test_a_spec_runs_itself(self):
        self.assertEqual(resolve(E2E + 'Transform.spec.ts').inspector, {'test/e2e/Transform.spec.ts'})

    def test_a_page_object_runs_the_specs_that_import_it(self):
        self.assertEqual(resolve(E2E + 'pageObjects/Transform.ts').inspector, {'test/e2e/Transform.spec.ts'})

    def test_a_helper_runs_the_specs_that_reach_it_through_page_objects(self):
        got = resolve(E2E + 'utils/sleep.ts').inspector
        self.assertIn('test/e2e/Transform.spec.ts', got)
        self.assertIn('test/e2e/MobileHud.spec.ts', got)

    def test_the_shared_fixture_runs_the_whole_suite(self):
        self.assertEqual(resolve(E2E + 'fixtures.ts').inspector, set(ALL_SPECS))

    def test_an_e2e_file_no_spec_imports_runs_the_whole_suite(self):
        self.assertEqual(resolve(E2E + 'types.d.ts').inspector, ci.ALL)

    def test_bundle_inputs_run_the_whole_inspector_suite(self):
        for path in (INSPECTOR + 'build.js', INSPECTOR + 'package.json', 'apps/creator-hub/pnpm-lock.yaml'):
            with self.subTest(path=path):
                self.assertEqual(resolve(path).inspector, ci.ALL)

    def test_creator_hub_source_requires_only_the_offline_suite(self):
        got = resolve(CH + 'renderer/src/components/Home/Home.tsx')
        self.assertEqual((got.inspector, got.offline, got.live), (set(), True, False))

    def test_deployment_code_requires_the_live_and_offline_suites(self):
        for path in (CH + 'renderer/src/modules/store/deployment/slice.ts', CH + 'main/src/modules/cli.ts',
                     CH + 'e2e/pages/PublishModal.ts'):
            with self.subTest(path=path):
                got = resolve(path)
                self.assertEqual((got.inspector, got.offline, got.live), (set(), True, True))

    def test_the_app_build_scripts_require_the_offline_suite(self):
        for path in ('apps/creator-hub/scripts/rolldown.js', 'apps/creator-hub/scripts/rolldown-env.js'):
            with self.subTest(path=path):
                got = resolve(path)
                self.assertEqual((got.creator_hub, got.offline), (True, True))

    def test_shop_files_the_renderer_only_typechecks_run_the_pipeline_without_e2e(self):
        for path in ('apps/ui3/src/shop/checkout/chain/config.ts', 'apps/ui3/src/shop/pageOverride.ts'):
            with self.subTest(path=path):
                got = resolve(path)
                self.assertEqual((got.creator_hub, got.inspector, got.offline, got.live), (True, set(), False, False))

    def test_shop_checkout_files_the_renderer_never_reaches_are_not_creator_hub_changes(self):
        for path in ('apps/ui3/src/shop/checkout/chain/metatx.ts', 'apps/ui3/src/shop/checkout/chain/wallet.ts'):
            with self.subTest(path=path):
                self.assertFalse(resolve(path).creator_hub)

    def test_what_the_live_specs_run_on_requires_the_live_suite(self):
        for path in ('apps/creator-hub/patches/@dcl__sdk-commands@7.29.0.patch', CH + 'playwright.config.ts',
                     CH + 'e2e/fixtures.ts', CH + 'e2e/helpers/app.ts', CH + 'e2e/pages/EditorPage.ts',
                     CH + 'e2e/pages/ScenesPage.ts', CH + 'e2e/pages/Auth.ts', 'apps/creator-hub/pnpm-lock.yaml',
                     'apps/creator-hub/Makefile', '.github/workflows/creator-hub-e2e.yml'):
            with self.subTest(path=path):
                got = resolve(path)
                self.assertEqual((got.offline, got.live), (True, True))

    def test_an_atom_shared_by_both_bundles_requires_both_suites(self):
        got = resolve('apps/ui3/src/atoms/Button.tsx', impact=None)
        self.assertEqual((got.inspector, got.offline), (ci.ALL, True))

    def test_outside_the_creator_hub_nothing_is_required(self):
        got = resolve('services/catalyst/src/main.rs', 'apps/ui3/src/play/Hud.tsx')
        self.assertEqual((got.creator_hub, got.inspector, got.offline, got.live), (False, set(), False, False))

    def test_the_planner_and_its_inputs_count_as_creator_hub_changes(self):
        for path in ('tooling/creator-hub-ci.py', 'apps/creator-hub/e2e-selection.json',
                     '.github/workflows/creator-hub-e2e.yml', 'library/web-core/site.ts'):
            with self.subTest(path=path):
                self.assertTrue(resolve(path).creator_hub)


class PlanTest(unittest.TestCase):

    def test_not_required_passes_the_status(self):
        out = pull_request(INSPECTOR + 'src/App.css')
        self.assertEqual((out['required'], out['run'], out['state']), ('false', 'false', 'success'))

    def test_required_without_the_label_leaves_the_status_pending_and_runs_nothing(self):
        out = pull_request(E2E + 'Transform.spec.ts')
        self.assertEqual((out['required'], out['run'], out['state']), ('true', 'false', 'pending'))
        self.assertIn('add label ci:creator-hub-e2e', out['description'])

    def test_the_label_runs_what_is_required(self):
        out = pull_request(E2E + 'Transform.spec.ts', labels=['ci:creator-hub-e2e'])
        self.assertEqual((out['run'], out['state'], out['inspector-specs']),
                         ('true', 'pending', 'test/e2e/Transform.spec.ts'))
        self.assertEqual((out['creator-hub-offline'], out['creator-hub-live']), ('false', 'false'))

    def test_the_label_runs_the_live_tier_for_a_deploy_change(self):
        out = pull_request(CH + 'renderer/src/modules/store/deployment/slice.ts', labels=['ci:creator-hub-e2e'])
        self.assertEqual((out['inspector'], out['creator-hub-offline'], out['creator-hub-live']),
                         ('false', 'true', 'true'))

    def test_the_label_keeps_the_live_tier_off_for_other_app_changes(self):
        out = pull_request(CH + 'renderer/src/components/Home/Home.tsx', labels=['ci:creator-hub-e2e'])
        self.assertEqual((out['creator-hub-offline'], out['creator-hub-live']), ('true', 'false'))

    def test_a_push_takes_the_label_off(self):
        out = pull_request(E2E + 'Transform.spec.ts', labels=['ci:creator-hub-e2e'], action='synchronize')
        self.assertEqual((out['run'], out['state'], out['remove-label']), ('false', 'pending', 'true'))

    def test_a_rerun_after_the_label_is_put_back_runs(self):
        out = pull_request(E2E + 'Transform.spec.ts', labels=['ci:creator-hub-e2e'], action='synchronize', attempt=2)
        self.assertEqual((out['run'], out['remove-label']), ('true', 'false'))

    def test_reopening_takes_the_label_off(self):
        out = pull_request(E2E + 'Transform.spec.ts', labels=['ci:creator-hub-e2e'], action='reopened')
        self.assertEqual((out['run'], out['state'], out['remove-label']), ('false', 'pending', 'true'))

    def test_a_spec_name_unsafe_for_the_shell_runs_the_whole_suite(self):
        for name in ('test/e2e/a;b.spec.ts', 'test/e2e/a b.spec.ts', 'test/e2e/$(id).spec.ts'):
            with self.subTest(name=name):
                path = E2E + name.removeprefix('test/e2e/')
                out = ci.plan('pull_request', 'opened', 1, [path], ['ci:creator-hub-e2e'], ci.load_selection(),
                              None, {path: {name}})
                self.assertEqual((out['inspector'], out['inspector-specs']), ('true', ''))

    def test_no_output_carries_a_newline_from_a_file_name(self):
        path = E2E + 'a\nrun=true.spec.ts'
        out = pull_request(path, labels=['ci:creator-hub-e2e'])
        out.pop('rows')
        self.assertFalse([key for key, value in out.items() if '\n' in value])

    def test_the_whole_suite_passes_no_spec_filter(self):
        out = pull_request(INSPECTOR + 'build.js', labels=['ci:creator-hub-e2e'])
        self.assertEqual((out['inspector'], out['inspector-specs']), ('true', ''))

    def test_the_description_fits_a_commit_status(self):
        out = pull_request(INSPECTOR + 'build.js', CH + 'renderer/src/lib/deploy.ts')
        self.assertLessEqual(len(out['description']), 140)

    def test_main_runs_everything(self):
        for event in ('push', 'workflow_dispatch'):
            with self.subTest(event=event):
                out = ci.plan(event, '', 1, [], [], ci.load_selection(), None, ci.e2e_importers())
                self.assertEqual((out['creator-hub'], out['run'], out['inspector'], out['inspector-specs'],
                                  out['creator-hub-offline'], out['creator-hub-live']),
                                 ('true', 'true', 'true', '', 'true', 'true'))

    def test_outside_the_creator_hub_the_status_passes(self):
        out = pull_request('services/catalyst/src/main.rs')
        self.assertEqual((out['creator-hub'], out['state']), ('false', 'success'))


class SelectionFileTest(unittest.TestCase):

    def test_every_pattern_names_a_tracked_file(self):
        tracked = subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard'], cwd=ROOT, text=True).splitlines()
        for name, patterns in ci.patterns(ci.load_selection()):
            for pattern in patterns:
                if pattern.startswith('!'):
                    continue
                with self.subTest(list=name, pattern=pattern):
                    self.assertTrue(ci.paths.matching_paths(tracked, [pattern]), pattern)


if __name__ == '__main__':
    unittest.main()
