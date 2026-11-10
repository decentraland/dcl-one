"""Checked graph validation and historical bootstrap contracts."""
import importlib.util
import json
import shutil
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
from hud_manifest_fixture import seal, SHARED, WEB, MANIFEST

SPEC = importlib.util.spec_from_file_location('hud_inputs', Path(__file__).parents[1] / 'hud_shared_inputs.py')
reader = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(reader)
CI_SPEC = importlib.util.spec_from_file_location('hud_ci', Path(__file__).parents[1] / 'bevy-ci.py')
ci = importlib.util.module_from_spec(CI_SPEC)
CI_SPEC.loader.exec_module(ci)


class CheckedGraphTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.git('init', '-q')
        self.git('config', 'maintenance.auto', 'false')
        self.git('config', 'gc.auto', '0')
        self.write(WEB + 'src/main.ts', 'export {}')
        self.write(SHARED[0], 'export const value = 1')
        seal(self.root)
        self.commit()

    def git(self, *args):
        return subprocess.check_output(['git', '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                                        *args], cwd=self.root, text=True, stderr=subprocess.PIPE).strip()

    def write(self, name, value):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(value)

    def commit(self):
        self.git('add', '.')
        self.git('commit', '-qm', 'fixture', '--allow-empty')
        return self.git('rev-parse', 'HEAD')

    def manifest(self, change):
        path = self.root / MANIFEST
        data = json.loads(path.read_text())
        change(data)
        path.write_text(json.dumps(data))

    def test_filtered_source_excludes_canonical_produced_bindings_from_owned_roots(self):
        expected = reader.input_hashes(self.root)
        shutil.rmtree(self.root / '.git')
        self.write(WEB + 'src/engine/generated/Player.ts', 'export interface Player {}')
        self.assertEqual(expected, reader.input_hashes(self.root))
        self.write(WEB + 'src/new.ts', 'export const newRoot = 1')
        with self.assertRaisesRegex(reader.ManifestError, 'root inventory'):
            reader.input_hashes(self.root)

    def test_new_nearer_resolver_control_refuses_stale_graph_before_selection(self):
        self.write(SHARED[2], 'export const token = 1')
        seal(self.root)
        self.commit()
        original = (self.root / MANIFEST).read_bytes()
        self.write('library/ui/foundations/tsconfig.json', '{}')
        self.git('add', '.')
        with self.assertRaisesRegex(reader.ManifestError, 'New HUD resolver control'):
            reader.input_hashes(self.root)
        self.assertEqual(original, (self.root / MANIFEST).read_bytes())

    def test_font_identity_control_drift_cannot_reuse_the_recorded_graph(self):
        self.write('library/ui/foundations/typography/font-source.json', 'changed font identity')
        with self.assertRaisesRegex(reader.ManifestError, 'Stale HUD input'):
            reader.input_hashes(self.root)

    def test_historical_graph_uses_its_own_blobs_and_current_drift_fails(self):
        before = self.git('rev-parse', 'HEAD')
        expected = reader.input_hashes(self.root, before)
        self.write(SHARED[0], 'changed')
        self.assertEqual(expected, reader.input_hashes(self.root, before))
        with self.assertRaisesRegex(reader.ManifestError, 'Stale HUD input'):
            reader.read_manifest(self.root)

    def test_new_tracked_root_fails_until_manifest_is_regenerated(self):
        self.write(WEB + 'src/new.json', '{}')
        self.git('add', '.')
        with self.assertRaisesRegex(reader.ManifestError, 'root inventory'):
            reader.read_manifest(self.root)
        seal(self.root)
        self.assertIn(WEB + 'src/new.json', reader.input_hashes(self.root))

    def test_unrelated_files_leave_graph_identity_unchanged(self):
        before = reader.input_hashes(self.root)
        self.write('library/ui/components/Other.tsx', 'export {}')
        self.commit()
        self.assertEqual(before, reader.input_hashes(self.root))

    def test_removed_dependency_still_selects_web_from_base_graph(self):
        before = self.git('rev-parse', 'HEAD')
        (self.root / SHARED[0]).unlink()
        seal(self.root)
        head = self.commit()
        with patch.object(ci, 'ROOT', self.root):
            self.assertTrue(ci.select(before, head)['web'])

    def test_missing_historical_manifest_bootstraps_but_missing_head_fails(self):
        (self.root / MANIFEST).unlink()
        old = self.commit()
        with self.assertRaises(reader.MissingManifest):
            reader.input_hashes(self.root, old)
        seal(self.root)
        head = self.commit()
        with patch.object(ci, 'ROOT', self.root):
            self.assertTrue(ci.select(old, head)['web'])
            with self.assertRaises(ci._artifact.shared_inputs.MissingManifest):
                ci.select(head, old)

    def test_unsafe_paths_and_unresolved_edges_fail_closed(self):
        for name in ('../escape', '/absolute', 'library//ui/file', 'library/./file', 'library\\escape'):
            with self.subTest(name=name):
                seal(self.root)
                self.manifest(lambda data: data['files'].append({'path': name, 'sha256': 'a' * 64}))
                with self.assertRaises(reader.ManifestError):
                    reader.read_manifest(self.root)
        seal(self.root)
        self.manifest(lambda data: data['edges'].append({'from': WEB + 'src/main.ts', 'to': 'missing', 'kind': 'import'}))
        with self.assertRaisesRegex(reader.ManifestError, 'Unresolved'):
            reader.read_manifest(self.root)

    def test_stale_head_is_an_error_instead_of_a_conservative_selection(self):
        before = self.git('rev-parse', 'HEAD')
        self.write(SHARED[0], 'stale graph')
        head = self.commit()
        with patch.object(ci, 'ROOT', self.root):
            with self.assertRaisesRegex(ci._artifact.shared_inputs.ManifestError, 'Stale'):
                ci.plan('local', before, head, 'unused')

    def test_unhashed_root_is_rejected(self):
        self.manifest(lambda data: data.update(files=[row for row in data['files']
                                                      if row['path'] != WEB + 'src/main.ts']))
        with self.assertRaisesRegex(reader.ManifestError, 'root missing'):
            reader.read_manifest(self.root)

    def test_omitted_resolver_control_is_rejected(self):
        self.manifest(lambda data: data.update(controls=[]))
        with self.assertRaisesRegex(reader.ManifestError, 'resolver controls'):
            reader.read_manifest(self.root)

    def test_historical_lfs_pointer_and_payload_share_the_logical_hash(self):
        payload = 'wOF2 fixture'
        oid = reader.sha(payload.encode())
        self.write(SHARED[0], f'version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize {len(payload)}\n')
        seal(self.root)
        self.manifest(lambda data: data['files'][0].update(sha256=oid, lfs={'oid': oid, 'size': len(payload)}))
        pointer = self.commit()
        self.write(SHARED[0], payload)
        materialized = self.commit()
        self.assertEqual(reader.input_hashes(self.root, pointer), reader.input_hashes(self.root, materialized))

    def test_symlinked_and_untracked_dependency_are_rejected(self):
        path = self.root / SHARED[0]
        path.unlink()
        path.symlink_to(self.root / (WEB + 'src/main.ts'))
        with self.assertRaisesRegex(reader.ManifestError, 'linked'):
            reader.read_manifest(self.root)
        path.unlink()
        self.write(SHARED[0], 'export const value = 1')
        self.git('rm', '--cached', SHARED[0])
        with self.assertRaisesRegex(reader.ManifestError, 'untracked'):
            reader.read_manifest(self.root)

    def test_lfs_pointer_and_materialized_payload_have_one_identity(self):
        payload = 'wOF2 fixture'
        oid = reader.sha(payload.encode())
        self.write(SHARED[0], payload)
        seal(self.root)
        self.manifest(lambda data: data['files'][0].update(lfs={'oid': oid, 'size': len(payload)}))
        expected = reader.input_hashes(self.root)
        self.write(SHARED[0], f'version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize {len(payload)}\n')
        self.assertEqual(expected, reader.input_hashes(self.root))
        self.write(SHARED[0], f'version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize 1\n')
        with self.assertRaisesRegex(reader.ManifestError, 'Stale'):
            reader.read_manifest(self.root)


if __name__ == '__main__':
    unittest.main()
