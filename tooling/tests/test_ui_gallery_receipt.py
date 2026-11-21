"""An image is published only with a successful key scan of the UI gallery bytes it ships."""
import importlib.util
import io
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import unittest
import unittest.mock
import urllib.error
import urllib.request
import zipfile
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('ui_gallery_receipt', ROOT / 'tooling/ui-gallery-receipt.py')
receipt = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(receipt)

FILES = {'index.html': b'<!doctype html>', 'hud.html': b'hud', 'assets/index-a1.js': b'export {}'}
HEAD, PARENT, GRANDPARENT = 'c' * 40, 'b' * 40, 'a' * 40
STARTED = '2026-11-21T00:00:00Z'


def archive(files, prefix='ui-gallery/'):
    data = io.BytesIO()
    with zipfile.ZipFile(data, 'w') as zipped:
        zipped.writestr('index.html', b'generated review')
        for name, content in files.items():
            zipped.writestr(prefix + name, content)
    return data.getvalue()


def scan_log(sha, artifact_id, ran=True):
    return (f'2026-11-21T00:10:00.0Z   HEAD_SHA: {sha}\n'
            f'2026-11-21T00:10:01.0Z - ui-review-{sha} (ID: {artifact_id}, Size: 1, Expected Digest: sha256:00)\n'
            + ('2026-11-21T00:10:02.0Z check key: ran\n' if ran else ''))


NOW = datetime(2026, 11, 21, 0, 30, tzinfo=timezone.utc)


class FakeGitHub:
    """Main history GRANDPARENT <- PARENT <- HEAD. `inputs` lists the commits that changed a gallery input, newest
    first; each may have a Shared UI library push run (id = commit index), its ui-review artifact and key-scan runs."""

    def __init__(self):
        self.inputs = [HEAD, PARENT, GRANDPARENT]
        self.library = {}        # sha -> library job conclusion
        self.run_status = {}     # sha -> run status, 'completed' by default
        self.artifacts = {}      # sha -> zip bytes
        self.artifact_run = {}   # artifact id -> uploading run id, when not the commit's push run
        self.scans = []          # key-scan runs
        self.unreadable = set()  # key-scan job ids whose log cannot be fetched
        self.scan_pages = []     # page numbers requested for scan lists

    @staticmethod
    def run_id(sha):
        return {HEAD: 3, PARENT: 2, GRANDPARENT: 1}[sha]

    def scan(self, sha, conclusion='success', **fields):
        self.scans.append({'id': 100 + len(self.scans), 'sha': sha, 'conclusion': conclusion,
                           'created': '2026-11-21T00:05:00Z', 'path': receipt.SCAN_WORKFLOW, 'event': 'workflow_run',
                           'branch': 'main', 'ran': True, 'artifact': 50 + self.run_id(sha), **fields})

    def api(self, path):
        if path == '':
            return {'default_branch': 'main'}
        if match := re.fullmatch(r'commits\?sha=(\w+)&path=([^&]+)&per_page=100', path):
            return [{'sha': sha, 'commit': {'committer': {'date': f'2026-11-2{3 - i}T00:00:00Z'}}}
                    for i, sha in enumerate(self.inputs)]
        if match := re.fullmatch(r'actions/artifacts/(\d+)', path):
            artifact = int(match[1])
            sha = next((sha for sha in self.artifacts if 50 + self.run_id(sha) == artifact), None)
            if sha is None:
                raise receipt.FetchError(f'GET {path} failed: 404')
            run = self.artifact_run.get(artifact, self.run_id(sha))
            return {'name': f'ui-review-{sha}', 'expired': False, 'workflow_run': {'id': run}}
        raise KeyError(path)

    def pages(self, path, key):
        if match := re.fullmatch(r'actions/workflows/ui-library\.yml/runs\?head_sha=(\w+)&event=push&per_page=100', path):
            sha = match[1]
            return [] if sha not in self.library else [
                {'id': self.run_id(sha), 'head_sha': sha, 'event': 'push', 'path': receipt.LIBRARY_WORKFLOW,
                 'status': self.run_status.get(sha, 'completed'), 'conclusion': 'success', 'run_started_at': STARTED,
                 'updated_at': '2026-11-21T00:04:00Z'}]
        if match := re.fullmatch(r'actions/runs/(\d)/jobs\?per_page=100', path):
            sha = next(sha for sha in (HEAD, PARENT, GRANDPARENT) if self.run_id(sha) == int(match[1]))
            return [{'name': 'library', 'conclusion': self.library[sha]}]
        if path.startswith('actions/workflows/ui-library-key-scan.yml/runs?'):
            self.scan_pages.append(path)
            return [{'id': scan['id'], 'path': scan['path'], 'event': scan['event'], 'head_branch': scan['branch'],
                     'created_at': scan['created']} for scan in self.scans]
        if match := re.fullmatch(r'actions/runs/(1\d\d)/jobs\?per_page=100', path):
            scan = self.scans[int(match[1]) - 100]
            return [{'id': scan['id'], 'name': 'key-scan', 'status': 'completed', 'conclusion': scan['conclusion']}]
        raise KeyError(path)

    def text(self, path):
        job = int(re.fullmatch(r'actions/jobs/(\d+)/logs', path)[1])
        scan = self.scans[job - 100]
        if job in self.unreadable or scan['conclusion'] == 'skipped':
            raise receipt.FetchError(f'GET {path} failed')
        return scan_log(scan['sha'], scan['artifact'], scan['ran'])

    def download(self, artifact_id):
        return next(data for sha, data in self.artifacts.items() if 50 + self.run_id(sha) == artifact_id)


