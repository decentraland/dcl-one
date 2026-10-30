import importlib.util
import unittest
from pathlib import Path

_spec = importlib.util.spec_from_file_location('bevy_e2e_inputs', Path(__file__).resolve().parents[1] / 'bevy-e2e-inputs.py')
inputs = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(inputs)

REPO = 'example/dcl-one'
WORKFLOW = '.github/workflows/bevy-explorer-ci.yml'
SHA_PR = 'a' * 40
SHA_MAIN = 'b' * 40
SHA_OLD = 'c' * 40


def run(run_id, sha, event, branch='main'):
    return {'id': run_id, 'path': WORKFLOW, 'status': 'completed', 'conclusion': 'success', 'event': event,
            'head_sha': sha, 'head_branch': branch, 'head_repository': {'full_name': REPO}}


def artifacts(*names):
    return {'total_count': len(names), 'artifacts': [{'name': name, 'id': index + 1, 'expired': False}
                                                     for index, name in enumerate(names)]}


class FakeApi:
    def __init__(self, responses):
        self.responses = responses
        self.calls = []

    def __call__(self, path, raw=False):
        self.calls.append(path)
        for prefix, response in self.responses.items():
            if path.startswith(prefix):
                return response
        raise AssertionError(f'unexpected API call {path}')


