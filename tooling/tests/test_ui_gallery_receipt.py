"""An image is published only with a successful key scan of the UI gallery bytes it ships."""
import importlib.util
import io
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('ui_gallery_receipt', ROOT / 'tooling/ui-gallery-receipt.py')
receipt = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(receipt)

FILES = {'index.html': b'<!doctype html>', 'hud.html': b'hud', 'assets/index-a1.js': b'export {}'}
HEAD, PARENT, GRANDPARENT = 'c' * 40, 'b' * 40, 'a' * 40


def archive(files, prefix='ui-gallery/'):
    data = io.BytesIO()
    with zipfile.ZipFile(data, 'w') as zipped:
        zipped.writestr('index.html', b'generated review')
        for name, content in files.items():
            zipped.writestr(prefix + name, content)
    return data.getvalue()


class FakeGitHub:
    """Commits GRANDPARENT <- PARENT <- HEAD, each with an optional key scan and ui-review artifact."""

    def __init__(self):
        self.scans = {}
        self.artifacts = {}
        self.library = {HEAD: 'success'}
        self.run_status = 'completed'
        self.parents = {HEAD: PARENT, PARENT: GRANDPARENT, GRANDPARENT: None}

    def api(self, path):
        if match := re.fullmatch(r'commits/(\w+)/check-runs\?.*', path):
            scan = self.scans.get(match[1])
            return {'check_runs': [] if scan is None else [
                {'id': 1, 'name': receipt.CHECK, 'app': {'slug': 'github-actions'}, 'status': 'completed',
                 'conclusion': scan, 'completed_at': '2026-11-21T00:00:00Z'}]}
        if match := re.fullmatch(r'actions/artifacts\?name=ui-review-(\w+)&per_page=100', path):
            sha = match[1]
            return {'artifacts': [] if sha not in self.artifacts else [
                {'id': int(sha[0], 16), 'expired': False, 'workflow_run': {'id': 7, 'head_sha': sha}}]}
        if path == 'actions/runs/7':
            return {'path': receipt.LIBRARY_WORKFLOW}
        if match := re.fullmatch(r'actions/workflows/ui-library\.yml/runs\?head_sha=(\w+)&per_page=100', path):
            sha = match[1]
            return {'workflow_runs': [] if sha not in self.library else [
                {'id': 9, 'head_sha': sha, 'event': 'push', 'status': self.run_status, 'conclusion': 'success'}]}
        if path == 'actions/runs/9/jobs?per_page=100':
            return {'jobs': [{'name': 'library', 'conclusion': self.library[HEAD]}]}
        if match := re.fullmatch(r'commits/(\w+)', path):
            parent = self.parents[match[1]]
            return {'parents': [] if parent is None else [{'sha': parent}]}
        raise KeyError(path)

    def download(self, artifact_id):
        return next(data for sha, data in self.artifacts.items() if int(sha[0], 16) == artifact_id)


class ReceiptTest(unittest.TestCase):
    def setUp(self):
        self.gh = FakeGitHub()
        self.digest = receipt.artifact_digest(archive(FILES))

    def require(self, **kwargs):
        return receipt.require_receipt(self.gh, HEAD, self.digest, wait=0, sleep=lambda _: None, **kwargs)

    def test_the_head_scan_of_the_same_bytes_is_the_receipt(self):
        self.gh.scans[HEAD] = 'success'
        self.gh.artifacts[HEAD] = archive(FILES)
        self.assertEqual(self.require(), HEAD)

    def test_a_failed_head_scan_fails_without_looking_further(self):
        self.gh.scans[HEAD] = 'failure'
        self.gh.scans[PARENT] = 'success'
        self.gh.artifacts[PARENT] = archive(FILES)
        with self.assertRaisesRegex(receipt.ReceiptError, 'concluded failure'):
            self.require()

    def test_a_head_scan_of_other_bytes_fails(self):
        self.gh.scans[HEAD] = 'success'
        self.gh.artifacts[HEAD] = archive({**FILES, 'hud.html': b'other'})
        with self.assertRaisesRegex(receipt.ReceiptError, 'but the image ships'):
            self.require()

    def test_a_head_scan_without_its_artifact_fails(self):
        self.gh.scans[HEAD] = 'success'
        with self.assertRaisesRegex(receipt.ReceiptError, 'no unexpired ui-review'):
            self.require()

    def test_an_artifact_without_the_gallery_fails(self):
        self.gh.scans[HEAD] = 'success'
        self.gh.artifacts[HEAD] = archive(FILES, prefix='gallery/')
        with self.assertRaisesRegex(receipt.ReceiptError, 'holds no ui-gallery/'):
            self.require()

    def test_a_missing_head_scan_the_library_run_requires_fails_after_waiting(self):
        with self.assertRaisesRegex(receipt.ReceiptError, 'no completed key scan'):
            self.require()

    def test_a_running_library_run_is_waited_for(self):
        self.gh.run_status = 'in_progress'
        slept = []
        with self.assertRaisesRegex(receipt.ReceiptError, 'no completed key scan'):
            receipt.require_receipt(self.gh, HEAD, self.digest, wait=0, sleep=slept.append)
        self.gh.run_status = 'completed'
        self.gh.scans[HEAD] = 'success'
        self.gh.artifacts[HEAD] = archive(FILES)
        self.assertEqual(self.require(), HEAD)

    def test_an_unchanged_library_reuses_the_nearest_scan_of_identical_bytes(self):
        self.gh.library[HEAD] = 'skipped'
        self.gh.scans[PARENT] = 'success'
        self.gh.artifacts[PARENT] = archive({**FILES, 'hud.html': b'older'})
        self.gh.scans[GRANDPARENT] = 'success'
        self.gh.artifacts[GRANDPARENT] = archive(FILES)
        self.assertEqual(self.require(), GRANDPARENT)

    def test_an_unchanged_library_never_reuses_a_failed_scan(self):
        self.gh.library[HEAD] = 'skipped'
        self.gh.scans[PARENT] = 'failure'
        self.gh.artifacts[PARENT] = archive(FILES)
        with self.assertRaisesRegex(receipt.ReceiptError, 'no successful key scan'):
            self.require()

    def test_no_scan_of_identical_bytes_anywhere_fails(self):
        self.gh.library[HEAD] = 'skipped'
        with self.assertRaisesRegex(receipt.ReceiptError, 'no successful key scan'):
            self.require()


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