class ReceiptTest(unittest.TestCase):
    def setUp(self):
        self.gh = FakeGitHub()
        self.digest = receipt.artifact_digest(archive(FILES))

    def built(self, sha, files=FILES):
        self.gh.library[sha] = 'success'
        self.gh.artifacts[sha] = archive(files)

    def require(self, now=NOW):
        return receipt.require_receipt(self.gh, HEAD, self.digest, wait=0, sleep=lambda _: None, now=lambda: now)

    def test_the_trusted_head_scan_of_the_same_bytes_is_the_receipt(self):
        self.built(HEAD)
        self.gh.scan(HEAD)
        self.assertEqual(self.require(), HEAD)

    def test_a_failed_head_scan_fails_without_looking_further(self):
        self.built(HEAD)
        self.gh.scan(HEAD, 'failure')
        self.built(PARENT)
        self.gh.scan(PARENT)
        with self.assertRaisesRegex(receipt.ReceiptError, 'concluded failure'):
            self.require()

    def test_a_head_scan_of_other_bytes_fails_and_says_how_to_reconcile(self):
        self.built(HEAD, {**FILES, 'hud.html': b'other'})
        self.gh.scan(HEAD)
        with self.assertRaisesRegex(receipt.ReceiptError, 'but the image ships .*setup-node .*nodejs-slim'):
            self.require()

    def test_a_scan_whose_artifact_is_gone_fails(self):
        self.gh.library[HEAD] = 'success'
        self.gh.scan(HEAD)
        with self.assertRaises(receipt.FetchError):
            self.require()

    def test_an_artifact_without_the_gallery_fails(self):
        self.gh.library[HEAD] = 'success'
        self.gh.artifacts[HEAD] = archive(FILES, prefix='gallery/')
        self.gh.scan(HEAD)
        with self.assertRaisesRegex(receipt.ReceiptError, 'holds no ui-gallery/'):
            self.require()

    def test_a_scan_that_never_read_the_artifact_fails(self):
        self.built(HEAD)
        self.gh.scan(HEAD, ran=False)
        with self.assertRaisesRegex(receipt.ReceiptError, 'without reading the artifact'):
            self.require()

    def test_a_missing_head_scan_is_waited_for_within_the_window(self):
        self.built(HEAD)
        with self.assertRaisesRegex(receipt.ReceiptError, 'did not finish'):
            self.require()

    def test_a_running_library_run_is_waited_for(self):
        self.built(HEAD)
        self.gh.run_status[HEAD] = 'in_progress'
        self.gh.scan(HEAD)
        with self.assertRaisesRegex(receipt.ReceiptError, 'did not finish'):
            self.require()

    def test_a_foreign_scan_fails_closed(self):
        # A run of another workflow, branch or event proves nothing, and no check run is read.
        self.built(HEAD)
        for fields in ({'path': '.github/workflows/forged.yml'}, {'branch': 'feat/forged'}, {'event': 'push'}):
            with self.subTest(**fields):
                self.gh.scans = []
                self.gh.scan(HEAD, **fields)
                with self.assertRaisesRegex(receipt.ReceiptError, 'did not finish|no trusted key scan'):
                    self.require()

    def test_a_stale_scan_of_an_earlier_attempt_fails_closed(self):
        self.built(HEAD)
        self.gh.scan(HEAD, created='2026-11-20T23:00:00Z')
        with self.assertRaisesRegex(receipt.ReceiptError, 'did not finish'):
            self.require()

    def test_the_newest_trusted_scan_decides(self):
        self.built(HEAD)
        self.gh.scan(HEAD)
        self.gh.scan(HEAD, 'failure')
        with self.assertRaisesRegex(receipt.ReceiptError, 'concluded failure'):
            self.require()

    def test_a_pull_request_scan_of_the_same_commit_does_not_decide(self):
        # A PR whose head is this commit gets a scan of its own run's artifact; the push run's scan still counts.
        self.built(HEAD)
        self.gh.scan(HEAD)
        self.gh.scan(HEAD, 'failure', artifact=99)
        self.gh.artifact_run[99] = 77
        self.gh.api = self._with_artifact_99(self.gh.api)
        self.assertEqual(self.require(), HEAD)

    @staticmethod
    def _with_artifact_99(api):
        def wrapped(path):
            if path == 'actions/artifacts/99':
                return {'name': f'ui-review-{HEAD}', 'expired': False, 'workflow_run': {'id': 77}}
            return api(path)
        return wrapped

    def test_an_unreadable_log_of_a_newer_failing_scan_fails_instead_of_reaching_an_older_pass(self):
        self.built(HEAD)
        self.gh.scan(HEAD)
        self.gh.scan(HEAD, 'failure')
        self.gh.unreadable.add(self.gh.scans[-1]['id'])
        with self.assertRaises(receipt.FetchError):
            self.require()

    def test_an_unreadable_log_fails_the_ancestor_search_too(self):
        self.gh.library[HEAD] = 'skipped'
        self.built(PARENT)
        self.gh.scan(PARENT)
        self.gh.unreadable.add(self.gh.scans[-1]['id'])
        with self.assertRaises(receipt.FetchError):
            self.require()

    def test_a_skipped_scan_job_without_a_log_is_passed_over(self):
        self.built(HEAD)
        self.gh.scan(HEAD)
        self.gh.scan(HEAD, 'skipped')
        self.assertEqual(self.require(), HEAD)

    def test_an_unchanged_gallery_reuses_the_newest_trusted_scan_of_identical_bytes(self):
        self.gh.library[HEAD] = 'skipped'
        self.built(PARENT, {**FILES, 'hud.html': b'older'})
        self.gh.scan(PARENT)
        self.built(GRANDPARENT)
        self.gh.scan(GRANDPARENT)
        self.assertEqual(self.require(), GRANDPARENT)

    def test_an_input_commit_whose_scan_ended_long_ago_is_found_on_a_later_page(self):
        # The scan list is paginated and windowed on the run, so an old receipt is still found.
        self.gh.library[HEAD] = 'skipped'
        self.built(PARENT)
        self.gh.scan(PARENT)
        self.assertEqual(self.require(now=NOW.replace(month=12)), PARENT)
        self.assertTrue(all('created=2026-11-21T00:00:00Z..2026-11-21T01:04:00Z' in path
                            for path in self.gh.scan_pages))

    def test_an_input_commit_with_no_scan_after_its_window_is_passed_over(self):
        self.gh.library[HEAD] = 'skipped'
        self.built(PARENT)
        self.built(GRANDPARENT)
        self.gh.scan(GRANDPARENT)
        self.assertEqual(self.require(now=NOW.replace(month=12)), GRANDPARENT)

    def test_a_running_scan_of_the_newest_input_commit_is_waited_for(self):
        self.gh.library[HEAD] = 'skipped'
        self.built(PARENT)
        self.built(GRANDPARENT)
        self.gh.scan(GRANDPARENT)
        with self.assertRaisesRegex(receipt.ReceiptError, f'scan of {PARENT} did not finish'):
            self.require()

    def test_an_unchanged_gallery_never_reuses_a_failed_or_foreign_scan(self):
        self.gh.library[HEAD] = 'skipped'
        self.built(PARENT)
        self.gh.scan(PARENT, 'failure')
        self.built(GRANDPARENT)
        self.gh.scan(GRANDPARENT, path='.github/workflows/forged.yml')
        with self.assertRaisesRegex(receipt.ReceiptError, 'ancestors that last changed'):
            self.require(now=NOW.replace(month=12))

    def test_no_receipt_says_why_and_how_to_make_one(self):
        self.gh.library[HEAD] = 'skipped'
        with self.assertRaisesRegex(receipt.ReceiptError, 'kept 90 days.*makes a new one'):
            self.require()


