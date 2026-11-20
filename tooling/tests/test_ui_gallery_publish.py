"""The UI gallery publisher publishes only the artifact the key scan cleared, for an open pull request at its head."""
import copy
import datetime
import hashlib
import importlib.util
import io
from pathlib import Path
import re
import stat
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('ui_gallery_publish', ROOT / 'tooling/ui-gallery-publish.py')
publish = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(publish)

REPO, REPO_ID = 'example/repo', 42
HEAD, BRANCH, PULL = 'a' * 40, 'feat/gallery', 17
SCAN, ATTEMPT, KEY_JOB = 800, 1, 600
RUN, ARTIFACT, DIGEST = 900, 700, 'sha256:' + 'b' * 64
VARIABLES = {
    'UI_GALLERY_ROLE_ARN': 'arn:aws:iam::123456789012:role/ui-gallery-writer',
    'UI_GALLERY_BUCKET': 'example-ui-gallery',
    'UI_GALLERY_AWS_REGION': 'us-east-1',
    'UI_GALLERY_BASE_URL': 'https://ui-gallery.example.com',
}
PAGE = f'https://ui-gallery.example.com/ui-gallery/pr/{PULL}/{HEAD}/index.html'


def scan_log(review=f'ui-review-{HEAD}', run=RUN, artifact=ARTIFACT, expected=DIGEST, downloaded=None, after=()):
    """A key-scan job log as the runner writes it: a timestamp on every line, one group per step. Without a review,
    the job skipped the Review download."""
    pin = '9' * 40
    download = [
        f'##[group]Run actions/download-artifact@{pin}', 'with:', f'  name: {review}',
        '  path: /home/runner/work/_temp/pr-artifact', f'  run-id: {run}', '  github-token: ***',
        f'  repository: {REPO}', '  digest-mismatch: error', '##[endgroup]',
        'Downloading single artifact', 'Preparing to download the following artifacts:',
        f'- {review} (ID: {artifact}, Size: 1721844, Expected Digest: {expected})',
        f"Downloading artifact '{artifact}' from '{REPO}'",
        f'SHA256 digest of downloaded artifact is {(downloaded or expected).removeprefix("sha256:")}',
        'Artifact download completed successfully.', 'Total of 1 artifact(s) downloaded',
    ] if review else []
    lines = [
        "Current runner version: '2.338.2'",
        f'##[group]Run actions/checkout@{pin}', 'with:', '  ref: main', '  path: trusted', '##[endgroup]',
        f'##[group]Run actions/setup-node@{pin}', 'with:', '  node-version: 24', '##[endgroup]',
        f'##[group]Run actions/download-artifact@{pin}', 'with:', '  name: key-scan-input',
        f'  run-id: {SCAN}', f'  repository: {REPO}', '##[endgroup]',
        f'- key-scan-input (ID: 1, Size: 10, Expected Digest: sha256:{"c" * 64})',
        f'SHA256 digest of downloaded artifact is {"c" * 64}', 'Artifact download completed successfully.',
        *download,
        '##[group]Run set -o pipefail', '\x1b[36;1mset -o pipefail\x1b[0m', '##[endgroup]',
        f'figma-key-guard at {"d" * 40}', 'result: PASS', *after,
    ]
    return '\ufeff' + ''.join(f'2026-11-20T20:23:33.7791372Z {line}\n' for line in lines)


def scan_jobs(key_scan='success', review_download='success'):
    steps = [('Set up job', 'success'), ('Run actions/download-artifact@x', 'success'),
             ('Run actions/download-artifact@x', review_download), ('Scan for the Figma file key', 'success'),
             ('Complete job', 'success')]
    return [{'id': KEY_JOB - 1, 'name': 'prepare', 'conclusion': 'success', 'steps': []},
            {'id': KEY_JOB, 'name': 'key-scan', 'conclusion': key_scan,
             'steps': [{'name': name, 'conclusion': conclusion} for name, conclusion in steps]}]


def pull_request(state='open', head=HEAD, repo=REPO):
    return {'number': PULL, 'state': state, 'base': {'repo': {'full_name': REPO}},
            'head': {'sha': head, 'ref': BRANCH, 'repo': {'full_name': repo}}}


