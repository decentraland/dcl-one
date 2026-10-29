"""A delayed build cannot move mutable publication tags behind a newer push."""
import importlib.util
import os
from pathlib import Path
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('publication', Path(__file__).parents[1] / 'ci-publication.py')
publication = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(publication)


class PublicationTests(unittest.TestCase):
    def setUp(self):
        self.env = patch.dict(os.environ, {'GITHUB_EVENT_NAME': 'push', 'GITHUB_REF': 'refs/heads/main',
            'GITHUB_RUN_ID': '42', 'GITHUB_SHA': 'a' * 40})
        self.env.start()
        self.addCleanup(self.env.stop)
        self.run = {'workflow_id': 1, 'run_number': 10, 'head_sha': 'a' * 40,
                    'event': 'push', 'head_branch': 'main'}

    def test_latest_triggered_run_is_publishable_without_requiring_branch_head(self):
        with patch.object(publication, 'api', side_effect=[self.run, {'workflow_runs': [self.run]}]):
            self.assertTrue(publication.current())

    def test_newer_failed_or_incomplete_run_still_supersedes_an_old_run(self):
        for conclusion in ('success', 'failure', None):
            newer = {**self.run, 'run_number': 11, 'conclusion': conclusion}
            with self.subTest(conclusion=conclusion), patch.object(publication, 'api',
                    side_effect=[self.run, {'workflow_runs': [self.run, newer]}]):
                self.assertFalse(publication.current())

    def test_rerunning_old_run_does_not_make_it_current(self):
        with patch.object(publication, 'api', side_effect=[{**self.run, 'run_attempt': 3},
                {'workflow_runs': [{**self.run, 'run_number': 11}]}]):
            self.assertFalse(publication.current())

    def test_manual_and_tag_releases_are_explicit(self):
        for env in ({'GITHUB_EVENT_NAME': 'workflow_dispatch'}, {'GITHUB_REF': 'refs/tags/v1.0.0'}):
            with patch.dict(os.environ, env), patch.object(publication, 'api') as api:
                self.assertTrue(publication.current())
            api.assert_not_called()

    def test_callback_uses_the_source_run_not_the_callback_sha(self):
        run = {**self.run, 'status': 'completed', 'conclusion': 'success', 'run_attempt': 1,
               'path': '.github/workflows/bevy-explorer-ci.yml',
               'head_repository': {'full_name': 'example/repo'}}
        env = {'GITHUB_EVENT_NAME': 'workflow_run', 'GITHUB_SHA': 'b' * 40,
               'GITHUB_REPOSITORY': 'example/repo', 'SOURCE_RUN': '42', 'SOURCE_SHA': 'a' * 40,
               'SOURCE_ATTEMPT': '1', 'SOURCE_CONSUMER': 'headless'}
        with patch.dict(os.environ, env):
            with patch.object(publication, 'api', side_effect=[run, {'workflow_runs': [run]}]) as api:
                self.assertTrue(publication.current())
                self.assertEqual(api.call_args_list[0].args, ('actions/runs/42',))
            for changed in [False, True]:
                with patch.object(publication, 'api', side_effect=[run, {'workflow_runs': [run,
                        {**run, 'run_number': 11, 'head_sha': 'c' * 40}]}]), \
                        patch.object(publication, 'consumer_changed', return_value=changed):
                    self.assertEqual(publication.current(), not changed)
            for bad in ({**run, 'run_attempt': 2}, {**run, 'conclusion': 'failure'},
                        {**run, 'head_repository': {'full_name': 'fork/repo'}},
                        {**run, 'path': '.github/workflows/untrusted.yml'}):
                with patch.object(publication, 'api', return_value=bad), self.assertRaises(ValueError):
                    publication.current()

    def test_identity_or_api_errors_fail_closed(self):
        for responses in ([{**self.run, 'head_sha': 'b' * 40}], [self.run, {'workflow_runs': []}]):
            with patch.object(publication, 'api', side_effect=responses), self.assertRaises(ValueError):
                publication.current()
        with patch.object(publication, 'api', side_effect=RuntimeError('unavailable')), self.assertRaises(RuntimeError):
            publication.current()


if __name__ == '__main__':
    unittest.main()