class _Response(io.BytesIO):
    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()


class GitHubFetchTest(unittest.TestCase):
    """Every answer the receipt reads either parses or fails: none reads as "nothing found"."""

    def fetch(self, body=None, error=None, call='api'):
        gh = receipt.GitHub('decentraland/dcl-one', 'token')
        opened = unittest.mock.Mock(side_effect=error) if error else unittest.mock.Mock(return_value=_Response(body))
        with unittest.mock.patch.object(gh.opener, 'open', opened):
            return getattr(gh, call)('actions/jobs/1/logs' if call == 'text' else 'actions/artifacts/1')

    def test_a_failed_call_fails(self):
        error = urllib.error.HTTPError('https://api.github.com/x', 404, 'Not Found', {}, None)
        for call in ('api', 'text'):
            with self.subTest(call=call), self.assertRaisesRegex(receipt.FetchError, 'failed'):
                self.fetch(error=error, call=call)

    def test_an_empty_answer_fails(self):
        for call in ('api', 'text'):
            with self.subTest(call=call), self.assertRaisesRegex(receipt.FetchError, 'answered nothing'):
                self.fetch(b'', call=call)

    def test_malformed_json_fails(self):
        with self.assertRaisesRegex(receipt.FetchError, 'answered no JSON'):
            self.fetch(b'{"workflow_runs": [')

    def test_a_log_with_escape_sequences_is_read_whole(self):
        log = b'\x1b[36;1mrange=x\x1b[0m\n2026-11-21T00:10:00Z   HEAD_SHA: abc\n'
        self.assertEqual(self.fetch(log, call='text'), log.decode())

    def test_a_list_without_its_key_or_past_the_page_limit_fails(self):
        gh = receipt.GitHub('decentraland/dcl-one', 'token')
        with unittest.mock.patch.object(gh, 'api', return_value={'message': 'x'}):
            with self.assertRaisesRegex(receipt.FetchError, 'answered no jobs'):
                gh.pages('actions/runs/1/jobs?per_page=100', 'jobs')
        with unittest.mock.patch.object(gh, 'api', return_value={'jobs': [{}] * receipt.PAGE}):
            with self.assertRaisesRegex(receipt.FetchError, 'more than 2 pages'):
                gh.pages('actions/runs/1/jobs?per_page=100', 'jobs', max_pages=2)

    def test_the_token_does_not_follow_a_redirect_to_storage(self):
        handler = receipt._NoAuthOnRedirect()
        request = urllib.request.Request('https://api.github.com/repos/x/y/actions/jobs/1/logs',
                                         headers={'Authorization': 'Bearer token'})
        storage = handler.redirect_request(request, None, 302, 'Found', {}, 'https://results.blob.example/log')
        self.assertIsNone(storage.get_header('Authorization'))