def artifact(id=ARTIFACT, name=f'ui-review-{HEAD}', run=RUN, digest=DIGEST, expired=False):
    return {'id': id, 'name': name, 'digest': digest, 'expired': expired,
            'workflow_run': {'id': run, 'head_sha': HEAD, 'repository_id': REPO_ID, 'head_repository_id': REPO_ID}}


def world():
    """The API answers for a pull request run whose Review artifact the key scan downloaded and cleared."""
    runs = f'repos/{REPO}/actions/runs'
    return {
        f'repos/{REPO}/environments/ui-gallery':
            {'deployment_branch_policy': {'protected_branches': False, 'custom_branch_policies': True}},
        f'repos/{REPO}/environments/ui-gallery/deployment-branch-policies':
            {'branch_policies': [{'name': 'main', 'type': 'branch'}]},
        f'repos/{REPO}/actions/workflows/ui-library-key-scan.yml': {'id': 11},
        f'repos/{REPO}/actions/workflows/ui-library.yml': {'id': 10},
        f'{runs}/{SCAN}': {'id': SCAN, 'path': '.github/workflows/ui-library-key-scan.yml', 'workflow_id': 11,
                           'event': 'workflow_run', 'head_branch': 'main', 'head_repository': {'full_name': REPO},
                           'status': 'completed', 'conclusion': 'success', 'run_attempt': ATTEMPT},
        f'{runs}/{SCAN}/attempts/{ATTEMPT}/jobs': scan_jobs(),
        f'repos/{REPO}/actions/jobs/{KEY_JOB}/logs': scan_log(),
        f'{runs}/{RUN}': {'id': RUN, 'path': '.github/workflows/ui-library.yml', 'workflow_id': 10,
                          'event': 'pull_request', 'head_sha': HEAD, 'head_branch': BRANCH,
                          'head_repository': {'full_name': REPO}, 'status': 'completed', 'conclusion': 'success'},
        f'{runs}/{RUN}/jobs': [{'name': 'key-guard', 'conclusion': 'success'},
                               {'name': 'library', 'conclusion': 'success'}],
        f'repos/{REPO}/pulls': [pull_request()],
        f'repos/{REPO}/actions/artifacts/{ARTIFACT}': artifact(),
        f'{runs}/{RUN}/artifacts': [artifact()],
    }


class FakeGitHub:
    def __init__(self, responses, archive=b''):
        self.responses, self.archive, self.calls = responses, archive, []

    def _answer(self, method, path, values):
        self.calls.append((method, path, values))
        if path not in self.responses:
            raise publish.ApiError(path, 404, 'HTTP 404: Not Found')
        return copy.deepcopy(self.responses[path])

    def get(self, path, **query):
        return self._answer('GET', path, query)

    def pages(self, path, key=None, **query):
        return self._answer('GET', path, query)

    def text(self, path):
        return self._answer('GET', path, {})

    def send(self, method, path, **fields):
        self.calls.append((method, path, fields))
        return {}

    def download(self, path, target, limit):
        self.calls.append(('DOWNLOAD', path, {}))
        Path(target).write_bytes(self.archive)


def environment(**changes):
    env = {'REPOSITORY': REPO, 'REPOSITORY_ID': str(REPO_ID), 'DEFAULT_BRANCH': 'main', 'SCAN_RUN_ID': str(SCAN),
           'SCAN_RUN_ATTEMPT': str(ATTEMPT), **VARIABLES, **changes}
    return {name: value for name, value in env.items() if value is not None}


