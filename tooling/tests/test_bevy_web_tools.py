import hashlib
import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    'web_tools', Path(__file__).parents[2] / 'apps/bevy-explorer/install-web-tools.py')
tools = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(tools)


class WebToolsTests(unittest.TestCase):
    def archive(self, member):
        output = io.BytesIO()
        with tarfile.open(fileobj=output, mode='w:gz') as archive:
            data = b'fixture executable'
            info = tarfile.TarInfo(member)
            info.size = len(data)
            archive.addfile(info, io.BytesIO(data))
        return output.getvalue()

    def test_checked_archive_installs_only_the_selected_binary(self):
        data = self.archive('release/wasm-bindgen')
        pin = ('0.2.114', 'https://example.invalid/tool', hashlib.sha256(data).hexdigest(),
               'release/wasm-bindgen')
        with tempfile.TemporaryDirectory() as temporary, \
                patch.dict(tools.TOOLS, {'wasm-bindgen': pin}), \
                patch.object(tools.protoc, 'download', return_value=data), \
                patch.object(tools.subprocess, 'check_output', return_value='wasm-bindgen 0.2.114\n'):
            directory = Path(temporary)
            tools.install('wasm-bindgen', directory)
            self.assertEqual(list(directory.iterdir()), [directory / 'wasm-bindgen'])
            self.assertEqual((directory / 'wasm-bindgen').read_bytes(), b'fixture executable')

    def test_corrupt_archive_is_rejected_before_execution(self):
        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(tools.protoc, 'download', return_value=b'corrupt'), \
                patch.object(tools.subprocess, 'check_output') as execute:
            with self.assertRaisesRegex(ValueError, 'SHA-256'):
                tools.install('wasm-bindgen', Path(temporary))
            execute.assert_not_called()
            self.assertEqual(list(Path(temporary).iterdir()), [])

    def test_installed_version_must_match_the_pin(self):
        data = self.archive('release/wasm-bindgen')
        pin = ('0.2.114', 'https://example.invalid/tool', hashlib.sha256(data).hexdigest(),
               'release/wasm-bindgen')
        with tempfile.TemporaryDirectory() as temporary, \
                patch.dict(tools.TOOLS, {'wasm-bindgen': pin}), \
                patch.object(tools.protoc, 'download', return_value=data), \
                patch.object(tools.subprocess, 'check_output', return_value='wasm-bindgen 0.2.115\n'):
            with self.assertRaisesRegex(ValueError, 'Unexpected wasm-bindgen version'):
                tools.install('wasm-bindgen', Path(temporary))


    def wasm_opt_output(self, output):
        data = self.archive('release/bin/wasm-opt')
        pin = ('129', 'https://example.invalid/tool', hashlib.sha256(data).hexdigest(),
               'release/bin/wasm-opt')
        with tempfile.TemporaryDirectory() as temporary, \
                patch.dict(tools.TOOLS, {'wasm-opt': pin}), \
                patch.object(tools.protoc, 'download', return_value=data), \
                patch.object(tools.subprocess, 'check_output', return_value=output):
            tools.install('wasm-opt', Path(temporary))

    def test_wasm_opt_accepts_binaryens_version_line(self):
        # binaryen 129's own `wasm-opt --version` output.
        self.wasm_opt_output('wasm-opt version 129 (version_129)\n')

    def test_wasm_opt_rejects_another_release(self):
        with self.assertRaisesRegex(ValueError, 'Unexpected wasm-opt version'):
            self.wasm_opt_output('wasm-opt version 128 (version_128)\n')


if __name__ == '__main__':
    unittest.main()
