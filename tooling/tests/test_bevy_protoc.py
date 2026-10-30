"""The CI compiler install bypasses release discovery and verifies its archive."""
import hashlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import urllib.error
import zipfile

SPEC = importlib.util.spec_from_file_location(
    'bevy_protoc', Path(__file__).resolve().parents[2] / 'apps/bevy-explorer/install-protoc.py')
protoc = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(protoc)


class ProtocTest(unittest.TestCase):
    def archive(self, executable):
        data = io.BytesIO()
        with zipfile.ZipFile(data, 'w') as bundle:
            bundle.writestr(f'bin/{executable}', 'compiler')
            bundle.writestr('include/google/protobuf/empty.proto', 'syntax = "proto3";')
        return data.getvalue()

    def test_host_archive_is_used_and_includes_are_preserved(self):
        for (system, machine), (name, _) in protoc.ARCHIVES.items():
            with self.subTest(system=system), tempfile.TemporaryDirectory() as temporary:
                executable = 'protoc.exe' if system == 'Windows' else 'protoc'
                data = self.archive(executable)
                pins = {(system, machine): (name, hashlib.sha256(data).hexdigest())}
                root = Path(temporary)
                with patch.object(protoc, 'ARCHIVES', pins), patch.object(protoc, 'download', return_value=data) as fetch, \
                        patch.object(protoc.subprocess, 'check_output', return_value='libprotoc 26.1\n'):
                    self.assertEqual(protoc.install(system, machine.upper(), root), root / 'bin')
                fetch.assert_called_once_with(
                    f'https://github.com/protocolbuffers/protobuf/releases/download/v26.1/protoc-26.1-{name}.zip')
                self.assertTrue((root / 'include/google/protobuf/empty.proto').is_file())
                self.assertTrue((root / 'bin' / executable).stat().st_mode & 0o111)

    def test_wrong_archive_is_rejected_before_extracting_or_running_it(self):
        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(protoc, 'download', return_value=b'wrong archive'), \
                patch.object(protoc.subprocess, 'check_output') as execute:
            root = Path(temporary)
            with self.assertRaisesRegex(ValueError, 'SHA-256'):
                protoc.install('Linux', 'x86_64', root)
            self.assertEqual(list(root.iterdir()), [])
            execute.assert_not_called()

    def test_transient_download_failure_retries_the_same_pinned_url(self):
        with patch.object(protoc.urllib.request, 'urlopen', side_effect=[urllib.error.URLError('temporary'), io.BytesIO(b'ok')]) as fetch, \
                patch.object(protoc.time, 'sleep'):
            self.assertEqual(protoc.download('https://example.invalid/pinned.zip'), b'ok')
            self.assertEqual(fetch.call_args_list[0], fetch.call_args_list[1])

    def test_download_failure_exhausts_bounded_retries(self):
        with patch.object(protoc.urllib.request, 'urlopen', side_effect=urllib.error.URLError('unavailable')) as fetch, \
                patch.object(protoc.time, 'sleep'), self.assertRaises(urllib.error.URLError):
            protoc.download('https://example.invalid/pinned.zip')
        self.assertEqual(fetch.call_count, 3)


if __name__ == '__main__':
    unittest.main()
