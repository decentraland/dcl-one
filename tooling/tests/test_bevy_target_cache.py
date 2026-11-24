"""The Explorer's native Cargo cache: keyed by what Cargo's fingerprints depend on, with the workspace crates in it."""
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('bevy_target_cache', ROOT / 'tooling/bevy-target-cache.py')
cache = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(cache)

ENV = {'RUNNER_OS': 'Linux', 'RUNNER_ARCH': 'X64', 'CC': '/opt/hostedtoolcache/llvm-21.1.8/bin/clang',
       'CXX': '/opt/hostedtoolcache/llvm-21.1.8/bin/clang++', 'CEF_PATH': '/opt/cef',
       'LD_LIBRARY_PATH': '/opt/cef', 'PATH': os.environ['PATH']}
RUSTC = 'rustc 1.99.1 (abcdef012 2026-10-01)\nhost: x86_64-unknown-linux-gnu\n'


class KeyTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.source = self.root / 'rendering/bevy-explorer/source'
        self.git('init', '-q')
        self.git('config', 'maintenance.auto', 'false')
        self.git('config', 'gc.auto', '0')
        for name, text in {'rendering/bevy-explorer/vendor/bevy_dui/src/lib.rs': 'pub fn layout() {}\n',
                           'library/vendor/rusty_enet/src/lib.rs': 'pub fn host() {}\n',
                           'rendering/bevy-explorer/source/vendor/basisu/src/lib.rs': 'pub fn transcode() {}\n',
                           'rendering/bevy-explorer/source/Cargo.lock': 'version = 4\n',
                           'rendering/bevy-explorer/source/Cargo.toml': '[workspace]\nmembers = ["crates/*"]\n',
                           'rendering/bevy-explorer/source/.cargo/config.toml': '[env]\nRUST_BACKTRACE = "1"\n',
                           '.cargo/config.toml': '[env]\nDCL_ONE_ROOT = { value = "", relative = true }\n',
                           'rendering/bevy-explorer/source/crates/common/Cargo.toml': '[package]\nname = "common"\n',
                           'rendering/bevy-explorer/source/src/lib.rs': 'fn main() {}\n',
                           'library/protocol/proto/decentraland/common/vectors.proto': 'syntax = "proto3";\n'}.items():
            self.write(name, text)
        self.commit()
        manifests = [self.source / 'Cargo.toml', self.source / 'crates/common/Cargo.toml']
        for target, value in (('workspace', manifests), ('rustc', RUSTC)):
            patcher = patch.object(cache, target, return_value=value)
            patcher.start()
            self.addCleanup(patcher.stop)

    def git(self, *args):
        return subprocess.check_output(['git', '-c', 'user.name=CI', '-c', 'user.email=ci@example.invalid', *args],
                                       cwd=self.root, text=True).strip()

    def write(self, name, text):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def commit(self):
        self.git('add', '.')
        self.git('commit', '-qm', 'fixture')

    def key(self, lane='test', env=ENV):
        return cache.key(lane, env, self.root, self.source)

    def test_key_names_lane_platform_compiler_tools_sources_and_members(self):
        exact, fallbacks = self.key()
        self.assertRegex(exact, r'^bevy-native-v1-test-Linux-X64-[0-9a-f]{16}-[0-9a-f]{16}-[0-9a-f]{16}-[0-9a-f]{16}$')
        self.assertEqual(fallbacks, [exact[:-17], exact[:-33], exact[:-50]])
        self.assertNotEqual(self.key('clippy')[0], exact)
        self.assertNotEqual(self.key(env={**ENV, 'RUNNER_OS': 'macOS', 'RUNNER_ARCH': 'ARM64'})[0], exact)

    def test_the_scheduled_cleanup_bounds_the_caches_of_this_key(self):
        # The cleanup keeps the newest cache of each prefix on a ref; the prefix ends with the platform.
        cleanup = (ROOT / '.github/workflows/closed-pr-cache.yml').read_text()
        family = re.search(r"r'\|\^(bevy-native-[^']+)'", cleanup).group(1)
        for lane, system, arch in (('test', 'Linux', 'X64'), ('clippy', 'macOS', 'ARM64'), ('headless', 'Windows', 'X64')):
            exact = self.key(lane, {**ENV, 'RUNNER_OS': system, 'RUNNER_ARCH': arch})[0]
            self.assertEqual(re.match(family, exact).group(0), exact[:-67])

    def test_the_tool_environment_changes_the_tools_digest_only(self):
        exact, fallbacks = self.key()
        moved, moved_fallbacks = self.key(env={**ENV, 'CC': '/opt/hostedtoolcache/llvm-22.1.0/bin/clang'})
        self.assertNotEqual(moved, exact)
        self.assertEqual(moved[-16:], exact[-16:])
        self.assertEqual(moved[-33:], exact[-33:])
        self.assertNotEqual(moved_fallbacks[0], fallbacks[0])
        self.assertNotEqual(moved_fallbacks[1], fallbacks[1])
        self.assertEqual(moved_fallbacks[2], fallbacks[2])
        self.assertEqual(self.key(env={**ENV, 'LD_LIBRARY_PATH': '/elsewhere', 'PATH': '/opt/bin'})[0], exact)

    def test_cargo_inputs_and_vendored_trees_change_the_sources_digest_only(self):
        exact, fallbacks = self.key()
        for name, text in (('rendering/bevy-explorer/source/Cargo.lock', 'version = 4\n[[package]]\nname = "cc"\n'),
                           ('rendering/bevy-explorer/source/crates/common/Cargo.toml', '[package]\nname = "common"\nversion = "0.2.0"\n'),
                           ('rendering/bevy-explorer/source/.cargo/config.toml', '[env]\nRUST_BACKTRACE = "full"\n'),
                           ('.cargo/config.toml', '[build]\nrustflags = ["-Cdebuginfo=0"]\n'),
                           ('rendering/bevy-explorer/vendor/bevy_dui/src/lib.rs', 'pub fn layout() { }\n'),
                           ('library/vendor/rusty_enet/src/lib.rs', 'pub fn host() { }\n'),
                           ('rendering/bevy-explorer/source/vendor/basisu/src/lib.rs', 'pub fn transcode() { }\n')):
            with self.subTest(name=name):
                self.write(name, text)
                self.commit()
                changed, changed_fallbacks = self.key()
                self.assertNotEqual(changed, exact)
                self.assertNotEqual(changed_fallbacks[0], fallbacks[0])
                self.assertEqual(changed_fallbacks[1:], fallbacks[1:])
                exact, fallbacks = changed, changed_fallbacks

    def test_workspace_sources_and_protocol_definitions_change_the_members_digest_only(self):
        # The entry holds the workspace crates: main saves one for each state of what they compile from,
        # and a pull request finds the newest of them under the unchanged first fallback.
        exact, fallbacks = self.key()
        for name, text in (('rendering/bevy-explorer/source/src/lib.rs', 'fn main() { }\n'),
                           ('library/protocol/proto/decentraland/common/vectors.proto', 'syntax = "proto3";\npackage common;\n')):
            with self.subTest(name=name):
                self.write(name, text)
                self.commit()
                changed, changed_fallbacks = self.key()
                self.assertNotEqual(changed, exact)
                self.assertEqual(changed[:-16], exact[:-16])
                self.assertEqual(changed_fallbacks, fallbacks)
                exact = changed

    def test_a_file_outside_the_cargo_inputs_changes_no_key(self):
        exact = self.key()[0]
        self.write('docs/development.md', 'notes\n')
        self.commit()
        self.assertEqual(self.key()[0], exact)

    def test_a_compiler_change_shares_no_fallback(self):
        exact, fallbacks = self.key()
        with patch.object(cache, 'rustc', return_value='rustc 1.100.0 (123456789 2026-11-20)\n'):
            bumped, bumped_fallbacks = self.key()
        self.assertNotEqual(bumped, exact)
        self.assertFalse(set(bumped_fallbacks) & set(fallbacks))

    def test_the_key_command_writes_github_output_in_its_multiline_form(self):
        exact, fallbacks = self.key()
        output = self.root / 'output'
        with patch('sys.argv', ['bevy-target-cache.py', 'key', '--lane', 'test']), \
                patch.dict(os.environ, {**ENV, 'GITHUB_OUTPUT': str(output)}, clear=True), \
                patch.object(cache, 'ROOT', self.root), patch.object(cache, 'SOURCE', self.source), \
                patch('sys.stdout', new_callable=io.StringIO) as stdout:
            cache.main()
        self.assertEqual(output.read_text(), f'key={exact}\ndependencies={fallbacks[0]}\nrestore-keys<<KEYS\n'
                                             f'{fallbacks[0]}\n{fallbacks[1]}\n{fallbacks[2]}\nKEYS\n')
        self.assertTrue(exact.startswith(fallbacks[0] + '-'))
        self.assertIn(exact, stdout.getvalue())


