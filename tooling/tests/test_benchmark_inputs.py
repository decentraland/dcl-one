"""The benchmark starts for this repository's PRs whose passed CI run built a web app, and finds its builds."""
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import tempfile
import unittest
from unittest.mock import patch
import zipfile

SPEC = importlib.util.spec_from_file_location('benchmark_inputs', Path(__file__).parents[1] / 'benchmark-inputs.py')
inputs = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(inputs)

REPO = 'example/project'
HEAD = 'a' * 40
MERGE = 'b' * 40
MAIN = ('c' * 40, 'd' * 40, 'e' * 40)
RUNNERS = ('windows-amd64', 'macos-arm64', 'linux-amd64')


def ci_run(run_id, sha, event='pull_request', branch='feature', **changes):
    return {'id': run_id, 'path': inputs.WORKFLOW, 'status': 'completed', 'conclusion': 'success',
            'event': event, 'head_branch': branch, 'head_sha': sha,
            'head_repository': {'full_name': REPO}, 'pull_requests': [{'number': 7}], **changes}


def artifacts(sha, web=True, runners=RUNNERS):
    names = [f'bevy-explorer-benchmark-{platform}-{sha}' for platform in runners]
    return names + ([f'bevy-explorer-web-{sha}'] if web else []) + [f'bevy-explorer-native-timings-Linux-X64-{sha}']


class GitHub:
    """The REST reads the resolver makes, answered from fixtures."""

    def __init__(self):
        self.pr = {'number': 7, 'state': 'open', 'labels': [],
                   'head': {'sha': HEAD, 'ref': 'feature', 'repo': {'full_name': REPO}}}
        self.change_runs = [ci_run(100, HEAD)]
        self.main_runs = [ci_run(90 - n, sha, event='push', branch='main') for n, sha in enumerate(MAIN)]
        # A PR run's artifacts carry the merge commit it built; a main run's its own commit.
        self.artifacts = {100: artifacts(MERGE), **{run['id']: artifacts(run['head_sha']) for run in self.main_runs}}
        self.expired = set()
        # build-info.json per web app, an optimized engine unless given here; None leaves it out.
        self.build_info = {}
        self.ids = {}
        self.calls = []

    def web_zip(self, name):
        archive = io.BytesIO()
        with zipfile.ZipFile(archive, 'w') as app:
            app.writestr('index.html', '<html>')
            info = self.build_info.get(name, {'engine': {'optimized': True}})
            if info is not None:
                app.writestr('build-info.json', json.dumps({'component': 'bevy-explorer-web', **info}))
        return archive.getvalue()

    def __call__(self, path, raw=False):
        self.calls.append(path)
        if path == f'repos/{REPO}/pulls/7':
            return self.pr
        if match := re.fullmatch(rf'repos/{REPO}/actions/runs/([0-9]+)/artifacts\?per_page=100', path):
            names = self.artifacts.get(int(match.group(1)), [])
            rows = [{'id': self.ids.setdefault(name, len(self.ids) + 1), 'name': name,
                     'expired': name in self.expired} for name in names]
            return {'total_count': len(rows), 'artifacts': rows}
        if match := re.fullmatch(rf'repos/{REPO}/actions/artifacts/([0-9]+)/zip', path):
            if not raw:
                raise AssertionError('an artifact zip is read as bytes')
            return self.web_zip(next(name for name, number in self.ids.items() if number == int(match.group(1))))
        if path == f'repos/{REPO}/{inputs.RUNS_PATH}?head_sha={HEAD}&status=success&per_page=100':
            return {'workflow_runs': self.change_runs}
        if path == f'repos/{REPO}/{inputs.RUNS_PATH}?branch=main&event=push&status=success&per_page=100':
            return {'workflow_runs': self.main_runs}
        if match := re.fullmatch(rf'repos/{REPO}/{inputs.RUNS_PATH}\?head_sha=([0-9a-f]{{40}})&status=success&per_page=100', path):
            return {'workflow_runs': [run for run in self.main_runs if run['head_sha'] == match.group(1)]}
        raise AssertionError(f'unexpected read: {path}')

    def zips_read(self):
        return sorted(name for name, number in self.ids.items()
                      if f'repos/{REPO}/actions/artifacts/{number}/zip' in self.calls)


