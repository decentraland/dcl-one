import copy
import importlib.util
import io
import json
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('journeys', ROOT / 'tooling/scene-cli-journey-report.py')
report = importlib.util.module_from_spec(spec)
spec.loader.exec_module(report)
PNG = b'\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR' + struct.pack('>II', 1440, 1000)
MANIFEST = {'schema': 1, 'revision': 'a' * 40, 'stages': [
    {'id': 'preview', 'journey': 'preview', 'title': 'Preview', 'status': 'passed', 'screenshot': 'preview.png'}]}
RUN = {'event': 'pull_request', 'status': 'completed', 'conclusion': 'success', 'id': 1,
       'html_url': 'https://github.com/org/repo/actions/runs/1', 'path': report.WORKFLOW,
       'head_sha': 'a' * 40, 'pull_requests': [{'number': 7}]}
PR = {'state': 'open', 'number': 7, 'base': {'repo': {'full_name': 'org/repo'}, 'sha': 'a' * 40}, 'head': {'sha': 'a' * 40}}
ARTIFACT = {'id': 1, 'name': report.ARTIFACT, 'expired': False, 'size_in_bytes': 100}


class JourneyReportTests(unittest.TestCase):
    def test_rejects_paths_duplicate_ids_invalid_status_and_oversized_images(self):
        for edit in ('path', 'duplicate', 'status'):
            with self.subTest(edit=edit):
                value = copy.deepcopy(MANIFEST)
                if edit == 'path':
                    value['stages'][0]['screenshot'] = '../../payload.py'
                elif edit == 'duplicate':
                    value['stages'] *= 2
                else:
                    value['stages'][0]['status'] = 'silently-skipped'
                with self.assertRaises(ValueError):
                    report.manifest(json.dumps(value))
        for value in (b'<svg onload=alert(1)>', PNG[:16] + struct.pack('>II', 100000, 1), PNG + b'x' * 5_000_000):
            with self.assertRaises(ValueError):
                report.png(value)

    def test_archive_only_reads_validated_images_and_recomputes_hash(self):
        buffer = io.BytesIO()
        with zipfile.ZipFile(buffer, 'w') as archive:
            archive.writestr('head/manifest.json', json.dumps(MANIFEST))
            archive.writestr('head/preview.png', PNG)
            archive.writestr('payload.py', 'raise RuntimeError("never execute artifacts")')
        results, images = report.read_archive(buffer.getvalue())
        self.assertEqual(images['head', 'preview'], PNG)
        self.assertEqual(len(results['head']['stages'][0]['sha256']), 64)
        self.assertTrue(results['base']['errors'])

    def test_fork_association_is_api_derived_and_stale_heads_are_ignored(self):
        with patch.object(report, 'api', return_value=PR):
            self.assertEqual(report.current_pr('org/repo', RUN)['number'], 7)
            self.assertIsNone(report.current_pr('another/repo', RUN))
            self.assertIsNone(report.current_pr('org/repo', {**RUN, 'head_sha': 'b' * 40}))
        for field, value in [('event', 'push'), ('path', '.github/workflows/other.yml'), ('status', 'in_progress')]:
            with self.assertRaises(ValueError):
                report.current_pr('org/repo', {**RUN, field: value})

    def test_redirect_does_not_forward_the_github_token(self):
        request = urllib.request.Request('https://api.github.com/archive', headers={'Authorization': 'secret'})
        redirected = report.Redirect().redirect_request(request, None, 302, '', {}, 'https://example.org/image')
        self.assertIsNone(redirected.get_header('Authorization'))

    def test_same_run_report_requires_a_finished_producer(self):
        run = {**RUN, 'status': 'in_progress', 'conclusion': None}
        with patch.object(report, 'api', return_value=PR):
            for result in ('success', 'failure'):
                self.assertEqual(report.current_pr('org/repo', run, result), PR)
            for result in ('', 'skipped', 'cancelled'):
                with self.assertRaises(ValueError):
                    report.current_pr('org/repo', run, result)

    def test_missing_capture_is_visible_and_html_escapes_fixture_text(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            (directory / 'head').mkdir()
            value = copy.deepcopy(MANIFEST)
            value['stages'][0]['title'] = '<script>alert(1)</script>'
            (directory / 'head/manifest.json').write_text(json.dumps(value))
            (directory / 'head/preview.png').write_bytes(PNG)
            output = directory / 'report.html'
            report.render(directory, output)
            text = output.read_text()
            self.assertIn('Build or capture did not complete', text)
            self.assertIn('&lt;script&gt;', text)
            self.assertIn('data:image/png;base64,', text)

    def test_comment_links_artifact_and_inlines_before_after_images(self):
        after = copy.deepcopy(MANIFEST)
        after['stages'][0]['sha256'] = 'b' * 64
        results = {'base': MANIFEST, 'head': after}
        text = report.comment('org/repo', {'id': 1, 'html_url': 'https://github.com/org/repo/actions/runs/1', 'conclusion': 'failure'},
                              {'head': {'sha': 'a' * 40}}, results,
                              {('base', 'preview'): 'https://example.org/base.png', ('head', 'preview'): 'https://example.org/head.png'}, {'id': 2})
        self.assertTrue(text.startswith(report.MARKER))
        self.assertIn('![base](https://example.org/base.png)', text)
        self.assertIn('![head](https://example.org/head.png)', text)
        self.assertIn('/actions/runs/1/artifacts/2', text)
        self.assertIn('**failure**', text)
        self.assertIn('following 1 journey:', text)
        self.assertIn('<details><summary>preview: 1 stage</summary>', text)

    def test_comparison_ignores_volatile_raw_pixels_but_keeps_failures(self):
        before, after = copy.deepcopy(MANIFEST), copy.deepcopy(MANIFEST)
        for value, raw in ((before, 'a'), (after, 'b')):
            value['stages'][0].update(sha256=raw * 64, comparison_sha256='c' * 64)
        self.assertEqual(report.changed_stages({'base': before, 'head': after}), {})
        after['stages'][0]['status'] = 'failed'
        self.assertEqual(report.changed_stages({'base': before, 'head': after}), {'preview': ['preview']})

    def test_unchanged_success_does_not_upload_or_post_a_comment(self):
        with patch.object(report, 'api', side_effect=[RUN, PR, {'artifacts': [ARTIFACT]}, b'archive', []]) as api:
            with patch.object(report, 'read_archive', return_value=({'base': MANIFEST, 'head': MANIFEST}, {})):
                report.publish('org/repo', 1)
        self.assertEqual(api.call_count, 5)
        self.assertTrue(all(call.args[0].startswith('repos/') and len(call.args) == 1 for call in api.call_args_list))

    def test_changed_journey_uses_direct_attachments_and_reuses_identical_bytes(self):
        before, after = copy.deepcopy(MANIFEST), copy.deepcopy(MANIFEST)
        before['stages'][0]['comparison_sha256'] = 'a' * 64
        after['stages'][0]['comparison_sha256'] = 'b' * 64
        url = 'https://github.com/user-attachments/assets/example'
        responses = [RUN, PR, {'artifacts': [ARTIFACT]}, b'archive', [], {'id': 123}, {'url': url}, PR, {}]
        with patch.object(report, 'api', side_effect=responses) as api:
            with patch.object(report, 'read_archive', return_value=({'base': before, 'head': after}, {('base','preview'): PNG, ('head','preview'): PNG})):
                report.publish('org/repo', 1)
        uploads = [call for call in api.call_args_list if call.args[0].startswith('https://uploads.github.com/')]
        self.assertEqual(len(uploads), 1)
        self.assertIn('/user-attachments/assets?', uploads[0].args[0])
        self.assertIn('repository_id=123', uploads[0].args[0])
        self.assertIn('![base](' + url + ')', api.call_args.args[1]['body'])
        self.assertIn('![head](' + url + ')', api.call_args.args[1]['body'])


if __name__ == '__main__':
    unittest.main()
