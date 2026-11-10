"""Checked graph validation and historical bootstrap contracts."""
import importlib.util
import contextlib
import io
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
from hud_manifest_fixture import seal, SHARED, WEB, MANIFEST, corpus_files, index_corpus_payload, corpus_manifest

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
        os.utime(self.root / (WEB + 'src/main.ts'), (time.time() + 60, time.time() + 60))
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

    def test_native_owned_root_deletion_and_rename_are_safe_misses_but_strict_reads_fail(self):
        source, calls, tools, recipe, env = self.native_recipe()
        name = WEB + 'src/main.ts'
        original = (self.root / name).read_bytes()
        manifest = (self.root / MANIFEST).read_bytes()
        for operation in ('delete', 'rename', 'unstaged-delete'):
            with self.subTest(operation=operation):
                if operation == 'delete':
                    self.git('rm', name)
                elif operation == 'rename':
                    self.git('mv', name, WEB + 'src/renamed.ts')
                else:
                    (self.root / name).unlink()
                with self.assertRaises(reader.StaleManifestError):
                    reader.read_manifest(self.root)
                result = subprocess.run(['sh', '-c', recipe], cwd=source, env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(calls.read_text().strip(), 'bundle-native')
                self.assertEqual(manifest, (self.root / MANIFEST).read_bytes())
                calls.unlink()
                if operation == 'rename':
                    (self.root / (WEB + 'src/renamed.ts')).unlink()
                    self.git('add', '-u', WEB + 'src/renamed.ts')
                (self.root / name).parent.mkdir(parents=True, exist_ok=True)
                (self.root / name).write_bytes(original)
                self.git('add', name)

    def test_native_missing_shared_or_resolver_and_unsafe_old_root_still_abort(self):
        source, calls, tools, recipe, env = self.native_recipe()
        manifest = (self.root / MANIFEST).read_bytes()
        for name in (SHARED[0], WEB + 'package.json', WEB + 'src/main.ts'):
            with self.subTest(path=name):
                path = self.root / name
                original = path.read_bytes()
                path.unlink()
                if name.endswith('main.ts'):
                    path.symlink_to(self.root / 'missing-target.ts')
                result = subprocess.run(['sh', '-c', recipe], cwd=source, env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertFalse(calls.exists())
                self.assertEqual(manifest, (self.root / MANIFEST).read_bytes())
                if path.is_symlink():
                    path.unlink()
                path.write_bytes(original)

    @unittest.skipIf(os.name == 'nt', 'Windows cannot materialize colon filenames')
    def test_literal_manifest_paths_cannot_exclude_engine_changes_or_disable_static_globs(self):
        name = ':(exclude)rendering'
        self.write(name, 'visible literal control')
        self.manifest(lambda data: data['controls'].append({'path': name, 'sha256': reader.sha(b'visible literal control')}))
        self.manifest(lambda data: data['controls'].sort(key=lambda row: row['path']))
        before = self.commit()
        self.write('rendering/bevy-explorer/source/src/lib.rs', 'changed engine source')
        head = self.commit()
        with patch.object(ci, 'ROOT', self.root):
            result = ci.select(before, head)
            self.assertTrue(all(result[lane] for lane in ('clippy', 'test', 'headless', 'web', 'scenes')))
        before = head
        self.write('tooling/tests/test_play_literal.py', 'static glob must still match')
        head = self.commit()
        with patch.object(ci, 'ROOT', self.root):
            self.assertIn('tooling/tests/test_play_literal.py', ci.changed_inputs(before, head, ci.INPUTS))
            self.assertFalse(any(ci.select(before, head)[lane] for lane in ci.LANES))

    def test_native_index_only_removed_owned_file_is_not_a_genuinely_missing_root(self):
        source, calls, tools, recipe, env = self.native_recipe()
        name = WEB + 'src/main.ts'
        self.git('rm', '--cached', name)
        self.assertTrue((self.root / name).is_file())
        result = subprocess.run(['sh', '-c', recipe], cwd=source, env=env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertFalse(calls.exists())

    def test_native_removed_old_src_resolver_aborts_instead_of_becoming_a_missing_root(self):
        name = WEB + 'src/package.json'
        self.write(name, '{}')
        seal(self.root)
        def record_control(data):
            data['files'] = [row for row in data['files'] if row['path'] != name]
            data['controls'] = [row for row in data['controls'] if row['path'] != name]
            data['controls'].append({'path': name, 'sha256': reader.sha(b'{}')})
            data['controls'].sort(key=lambda row: row['path'])
        self.manifest(record_control)
        self.commit()
        reader.read_manifest(self.root)
        source, calls, tools, recipe, env = self.native_recipe()
        manifest = (self.root / MANIFEST).read_bytes()
        self.git('rm', name)
        result = subprocess.run(['sh', '-c', recipe], cwd=source, env=env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertFalse(calls.exists())
        self.assertEqual(manifest, (self.root / MANIFEST).read_bytes())

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
                    self.manifest(lambda data: data.update(schema=1))
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

    def test_stale_owned_head_keeps_ci_strict_and_explains_the_canonical_regeneration(self):
        before = self.git('rev-parse', 'HEAD')
        original = (self.root / MANIFEST).read_bytes()
        self.write(WEB + 'src/main.ts', 'export {} // comment-only edit')
        head = self.commit()
        with patch.object(ci, 'ROOT', self.root):
            with self.assertRaises(ci._artifact.shared_inputs.StaleManifestError) as raised:
                ci.plan('local', before, head, 'unused')
        self.assertIn('Stale HUD input: ' + WEB + 'src/main.ts', str(raised.exception))
        self.assertIn('npm --prefix ' + WEB.rstrip('/') + ' run gen:shared-inputs', str(raised.exception))
        self.assertIn('repository root', str(raised.exception))
        self.assertEqual(original, (self.root / MANIFEST).read_bytes())

    def test_missing_owned_html_rebuilds_but_unsafe_or_untracked_html_never_executes(self):
        for name in ('app.html', 'index.html'):
            self.write(WEB + name, '<main>HUD</main>')
        seal(self.root)
        self.commit()
        source, calls, tools, recipe, env = self.native_recipe()
        manifest = (self.root / MANIFEST).read_bytes()
        for name in (WEB + 'app.html', WEB + 'index.html'):
            with self.subTest(path=name):
                path = self.root / name
                original = path.read_bytes()
                self.git('rm', name)
                with self.assertRaises(reader.StaleManifestError):
                    reader.read_manifest(self.root)
                result = subprocess.run(['sh', '-c', recipe], cwd=source, env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(calls.read_text().strip(), 'bundle-native')
                self.assertEqual(manifest, (self.root / MANIFEST).read_bytes())
                calls.unlink()
                path.write_bytes(original)
                self.git('add', name)
                for unsafe in ('symlink', 'untracked-present'):
                    with self.subTest(unsafe=unsafe):
                        if unsafe == 'symlink':
                            path.unlink()
                            path.symlink_to(self.root / 'missing-html-target')
                        else:
                            self.git('rm', '--cached', name)
                        result = subprocess.run(['sh', '-c', recipe], cwd=source, env=env, capture_output=True, text=True)
                        self.assertEqual(result.returncode, 2, result.stderr)
                        self.assertFalse(calls.exists())
                        self.assertEqual(manifest, (self.root / MANIFEST).read_bytes())
                        if path.is_symlink():
                            path.unlink()
                            path.write_bytes(original)
                        self.git('add', name)

    def test_windows_autocrlf_checkout_preserves_declared_controls_and_font_attributes(self):
        repository = Path(__file__).parents[2]
        attributes = (repository / '.gitattributes').read_text()
        protected = ('library/ui/package.json', 'library/ui/tsconfig.json',
                     'library/ui/foundations/typography/font-source.json', WEB + 'tsconfig.json')
        for name in protected:
            self.write(name, (repository / name).read_text())
        seal(self.root)
        self.manifest(lambda data: data['controls'].append({
            'path': 'library/ui/tsconfig.json',
            'sha256': reader.sha((self.root / 'library/ui/tsconfig.json').read_bytes())}))
        self.manifest(lambda data: data['controls'].sort(key=lambda row: row['path']))
        font = 'library/ui/foundations/typography/fonts/inter-latin-wght-normal.woff2'
        self.write('.gitattributes', attributes)
        self.commit()
        font_attributes = self.git('check-attr', 'filter', 'diff', 'merge', 'text', '--', font)
        self.assertEqual(font_attributes.splitlines(), [font + ': filter: lfs', font + ': diff: lfs',
                                                       font + ': merge: lfs', font + ': text: unset'])
        expected = reader.input_hashes(self.root)
        baseline = attributes.replace('library/ui/package.json text eol=lf\n', '').replace(
            'library/ui/tsconfig.json text eol=lf\n', '')
        for content, valid in ((baseline, False), (attributes, True)):
            with self.subTest(protected=valid), tempfile.TemporaryDirectory() as temporary:
                self.write('.gitattributes', content)
                self.git('add', '.gitattributes')
                checkout = Path(temporary)
                self.git('-c', 'core.autocrlf=true', 'checkout-index', '--all', '--force',
                         '--prefix=' + str(checkout) + os.sep)
                for name in protected:
                    expected_bytes = (self.root / name).read_bytes()
                    actual_bytes = (checkout / name).read_bytes()
                    should_match = valid or name not in protected[:2]
                    self.assertEqual(actual_bytes == expected_bytes, should_match, name)
                    self.assertEqual(reader.sha(actual_bytes) == reader.sha(expected_bytes), should_match, name)
                self.assertEqual(font_attributes,
                                 self.git('check-attr', 'filter', 'diff', 'merge', 'text', '--', font))
                if valid:
                    self.assertEqual(expected, reader.input_hashes(checkout))

    def test_shared_control_keeps_owned_engine_lane_classification(self):
        name = ci.SOURCE + 'crates/system_api_types/runtime-table.ts'
        self.write(name, 'export const version = 1')
        self.manifest(lambda data: data['controls'].append({
            'path': name, 'sha256': reader.sha((self.root / name).read_bytes())}))
        self.manifest(lambda data: data['controls'].sort(key=lambda row: row['path']))
        before = self.commit()
        self.write(name, 'export const version = 2')
        self.manifest(lambda data: [row.update(sha256=reader.sha((self.root / name).read_bytes()))
                                    for row in data['controls'] if row['path'] == name])
        head = self.commit()
        with patch.object(ci, 'ROOT', self.root):
            selected = ci.select(before, head)
        self.assertTrue(all(selected[lane] for lane in (*ci.LANES, 'scenes')))

    def test_unrelated_index_conflict_is_ignored_but_consumed_conflicts_never_execute(self):
        def conflict(name):
            entries = []
            for stage in (1, 2, 3):
                oid = subprocess.check_output(['git', 'hash-object', '-w', '--stdin'],
                                              cwd=self.root, input=('stage' + str(stage)).encode()).decode().strip()
                entries.append('100644 ' + oid + ' ' + str(stage) + '\t' + name + '\n')
            subprocess.check_output(['git', 'update-index', '--index-info'], cwd=self.root,
                                    input=('0 ' + '0' * 40 + '\t' + name + '\n' + ''.join(entries)).encode())
        self.write('library/shop/model/unconsumed.ts', 'export {}')
        seal(self.root)
        self.commit()
        source, calls, tools, recipe, env = self.native_recipe()
        original = (self.root / MANIFEST).read_bytes()
        conflict('apps/sites/unrelated.ts')
        conflict('library/shop/model/unconsumed.ts')
        reader.read_manifest(self.root)
        result = subprocess.run(['sh', '-c', recipe], cwd=source, env=env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(calls.exists())
        for name in (WEB + 'src/main.ts', WEB + 'package.json', SHARED[0]):
            with self.subTest(path=name):
                conflict(name)
                with self.assertRaisesRegex(reader.ManifestError, 'Unmerged HUD input'):
                    reader.read_manifest(self.root)
                result = subprocess.run(['sh', '-c', recipe], cwd=source, env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertFalse(calls.exists())
                self.assertEqual(original, (self.root / MANIFEST).read_bytes())
                self.git('add', name)

    def test_all_strict_planners_refuse_malformed_head_with_regeneration_guidance(self):
        before = self.git('rev-parse', 'HEAD')
        self.manifest(lambda data: data.update(schema=99))
        head = self.commit()
        for planner in (ci.plan, ci.scenes_plan):
            with self.subTest(planner=planner.__name__), patch.object(ci, 'ROOT', self.root):
                errors = io.StringIO()
                with contextlib.redirect_stderr(errors):
                    with self.assertRaises(ci._artifact.shared_inputs.ManifestError):
                        planner('local', before, head, '') if planner is ci.plan else planner('local', before, head)
                self.assertIn(reader.REGENERATE_COMMAND, errors.getvalue())
                self.assertIn('Unsupported HUD input manifest schema', errors.getvalue())

    def test_deleted_shared_input_remains_safe_abort_with_actionable_cli_guidance(self):
        source, calls, tools, recipe, env = self.native_recipe()
        original = (self.root / MANIFEST).read_bytes()
        self.git('rm', SHARED[0])
        result = subprocess.run(['sh', '-c', recipe], cwd=source, env=env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertFalse(calls.exists())
        self.assertIn(reader.REGENERATE_COMMAND, result.stderr)
        self.assertIn('Missing or untracked HUD input', result.stderr)
        self.assertEqual(original, (self.root / MANIFEST).read_bytes())

    def test_shared_invalid_manifest_corpus_matches_the_producer_contract(self):
        cases = json.loads((Path(__file__).with_name('hud_manifest_invalid_cases.json')).read_text())
        node_module = 'library/shop/node_modules/fixture.ts'
        self.write(node_module, 'export const dependency = 1')
        lfs_files = corpus_files(self.root)
        seal(self.root)
        self.commit()
        index_corpus_payload(self.root, lfs_files)
        baseline = json.loads((self.root / MANIFEST).read_text())
        for case in cases:
            with self.subTest(case=case['name']):
                data = corpus_manifest(baseline, case, self.root, lfs_files)
                (self.root / MANIFEST).write_text(json.dumps(data))
                if case['valid']:
                    reader.read_manifest(self.root)
                else:
                    with self.assertRaises(reader.ManifestError):
                        reader.read_manifest(self.root)
        (self.root / MANIFEST).write_text(json.dumps(baseline))
        reader.read_manifest(self.root)

    def test_finder_metadata_is_not_a_resolution_candidate_but_unsafe_aliases_are_refused(self):
        metadata = self.root / 'library/shop/.DS_Store'
        metadata.parent.mkdir(parents=True, exist_ok=True)
        # Use a witnessed directory rather than introducing a new source namespace.
        metadata = self.root / Path(SHARED[0]).parent / '.DS_Store'
        expected = reader.input_hashes(self.root)
        metadata.write_text('Finder metadata')
        self.assertEqual(reader.input_hashes(self.root), expected)
        metadata.unlink()
        metadata.symlink_to(self.root / SHARED[0])
        with self.assertRaisesRegex(reader.ManifestError, 'Unsafe'):
            reader.input_hashes(self.root)

    def test_namespace_shadow_blocks_old_artifact_and_cache_until_regeneration_without_engine_churn(self):
        source, calls, tools, recipe, env = self.native_recipe()
        for name in ci._artifact.INPUTS:
            self.write(name, 'synthetic producer input')
        self.write(ci.SOURCE + 'src/lib.rs', 'fn main() {}')
        old = 'library/ui/foundations/foo/index.ts'
        new = 'library/ui/foundations/foo.ts'
        self.write(old, 'export const shadow = 73')
        seal(self.root, shared=(*SHARED, old))
        before = self.commit()
        assembled = ci._artifact.inputs(self.root)
        engine = ci._artifact.engine_inputs(self.root)
        original = (self.root / MANIFEST).read_bytes()
        self.write(new, 'export const shadow = 99')
        stale_head = self.commit()
        with self.assertRaises(reader.StaleManifestError):
            reader.read_manifest(self.root)
        with self.assertRaises(ci._artifact.shared_inputs.StaleManifestError):
            ci._artifact.inputs(self.root)
        with patch.object(ci, 'ROOT', self.root):
            with self.assertRaises(ci._artifact.shared_inputs.StaleManifestError):
                ci.select(before, stale_head)
        result = subprocess.run(['sh', '-c', recipe], cwd=source, env=env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls.read_text().strip(), 'bundle-native')
        self.assertEqual(original, (self.root / MANIFEST).read_bytes())
        calls.unlink()
        seal(self.root, shared=(*SHARED, new))
        head = self.commit()
        self.assertNotEqual(assembled, ci._artifact.inputs(self.root))
        self.assertEqual(engine, ci._artifact.engine_inputs(self.root))
        with patch.object(ci, 'ROOT', self.root):
            selected = ci.select(before, head)
        self.assertTrue(selected['web'])
        self.assertFalse(any(selected[lane] for lane in ('clippy', 'test', 'headless')))
        self.write('library/ui/foundations/untracked-shadow.ts', 'export {}')
        result = subprocess.run(['sh', '-c', recipe], cwd=source, env=env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertFalse(calls.exists())

    def test_schema1_base_bootstraps_but_schema1_head_refuses_with_a_migration_hint(self):
        self.manifest(lambda data: (data.update(schema=1), data.pop('resolutionDirectories')))
        old = self.commit()
        seal(self.root)
        current = self.commit()
        with patch.object(ci, 'ROOT', self.root):
            self.assertTrue(ci.select(old, current)['web'])
            errors = io.StringIO()
            with contextlib.redirect_stderr(errors):
                with self.assertRaises(ci._artifact.shared_inputs.ManifestError):
                    ci.plan('local', current, old, '')
            self.assertIn(reader.REGENERATE_COMMAND, errors.getvalue())

    def test_oversized_json_integer_never_becomes_a_native_cache_miss(self):
        source, calls, tools, recipe, env = self.native_recipe()
        original = (self.root / MANIFEST).read_bytes()
        for field in ('schema', 'lfs'):
            with self.subTest(field=field):
                data = json.loads(original)
                if field == 'schema':
                    data['schema'] = 10 ** 400
                else:
                    row = data['files'][0]
                    row['lfs'] = {'oid': row['sha256'], 'size': 10 ** 400}
                (self.root / MANIFEST).write_text(json.dumps(data))
                with self.assertRaises(reader.ManifestError):
                    reader.read_manifest(self.root)
                result = subprocess.run(['sh', '-c', recipe], cwd=source, env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertFalse(calls.exists())
                self.assertNotIn('OverflowError', result.stderr)
        (self.root / MANIFEST).write_bytes(original)

    def test_native_owned_body_edit_rebuilds_once_but_structure_and_unsafe_inputs_never_hit(self):
        source, calls, tools, recipe, env = self.native_recipe()
        stamp = source / 'target/.bundle-native-stamp'
        name = WEB + 'src/main.ts'
        original = (self.root / MANIFEST).read_bytes()
        self.write(name, 'export {} // local body edit')
        os.utime(self.root / name, (time.time() + 60, time.time() + 60))
        run = lambda: subprocess.run(['sh', '-c', recipe], cwd=source, env=env, capture_output=True, text=True)
        self.assertEqual(run().returncode, 0)
        self.assertEqual(calls.read_text().strip(), 'bundle-native')
        calls.unlink()
        os.utime(stamp, (time.time() + 90, time.time() + 90))
        self.assertEqual(run().returncode, 0)
        self.assertFalse(calls.exists())
        with self.assertRaises(reader.StaleManifestError):
            reader.read_manifest(self.root)
        self.write(name, 'export {} // another local body edit')
        os.utime(self.root / name, (time.time() + 120, time.time() + 120))
        self.assertEqual(run().returncode, 0)
        self.assertEqual(calls.read_text().strip(), 'bundle-native')
        calls.unlink()
        os.utime(stamp, (time.time() + 150, time.time() + 150))
        self.write(WEB + 'src/new.ts', 'export {}')
        self.git('add', WEB + 'src/new.ts')
        self.assertEqual(run().returncode, 0)
        self.assertEqual(calls.read_text().strip(), 'bundle-native')
        calls.unlink()
        (self.root / name).unlink()
        (self.root / name).symlink_to(self.root / 'missing-target')
        self.assertEqual(run().returncode, 2)
        self.assertFalse(calls.exists())
        self.assertEqual(original, (self.root / MANIFEST).read_bytes())

    def test_resealed_lf_and_crlf_pointer_payloads_never_reach_native_or_artifact_acceptance(self):
        source, calls, tools, recipe, env = self.native_recipe()
        baseline = json.loads((self.root / MANIFEST).read_text())
        target = SHARED[0]
        for newline in ('\n', '\r\n'):
            for with_lfs in (False, True):
                with self.subTest(newline=repr(newline), lfs=with_lfs):
                    pointer = newline.join(('version https://git-lfs.github.com/spec/v1', 'oid sha256:' + 'a' * 64, 'size 64', '')).encode()
                    (self.root / target).write_bytes(pointer)
                    data = json.loads(json.dumps(baseline))
                    row = next(row for row in data['files'] if row['path'] == target)
                    row['sha256'] = reader.sha(pointer)
                    if with_lfs:
                        row['lfs'] = {'oid': row['sha256'], 'size': len(pointer)}
                    (self.root / MANIFEST).write_text(json.dumps(data))
                    original = (self.root / MANIFEST).read_bytes()
                    with self.assertRaisesRegex(reader.ManifestError, 'Unmaterialized'):
                        reader.read_manifest(self.root)
                    with self.assertRaisesRegex(ci._artifact.shared_inputs.ManifestError, 'Unmaterialized'):
                        ci._artifact.inputs(self.root)
                    result = subprocess.run(['sh', '-c', recipe], cwd=source, env=env, capture_output=True, text=True)
                    self.assertEqual(result.returncode, 2, result.stderr)
                    self.assertFalse(calls.exists())
                    self.assertEqual(original, (self.root / MANIFEST).read_bytes())

    def test_unhashed_root_is_rejected(self):
        self.manifest(lambda data: data.update(controls=[row for row in data['controls']
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

    def test_current_lfs_pointer_is_refused_and_historical_logical_identity_is_preserved(self):
        payload = 'wOF2 fixture'
        oid = reader.sha(payload.encode())
        self.write(SHARED[0], payload)
        seal(self.root)
        self.manifest(lambda data: data['files'][0].update(lfs={'oid': oid, 'size': len(payload)}))
        expected = reader.input_hashes(self.root)
        self.write(SHARED[0], f'version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize {len(payload)}\n')
        revision = self.commit()
        with self.assertRaisesRegex(reader.ManifestError, 'Unmaterialized'):
            reader.input_hashes(self.root)
        self.assertEqual(expected, reader.input_hashes(self.root, revision))
        self.write(SHARED[0], f'version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize 1\n')
        revision = self.commit()
        with self.assertRaisesRegex(reader.ManifestError, 'Stale'):
            reader.read_manifest(self.root, revision)


if __name__ == '__main__':
    unittest.main()
