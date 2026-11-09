import importlib.util
import io
import json
from pathlib import Path
import tempfile
import subprocess
import os
import re
import shlex
from types import SimpleNamespace
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    'web_preview', Path(__file__).resolve().parents[1] / 'web-preview.py'
)
preview = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(preview)


def review_pr(number=9, base='main', base_sha='b' * 40):
    return {
        'number': number,
        'state': 'open',
        'base': {'ref': base, 'sha': base_sha},
        'head': {'sha': 'a' * 40, 'repo': {'full_name': 'example/one'}},
    }


def build_run(pr):
    return {
        'event': 'pull_request',
        'status': 'completed',
        'conclusion': 'success',
        'path': preview.WORKFLOW,
        'head_repository': {'full_name': 'example/one'},
        'head_sha': 'a' * 40,
        'pull_requests': [pr],
    }


def cdn_responses(attempt=1):
    identity = io.BytesIO(
        json.dumps({'revision': 'a' * 40, 'run': 123, 'attempt': attempt}).encode()
    )
    html = io.BytesIO(b'<html><script src="./assets/main-Abc123.js"></script></html>')
    html.status = 200
    html.headers = {'Content-Type': 'text/html'}
    asset = io.BytesIO(b'console.log("review")')
    asset.headers = {'Content-Type': 'application/javascript'}
    return [identity, html, asset]


def cdn_event():
    return {
        'deployment': {
            'environment': 'cdn',
            'task': 'upload-to-cdn',
            'payload': {
                'packageName': preview.PACKAGE,
                'packageVersion': '0.1.123-123.commit-abcdef0',
            },
        },
        'deployment_status': {'state': 'success'},
    }


def cdn_preview_url():
    return (
        preview.cdn_url(cdn_event()['deployment']['payload']['packageVersion'])
        + '/dcl-ui-review/index.html'
    )


def published_comment(url, extra=''):
    return {
        'id': 5,
        'user': {'login': 'github-actions[bot]'},
        'body': preview.MARKER
        + '\n<!-- verified-preview:123:2 -->\n'
        + f'[Preview]({url})\n'
        + extra,
    }


def workflow_jobs(workflow):
    return dict(re.findall(r'(?ms)^  (\w+):\n(.*?)(?=^  \w+:|\Z)', workflow.split('jobs:\n', 1)[1]))


class PreviewGitHub:
    """External GitHub boundary with real resolve/report code above it."""

    def __init__(self):
        self.pr = review_pr()
        self.run = {
            **build_run(self.pr),
            'run_attempt': 2,
            'html_url': 'https://github.com/example/one/actions/runs/123',
        }
        self.comments = []
        self.writes = []
        self.run_reads = 0
        self.supersede_on_read = None
        self.jobs = [{'name': 'UI reviewer', 'conclusion': 'success'}]

    def request(self, path, *, method='GET', body=None, **kwargs):
        if method in ('POST', 'PATCH'):
            self.writes.append(body['body'])
            self.comments[:] = [
                {'id': 5, 'user': {'login': 'github-actions[bot]'}, 'body': body['body']}
            ]
            return {}
        if path.endswith('/actions/runs/123'):
            self.run_reads += 1
            if self.run_reads == self.supersede_on_read:
                self.run = {**self.run, 'run_attempt': 3}
            return self.run
        if path.endswith('/pulls/9'):
            return self.pr
        if '/actions/runs/123/jobs?' in path:
            return {'jobs': self.jobs}
        if '/issues/9/comments?' in path:
            return self.comments
        if '/actions/workflows/web-previews.yml/runs?' in path:
            return [{'id': 123, 'pull_requests': [self.pr]}]
        raise AssertionError(f'Unexpected GitHub request: {method} {path}')


