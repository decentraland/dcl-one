"""The CI compiler install bypasses release discovery and verifies its archive."""
import hashlib
import http.client
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


class StalledBody(io.BytesIO):
    """A response whose connection opened and then failed while its body was read."""

    def __init__(self, failure):
        super().__init__()
        self.failure = failure

    def read(self, size=-1):
        raise self.failure


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
                        patch.object(protoc.subprocess, 'check_output', return_value=f'libprotoc {protoc.VERSION}\n'):
                    self.assertEqual(protoc.install(system, machine.upper(), root), root / 'bin')
                fetch.assert_called_once_with('https://github.com/protocolbuffers/protobuf/releases/download/'
                                              f'v{protoc.VERSION}/protoc-{protoc.VERSION}-{name}.zip')
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

    def test_download_failure_backs_off_until_the_retry_budget_is_spent(self):
        clock = [0.0]
        with patch.object(protoc.urllib.request, 'urlopen', side_effect=urllib.error.URLError('unavailable')) as fetch, \
                patch.object(protoc.time, 'monotonic', side_effect=lambda: clock[0]), \
                patch.object(protoc.time, 'sleep', side_effect=lambda seconds: clock.__setitem__(0, clock[0] + seconds)) as sleep, \
                self.assertRaises(urllib.error.URLError):
            protoc.download('https://example.invalid/pinned.zip')
        self.assertEqual(clock[0], protoc.RETRY_BUDGET)
        self.assertEqual([call.args[0] for call in sleep.call_args_list], [1, 2, 4, 8, 16, 30, 30, 29])
        self.assertEqual(fetch.call_count, 9)

    def test_failures_while_reading_the_body_are_retried(self):
        for failure in (http.client.IncompleteRead(b'partial'), ConnectionResetError('reset by peer')):
            with self.subTest(failure=type(failure).__name__), \
                    patch.object(protoc.urllib.request, 'urlopen', side_effect=[StalledBody(failure), io.BytesIO(b'ok')]) as fetch, \
                    patch.object(protoc.time, 'sleep') as sleep:
                self.assertEqual(protoc.download('https://example.invalid/pinned.zip'), b'ok')
                self.assertEqual(fetch.call_count, 2)
                sleep.assert_called_once_with(1)

    def test_server_errors_are_retried_and_client_errors_are_not(self):
        url = 'https://example.invalid/pinned.zip'
        unavailable = urllib.error.HTTPError(url, 503, 'unavailable', {}, None)
        with patch.object(protoc.urllib.request, 'urlopen', side_effect=[unavailable, io.BytesIO(b'ok')]), \
                patch.object(protoc.time, 'sleep'):
            self.assertEqual(protoc.download(url), b'ok')
        missing = urllib.error.HTTPError(url, 404, 'missing', {}, None)
        with patch.object(protoc.urllib.request, 'urlopen', side_effect=missing) as fetch, \
                patch.object(protoc.time, 'sleep') as sleep, self.assertRaises(urllib.error.HTTPError):
            protoc.download(url)
        fetch.assert_called_once()
        sleep.assert_not_called()

    def test_verified_cached_archive_installs_without_the_network(self):
        data = self.archive('protoc.exe')
        digest = hashlib.sha256(data).hexdigest()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'cache').mkdir()
            (root / f'cache/protoc-{protoc.VERSION}-win64-{digest}.zip').write_bytes(data)
            with patch.object(protoc, 'ARCHIVES', {('Windows', 'amd64'): ('win64', digest)}), \
                    patch.object(protoc, 'download', side_effect=urllib.error.URLError('offline')) as fetch, \
                    patch.object(protoc.subprocess, 'check_output', return_value=f'libprotoc {protoc.VERSION}\n'):
                self.assertEqual(protoc.install('Windows', 'AMD64', root / 'install', root / 'cache'), root / 'install/bin')
            fetch.assert_not_called()

    def test_corrupt_cached_archive_is_replaced_by_the_verified_download(self):
        data = self.archive('protoc.exe')
        digest = hashlib.sha256(data).hexdigest()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            entry = root / f'cache/protoc-{protoc.VERSION}-win64-{digest}.zip'
            entry.parent.mkdir()
            entry.write_bytes(b'bad cached archive')
            with patch.object(protoc, 'ARCHIVES', {('Windows', 'amd64'): ('win64', digest)}), \
                    patch.object(protoc, 'download', return_value=data) as fetch, \
                    patch.object(protoc.subprocess, 'check_output', return_value=f'libprotoc {protoc.VERSION}\n'):
                protoc.install('Windows', 'AMD64', root / 'install', root / 'cache')
            fetch.assert_called_once()
            self.assertEqual(entry.read_bytes(), data)
            self.assertEqual([path.name for path in entry.parent.iterdir()], [entry.name])

    def test_rejected_download_is_neither_cached_nor_extracted(self):
        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(protoc, 'download', return_value=b'wrong archive'), \
                patch.object(protoc.subprocess, 'check_output') as execute:
            root = Path(temporary)
            with self.assertRaisesRegex(ValueError, 'SHA-256'):
                protoc.install('Windows', 'AMD64', root / 'install', root / 'cache')
            self.assertEqual(list(root.iterdir()), [])
            execute.assert_not_called()


if __name__ == '__main__':
    unittest.main()