class DigestTest(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp())
        for name, content in FILES.items():
            (self.dir / name).parent.mkdir(parents=True, exist_ok=True)
            (self.dir / name).write_bytes(content)

    def tearDown(self):
        shutil.rmtree(self.dir)

    def test_the_tree_and_the_artifact_of_the_same_bytes_agree(self):
        self.assertEqual(receipt.tree_digest(self.dir), receipt.artifact_digest(archive(FILES)))

    @unittest.skipUnless(shutil.which('node'), 'node is not installed')
    def test_the_gallery_script_computes_the_same_digest(self):
        script = ROOT / 'library/ui/gallery/scripts/hosted-digest.mjs'
        printed = subprocess.check_output(['node', str(script), str(self.dir)], text=True).strip()
        self.assertEqual(printed, receipt.tree_digest(self.dir))

    def test_a_hidden_path_fails_on_both_sides(self):
        (self.dir / 'assets/.map').write_bytes(b'{}')
        with self.assertRaisesRegex(receipt.ReceiptError, 'hidden'):
            receipt.tree_digest(self.dir)
        with self.assertRaisesRegex(receipt.ReceiptError, 'hidden'):
            receipt.artifact_digest(archive({**FILES, 'assets/.map': b'{}'}))

    def test_a_link_fails(self):
        (self.dir / 'link.html').symlink_to(self.dir / 'index.html')
        with self.assertRaisesRegex(receipt.ReceiptError, 'is a link'):
            receipt.tree_digest(self.dir)