class WebPreviewTests(unittest.TestCase):
    def test_cdn_rejects_effective_asset_path_escapes_before_fetching(self):
        version = '0.1.123-123.commit-abcdef0'
        prefix = preview.cdn_url(version)
        for source in (
            './assets/%2e%2e/%2e%2e/%2e%2e/assets/main-Abc123.js',
            './assets/%2E%2E%2f%2E%2E%2fmain-Abc123.js',
            './assets/main%252e-Abc123.js',
            './assets/folder%5cmain-Abc123.js',
            './assets/folder\\main-Abc123.js',
            './assets/main%00-Abc123.js',
            './assets/main%0a-Abc123.js',
            './assets/main\n-Abc123.js',
            './assets/main%zz-Abc123.js',
            'https://evil.example/assets/main-Abc123.js',
            'https://cdn.decentraland.org@evil.example/assets/main-Abc123.js',
            prefix + '-other/assets/main-Abc123.js',
            prefix + '/index.html#/assets/main-Abc123.js',
            prefix + '/index.html?resource=/assets/main-Abc123.js',
        ):
            responses = cdn_responses()
            responses[1] = io.BytesIO(f'<script src="{source}"></script>'.encode())
            responses[1].status = 200
            responses[1].headers = {'Content-Type': 'text/html'}
            with (
                self.subTest(source=source),
                patch.object(preview, 'urlopen', side_effect=responses) as request,
                patch.object(preview.time, 'sleep') as sleep,
            ):
                with self.assertRaisesRegex(ValueError, 'asset'):
                    preview.verify_cdn(version, 'a' * 40, 123, timeout=0, attempt=1)
            self.assertEqual(request.call_count, 2)
            self.assertTrue(
                all(
                    call.args[0].full_url.startswith(prefix + '/')
                    for call in request.call_args_list
                )
            )
            sleep.assert_not_called()

    def test_cdn_preserves_legitimate_encoded_asset_filenames_inside_the_version(self):
        version = '0.1.123-123.commit-abcdef0'
        for name in (
            'UI%20bundle-Abc123.js',
            'caf%C3%A9-Abc123.js',
            'percent%25-Abc123.js',
            'hash%23name-Abc123.js',
            'query%3Fname-Abc123.js',
            'file%2ename-Abc123.js',
        ):
            responses = cdn_responses()
            responses[1] = io.BytesIO(f'<script src="./assets/{name}"></script>'.encode())
            responses[1].status = 200
            responses[1].headers = {'Content-Type': 'text/html'}
            with (
                self.subTest(name=name),
                patch.object(preview, 'urlopen', side_effect=responses) as request,
            ):
                self.assertTrue(
                    preview.verify_cdn(version, 'a' * 40, 123, timeout=0, attempt=1).endswith(
                        '/index.html'
                    )
                )
            self.assertEqual(
                request.call_args.args[0].full_url,
                preview.cdn_url(version) + '/dcl-ui-review/assets/' + name,
            )

    def test_old_source_run_preview_is_cleared_without_a_cdn_request(self):
        github = PreviewGitHub()
        old = preview.cdn_url('0.1.122-123.commit-abcdef0') + '/dcl-ui-review/index.html'
        github.comments = [published_comment(old)]
        with (
            patch.object(preview, 'api', side_effect=github.request),
            patch.object(preview, 'urlopen') as request,
        ):
            self.assertTrue(
                preview.report(
                    'example/one', 123, '', 'waiting for human approval', expected_attempt=2
                )
            )
        request.assert_not_called()
        self.assertNotIn('[Preview](', github.writes[0])
        self.assertIn('waiting for human approval', github.writes[0])

    def test_pending_approval_cli_fails_if_no_current_status_can_be_written(self):
        for condition in (
            'closed',
            'head',
            'attempt',
            'attempt_during_report',
            'newest',
            'no_products',
            'metadata',
        ):
            github = PreviewGitHub()
            if condition == 'closed':
                github.pr['state'] = 'closed'
            elif condition == 'head':
                github.pr['head']['sha'] = 'b' * 40
            elif condition == 'attempt':
                github.run['run_attempt'] = 3
            elif condition == 'attempt_during_report':
                github.supersede_on_read = 2
            elif condition == 'no_products':
                github.jobs = []
            elif condition == 'metadata':
                github.jobs = [{'name': 'changes', 'conclusion': 'skipped'}]

            def request(path, **kwargs):
                if condition == 'newest' and '/actions/workflows/web-previews.yml/runs?' in path:
                    return [{'id': 123}, {'id': 124, 'pull_requests': [github.pr]}]
                if condition == 'newest' and '/actions/runs/124/jobs?' in path:
                    return {'jobs': []}
                return github.request(path, **kwargs)

            with (
                self.subTest(condition=condition),
                patch.object(preview, 'api', side_effect=request),
                patch.object(preview.sys, 'argv', ['web-preview.py', 'report']),
                patch.dict(
                    os.environ,
                    {
                        'GITHUB_REPOSITORY': 'example/one',
                        'SOURCE_RUN_ID': '123',
                        'SOURCE_RUN_ATTEMPT': '2',
                        'PUBLICATION_RESULT': 'waiting for human approval',
                    },
                ),
            ):
                with self.assertRaisesRegex(ValueError, 'Pending approval status was not updated'):
                    preview.main()
            self.assertEqual(github.writes, [])

    def test_pending_approval_cli_succeeds_only_after_posting_the_current_status(self):
        github = PreviewGitHub()
        with (
            patch.object(preview, 'api', side_effect=github.request),
            patch.object(preview.sys, 'argv', ['web-preview.py', 'report']),
            patch.dict(
                os.environ,
                {
                    'GITHUB_REPOSITORY': 'example/one',
                    'SOURCE_RUN_ID': '123',
                    'SOURCE_RUN_ATTEMPT': '2',
                    'PUBLICATION_RESULT': 'waiting for human approval',
                },
            ),
        ):
            preview.main()
        self.assertEqual(len(github.writes), 1)
        self.assertIn('Publication: waiting for human approval', github.writes[0])

    def test_ordinary_report_cli_keeps_no_product_and_metadata_skips_successful(self):
        for jobs in ([], [{'name': 'changes', 'conclusion': 'skipped'}]):
            github = PreviewGitHub()
            github.jobs = jobs
            with (
                self.subTest(jobs=jobs),
                patch.object(preview, 'api', side_effect=github.request),
                patch.object(preview.sys, 'argv', ['web-preview.py', 'report']),
                patch.dict(
                    os.environ,
                    {
                        'GITHUB_REPOSITORY': 'example/one',
                        'SOURCE_RUN_ID': '123',
                        'SOURCE_RUN_ATTEMPT': '2',
                        'PUBLICATION_RESULT': 'not eligible for publication',
                    },
                ),
            ):
                preview.main()
            self.assertEqual(github.writes, [])

    def test_late_progress_rebuilds_a_receipt_verified_url_without_forged_extras(self):
        github = PreviewGitHub()
        url = cdn_preview_url()
        github.comments = [published_comment(url, '[Other preview](https://evil.example/)')]
        with (
            patch.object(preview, 'api', side_effect=github.request),
            patch.object(preview, 'urlopen', return_value=cdn_responses(2)[0]) as request,
        ):
            preview.report('example/one', 123, '', 'uploading', expected_attempt=2)
        self.assertEqual(len(github.writes), 1)
        self.assertIn(f'[Preview]({url})', github.writes[0])
        self.assertNotIn('evil.example', github.writes[0])
        self.assertEqual(
            request.call_args.args[0].full_url, url.rsplit('/', 2)[0] + '/preview-source.json'
        )

    def test_progress_rejects_external_or_noncanonical_preview_links_before_fetching(self):
        valid = cdn_preview_url()
        for url in (
            'https://evil.example/review',
            valid.replace(preview.PACKAGE, '@other/package'),
            valid.replace(cdn_event()['deployment']['payload']['packageVersion'], 'latest'),
            valid + '?external=1',
            valid + '#extra',
            valid.replace('https://', 'http://'),
            valid.replace('cdn.decentraland.org/', 'cdn.decentraland.org@evil.example/'),
        ):
            github = PreviewGitHub()
            github.comments = [published_comment(url)]
            with (
                self.subTest(url=url),
                patch.object(preview, 'api', side_effect=github.request),
                patch.object(preview, 'urlopen') as request,
            ):
                preview.report('example/one', 123, '', 'uploading', expected_attempt=2)
            request.assert_not_called()
            self.assertNotIn('[Preview](', github.writes[0])

    def test_missing_or_mismatched_prior_receipt_clears_the_unverified_preview(self):
        source = {'revision': 'a' * 40, 'run': 123, 'attempt': 2}
        for receipt in (
            {},
            [],
            None,
            {**source, 'revision': 'b' * 40},
            {**source, 'run': 124},
            {**source, 'attempt': 3},
            {**source, 'attempt': True},
            {**source, 'extra': 1},
        ):
            github = PreviewGitHub()
            github.comments = [published_comment(cdn_preview_url())]
            with (
                self.subTest(receipt=receipt),
                patch.object(preview, 'api', side_effect=github.request),
                patch.object(
                    preview, 'urlopen', return_value=io.BytesIO(json.dumps(receipt).encode())
                ),
            ):
                preview.report('example/one', 123, '', 'uploading', expected_attempt=2)
            self.assertNotIn('[Preview](', github.writes[0])
        for status in (404, 410):
            github = PreviewGitHub()
            github.comments = [published_comment(cdn_preview_url())]
            with (
                self.subTest(status=status),
                patch.object(preview, 'api', side_effect=github.request),
                patch.object(
                    preview,
                    'urlopen',
                    side_effect=preview.HTTPError(cdn_preview_url(), status, 'missing', {}, None),
                ),
            ):
                preview.report('example/one', 123, '', 'uploading', expected_attempt=2)
            self.assertNotIn('[Preview](', github.writes[0])

    def test_transient_prior_receipt_failure_leaves_the_comment_unchanged_and_fails(self):
        for error in (
            OSError('network unavailable'),
            preview.HTTPError(cdn_preview_url(), 503, 'unavailable', {}, None),
            preview.HTTPError(cdn_preview_url(), 429, 'limited', {}, None),
        ):
            github = PreviewGitHub()
            old = published_comment(cdn_preview_url())
            github.comments = [old]
            with (
                self.subTest(error=type(error).__name__),
                patch.object(preview, 'api', side_effect=github.request),
                patch.object(preview, 'urlopen', side_effect=error),
            ):
                with self.assertRaises(type(error)):
                    preview.report('example/one', 123, '', 'uploading', expected_attempt=2)
            self.assertEqual(github.writes, [])
            self.assertEqual(github.comments, [old])

    def test_malformed_prior_receipt_bytes_clear_the_obsolete_url_and_allow_current_status(self):
        for payload in (b'<html>not json</html>', b'{', b'\xff', b'"\xc3("'):
            for state in ('uploading', 'waiting for human approval'):
                github = PreviewGitHub()
                github.comments = [
                    published_comment(cdn_preview_url(), '[Other](https://evil.example/)')
                ]
                with (
                    self.subTest(payload=payload, state=state),
                    patch.object(preview, 'api', side_effect=github.request),
                    patch.object(preview, 'urlopen', return_value=io.BytesIO(payload)),
                ):
                    self.assertTrue(
                        preview.report('example/one', 123, '', state, expected_attempt=2)
                    )
                self.assertEqual(len(github.writes), 1)
                self.assertNotIn('[Preview](', github.writes[0])
                self.assertNotIn('evil.example', github.writes[0])

    def test_prior_comment_api_failure_leaves_the_comment_unchanged(self):
        github = PreviewGitHub()
        old = published_comment(cdn_preview_url())
        github.comments = [old]

        def request(path, **kwargs):
            if '/issues/9/comments?' in path:
                raise subprocess.CalledProcessError(1, ['gh'])
            return github.request(path, **kwargs)

        with (
            patch.object(preview, 'api', side_effect=request),
            patch.object(preview, 'urlopen') as fetch,
        ):
            with self.assertRaises(subprocess.CalledProcessError):
                preview.report(
                    'example/one', 123, '', 'waiting for human approval', expected_attempt=2
                )
        fetch.assert_not_called()
        self.assertEqual(github.writes, [])
        self.assertEqual(github.comments, [old])

    def test_new_completion_overwrites_forged_links_without_reusing_the_old_comment(self):
        github = PreviewGitHub()
        github.comments = [
            published_comment('https://evil.example/', '[Preview](https://other.example/)')
        ]
        with (
            patch.object(preview, 'api', side_effect=github.request),
            patch.object(preview, 'urlopen') as request,
        ):
            preview.report('example/one', 123, cdn_preview_url(), 'success', expected_attempt=2)
        request.assert_not_called()
        self.assertIn(f'[Preview]({cdn_preview_url()})', github.writes[0])
        self.assertNotIn('evil.example', github.writes[0])
        self.assertNotIn('other.example', github.writes[0])

    def test_progress_does_not_reuse_multiple_preview_links_or_hide_failure_diagnostics(self):
        for state, extra in (
            ('uploading', f'[Preview]({cdn_preview_url()})'),
            ('pending status update failed', ''),
            ('pending status update cancelled', ''),
            ('failure', ''),
        ):
            github = PreviewGitHub()
            github.comments = [published_comment(cdn_preview_url(), extra)]
            with (
                self.subTest(state=state),
                patch.object(preview, 'api', side_effect=github.request),
                patch.object(preview, 'urlopen') as request,
            ):
                preview.report('example/one', 123, '', state, expected_attempt=2)
            request.assert_not_called()
            self.assertNotIn('[Preview](', github.writes[0])
            if state != 'uploading':
                self.assertIn(state, github.writes[0])

    def test_generated_package_versions_distinguish_runs_at_the_same_oddish_timestamp(self):
        versions = []
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'input'
            (source / 'dcl-ui-review').mkdir(parents=True)
            (source / 'dcl-ui-review/index.html').write_text('<html></html>')
            for run_id in (123, 124, 9007199254740991):
                target = root / str(run_id)
                preview.package_reviewer(source, target, 'a' * 40, run_id, 1)
                version = json.loads((target / 'package.json').read_text())['version']
                self.assertEqual(version, f'0.1.{run_id}')
                snapshot = f'{version}-20261109160000.commit-abcdef0'
                self.assertTrue(preview.cdn_url(snapshot).endswith('/' + snapshot))
                versions.append(snapshot)
        self.assertEqual(len(set(versions)), 3)

    def test_invalid_semver_run_ids_cannot_write_a_package(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'input'
            (source / 'dcl-ui-review').mkdir(parents=True)
            (source / 'dcl-ui-review/index.html').write_text('<html></html>')
            for run_id in (None, True, 0, -1, 1.5, '123', 9007199254740992):
                target = root / 'output'
                with self.subTest(run_id=run_id), self.assertRaises(ValueError):
                    preview.package_reviewer(source, target, 'a' * 40, run_id, 1)
                self.assertFalse(target.exists())

    def test_cli_github_budget_is_shared_across_calls_and_restored_after_failure(self):
        clock = [0]
        requests = []
        sleeps = []

        def request(*args, **kwargs):
            requests.append(kwargs['timeout'])
            clock[0] += 5
            return (
                subprocess.CompletedProcess(['gh'], 1, '', 'HTTP 429: limited')
                if len(requests) % 2
                else subprocess.CompletedProcess(['gh'], 0, '{"ok":true}', '')
            )

        def sleep(delay):
            sleeps.append(delay)
            clock[0] += delay

        def command():
            for number in range(3):
                preview.api(f'repos/example/one/{number}')

        with (
            patch.object(preview, 'run_cli', side_effect=command),
            patch.object(preview.time, 'monotonic', side_effect=lambda: clock[0]),
            patch.object(preview.time, 'sleep', side_effect=sleep),
            patch.object(preview.subprocess, 'run', side_effect=request),
        ):
            with self.assertRaisesRegex(TimeoutError, 'GitHub API command budget'):
                preview.main()
        self.assertEqual(sleeps, [60, 60])
        self.assertEqual(requests, [30] * 5)
        self.assertLess(clock[0], 180)
        self.assertIsNone(preview.API_DEADLINE.get())

    def test_api_budget_requires_room_for_the_request_and_the_minimum_rate_limit_delay(self):
        failure = subprocess.CompletedProcess(['gh'], 1, '', 'HTTP 429: limited')
        for budget, calls in ((29, 0), (89, 1)):
            with (
                self.subTest(budget=budget),
                patch.object(preview.time, 'monotonic', return_value=0),
                patch.object(preview.time, 'sleep') as sleep,
                patch.object(preview.subprocess, 'run', return_value=failure) as request,
                preview.api_budget(budget),
            ):
                with self.assertRaisesRegex(TimeoutError, 'GitHub API command budget'):
                    preview.api('repos/example/one')
                self.assertEqual(request.call_count, calls)
                sleep.assert_not_called()

    def test_cancelled_or_timed_out_current_build_clears_an_older_preview(self):
        for conclusion in ('cancelled', 'timed_out'):
            for selection in ('cancelled', 'timed_out', None):
                github = PreviewGitHub()
                github.run['conclusion'] = conclusion
                github.jobs = (
                    [
                        {
                            'name': 'changes',
                            'conclusion': selection,
                            'html_url': 'https://github.com/example/one/actions/runs/123/job/1',
                        }
                    ]
                    if selection
                    else []
                )
                github.comments = [
                    {
                        'id': 5,
                        'user': {'login': 'github-actions[bot]'},
                        'body': preview.MARKER + '\n[Preview](https://cdn.example/old)',
                    }
                ]
                with (
                    self.subTest(conclusion=conclusion, selection=selection),
                    patch.object(preview, 'api', side_effect=github.request),
                ):
                    preview.report('example/one', 123, '', 'not published', expected_attempt=2)
                self.assertEqual(len(github.writes), 1)
                self.assertNotIn('https://cdn.example/old', github.writes[0])
                self.assertIn(
                    f'| Preview selection | {selection or conclusion} |', github.writes[0]
                )
                self.assertNotIn('not selected', github.writes[0])
                self.assertFalse(preview.publishable(github.run, github.pr, 'example/one'))

    def test_reporting_terminal_builds_keeps_source_identity_and_attempt_checks(self):
        for invalid in ('fork', 'bot', 'closed', 'old_head', 'attempt', 'running'):
            github = PreviewGitHub()
            github.run['conclusion'] = 'cancelled'
            if invalid == 'fork':
                github.run['head_repository'] = {'full_name': 'outside/one'}
            elif invalid == 'bot':
                github.run['actor'] = {'type': 'Bot', 'login': 'app[bot]'}
            elif invalid == 'closed':
                github.pr['state'] = 'closed'
            elif invalid == 'old_head':
                github.pr['head']['sha'] = 'b' * 40
            elif invalid == 'attempt':
                github.run['run_attempt'] = 3
            else:
                github.run['status'] = 'in_progress'
            with (
                self.subTest(invalid=invalid),
                patch.object(preview, 'api', side_effect=github.request),
            ):
                preview.report('example/one', 123, '', 'cancelled', expected_attempt=2)
            self.assertEqual(github.writes, [])

    def test_newer_meaningful_cancelled_or_timed_out_runs_supersede_older_builds(self):
        pr = review_pr()
        for conclusion in ('cancelled', 'timed_out'):
            for associated in (True, False):
                candidate = {
                    **build_run(pr),
                    'id': 124,
                    'conclusion': conclusion,
                    'pull_requests': [pr] if associated else [],
                }
                responses = [
                    [{'id': 123}, candidate],
                    {'jobs': [{'name': 'changes', 'conclusion': conclusion}]},
                ]
                if not associated:
                    responses += [candidate, [pr], pr]
                with (
                    self.subTest(conclusion=conclusion, associated=associated),
                    patch.object(preview, 'api', side_effect=responses),
                ):
                    self.assertFalse(
                        preview.newest_run_for_pr('example/one', 123, build_run(pr), pr)
                    )

    def test_cancelled_metadata_run_with_skipped_selection_does_not_supersede_a_build(self):
        pr = review_pr()
        runs = [
            {'id': 123},
            {'id': 124, 'conclusion': 'cancelled', 'pull_requests': [pr]},
        ]
        with patch.object(
            preview,
            'api',
            side_effect=[
                runs,
                {'jobs': [{'name': 'changes', 'conclusion': 'skipped'}]},
            ],
        ):
            self.assertTrue(preview.newest_run_for_pr('example/one', 123, build_run(pr), pr))

    def test_running_newer_build_without_pr_metadata_supersedes_the_older_build(self):
        pr = review_pr()
        candidate = {
            **build_run(pr),
            'id': 124,
            'status': 'in_progress',
            'conclusion': None,
            'pull_requests': [],
        }
        with patch.object(
            preview,
            'api',
            side_effect=[
                [{'id': 123}, candidate],
                {'jobs': [{'name': 'changes', 'conclusion': None}]},
                candidate,
                [pr],
                pr,
            ],
        ):
            self.assertFalse(preview.newest_run_for_pr('example/one', 123, build_run(pr), pr))
        for conclusion in (None, 'success', 'cancelled'):
            with self.subTest(conclusion=conclusion):
                running = {**candidate, 'conclusion': conclusion}
                self.assertFalse(preview.publishable(running, pr, 'example/one'))
                self.assertFalse(preview.reportable(running, pr, 'example/one'))

    def test_newest_run_fallback_matches_its_target_among_two_base_associations(self):
        pr = review_pr()
        other = review_pr(10, 'feature/other')
        candidate = {
            **build_run(pr),
            'id': 124,
            'status': 'in_progress',
            'conclusion': None,
            'pull_requests': [],
        }
        for jobs in ([], [{'name': 'changes', 'conclusion': None}]):
            for associations, expected in (
                ([other, pr], False),
                ([pr, other], False),
                ([pr, pr], False),
                ([other], True),
            ):
                responses = [
                    [{'id': 123}, candidate],
                    {'jobs': jobs},
                    candidate,
                    associations,
                ]
                responses += [pr for match in associations if match == pr]
                with (
                    self.subTest(jobs=jobs, associations=associations),
                    patch.object(preview, 'api', side_effect=responses) as request,
                ):
                    self.assertEqual(
                        preview.newest_run_for_pr('example/one', 123, build_run(pr), pr), expected
                    )
                self.assertFalse(
                    any(call.args[0].endswith('/pulls/10') for call in request.call_args_list)
                )

    def test_identity_only_fallback_keeps_source_and_current_pr_checks(self):
        pr = review_pr()
        candidate = {
            **build_run(pr),
            'id': 124,
            'status': 'in_progress',
            'conclusion': None,
            'pull_requests': [],
        }
        for invalid in ('fork', 'workflow', 'actor', 'author', 'closed', 'head', 'base'):
            run = dict(candidate)
            live = review_pr()
            if invalid == 'fork':
                run['head_repository'] = {'full_name': 'outside/one'}
            elif invalid == 'workflow':
                run['path'] = '.github/workflows/other.yml'
            elif invalid == 'actor':
                run['triggering_actor'] = {'type': 'Bot', 'login': 'app[bot]'}
            elif invalid == 'author':
                live['user'] = {'type': 'Bot', 'login': 'app[bot]'}
            elif invalid == 'closed':
                live['state'] = 'closed'
            elif invalid == 'head':
                live['head']['sha'] = 'b' * 40
            else:
                live['base']['ref'] = 'feature/other'
            responses = [
                [{'id': 123}, candidate],
                {'jobs': [{'name': 'changes', 'conclusion': None}]},
                run,
            ]
            if invalid not in ('fork', 'workflow'):
                responses += [[pr], live]
            with (
                self.subTest(invalid=invalid),
                patch.object(preview, 'api', side_effect=responses),
            ):
                self.assertTrue(preview.newest_run_for_pr('example/one', 123, build_run(pr), pr))

    def test_failed_newer_run_association_lookup_cannot_authorize_an_older_publication(self):
        pr = review_pr()
        candidate = {
            **build_run(pr),
            'id': 124,
            'status': 'in_progress',
            'conclusion': None,
            'pull_requests': [],
        }
        for successful_reads in ([], [candidate], [candidate, [pr]]):
            with (
                self.subTest(successful_reads=len(successful_reads)),
                patch.object(
                    preview,
                    'api',
                    side_effect=[
                        [{'id': 123}, candidate],
                        {'jobs': [{'name': 'changes', 'conclusion': None}]},
                        *successful_reads,
                        subprocess.CalledProcessError(1, ['gh']),
                    ],
                ),
            ):
                with self.assertRaises(subprocess.CalledProcessError):
                    preview.newest_run_for_pr('example/one', 123, build_run(pr), pr)

    def test_older_run_is_ineligible_before_approval_and_before_publication(self):
        for command in ('resolve', 'check-publication', 'package-reviewer'):
            github = PreviewGitHub()

            def request(path, **kwargs):
                if '/actions/workflows/web-previews.yml/runs?' in path:
                    return [{'id': 123}, {'id': 124, 'pull_requests': [github.pr]}]
                if '/actions/runs/124/jobs?' in path:
                    return {'jobs': [{'name': 'changes', 'conclusion': 'success'}]}
                return github.request(path, **kwargs)

            argv = ['web-preview.py', command]
            if command == 'package-reviewer':
                argv += ['source', 'target']
            with (
                self.subTest(command=command),
                patch.object(preview.sys, 'argv', argv),
                patch.dict(
                    os.environ,
                    {
                        'GITHUB_REPOSITORY': 'example/one',
                        'SOURCE_RUN_ID': '123',
                        'SOURCE_RUN_ATTEMPT': '2',
                    },
                ),
                patch.object(preview, 'api', side_effect=request),
                patch.object(preview, 'require_reviewer', return_value=True),
                patch.object(preview, 'require_publication_environment'),
                patch.object(preview, 'package_reviewer') as package,
                patch('sys.stdout', new_callable=io.StringIO) as stdout,
            ):
                if command == 'resolve':
                    preview.main()
                    self.assertIn('pr=\npublish=false\n', stdout.getvalue())
                else:
                    with self.assertRaisesRegex(ValueError, 'newer meaningful'):
                        preview.main()
                package.assert_not_called()

    def test_workflow_preflights_protections_before_scheduling_the_environment_job(self):
        workflow = (
            Path(__file__).parents[2] / '.github/workflows/web-previews-publish.yml'
        ).read_text()
        jobs = dict(
            re.findall(r'(?ms)^  (\w+):\n(.*?)(?=^  \w+:|\Z)', workflow.split('jobs:\n', 1)[1])
        )
        source = jobs['source']
        step = '      - name: Validate publication protections before approval\n'
        self.assertIn(step, source)
        preflight = source.split(step)[1]
        self.assertNotRegex(source, r'(?m)^    environment:')
        self.assertLess(
            source.index('web-preview.py resolve'), source.index('web-preview.py check-environment')
        )
        self.assertIn('GH_TOKEN: ${{ github.token }}', preflight)
        self.assertIn('run: python3 tooling/web-preview.py check-environment', preflight)
        self.assertNotIn('continue-on-error', preflight)
        guard = re.search(r'^        if: (.+)$', preflight, re.M).group(1)
        expression = guard.replace('&&', 'and')
        for enabled, publish, reviewer in (
            ('true', 'true', 'true'),
            ('false', 'true', 'true'),
            ('', 'true', 'true'),
            ('true', 'false', 'true'),
            ('true', 'true', 'false'),
        ):
            with self.subTest(enabled=enabled, publish=publish, reviewer=reviewer):
                context = {
                    'vars': SimpleNamespace(WEB_PREVIEW_REVIEWER_ENABLED=enabled),
                    'steps': SimpleNamespace(
                        source=SimpleNamespace(
                            outputs=SimpleNamespace(publish=publish, reviewer=reviewer)
                        )
                    ),
                }
                self.assertEqual(
                    bool(eval(expression, {'__builtins__': {}}, context)),
                    (enabled, publish, reviewer) == ('true', 'true', 'true'),
                )
        self.assertIn('    needs: source\n', jobs['pending'])
        self.assertIn('    needs: [source, pending]\n', jobs['reviewer'])
        guard = re.search(r'^    if: \$\{\{ (.+) \}\}$', jobs['reviewer'], re.M).group(1)
        expression = guard.replace('&&', 'and').replace('!cancelled()', 'not cancelled()')
        for result in ('success', 'failure', 'cancelled', 'skipped', ''):
            with self.subTest(preflight_result=result):
                context = {
                    'needs': SimpleNamespace(
                        source=SimpleNamespace(
                            result=result, outputs=SimpleNamespace(publish='true', reviewer='true')
                        ),
                        pending=SimpleNamespace(result='success'),
                    ),
                    'vars': SimpleNamespace(WEB_PREVIEW_REVIEWER_ENABLED='true'),
                    'cancelled': lambda: False,
                }
                self.assertEqual(
                    bool(eval(expression, {'__builtins__': {}}, context)), result == 'success'
                )

    def test_environment_preflight_cli_fails_closed_on_missing_denied_or_unavailable_api(self):
        for status in (404, 403, 500):
            response = subprocess.CompletedProcess(
                ['gh'], 1, '', f'gh: unavailable (HTTP {status})'
            )
            with (
                self.subTest(status=status),
                patch.object(preview.sys, 'argv', ['web-preview.py', 'check-environment']),
                patch.dict(os.environ, {'GITHUB_REPOSITORY': 'example/one'}),
                patch.object(preview.subprocess, 'run', return_value=response) as request,
                patch.object(preview.time, 'sleep'),
                patch('sys.stderr', new_callable=io.StringIO),
            ):
                with self.assertRaises(subprocess.CalledProcessError):
                    preview.main()
                for call in request.call_args_list:
                    self.assertIn('repos/example/one/environments/web-preview', call.args[0])
                    self.assertNotIn('POST', call.args[0])
                    self.assertNotIn('PATCH', call.args[0])

    def test_environment_preflight_cli_accepts_only_verified_protections(self):
        environment = {
            'can_admins_bypass': False,
            'protection_rules': [
                {
                    'type': 'required_reviewers',
                    'reviewers': [
                        {'type': 'User', 'reviewer': {'type': 'User', 'login': 'maintainer'}}
                    ],
                }
            ],
            'deployment_branch_policy': {
                'custom_branch_policies': True,
                'protected_branches': False,
            },
        }
        for candidate, valid in (
            (environment, True),
            ({}, False),
            ({**environment, 'can_admins_bypass': True}, False),
            ({**environment, 'protection_rules': []}, False),
            ({**environment, 'deployment_branch_policy': None}, False),
        ):
            responses = [
                candidate,
                {'default_branch': 'main'},
                {'total_count': 1, 'branch_policies': [{'name': 'main', 'type': 'branch'}]},
            ]
            with (
                self.subTest(environment=candidate),
                patch.object(preview.sys, 'argv', ['web-preview.py', 'check-environment']),
                patch.dict(os.environ, {'GITHUB_REPOSITORY': 'example/one'}),
                patch.object(preview, 'api', side_effect=responses),
            ):
                if valid:
                    preview.main()
                else:
                    with self.assertRaises(ValueError):
                        preview.main()

    def test_reviewer_approval_requires_a_successful_pending_comment(self):
        workflow = (
            Path(__file__).parents[2] / '.github/workflows/web-previews-publish.yml'
        ).read_text()
        jobs = dict(
            re.findall(r'(?ms)^  (\w+):\n(.*?)(?=^  \w+:|\Z)', workflow.split('jobs:\n', 1)[1])
        )
        guard = re.search(r'^    if: \$\{\{ (.+) \}\}$', jobs['reviewer'], re.M).group(1)
        expression = guard.replace('&&', 'and').replace('!cancelled()', 'not cancelled()')
        for result in ('success', 'failure', 'cancelled', 'skipped', ''):
            with self.subTest(pending=result):
                context = {
                    'needs': SimpleNamespace(
                        source=SimpleNamespace(
                            result='success',
                            outputs=SimpleNamespace(publish='true', reviewer='true'),
                        ),
                        pending=SimpleNamespace(result=result),
                    ),
                    'vars': SimpleNamespace(WEB_PREVIEW_REVIEWER_ENABLED='true'),
                    'cancelled': lambda: False,
                }
                self.assertEqual(
                    bool(eval(expression, {'__builtins__': {}}, context)), result == 'success'
                )

    def test_publication_checks_newest_run_after_artifact_and_environment_validation(self):
        pr = review_pr()
        run = {**build_run(pr), 'run_attempt': 2}
        for command in ('check-publication', 'package-reviewer'):
            order = []
            argv = ['web-preview.py', command]
            if command == 'package-reviewer':
                argv += ['source', 'target']
            with (
                self.subTest(command=command),
                patch.object(preview.sys, 'argv', argv),
                patch.dict(
                    os.environ,
                    {
                        'GITHUB_REPOSITORY': 'example/one',
                        'SOURCE_RUN_ID': '123',
                    },
                ),
                patch.object(preview, 'resolve', return_value=(run, pr)),
                patch.object(
                    preview,
                    'require_reviewer',
                    side_effect=lambda *_: order.append('artifact') or True,
                ),
                patch.object(
                    preview,
                    'require_publication_environment',
                    side_effect=lambda *_: order.append('environment'),
                ),
                patch.object(
                    preview,
                    'newest_run_for_pr',
                    side_effect=lambda *_: order.append('latest') or True,
                ),
                patch.object(
                    preview, 'package_reviewer', side_effect=lambda *_: order.append('package')
                ),
            ):
                preview.main()
            self.assertEqual(
                order,
                ['artifact', 'environment', 'latest']
                if command == 'check-publication'
                else ['artifact', 'latest', 'package'],
            )

    def test_unavailable_newest_run_check_cannot_write_a_package(self):
        pr = review_pr()
        with (
            patch.object(
                preview.sys, 'argv', ['web-preview.py', 'package-reviewer', 'source', 'target']
            ),
            patch.dict(
                os.environ,
                {
                    'GITHUB_REPOSITORY': 'example/one',
                    'SOURCE_RUN_ID': '123',
                },
            ),
            patch.object(preview, 'resolve', return_value=(build_run(pr), pr)),
            patch.object(preview, 'require_reviewer', return_value=True),
            patch.object(
                preview, 'newest_run_for_pr', side_effect=subprocess.CalledProcessError(1, ['gh'])
            ),
            patch.object(preview, 'package_reviewer') as package,
        ):
            with self.assertRaises(subprocess.CalledProcessError):
                preview.main()
            package.assert_not_called()

    def test_static_package_rejects_retired_comparison_outputs_without_writing_a_package(self):
        for name in (
            'references/reference.png',
            'renders/render.jpg',
            'fidelity.json',
            'icons/export.svg',
        ):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                source = root / 'input'
                (source / 'dcl-ui-review').mkdir(parents=True)
                (source / 'dcl-ui-review/index.html').write_text('<html></html>')
                evidence = source / 'dcl-ui-review' / name
                evidence.parent.mkdir(parents=True, exist_ok=True)
                evidence.write_text('retired comparison output')
                with self.assertRaisesRegex(ValueError, 'retired'):
                    preview.package_reviewer(source, root / 'output', 'a' * 40, 123, 1)
                self.assertFalse((root / 'output').exists())

    def test_static_package_keeps_actual_product_images_and_icons(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'input'
            assets = source / 'dcl-ui-review/assets'
            assets.mkdir(parents=True)
            (source / 'dcl-ui-review/index.html').write_text('<html></html>')
            for name, data in (('product.svg', b'<svg/>'), ('product.png', b'\x89PNG\r\n\x1a\n')):
                (assets / name).write_bytes(data)
            preview.package_reviewer(source, root / 'output', 'a' * 40, 123, 1)
            for name in ('product.svg', 'product.png'):
                self.assertEqual(
                    (root / 'output/dcl-ui-review/assets' / name).read_bytes(),
                    (assets / name).read_bytes(),
                )

    def test_cdn_readiness_needs_only_the_bound_receipt_entry_and_runtime_asset(self):
        responses = cdn_responses()[:3]
        with patch.object(preview, 'urlopen', side_effect=responses) as request:
            url = preview.verify_cdn(
                '0.1.0-123.commit-abcdef0', 'a' * 40, 123, timeout=0, attempt=1
            )
        self.assertTrue(url.endswith('/dcl-ui-review/index.html'))
        self.assertEqual(request.call_count, 3)

    def test_workflow_reports_successful_uploads_and_keeps_other_events_out(self):
        workflow = (
            Path(__file__).parents[2] / '.github/workflows/web-previews-publish.yml'
        ).read_text()
        jobs = dict(
            re.findall(r'(?ms)^  (\w+):\n(.*?)(?=^  \w+:|\Z)', workflow.split('jobs:\n', 1)[1])
        )
        job = jobs['report']
        guard = re.search(r'^    if: \$\{\{ (.+) \}\}$', job, re.M).group(1)
        result = re.search(r'^          PUBLICATION_RESULT: \$\{\{ (.+) \}\}$', job, re.M).group(1)
        for event, cancelled, enabled, source, pending, reviewer, eligible, expected in (
            ('pull_request', False, 'true', 'success', 'success', 'success', 'true', 'uploading'),
            ('pull_request', False, 'true', 'success', 'success', 'failure', 'true', 'failure'),
            (
                'pull_request',
                False,
                'true',
                'success',
                'success',
                'skipped',
                'true',
                'not eligible for publication',
            ),
            (
                'pull_request',
                False,
                'true',
                'success',
                'failure',
                'skipped',
                'true',
                'pending status update failed',
            ),
            (
                'pull_request',
                False,
                'true',
                'success',
                'cancelled',
                'skipped',
                'true',
                'pending status update cancelled',
            ),
            (
                'pull_request',
                False,
                'false',
                'success',
                'failure',
                'skipped',
                'true',
                'disabled pending CDN base verification',
            ),
            (
                'pull_request',
                False,
                'true',
                'failure',
                'failure',
                'skipped',
                'true',
                'source validation failed',
            ),
            (
                'pull_request',
                False,
                'true',
                'success',
                'failure',
                'skipped',
                'false',
                'not eligible for publication',
            ),
            ('', False, 'true', 'success', 'success', 'success', 'true', None),
            ('pull_request', True, 'true', 'success', 'success', 'success', 'true', None),
        ):
            with self.subTest(
                event=event,
                cancelled=cancelled,
                enabled=enabled,
                reviewer=reviewer,
                eligible=eligible,
            ):
                context = {
                    'github': SimpleNamespace(
                        event=SimpleNamespace(workflow_run=SimpleNamespace(event=event))
                    ),
                    'needs': SimpleNamespace(
                        source=SimpleNamespace(
                            result=source,
                            outputs=SimpleNamespace(
                                publish=eligible, publication_environment='success'
                            ),
                        ),
                        pending=SimpleNamespace(result=pending),
                        reviewer=SimpleNamespace(result=reviewer),
                    ),
                    'vars': SimpleNamespace(WEB_PREVIEW_REVIEWER_ENABLED=enabled),
                    'cancelled': lambda: cancelled,
                }

                def evaluate(expression):
                    expression = (
                        expression.replace('&&', 'and')
                        .replace('||', 'or')
                        .replace('!cancelled()', 'not cancelled()')
                    )
                    return eval(expression, {'__builtins__': {}}, context)

                if expected is None:
                    self.assertFalse(evaluate(guard))
                    continue
                self.assertTrue(evaluate(guard))
                state = evaluate(result)
                self.assertEqual(state, expected)
                if state == 'uploading':
                    github = PreviewGitHub()
                    with patch.object(preview, 'api', side_effect=github.request):
                        preview.report('example/one', 123, '', state, expected_attempt=2)
                    self.assertIn('Upload in progress', github.comments[0]['body'])
                    self.assertIn(
                        '/actions/workflows/web-previews-publish.yml', github.comments[0]['body']
                    )

    def test_environment_preflight_failure_has_distinct_publication_diagnostics(self):
        root = Path(__file__).parents[2] / '.github/workflows/web-previews-publish.yml'
        jobs = workflow_jobs(root.read_text())
        self.assertIn('publication_environment: ${{ steps.protections.outcome }}', jobs['source'])
        self.assertIn(
            '      - name: Validate publication protections before approval\n        id: protections\n',
            jobs['source'],
        )
        result = re.search(
            r'^          PUBLICATION_RESULT: \$\{\{ (.+) \}\}$', jobs['report'], re.M
        ).group(1)
        for outcome, enabled, expected in (
            ('failure', 'true', 'publication environment verification failed'),
            ('failure', 'false', 'disabled pending CDN base verification'),
            ('skipped', 'true', 'source validation failed'),
            ('', 'true', 'source validation failed'),
            ('success', 'true', 'source validation failed'),
        ):
            context = {
                'vars': SimpleNamespace(WEB_PREVIEW_REVIEWER_ENABLED=enabled),
                'needs': SimpleNamespace(
                    source=SimpleNamespace(
                        result='failure', outputs=SimpleNamespace(publication_environment=outcome)
                    )
                ),
            }
            with self.subTest(outcome=outcome, enabled=enabled):
                actual = eval(
                    result.replace('&&', 'and').replace('||', 'or'), {'__builtins__': {}}, context
                )
                self.assertEqual(actual, expected)
        github = PreviewGitHub()
        github.comments = [published_comment(cdn_preview_url())]
        with (
            patch.object(preview, 'api', side_effect=github.request),
            patch.object(preview, 'urlopen') as fetch,
        ):
            preview.report(
                'example/one',
                123,
                '',
                'publication environment verification failed',
                expected_attempt=2,
            )
        fetch.assert_not_called()
        self.assertNotIn('[Preview](', github.writes[0])
        self.assertIn('publication environment verification failed', github.writes[0])
        self.assertIn(
            'https://github.com/example/one/actions/workflows/web-previews-publish.yml',
            github.writes[0],
        )

    def test_publication_and_completion_use_the_workflow_commit_instead_of_a_moving_ref(self):
        root = Path(__file__).parents[2] / '.github/workflows'
        github = SimpleNamespace(
            workflow_sha='b' * 40,
            event=SimpleNamespace(repository=SimpleNamespace(default_branch='main')),
        )
        for filename, name in (
            ('web-previews-publish.yml', 'reviewer'),
            ('web-previews-complete.yml', 'cdn'),
        ):
            with self.subTest(job=name):
                path = root / filename
                self.assertTrue(path.is_file())
                job = workflow_jobs(path.read_text())[name]
                ref = re.search(r'^          ref: \$\{\{ (.+) \}\}$', job, re.M).group(1)
                self.assertEqual(eval(ref, {'__builtins__': {}}, {'github': github}), 'b' * 40)
                self.assertIn('persist-credentials: false', job)

    def test_completion_event_is_isolated_from_key_readers_and_shares_the_comment_queue(self):
        root = Path(__file__).parents[2] / '.github/workflows'
        publisher = (root / 'web-previews-publish.yml').read_text()
        self.assertEqual(
            re.findall(r'^  (\w+):', publisher.split('permissions:', 1)[0], re.M), ['workflow_run']
        )
        path = root / 'web-previews-complete.yml'
        self.assertTrue(path.is_file())
        completion = path.read_text()
        self.assertEqual(
            re.findall(r'^  (\w+):', completion.split('permissions:', 1)[0], re.M),
            ['deployment_status'],
        )
        publishers = workflow_jobs(publisher)
        self.assertNotIn('cdn', publishers)
        jobs = workflow_jobs(completion)
        self.assertEqual(set(jobs), {'cdn'})
        self.assertIn('permissions: {}\n', completion)
        self.assertNotRegex(completion, r'(?m)^    environment:|id-token:|secrets\.')
        self.assertNotIn('FIGMA', completion)
        self.assertNotIn('oddish', completion)
        self.assertEqual(
            re.findall(r'^        run: (.+)$', jobs['cdn'], re.M),
            ['python3 tooling/web-preview.py complete-cdn'],
        )
        for writer in (publishers['pending'], publishers['report'], jobs['cdn']):
            group = re.search(r'^      group: (.+)$', writer, re.M).group(1)
            self.assertEqual(group, 'web-previews-comments')
            self.assertNotIn('${{', group)
            self.assertIn('      cancel-in-progress: false\n      queue: max\n', writer)
        permissions = re.search(r'(?m)^    permissions:\n((?:      [^\n]+\n)+)', jobs['cdn']).group(
            1
        )
        self.assertEqual(
            permissions,
            '      contents: read\n      actions: read\n      pull-requests: write\n',
        )
        validator = root / 'web-preview-publisher-check.yml'
        if validator.exists():
            self.assertIn(
                "      - '.github/workflows/web-previews-complete.yml'", validator.read_text()
            )

    def test_completion_workflow_accepts_only_enabled_matching_successful_cdn_events(self):
        path = Path(__file__).parents[2] / '.github/workflows/web-previews-complete.yml'
        self.assertTrue(path.is_file())
        guard = re.search(r'^    if: (.+)$', workflow_jobs(path.read_text())['cdn'], re.M).group(1)
        for field, value in (
            ('valid', ''),
            ('enabled', 'false'),
            ('event_name', 'workflow_run'),
            ('environment', 'zone'),
            ('task', 'deploy'),
            ('packageName', '@other/package'),
            ('state', 'failure'),
            ('state', 'pending'),
            ('state', ''),
        ):
            values = {
                'enabled': 'true',
                'event_name': 'deployment_status',
                'environment': 'cdn',
                'task': 'upload-to-cdn',
                'packageName': preview.PACKAGE,
                'state': 'success',
            }
            values[field] = value
            with self.subTest(field=field, value=value):
                context = {
                    'vars': SimpleNamespace(WEB_PREVIEW_REVIEWER_ENABLED=values['enabled']),
                    'github': SimpleNamespace(
                        event_name=values['event_name'],
                        event=SimpleNamespace(
                            deployment=SimpleNamespace(
                                environment=values['environment'],
                                task=values['task'],
                                payload=SimpleNamespace(packageName=values['packageName']),
                            ),
                            deployment_status=SimpleNamespace(state=values['state']),
                        ),
                    ),
                }
                self.assertEqual(
                    bool(eval(guard.replace('&&', 'and'), {'__builtins__': {}}, context)),
                    field == 'valid',
                )

    def test_completion_retries_the_initial_receipt_and_posts_the_real_attempt(self):
        github = PreviewGitHub()
        responses = [
            preview.HTTPError('https://cdn.example', 503, 'temporary', {}, None),
            cdn_responses(attempt=2)[0],
            *cdn_responses(attempt=2),
        ]
        with (
            patch.object(preview, 'urlopen', side_effect=responses),
            patch.object(preview, 'api', side_effect=github.request),
            patch.object(preview.time, 'sleep') as sleep,
        ):
            preview.complete_cdn('example/one', cdn_event())
        self.assertEqual(len(github.writes), 1)
        self.assertIn('[Preview](https://cdn.decentraland.org/', github.writes[0])
        sleep.assert_called_once_with(5)

    def test_completion_retries_a_transient_resource_error_before_posting(self):
        github = PreviewGitHub()
        responses = [
            cdn_responses(attempt=2)[0],
            *cdn_responses(attempt=2)[:2],
            preview.HTTPError('https://cdn.example', 503, 'temporary', {}, None),
            *cdn_responses(attempt=2),
        ]
        with (
            patch.object(preview, 'urlopen', side_effect=responses),
            patch.object(preview, 'api', side_effect=github.request),
            patch.object(preview.time, 'sleep') as sleep,
        ):
            preview.complete_cdn('example/one', cdn_event())
        self.assertEqual(len(github.writes), 1)
        self.assertIn('[Preview](', github.writes[0])
        sleep.assert_called_once_with(5)

    def test_an_unavailable_initial_receipt_exhausts_one_minute_without_posting(self):
        clock = [0]
        github = PreviewGitHub()

        def sleep(seconds):
            clock[0] += seconds

        def unavailable(*args, **kwargs):
            raise preview.HTTPError('https://cdn.example', 503, 'temporary', {}, None)

        with (
            patch.object(preview, 'urlopen', side_effect=unavailable),
            patch.object(preview, 'api', side_effect=github.request),
            patch.object(preview.time, 'monotonic', side_effect=lambda: clock[0]),
            patch.object(preview.time, 'sleep', side_effect=sleep),
        ):
            with self.assertRaises(TimeoutError):
                preview.complete_cdn('example/one', cdn_event())
        self.assertEqual(clock[0], 60)
        self.assertEqual(github.writes, [])

    def test_late_upload_progress_preserves_the_verified_link_for_the_same_attempt(self):
        github = PreviewGitHub()
        with (
            patch.object(
                preview,
                'urlopen',
                side_effect=[
                    cdn_responses(2)[0],
                    *cdn_responses(2),
                    cdn_responses(2)[0],
                ],
            ),
            patch.object(preview, 'api', side_effect=github.request),
        ):
            preview.complete_cdn('example/one', cdn_event())
            preview.report('example/one', 123, '', 'uploading', expected_attempt=2)
        self.assertEqual(len(github.writes), 2)
        self.assertIn('[Preview](', github.comments[0]['body'])

    def test_new_attempt_upload_progress_replaces_the_previous_verified_link(self):
        github = PreviewGitHub()
        with (
            patch.object(
                preview,
                'urlopen',
                side_effect=[
                    cdn_responses(2)[0],
                    *cdn_responses(2),
                    cdn_responses(2)[0],
                ],
            ),
            patch.object(preview, 'api', side_effect=github.request),
        ):
            preview.complete_cdn('example/one', cdn_event())
            github.run = {**github.run, 'run_attempt': 3}
            preview.report('example/one', 123, '', 'uploading', expected_attempt=3)
        self.assertEqual(len(github.writes), 2)
        self.assertNotIn('[Preview](', github.comments[0]['body'])
        self.assertIn('Upload in progress', github.comments[0]['body'])
        self.assertIn('6-10 minutes', github.comments[0]['body'])
        self.assertIn(
            'https://github.com/example/one/actions/workflows/web-previews-publish.yml',
            github.comments[0]['body'],
        )

    def test_completion_rechecks_the_real_source_attempt_immediately_before_writing(self):
        github = PreviewGitHub()
        github.supersede_on_read = 3
        with (
            patch.object(preview, 'urlopen', side_effect=[cdn_responses(2)[0], *cdn_responses(2)]),
            patch.object(preview, 'api', side_effect=github.request),
        ):
            preview.complete_cdn('example/one', cdn_event())
        self.assertEqual(github.writes, [])

    def test_cdn_completion_cli_uses_the_event_without_a_source_run_environment(self):
        event = cdn_event()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'event.json'
            path.write_text(json.dumps(event))
            with (
                patch.object(preview.sys, 'argv', ['web-preview.py', 'complete-cdn']),
                patch.dict(
                    os.environ,
                    {'GITHUB_REPOSITORY': 'example/one', 'GITHUB_EVENT_PATH': str(path)},
                    clear=True,
                ),
                patch.object(preview, 'complete_cdn') as complete,
            ):
                preview.main()
                complete.assert_called_once_with('example/one', event)

    def test_completion_rejects_versions_not_bound_to_the_canonical_receipt_run(self):
        for version in (
            '0.1.122-123.commit-abcdef0',
            '0.1.0123-123.commit-abcdef0',
            '1.1.123-123.commit-abcdef0',
            '',
            'latest',
        ):
            event = cdn_event()
            event['deployment']['payload']['packageVersion'] = version
            github = PreviewGitHub()
            old = published_comment(cdn_preview_url())
            github.comments = [old]
            with (
                self.subTest(version=version),
                patch.object(preview, 'urlopen', return_value=cdn_responses(2)[0]),
                patch.object(preview, 'api', side_effect=github.request) as api,
                patch.object(preview, 'verify_cdn') as verify,
            ):
                with self.assertRaises(ValueError):
                    preview.complete_cdn('example/one', event)
            verify.assert_not_called()
            api.assert_not_called()
            self.assertEqual(github.writes, [])
            self.assertEqual(github.comments, [old])

    def test_completion_run_version_binding_preserves_valid_integer_boundaries(self):
        for run_id in (1, preview.MAX_RUN_ID, 0, -1, preview.MAX_RUN_ID + 1, True, '123', None):
            event = cdn_event()
            version = f'0.1.{run_id}-123.commit-abcdef0'
            event['deployment']['payload']['packageVersion'] = version
            receipt = {'revision': 'a' * 40, 'run': run_id, 'attempt': 1}
            pr = review_pr()
            with (
                self.subTest(run_id=run_id),
                patch.object(
                    preview, 'urlopen', return_value=io.BytesIO(json.dumps(receipt).encode())
                ),
                patch.object(preview, 'resolve', return_value=(build_run(pr), pr)) as resolve,
                patch.object(preview, 'verify_cdn', return_value='verified') as verify,
                patch.object(preview, 'report') as report,
            ):
                if type(run_id) is int and 1 <= run_id <= preview.MAX_RUN_ID:
                    preview.complete_cdn('example/one', event)
                    verify.assert_called_once_with(
                        version, 'a' * 40, run_id, timeout=unittest.mock.ANY, attempt=1
                    )
                    report.assert_called_once_with(
                        'example/one', run_id, 'verified', 'success', expected_attempt=1
                    )
                else:
                    with self.assertRaises(ValueError):
                        preview.complete_cdn('example/one', event)
                    resolve.assert_not_called()
                    verify.assert_not_called()
                    report.assert_not_called()

    def test_all_cdn_requests_send_the_preview_user_agent(self):
        with patch.object(preview, 'urlopen', side_effect=cdn_responses()) as fetch:
            preview.verify_cdn('0.1.0-123.commit-abcdef0', 'a' * 40, 123, timeout=0, attempt=1)
        self.assertEqual(fetch.call_count, 3)
        for call in fetch.call_args_list:
            self.assertIsInstance(call.args[0], preview.Request)
            self.assertEqual(call.args[0].get_header('User-agent'), preview.CDN_USER_AGENT)

    def test_cdn_success_signal_verifies_and_reports_without_waiting_for_the_upload(self):
        event = cdn_event()
        # The upload may finish much later than the former five-minute publishing window.
        event['deployment_status']['created_at'] = '2026-11-06T12:30:00Z'
        pr = review_pr()
        run = {**build_run(pr), 'run_attempt': 1, 'created_at': '2026-11-06T12:00:00Z'}
        receipt = io.BytesIO(json.dumps({'revision': 'a' * 40, 'run': 123, 'attempt': 1}).encode())
        with (
            patch.object(preview, 'urlopen', side_effect=[receipt, *cdn_responses()]),
            patch.object(preview, 'api', side_effect=[run, pr]),
            patch.object(preview, 'report') as report,
            patch.object(preview.time, 'sleep') as sleep,
        ):
            preview.complete_cdn('example/one', event)
            report.assert_called_once_with(
                'example/one',
                123,
                preview.cdn_url(event['deployment']['payload']['packageVersion'])
                + '/dcl-ui-review/index.html',
                'success',
                expected_attempt=1,
            )
            sleep.assert_not_called()

    def test_other_or_incomplete_cdn_deployments_do_not_probe_or_report(self):
        changes = [
            ('deployment_status', {'state': 'pending'}),
            ('deployment_status', {'state': 'failure'}),
            ('deployment_status', {}),
            ('deployment', {**cdn_event()['deployment'], 'environment': 'zone'}),
            ('deployment', {**cdn_event()['deployment'], 'task': 'deploy'}),
            ('deployment', {**cdn_event()['deployment'], 'payload': {'packageName': 'other'}}),
            ('deployment', {**cdn_event()['deployment'], 'payload': 'invalid'}),
        ]
        for field, value in changes:
            with self.subTest(field=field, value=value), patch.object(preview, 'urlopen') as fetch:
                preview.complete_cdn('example/one', {**cdn_event(), field: value})
                fetch.assert_not_called()

    def test_cdn_signal_cannot_link_a_closed_or_superseded_source_attempt(self):
        receipt = {'revision': 'a' * 40, 'run': 123, 'attempt': 1}
        pr = review_pr()
        for run, current in (
            ({**build_run(pr), 'run_attempt': 2}, pr),
            ({**build_run(pr), 'run_attempt': 1}, {**pr, 'state': 'closed'}),
            ({**build_run(pr), 'run_attempt': 1}, {**pr, 'head': {**pr['head'], 'sha': 'b' * 40}}),
        ):
            responses = [run] if run['run_attempt'] != 1 else [run, current]
            with (
                self.subTest(run=run, current=current),
                patch.object(
                    preview, 'urlopen', return_value=io.BytesIO(json.dumps(receipt).encode())
                ) as fetch,
                patch.object(preview, 'api', side_effect=responses),
                patch.object(preview, 'report') as report,
            ):
                preview.complete_cdn('example/one', cdn_event())
                report.assert_not_called()
                self.assertEqual(fetch.call_count, 1)

    def test_cdn_verification_retry_reuses_the_event_version_without_publishing(self):
        event = cdn_event()
        pr = review_pr()
        run = {**build_run(pr), 'run_attempt': 1}
        source = {'revision': 'a' * 40, 'run': 123, 'attempt': 1}
        failed = iter(
            [
                io.BytesIO(json.dumps(source).encode()),
                *cdn_responses()[:2],
                preview.HTTPError('https://cdn.example', 404, 'propagating', {}, None),
            ]
        )
        clock = [0]

        def fetch_failed(*args, **kwargs):
            response = next(failed)
            if isinstance(response, Exception):
                clock[0] = 60
                raise response
            return response

        with (
            patch.object(preview, 'urlopen', side_effect=fetch_failed),
            patch.object(preview, 'api', side_effect=[run, pr]),
            patch.object(preview, 'report') as report,
            patch.object(preview.time, 'monotonic', side_effect=lambda: clock[0]),
        ):
            with self.assertRaises(TimeoutError):
                preview.complete_cdn('example/one', event)
            report.assert_not_called()
        with (
            patch.object(
                preview,
                'urlopen',
                side_effect=[io.BytesIO(json.dumps(source).encode()), *cdn_responses()],
            ) as fetch,
            patch.object(preview, 'api', side_effect=[run, pr]),
            patch.object(preview, 'report') as report,
        ):
            preview.complete_cdn('example/one', event)
            self.assertEqual(
                fetch.call_args_list[0].args[0].full_url,
                preview.cdn_url(event['deployment']['payload']['packageVersion'])
                + '/preview-source.json',
            )
            report.assert_called_once()

    def test_bad_or_mismatched_cdn_receipts_never_link(self):
        pr = review_pr()
        source = {'revision': 'a' * 40, 'run': 123, 'attempt': 1}
        for receipt in (
            [],
            {},
            {**source, 'run': True},
            {**source, 'attempt': 0},
            {**source, 'attempt': None},
            {**source, 'revision': None},
        ):
            with (
                self.subTest(receipt=receipt),
                patch.object(
                    preview, 'urlopen', return_value=io.BytesIO(json.dumps(receipt).encode())
                ),
                patch.object(preview, 'api') as api,
            ):
                with self.assertRaisesRegex(ValueError, 'Invalid CDN source'):
                    preview.complete_cdn('example/one', cdn_event())
                api.assert_not_called()
        with (
            patch.object(
                preview,
                'urlopen',
                return_value=io.BytesIO(json.dumps({**source, 'revision': 'b' * 40}).encode()),
            ),
            patch.object(preview, 'api', side_effect=[{**build_run(pr), 'run_attempt': 1}, pr]),
            patch.object(preview, 'report') as report,
        ):
            with self.assertRaisesRegex(ValueError, 'does not match'):
                preview.complete_cdn('example/one', cdn_event())
            report.assert_not_called()

    def test_ordering_does_not_compare_attempt_numbers_from_different_runs(self):
        pr = review_pr()
        candidate = {**build_run(pr), 'run_attempt': 1}
        with (
            patch.dict(os.environ, {'SOURCE_RUN_ATTEMPT': '2'}),
            patch.object(
                preview,
                'api',
                side_effect=[
                    [{'id': 123}, {'id': 124}],
                    {'jobs': [{'name': 'changes', 'conclusion': 'success'}]},
                    candidate,
                    pr,
                ],
            ),
        ):
            self.assertFalse(preview.newest_run_for_pr('example/one', 123, build_run(pr), pr))

    def test_products_are_selected_independently(self):
        self.assertEqual(preview.products(['apps/sites/src/index.ts']), ['sites'])
        self.assertEqual(preview.products(['library/ui/components/Button.tsx']), ['reviewer'])
        self.assertEqual(
            preview.products(['apps/sites/package.json', 'library/ui/package.json']),
            ['sites', 'reviewer'],
        )

    def test_other_apps_and_api_contracts_do_not_trigger_previews(self):
        self.assertEqual(
            preview.products(
                [
                    'apps/ui3/src/index.ts',
                    'library/catalyst-api/a.ts',
                    'apps/dcl-scene-cli/src/main.rs',
                    'README.md',
                ]
            ),
            [],
        )

    def test_preview_pipeline_changes_select_both_products(self):
        for path in [
            'tooling/web-preview.py',
            'tooling/tests/test_web_preview.py',
            '.github/workflows/web-previews.yml',
            '.github/workflows/web-previews-publish.yml',
            '.github/workflows/web-previews-complete.yml',
        ]:
            with self.subTest(path=path):
                self.assertEqual(preview.products([path]), ['sites', 'reviewer'])

    def test_only_current_same_repository_pr_runs_can_publish(self):
        run = {
            'event': 'pull_request',
            'status': 'completed',
            'conclusion': 'success',
            'head_sha': 'a' * 40,
            'path': '.github/workflows/web-previews.yml',
            'head_repository': {'full_name': 'example/one'},
        }
        pr = {'state': 'open', 'head': {'sha': 'a' * 40, 'repo': {'full_name': 'example/one'}}}
        self.assertTrue(preview.publishable(run, pr, 'example/one'))
        for key, value in [
            ('event', 'push'),
            ('conclusion', 'cancelled'),
            ('status', 'in_progress'),
            ('head_sha', 'b' * 40),
            ('path', '.github/workflows/other.yml'),
            ('head_repository', {'full_name': 'fork/one'}),
        ]:
            with self.subTest(key=key):
                self.assertFalse(preview.publishable({**run, key: value}, pr, 'example/one'))
        self.assertFalse(preview.publishable(run, {**pr, 'state': 'closed'}, 'example/one'))
        self.assertFalse(
            preview.publishable(run, {**pr, 'head': {**pr['head'], 'repo': None}}, 'example/one')
        )

    def test_static_package_contains_only_assets_and_trusted_metadata(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, target = root / 'input', root / 'output'
            (source / 'dcl-ui-review').mkdir(parents=True)
            (source / 'dcl-ui-review/index.html').write_text('<html></html>')
            (source / 'assets').mkdir()
            (source / 'assets/main.js').write_text('console.log("review")')
            preview.package_reviewer(source, target, 'a' * 40, 123, 1)
            metadata = json.loads((target / 'package.json').read_text())
            self.assertEqual(metadata['name'], '@dcl-regenesislabs/dcl-one-ui-review')
            self.assertEqual(metadata['version'], '0.1.123')
            self.assertEqual(
                metadata['repository']['url'], 'git+https://github.com/decentraland/dcl-one.git'
            )
            self.assertNotIn('scripts', metadata)
            self.assertEqual(
                json.loads((target / 'preview-source.json').read_text()),
                {'revision': 'a' * 40, 'run': 123, 'attempt': 1},
            )
            self.assertTrue((target / 'assets/main.js').is_file())

    def test_reviewer_can_publish_when_sites_failed(self):
        run = {
            'event': 'pull_request',
            'status': 'completed',
            'conclusion': 'failure',
            'head_sha': 'a' * 40,
            'path': '.github/workflows/web-previews.yml',
            'head_repository': {'full_name': 'example/one'},
        }
        pr = {'state': 'open', 'head': {'sha': 'a' * 40, 'repo': {'full_name': 'example/one'}}}
        self.assertTrue(preview.publishable(run, pr, 'example/one'))

    def test_only_a_successful_reviewer_job_and_artifact_can_publish(self):
        job = {'name': 'UI reviewer', 'status': 'completed', 'conclusion': 'success'}
        artifact = {'name': 'web-preview-reviewer', 'expired': False}
        self.assertTrue(preview.reviewer_ready([job], [artifact]))
        for conclusion in ['failure', 'cancelled', 'skipped', None]:
            with self.subTest(conclusion=conclusion):
                self.assertFalse(
                    preview.reviewer_ready([{**job, 'conclusion': conclusion}], [artifact])
                )
        self.assertFalse(preview.reviewer_ready([job], []))
        self.assertFalse(preview.reviewer_ready([job], [{**artifact, 'expired': True}]))
        self.assertFalse(preview.reviewer_ready([], [artifact]))
        self.assertFalse(preview.reviewer_ready([job, job], [artifact]))
        self.assertFalse(preview.reviewer_ready([job], [artifact, artifact]))

    def test_static_package_rejects_hidden_files_source_scripts_and_symlinks(self):
        for name in ['.npmrc', 'package.json', 'server.ts', 'run.sh', 'source.map']:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / 'input/dcl-ui-review').mkdir(parents=True)
                (root / 'input/dcl-ui-review/index.html').write_text('ok')
                (root / 'input' / name).write_text('untrusted')
                with self.assertRaises(ValueError):
                    preview.package_reviewer(root / 'input', root / 'output', 'a' * 40, 123, 1)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'input/dcl-ui-review').mkdir(parents=True)
            (root / 'input/dcl-ui-review/index.html').write_text('ok')
            (root / 'input/link.js').symlink_to('/etc/passwd')
            with self.assertRaises(ValueError):
                preview.package_reviewer(root / 'input', root / 'output', 'a' * 40, 123, 1)

    def test_static_package_requires_the_reviewer_entry_and_valid_source_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'input'
            source.mkdir()
            with self.assertRaises(ValueError):
                preview.package_reviewer(source, root / 'output', 'a' * 40, 123, 1)
            (source / 'dcl-ui-review').mkdir()
            (source / 'dcl-ui-review/index.html').write_text('ok')
            for sha, run in [('bad', 123), ('a' * 40, 0)]:
                with self.subTest(sha=sha, run=run), self.assertRaises(ValueError):
                    preview.package_reviewer(source, root / 'output', sha, run, 1)

    def test_cdn_requires_the_expected_source_build(self):
        response = io.BytesIO(json.dumps({'revision': 'b' * 40, 'run': 123}).encode())
        with patch.object(preview, 'urlopen', return_value=response):
            with self.assertRaisesRegex(ValueError, 'different source'):
                preview.verify_cdn('0.1.0-123.commit-abcdef0', 'a' * 40, 123, timeout=0, attempt=1)

    def test_cdn_checks_the_html_entry_before_returning_a_link(self):
        with patch.object(preview, 'urlopen', side_effect=cdn_responses()) as fetch:
            self.assertEqual(
                preview.verify_cdn('0.1.0-123.commit-abcdef0', 'a' * 40, 123, timeout=0, attempt=1),
                'https://cdn.decentraland.org/@dcl-regenesislabs/dcl-one-ui-review/'
                '0.1.0-123.commit-abcdef0/dcl-ui-review/index.html',
            )
            self.assertTrue(
                fetch.call_args_list[2]
                .args[0]
                .full_url.endswith('/dcl-ui-review/assets/main-Abc123.js')
            )

    def test_cdn_rejects_non_html_entries_and_invalid_versions(self):
        identity = io.BytesIO(json.dumps({'revision': 'a' * 40, 'run': 123, 'attempt': 1}).encode())
        html = io.BytesIO(b'not html')
        html.status = 200
        html.headers = {'Content-Type': 'application/json'}
        with patch.object(preview, 'urlopen', side_effect=[identity, html]):
            with self.assertRaises(TimeoutError):
                preview.verify_cdn('0.1.0-123.commit-abcdef0', 'a' * 40, 123, timeout=0, attempt=1)
        for version in ['../other', '0.1.0-123.commit-abcdef0\n', 'latest']:
            with self.subTest(version=version), self.assertRaises(ValueError):
                preview.cdn_url(version)

    def test_report_rechecks_the_head_before_posting(self):
        run = {
            'head_sha': 'a' * 40,
            'run_attempt': 1,
            'html_url': 'https://github.com/example/one/actions/runs/123',
        }
        pr = review_pr()
        jobs = {'jobs': [{'name': 'UI reviewer', 'conclusion': 'success'}]}
        with patch.object(preview, 'resolve', side_effect=[(run, pr), (run, None)]):
            with patch.object(preview, 'api', side_effect=[jobs, []]) as api:
                preview.report('example/one', 123, 'https://cdn.example/review', 'success')
                self.assertEqual(api.call_count, 2)

    def test_report_ignores_a_run_superseded_on_the_same_head(self):
        run = {
            'head_sha': 'a' * 40,
            'run_attempt': 1,
            'html_url': 'https://github.com/example/one/actions/runs/123',
        }
        pr = review_pr()
        jobs = {'jobs': [{'name': 'UI reviewer', 'conclusion': 'success'}]}
        comments = [
            {
                'id': 1,
                'user': {'login': 'github-actions[bot]'},
                'body': preview.MARKER + '\n<!-- source-run:123:2 -->',
            }
        ]
        with patch.object(preview, 'resolve', return_value=(run, pr)):
            with patch.object(preview, 'api', side_effect=[jobs, comments, [{'id': 124}]]) as api:
                preview.report('example/one', 123, '', 'failure')
                self.assertEqual(api.call_count, 3)

    def test_dependabot_runs_cannot_publish(self):
        run = {
            'event': 'pull_request',
            'status': 'completed',
            'conclusion': 'success',
            'path': preview.WORKFLOW,
            'head_repository': {'full_name': 'example/one'},
            'head_sha': 'a' * 40,
        }
        pr = {'state': 'open', 'head': {'sha': 'a' * 40, 'repo': {'full_name': 'example/one'}}}
        for field in ('actor', 'triggering_actor'):
            with self.subTest(field=field):
                self.assertFalse(
                    preview.publishable(
                        {**run, field: {'login': 'dependabot[bot]'}}, pr, 'example/one'
                    )
                )

    def test_cdn_accepts_bumped_package_versions(self):
        self.assertTrue(
            preview.cdn_url('0.1.1-123.commit-abcdef0').endswith('/0.1.1-123.commit-abcdef0')
        )

    def test_cdn_retries_transient_decode_and_transport_failures(self):
        errors = [
            json.JSONDecodeError('not JSON', '', 0),
            ConnectionResetError(),
            preview.http.client.IncompleteRead(b'partial'),
            preview.http.client.RemoteDisconnected(),
        ]
        for error in errors:
            with self.subTest(error=type(error).__name__):
                with (
                    patch.object(preview, 'urlopen', side_effect=[error, *cdn_responses()]),
                    patch.object(preview.time, 'sleep') as sleep,
                ):
                    self.assertTrue(
                        preview.verify_cdn(
                            '0.1.0-123.commit-abcdef0', 'a' * 40, 123, attempt=1
                        ).endswith('index.html')
                    )
                    sleep.assert_called_once_with(5)

    def test_resolve_uses_commit_fallback_and_skips_outdated_candidates(self):
        pr = review_pr(2)
        closed = {**review_pr(1), 'state': 'closed'}
        run = {**build_run(pr), 'pull_requests': []}
        with patch.object(preview, 'api', side_effect=[run, [closed, pr], closed, pr]) as api:
            self.assertEqual(preview.resolve('example/one', 123), (run, pr))
            self.assertTrue(api.call_args_list[1].kwargs['paginate'])

    def test_resolve_rejects_cancelled_runs(self):
        pr = review_pr()
        run = {**build_run(pr), 'conclusion': 'cancelled'}
        with patch.object(preview, 'api', side_effect=[run, pr]):
            self.assertEqual(preview.resolve('example/one', 123), (run, None))

    def test_report_posts_or_patches_current_comment(self):
        run = {
            'head_sha': 'a' * 40,
            'run_attempt': 2,
            'html_url': 'https://github.com/example/one/actions/runs/123',
        }
        pr = review_pr()
        jobs = {'jobs': [{'name': 'UI reviewer', 'conclusion': 'success'}]}
        for comments, method, path in [
            ([], 'POST', 'repos/example/one/issues/9/comments'),
            (
                [
                    {
                        'id': 5,
                        'user': {'login': 'github-actions[bot]'},
                        'body': preview.MARKER + '\n<!-- source-run:999999999:99999 -->',
                    }
                ],
                'PATCH',
                'repos/example/one/issues/comments/5',
            ),
        ]:
            with (
                self.subTest(method=method),
                patch.object(preview, 'resolve', return_value=(run, pr)),
                patch.object(
                    preview,
                    'api',
                    side_effect=[jobs, comments, [{'id': 123}], {}],
                ) as api,
            ):
                preview.report('example/one', 123, 'https://cdn.example/review', 'success')
                self.assertEqual(api.call_args.args[0], path)
                self.assertEqual(api.call_args.kwargs['method'], method)
                self.assertIn(
                    '[Preview](https://cdn.example/review)', api.call_args.kwargs['body']['body']
                )

    def test_api_prints_error_details_before_raising(self):
        result = subprocess.CompletedProcess(['gh'], 1, '', 'permission denied (HTTP 403)')
        with (
            patch.object(preview.subprocess, 'run', return_value=result),
            patch('sys.stderr', new_callable=io.StringIO) as stderr,
        ):
            with self.assertRaises(subprocess.CalledProcessError):
                preview.api('repos/example/one')
            self.assertIn('permission denied', stderr.getvalue())

    def test_cdn_rejects_root_absolute_assets_and_missing_bundles(self):
        entry = preview.cdn_url('0.1.0-123.commit-abcdef0')
        for html in [
            '<script src="/dcl-ui-review/assets/main-Abc123.js"></script>',
            '<html>not a reviewer bundle</html>',
        ]:
            with self.subTest(html=html), self.assertRaises(ValueError):
                preview.verify_reviewer_resources(entry, entry + '/dcl-ui-review/index.html', html)

    def test_cdn_rejects_asset_fallback_html(self):
        responses = cdn_responses()
        responses[2].headers = {'Content-Type': 'text/html'}
        with (
            patch.object(preview, 'urlopen', side_effect=responses),
            self.assertRaises(TimeoutError),
        ):
            preview.verify_cdn('0.1.0-123.commit-abcdef0', 'a' * 40, 123, timeout=0, attempt=1)

    def test_cdn_retries_a_missing_runtime_bundle(self):
        failed = cdn_responses()[:2] + [
            preview.HTTPError('https://cdn.example', 404, 'missing', {}, None)
        ]
        with (
            patch.object(preview, 'urlopen', side_effect=failed + cdn_responses()),
            patch.object(preview.time, 'sleep') as sleep,
        ):
            preview.verify_cdn('0.1.0-123.commit-abcdef0', 'a' * 40, 123, attempt=1)
            sleep.assert_called_once_with(5)

    def test_workflow_key_guard_cli_scans_artifacts_and_rejects_secrets(self):
        root = Path(__file__).resolve().parents[2]
        workflow = (root / '.github/workflows/web-previews-publish.yml').read_text()
        command = re.search(r'(node library/ui/figma/key-guard.mjs[^|]+) \| tee', workflow).group(1)
        args = shlex.split(command.replace('\\\n', ''))
        with tempfile.TemporaryDirectory() as directory:
            artifact = Path(directory) / 'index.html'
            artifact.write_text('<html>review</html>')
            args = [directory if arg == '.preview/input' else arg for arg in args]
            env = {**os.environ, 'FIGMA_FILE_KEY': 'synthetic_preview_guard_key'}
            clean = subprocess.run(args, cwd=root, env=env, capture_output=True, text=True)
            self.assertEqual(clean.returncode, 0, clean.stdout + clean.stderr)
            self.assertIn('check key: ran', clean.stdout)
            artifact.write_text('synthetic_preview_guard_key')
            leaked = subprocess.run(args, cwd=root, env=env, capture_output=True, text=True)
            self.assertEqual(leaked.returncode, 1)
            self.assertNotIn('synthetic_preview_guard_key', leaked.stdout + leaked.stderr)

    def test_report_omits_skipped_products_and_reports_sites_as_pending(self):
        run = {'head_sha': 'a' * 40, 'run_attempt': 1, 'html_url': 'https://github.com/example/run'}
        pr = review_pr()
        jobs = {
            'jobs': [
                {'name': 'UI reviewer', 'conclusion': 'skipped'},
                {'name': 'Sites', 'conclusion': 'success'},
            ]
        }
        with (
            patch.object(preview, 'resolve', return_value=(run, pr)),
            patch.object(preview, 'api', side_effect=[jobs, [], [{'id': 123}], {}]) as api,
        ):
            preview.report('example/one', 123, '', 'disabled')
            body = api.call_args.kwargs['body']['body']
            self.assertNotIn('| UI reviewer |', body)
            self.assertIn('| Sites | success | SSR preview pending', body)

    def test_report_links_failed_jobs_and_reports_disabled_publication(self):
        run = {'head_sha': 'a' * 40, 'run_attempt': 1, 'html_url': 'https://github.com/example/run'}
        pr = review_pr()
        for conclusion, expected in [
            ('failure', '[Build log](https://github.com/example/job)'),
            ('success', 'Publication: disabled'),
        ]:
            jobs = {
                'jobs': [
                    {
                        'name': 'UI reviewer',
                        'conclusion': conclusion,
                        'html_url': 'https://github.com/example/job',
                    }
                ]
            }
            with (
                self.subTest(conclusion=conclusion),
                patch.object(preview, 'resolve', return_value=(run, pr)),
                patch.object(preview, 'api', side_effect=[jobs, [], [{'id': 123}], {}]) as api,
            ):
                preview.report('example/one', 123, '', 'disabled')
                self.assertIn(expected, api.call_args.kwargs['body']['body'])

    def test_no_selected_products_post_no_new_comment(self):
        pr = review_pr()
        run = {**build_run(pr), 'run_attempt': 1, 'html_url': 'https://github.com/example/run'}
        jobs = {
            'jobs': [
                {'name': 'Sites', 'conclusion': 'skipped'},
                {'name': 'UI reviewer', 'conclusion': 'skipped'},
            ]
        }
        with (
            patch.object(preview, 'resolve', return_value=(run, pr)),
            patch.object(preview, 'api', side_effect=[jobs, []]) as api,
        ):
            preview.report('example/one', 123, '', 'skipped')
            self.assertEqual(api.call_count, 2)

    def test_resolve_cli_emits_github_output_contract(self):
        run = {
            'event': 'pull_request',
            'status': 'completed',
            'conclusion': 'success',
            'path': preview.WORKFLOW,
            'head_repository': {'full_name': 'example/one'},
            'head_sha': 'a' * 40,
        }
        pr = {
            'number': 9,
            'state': 'open',
            'head': {'sha': 'a' * 40, 'repo': {'full_name': 'example/one'}},
        }
        for selected, expected in [
            (pr, 'pr=9\npublish=true\nreviewer=true\n'),
            (None, 'pr=\npublish=false\nreviewer=true\n'),
        ]:
            with (
                self.subTest(selected=selected),
                patch.object(preview.sys, 'argv', ['web-preview.py', 'resolve']),
                patch.dict(
                    os.environ, {'GITHUB_REPOSITORY': 'example/one', 'SOURCE_RUN_ID': '123'}
                ),
                patch.object(preview, 'resolve', return_value=(run, selected)),
                patch.object(preview, 'newest_run_for_pr', return_value=True) as newest,
                patch.object(preview, 'require_reviewer', return_value=True),
                patch('sys.stdout', new_callable=io.StringIO) as stdout,
            ):
                preview.main()
                self.assertEqual(stdout.getvalue(), expected)
                self.assertEqual(newest.call_count, int(selected is not None))

    def test_select_uses_cumulative_real_git_diff_on_a_later_docs_commit(self):
        script = Path(preview.__file__).resolve()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            def git(*args):
                return subprocess.check_output(
                    [
                        'git',
                        '-c',
                        'user.name=Preview Test',
                        '-c',
                        'user.email=preview@example.invalid',
                        '-c',
                        'commit.gpgsign=false',
                        *args,
                    ],
                    cwd=root,
                    text=True,
                ).strip()

            git('init', '-q')
            (root / 'README.md').write_text('base')
            git('add', '.')
            git('commit', '-qm', 'base')
            base = git('rev-parse', 'HEAD')
            (root / 'apps/sites').mkdir(parents=True)
            (root / 'apps/sites/page.ts').write_text('web')
            git('add', '.')
            git('commit', '-qm', 'web change')
            (root / 'README.md').write_text('later documentation')
            git('add', '.')
            git('commit', '-qm', 'docs change')
            output = subprocess.check_output(
                ['python3', str(script), 'select', '--base', base, '--head', 'HEAD'],
                cwd=root,
                text=True,
            )
            self.assertEqual(output, 'sites=true\nreviewer=false\n')

    def test_run_resolves_its_own_pr_and_base_for_a_shared_head(self):
        first = review_pr(9, 'main')
        second = review_pr(10, 'feature/base', 'c' * 40)
        for pr in (first, second):
            run = build_run(pr)
            with (
                self.subTest(number=pr['number']),
                patch.object(preview, 'api', side_effect=[run, pr]),
            ):
                self.assertEqual(preview.resolve('example/one', 123), (run, pr))

    def test_resolve_rejects_changed_base_identity(self):
        pr = review_pr()
        run = build_run(pr)
        for replacement in (review_pr(base='other'), {**pr, 'base': None}):
            with (
                self.subTest(base=replacement['base']),
                patch.object(preview, 'api', side_effect=[run, replacement]),
            ):
                self.assertEqual(preview.resolve('example/one', 123), (run, None))

    def test_ambiguous_commit_fallback_cannot_pick_an_arbitrary_pr(self):
        first = review_pr(9)
        second = review_pr(10, 'feature/base', 'c' * 40)
        run = {**build_run(first), 'pull_requests': []}
        with patch.object(preview, 'api', side_effect=[run, [first, second], first, second]):
            self.assertEqual(preview.resolve('example/one', 123), (run, None))

    def test_newest_run_is_scoped_to_the_same_pr_and_base(self):
        pr = review_pr()
        runs = [
            {'id': 123, 'pull_requests': [pr]},
            {'id': 124, 'pull_requests': [review_pr(10, 'feature/base')]},
        ]
        with patch.object(preview, 'api', return_value=runs):
            self.assertTrue(preview.newest_run_for_pr('example/one', 123, build_run(pr), pr))
        runs.append({'id': 125, 'pull_requests': [pr]})
        with patch.object(
            preview,
            'api',
            side_effect=[runs, {'jobs': [{'name': 'changes', 'conclusion': 'success'}]}],
        ):
            self.assertFalse(preview.newest_run_for_pr('example/one', 123, build_run(pr), pr))

    def test_newest_run_without_pr_metadata_is_resolved_before_matching(self):
        pr = review_pr()
        runs = [{'id': 123}, {'id': 124}]
        for candidate, expected in ((review_pr(10), True), (pr, False), (None, True)):
            with (
                self.subTest(candidate=candidate),
                patch.object(
                    preview,
                    'api',
                    side_effect=[runs, {'jobs': [{'name': 'changes', 'conclusion': 'success'}]}],
                ),
                patch.object(preview, 'resolve', return_value=({}, candidate)),
            ):
                self.assertEqual(
                    preview.newest_run_for_pr('example/one', 123, build_run(pr), pr), expected
                )

    def test_failed_selection_replaces_a_previous_successful_preview(self):
        pr = review_pr()
        run = {
            **build_run(pr),
            'conclusion': 'failure',
            'run_attempt': 1,
            'html_url': 'https://github.com/example/run',
        }
        old = {
            'id': 5,
            'user': {'login': 'github-actions[bot]'},
            'body': preview.MARKER + '\n[Preview](https://cdn.example/old)',
        }
        for selection in (
            [
                {
                    'name': 'changes',
                    'conclusion': 'failure',
                    'html_url': 'https://github.com/example/selection',
                }
            ],
            [],
        ):
            jobs = {
                'jobs': selection
                + [
                    {'name': 'Sites', 'conclusion': 'skipped'},
                    {'name': 'UI reviewer', 'conclusion': 'skipped'},
                ]
            }
            with (
                self.subTest(selection=selection),
                patch.object(preview, 'resolve', return_value=(run, pr)),
                patch.object(preview, 'api', side_effect=[jobs, [old], [{'id': 123}], {}]) as api,
            ):
                preview.report('example/one', 123, '', 'skipped')
                self.assertEqual(api.call_args.kwargs['method'], 'PATCH')
                body = api.call_args.kwargs['body']['body']
                self.assertIn('| Preview selection | failure |', body)
                self.assertNotIn('https://cdn.example/old', body)
                expected = (
                    'https://github.com/example/selection'
                    if selection
                    else 'https://github.com/example/run'
                )
                self.assertIn(expected, body)

    def test_report_cannot_switch_pr_between_identity_checks(self):
        pr = review_pr()
        run = {**build_run(pr), 'run_attempt': 1, 'html_url': 'https://github.com/example/run'}
        jobs = {'jobs': [{'name': 'Sites', 'conclusion': 'success'}]}
        with (
            patch.object(preview, 'resolve', side_effect=[(run, pr), (run, review_pr(10))]),
            patch.object(preview, 'api', side_effect=[jobs, []]) as api,
        ):
            preview.report('example/one', 123, '', 'skipped')
            self.assertEqual(api.call_count, 2)

    def test_incomplete_pr_identity_cannot_match_a_run(self):
        pr = review_pr()
        for candidate in (
            {},
            {**pr, 'base': None},
            {**pr, 'head': None},
            {**pr, 'base': {'sha': 'b' * 40}},
        ):
            with self.subTest(candidate=candidate):
                self.assertFalse(preview.same_pull_request(candidate, pr))

    def test_ambiguous_run_metadata_cannot_pick_an_arbitrary_pr(self):
        first = review_pr()
        second = review_pr(10, 'feature/base')
        run = {**build_run(first), 'pull_requests': [first, second]}
        with patch.object(preview, 'api', side_effect=[run, first, second]):
            self.assertEqual(preview.resolve('example/one', 123), (run, None))

    def test_source_run_absent_from_latest_listing_cannot_replace_a_comment(self):
        pr = review_pr()
        with patch.object(preview, 'api', return_value=[]):
            self.assertFalse(preview.newest_run_for_pr('example/one', 123, build_run(pr), pr))

    def test_reverted_web_changes_clear_the_existing_preview_comment(self):
        pr = review_pr()
        run = {**build_run(pr), 'run_attempt': 1, 'html_url': 'https://github.com/example/run'}
        jobs = {
            'jobs': [
                {'name': 'Sites', 'conclusion': 'skipped'},
                {'name': 'UI reviewer', 'conclusion': 'skipped'},
            ]
        }
        old = {
            'id': 5,
            'user': {'login': 'github-actions[bot]'},
            'body': preview.MARKER + '\n[Preview](https://cdn.example/old)',
        }
        with (
            patch.object(preview, 'resolve', return_value=(run, pr)),
            patch.object(preview, 'api', side_effect=[jobs, [old], [{'id': 123}], {}]) as api,
        ):
            preview.report('example/one', 123, '', 'skipped')
            self.assertEqual(api.call_args.kwargs['method'], 'PATCH')
            body = api.call_args.kwargs['body']['body']
            self.assertIn('| Web previews | not selected |', body)
            self.assertNotIn('https://cdn.example/old', body)
            self.assertIn('Commit `aaaaaaa`', body)

    def test_resolve_accepts_a_base_tip_advance_without_retargeting_the_pr(self):
        original = review_pr()
        live = review_pr(base_sha='c' * 40)
        run = build_run(original)
        with patch.object(preview, 'api', side_effect=[run, live]):
            self.assertEqual(preview.resolve('example/one', 123), (run, live))

    def test_newest_run_counts_the_same_pr_after_a_base_tip_advance(self):
        pr = review_pr()
        runs = [
            {'id': 123},
            {'id': 124, 'pull_requests': [review_pr(base_sha='c' * 40)]},
        ]
        with patch.object(
            preview,
            'api',
            side_effect=[runs, {'jobs': [{'name': 'changes', 'conclusion': 'success'}]}],
        ):
            self.assertFalse(preview.newest_run_for_pr('example/one', 123, build_run(pr), pr))

    def test_other_bot_actors_and_pr_authors_cannot_publish(self):
        pr = review_pr()
        run = build_run(pr)
        for account in (
            {'login': 'renovate[bot]', 'type': 'Bot'},
            {'login': 'other-app', 'type': 'Bot'},
            {'login': 'other[bot]'},
        ):
            for field in ('actor', 'triggering_actor'):
                with self.subTest(account=account, field=field):
                    self.assertFalse(
                        preview.publishable({**run, field: account}, pr, 'example/one')
                    )
            self.assertFalse(preview.publishable(run, {**pr, 'user': account}, 'example/one'))

    def test_api_retries_transient_server_errors(self):
        failure = subprocess.CompletedProcess(['gh'], 1, '', 'HTTP 502: Bad Gateway')
        success = subprocess.CompletedProcess(['gh'], 0, '{"ok": true}', '')
        for method in ('GET', 'PATCH'):
            with (
                self.subTest(method=method),
                patch.object(preview.subprocess, 'run', side_effect=[failure, success]) as request,
                patch.object(preview.time, 'sleep') as sleep,
            ):
                self.assertEqual(preview.api('repos/example/one', method=method), {'ok': True})
                self.assertEqual(request.call_count, 2)
                sleep.assert_called_once_with(1)

    def test_api_does_not_retry_permanent_errors_or_comment_creation(self):
        for method, status in (('GET', 404), ('GET', 403), ('POST', 503)):
            failure = subprocess.CompletedProcess(['gh'], 1, '', f'HTTP {status}: error')
            with (
                self.subTest(method=method, status=status),
                patch.object(preview.subprocess, 'run', return_value=failure) as request,
                patch.object(preview.time, 'sleep') as sleep,
            ):
                with self.assertRaises(subprocess.CalledProcessError):
                    preview.api('repos/example/one', method=method)
                self.assertEqual(request.call_count, 1)
                sleep.assert_not_called()

    def test_api_timeout_retries_are_bounded(self):
        error = subprocess.TimeoutExpired(['gh'], 30)
        with (
            patch.object(preview.subprocess, 'run', side_effect=error) as request,
            patch.object(preview.time, 'sleep') as sleep,
        ):
            with self.assertRaises(subprocess.TimeoutExpired):
                preview.api('repos/example/one')
            self.assertEqual(request.call_count, 3)
            self.assertEqual([call.args[0] for call in sleep.call_args_list], [1, 2])

    def test_api_retries_a_transient_invalid_json_response(self):
        bad = subprocess.CompletedProcess(['gh'], 0, '<html>upstream error</html>', '')
        good = subprocess.CompletedProcess(['gh'], 0, '[1, 2]', '')
        with (
            patch.object(preview.subprocess, 'run', side_effect=[bad, good]),
            patch.object(preview.time, 'sleep') as sleep,
        ):
            self.assertEqual(preview.api('repos/example/one'), [1, 2])
            sleep.assert_called_once_with(1)

    def test_publication_requires_a_human_reviewer_and_no_admin_bypass(self):
        reviewer = {'type': 'User', 'reviewer': {'type': 'User', 'login': 'maintainer'}}
        environment = {
            'can_admins_bypass': False,
            'protection_rules': [
                {
                    'type': 'required_reviewers',
                    'prevent_self_review': False,
                    'reviewers': [reviewer],
                }
            ],
            'deployment_branch_policy': {
                'custom_branch_policies': True,
                'protected_branches': False,
            },
        }
        with patch.object(
            preview,
            'api',
            side_effect=[
                environment,
                {'default_branch': 'main'},
                {'total_count': 1, 'branch_policies': [{'name': 'main', 'type': 'branch'}]},
            ],
        ) as request:
            preview.require_publication_environment('example/one')
            self.assertEqual(
                request.call_args_list[0].args[0], 'repos/example/one/environments/web-preview'
            )
        for invalid in (
            {},
            {**environment, 'can_admins_bypass': True},
            {**environment, 'can_admins_bypass': None},
            {**environment, 'protection_rules': []},
            {**environment, 'protection_rules': [{'type': 'required_reviewers', 'reviewers': []}]},
        ):
            with (
                self.subTest(environment=invalid),
                patch.object(preview, 'api', return_value=invalid),
            ):
                with self.assertRaisesRegex(ValueError, 'human reviewer'):
                    preview.require_publication_environment('example/one')
        for invalid in (
            {},
            {'type': 'Team', 'reviewer': {'type': 'User', 'login': 'team'}},
            {'type': 'User', 'reviewer': None},
            {'type': 'User', 'reviewer': {'type': 'Bot', 'login': 'app'}},
            {'type': 'User', 'reviewer': {'type': 'User', 'login': 'app[bot]'}},
        ):
            rules = [{'type': 'required_reviewers', 'reviewers': [invalid]}]
            with (
                self.subTest(reviewer=invalid),
                patch.object(
                    preview, 'api', return_value={**environment, 'protection_rules': rules}
                ),
            ):
                with self.assertRaisesRegex(ValueError, 'human reviewer'):
                    preview.require_publication_environment('example/one')

    def test_missing_or_unreadable_environment_cannot_authorize_publication(self):
        with patch.object(preview, 'api', side_effect=subprocess.CalledProcessError(1, ['gh'])):
            with self.assertRaises(subprocess.CalledProcessError):
                preview.require_publication_environment('example/one')

    def test_environment_rejects_missing_branch_policy_tags_wildcards_and_other_branches(self):
        environment = {
            'can_admins_bypass': False,
            'protection_rules': [
                {
                    'type': 'required_reviewers',
                    'reviewers': [
                        {'type': 'User', 'reviewer': {'type': 'User', 'login': 'maintainer'}},
                    ],
                }
            ],
        }
        for policy in (None, {}, {'protected_branches': True}, {'custom_branch_policies': False}):
            with (
                self.subTest(policy=policy),
                patch.object(
                    preview,
                    'api',
                    return_value={
                        **environment,
                        'deployment_branch_policy': policy,
                    },
                ),
            ):
                with self.assertRaisesRegex(ValueError, 'restrict deployments'):
                    preview.require_publication_environment('example/one')
        environment['deployment_branch_policy'] = {
            'custom_branch_policies': True,
            'protected_branches': False,
        }
        for branches in (
            [],
            [{'name': '*', 'type': 'branch'}],
            [{'name': 'main', 'type': 'tag'}],
            [{'name': 'feature', 'type': 'branch'}],
            [{}],
            [{'name': 'main', 'type': 'branch'}, {'name': 'feature', 'type': 'branch'}],
        ):
            with (
                self.subTest(branches=branches),
                patch.object(
                    preview,
                    'api',
                    side_effect=[
                        environment,
                        {'default_branch': 'main'},
                        {'total_count': len(branches), 'branch_policies': branches},
                    ],
                ),
            ):
                with self.assertRaisesRegex(ValueError, 'allow only the default branch'):
                    preview.require_publication_environment('example/one')

    def test_pending_approval_replaces_a_previous_preview_without_claiming_publication(self):
        pr = review_pr()
        run = {**build_run(pr), 'run_attempt': 1, 'html_url': 'https://github.com/example/run'}
        old = {
            'id': 5,
            'user': {'login': 'github-actions[bot]'},
            'body': preview.MARKER + '\n[Preview](https://cdn.example/old)',
        }
        with (
            patch.object(preview, 'resolve', return_value=(run, pr)),
            patch.object(
                preview,
                'api',
                side_effect=[
                    {'jobs': [{'name': 'UI reviewer', 'conclusion': 'success'}]},
                    [old],
                    [{'id': 123}],
                    {},
                ],
            ) as request,
        ):
            preview.report('example/one', 123, '', 'waiting for human approval')
            body = request.call_args.kwargs['body']['body']
            self.assertIn('Publication: waiting for human approval', body)
            self.assertNotIn('https://cdn.example/old', body)
            self.assertNotIn('<!-- source-run:', body)

    def test_metadata_edit_keeps_the_existing_comment_when_selection_was_skipped(self):
        pr = review_pr()
        run = build_run(pr)
        with (
            patch.object(preview, 'resolve', return_value=(run, pr)),
            patch.object(
                preview,
                'api',
                return_value={
                    'jobs': [{'name': 'changes', 'conclusion': 'skipped'}],
                },
            ) as request,
        ):
            preview.report('example/one', 123, '', 'skipped')
            self.assertEqual(request.call_count, 1)

    def test_metadata_edit_does_not_supersede_an_actual_build_on_the_same_head(self):
        pr = review_pr()
        runs = [
            {'id': 123, 'pull_requests': [pr]},
            {'id': 124, 'pull_requests': [pr]},
        ]
        for conclusion, expected in (('skipped', True), ('success', False), (None, False)):
            with (
                self.subTest(conclusion=conclusion),
                patch.object(
                    preview,
                    'api',
                    side_effect=[
                        runs,
                        {'jobs': [{'name': 'changes', 'conclusion': conclusion}]},
                    ],
                ),
            ):
                self.assertEqual(
                    preview.newest_run_for_pr('example/one', 123, build_run(pr), pr), expected
                )

    def test_api_paginates_arrays_and_named_collections(self):
        for pages, collection in (
            ([[{'id': 1}], [{'id': 2}]], None),
            ([{'workflow_runs': [{'id': 1}]}, {'workflow_runs': [{'id': 2}]}], 'workflow_runs'),
        ):
            result = subprocess.CompletedProcess(['gh'], 0, json.dumps(pages), '')
            with (
                self.subTest(collection=collection),
                patch.object(preview.subprocess, 'run', return_value=result) as request,
            ):
                self.assertEqual(
                    preview.api('repos/example/one', paginate=True, collection=collection),
                    [{'id': 1}, {'id': 2}],
                )
                self.assertIn('--paginate', request.call_args.args[0])
                self.assertIn('--slurp', request.call_args.args[0])

    def test_many_metadata_runs_cannot_hide_the_source_run_or_trigger_resolve_calls(self):
        pr = review_pr()
        runs = [{'id': value, 'conclusion': 'skipped'} for value in range(250, 123, -1)]
        runs.append({'id': 123, 'pull_requests': [pr]})
        with (
            patch.object(preview, 'api', return_value=runs) as request,
            patch.object(preview, 'resolve') as resolve,
        ):
            self.assertTrue(preview.newest_run_for_pr('example/one', 123, build_run(pr), pr))
            request.assert_called_once()
            self.assertTrue(request.call_args.kwargs['paginate'])
            self.assertEqual(request.call_args.kwargs['collection'], 'workflow_runs')
            resolve.assert_not_called()

    def test_skipped_selection_is_checked_before_resolving_missing_pr_metadata(self):
        pr = review_pr()
        with (
            patch.object(
                preview,
                'api',
                side_effect=[
                    [{'id': 123}, {'id': 124}],
                    {'jobs': [{'name': 'changes', 'conclusion': 'skipped'}]},
                ],
            ),
            patch.object(preview, 'resolve') as resolve,
        ):
            self.assertTrue(preview.newest_run_for_pr('example/one', 123, build_run(pr), pr))
            resolve.assert_not_called()

    def test_cdn_retries_transient_fallback_assets_and_invalid_utf8(self):
        for scenario in ('mime', 'empty_asset', 'utf8', 'missing_bundle'):
            bad = cdn_responses()
            if scenario == 'mime':
                bad[2].headers = {'Content-Type': 'text/html'}
                bad = bad[:3]
            elif scenario == 'empty_asset':
                bad[2] = io.BytesIO(b'')
                bad[2].headers = {'Content-Type': 'application/javascript'}
                bad = bad[:3]
            else:
                bad[1] = io.BytesIO(b'\xff' if scenario == 'utf8' else b'<html>propagating</html>')
                bad[1].status = 200
                bad[1].headers = {'Content-Type': 'text/html'}
                bad = bad[:2]
            with (
                self.subTest(scenario=scenario),
                patch.object(preview, 'urlopen', side_effect=bad + cdn_responses()),
                patch.object(preview.time, 'sleep') as sleep,
            ):
                self.assertTrue(
                    preview.verify_cdn(
                        '0.1.0-123.commit-abcdef0', 'a' * 40, 123, attempt=1
                    ).endswith('index.html')
                )
                sleep.assert_called_once_with(5)

    def test_cdn_does_not_retry_a_bundle_escaping_the_version_prefix(self):
        responses = cdn_responses()
        responses[1] = io.BytesIO(b'<script src="/assets/main-Abc123.js"></script>')
        responses[1].status = 200
        responses[1].headers = {'Content-Type': 'text/html'}
        with (
            patch.object(preview, 'urlopen', side_effect=responses),
            patch.object(preview.time, 'sleep') as sleep,
        ):
            with self.assertRaisesRegex(ValueError, 'escapes'):
                preview.verify_cdn('0.1.0-123.commit-abcdef0', 'a' * 40, 123, attempt=1)
            sleep.assert_not_called()

    def test_newer_queued_or_cancelled_runs_with_no_jobs_supersede_an_older_build(self):
        pr = review_pr()
        for status, conclusion in (
            ('queued', None),
            ('in_progress', None),
            ('completed', 'cancelled'),
        ):
            for associated in (True, False):
                candidate = {
                    **build_run(pr),
                    'id': 124,
                    'status': status,
                    'conclusion': conclusion,
                    'pull_requests': [pr] if associated else [],
                }
                responses = [[{'id': 123}, candidate], {'jobs': []}]
                if not associated:
                    responses += [candidate, [pr], pr]
                with (
                    self.subTest(status=status, associated=associated),
                    patch.object(preview, 'api', side_effect=responses),
                ):
                    self.assertFalse(
                        preview.newest_run_for_pr('example/one', 123, build_run(pr), pr)
                    )

    def test_api_retries_rate_limits_without_retrying_other_forbidden_errors(self):
        success = subprocess.CompletedProcess(['gh'], 0, '{"ok": true}', '')
        for method in ('GET', 'PATCH'):
            for error in (
                'HTTP 429: Too many requests',
                'HTTP 403: API rate limit exceeded',
                'HTTP 403: secondary rate-limit',
            ):
                failure = subprocess.CompletedProcess(['gh'], 1, '', error)
                with (
                    self.subTest(method=method, error=error),
                    patch.object(preview.subprocess, 'run', side_effect=[failure, success]),
                    patch.object(preview.time, 'sleep') as sleep,
                ):
                    self.assertEqual(preview.api('repos/example/one', method=method), {'ok': True})
                    sleep.assert_called_once_with(60)

    def test_rate_limit_retries_are_bounded_and_do_not_retry_comment_creation(self):
        failure = subprocess.CompletedProcess(['gh'], 1, '', 'HTTP 429: Too many requests')
        for method in ('GET', 'PATCH', 'POST'):
            with (
                self.subTest(method=method),
                patch.object(preview.subprocess, 'run', return_value=failure) as request,
                patch.object(preview.time, 'sleep') as sleep,
            ):
                with self.assertRaises(subprocess.CalledProcessError):
                    preview.api('repos/example/one', method=method)
                self.assertEqual(request.call_count, 1 if method == 'POST' else 3)
                self.assertEqual(
                    [call.args[0] for call in sleep.call_args_list],
                    [] if method == 'POST' else [60, 120],
                )

    def test_other_transient_errors_reset_to_short_backoff_after_a_rate_limit(self):
        limited = subprocess.CompletedProcess(['gh'], 1, '', 'HTTP 403: secondary rate-limit')
        bad_gateway = subprocess.CompletedProcess(['gh'], 1, '', 'HTTP 502: Bad Gateway')
        success = subprocess.CompletedProcess(['gh'], 0, '{"ok": true}', '')
        with (
            patch.object(preview.subprocess, 'run', side_effect=[limited, bad_gateway, success]),
            patch.object(preview.time, 'sleep') as sleep,
        ):
            self.assertEqual(preview.api('repos/example/one'), {'ok': True})
            self.assertEqual([call.args[0] for call in sleep.call_args_list], [60, 2])

    def test_artifact_guard_rejects_links_and_oversized_files(self):
        root = Path(__file__).resolve().parents[2]
        workflow = (root / '.github/workflows/web-previews-publish.yml').read_text()
        command = re.search(r'(node library/ui/figma/key-guard.mjs[^|]+) \| tee', workflow).group(1)
        args = shlex.split(command.replace('\\\n', ''))
        self.assertIn('--artifact', args)
        for case in ('symlink', 'size'):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as directory:
                artifact = Path(directory) / 'asset.js'
                if case == 'symlink':
                    artifact.symlink_to('/etc/hosts')
                else:
                    with artifact.open('wb') as output:
                        output.truncate(17 * 1024**2)
                result = subprocess.run(
                    [directory if arg == '.preview/input' else arg for arg in args],
                    cwd=root,
                    env={**os.environ, 'FIGMA_FILE_KEY': 'synthetic_preview_guard_key'},
                    capture_output=True,
                    text=True,
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('FAIL artifact', result.stdout)

    def test_rerun_cannot_reuse_an_earlier_attempts_publication_approval(self):
        pr = review_pr()
        run = {**build_run(pr), 'run_attempt': 2}
        with (
            patch.object(preview, 'api', return_value=run) as request,
        ):
            self.assertEqual(preview.resolve('example/one', 123, expected_attempt=1), (run, None))
            request.assert_called_once()
        with (
            patch.object(preview, 'api', side_effect=[run, pr]),
        ):
            self.assertEqual(preview.resolve('example/one', 123, expected_attempt=2), (run, pr))

    def test_build_summary_distinguishes_artifacts_and_live_publication(self):
        for enabled, message in (
            (False, 'CDN publication disabled'),
            (True, 'Pending the separate publisher, source checks and human approval'),
        ):
            with self.subTest(enabled=enabled):
                summary = preview.build_summary(
                    'success', 'success', 'success', enabled, 'example/one', 123
                )
                self.assertIn(message, summary)
                self.assertIn('SSR hosting integration is pending', summary)
                self.assertIn('https://github.com/example/one/actions/runs/123', summary)
                self.assertIn('must be installed on the default branch', summary)
                self.assertNotIn('[Preview]', summary)

    def test_build_summary_reports_failed_selection_and_failed_or_cancelled_products(self):
        for selection, reviewer in (
            ('failure', 'skipped'),
            ('success', 'failure'),
            ('success', 'cancelled'),
        ):
            with self.subTest(selection=selection, reviewer=reviewer):
                summary = preview.build_summary(
                    selection, reviewer, 'skipped', False, 'example/one', 123
                )
                if selection == 'failure':
                    self.assertIn('| Preview selection | failure |', summary)
                else:
                    self.assertIn(
                        'Unavailable: build or CDN-path validation did not succeed', summary
                    )
                self.assertNotIn('CDN publication disabled', summary)

    def test_build_summary_reports_no_selected_products(self):
        summary = preview.build_summary('success', 'skipped', 'skipped', False, 'example/one', 123)
        self.assertIn('| Web previews | not selected |', summary)
        self.assertNotIn('| UI reviewer |', summary)

    def test_summary_cli_writes_the_github_job_summary_without_a_source_run(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'summary.md'
            with (
                patch.object(preview.sys, 'argv', ['web-preview.py', 'build-summary']),
                patch.dict(
                    os.environ,
                    {
                        'GITHUB_REPOSITORY': 'example/one',
                        'GITHUB_RUN_ID': '123',
                        'SELECTION_RESULT': 'success',
                        'REVIEWER_RESULT': 'success',
                        'SITES_RESULT': 'skipped',
                        'GITHUB_STEP_SUMMARY': str(output),
                    },
                    clear=True,
                ),
            ):
                preview.main()
            self.assertIn('CDN publication disabled', output.read_text())


if __name__ == '__main__':
    unittest.main()
