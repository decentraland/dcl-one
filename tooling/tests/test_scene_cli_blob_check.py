"""`build-base-blob.py --check` judges the vendored SDK archive by its decompressed bytes, never by the
central directory's own account of them."""
import contextlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
import zipfile

SPEC = importlib.util.spec_from_file_location(
    'build_base_blob', Path(__file__).resolve().parents[2] / 'apps/dcl-scene-cli/scripts/build-base-blob.py')
blob = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(blob)

FILES = {
    'node_modules/typescript/lib/tsc.js': b'console.log("tsc ' + b'x' * 400 + b'")\n',
    'node_modules/@dcl/sdk/package.json': b'{"name":"@dcl/sdk","version":"7.0.0"}\n',
}


def stored(files):
    """The same entries as `zip_bytes`, deflated at level 0: different compressed bytes, same content."""
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, 'w') as z:
        for name in sorted(files):
            info = zipfile.ZipInfo(name, date_time=blob.FIXED_DATE)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            z.writestr(info, files[name], compresslevel=0)
    return buf.getvalue()


class BlobCheckTest(unittest.TestCase):
    def check(self, built, committed):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory, 'node_modules.zip')
            path.write_bytes(committed)
            out, err = io.StringIO(), io.StringIO()
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                status = blob.compare_with_committed(built, str(path))
        return status, out.getvalue() + err.getvalue()

    def test_identical_bytes_pass(self):
        built = blob.zip_bytes(FILES)
        status, text = self.check(built, built)
        self.assertEqual(status, 0)
        self.assertIn('matches the tree', text)

    def test_the_same_content_under_another_compression_passes(self):
        status, text = self.check(blob.zip_bytes(FILES), stored(FILES))
        self.assertEqual(status, 0)
        self.assertIn('different compression', text)

    def test_tampered_bytes_behind_the_original_directory_fail(self):
        # Level-0 deflate carries the content verbatim, so a byte swap leaves the CRC and
        # the size the central directory lists untouched: the metadata still matches the tree.
        committed = stored(FILES)
        original = b'console.log("tsc '
        tampered = b'require("./evil")'
        self.assertEqual(len(original), len(tampered))
        self.assertEqual(committed.count(original), 1)
        status, text = self.check(blob.zip_bytes(FILES), committed.replace(original, tampered))
        self.assertEqual(status, 1)
        self.assertIn('corrupt:        node_modules/typescript/lib/tsc.js', text)
        self.assertNotIn('matches the tree', text)

    def test_a_swapped_file_fails_and_is_named(self):
        swapped = dict(FILES, **{'node_modules/typescript/lib/tsc.js': b'require("./evil")\n'})
        status, text = self.check(blob.zip_bytes(FILES), blob.zip_bytes(swapped))
        self.assertEqual(status, 1)
        self.assertIn('changed:        node_modules/typescript/lib/tsc.js', text)
        self.assertNotIn('@dcl/sdk/package.json', text)

    def test_added_and_missing_entries_are_named(self):
        fewer = {k: v for k, v in FILES.items() if 'typescript' not in k}
        status, text = self.check(blob.zip_bytes(FILES), blob.zip_bytes(fewer))
        self.assertEqual(status, 1)
        self.assertIn('only built:     node_modules/typescript/lib/tsc.js', text)
        status, text = self.check(blob.zip_bytes(fewer), blob.zip_bytes(FILES))
        self.assertEqual(status, 1)
        self.assertIn('only committed: node_modules/typescript/lib/tsc.js', text)

    def test_a_file_that_is_not_an_archive_fails(self):
        status, text = self.check(blob.zip_bytes(FILES), b'not a zip')
        self.assertEqual(status, 1)
        self.assertIn('not a zip archive', text)


if __name__ == '__main__':
    unittest.main()