class VerifyTest(unittest.TestCase):
    def verify(self, change=lambda responses: None, **env):
        responses = world()
        change(responses)
        with patch('sys.stdout', new=io.StringIO()):
            return publish.verify(environment(**env), FakeGitHub(responses))

    def test_names_the_artifact_the_scan_downloaded_for_the_open_pull_request_at_its_head(self):
        self.assertEqual(self.verify(), {'publish': 'true', 'pr': PULL, 'head': HEAD, 'run': RUN,
                                         'artifact': ARTIFACT, 'digest': DIGEST})

    def test_skips_before_any_api_call_while_a_variable_is_missing(self):
        for name in VARIABLES:
            for value in ('', None, '  '):
                with self.subTest(name=name, value=value):
                    github = FakeGitHub(world())
                    with self.assertRaisesRegex(publish.Skip, f'not configured.*{name}'):
                        publish.verify(environment(**{name: value}), github)
                    self.assertEqual(github.calls, [])

    def test_refuses_a_malformed_variable(self):
        for name, value in (('UI_GALLERY_BASE_URL', 'https://ui-gallery.example.com/'),
                            ('UI_GALLERY_BASE_URL', 'http://ui-gallery.example.com'),
                            ('UI_GALLERY_BASE_URL', 'https://ui-gallery.example.com/path'),
                            ('UI_GALLERY_ROLE_ARN', 'ui-gallery-writer'), ('UI_GALLERY_BUCKET', 'Example_Bucket'),
                            ('UI_GALLERY_BUCKET', ' example-ui-gallery'), ('UI_GALLERY_AWS_REGION', 'us east 1')):
            with self.subTest(name=name, value=value), self.assertRaisesRegex(publish.Reject, name):
                self.verify(**{name: value})

    def test_skips_until_the_environment_allows_main_only(self):
        environment_path = f'repos/{REPO}/environments/ui-gallery'
        cases = {
            'missing': lambda r: r.pop(environment_path),
            'every branch': lambda r: r[environment_path].update(deployment_branch_policy=None),
            'protected branches': lambda r: r[environment_path].update(
                deployment_branch_policy={'protected_branches': True, 'custom_branch_policies': False}),
            'another branch too': lambda r: r[f'{environment_path}/deployment-branch-policies'].update(
                branch_policies=[{'name': 'main', 'type': 'branch'}, {'name': 'release/*', 'type': 'branch'}]),
        }
        for case, change in cases.items():
            with self.subTest(case=case), self.assertRaisesRegex(publish.Skip, 'ui-gallery environment'):
                self.verify(change)

    def test_skips_a_key_scan_that_did_not_succeed(self):
        with self.assertRaisesRegex(publish.Skip, 'concluded failure'):
            self.verify(lambda r: r[f'repos/{REPO}/actions/runs/{SCAN}'].update(conclusion='failure'))

    def test_skips_a_key_scan_that_read_no_review_artifact(self):
        for jobs in (scan_jobs(key_scan='skipped'), scan_jobs(review_download='skipped')):
            with self.subTest(jobs=jobs[1]), self.assertRaises(publish.Skip):
                self.verify(lambda r: r.update({f'repos/{REPO}/actions/runs/{SCAN}/attempts/{ATTEMPT}/jobs': jobs}))

    def test_skips_a_trigger_that_is_not_the_default_branch_key_scan(self):
        for change in ({'path': '.github/workflows/other.yml'}, {'workflow_id': 12}, {'event': 'pull_request'},
                       {'head_branch': BRANCH}, {'head_repository': {'full_name': 'outside/fork'}}):
            with self.subTest(change=change), self.assertRaisesRegex(publish.Skip, 'triggering'):
                self.verify(lambda r: r[f'repos/{REPO}/actions/runs/{SCAN}'].update(change))

    def test_leaves_publication_to_a_newer_attempt_of_the_key_scan(self):
        for change in ({'run_attempt': ATTEMPT + 1}, {'status': 'in_progress', 'conclusion': None}):
            with self.subTest(change=change), self.assertRaisesRegex(publish.Skip, 'newer attempt'):
                self.verify(lambda r: r[f'repos/{REPO}/actions/runs/{SCAN}'].update(change))

    def test_skips_a_fork_pull_request(self):
        with self.assertRaisesRegex(publish.Skip, 'fork'):
            self.verify(lambda r: r[f'repos/{REPO}/actions/runs/{RUN}'].update(
                head_repository={'full_name': 'outside/fork'}))

    def test_skips_a_run_that_no_pull_request_triggered(self):
        with self.assertRaisesRegex(publish.Skip, 'triggered by push'):
            self.verify(lambda r: r[f'repos/{REPO}/actions/runs/{RUN}'].update(event='push'))

    def test_skips_a_run_whose_ui_changed_gate_skipped_the_library(self):
        with self.assertRaisesRegex(publish.Skip, 'no UI change'):
            self.verify(lambda r: r[f'repos/{REPO}/actions/runs/{RUN}/jobs'][1].update(conclusion='skipped'))

    def test_skips_a_closed_pull_request(self):
        for pulls in ([pull_request(state='closed')], []):
            with self.subTest(pulls=pulls), self.assertRaisesRegex(publish.Skip, 'no open pull request'):
                self.verify(lambda r: r.update({f'repos/{REPO}/pulls': pulls}))

    def test_skips_a_pull_request_that_moved_on_to_another_head(self):
        with self.assertRaisesRegex(publish.Skip, f'#{PULL} moved on to {"c" * 40}'):
            self.verify(lambda r: r.update({f'repos/{REPO}/pulls': [pull_request(head='c' * 40)]}))

    def test_refuses_an_artifact_id_the_scan_did_not_download(self):
        held = f'repos/{REPO}/actions/runs/{RUN}/artifacts'
        cases = {
            'the run holds another artifact': lambda r: r.update({held: [artifact(id=ARTIFACT + 1)]}),
            'the run holds a second artifact': lambda r: r[held].append(artifact(id=ARTIFACT + 1)),
            'the id belongs to another run': lambda r: r.update(
                {f'repos/{REPO}/actions/artifacts/{ARTIFACT}': artifact(run=RUN + 1)}),
            'the id names another commit': lambda r: r.update(
                {f'repos/{REPO}/actions/artifacts/{ARTIFACT}': artifact(name=f'ui-review-{"c" * 40}')}),
        }
        for case, change in cases.items():
            with self.subTest(case=case), self.assertRaisesRegex(publish.Reject, 'artifact ID mismatch'):
                self.verify(change)

    def test_refuses_a_digest_that_differs_from_what_the_scan_downloaded(self):
        other = 'sha256:' + 'e' * 64
        cases = {
            'the artifact API reports another digest': lambda r: r.update(
                {f'repos/{REPO}/actions/artifacts/{ARTIFACT}': artifact(digest=other)}),
            'the scan downloaded other bytes': lambda r: r.update(
                {f'repos/{REPO}/actions/jobs/{KEY_JOB}/logs': scan_log(downloaded=other)}),
        }
        for case, change in cases.items():
            with self.subTest(case=case), self.assertRaisesRegex(publish.Reject, 'digest mismatch'):
                self.verify(change)

    def test_reads_the_review_download_only_from_the_steps_before_the_key_step(self):
        forged = [f'##[group]Run actions/download-artifact@{"9" * 40}', 'with:', f'  name: ui-review-{"c" * 40}',
                  f'  run-id: {RUN + 1}', '##[endgroup]',
                  f'- ui-review-{"c" * 40} (ID: 1, Size: 1, Expected Digest: sha256:{"f" * 64})',
                  f'SHA256 digest of downloaded artifact is {"f" * 64}', 'Artifact download completed successfully.']
        # Printed by the key step after the real download, it changes nothing.
        self.assertEqual(publish.scanned_artifact(scan_log(after=forged), REPO),
                         {'head': HEAD, 'run': RUN, 'artifact': ARTIFACT, 'digest': DIGEST})
        # Without the real download, it is not evidence of one.
        with self.assertRaisesRegex(publish.Reject, '0 Review downloads'):
            publish.scanned_artifact(scan_log(review=None, after=forged), REPO)


