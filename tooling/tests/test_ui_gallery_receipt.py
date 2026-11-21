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
import zipfile

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


class FakeGitHub:
    """Commits GRANDPARENT <- PARENT <- HEAD on main. Each may have a Shared UI library push run (run id = commit
    index), its ui-review artifact and key-scan runs of the trusted workflow, or forged ones."""

    def __init__(self):
        self.parents = {HEAD: PARENT, PARENT: GRANDPARENT, GRANDPARENT: None}
        self.library = {}     # sha -> library job conclusion
        self.run_status = {}  # sha -> run status, 'completed' by default
        self.artifacts = {}   # sha -> zip bytes
        self.scans = []       # key-scan runs: dict(id, sha, conclusion, created, path, event, branch, ran)
        self.unreadable = set()  # key-scan job ids whose log cannot be fetched

    @staticmethod
    def run_id(sha):
        return {HEAD: 3, PARENT: 2, GRANDPARENT: 1}[sha]

    def scan(self, sha, conclusion='success', **fields):
        self.scans.append({'id': 100 + len(self.scans), 'sha': sha, 'conclusion': conclusion, 'created': '2026-11-21T00:05:00Z',
                           'path': receipt.SCAN_WORKFLOW, 'event': 'workflow_run', 'branch': 'main', 'ran': True,
                           'artifact': 50 + self.run_id(sha), **fields})

    def api(self, path):
        if path == '':
            return {'default_branch': 'main'}
        if match := re.fullmatch(r'actions/workflows/ui-library\.yml/runs\?head_sha=(\w+)&event=push&per_page=100', path):
            sha = match[1]
            return {'workflow_runs': [] if sha not in self.library else [
                {'id': self.run_id(sha), 'head_sha': sha, 'event': 'push', 'path': receipt.LIBRARY_WORKFLOW,
                 'status': self.run_status.get(sha, 'completed'), 'conclusion': 'success', 'run_started_at': STARTED}]}
        if match := re.fullmatch(r'actions/runs/(\d)/jobs\?per_page=100', path):
            sha = next(sha for sha in self.parents if self.run_id(sha) == int(match[1]))
            return {'jobs': [{'name': 'library', 'conclusion': self.library[sha]}]}
        if path.startswith('actions/workflows/ui-library-key-scan.yml/runs?'):
            return {'workflow_runs': [
                {'id': scan['id'], 'path': scan['path'], 'event': scan['event'], 'head_branch': scan['branch'],
                 'created_at': scan['created']} for scan in self.scans]}
        if match := re.fullmatch(r'actions/runs/(1\d\d)/jobs\?per_page=100', path):
            scan = self.scans[int(match[1]) - 100]
            return {'jobs': [{'id': scan['id'], 'name': 'key-scan', 'status': 'completed', 'conclusion': scan['conclusion']}]}
        if match := re.fullmatch(r'actions/artifacts/(\d+)', path):
            sha = next((sha for sha in self.artifacts if 50 + self.run_id(sha) == int(match[1])), None)
            if sha is None:
                raise receipt.ReceiptError('artifact not found')
            return {'name': f'ui-review-{sha}', 'expired': False, 'workflow_run': {'id': self.run_id(sha)}}
        if match := re.fullmatch(r'commits/(\w+)', path):
            parent = self.parents[match[1]]
            return {'parents': [] if parent is None else [{'sha': parent}]}
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

    def require(self):
        return receipt.require_receipt(self.gh, HEAD, self.digest, wait=0, sleep=lambda _: None)

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
        with self.assertRaisesRegex(receipt.ReceiptError, 'artifact not found'):
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

    def test_a_missing_head_scan_fails_after_waiting(self):
        self.built(HEAD)
        with self.assertRaisesRegex(receipt.ReceiptError, 'no completed trusted key scan'):
            self.require()

    def test_a_running_library_run_is_waited_for(self):
        self.built(HEAD)
        self.gh.run_status[HEAD] = 'in_progress'
        self.gh.scan(HEAD)
        with self.assertRaisesRegex(receipt.ReceiptError, 'no completed trusted key scan'):
            self.require()

    def test_a_foreign_scan_fails_closed(self):
        # A check run or a run of another workflow, branch or event proves nothing.
        self.built(HEAD)
        for fields in ({'path': '.github/workflows/forged.yml'}, {'branch': 'feat/forged'}, {'event': 'push'}):
            with self.subTest(**fields):
                self.gh.scans = []
                self.gh.scan(HEAD, **fields)
                with self.assertRaisesRegex(receipt.ReceiptError, 'no completed trusted key scan'):
                    self.require()

    def test_a_stale_scan_of_an_earlier_attempt_fails_closed(self):
        self.built(HEAD)
        self.gh.scan(HEAD, created='2026-11-20T23:00:00Z')
        with self.assertRaisesRegex(receipt.ReceiptError, 'no completed trusted key scan'):
            self.require()

    def test_the_newest_trusted_scan_decides(self):
        self.built(HEAD)
        self.gh.scan(HEAD)
        self.gh.scan(HEAD, 'failure')
        with self.assertRaisesRegex(receipt.ReceiptError, 'concluded failure'):
            self.require()

    def test_an_unreadable_log_of_a_newer_failing_scan_fails_instead_of_reaching_an_older_pass(self):
        self.built(HEAD)
        self.gh.scan(HEAD)
        self.gh.scan(HEAD, 'failure')
        self.gh.unreadable.add(self.gh.scans[-1]['id'])
        with self.assertRaises(receipt.FetchError):
            self.require()

    def test_an_unreadable_log_fails_the_ancestor_walk_too(self):
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

    def test_an_unchanged_library_reuses_the_nearest_trusted_scan_of_identical_bytes(self):
        self.gh.library[HEAD] = 'skipped'
        self.built(PARENT, {**FILES, 'hud.html': b'older'})
        self.gh.scan(PARENT)
        self.built(GRANDPARENT)
        self.gh.scan(GRANDPARENT)
        self.assertEqual(self.require(), GRANDPARENT)

    def test_an_unchanged_library_never_reuses_a_failed_or_foreign_scan(self):
        self.gh.library[HEAD] = 'skipped'
        self.built(PARENT)
        self.gh.scan(PARENT, 'failure')
        self.built(GRANDPARENT)
        self.gh.scan(GRANDPARENT, path='.github/workflows/forged.yml')
        with self.assertRaisesRegex(receipt.ReceiptError, 'or its ancestors read the bytes'):
            self.require()

    def test_the_end_of_the_walk_says_why_and_how_to_make_a_receipt(self):
        self.gh.library[HEAD] = 'skipped'
        self.gh.parents = {HEAD: PARENT, PARENT: HEAD}
        with self.assertRaisesRegex(receipt.ReceiptError, 'last 200 first-parent ancestors.*kept 90 days.*flake.lock'):
            self.require()