def completed(**changes):
    """The workflow_run event of the PR's passed CI run."""
    return {'workflow_run': ci_run(100, HEAD, **changes)}


class ResolverTest(unittest.TestCase):
    def setUp(self):
        self.github = GitHub()
        api = patch.object(inputs, 'api', side_effect=self.github)
        api.start()
        self.addCleanup(api.stop)

    def resolve(self, event_name='workflow_run', event=None, enabled='', runs=''):
        return inputs.resolve(event_name, completed() if event is None else event, REPO, enabled, runs)

    def assert_skipped(self, result):
        self.assertFalse(result['run'])
        self.assertEqual(result['notes'], '')
        self.assertFalse(result.get('legs'))

    def test_a_pr_with_a_passed_ci_run_runs_every_laptop_against_the_newest_main_build(self):
        result = self.resolve()
        self.assertTrue(result['run'])
        self.assertEqual((result['pr'], result['change_sha'], result['base_sha'], result['runs']),
                         ('7', HEAD, MAIN[0], '3'))
        self.assertEqual(result['expected_labels'],
                         'intel-i5-chrome,apple-m1-chrome,amd-ryzen-3-chrome,intel-i3-chrome,framework-13-i7-chrome')
        self.assertEqual(result['legs'][1], {
            'label': 'apple-m1-chrome', 'runs_on': 'apple-m1', 'os': 'macos',
            'runner_artifact': f'bevy-explorer-benchmark-macos-arm64-{MERGE}', 'runner_run_id': '100',
            'change_artifact': f'bevy-explorer-web-{MERGE}', 'change_run_id': '100',
            'base_artifact': f'bevy-explorer-web-{MAIN[0]}', 'base_run_id': '90'})
        self.assertEqual((result['report_runner_artifact'], result['report_runner_run_id']),
                         (f'bevy-explorer-benchmark-linux-amd64-{MERGE}', '100'))
        self.assertEqual(result['notes'], '')
        self.assertEqual(self.github.zips_read(), [f'bevy-explorer-web-{MERGE}', f'bevy-explorer-web-{MAIN[0]}'])

    def test_an_unoptimized_change_build_is_not_measured_and_the_pr_is_told_how_to_get_one(self):
        for info in ({'engine': {'optimized': False}}, {'engine': {}}, None):
            with self.subTest(info=info):
                self.github.build_info = {f'bevy-explorer-web-{MERGE}': info}
                result = self.resolve()
                self.assertFalse(result['run'])
                self.assertFalse(result['legs'])
                self.assertIn('change web build (CI run 100) is not optimized', result['notes'])
                self.assertIn('push again for a build it can compare', result['notes'])

    def test_an_unoptimized_base_build_is_not_measured(self):
        self.github.build_info = {f'bevy-explorer-web-{MAIN[0]}': {'engine': {'optimized': False}}}
        result = self.resolve()
        self.assertFalse(result['run'])
        self.assertFalse(result['legs'])
        self.assertIn(f'base web build ({MAIN[0]}, CI run 90) is not optimized', result['notes'])

    def test_fork_pr(self):
        self.assert_skipped(self.resolve(event=completed(head_repository={'full_name': 'fork/project'},
                                                         pull_requests=[])))
        self.github.pr['head']['repo'] = {'full_name': 'fork/project'}
        self.assert_skipped(self.resolve('workflow_dispatch', {'inputs': {'pr': '7'}}))

    def test_failed_ci_run(self):
        for conclusion in ('failure', 'cancelled'):
            with self.subTest(conclusion=conclusion):
                self.assert_skipped(self.resolve(event=completed(conclusion=conclusion)))
        self.assertEqual(self.github.calls, [])

    def test_a_ci_run_of_an_older_head_leaves_the_benchmark_to_the_newer_one(self):
        self.github.pr['head']['sha'] = 'f' * 40
        self.assert_skipped(self.resolve())

    def test_a_ci_run_that_built_no_web_app_is_not_an_explorer_change_and_starts_nothing(self):
        self.github.artifacts[100] = artifacts(MERGE, web=False)
        result = self.resolve()
        self.assert_skipped(result)
        self.assertIn('built no web app', result['reason'])

    def test_a_manual_run_without_a_web_app_is_told(self):
        for expired in (False, True):
            with self.subTest(expired=expired):
                self.github.artifacts[100] = artifacts(MERGE, web=expired)
                self.github.expired = {f'bevy-explorer-web-{MERGE}'} if expired else set()
                result = self.resolve('workflow_dispatch', {'inputs': {'pr': '7'}})
                self.assertFalse(result['run'])
                self.assertEqual(result['pr'], '7')
                self.assertIn('has no web app', result['notes'])

    def test_a_manual_run_without_a_passed_ci_run_says_so(self):
        self.github.change_runs = []
        result = self.resolve('workflow_dispatch', {'inputs': {'pr': '7'}})
        self.assertFalse(result['run'])
        self.assertIn('no successful Bevy Explorer / CI run', result['notes'])

    def test_base_artifact_expired(self):
        self.github.expired = {f'bevy-explorer-web-{MAIN[0]}'}
        self.github.artifacts[89] = artifacts(MAIN[1], web=False)
        result = self.resolve()
        self.assertTrue(result['run'])
        self.assertEqual(result['base_sha'], MAIN[2])
        self.assertEqual({(leg['base_artifact'], leg['base_run_id']) for leg in result['legs']},
                         {(f'bevy-explorer-web-{MAIN[2]}', '88')})

    def test_no_main_run_still_has_a_web_app(self):
        self.github.expired = {f'bevy-explorer-web-{sha}' for sha in MAIN}
        result = self.resolve()
        self.assertFalse(result['run'])
        self.assertIn('web app to compare with', result['notes'])

    def test_enabled_runners(self):
        result = self.resolve(enabled='intel-i5,apple-m1')
        self.assertEqual([leg['label'] for leg in result['legs']], ['intel-i5-chrome', 'apple-m1-chrome'])
        self.assertEqual(result['expected_labels'], 'intel-i5-chrome,apple-m1-chrome')
        self.assertFalse(self.resolve(enabled='nothing-online')['run'])

    def test_a_runner_the_change_lacks_comes_from_main_and_a_missing_one_drops_its_laptops(self):
        self.github.artifacts[100] = artifacts(MERGE, runners=('linux-amd64',))
        for run_id in (90, 89, 88):
            self.github.artifacts[run_id] = [name for name in self.github.artifacts[run_id] if 'windows' not in name]
        result = self.resolve()
        self.assertTrue(result['run'])
        self.assertEqual([leg['label'] for leg in result['legs']], ['apple-m1-chrome'])
        self.assertEqual((result['legs'][0]['runner_artifact'], result['legs'][0]['runner_run_id']),
                         (f'bevy-explorer-benchmark-macos-arm64-{MAIN[0]}', '90'))
        self.assertEqual(result['report_runner_run_id'], '100')
        self.assertIn('intel-i5-chrome did not run', result['notes'])

    def test_no_linux_runner_means_no_report(self):
        for run_id in (100, 90, 89, 88):
            self.github.artifacts[run_id] = [name for name in self.github.artifacts[run_id] if 'linux-amd64' not in name]
        result = self.resolve()
        self.assertFalse(result['run'])
        self.assertIn('linux-amd64', result['notes'])

    def test_a_dispatch_takes_the_pr_as_a_number_or_url(self):
        for pr in ('7', f'https://github.com/{REPO}/pull/7', f'https://github.com/{REPO}/pull/7/files'):
            with self.subTest(pr=pr):
                self.assertTrue(self.resolve('workflow_dispatch', {'inputs': {'pr': pr}})['run'])
        for pr in ('', 'seven', 'https://github.com/other/project/pull/7', '7; echo'):
            with self.subTest(pr=pr), self.assertRaises(ValueError):
                self.resolve('workflow_dispatch', {'inputs': {'pr': pr}})

    def test_a_dispatch_pins_the_base_commit_and_the_runs(self):
        event = {'inputs': {'pr': '7', 'base_sha': MAIN[1], 'runs': '2'}}
        self.github.artifacts[89] = artifacts(MAIN[1])
        result = self.resolve('workflow_dispatch', event)
        self.assertEqual((result['base_sha'], result['runs'], result['legs'][0]['base_run_id']), (MAIN[1], '2', '89'))
        for bad in ({'base_sha': 'main'}, {'change_sha': 'abc'}, {'runs': '4'}, {'runs': '0'}, {'runs': 'x'}):
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                self.resolve('workflow_dispatch', {'inputs': {'pr': '7', **bad}})

    def test_the_runs_stay_within_the_legs_hour_and_reach_the_report_as_a_plain_integer(self):
        self.assertEqual(inputs.MAX_RUNS, 3)
        for given, expected in (('2', '2'), (' 2 ', '2'), ('02', '2'), ('', '3')):
            with self.subTest(given=given):
                self.assertEqual(self.resolve(runs=given)['runs'], expected)
        for bad in ('4', '2.0', '-1'):
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                self.resolve(runs=bad)