def archive(entries):
    """A zip of the given entries: name -> bytes, or name -> (bytes, unix mode)."""
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, 'w') as bundle:
        for name, content in entries.items():
            content, mode = content if isinstance(content, tuple) else (content, stat.S_IFREG | 0o644)
            info = zipfile.ZipInfo(name)
            info.external_attr = mode << 16
            bundle.writestr(info, content)
    return buffer.getvalue()


ENTRIES = {'index.html': b'review', 'gallery/index.html': b'gallery', 'dcl-ui-review/x.js': b'review',
           'hosted/index.html': b'<!doctype html>', 'hosted/frame.html': b'<!doctype html>',
           'hosted/assets/index-B8BLG4hy.js': b'export {}', 'hosted/assets/inter-latin.woff2': b'font'}


class FetchTest(unittest.TestCase):
    def fetch(self, entries=ENTRIES, digest=None, api=None):
        data = archive(entries)
        cleared = digest or 'sha256:' + hashlib.sha256(data).hexdigest()
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        work = self.root / 'ui-gallery'
        env = {'REPOSITORY': REPO, 'HEAD_SHA': HEAD, 'SCANNED_RUN_ID': str(RUN), 'ARTIFACT_ID': str(ARTIFACT),
               'ARTIFACT_DIGEST': cleared, 'WORK_DIR': str(work)}
        responses = {f'repos/{REPO}/actions/artifacts/{ARTIFACT}': api or artifact(digest=cleared)}
        with patch('sys.stdout', new=io.StringIO()):
            publish.fetch(env, FakeGitHub(responses, data))
        return work

    def test_unpacks_only_hosted_from_the_cleared_bytes(self):
        work = self.fetch()
        self.assertEqual(publish.site_files(work / 'site'),
                         ['assets/index-B8BLG4hy.js', 'assets/inter-latin.woff2', 'frame.html', 'index.html'])

    def test_refuses_bytes_that_differ_from_the_cleared_digest(self):
        with self.assertRaisesRegex(publish.Reject, 'digest mismatch: the downloaded artifact'):
            self.fetch(digest='sha256:' + 'e' * 64)

    def test_refuses_an_artifact_the_api_describes_differently(self):
        for api, problem in ((artifact(digest='sha256:' + 'e' * 64), 'digest mismatch'),
                             (artifact(run=RUN + 1), 'artifact ID mismatch'),
                             (artifact(name='ui-review-other'), 'artifact ID mismatch')):
            with self.subTest(api=api), self.assertRaisesRegex(publish.Reject, problem):
                self.fetch(digest=DIGEST, api=api)

    def test_refuses_an_artifact_without_hosted_index(self):
        entries = {name: content for name, content in ENTRIES.items() if name != 'hosted/index.html'}
        with self.assertRaisesRegex(publish.Reject, 'no hosted/index.html'):
            self.fetch(entries)

    def test_refuses_an_unsafe_or_unknown_entry_and_writes_nothing_outside_the_site(self):
        for name, content in (('../escape.js', b'x'), ('hosted/../../escape.js', b'x'), ('/hosted/abs.js', b'x'),
                              ('hosted/assets\\x.js', b'x'), ('hosted/run.sh', b'x'), ('hosted/.env', b'x'),
                              ('hosted/assets/link.js', (b'/etc/passwd', stat.S_IFLNK | 0o777)), ('hosted', b'x')):
            with self.subTest(name=name):
                with self.assertRaises(publish.Reject):
                    self.fetch({**ENTRIES, name: content})
                written = {path.relative_to(self.root).as_posix() for path in self.root.rglob('*') if path.is_file()}
                self.assertEqual({path for path in written if not path.startswith('ui-gallery/site/')},
                                 {'ui-gallery/artifact.zip'})