class ResolveTests(unittest.TestCase):
    def setUp(self):
        self.main_runs = {'workflow_runs': [run(20, SHA_MAIN, 'push'), run(10, SHA_OLD, 'push')]}
        self.api = FakeApi({
            f'repos/{REPO}/actions/runs/30/artifacts': artifacts(f'bevy-explorer-web-{SHA_PR}', 'other'),
            f'repos/{REPO}/actions/runs/31/artifacts': artifacts('other'),
            f'repos/{REPO}/actions/runs/20/artifacts': artifacts(f'bevy-explorer-web-{SHA_MAIN}'),
            f'repos/{REPO}/actions/runs/10/artifacts': artifacts(f'bevy-explorer-web-{SHA_OLD}'),
            f'repos/{REPO}/actions/workflows/bevy-explorer-ci.yml/runs?branch=main': self.main_runs,
            f'repos/{REPO}/actions/workflows/bevy-explorer-ci.yml/runs?head_sha={SHA_OLD}': {'workflow_runs': [run(10, SHA_OLD, 'push')]},
            f'repos/{REPO}/actions/workflows/bevy-explorer-ci.yml/runs?head_sha={SHA_PR}': {'workflow_runs': []},
            f'repos/{REPO}/pulls/194/files': [{'filename': 'services/catalyst/src/main.rs'}, {'filename': 'apps/bevy-explorer/e2e/specs/chat.test.ts'}],
            f'repos/{REPO}/pulls/195/files': [{'filename': 'services/catalyst/src/main.rs'}, {'filename': 'apps/sites/package.json'}],
        })
        inputs.bench.api = self.api

    def workflow_run(self, run_id, sha, event, pulls=(), repo=REPO, conclusion='success', branch='main'):
        return {'workflow_run': {'id': run_id, 'event': event, 'conclusion': conclusion, 'head_sha': sha,
                                 'head_branch': branch, 'head_repository': {'full_name': repo},
                                 'pull_requests': [{'number': number} for number in pulls]}}

    def test_a_pull_request_run_with_a_web_app_tests_its_own(self):
        result = inputs.resolve('workflow_run', self.workflow_run(30, SHA_PR, 'pull_request', pulls=(194,)), REPO)
        self.assertEqual(result['run'], True)
        self.assertEqual(result['sha'], SHA_PR)
        self.assertEqual(result['web_artifact'], f'bevy-explorer-web-{SHA_PR}')
        self.assertEqual(result['web_run_id'], '30')
        self.assertEqual(result['pr'], '194')

    def test_a_pull_request_run_without_a_web_app_takes_mains_newest_when_the_suite_changed(self):
        result = inputs.resolve('workflow_run', self.workflow_run(31, SHA_PR, 'pull_request', pulls=(194,)), REPO)
        self.assertEqual(result['run'], True)
        self.assertEqual(result['sha'], SHA_PR)
        self.assertEqual(result['web_artifact'], f'bevy-explorer-web-{SHA_MAIN}')
        self.assertEqual(result['web_run_id'], '20')
        self.assertIn('no web input changed', result['reason'])
        self.assertIn(f'repos/{REPO}/pulls/194/files?per_page=100&page=1', self.api.calls)

    def test_a_pull_request_that_changes_neither_the_web_app_nor_the_suite_is_skipped(self):
        result = inputs.resolve('workflow_run', self.workflow_run(31, SHA_PR, 'pull_request', pulls=(195,)), REPO)
        self.assertEqual(result['run'], False)
        self.assertIn('neither', result['reason'])
        self.assertNotIn(f'repos/{REPO}/actions/workflows/bevy-explorer-ci.yml/runs?branch=main', self.api.calls)

    def test_changes_suite_reads_every_page_of_files(self):
        pages = {
            f'repos/{REPO}/pulls/7/files?per_page=100&page=1': [{'filename': f'services/catalyst/src/{index}.rs'} for index in range(100)],
            f'repos/{REPO}/pulls/7/files?per_page=100&page=2': [{'filename': 'tooling/bevy-e2e-report.py'}],
        }
        inputs.bench.api = lambda path, raw=False: pages[path]
        self.assertEqual(inputs.changes_suite(REPO, 7), True)
        pages[f'repos/{REPO}/pulls/7/files?per_page=100&page=2'] = [{'filename': 'tooling/bevy-ci.py'}]
        self.assertEqual(inputs.changes_suite(REPO, 7), False)

    def test_a_main_push_run_tests_its_own_and_skips_without_one(self):
        result = inputs.resolve('workflow_run', self.workflow_run(20, SHA_MAIN, 'push'), REPO)
        self.assertEqual((result['run'], result['pr'], result['web_run_id']), (True, '', '20'))
        result = inputs.resolve('workflow_run', self.workflow_run(31, SHA_MAIN, 'push'), REPO)
        self.assertEqual(result['run'], False)
        self.assertIn('built no web app', result['reason'])

    def test_other_runs_are_skipped(self):
        for event in (self.workflow_run(30, SHA_PR, 'pull_request', pulls=(1,), repo='someone/fork'),
                      self.workflow_run(30, SHA_PR, 'pull_request', pulls=(1,), conclusion='failure'),
                      self.workflow_run(30, SHA_PR, 'pull_request'),
                      self.workflow_run(30, SHA_PR, 'push', branch='feature'),
                      self.workflow_run(30, SHA_PR, 'workflow_dispatch')):
            with self.subTest(event=event['workflow_run']):
                self.assertEqual(inputs.resolve('workflow_run', event, REPO)['run'], False)
        self.assertEqual(inputs.resolve('pull_request', {}, REPO)['run'], False)

    def test_the_schedule_takes_mains_newest(self):
        result = inputs.resolve('schedule', {}, REPO)
        self.assertEqual((result['run'], result['sha'], result['web_run_id'], result['pr']), (True, SHA_MAIN, '20', ''))

    def test_a_manual_run_takes_the_named_commit(self):
        result = inputs.resolve('workflow_dispatch', {'inputs': {'sha': SHA_OLD}}, REPO)
        self.assertEqual((result['run'], result['sha'], result['web_run_id']), (True, SHA_OLD, '10'))
        self.assertEqual(inputs.resolve('workflow_dispatch', {'inputs': {'sha': SHA_PR}}, REPO)['run'], False)
        with self.assertRaises(ValueError):
            inputs.resolve('workflow_dispatch', {'inputs': {'sha': 'abc'}}, REPO)
        result = inputs.resolve('workflow_dispatch', {'inputs': {'sha': ''}}, REPO)
        self.assertEqual(result['sha'], SHA_MAIN)


if __name__ == '__main__':
    unittest.main()