class PruneTest(unittest.TestCase):
    KEPT = ('release/deps/libbevy_ecs-1a.rlib', 'release/deps/libbevy_ecs-1a.rmeta', 'release/deps/bevy_ecs-1a.d',
            'release/deps/libbasisu-2b.rlib', 'release/deps/liblibsqlite3_sys-3c.rlib',
            'release/build/openssl-sys-4d/output', 'release/build/openssl-sys-4d/out/lib/libssl.a',
            'release/build/basisu-2b/output', 'release/.fingerprint/bevy_ecs-1a/lib-bevy_ecs',
            'release/.fingerprint/openssl-sys-4d/run-build-script-build-script-build', 'release/.fingerprint/bevy-mtimes.json',
            'ci-test/deps/libbevy_app-5e.rlib', 'ci-test/.fingerprint/bevy_app-5e/lib-bevy_app',
            'x86_64-unknown-linux-gnu/release/deps/libcc-6f.rlib', 'x86_64-unknown-linux-gnu/release/.fingerprint/cc-6f/lib-cc',
            'CACHEDIR.TAG', '.rustc_info.json')
    # The workspace crates' artifacts, build script outputs and fingerprints stay: a later run reuses them.
    WORKSPACE = ('release/deps/libwebgpu_build-7a.rlib', 'release/deps/webgpu_build-7a.d', 'release/deps/benchmark-8b',
                 'release/deps/benchmark-8b.exe', 'release/deps/benchmark-8b.pdb', 'release/deps/libharness_util-9c.rmeta',
                 'release/build/decentra-bevy-7a/output', 'release/build/decentra-bevy-7a/build_script_build-7a',
                 'release/.fingerprint/decentra-bevy-7a/lib-webgpu_build', 'release/.fingerprint/benchmark-8b/bin-benchmark',
                 'ci-test/deps/webgpu_build-7b', 'ci-test/.fingerprint/decentra-bevy-7b/test-lib-webgpu_build',
                 'x86_64-unknown-linux-gnu/release/deps/libdcl_scene_wasm-0d.rlib')
    PRUNED = ('release/incremental/webgpu_build-xyz/s-abc.lock', 'release/examples/demo-1', 'release/avatar_render',
              'release/.cargo-lock', 'ci-test/webgpu_build', 'x86_64-unknown-linux-gnu/release/dcl_scene_wasm.wasm',
              'cargo-timings/cargo-timing.html', 'native-test-artifacts.jsonl', 'sccache-stats.json', 'tmp/scratch')
    # The Cargo home is the runner's own: the prune removes nothing in it, saved path or not.
    HOME_KEPT = ('registry/index/index.crates.io-1cd66030c949c28d/config.json',
                 'registry/cache/index.crates.io-1cd66030c949c28d/cc-1.2.57.crate', 'git/db/livekit-abc/HEAD', 'bin/export-cef-dir',
                 'registry/src/index.crates.io-1cd66030c949c28d/cc-1.2.57/src/lib.rs',
                 'git/checkouts/livekit-abc/def/Cargo.toml', '.package-cache', '.package-cache-mutate', 'credentials.toml')

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        # Resolved, as the prune command resolves its target: the macOS temporary directory is behind a link.
        root = Path(temporary.name).resolve()
        self.target = root / 'target'
        self.home = root / 'cargo-home'
        for name in self.KEPT + self.WORKSPACE + self.PRUNED:
            self.write(self.target / name)
        for name in self.HOME_KEPT:
            self.write(self.home / name)

    def write(self, path):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text('x')

    def assert_pruned(self):
        for name in self.KEPT + self.WORKSPACE:
            self.assertTrue((self.target / name).exists(), name)
        for name in self.PRUNED:
            self.assertFalse((self.target / name).exists(), name)
        for name in self.HOME_KEPT:
            self.assertTrue((self.home / name).exists(), name)
        for directory in ('release/incremental', 'release/examples', 'tmp', 'cargo-timings'):
            self.assertFalse((self.target / directory).exists(), directory)

    def test_prune_keeps_every_crates_artifacts_and_the_cargo_home_and_drops_final_binaries_and_reports(self):
        removed = []
        cache.prune_target(self.target, removed)
        self.assert_pruned()
        for name in self.PRUNED:
            path = (self.target / name).resolve()
            self.assertTrue(any(path == gone or gone in path.parents for gone in removed), name)
        for name in self.KEPT + self.WORKSPACE + tuple(f'../cargo-home/{name}' for name in self.HOME_KEPT):
            path = (self.target / name).resolve()
            self.assertFalse(any(path == gone or gone in path.parents for gone in removed), name)

    def test_the_prune_command_reports_what_it_removed(self):
        with patch('sys.argv', ['bevy-target-cache.py', 'prune', str(self.target)]), \
                patch.dict(os.environ, {'CARGO_HOME': str(self.home)}), \
                patch('sys.stdout', new_callable=io.StringIO) as stdout:
            cache.main()
        self.assert_pruned()
        self.assertRegex(stdout.getvalue(), r'^::warning::no Cargo message in \S+ names a unit: every build generation stays '
                                            r'in the cache\n\d+ entries pruned; \d+ MB left in \S+\n$')

    def test_the_size_does_not_follow_a_link_and_skips_what_it_cannot_read(self):
        # A build script of a git dependency leaves a link to the crate's checkout in its output.
        outside = self.target.parent / 'checkout'
        (outside / 'src').mkdir(parents=True)
        (outside / 'src/lib.rs').write_bytes(b'x' * 4096)
        out = self.target / 'release/build/webrtc-sys-5a/out/cxxbridge/crate'
        out.mkdir(parents=True)
        os.symlink(outside, out / 'webrtc-sys', target_is_directory=True)
        os.symlink(self.target / 'missing', self.target / 'dangling')
        measured = cache.size(self.target)
        self.assertLess(measured, 4096)
        scandir = os.scandir

        def refuse(path):  # On Windows a listing at the link failed with WinError 5.
            if Path(path) == out:
                raise PermissionError(13, 'Access is denied')
            return scandir(path)
        with patch.object(cache.os, 'scandir', side_effect=refuse):
            self.assertEqual(cache.size(self.target), measured)
        with patch.object(cache.os, 'lstat', side_effect=PermissionError(13, 'Access is denied')):
            self.assertEqual(cache.size(self.target), 0)
        self.assertEqual(cache.size(self.target / 'absent'), 0)


