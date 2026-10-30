import importlib.util
import json
import os
import tempfile
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
SHA_NEW = 'd' * 40


def run(run_id, sha, event, branch='main', status='completed', conclusion='success'):
    return {'id': run_id, 'path': WORKFLOW, 'status': status, 'conclusion': conclusion, 'event': event,
            'head_sha': sha, 'head_branch': branch, 'head_repository': {'full_name': REPO}}


def artifacts(*names, expired=()):
    return {'total_count': len(names), 'artifacts': [{'name': name, 'id': index + 1, 'expired': name in expired}
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


def pull_request(number, sha, action='labeled', labels=(inputs.RUN_LABEL,), label=inputs.RUN_LABEL, draft=False, repo=REPO):
    """A pull_request event: by default, the maintainer's `labeled` event for the run label."""
    event = {'action': action,
             'pull_request': {'number': number, 'draft': draft, 'head': {'sha': sha, 'repo': {'full_name': repo}},
                              'labels': [{'name': name} for name in labels]}}
    if action == 'labeled':
        event['label'] = {'name': label}
    return event


class ResolveTests(unittest.TestCase):
    def setUp(self):
        self.main_runs = {'workflow_runs': [run(20, SHA_MAIN, 'push'), run(10, SHA_OLD, 'push')]}
        self.api = FakeApi({
            f'repos/{REPO}/actions/runs/30/artifacts': artifacts(f'bevy-explorer-web-{SHA_PR}', 'other'),
            f'repos/{REPO}/actions/runs/31/artifacts': artifacts('other'),
            f'repos/{REPO}/actions/runs/32/artifacts': artifacts(f'bevy-explorer-web-{SHA_NEW}', 'other', expired=(f'bevy-explorer-web-{SHA_NEW}',)),
            f'repos/{REPO}/actions/workflows/bevy-explorer-ci.yml/runs?head_sha={SHA_NEW}&status=success': {'workflow_runs': [run(32, SHA_NEW, 'pull_request')]},
            f'repos/{REPO}/actions/runs/20/artifacts': artifacts(f'bevy-explorer-web-{SHA_MAIN}'),
            f'repos/{REPO}/actions/runs/10/artifacts': artifacts(f'bevy-explorer-web-{SHA_OLD}'),
            f'repos/{REPO}/actions/workflows/bevy-explorer-ci.yml/runs?branch=main': self.main_runs,
            f'repos/{REPO}/actions/workflows/bevy-explorer-ci.yml/runs?head_sha={SHA_OLD}': {'workflow_runs': [run(10, SHA_OLD, 'push')]},
            f'repos/{REPO}/actions/workflows/bevy-explorer-ci.yml/runs?head_sha={SHA_PR}&status=success': {'workflow_runs': []},
            f'repos/{REPO}/pulls/194/files': [{'filename': 'services/catalyst/src/main.rs'}, {'filename': 'apps/bevy-explorer/e2e/specs/chat.test.ts'}],
            f'repos/{REPO}/pulls/195/files': [{'filename': 'services/catalyst/src/main.rs'}, {'filename': 'apps/sites/package.json'}],
        })
        inputs.bench.api = self.api
        self.waits = []
        self.deleted = []
        inputs.api_delete = self.deleted.append

    def wait(self, result):
        """A `wait` that answers at once: a run, None (no CI run started) or a StillRunning."""
        def wait_(repo, sha):
            self.waits.append((repo, sha))
            if isinstance(result, Exception):
                raise result
            return result
        return wait_

    def workflow_run(self, run_id, sha, event, pulls=(), repo=REPO, conclusion='success', branch='main'):
        return {'workflow_run': {'id': run_id, 'event': event, 'conclusion': conclusion, 'head_sha': sha,
                                 'head_branch': branch, 'head_repository': {'full_name': repo},
                                 'pull_requests': [{'number': number} for number in pulls]}}

    # --- pull requests, from their own event ---

    def test_a_labelled_pull_request_tests_the_web_app_its_ci_run_built(self):
        result = inputs.resolve('pull_request', pull_request(194, SHA_PR), REPO, wait=self.wait(run(30, SHA_PR, 'pull_request')))
        self.assertEqual(result['run'], True)
        self.assertEqual((result['sha'], result['web_artifact'], result['web_run_id'], result['pr']), (SHA_PR, f'bevy-explorer-web-{SHA_PR}', '30', '194'))
        self.assertEqual(self.waits, [(REPO, SHA_PR)])

    def test_a_labelled_pull_request_whose_ci_built_no_web_app_takes_mains_newest_when_the_suite_changed(self):
        result = inputs.resolve('pull_request', pull_request(194, SHA_PR), REPO, wait=self.wait(run(31, SHA_PR, 'pull_request')))
        self.assertEqual(result['run'], True)
        self.assertEqual((result['sha'], result['web_artifact'], result['web_run_id']), (SHA_PR, f'bevy-explorer-web-{SHA_MAIN}', '20'))
        self.assertIn('no web input changed', result['reason'])
        self.assertIn(f'repos/{REPO}/pulls/194/files?per_page=100&page=1', self.api.calls)

    def test_a_labelled_pull_request_that_started_no_ci_run_takes_mains_newest_when_the_suite_changed(self):
        result = inputs.resolve('pull_request', pull_request(194, SHA_PR), REPO, wait=self.wait(None))
        self.assertEqual(result['run'], True)
        self.assertEqual(result['web_run_id'], '20')
        self.assertIn('started no CI run', result['reason'])

    def test_a_labelled_pull_request_that_changes_neither_the_web_app_nor_the_suite_is_skipped(self):
        result = inputs.resolve('pull_request', pull_request(195, SHA_PR), REPO, wait=self.wait(run(31, SHA_PR, 'pull_request')))
        self.assertEqual(result['run'], False)
        self.assertIn('neither', result['reason'])
        self.assertNotIn(f'repos/{REPO}/actions/workflows/bevy-explorer-ci.yml/runs?branch=main', self.api.calls)

    def test_a_push_to_a_labelled_pull_request_takes_the_label_off_and_runs_nothing(self):
        result = inputs.resolve('pull_request', pull_request(194, SHA_NEW, action='synchronize'), REPO, wait=self.wait(run(30, SHA_NEW, 'pull_request')))
        self.assertEqual(result['run'], False)
        self.assertIn('is taken off', result['reason'])
        self.assertIn(SHA_NEW, result['reason'])
        self.assertEqual(self.deleted, [f'repos/{REPO}/issues/194/labels/ci%3Ae2e'])
        self.assertEqual((self.waits, self.api.calls), ([], []))

    def test_a_push_to_an_unlabelled_pull_request_runs_nothing_and_touches_nothing(self):
        result = inputs.resolve('pull_request', pull_request(194, SHA_NEW, action='synchronize', labels=('explorer',)), REPO, wait=self.wait(run(30, SHA_NEW, 'pull_request')))
        self.assertEqual(result['run'], False)
        self.assertIn(f'`{inputs.RUN_LABEL}` label', result['reason'])
        self.assertEqual((self.deleted, self.waits, self.api.calls), ([], [], []))

    def test_other_events_run_nothing_even_with_the_label_on(self):
        for action in ('opened', 'reopened', 'ready_for_review'):
            with self.subTest(action=action):
                result = inputs.resolve('pull_request', pull_request(194, SHA_PR, action=action), REPO, wait=self.wait(run(30, SHA_PR, 'pull_request')))
                self.assertEqual(result['run'], False)
                self.assertIn(f'not on {action}', result['reason'])
        self.assertEqual((self.deleted, self.waits), ([], []))

    def test_the_labeled_event_runs_only_for_the_run_label(self):
        other = pull_request(194, SHA_PR, label='explorer')
        result = inputs.resolve('pull_request', other, REPO, wait=self.wait(run(30, SHA_PR, 'pull_request')))
        self.assertEqual(result['run'], False)
        self.assertIn('is not `', result['reason'])
        self.assertEqual(inputs.resolve('pull_request', pull_request(194, SHA_PR), REPO, wait=self.wait(run(30, SHA_PR, 'pull_request')))['run'], True)

    def test_an_expired_web_app_is_skipped_and_never_replaced_by_mains(self):
        result = inputs.resolve('pull_request', pull_request(194, SHA_NEW), REPO, wait=self.wait(run(32, SHA_NEW, 'pull_request')))
        self.assertEqual(result['run'], False)
        self.assertIn('has expired', result['reason'])
        self.assertNotIn(f'repos/{REPO}/pulls/194/files?per_page=100&page=1', self.api.calls)
        by_hand = inputs.resolve('workflow_dispatch', {'inputs': {'sha': SHA_NEW}}, REPO)
        self.assertEqual(by_hand['run'], False)
        self.assertIn('has expired', by_hand['reason'])

    def test_a_fork_or_a_draft_is_skipped(self):
        for event in (pull_request(194, SHA_PR, repo='someone/fork'), pull_request(194, SHA_PR, draft=True)):
            with self.subTest(event=event['pull_request']):
                self.assertEqual(inputs.resolve('pull_request', event, REPO, wait=self.wait(run(30, SHA_PR, 'pull_request')))['run'], False)
        self.assertEqual(self.waits, [])

    def test_a_failed_or_unfinished_ci_run_is_skipped_with_its_reason(self):
        failed = inputs.resolve('pull_request', pull_request(194, SHA_PR), REPO, wait=self.wait(run(30, SHA_PR, 'pull_request', conclusion='failure')))
        self.assertEqual(failed['run'], False)
        self.assertIn('ended failure', failed['reason'])
        late = inputs.resolve('pull_request', pull_request(194, SHA_PR), REPO, wait=self.wait(inputs.StillRunning(30)))
        self.assertEqual(late['run'], False)
        self.assertIn('CI run 30 has not completed', late['reason'])

    # --- waiting for the commit's CI run ---

    def test_wait_for_ci_polls_until_the_run_completes(self):
        states = [{'workflow_runs': []},
                  {'workflow_runs': [run(30, SHA_PR, 'pull_request', status='in_progress', conclusion=None)]},
                  {'workflow_runs': [run(30, SHA_PR, 'pull_request', status='in_progress', conclusion=None)]},
                  {'workflow_runs': [run(30, SHA_PR, 'pull_request')]}]
        inputs.bench.api = lambda path, raw=False: states.pop(0)
        clock = iter(range(0, 10_000, 30))
        slept = []
        found = inputs.wait_for_ci(REPO, SHA_PR, 600, poll_seconds=30, sleep=slept.append, clock=lambda: next(clock))
        self.assertEqual(found['id'], 30)
        self.assertEqual(slept, [30, 30, 30])

    def test_wait_for_ci_gives_up_on_a_commit_that_started_no_run(self):
        inputs.bench.api = lambda path, raw=False: {'workflow_runs': [run(40, SHA_PR, 'pull_request', branch='x') | {'head_sha': SHA_NEW}]}
        clock = iter(range(0, 10_000, 60))
        self.assertIsNone(inputs.wait_for_ci(REPO, SHA_PR, 600, sleep=lambda _: None, clock=lambda: next(clock)))

    def test_wait_for_ci_raises_past_the_wait_for_a_run_still_going(self):
        inputs.bench.api = lambda path, raw=False: {'workflow_runs': [run(30, SHA_PR, 'pull_request', status='in_progress', conclusion=None)]}
        clock = iter(range(0, 10_000, 100))
        with self.assertRaises(inputs.StillRunning) as caught:
            inputs.wait_for_ci(REPO, SHA_PR, 300, sleep=lambda _: None, clock=lambda: next(clock))
        self.assertEqual(caught.exception.args[0], 30)

    # --- main, nightly, by hand ---

    def test_a_main_push_run_tests_its_own_and_skips_without_one(self):
        result = inputs.resolve('workflow_run', self.workflow_run(20, SHA_MAIN, 'push'), REPO)
        self.assertEqual((result['run'], result['pr'], result['web_run_id']), (True, '', '20'))
        result = inputs.resolve('workflow_run', self.workflow_run(31, SHA_MAIN, 'push'), REPO)
        self.assertEqual(result['run'], False)
        self.assertIn('built no web app', result['reason'])

    def test_a_pull_requests_ci_run_is_skipped_as_a_workflow_run(self):
        result = inputs.resolve('workflow_run', self.workflow_run(30, SHA_PR, 'pull_request', pulls=(194,)), REPO)
        self.assertEqual(result['run'], False)
        self.assertIn('its own event', result['reason'])
        self.assertEqual(self.api.calls, [])

    def test_other_runs_are_skipped(self):
        for event in (self.workflow_run(30, SHA_PR, 'push', repo='someone/fork'),
                      self.workflow_run(30, SHA_MAIN, 'push', conclusion='failure'),
                      self.workflow_run(30, SHA_PR, 'push', branch='feature'),
                      self.workflow_run(30, SHA_PR, 'workflow_dispatch')):
            with self.subTest(event=event['workflow_run']):
                self.assertEqual(inputs.resolve('workflow_run', event, REPO)['run'], False)
        self.assertEqual(inputs.resolve('issue_comment', {}, REPO)['run'], False)

    def test_the_schedule_takes_mains_newest(self):
        result = inputs.resolve('schedule', {}, REPO)
        self.assertEqual((result['run'], result['sha'], result['web_run_id'], result['pr']), (True, SHA_MAIN, '20', ''))

    def test_a_manual_run_takes_the_named_commit(self):
        result = inputs.resolve('workflow_dispatch', {'inputs': {'sha': SHA_OLD}}, REPO)
        self.assertEqual((result['run'], result['sha'], result['web_run_id']), (True, SHA_OLD, '10'))
        self.assertIn('no successful', inputs.resolve('workflow_dispatch', {'inputs': {'sha': SHA_PR}}, REPO)['reason'])
        with self.assertRaises(ValueError):
            inputs.resolve('workflow_dispatch', {'inputs': {'sha': 'abc'}}, REPO)
        result = inputs.resolve('workflow_dispatch', {'inputs': {'sha': ''}}, REPO)
        self.assertEqual(result['sha'], SHA_MAIN)

    # --- the files and the summary ---

    def test_changes_suite_reads_every_page_of_files(self):
        pages = {
            f'repos/{REPO}/pulls/7/files?per_page=100&page=1': [{'filename': f'services/catalyst/src/{index}.rs'} for index in range(100)],
            f'repos/{REPO}/pulls/7/files?per_page=100&page=2': [{'filename': 'tooling/bevy-e2e-report.py'}],
        }
        inputs.bench.api = lambda path, raw=False: pages[path]
        self.assertEqual(inputs.changes_suite(REPO, 7), True)
        pages[f'repos/{REPO}/pulls/7/files?per_page=100&page=2'] = [{'filename': 'tooling/bevy-ci.py'}]
        self.assertEqual(inputs.changes_suite(REPO, 7), False)

    def test_the_summary_names_the_code_under_test(self):
        with tempfile.NamedTemporaryFile('w+', suffix='.md', delete=False) as handle:
            path = handle.name
        try:
            os.environ['GITHUB_STEP_SUMMARY'] = path
            inputs.write_outputs(inputs.chosen(SHA_PR, 'web', 30, '194', 'the reason'))
            inputs.write_outputs(inputs.skip('no label'))
        finally:
            del os.environ['GITHUB_STEP_SUMMARY']
        text = Path(path).read_text(encoding='utf-8')
        os.unlink(path)
        self.assertIn(f'Code under test: pull request #194, commit {SHA_PR}.', text)
        self.assertEqual(text.count('Code under test'), 1)

    def test_suite_changed_writes_the_answer_for_the_checks_job(self):
        with tempfile.TemporaryDirectory() as folder:
            event_path = Path(folder) / 'event.json'
            event_path.write_text(json.dumps(pull_request(194, SHA_PR, labels=())), encoding='utf-8')
            output_path = Path(folder) / 'output.txt'
            os.environ.update({'GITHUB_EVENT_PATH': str(event_path), 'GH_REPO': REPO, 'GITHUB_OUTPUT': str(output_path)})
            try:
                self.assertEqual(inputs.main(['bevy-e2e-inputs.py', 'suite-changed']), 0)
                self.assertEqual(inputs.main(['bevy-e2e-inputs.py', 'nonsense']), 2)
            finally:
                for key in ('GITHUB_EVENT_PATH', 'GH_REPO', 'GITHUB_OUTPUT'):
                    del os.environ[key]
            self.assertEqual(output_path.read_text(encoding='utf-8'), 'changed=true\n')


if __name__ == '__main__':
    unittest.main()
