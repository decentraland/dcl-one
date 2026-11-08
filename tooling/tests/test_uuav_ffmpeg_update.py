"""The FFmpeg updater picks the highest release tag and rewrites exactly the uuav.nix pin."""
import importlib.util
from pathlib import Path
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('ffmpeg_update', ROOT / 'tooling/uuav/ffmpeg-update.py')
update = importlib.util.module_from_spec(spec)
spec.loader.exec_module(update)

OLD = '946fcce07b6dcd0331c8cc609192aeff5e1924f8'
NEW = '74e4393595cf2b5857f0da925c4e0dd74ea306e5'
TAG = '9d106a4c5a139fac2a62c349817017191217a9d8'
OLD_HASH = 'sha256-/c6cVfMSF6JJxCuJs58bxBlmBXDJ+jbnC4pELB97Qb0='
NEW_HASH = 'sha256-XK9nk9wqoINbMqjTE6dWm4E5b3wmke4vsqeHshgu6UQ='
PACKAGE = textwrap.dedent(f'''
    ffmpegSrc = pkgs.fetchFromGitHub {{
      owner = "FFmpeg";
      repo = "FFmpeg";
      rev = "{OLD}";
      hash = "{OLD_HASH}";
    }};
    ffmpegVersion = "9.0.2";
    other = pkgs.fetchFromGitHub {{ repo = "dav1d"; rev = "{TAG}"; hash = "{OLD_HASH}"; }};
''')
LISTING = '\n'.join([
    f'{OLD}\trefs/tags/n9.0.2',
    f'{TAG}\trefs/tags/n9.1',
    f'{NEW}\trefs/tags/n9.1^{{}}',
    f'{TAG}\trefs/tags/n9.2-dev',
    f'{TAG}\trefs/tags/n10.0-dev',
    f'{TAG}\trefs/tags/n8.1.3',
    f'{TAG}\trefs/tags/v0.6.1',
])


class FfmpegUpdateTest(unittest.TestCase):
    def test_reads_the_pin(self):
        self.assertEqual(update.current_pin(PACKAGE), (OLD, OLD_HASH, '9.0.2'))

    def test_rewrites_only_the_ffmpeg_pin(self):
        rewritten = update.replace_pin(PACKAGE, NEW, NEW_HASH, '9.1')
        self.assertEqual(update.current_pin(rewritten), (NEW, NEW_HASH, '9.1'))
        self.assertIn(f'repo = "dav1d"; rev = "{TAG}"; hash = "{OLD_HASH}";', rewritten)

    def test_rejects_a_package_without_the_pin(self):
        with self.assertRaises(ValueError):
            update.current_pin(PACKAGE.replace('ffmpegVersion', 'version'))

    def test_takes_the_highest_release_and_the_commit_of_its_tag(self):
        self.assertEqual(update.newest_release(LISTING), ('9.1', NEW))

    def test_orders_releases_by_number(self):
        listing = f'{OLD}\trefs/tags/n9.9\n{NEW}\trefs/tags/n9.10\n{TAG}\trefs/tags/n9.9.9'
        self.assertEqual(update.newest_release(listing), ('9.10', NEW))

    def test_rejects_a_listing_without_a_release(self):
        with self.assertRaises(ValueError):
            update.newest_release(f'{TAG}\trefs/tags/n9.2-dev')

    def test_renames_the_release_in_a_document(self):
        text = 'built on FFmpeg 9.0.2, from the `n9.0.2` commit; majors of 9.0'
        self.assertEqual(update.rename_release(text, '9.0.2', '9.1'),
                         'built on FFmpeg 9.1, from the `n9.1` commit; majors of 9.0')

    def test_repository_documents_name_the_pinned_release(self):
        version = update.current_pin(update.PACKAGE.read_text(encoding='utf-8'))[2]
        for document in update.DOCUMENTS:
            text = (ROOT / document).read_text(encoding='utf-8')
            self.assertTrue(f'FFmpeg {version}' in text or f'`n{version}`' in text, document)


if __name__ == '__main__':
    unittest.main()