class OutputTest(unittest.TestCase):
    def test_the_step_writes_every_output_the_workflow_reads_on_one_line_each(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            (folder / 'event.json').write_text(json.dumps(completed()))
            env = {'GH_REPO': REPO, 'GITHUB_EVENT_NAME': 'workflow_run',
                   'GITHUB_EVENT_PATH': str(folder / 'event.json'), 'GITHUB_OUTPUT': str(folder / 'output'),
                   'GITHUB_STEP_SUMMARY': str(folder / 'summary'), 'BENCHMARK_ENABLED_RUNNERS': 'apple-m1',
                   'BENCHMARK_RUNS': ''}
            with patch.dict(os.environ, env), patch.object(inputs, 'api', side_effect=GitHub()), \
                    patch('builtins.print'):
                self.assertEqual(inputs.main(), 0)
            values = dict(line.split('=', 1) for line in (folder / 'output').read_text().splitlines())
            self.assertIn('Benchmark runs', (folder / 'summary').read_text())
        for name in ('run', 'pr', 'change_sha', 'base_sha', 'runs', 'legs', 'expected_labels',
                     'report_runner_artifact', 'report_runner_run_id', 'notes'):
            self.assertIn(name, values)
        self.assertEqual(values['run'], 'true')
        self.assertEqual([leg['runs_on'] for leg in json.loads(values['legs'])], ['apple-m1'])

    def test_an_invalid_input_fails_the_step_with_its_message(self):
        with tempfile.TemporaryDirectory() as temporary:
            event = Path(temporary) / 'event.json'
            event.write_text(json.dumps({'inputs': {'pr': 'not a pr'}}))
            env = {'GH_REPO': REPO, 'GITHUB_EVENT_NAME': 'workflow_dispatch', 'GITHUB_EVENT_PATH': str(event),
                   'GITHUB_OUTPUT': '', 'GITHUB_STEP_SUMMARY': ''}
            with patch.dict(os.environ, env), patch.object(inputs, 'api', side_effect=GitHub()), \
                    patch('builtins.print') as printed:
                self.assertEqual(inputs.main(), 1)
            self.assertTrue(printed.call_args.args[0].startswith('::error::'))


if __name__ == '__main__':
    unittest.main()