def unit(prefix):
    return prefix * 8


class GenerationTest(unittest.TestCase):
    """A dependency change leaves the old build of every unit it reaches; the prune keeps only the units Cargo named."""
    LIVE, STALE = unit('a1'), unit('b2')
    KEPT = (f'release/deps/libbevy_ecs-{LIVE}.rlib', f'release/deps/libbevy_ecs-{LIVE}.rmeta',
            f'release/deps/bevy_ecs-{LIVE}.d', f'release/deps/bevy_ecs-{LIVE}.bevy_ecs.abc123-cgu.0.rcgu.o',
            f'release/.fingerprint/bevy_ecs-{LIVE}/lib-bevy_ecs',
            f'release/build/openssl-sys-{unit("c3")}/build-script-build',
            f'release/.fingerprint/openssl-sys-{unit("c3")}/build-script-build-script-build',
            f'release/build/openssl-sys-{unit("d4")}/out/lib/libssl.a', f'release/build/openssl-sys-{unit("d4")}/output',
            f'release/.fingerprint/openssl-sys-{unit("d4")}/run-build-script-build-script-build',
            f'release/.fingerprint/decentra-bevy-{unit("a6")}/bin-decentra-bevy',
            f'release/.fingerprint/headless-{unit("a8")}/bin-headless',
            'release/deps/libwebgpu_build.rlib', 'release/deps/libwebgpu_build.so',
            f'release/.fingerprint/decentra-bevy-{unit("f0")}/lib-webgpu_build',
            'release/deps/benchmark.exe', f'release/.fingerprint/benchmark-{unit("0f")}/bin-benchmark',
            'release/.fingerprint/bevy-mtimes.json',
            f'release/deps/sidecar-{unit("aa")}', f'release/deps/sidecar-{unit("ab")}',
            f'fork-tests/debug/deps/libbevy_image-{unit("ac")}.rlib',
            f'wasm32-unknown-unknown/debug/deps/dcl_wasm-{unit("ad")}.wasm')
    STALE_ENTRIES = (f'release/deps/libbevy_ecs-{STALE}.rlib', f'release/deps/libbevy_ecs-{STALE}.rmeta',
                     f'release/deps/bevy_ecs-{STALE}.d', f'release/.fingerprint/bevy_ecs-{STALE}/lib-bevy_ecs',
                     f'release/build/openssl-sys-{unit("e5")}/output',
                     f'release/.fingerprint/openssl-sys-{unit("e5")}/run-build-script-build-script-build',
                     f'release/deps/decentra_bevy-{unit("b7")}', f'release/.fingerprint/decentra-bevy-{unit("b7")}/bin-decentra-bevy',
                     f'release/deps/headless-{unit("b9")}',
                     f'fork-tests/debug/deps/libbevy_image-{unit("bc")}.rlib',
                     f'wasm32-unknown-unknown/debug/deps/dcl_wasm-{unit("bd")}.wasm')

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.target = Path(temporary.name).resolve() / 'target'
        for name in self.KEPT + self.STALE_ENTRIES:
            self.write(name, 'x')
        # Cargo links a built binary into the profile on Linux and copies it on macOS.
        self.write('release/decentra-bevy', 'engine')
        os.link(self.target / 'release/decentra-bevy', self.target / f'release/deps/decentra_bevy-{unit("a6")}')
        self.write('release/headless', 'headless')
        self.write(f'release/deps/headless-{unit("a8")}', 'headless')
        self.write(f'release/deps/headless-{unit("b9")}', 'headless, older')
        self.write('release/sidecar', 'no deps file holds these bytes')
        artifact = self.artifact
        self.messages = [artifact(f'release/deps/libbevy_ecs-{self.LIVE}.rlib', f'release/deps/libbevy_ecs-{self.LIVE}.rmeta'),
                         artifact(f'release/build/openssl-sys-{unit("c3")}/build-script-build'),
                         {'reason': 'build-script-executed', 'out_dir': str(self.target / f'release/build/openssl-sys-{unit("d4")}/out')},
                         artifact('release/decentra-bevy', executable='release/decentra-bevy'),
                         artifact('release/headless', executable='release/headless'),
                         artifact('release/sidecar', executable='release/sidecar'),
                         artifact('release/libwebgpu_build.so', 'release/libwebgpu_build.rlib'),
                         artifact('release/benchmark.exe', executable='release/benchmark.exe'),
                         artifact(f'fork-tests/debug/deps/libbevy_image-{unit("ac")}.rlib'),
                         artifact(f'wasm32-unknown-unknown/debug/deps/dcl_wasm-{unit("ad")}.wasm'),
                         {'reason': 'compiler-message', 'message': {'rendered': 'warning: unused'}},
                         {'reason': 'build-finished', 'success': True}]

    def write(self, name, text):
        path = self.target / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def artifact(self, *names, executable=None):
        return {'reason': 'compiler-artifact', 'fresh': True, 'filenames': [str(self.target / name) for name in names],
                'executable': executable and str(self.target / executable)}

    def prune(self, lines):
        self.write('cargo-messages/native-tests.jsonl', ''.join(f'{line}\n' for line in lines))
        with patch('sys.argv', ['bevy-target-cache.py', 'prune', str(self.target)]), \
                patch('sys.stdout', new_callable=io.StringIO) as stdout:
            cache.main()
        return stdout.getvalue()

    def test_the_prune_keeps_the_units_cargo_named_and_removes_the_other_generations(self):
        output = self.prune(['test result: ok. 3 passed', *map(json.dumps, self.messages)])
        for name in self.KEPT:
            self.assertTrue((self.target / name).exists(), name)
        for name in self.STALE_ENTRIES:
            self.assertFalse((self.target / name).exists(), name)
        self.assertFalse((self.target / 'cargo-messages').exists())
        self.assertRegex(output, r'^\d+ live units; 11 entries of stale generations removed \(\d+ MB\)\n'
                                 r'\d+ entries pruned; \d+ MB left in \S+\n$')

    def test_an_msvc_executable_keeps_no_hash_so_only_its_live_test_harness_stays(self):
        live, stale = unit('c1'), unit('c2')
        kept = ('ci-test/deps/dcl_deno_ipc.exe', f'ci-test/deps/dcl_deno_ipc-{live}.exe', f'ci-test/deps/dcl_deno_ipc-{live}.pdb',
                f'ci-test/.fingerprint/dcl_deno_ipc-{live}/test-bin-dcl_deno_ipc',
                f'ci-test/.fingerprint/dcl_deno_ipc-{unit("ce")}/bin-dcl_deno_ipc')
        gone = (f'ci-test/deps/dcl_deno_ipc-{stale}.exe', f'ci-test/deps/dcl_deno_ipc-{stale}.pdb',
                f'ci-test/.fingerprint/dcl_deno_ipc-{stale}/test-bin-dcl_deno_ipc')
        for name in kept + gone:
            self.write(name, 'harness')
        self.write('ci-test/deps/dcl_deno_ipc.exe', 'sidecar')
        os.link(self.target / 'ci-test/deps/dcl_deno_ipc.exe', self.target / 'ci-test/dcl_deno_ipc.exe')
        self.prune(map(json.dumps, [*self.messages,
                                    self.artifact('ci-test/dcl_deno_ipc.exe', executable='ci-test/dcl_deno_ipc.exe'),
                                    self.artifact(f'ci-test/deps/dcl_deno_ipc-{live}.exe', f'ci-test/deps/dcl_deno_ipc-{live}.pdb',
                                                  executable=f'ci-test/deps/dcl_deno_ipc-{live}.exe')]))
        for name in kept:
            self.assertTrue((self.target / name).exists(), name)
        for name in gone:
            self.assertFalse((self.target / name).exists(), name)

    def test_a_binary_keeps_its_linked_unit_over_an_equal_copy_and_every_candidate_when_unsure(self):
        linked, equal, copy, other, gone, also_gone = (unit(p) for p in ('d1', 'd2', 'd3', 'd4', 'd5', 'd6'))
        self.write(f'release/deps/app-{equal}', 'app')
        self.write('release/app', 'app')
        os.link(self.target / 'release/app', self.target / f'release/deps/app-{linked}')
        for name in (f'release/deps/tool-{copy}', f'release/deps/tool-{other}', 'release/tool'):
            self.write(name, 'tool')
        for name in (f'release/deps/gone-{gone}', f'release/deps/gone-{also_gone}'):
            self.write(name, 'gone')
        self.prune(map(json.dumps, [*self.messages, *(self.artifact(f'release/{name}', executable=f'release/{name}')
                                                       for name in ('app', 'tool', 'gone'))]))
        self.assertTrue((self.target / f'release/deps/app-{linked}').exists())
        self.assertFalse((self.target / f'release/deps/app-{equal}').exists())
        for name in (f'tool-{copy}', f'tool-{other}', f'gone-{gone}', f'gone-{also_gone}'):
            self.assertTrue((self.target / 'release/deps' / name).exists(), name)

    def test_an_uplifted_library_keeps_its_linked_unit(self):
        live, stale = unit('e1'), unit('e2')
        for name, text in ((f'release/deps/libpm-{live}.so', 'pm'), (f'release/deps/libpm-{stale}.so', 'pm, older'),
                           (f'release/.fingerprint/pm-{live}/lib-pm', 'x'), (f'release/.fingerprint/pm-{stale}/lib-pm', 'x')):
            self.write(name, text)
        os.link(self.target / f'release/deps/libpm-{live}.so', self.target / 'release/libpm.so')
        self.prune(map(json.dumps, [*self.messages, self.artifact('release/libpm.so')]))
        for name in (f'release/deps/libpm-{live}.so', f'release/.fingerprint/pm-{live}/lib-pm'):
            self.assertTrue((self.target / name).exists(), name)
        for name in (f'release/deps/libpm-{stale}.so', f'release/.fingerprint/pm-{stale}/lib-pm'):
            self.assertFalse((self.target / name).exists(), name)

    def test_without_a_message_that_names_a_unit_every_generation_stays(self):
        for lines in ([], ['not json'], [json.dumps({'reason': 'build-finished', 'success': True})]):
            with self.subTest(lines=lines):
                output = self.prune(lines)
                self.assertTrue(output.startswith('::warning::no Cargo message in '), output)
                for name in self.KEPT + self.STALE_ENTRIES:
                    self.assertTrue((self.target / name).exists(), name)

    def test_an_entry_that_cannot_be_removed_stays_and_is_reported(self):
        stale = self.target / f'release/deps/libbevy_ecs-{self.STALE}.rlib'
        remove = cache.remove

        def refuse(path, removed):
            if path == stale:
                raise PermissionError(13, 'Access is denied')
            remove(path, removed)
        self.write('cargo-messages/native-tests.jsonl', ''.join(f'{json.dumps(m)}\n' for m in self.messages))
        live = cache.live_units(self.target)
        with patch.object(cache, 'remove', side_effect=refuse):
            _, failed = cache.prune_generations(self.target, live, [])
        self.assertTrue(stale.exists())
        self.assertEqual(len(failed), 1)
        self.assertIn('Access is denied', failed[0])
        self.assertFalse((self.target / f'release/deps/libbevy_ecs-{self.STALE}.rmeta').exists())


