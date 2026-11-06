import importlib.util
import io
import json
from pathlib import Path
import tempfile
import subprocess
import os
import re
import shlex
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


def cdn_responses():
    identity = io.BytesIO(json.dumps({'revision': 'a' * 40, 'run': 123}).encode())
    html = io.BytesIO(b'<html><script src="./assets/main-Abc123.js"></script></html>')
    html.status = 200
    html.headers = {'Content-Type': 'text/html'}
    asset = io.BytesIO(b'console.log("review")')
    asset.headers = {'Content-Type': 'application/javascript'}
    fidelity = io.BytesIO(b'{"cases": []}')
    return [identity, html, asset, fidelity]


class WebPreviewTests(unittest.TestCase):
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
            'tooling/web-preview-browser.mjs',
            '.github/workflows/web-previews.yml',
            '.github/workflows/web-previews-publish.yml',
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
            preview.package_reviewer(source, target, 'a' * 40, 123)
            metadata = json.loads((target / 'package.json').read_text())
            self.assertEqual(metadata['name'], '@dcl-regenesislabs/dcl-one-ui-review')
            self.assertNotIn('scripts', metadata)
            self.assertEqual(
                json.loads((target / 'preview-source.json').read_text()),
                {'revision': 'a' * 40, 'run': 123},
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
                    preview.package_reviewer(root / 'input', root / 'output', 'a' * 40, 123)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'input/dcl-ui-review').mkdir(parents=True)
            (root / 'input/dcl-ui-review/index.html').write_text('ok')
            (root / 'input/link.js').symlink_to('/etc/passwd')
            with self.assertRaises(ValueError):
                preview.package_reviewer(root / 'input', root / 'output', 'a' * 40, 123)

    def test_static_package_requires_the_reviewer_entry_and_valid_source_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'input'
            source.mkdir()
            with self.assertRaises(ValueError):
                preview.package_reviewer(source, root / 'output', 'a' * 40, 123)
            (source / 'dcl-ui-review').mkdir()
            (source / 'dcl-ui-review/index.html').write_text('ok')
            for sha, run in [('bad', 123), ('a' * 40, 0)]:
                with self.subTest(sha=sha, run=run), self.assertRaises(ValueError):
                    preview.package_reviewer(source, root / 'output', sha, run)

    def test_cdn_requires_the_expected_source_build(self):
        response = io.BytesIO(json.dumps({'revision': 'b' * 40, 'run': 123}).encode())
        with patch.object(preview, 'urlopen', return_value=response):
            with self.assertRaisesRegex(ValueError, 'different source'):
                preview.verify_cdn('0.1.0-123.commit-abcdef0', 'a' * 40, 123, timeout=0)

    def test_cdn_checks_the_html_entry_before_returning_a_link(self):
        with patch.object(preview, 'urlopen', side_effect=cdn_responses()) as fetch:
            self.assertEqual(
                preview.verify_cdn('0.1.0-123.commit-abcdef0', 'a' * 40, 123, timeout=0),
                'https://cdn.decentraland.org/@dcl-regenesislabs/dcl-one-ui-review/'
                '0.1.0-123.commit-abcdef0/dcl-ui-review/index.html',
            )
            self.assertTrue(
                fetch.call_args_list[2].args[0].endswith('/dcl-ui-review/assets/main-Abc123.js')
            )
            self.assertTrue(
                fetch.call_args_list[3].args[0].endswith('/dcl-ui-review/fidelity.json')
            )

    def test_cdn_rejects_non_html_entries_and_invalid_versions(self):
        identity = io.BytesIO(json.dumps({'revision': 'a' * 40, 'run': 123}).encode())
        html = io.BytesIO(b'not html')
        html.status = 200
        html.headers = {'Content-Type': 'application/json'}
        with patch.object(preview, 'urlopen', side_effect=[identity, html]):
            with self.assertRaises(TimeoutError):
                preview.verify_cdn('0.1.0-123.commit-abcdef0', 'a' * 40, 123, timeout=0)
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
            with patch.object(
                preview, 'api', side_effect=[jobs, comments, {'workflow_runs': [{'id': 124}]}]
            ) as api:
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
                        preview.verify_cdn('0.1.0-123.commit-abcdef0', 'a' * 40, 123).endswith(
                            'index.html'
                        )
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
                    side_effect=[jobs, comments, {'workflow_runs': [{'id': 123}]}, {}],
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
        with patch.object(preview, 'urlopen', side_effect=responses), self.assertRaises(ValueError):
            preview.verify_cdn('0.1.0-123.commit-abcdef0', 'a' * 40, 123, timeout=0)

    def test_cdn_retries_missing_fidelity_data(self):
        failed = cdn_responses()[:3] + [
            preview.HTTPError('https://cdn.example', 404, 'missing', {}, None)
        ]
        with (
            patch.object(preview, 'urlopen', side_effect=failed + cdn_responses()),
            patch.object(preview.time, 'sleep') as sleep,
        ):
            preview.verify_cdn('0.1.0-123.commit-abcdef0', 'a' * 40, 123)
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
            patch.object(
                preview, 'api', side_effect=[jobs, [], {'workflow_runs': [{'id': 123}]}, {}]
            ) as api,
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
                patch.object(
                    preview, 'api', side_effect=[jobs, [], {'workflow_runs': [{'id': 123}]}, {}]
                ) as api,
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
                patch.object(preview, 'require_reviewer', return_value=True),
                patch('sys.stdout', new_callable=io.StringIO) as stdout,
            ):
                preview.main()
                self.assertEqual(stdout.getvalue(), expected)

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
        runs = {
            'workflow_runs': [
                {'id': 123, 'pull_requests': [pr]},
                {'id': 124, 'pull_requests': [review_pr(10, 'feature/base')]},
            ]
        }
        with patch.object(preview, 'api', return_value=runs):
            self.assertTrue(preview.newest_run_for_pr('example/one', 123, build_run(pr), pr))
        runs['workflow_runs'].append({'id': 125, 'pull_requests': [pr]})
        with patch.object(preview, 'api', side_effect=[runs, {'jobs': []}]):
            self.assertFalse(preview.newest_run_for_pr('example/one', 123, build_run(pr), pr))

    def test_newest_run_without_pr_metadata_is_resolved_before_matching(self):
        pr = review_pr()
        runs = {'workflow_runs': [{'id': 123}, {'id': 124}]}
        for candidate, expected in ((review_pr(10), True), (pr, False), (None, True)):
            with (
                self.subTest(candidate=candidate),
                patch.object(preview, 'api', side_effect=[runs, {'jobs': []}]),
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
                patch.object(
                    preview, 'api', side_effect=[jobs, [old], {'workflow_runs': [{'id': 123}]}, {}]
                ) as api,
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
        with patch.object(preview, 'api', return_value={'workflow_runs': []}):
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
            patch.object(
                preview, 'api', side_effect=[jobs, [old], {'workflow_runs': [{'id': 123}]}, {}]
            ) as api,
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
        runs = {
            'workflow_runs': [
                {'id': 123},
                {'id': 124, 'pull_requests': [review_pr(base_sha='c' * 40)]},
            ]
        }
        with patch.object(preview, 'api', side_effect=[runs, {'jobs': []}]):
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
                    {'workflow_runs': [{'id': 123}]},
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
        runs = {
            'workflow_runs': [
                {'id': 123, 'pull_requests': [pr]},
                {'id': 124, 'pull_requests': [pr]},
            ]
        }
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