def site(root, files=('index.html', 'frame.html', 'assets/index-B8BLG4hy.js', 'assets/index-BqURr6sZ.css',
                      'assets/inter-latin.woff2')):
    for path in files:
        (root / 'site' / path).parent.mkdir(parents=True, exist_ok=True)
        (root / 'site' / path).write_bytes(b'x')
    return root


class UploadTest(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.work = site(Path(temp.name))
        self.env = {'PR_NUMBER': str(PULL), 'HEAD_SHA': HEAD, 'WORK_DIR': str(self.work), **VARIABLES}

    def test_copies_each_type_with_its_content_type_and_cache_header(self):
        commands = []
        with patch('sys.stdout', new=io.StringIO()):
            outputs = publish.upload(self.env, today=datetime.date(2026, 11, 20),
                                     run=lambda command: commands.append(command) or SimpleNamespace(returncode=0))
        self.assertEqual(outputs, {'expires': '2026-11-25'})
        typed = {command[command.index('--include') + 1]: (command[command.index('--content-type') + 1],
                                                           command[command.index('--cache-control') + 1])
                 for command in commands}
        self.assertEqual(typed, {
            '*.css': ('text/css; charset=utf-8', publish.ASSET_CACHE),
            '*.html': ('text/html; charset=utf-8', publish.PAGE_CACHE),
            '*.js': ('text/javascript; charset=utf-8', publish.ASSET_CACHE),
            '*.woff2': ('font/woff2', publish.ASSET_CACHE),
        })
        for command in commands:
            self.assertEqual(command[:6], ['aws', 's3', 'cp', str(self.work / 'site'),
                                           f's3://example-ui-gallery/ui-gallery/pr/{PULL}/{HEAD}/', '--recursive'])
            self.assertEqual(command[6:8], ['--exclude', '*'])
            # Put only: no ACL, no listing (sync) and no deletion.
            self.assertFalse({'--acl', '--delete', 'sync', 'rm'} & set(command))

    def test_fails_when_a_copy_fails(self):
        with self.assertRaisesRegex(publish.Reject, 'upload of the .css files'):
            publish.upload(self.env, run=lambda command: SimpleNamespace(returncode=1))


class ProbeTest(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.env = {'PR_NUMBER': str(PULL), 'HEAD_SHA': HEAD, 'WORK_DIR': str(site(Path(temp.name))), **VARIABLES}

    def probe(self, answers):
        requested, sleeps = [], []
        answers = iter(answers)

        def request(url):
            requested.append(url)
            return next(answers)
        with patch('sys.stdout', new=io.StringIO()) as printed:
            publish.probe(self.env, request=request, sleep=sleeps.append)
        return requested, sleeps, printed.getvalue()

    def test_waits_until_the_page_and_an_asset_answer(self):
        robots = 'noindex, nofollow'
        requested, sleeps, printed = self.probe([(403, robots), (403, robots), (200, robots), (200, robots)])
        self.assertEqual(requested[:2], [PAGE, PAGE.replace('index.html', 'assets/index-B8BLG4hy.js')])
        self.assertEqual(sleeps, [5])
        self.assertNotIn('::warning', printed)

    def test_fails_when_the_preview_never_answers(self):
        with self.assertRaisesRegex(publish.Reject, 'did not answer 200'):
            self.probe([(403, '')] * 2 * (len(publish.PROBE_DELAYS) + 1))

    def test_warns_when_the_preview_lets_robots_index_it(self):
        _, _, printed = self.probe([(200, ''), (200, '')])
        self.assertIn('::warning title=UI gallery::', printed)


class CommentTest(unittest.TestCase):
    def comment(self, comments=(), pull=None):
        github = FakeGitHub({f'repos/{REPO}/pulls/{PULL}': pull or pull_request(),
                             f'repos/{REPO}/issues/{PULL}/comments': list(comments)})
        env = {'REPOSITORY': REPO, 'PR_NUMBER': str(PULL), 'HEAD_SHA': HEAD, 'EXPIRES': '2026-11-25', **VARIABLES}
        with patch('sys.stdout', new=io.StringIO()):
            publish.comment(env, github)
        return [call for call in github.calls if call[0] in ('POST', 'PATCH')]

    def test_creates_one_comment_that_opens_the_gallery_on_its_first_line(self):
        [(method, path, fields)] = self.comment()
        self.assertEqual((method, path), ('POST', f'repos/{REPO}/issues/{PULL}/comments'))
        lines = [line for line in fields['body'].splitlines() if line and not line.startswith('<!--')]
        self.assertEqual(lines, [f'[Open the UI gallery]({PAGE})', 'Built from aaaaaaa, kept until 2026-11-25 (UTC).'])
        self.assertIn(publish.MARKER, fields['body'])

    def test_updates_its_own_comment_and_never_another_one_with_the_marker(self):
        writes = self.comment([{'id': 1, 'user': {'login': 'someone'}, 'body': publish.MARKER},
                               {'id': 2, 'user': {'login': publish.BOT}, 'body': f'old {publish.MARKER}'}])
        self.assertEqual([(method, path) for method, path, _ in writes], [('PATCH', f'repos/{REPO}/issues/comments/2')])

    def test_leaves_an_unchanged_comment_alone(self):
        body = publish.render(PAGE, HEAD, '2026-11-25')
        self.assertEqual(self.comment([{'id': 2, 'user': {'login': publish.BOT}, 'body': body}]), [])

    def test_skips_a_pull_request_that_closed_or_moved_on(self):
        for pull in (pull_request(state='closed'), pull_request(head='c' * 40)):
            with self.subTest(pull=pull), self.assertRaises(publish.Skip):
                self.comment(pull=pull)


class MainTest(unittest.TestCase):
    def run_main(self, env, command='verify'):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        output = Path(temp.name) / 'output'
        output.write_text('')
        with patch('sys.stdout', new=io.StringIO()) as printed:
            code = publish.main([command], {**env, 'GITHUB_OUTPUT': str(output)})
        return code, output.read_text(), printed.getvalue()

    def test_an_ineligible_run_succeeds_with_a_notice_and_publishes_nothing(self):
        code, outputs, printed = self.run_main(environment(UI_GALLERY_ROLE_ARN=''))
        self.assertEqual((code, outputs), (0, 'publish=false\n'))
        self.assertIn('::notice title=UI gallery::No UI gallery preview: publication is not configured', printed)

    def test_mismatched_evidence_fails_with_an_error(self):
        def reject(env):
            raise publish.Reject('digest mismatch: x')
        with patch.dict(publish.COMMANDS, verify=reject):
            code, outputs, printed = self.run_main(environment())
        self.assertEqual((code, outputs), (1, ''))
        self.assertIn('::error title=UI gallery::UI gallery preview refused: digest mismatch: x', printed)


class WorkflowTest(unittest.TestCase):
    """ui-gallery-publish.yml holds the writer role: its shape is part of what the role's trust policy relies on."""
    text = (ROOT / '.github/workflows/ui-gallery-publish.yml').read_text()

    def jobs(self):
        body = self.text.split('\njobs:\n', 1)[1]
        return dict(re.findall(r'(?ms)^  ([\w-]+):\n(.*?)(?=^  [\w-]+:\n|\Z)', body))

    def test_runs_after_the_key_scan_it_names_with_no_default_permission(self):
        scan = (ROOT / '.github/workflows/ui-library-key-scan.yml').read_text()
        name = re.search(r'(?m)^name: (.+)$', scan).group(1)
        self.assertIn(f'on:\n  workflow_run:\n    workflows: ["{name}"]\n    types: [completed]\n\npermissions: {{}}\n',
                      self.text)

    def test_gives_the_token_the_environment_and_comment_writes_to_one_job_each(self):
        jobs = self.jobs()
        self.assertEqual(list(jobs), ['verify', 'publish', 'comment'])
        holding = lambda text: [name for name, body in jobs.items() if text in body]
        self.assertEqual(holding('id-token: write'), ['publish'])
        self.assertEqual(holding('environment: ui-gallery'), ['publish'])
        self.assertEqual(holding('pull-requests: write'), ['comment'])
        self.assertEqual(self.text.count('environment:'), 1)
        publish_job = jobs['publish']
        self.assertLess(publish_job.index('ui-gallery-publish.py fetch'),
                        publish_job.index('aws-actions/configure-aws-credentials@'))

    def test_runs_only_the_default_branch_script_with_values_from_env(self):
        # Step scripts only: a job output may also be called run.
        runs = re.findall(r'(?m)^ {6}(?:- | {2})run: ?(.*)$', self.text)
        self.assertEqual(sorted(runs), sorted(f'python3 -I tooling/ui-gallery-publish.py {command}'
                                              for command in ('verify', 'fetch', 'upload', 'probe', 'comment')))
        for uses in re.findall(r'(?m)^ +- uses: (\S+)', self.text):
            self.assertRegex(uses, r'@[0-9a-f]{40}$')
        checkouts = re.findall(r'(?ms)- uses: actions/checkout@\S+ # \S+\n(.*?)(?=\n      - )', self.text)
        self.assertEqual(len(checkouts), 3)
        for checkout in checkouts:
            self.assertIn('ref: ${{ github.workflow_sha }}', checkout)
            self.assertIn('persist-credentials: false', checkout)


if __name__ == '__main__':
    unittest.main()
