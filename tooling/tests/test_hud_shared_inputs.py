"""Checked graph validation and historical bootstrap contracts."""
import importlib.util
import os
import shlex
import time
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

    def native_recipe(self):
        source = self.root / WEB.rstrip('/').rsplit('/', 1)[0]
        calls = self.root / 'native-calls'
        tools = self.root / 'native-tools'
        tools.mkdir(exist_ok=True)
        executable = tools / 'just'
        executable.write_text('#!/bin/sh\nprintf "%s\\n" "$*" >> "$NATIVE_CALLS"\n')
        executable.chmod(0o755)
        for name in ('assets/react-hud/app.html', 'assets/bridge-scene/BevyExplorerUI/about'):
            self.write(str(source.relative_to(self.root) / name), 'built fixture')
        (source / 'crates/system_api_types').mkdir(parents=True, exist_ok=True)
        stamp = source / 'target/.bundle-native-stamp'
        stamp.parent.mkdir(parents=True, exist_ok=True)
        stamp.touch()
        os.utime(stamp, (time.time() + 30, time.time() + 30))
        recipe = (Path(__file__).parents[2] / 'rendering/bevy-explorer/source/justfile').read_text()
        recipe = recipe.split('_bundle-native-if-stale:\n', 1)[1].split('\n# build + run', 1)[0]
        recipe = '\n'.join(line[4:] for line in recipe.splitlines())
        recipe = recipe.replace('../../../tooling/hud_shared_inputs.py', shlex.quote(str(Path(__file__).parents[1] / 'hud_shared_inputs.py')))
        env = {**os.environ, 'PATH': str(tools) + os.pathsep + os.environ['PATH'], 'NATIVE_CALLS': str(calls)}
        return source, calls, tools, recipe, env

    def test_native_cache_stale_identity_rebuilds_and_malformed_inputs_never_execute(self):
        source, calls, tools, recipe, env = self.native_recipe()
        run = lambda: subprocess.run(['sh', '-c', recipe], cwd=source, env=env, capture_output=True, text=True)
        self.assertEqual(run().returncode, 0)
        self.assertFalse(calls.exists())
        self.write(WEB + 'src/main.ts', 'changed HUD without generation')
        self.assertEqual(run().returncode, 0)
        self.assertEqual(calls.read_text().strip(), 'bundle-native')
        calls.unlink()
        seal(self.root)
        self.write(WEB + 'package-lock.json', 'changed local dependency lock')
        self.assertEqual(run().returncode, 0)
        self.assertEqual(calls.read_text().strip(), 'bundle-native')
        calls.unlink()
        for failure in ('malformed', 'unsafe', 'missing'):
            with self.subTest(failure=failure):
                seal(self.root)
                if failure == 'malformed':
                    self.manifest(lambda data: data.update(schema=99))
                elif failure == 'unsafe':
                    self.manifest(lambda data: data['files'].append({'path': '../escape', 'sha256': 'a' * 64}))
                else:
                    (self.root / MANIFEST).unlink()
                result = run()
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertFalse(calls.exists())

    def test_native_new_symlink_roots_and_controls_abort_before_rebuild(self):
        source, calls, tools, recipe, env = self.native_recipe()
        manifest = (self.root / MANIFEST).read_bytes()
        destination = self.root / 'regular-target.ts'
        destination.write_text('export const value = 1')
        for name in (WEB + 'src/audit-new-link.ts', WEB + 'src/package.json'):
            with self.subTest(path=name):
                path = self.root / name
                path.symlink_to(destination)
                self.git('add', name)
                result = subprocess.run(['sh', '-c', recipe], cwd=source, env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertFalse(calls.exists())
                self.assertEqual(manifest, (self.root / MANIFEST).read_bytes())
                self.git('rm', '--cached', name)
                path.unlink()
        shutil.rmtree(self.root / '.git')
        path = self.root / (WEB + 'src/audit-new-link.ts')
        path.symlink_to(destination)
        result = subprocess.run(['sh', '-c', recipe], cwd=source, env=env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertFalse(calls.exists())
        self.assertEqual(manifest, (self.root / MANIFEST).read_bytes())

    def test_native_new_regular_root_remains_a_cache_miss_without_repinning(self):
        source, calls, tools, recipe, env = self.native_recipe()
        manifest = (self.root / MANIFEST).read_bytes()
        self.write(WEB + 'src/new-regular.ts', 'export const local = 1')
        self.git('add', WEB + 'src/new-regular.ts')
        result = subprocess.run(['sh', '-c', recipe], cwd=source, env=env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls.read_text().strip(), 'bundle-native')
        self.assertEqual(manifest, (self.root / MANIFEST).read_bytes())

    def test_native_git_failure_aborts_instead_of_rebuilding(self):
        source, calls, tools, recipe, env = self.native_recipe()
        executable = tools / 'git'
        executable.write_text('#!/bin/sh\nexit 42\n')
        executable.chmod(0o755)
        result = subprocess.run(['sh', '-c', recipe], cwd=source, env=env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertFalse(calls.exists())

    def test_native_freshness_io_failure_is_not_a_stale_identity(self):
        source, calls, tools, recipe, env = self.native_recipe()
        original = reader.input_hashes
        def changed_after_validation(root):
            result = original(root)
            (self.root / SHARED[0]).unlink()
            return result
        with patch.object(reader, 'input_hashes', side_effect=changed_after_validation), \
                patch('sys.argv', ['hud_shared_inputs.py', '--root', str(self.root), '--newer-than', str(source / 'target/.bundle-native-stamp')]):
            with self.assertRaises(SystemExit) as error:
                reader.main()
        self.assertEqual(error.exception.code, 2)
        self.assertFalse(calls.exists())

    def test_native_stamp_io_failure_aborts_instead_of_becoming_a_cache_miss(self):
        source, calls, tools, recipe, env = self.native_recipe()
        stamp = source / 'target/.bundle-native-stamp'
        original = reader.Path.stat
        def denied(path, *args, **kwargs):
            if path == stamp:
                raise PermissionError('stamp access denied')
            return original(path, *args, **kwargs)
        with patch.object(reader.Path, 'stat', denied), \
                patch('sys.argv', ['hud_shared_inputs.py', '--root', str(self.root), '--newer-than', str(stamp)]):
            with self.assertRaises(SystemExit) as error:
                reader.main()
        self.assertEqual(error.exception.code, 2)
        self.assertFalse(calls.exists())

    def test_unavailable_historical_proof_rebuilds_but_the_same_invalid_head_fails(self):
        for failure in ('missing', 'schema', 'malformed', 'stale', 'required-control'):
            with self.subTest(failure=failure):
                seal(self.root)
                if failure == 'missing':
                    (self.root / MANIFEST).unlink()
                elif failure == 'schema':
                    self.manifest(lambda data: data.update(schema=2))
                elif failure == 'malformed':
                    (self.root / MANIFEST).write_text('{invalid')
                elif failure == 'stale':
                    self.write(SHARED[0], 'changed historical source ' + failure)
                else:
                    self.manifest(lambda data: data.update(controls=data['controls'][1:]))
                invalid = self.commit()
                seal(self.root)
                head = self.commit()
                with patch.object(ci, 'ROOT', self.root):
                    self.assertTrue(ci.plan('local', invalid, head, 'unused')['web'])
                    with self.assertRaises(ci._artifact.shared_inputs.ManifestError):
                        ci.plan('local', head, invalid, 'unused')

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
