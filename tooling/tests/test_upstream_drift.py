"""The drift collector's parsing and version logic, and the site's markdown rendering."""
import importlib.util
from pathlib import Path
import textwrap
import unittest

TOOLING = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('upstream_drift', TOOLING / 'upstream-drift.py')
drift = importlib.util.module_from_spec(spec)
spec.loader.exec_module(drift)
spec2 = importlib.util.spec_from_file_location('upstream_drift_site', TOOLING / 'upstream-drift-site.py')
site = importlib.util.module_from_spec(spec2)
spec2.loader.exec_module(site)


class VersionLogicTests(unittest.TestCase):
    def test_requirements_follow_cargo_semantics(self):
        self.assertTrue(drift.matches_req('0.23.37', '>= 0.23.0'))
        self.assertFalse(drift.matches_req('0.23.37', '>= 0.23.38'))
        self.assertTrue(drift.matches_req('1.2.3', '^1.0.0'))
        self.assertFalse(drift.matches_req('2.0.0', '^1.0.0'))
        self.assertTrue(drift.matches_req('0.3.3', '^0.3.1'))
        self.assertFalse(drift.matches_req('0.4.0', '^0.3.1'))
        self.assertTrue(drift.matches_req('0.9.18', '>= 0.9.0, < 0.9.19'))
        self.assertFalse(drift.matches_req('0.9.19', '>= 0.9.0, < 0.9.19'))

    def test_affected_means_neither_patched_nor_unaffected(self):
        advisory = {'patched': ['>= 0.23.40'], 'unaffected': ['< 0.20.0']}
        self.assertTrue(drift.affected('0.23.37', advisory))
        self.assertFalse(drift.affected('0.23.40', advisory))
        self.assertFalse(drift.affected('0.19.0', advisory))

    def test_advisory_markdown_parses_front_matter_and_title(self):
        text = textwrap.dedent('''
            ```toml
            [advisory]
            id = "RUSTSEC-2026-0001"
            package = "demo"
            date = "2026-01-02"
            aliases = ["CVE-2026-1"]
            informational = "unmaintained"

            [versions]
            patched = []
            unaffected = ["< 0.1.0"]
            ```

            # demo is unmaintained

            Body text.
        ''')
        advisory = drift.parse_advisory(text)
        self.assertEqual(advisory['id'], 'RUSTSEC-2026-0001')
        self.assertEqual(advisory['title'], 'demo is unmaintained')
        self.assertEqual(advisory['informational'], 'unmaintained')
        self.assertEqual(advisory['unaffected'], ['< 0.1.0'])
        self.assertIsNone(drift.parse_advisory('no toml here'))

    def test_tag_helpers(self):
        self.assertEqual(drift.version_key('v0.16.1'), (0, 16, 1, 0))
        self.assertGreater(drift.version_key('v1.46.3'), drift.version_key('v0.28.0'))
        self.assertEqual(drift.tag_prefix('livekit-ffi/v0.12.83'), 'livekit-ffi/v')
        self.assertEqual(drift.tag_prefix('0.307.0'), '')

    def test_lock_packages_and_shape(self):
        lock = textwrap.dedent('''
            [[package]]
            name = "a"
            version = "1.0.0"
            source = "registry+https://github.com/rust-lang/crates.io-index"

            [[package]]
            name = "a"
            version = "2.0.0"
            source = "registry+https://github.com/rust-lang/crates.io-index"

            [[package]]
            name = "b"
            version = "0.1.0"
            source = "git+https://github.com/x/b?branch=main#abc"

            [[package]]
            name = "ws"
            version = "0.0.0"
        ''')
        pkgs = drift.lock_packages(lock)
        shape = drift.lock_shape(pkgs)
        self.assertEqual(shape['packages'], 4)
        self.assertEqual(shape['by_source'], {'crates.io': 2, 'git': 1, 'workspace': 1})
        self.assertEqual(shape['duplicates'], [{'name': 'a', 'versions': ['1.0.0', '2.0.0']}])


class SiteTests(unittest.TestCase):
    def test_markdown_covers_the_review_constructs(self):
        html = site.markdown_to_html('# Title\n\nA **bold** `code` [link](https://x.y) line.\n\n- one\n- two\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n')
        self.assertIn('<h1 id="title">Title</h1>', html)
        self.assertIn('<strong>bold</strong> <code>code</code> <a href="https://x.y"', html)
        self.assertIn('<ul>\n<li>one</li>\n<li>two</li>\n</ul>', html)
        self.assertIn('<table><thead><tr><th>a</th><th>b</th></tr></thead><tbody><tr><td>1</td><td>2</td></tr></tbody></table>', html)

    def test_slug(self):
        self.assertEqual(site.slug('https://github.com/robtfm/bevy_kira_audio'), 'robtfm-bevy-kira-audio')


if __name__ == '__main__':
    unittest.main()
