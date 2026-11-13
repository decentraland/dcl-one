"""The vendored Bevy keeps its cached build across CI checkouts, and only unchanged files count."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest

SPEC = importlib.util.spec_from_file_location('bevy_mtimes', Path(__file__).parents[1] / 'bevy-mtimes.py')
mtimes = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(mtimes)

SECOND = 10**9


class BevyMtimesTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.git('init', '-q')
        self.git('config', 'maintenance.auto', 'false')
        self.git('config', 'gc.auto', '0')
        self.write('.gitignore', 'target/\n')
        self.write(mtimes.TREE + '/crates/bevy_scene/src/lib.rs', 'pub fn spawn() {}\n')
        self.write(mtimes.TREE + '/crates/bevy_ecs/src/lib.rs', 'pub fn world() {}\n')
        self.write('rendering/bevy-explorer/source/src/lib.rs', 'fn main() {}\n')
        self.commit()
        self.target = self.root / 'rendering/bevy-explorer/source/target'
        self.profiles = [self.target / 'ci-test', self.target / 'x86_64-unknown-linux-gnu' / 'release']
        for profile in self.profiles:
            (profile / '.fingerprint').mkdir(parents=True)

    def git(self, *args):
        return subprocess.check_output(['git', '-c', 'user.name=CI', '-c', 'user.email=ci@example.invalid', *args],
                                       cwd=self.root, text=True).strip()

    def write(self, name, value):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(value)
        return path

    def commit(self):
        self.git('add', '.')
        self.git('commit', '-qm', 'fixture')

    def set_mtime(self, name, ns):
        os.utime(self.root / name, ns=(ns, ns))

    def mtime(self, name):
        return (self.root / name).stat().st_mtime_ns

    def checkout_now(self):
        """What actions/checkout does: every file is written again, so every mtime is now."""
        now = time.time_ns()
        for name in self.git('ls-files').splitlines():
            self.set_mtime(name, now)
        return now

    def test_unchanged_files_get_their_recorded_time_back(self):
        scene = mtimes.TREE + '/crates/bevy_scene/src/lib.rs'
        self.set_mtime(scene, 1_700_000_000 * SECOND + 750_000_000)
        mtimes.record(self.root, self.target)
        self.checkout_now()
        self.assertEqual(mtimes.restore(self.root, self.target), (3, 0))
        self.assertEqual(self.mtime(scene), 1_700_000_000 * SECOND)

    def test_changed_files_keep_their_checkout_time(self):
        scene, ecs = mtimes.TREE + '/crates/bevy_scene/src/lib.rs', mtimes.TREE + '/crates/bevy_ecs/src/lib.rs'
        self.set_mtime(ecs, 1_700_000_000 * SECOND)
        mtimes.record(self.root, self.target)
        self.write(scene, 'pub fn spawn_sliced() {}\n')
        self.commit()
        now = self.checkout_now()
        self.assertEqual(mtimes.restore(self.root, self.target), (2, 1))
        self.assertEqual(self.mtime(scene), now)
        self.assertEqual(self.mtime(ecs), 1_700_000_000 * SECOND)

    def test_recorded_times_are_whole_seconds(self):
        # A cache archive may keep whole seconds only; a source a fraction newer than its
        # truncated build stamp would make Cargo rebuild it.
        self.set_mtime(mtimes.TREE + '/crates/bevy_ecs/src/lib.rs', 1_700_000_000 * SECOND + 1)
        mtimes.record(self.root, self.target)
        manifest = json.loads((self.profiles[0] / '.fingerprint' / mtimes.MANIFEST).read_text())
        self.assertTrue(all(ns % SECOND == 0 for _, ns in manifest['files'].values()))

    def test_every_profile_gets_the_manifest(self):
        self.assertEqual(mtimes.record(self.root, self.target), 2)
        for profile in self.profiles:
            self.assertTrue((profile / '.fingerprint' / mtimes.MANIFEST).is_file())

    def test_restore_without_a_manifest_changes_nothing(self):
        now = self.checkout_now()
        self.assertEqual(mtimes.restore(self.root, self.target), (0, 3))
        self.assertEqual(self.mtime(mtimes.TREE + '/crates/bevy_ecs/src/lib.rs'), now)

    def test_unchanged_engine_sources_keep_their_cached_time(self):
        source = 'rendering/bevy-explorer/source/src/lib.rs'
        self.set_mtime(source, 1_700_000_000 * SECOND)
        mtimes.record(self.root, self.target)
        now = self.checkout_now()
        mtimes.restore(self.root, self.target)
        self.assertEqual(self.mtime(source), 1_700_000_000 * SECOND)

    def test_changed_engine_and_protocol_sources_keep_the_checkout_time(self):
        engine = 'rendering/bevy-explorer/source/src/lib.rs'
        proto = 'library/protocol/proto/example.proto'
        self.write(proto, 'message Example {}\n')
        self.commit()
        mtimes.record(self.root, self.target)
        self.write(engine, 'fn changed() {}\n')
        self.write(proto, 'message Changed {}\n')
        self.commit()
        now = self.checkout_now()
        self.assertEqual(mtimes.restore(self.root, self.target), (2, 2))
        self.assertEqual(self.mtime(engine), now)
        self.assertEqual(self.mtime(proto), now)

    def test_the_manifest_survives_rust_cache_pruning(self):
        # rust-cache keeps a .fingerprint entry only if its name, up to the last '-', is a
        # dependency's package name; `bevy` is one.
        self.assertEqual(mtimes.MANIFEST.rsplit('-', 1)[0], 'bevy')


if __name__ == '__main__':
    unittest.main()
