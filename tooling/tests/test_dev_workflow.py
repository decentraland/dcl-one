"""Development CI keeps platform coverage and reusable, isolated build snapshots."""
from pathlib import Path
import json
import os
import re
import subprocess
import tempfile
import textwrap
from types import SimpleNamespace
import unittest
from unittest.mock import patch


WORKFLOW = Path(__file__).parents[2] / '.github/workflows/development.yml'
CLEANUP = WORKFLOW.with_name('closed-pr-cache.yml')


def jobs():
    source = WORKFLOW.read_text().split('jobs:\n', 1)[1]
    return dict(re.findall(r'(?ms)^  (\w+):\n(.*?)(?=^  \w+:|\Z)', source))


class DevelopmentWorkflowTest(unittest.TestCase):
    def test_tests_and_smoke_have_separate_cache_owners_on_both_platforms(self):
        namespaces = []
        for name in ('rust', 'smoke'):
            job = jobs()[name]
            # Both platforms on main; PRs add macOS with the ci:macos label.
            self.assertRegex(job, r'os: (\[ubuntu-26\.04, macos-26\]|.*"ubuntu-26\.04", "macos-26")')
            keys = re.findall(r'^          key: (foundation-dev-v2-[^\n]+)', job, re.M)
            self.assertEqual(len(keys), 1)
            namespaces.append(keys[0].split('${{', 1)[0])
            self.assertIn('steps.rust-inputs.outputs.source', keys[0])
            self.assertIn("hashFiles('services/catalyst/Cargo.lock')", keys[0])
            saves = re.findall(
                r'(?ms)^      - name: Save successful[^\n]*\n(.*?)(?=^      - |\Z)', job)
            self.assertEqual(len(saves), 1)
            self.assertIn('if: success()', saves[0])
            self.assertIn("github.event_name == 'push'", saves[0])
            self.assertIn("github.ref == 'refs/heads/main'", saves[0])
            self.assertIn('steps.rust-cache.outputs.cache-primary-key', saves[0])
        self.assertNotEqual(*namespaces)
        self.assertIn('bash tooling/dev-test.sh rust', jobs()['rust'])
        self.assertIn('bash tooling/dev-smoke.sh', jobs()['smoke'])
        self.assertIn('bash tooling/dev-test.sh e2e', jobs()['smoke'])

    def test_large_pr_snapshots_are_only_same_repository_linux_tests(self):
        def allowed(job, event, ref, platform, same_repository=True, passed=True, hit=False, cache_enabled=False):
            save = re.search(r'(?ms)^      - name: Save successful[^\n]*\n(.*?)(?=^      - |\Z)',
                             jobs()[job]).group(1)
            expression = re.search(r'^        if: (.+)$', save, re.M).group(1)
            expression = expression.replace('&&', 'and').replace('||', 'or')
            expression = expression.replace('steps.rust-cache.outputs.cache-hit', 'cache_hit')
            expression = expression.replace("contains(github.event.pull_request.labels.*.name, 'ci:cache')", 'cache_enabled')
            github = SimpleNamespace(
                event_name=event, ref=ref, repository='decentraland/dcl-one',
                event=SimpleNamespace(pull_request=SimpleNamespace(head=SimpleNamespace(
                    repo=SimpleNamespace(full_name='decentraland/dcl-one' if same_repository else 'fork/dcl-one')))))
            return eval(expression, {'__builtins__': {}}, {
                'github': github, 'runner': SimpleNamespace(os=platform),
                'success': lambda: passed, 'cache_hit': 'true' if hit else 'false',
                'cache_enabled': cache_enabled,
            })

        for job in ('rust', 'smoke'):
            for platform in ('Linux', 'macOS'):
                with self.subTest(job=job, platform=platform):
                    self.assertTrue(allowed(job, 'push', 'refs/heads/main', platform))
                    self.assertFalse(allowed(job, 'push', 'refs/heads/feature', platform))
                    self.assertFalse(allowed(job, 'workflow_dispatch', 'refs/heads/main', platform))
                    self.assertFalse(allowed(job, 'push', 'refs/heads/main', platform, passed=False))
                    self.assertFalse(allowed(job, 'push', 'refs/heads/main', platform, hit=True))
                    self.assertFalse(allowed(job, 'pull_request', 'refs/pull/150/merge', platform))
                    self.assertEqual(allowed(job, 'pull_request', 'refs/pull/150/merge', platform, cache_enabled=True),
                                     job == 'rust' and platform == 'Linux')
                    self.assertFalse(allowed(job, 'pull_request', 'refs/pull/150/merge', platform,
                                             same_repository=False, cache_enabled=True))

    def test_closed_pr_cleanup_deletes_only_exact_merge_ref_from_every_page(self):
        source = CLEANUP.read_text()
        self.assertIn('pull_request_target:\n    types: [closed]', source)
        self.assertIn('permissions: {}', source)
        self.assertNotIn('checkout', source.split('steps:', 1)[1].replace('checks out', ''))
        self.assertNotIn('uses:', source)
        script = textwrap.dedent(source.split("python3 - <<'PY'\n", 1)[1].rsplit('\n          PY', 1)[0])
        self.assertNotIn('${{', script)
        pages = [{'actions_caches': [
            {'id': 1, 'ref': 'refs/heads/main'},
            {'id': 2, 'ref': 'refs/pull/151/merge'},
            {'id': 3, 'ref': 'refs/pull/150/merge'},
        ]}, {'actions_caches': [
            {'id': 4, 'ref': 'refs/pull/150/head'},
            {'id': 5, 'ref': 'refs/pull/150/merge'},
            {'id': 'unsafe', 'ref': 'refs/pull/150/merge'},
        ]}]
        with patch.dict(os.environ, {'PR_NUMBER': '150', 'GITHUB_REPOSITORY': 'decentraland/dcl-one'}), \
                patch('subprocess.check_output', return_value=json.dumps(pages)) as listing, \
                patch('subprocess.run') as deletion, patch('builtins.print'):
            exec(compile(script, str(CLEANUP), 'exec'), {})
            request = listing.call_args.args[0]
            self.assertIn('ref=refs/pull/150/merge', request)
            self.assertIn('--paginate', request)
            self.assertEqual([call.args[0][-1] for call in deletion.call_args_list], [
                'repos/decentraland/dcl-one/actions/caches/3',
                'repos/decentraland/dcl-one/actions/caches/5',
            ])
            for number in ('', '0', '-1', '../main', '150; echo unsafe'):
                with self.subTest(number=number), patch.dict(os.environ, {'PR_NUMBER': number}):
                    listing.reset_mock()
                    deletion.reset_mock()
                    with self.assertRaises(ValueError):
                        exec(compile(script, str(CLEANUP), 'exec'), {})
                    listing.assert_not_called()
                    deletion.assert_not_called()

    def test_scheduled_sweep_catches_late_uploads_and_bounds_only_dev_snapshots(self):
        source = CLEANUP.read_text()
        self.assertIn("cron: '17 * * * *'", source)
        script = textwrap.dedent(source.split("python3 - <<'PY'\n", 1)[1].rsplit('\n          PY', 1)[0])
        prefix = 'foundation-dev-v2-tests-decentraland/dcl-one-Linux-X64-'
        caches = [
            {'id': 1, 'ref': 'refs/heads/main', 'key': prefix + 'old'},
            {'id': 2, 'ref': 'refs/heads/main', 'key': prefix + 'new'},
            {'id': 3, 'ref': 'refs/heads/main', 'key': 'images-v2-main-exact'},
            {'id': 4, 'ref': 'refs/pull/10/merge', 'key': prefix + 'late'},
            {'id': 5, 'ref': 'refs/pull/10/merge', 'key': 'images-v2-pr-10-late'},
            {'id': 6, 'ref': 'refs/pull/11/merge', 'key': prefix + 'old'},
            {'id': 7, 'ref': 'refs/pull/11/merge', 'key': prefix + 'new'},
            {'id': 8, 'ref': 'refs/pull/11/merge', 'key': 'images-v2-pr-11-exact'},
            {'id': 9, 'ref': 'refs/pull/10/head', 'key': prefix + 'ignored'},
            {'id': 10, 'ref': 'refs/heads/other', 'key': prefix + 'ignored'},
            {'id': 11, 'ref': 'refs/heads/main', 'key': 'bevy-cache'},
        ]
        def api(args, **kwargs):
            if any(arg.endswith('/actions/caches') for arg in args):
                return json.dumps([{'actions_caches': caches}])
            if args[-1].endswith('/pulls/10'):
                return json.dumps({'state': 'closed'})
            if args[-1].endswith('/pulls/11'):
                return json.dumps({'state': 'open'})
            self.fail(f'Unexpected API call: {args}')
        with patch.dict(os.environ, {'GITHUB_EVENT_NAME': 'schedule', 'PR_NUMBER': '',
                                     'GITHUB_REPOSITORY': 'decentraland/dcl-one'}), \
                patch('subprocess.check_output', side_effect=api) as listing, \
                patch('subprocess.run') as deletion, patch('builtins.print'):
            exec(compile(script, str(CLEANUP), 'exec'), {})
        self.assertEqual([call.args[0][-1] for call in deletion.call_args_list], [
            'repos/decentraland/dcl-one/actions/caches/1',
            'repos/decentraland/dcl-one/actions/caches/4',
            'repos/decentraland/dcl-one/actions/caches/5',
            'repos/decentraland/dcl-one/actions/caches/6',
        ])
        self.assertEqual(listing.call_count, 3, 'one state lookup per PR, regardless of cache count')

    def test_source_key_tracks_embedded_inputs_but_not_unrelated_commits(self):
        commands = [re.search(r'^          source=(.*)$', jobs()[name], re.M).group(1)
                    for name in ('rust', 'smoke')]
        self.assertEqual(*commands)
        inputs = ('services/catalyst/worlds/src/storage/export.rs',
                  'library/protocol/proto/message.proto', 'schemas/20_worlds.sql',
                  'apps/dcl-scene-cli/src/vendor/node_modules.zip', '.cargo/config.toml')
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            def git(*args):
                return subprocess.run(['git', *args], cwd=root, check=True, capture_output=True)

            def commit():
                git('add', '.')
                git('-c', 'user.name=CI test', '-c', 'user.email=ci@example.invalid',
                    '-c', 'commit.gpgsign=false', 'commit', '-qm', 'fixture')

            def fingerprint():
                result = subprocess.run(['bash', '-eo', 'pipefail', '-c',
                                         'source=' + commands[0] + '\nprintf "%s" "$source"'],
                                        cwd=root, check=True, capture_output=True, text=True)
                return result.stdout

            git('init', '-q')
            for name in (*inputs, 'docs/unrelated.md'):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text('original')
            commit()
            previous = fingerprint()
            (root / 'docs/unrelated.md').write_text('changed')
            commit()
            self.assertEqual(previous, fingerprint())
            for name in inputs:
                with self.subTest(name=name):
                    (root / name).write_text('changed')
                    commit()
                    current = fingerprint()
                    self.assertNotEqual(previous, current)
                    previous = current

    def test_only_unit_suites_are_gated_by_web_selection(self):
        web = jobs()['web']
        self.assertIn('name: Web tests and contracts', web)
        self.assertIn('run: python3 tooling/dev-ci.py', web)
        self.assertIn('BASE_SHA: ${{ github.event.pull_request.base.sha }}', web)
        steps = re.split(r'^      - ', web, flags=re.M)[1:]
        selected = [step for step in steps if 'if: steps.web-coverage.outputs' in step]
        self.assertEqual(len(selected), 3)
        for app in ('ui3', 'sites', 'ui3/editor-scene'):
            self.assertTrue(any(f'pnpm --dir apps/{app} test\n' in step for step in selected))
        contracts = next(step for step in steps if 'name: Check web types' in step)
        self.assertNotIn('\n        if:', contracts)
        self.assertIn('typecheck:scripts', contracts)
        self.assertIn('gen:schemas:check', contracts)


if __name__ == '__main__':
    unittest.main()