class GitHubFetchTest(unittest.TestCase):
    """Every answer the receipt reads either parses or fails: none reads as "nothing found"."""

    def fetch(self, returncode, stdout, call='api'):
        result = subprocess.CompletedProcess([], returncode, stdout, b'')
        with unittest.mock.patch.object(receipt.subprocess, 'run', return_value=result):
            gh = receipt.GitHub('decentraland/dcl-one')
            return getattr(gh, call)('actions/jobs/1/logs' if call == 'text' else 'actions/artifacts/1')

    def test_a_failed_call_fails(self):
        for call in ('api', 'text', 'download'):
            with self.subTest(call=call), self.assertRaisesRegex(receipt.FetchError, 'exit 1'):
                self.fetch(1, b'{}', call if call != 'download' else 'api')
        with self.assertRaises(receipt.FetchError):
            result = subprocess.CompletedProcess([], 1, b'PK', b'')
            with unittest.mock.patch.object(receipt.subprocess, 'run', return_value=result):
                receipt.GitHub('decentraland/dcl-one').download(1)

    def test_an_empty_answer_fails(self):
        for call in ('api', 'text'):
            with self.subTest(call=call), self.assertRaisesRegex(receipt.FetchError, 'answered nothing'):
                self.fetch(0, b'', call)

    def test_malformed_json_fails(self):
        with self.assertRaisesRegex(receipt.FetchError, 'answered no JSON'):
            self.fetch(0, b'{"workflow_runs": [')

    def test_a_parsed_answer_is_returned(self):
        self.assertEqual(self.fetch(0, b'{"default_branch": "main"}'), {'default_branch': 'main'})
        self.assertEqual(self.fetch(0, b'HEAD_SHA: x\n', 'text'), 'HEAD_SHA: x\n')


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

    def test_every_published_build_requires_the_receipt_before_its_smoke_test(self):
        images = (ROOT / '.github/workflows/apis-sites-images.yml').read_text()
        build = images.split('  build:\n', 1)[1].split('  production-ready:\n', 1)[0]
        step = build.split('      - name: Require a key scan of the UI gallery the Sites image ships\n', 1)[1]
        step = step.split('      - name:', 1)[0]
        self.assertIn("if: github.event_name != 'pull_request'\n", step)
        self.assertIn('tooling/ui-gallery-receipt.py', step)
        self.assertIn('/build/client/ui-gallery', step)
        self.assertLess(build.index('Require a key scan of the UI gallery'), build.index('Upload tested images'))
        self.assertIn('\n  checks: read', images.split('\njobs:', 1)[0])
        caller = (ROOT / '.github/workflows/bevy-explorer-ci.yml').read_text()
        self.assertIn('      checks: read\n    uses: ./.github/workflows/apis-sites-images.yml', caller)


if __name__ == '__main__':
    unittest.main()