class WorkflowTest(unittest.TestCase):
    def test_a_commit_changing_the_gallerys_nix_inputs_gets_a_scan(self):
        nix = (ROOT / 'infra/packages/ui-gallery.nix').read_text()
        roots = {'library/ui', 'infra/packages/ui-gallery.nix', 'flake.lock'}
        roots |= {f'library/{name}' for name in re.findall(r'root \+ /library/([\w-]+)', nix)}
        for workflow in ('ui-library.yml', 'ui-library-key-scan.yml'):
            text = (ROOT / '.github/workflows' / workflow).read_text()
            scope = re.search(r'--diff-merges=cc "\$range" -- ([^)\n]+)\)', text)[1].split()
            with self.subTest(workflow=workflow):
                self.assertTrue(roots <= set(scope), roots - set(scope))

    def test_the_scanned_build_and_the_nix_build_use_the_same_node_major(self):
        library = (ROOT / '.github/workflows/ui-library.yml').read_text().split('\n  library:\n', 1)[1]
        ci = re.search(r'node-version: (\d+)', library)[1]
        nix = set(re.findall(r'nodejs-slim_(\d+)', (ROOT / 'infra/packages/ui-gallery.nix').read_text()))
        self.assertEqual(nix, {ci})
        sites = (ROOT / 'infra/packages/sites.nix').read_text()
        self.assertIn(f'import ./ui-gallery.nix {{ inherit lib stdenv fetchPnpmDeps pnpmConfigHook pnpm nodejs-slim_{ci}; }}', sites)

    def test_every_build_that_can_publish_requires_the_receipt_before_its_smoke_test(self):
        images = (ROOT / '.github/workflows/apis-sites-images.yml').read_text()
        build = images.split('  build:\n', 1)[1].split('  production-ready:\n', 1)[0]
        step = build.split('      - name: Require a key scan of the UI gallery the Sites image ships\n', 1)[1]
        step = step.split('      - name:', 1)[0]
        self.assertIn("if: github.event_name == 'push' || (github.event_name == 'workflow_dispatch' && inputs.publish "
                      "&& github.ref == 'refs/heads/main')\n", step)
        self.assertIn('tooling/ui-gallery-receipt.py', step)
        self.assertIn('/build/client/ui-gallery', step)
        self.assertLess(build.index('Require a key scan of the UI gallery'), build.index('Test the exact images'))
        # The receipt reads runs, jobs, logs and artifacts (actions: read), never check runs.
        self.assertNotIn('checks:', images)


if __name__ == '__main__':
    unittest.main()