class WorkflowTest(unittest.TestCase):
    """The native jobs key after their tool steps, build after the restore and save from main's first attempts;
    a pull request saves only when the restored entry was not built for its dependencies."""
    WORKFLOW = (ROOT / '.github/workflows/bevy-explorer-ci.yml').read_text()
    GUARD = ("        if: >-\n"
             "          github.run_attempt == '1' && steps.target-cache.outputs.cache-hit != 'true' &&\n"
             "          (github.ref == 'refs/heads/main' || (github.event_name == 'pull_request' &&\n"
             "          github.event.pull_request.head.repo.full_name == github.repository &&\n"
             "          !startsWith(steps.target-cache.outputs.cache-matched-key, steps.target-cache-key.outputs.dependencies)))\n")
    PATHS = ('          path: |\n            ~/.cargo/registry/index/\n            ~/.cargo/registry/cache/\n'
             '            ~/.cargo/git/db/\n            rendering/bevy-explorer/source/target/\n')

    def job(self, name):
        return re.search(rf'^  {name}:\n(.*?)(?=^  [\w-]+:|\Z)', self.WORKFLOW, re.M | re.S).group(1)

    def test_no_explorer_job_uses_rust_cache(self):
        self.assertNotIn('Swatinem/rust-cache', self.WORKFLOW)
        self.assertNotIn('prepare-native-cache.py', self.WORKFLOW)

    def test_native_jobs_restore_after_their_tool_steps_and_save_after_their_uploads(self):
        for name, lane, last_tool, build in (('test', 'test', 'Export CEF distribution', 'cargo test --manifest-path vendor/basisu'),
                                             ('headless-smoke', 'headless', 'Install clang for webrtc-sys', 'build-headless.py'),
                                             ('clippy', 'clippy', 'Install clang for webrtc-sys', 'cargo clippy'),
                                             ('relay-wake', 'relay-wake', 'Install the pinned WASM binding generator and its test runner',
                                              'cargo test --locked -p dcl_wasm')):
            with self.subTest(job=name):
                job = self.job(name)
                self.assertIn("CARGO_INCREMENTAL: '0'", job)
                key = job.index(f'bevy-target-cache.py" key --lane {lane}')
                self.assertLess(job.index('dtolnay/rust-toolchain'), key)
                self.assertLess(job.index(f'name: {last_tool}'), key)
                restore = job.index('actions/cache/restore@', key)
                self.assertEqual(job.count('key: ${{ steps.target-cache-key.outputs.key }}'), 2)
                self.assertIn('restore-keys: ${{ steps.target-cache-key.outputs.restore-keys }}', job[restore:])
                mtimes = job.index('bevy-mtimes.py" restore target', restore)
                self.assertLess(mtimes, job.index(build))
                record = job.index('bevy-mtimes.py" record target')
                prune = job.index('- name: Prune the ')
                save = job.index('actions/cache/save@', prune)
                self.assertLess(job.index('bevy-target-cache.py" prune target', prune), save)
                self.assertLess(job.index(build), record)
                self.assertLess(record, prune)
                self.assertLess(prune, save)
                if 'actions/upload-artifact@' in job:
                    self.assertLess(job.rindex('actions/upload-artifact@'), prune)
                self.assertEqual(job.count(self.GUARD), 2)
                self.assertEqual(job.count(self.PATHS), 2)

    # A Cargo call in shell or in a script's argument list, aliases included; `metadata` builds nothing.
    CALL = re.compile(r"""\bcargo['"]?,?\s*['"]?(\+\S+\s+)?(build|test|check|clippy|run|doc|nextest|rustc|bench|[btcrd])\b""")
    # Scripts the native jobs run that call Cargo, and why their units are still named.
    CARGO_SCRIPTS = {'apps/bevy-explorer/build-headless.py': 'writes CARGO_MESSAGES',
                     'tooling/dev-regressions.py': 'runs no Cargo with --bevy-artifacts'}

    def test_every_cargo_call_of_a_native_job_writes_its_messages(self):
        # A call that writes none loses its units at every save, and every later run rebuilds them.
        def units(line):
            # What selects the units: the env prefix and arguments, without test arguments, filters, output-only flags or redirect.
            start = self.CALL.search(line).start()
            prefix = re.search(r'((?:[A-Za-z_]\w*=\S*\s+)*)$', line[:start]).group(1)
            command = re.split(r' -- |\s[>|]', prefix + line[start:])[0]
            return [word for word in command.split()
                    if '::' not in word and word not in ('--no-run', '--timings', '--message-format=json-render-diagnostics')]
        for name in ('test', 'headless-smoke', 'clippy', 'relay-wake'):
            job = self.job(name)
            self.assertRegex(job, r'(?m)^    defaults:\n      run:\n(?:        .*\n)*?        working-directory: rendering/bevy-explorer/source$')
            owners = set()
            for step in re.split(r'^      - ', job, flags=re.M):
                step = re.split(r'(?m)^    \S', step)[0]
                # One file per step, or a later step overwrites the units an earlier one named.
                for file in set(re.findall(r'target/cargo-messages/([\w.-]+\.jsonl)', step)):
                    with self.subTest(job=name, file=file):
                        self.assertNotIn(file, owners)
                    owners.add(file)
                if self.CALL.search(step) or 'build-headless.py' in step:
                    # target/cargo-messages is relative to the job's working directory, the pruned target's parent.
                    self.assertNotIn('working-directory:', step)
                listed = None
                for line in re.sub(r'\\\n\s*', ' ', step).splitlines():
                    if line.lstrip().startswith('#'):
                        continue
                    if not self.CALL.search(line):
                        if re.search(r'(^|[;&|]\s*)[A-Za-z_]\w*=|\b(export|unset)\s+[A-Za-z_]', line.strip()):
                            listed = None
                        continue
                    writes = '--message-format=json-render-diagnostics' in line and '> target/cargo-messages/' in line
                    with self.subTest(job=name, line=line.strip()):
                        self.assertTrue(writes or units(line) == listed)
                    if writes and '--no-run' in line:
                        listed = units(line)
                if 'build-headless.py' in step:
                    self.assertIn('CARGO_MESSAGES: ${{ github.workspace }}/rendering/bevy-explorer/source/target/cargo-messages/', step)

    def test_every_script_of_a_native_job_that_calls_cargo_keeps_its_units_named(self):
        source = ROOT / 'rendering/bevy-explorer/source'
        for name in ('test', 'headless-smoke', 'clippy', 'relay-wake'):
            job = self.job(name)
            paths = {ROOT / token[len('$GITHUB_WORKSPACE/'):] if token.startswith('$GITHUB_WORKSPACE/') else (source / token).resolve()
                     for token in re.findall(r'[\w$./-]+\.(?:py|sh|js|mjs|cjs)\b', job)}
            paths |= {ROOT / action / 'action.yml' for action in re.findall(r'uses: \./(\.github/actions/[\w-]+)', job)}
            for path in sorted(path for path in paths if path.is_file()):
                script = path.relative_to(ROOT).as_posix()
                if self.CALL.search(path.read_text(encoding='utf-8')):
                    with self.subTest(job=name, script=script):
                        self.assertIn(script, self.CARGO_SCRIPTS)


if __name__ == '__main__':
    unittest.main()
