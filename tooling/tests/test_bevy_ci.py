"""Engine selection uses committed inputs; publication requires explicit producers."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('bevy_ci', Path(__file__).parents[1] / 'bevy-ci.py')
ci = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ci)


def source_run(**changes):
    return {'id': 42, 'status': 'completed', 'conclusion': 'success', 'head_branch': 'main',
            'event': 'push', 'head_repository': {'full_name': 'example/project'},
            'path': ci.WORKFLOW, 'head_sha': 'a' * 40, **changes}


def producers(outcome='success'):
    return [{'name': name, 'status': 'completed', 'conclusion': outcome} for name in ci.PRODUCERS.values()]


class SelectionTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.root_patch = patch.object(ci, 'ROOT', self.root)
        self.root_patch.start()
        self.addCleanup(self.root_patch.stop)
        self.git('init', '-q')
        # A detached Git maintenance process can recreate .git entries while
        # TemporaryDirectory removes this fixture after the last commit.
        self.git('config', 'maintenance.auto', 'false')
        self.git('config', 'gc.auto', '0')
        self.write('rendering/bevy-explorer/source/src/lib.rs', 'fn main() {}\n')
        self.base = self.commit()

    def git(self, *args):
        return subprocess.check_output(['git', '-c', 'user.name=CI', '-c', 'user.email=ci@example.invalid',
                                        *args], cwd=self.root, text=True).strip()

    def write(self, name, value):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(value)

    def commit(self):
        self.git('add', '.')
        self.git('commit', '-qm', 'fixture', '--allow-empty')
        return self.git('rev-parse', 'HEAD')

    def test_sites_hud_review_does_not_select_engine_builds(self):
        self.write('rendering/bevy-explorer/upstream.lock.json', json.dumps({'hud_source_sha256': 'e' * 64}))
        result = ci.select(self.base, self.commit())
        self.assertFalse(any(result[lane] for lane in ci.LANES))
        self.assertFalse(result['scenes'])

    def test_source_and_integration_inputs_select_builds(self):
        for name in ('rendering/bevy-explorer/source/src/lib.rs',
                     'rendering/bevy-explorer/source/react-web/package-lock.json',
                     'apps/bevy-explorer/build-web.py', 'apps/bevy-explorer/Dockerfile.web',
                     'tooling/bevy-ci.py', ci.WORKFLOW,
                     '.github/workflows/bevy-explorer-images.yml'):
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write(name, 'changed')
                result = ci.select(before, self.commit())
                self.assertTrue(any(result[lane] for lane in ci.LANES))

    def test_dependency_toolchain_and_workflow_changes_select_native_matrix(self):
        for name in ('rendering/bevy-explorer/source/Cargo.lock',
                     'rendering/bevy-explorer/source/crates/example/Cargo.toml',
                     'rendering/bevy-explorer/source/.cargo/config.toml',
                     'rendering/bevy-explorer/source/rust-toolchain.toml',
                     '.cargo/config.toml', 'rust-toolchain.toml', ci.WORKFLOW,
                     'library/vendor/rusty_enet/src/packet.rs',
                     'library/vendor/rusty_enet/checksums.sha256',
                     'tooling/dev-regressions.py', 'tooling/dev-regressions.json'):
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write(name, 'changed')
                self.assertTrue(ci.select(before, self.commit())['native-matrix'])

    def test_renaming_or_deleting_source_selects_builds(self):
        path = self.root / 'rendering/bevy-explorer/source/src/lib.rs'
        path.rename(self.root / 'moved.rs')
        result = ci.select(self.base, self.commit())
        self.assertTrue(all(result[lane] for lane in ci.LANES))

    def test_web_assets_validate_web_without_recompiling_native_targets(self):
        for name in ('react-web/src/App.tsx', 'react-web/package-lock.json',
                     'react-web/bridge-scene/src/index.ts', 'deploy/web/engine/boot.js',
                     'deploy/web/service_worker.js'):
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write('rendering/bevy-explorer/source/' + name, 'changed')
                selected = ci.select(before, self.commit())
                self.assertEqual({lane for lane in (*ci.LANES, 'native-matrix', 'scenes')
                                  if selected[lane]}, {'web'})

    def test_web_files_the_harness_tests_read_also_select_the_test_suite(self):
        for name in ('deploy/web/engine/perf_stream.js', 'react-web/src/features/auth/sso.ts',
                     'react-web/src/lib/hudAutomation.ts'):
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write('rendering/bevy-explorer/source/' + name, 'changed')
                selected = ci.select(before, self.commit())
                self.assertEqual({lane for lane in (*ci.LANES, 'native-matrix', 'scenes')
                                  if selected[lane]}, {'web', 'test'})

    def test_web_change_cannot_hide_a_native_or_shared_input_change(self):
        self.write('rendering/bevy-explorer/source/react-web/src/App.tsx', 'web edit')
        self.write('rendering/bevy-explorer/source/crates/system_api_types/src/lib.rs', 'protocol edit')
        selected = ci.select(self.base, self.commit())
        self.assertTrue(all(selected[lane] for lane in (*ci.LANES, 'native-matrix', 'scenes')))

    def test_headless_packages_select_headless_without_web_native_tests_or_scenes(self):
        for name in ('launcher/package.json', 'launcher/bin/cli.js',
                     'launcher/lib/resolve-binary.js', 'build-platform-package.js'):
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write('rendering/bevy-explorer/source/deploy/headless/' + name, 'changed')
                selected = ci.select(before, self.commit())
                self.assertEqual({lane for lane in (*ci.LANES, 'native-matrix', 'scenes')
                                  if selected[lane]}, {'headless'})

    def test_mixed_packages_keep_both_producers_and_rust_changes_widen_coverage(self):
        self.write('rendering/bevy-explorer/source/deploy/headless/launcher/package.json', 'headless')
        self.write('rendering/bevy-explorer/source/deploy/web/package.json', 'web')
        selected = ci.select(self.base, self.commit())
        self.assertEqual({lane for lane in (*ci.LANES, 'native-matrix', 'scenes')
                          if selected[lane]}, {'web', 'headless'})
        self.write('rendering/bevy-explorer/source/crates/dcl/src/lib.rs', 'changed Rust')
        selected = ci.select(self.base, self.commit())
        self.assertTrue(all(selected[lane] for lane in (*ci.LANES, 'native-matrix', 'scenes')))

    def test_deleted_headless_launcher_still_selects_headless(self):
        name = 'rendering/bevy-explorer/source/deploy/headless/launcher/bin/cli.js'
        self.write(name, 'launcher')
        before = self.commit()
        (self.root / name).unlink()
        selected = ci.select(before, self.commit())
        self.assertTrue(selected['headless'])
        self.assertFalse(selected['test'])
        self.assertFalse(selected['web'])

    def test_deleted_web_assets_still_select_their_producer(self):
        name = 'rendering/bevy-explorer/source/react-web/src/App.tsx'
        self.write(name, 'web asset')
        before = self.commit()
        (self.root / name).unlink()
        selected = ci.select(before, self.commit())
        self.assertTrue(selected['web'])
        self.assertFalse(selected['test'])

    def test_pr_uses_supplied_base_and_merge_tree_without_api(self):
        self.write('apps/sites/only-sites.ts', 'changed')
        with patch.object(ci, 'api', side_effect=AssertionError('PR must not consult main runs')):
            self.assertFalse(ci.plan('pull_request', self.base, self.commit(), '')['test'])

    def test_upstream_watch_and_manual_packaging_are_not_ci_build_inputs(self):
        for name in ('upstream', 'desktop', 'rollout', 'rollout-manual'):
            self.write(f'.github/workflows/bevy-explorer-{name}.yml', 'changed')
        result = ci.select(self.base, self.commit())
        self.assertFalse(any(result[lane] for lane in ci.LANES))

    def test_platform_workflow_changes_select_the_shared_linux_producer(self):
        for name in ('bevy-explorer-headless.yml', 'bevy-explorer-headless-build.yml'):
            before = self.git('rev-parse', 'HEAD')
            self.write('.github/workflows/' + name, 'changed')
            selected = ci.select(before, self.commit())
            self.assertTrue(selected['headless'])
            self.assertFalse(selected['web'])
            self.assertFalse(selected['test'])

    def test_image_only_change_selects_consumer_without_engine(self):
        self.write('apps/sites/server.ts', 'changed')
        head = self.commit()
        self.assertTrue(ci.consumer_plan('pull_request', self.base, head, '')['images'])
        self.assertFalse(any(ci.select(self.base, head)[lane] for lane in ci.LANES))

    def test_docs_only_change_skips_image_consumer(self):
        self.write('docs/example.md', 'changed')
        self.assertFalse(ci.consumer_plan('pull_request', self.base, self.commit(), '')['images'])

    def test_consumer_failure_to_find_a_successful_baseline_builds_images(self):
        with patch.object(ci, 'api', return_value={'workflow_runs': []}):
            self.assertTrue(ci.consumer_plan('push', self.base, self.base, 'example/project')['images'])

    def test_push_uses_successful_build_not_previous_canceled_commit(self):
        self.write('rendering/bevy-explorer/source/src/lib.rs', 'engine edit')
        canceled = self.commit()
        self.write('apps/bevy-explorer/README.md', 'docs only')
        with patch.object(ci, 'main_baselines', return_value=dict.fromkeys(ci.LANES, self.base)):
            self.assertTrue(ci.plan('push', canceled, self.commit(), 'example/project')['test'])

    def test_manual_missing_history_and_api_errors_run_full(self):
        with patch.object(ci, 'api', side_effect=OSError('unavailable')):
            for event, base in (('workflow_dispatch', self.base), ('push', self.base),
                                ('pull_request', '0' * 40), ('unknown', self.base)):
                with self.subTest(event=event):
                    result = ci.plan(event, base, self.base, 'example/project')
                    self.assertTrue(all(result[lane] for lane in ci.LANES))
                    self.assertTrue(result['native-matrix'])

    def test_known_integration_inputs_select_only_their_consumers(self):
        cases = {
            'apps/bevy-explorer/Dockerfile.web': {'web'},
            'apps/bevy-explorer/nginx.conf': {'web'},
            'apps/bevy-explorer/build-web.py': {'web'},
            'apps/bevy-explorer/build-headless.py': {'headless'},
            'apps/bevy-explorer/Dockerfile.headless': {'headless'},
            'apps/bevy-explorer/image-smoke.py': {'web', 'headless'},
            '.github/workflows/bevy-explorer-images.yml': {'web', 'headless'},
            'apps/bevy-explorer/prepare-native-cache.py': {'clippy', 'test', 'headless', 'native-matrix', 'scenes'},
            'apps/bevy-explorer/install-linux-deps.py': {*ci.LANES, 'scenes'},
            ci.SCENES_WORKFLOW: {'scenes'},
            ci.WORKFLOW: {*ci.LANES, 'native-matrix'},
            'apps/bevy-explorer/README.md': set(),
            'tooling/tests/test_bevy_linux_deps.py': set(),
        }
        for path, expected in cases.items():
            with self.subTest(path=path):
                before = self.git('rev-parse', 'HEAD')
                self.write(path, 'changed')
                result = ci.select(before, self.commit())
                self.assertEqual({lane for lane in (*ci.LANES, 'native-matrix', 'scenes') if result[lane]}, expected)

    def test_unknown_inputs_and_shared_protoc_changes_run_all_platforms_and_scenes(self):
        for path in ('apps/bevy-explorer/future-build-input', 'apps/bevy-explorer/install-protoc.py',
                     'rendering/bevy-explorer/source/assets/image.png', 'tooling/bevy-ci.py'):
            before = self.git('rev-parse', 'HEAD')
            self.write(path, 'changed')
            result = ci.select(before, self.commit())
            self.assertTrue(all(result[lane] for lane in (*ci.LANES, 'native-matrix', 'scenes')))

    def test_newer_web_pass_cannot_hide_an_unvalidated_native_change_on_main(self):
        self.write('rendering/bevy-explorer/source/src/lib.rs', 'native change')
        later_web = self.commit()
        self.write('apps/bevy-explorer/nginx.conf', 'web change')
        head = self.commit()
        baselines = {**dict.fromkeys(ci.LANES, self.base), 'web': head}
        with patch.object(ci, 'main_baselines', return_value=baselines):
            result = ci.plan('push', later_web, head, 'example/project')
        self.assertFalse(result['web'])
        self.assertTrue(result['test'])
        self.assertTrue(result['headless'])
        self.assertTrue(result['native-matrix'])

    def test_missing_main_lane_baseline_runs_only_that_unproven_lane(self):
        baselines = dict.fromkeys(('clippy', 'test', 'web'), self.base)
        with patch.object(ci, 'main_baselines', return_value=baselines):
            result = ci.plan('push', self.base, self.base, 'example/project')
        self.assertTrue(result['headless'])
        self.assertFalse(result['web'])
        self.assertFalse(result['test'])
        self.assertFalse(result['native-matrix'])

    def test_graphical_selection_uses_event_diff_and_manual_run_is_full(self):
        self.write('apps/bevy-explorer/Dockerfile.web', 'web image change')
        head = self.commit()
        self.assertFalse(ci.scenes_plan('pull_request', self.base, head)['scenes'])
        self.assertFalse(ci.scenes_plan('push', self.base, head)['scenes'])
        self.assertTrue(ci.scenes_plan('workflow_dispatch', self.base, head)['scenes'])
        self.write('rendering/bevy-explorer/source/src/lib.rs', 'engine change')
        head = self.commit()
        self.assertTrue(ci.scenes_plan('pull_request', self.base, head)['scenes'])
        self.assertEqual(ci.scenes_plan('push', self.base, head)['sha'], head)
        self.assertTrue(ci.scenes_plan('push', '0' * 40, head)['scenes'])


class PublicationTest(unittest.TestCase):
    def test_only_successful_same_repository_main_ci_is_trusted(self):
        self.assertTrue(ci.trusted_run(source_run(), 'example/project'))
        for changed in ({'status': 'in_progress'}, {'conclusion': 'failure'}, {'event': 'pull_request'},
                        {'head_branch': 'dev'}, {'head_repository': {'full_name': 'fork/project'}},
                        {'path': '.github/workflows/unrelated.yml'}, {'head_sha': 'not-a-sha'}):
            with self.subTest(changed=changed):
                self.assertFalse(ci.trusted_run(source_run(**changed), 'example/project'))

    def test_products_are_selected_independently_from_successful_producers(self):
        self.assertEqual(ci.produced_artifacts(producers('skipped')), [])
        self.assertEqual(ci.produced_artifacts(producers()), ['web', 'headless'])
        for product in ci.PRODUCERS:
            jobs = producers('skipped')
            next(job for job in jobs if job['name'] == ci.PRODUCERS[product])['conclusion'] = 'success'
            self.assertEqual(ci.produced_artifacts(jobs), [product])
            entries = ci.image_matrix([product])['include']
            self.assertEqual({row['product'] for row in entries}, {product})
            self.assertEqual({row['arch'] for row in entries},
                             {'amd64', 'arm64'} if product == 'web' else {'amd64'})

    def test_missing_failed_duplicate_or_running_producer_never_allows_publication(self):
        invalid = [[], producers()[:1], producers() * 2, producers('failure'), producers('cancelled')]
        for outcome in ('failure', 'cancelled', 'timed_out'):
            mixed = producers()
            mixed[0]['conclusion'] = outcome
            invalid.append(mixed)
        running = producers()
        running[0]['status'] = 'in_progress'
        invalid.append(running)
        for jobs in invalid:
            with self.subTest(jobs=jobs), self.assertRaises(ValueError):
                ci.produced_artifacts(jobs)

    def test_baselines_are_per_lane_and_ignore_untrusted_or_skipped_jobs(self):
        runs = [source_run(id=44, event='pull_request'), source_run(id=43),
                source_run(id=42, head_sha='b' * 40), source_run(id=41, head_sha='c' * 40)]
        web_only = producers('skipped')
        web_only[0]['conclusion'] = 'success'
        native = producers() + [
            {'name': name, 'status': 'completed', 'conclusion': 'success'}
            for name in ('Clippy', *ci.NATIVE_JOBS)]
        with patch.object(ci, 'api', return_value={'workflow_runs': runs}), \
                patch.object(ci, 'run_jobs', side_effect=[producers('skipped'), web_only, native]) as jobs:
            expected = {**dict.fromkeys(ci.LANES, 'c' * 40), 'web': 'b' * 40}
            self.assertEqual(ci.main_baselines('example/project'), expected)
            self.assertEqual([call.args[1] for call in jobs.call_args_list], [43, 42, 41])

    def test_partial_or_duplicate_native_matrix_is_not_a_passing_baseline(self):
        jobs = [{'name': name, 'status': 'completed', 'conclusion': 'success'} for name in sorted(ci.NATIVE_JOBS)]
        self.assertTrue(ci.passed_lane(jobs, 'test'))
        linux = next(job for job in jobs if job['name'] == 'Test Suite (ubuntu-24.04)')
        self.assertTrue(ci.passed_lane([linux], 'test'))
        for invalid in ([], jobs[:2], jobs + [linux], [{**linux, 'conclusion': 'skipped'}]):
            self.assertFalse(ci.passed_lane(invalid, 'test'))

    def test_missing_baseline_or_incomplete_job_list_cannot_prove_a_pass(self):
        with patch.object(ci, 'api', return_value={'workflow_runs': []}):
            self.assertEqual(ci.main_baselines('example/project'), {})
        with patch.object(ci, 'api', return_value={'total_count': 3, 'jobs': producers()}), \
                self.assertRaises(ValueError):
            ci.run_jobs('example/project', 42)

    def test_source_command_reports_skip_without_relabeling_another_commit(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / 'output'
            env = {'GH_REPO': 'example/project', 'SOURCE_RUN': '42', 'GITHUB_REF': 'refs/heads/dev',
                   'GITHUB_SHA': 'b' * 40, 'GITHUB_OUTPUT': str(output), 'GITHUB_STEP_SUMMARY': ''}
            with patch.dict(os.environ, env), patch('sys.argv', ['bevy-ci.py', 'source']), \
                    patch.object(ci, 'api', return_value=source_run()), \
                    patch.object(ci, 'run_jobs', return_value=producers('skipped')):
                ci.main()
            text = output.read_text()
            self.assertIn('build=false\n', text)
            self.assertIn(f"sha={'a' * 40}\n", text)
            self.assertNotIn('b' * 40, text)

    def test_source_command_exports_only_the_successful_product_and_exact_source_sha(self):
        jobs = producers('skipped')
        jobs[0]['conclusion'] = 'success'
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / 'output'
            env = {'GH_REPO': 'example/project', 'SOURCE_RUN': '42',
                   'GITHUB_OUTPUT': str(output), 'GITHUB_STEP_SUMMARY': ''}
            with patch.dict(os.environ, env), patch('sys.argv', ['bevy-ci.py', 'source']), \
                    patch.object(ci, 'api', return_value=source_run()), \
                    patch.object(ci, 'run_jobs', return_value=jobs):
                ci.main()
            values = dict(line.split('=', 1) for line in output.read_text().splitlines())
            self.assertEqual(values['sha'], 'a' * 40)
            self.assertEqual(values['build'], 'true')
            self.assertEqual(json.loads(values['products']), ['web'])
            self.assertEqual(json.loads(values['build-matrix']), ci.image_matrix(['web']))


if __name__ == '__main__':
    unittest.main()
