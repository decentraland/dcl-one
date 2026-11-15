"""The path dependencies keep their cached build across CI checkouts, and only unchanged files count."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import unittest

SPEC = importlib.util.spec_from_file_location('bevy_mtimes', Path(__file__).parents[1] / 'bevy-mtimes.py')
mtimes = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(mtimes)

SECOND = 10**9
VENDOR = 'rendering/bevy-explorer/vendor'
BOIMP, DUI = VENDOR + '/boimp/src/lib.rs', VENDOR + '/bevy_dui/src/lib.rs'


class BevyMtimesTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.git('init', '-q')
        self.git('config', 'maintenance.auto', 'false')
        self.git('config', 'gc.auto', '0')
        self.write('.gitignore', 'target/\n/rendering/bevy-explorer/bevy/\n')
        self.write(BOIMP, 'pub fn import() {}\n')
        self.write(DUI, 'pub fn layout() {}\n')
        self.write('rendering/bevy-explorer/source/src/lib.rs', 'fn main() {}\n')
        self.write('library/vendor/rusty_enet/src/lib.rs', 'pub fn host() {}\n')
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
        """What a CI job does: the checkout writes every file and directory again, so every mtime is now."""
        now = time.time_ns()
        tracked = self.git('ls-files').splitlines()
        for name in {*tracked, *(parent.as_posix() for name in tracked for parent in Path(name).parents)}:
            self.set_mtime(name, now)
        return now

    def test_unchanged_files_get_their_recorded_time_back(self):
        self.set_mtime(BOIMP, 1_700_000_000 * SECOND + 750_000_000)
        mtimes.record(self.root, self.target)
        self.checkout_now()
        self.assertEqual(mtimes.restore(self.root, self.target), (4, 0))
        self.assertEqual(self.mtime(BOIMP), 1_700_000_000 * SECOND + 750_000_000)

    def test_changed_files_keep_their_checkout_time(self):
        # Path dependencies tracked as exact bytes: Cargo judges them by file times.
        self.set_mtime(DUI, 1_700_000_000 * SECOND)
        mtimes.record(self.root, self.target)
        self.write(BOIMP, 'pub fn import_changed() {}\n')
        self.commit()
        now = self.checkout_now()
        self.assertEqual(mtimes.restore(self.root, self.target), (3, 1))
        self.assertEqual(self.mtime(BOIMP), now)
        self.assertEqual(self.mtime(DUI), 1_700_000_000 * SECOND)

    def test_recorded_times_keep_their_nanoseconds(self):
        # Cargo stores the newest file time of a package to the nanosecond: a rounded time is a change to it.
        self.set_mtime(DUI, 1_700_000_000 * SECOND + 1)
        mtimes.record(self.root, self.target)
        manifest = json.loads((self.profiles[0] / '.fingerprint' / mtimes.MANIFEST).read_text())
        self.assertEqual(manifest['files'][DUI][1], 1_700_000_000 * SECOND + 1)

    def test_a_directory_keeps_its_cached_time_while_it_holds_the_same_names(self):
        # A build script may name a directory; Cargo then reads the time of every directory below it.
        proto = 'library/protocol/proto'
        kept, grown, shrunk = (f'{proto}/decentraland/{name}' for name in ('common', 'kernel', 'sdk'))
        for directory in (kept, grown, shrunk):
            self.write(directory + '/first.proto', 'message First {}\n')
        self.write(shrunk + '/second.proto', 'message Second {}\n')
        self.commit()
        for name in (proto, proto + '/decentraland', kept, grown, shrunk):
            self.set_mtime(name, 1_700_000_000 * SECOND + 7)
        mtimes.record(self.root, self.target)
        self.write(kept + '/first.proto', 'message Changed {}\n')
        self.write(grown + '/second.proto', 'message Second {}\n')
        self.git('rm', '-q', shrunk + '/second.proto')
        self.commit()
        now = self.checkout_now()
        for name in (grown, shrunk):
            self.set_mtime(name, now)
        mtimes.restore(self.root, self.target)
        for name in (proto, proto + '/decentraland', kept):
            self.assertEqual(self.mtime(name), 1_700_000_000 * SECOND + 7)
        for name in (grown, shrunk, kept + '/first.proto'):
            self.assertEqual(self.mtime(name), now)

    def test_directories_above_the_trees_are_left_alone(self):
        self.set_mtime('rendering/bevy-explorer', 1_700_000_000 * SECOND)
        mtimes.record(self.root, self.target)
        now = self.checkout_now()
        mtimes.restore(self.root, self.target)
        self.assertEqual(self.mtime('rendering/bevy-explorer'), now)

    def test_a_manifest_without_directories_restores_the_files(self):
        source = 'rendering/bevy-explorer/source/src/lib.rs'
        self.set_mtime(source, 1_700_000_000 * SECOND)
        mtimes.record(self.root, self.target)
        for profile in self.profiles:
            manifest = profile / '.fingerprint' / mtimes.MANIFEST
            recorded = json.loads(manifest.read_text())
            del recorded['directories']
            manifest.write_text(json.dumps(recorded))
        now = self.checkout_now()
        self.assertEqual(mtimes.restore(self.root, self.target), (4, 0))
        self.assertEqual(self.mtime(source), 1_700_000_000 * SECOND)
        self.assertEqual(self.mtime('rendering/bevy-explorer/source/src'), now)

    def test_every_profile_gets_the_manifest(self):
        self.assertEqual(mtimes.record(self.root, self.target), 2)
        for profile in self.profiles:
            self.assertTrue((profile / '.fingerprint' / mtimes.MANIFEST).is_file())

    def test_restore_without_a_manifest_changes_nothing(self):
        now = self.checkout_now()
        self.assertEqual(mtimes.restore(self.root, self.target), (0, 4))
        self.assertEqual(self.mtime(DUI), now)

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
        self.assertEqual(mtimes.restore(self.root, self.target), (3, 2))
        self.assertEqual(self.mtime(engine), now)
        self.assertEqual(self.mtime(proto), now)

    def materialise(self):
        """What `tooling/patched-sources.py materialise bevy` leaves for the Bevy crate tests: an ignored tree and its stamp."""
        (self.root / mtimes.TREE / 'crates/bevy_image/src').mkdir(parents=True)
        (self.root / mtimes.TREE / 'crates/bevy_image/src/lib.rs').write_bytes(b'pub fn decode() {}\r\n')
        self.write(mtimes.TREE + '/.patched-stamp.json', '{"tree": "1"}\n')
        return [mtimes.TREE + '/crates/bevy_image/src/lib.rs', mtimes.TREE + '/.patched-stamp.json']

    def rewrite(self, names):
        """A new materialise on a fresh checkout: the same bytes, written now."""
        now = self.checkout_now()
        for name in names:
            path = self.root / name
            path.write_bytes(path.read_bytes())
            self.set_mtime(name, now)
        return now

    def test_the_rebuilt_bevy_tree_is_matched_by_its_bytes(self):
        written = self.materialise()
        self.assertEqual(self.git('ls-files', mtimes.TREE), '')
        for name in written:
            self.set_mtime(name, 1_700_000_000 * SECOND + 5)
        mtimes.record(self.root, self.target)
        manifest = json.loads((self.profiles[0] / '.fingerprint' / mtimes.MANIFEST).read_text())
        source = written[0]
        self.assertEqual(manifest['files'][source], [self.git('hash-object', '--no-filters', source), 1_700_000_000 * SECOND + 5])
        self.rewrite(written)
        self.assertEqual(mtimes.restore(self.root, self.target), (6, 0))
        for name in written:
            self.assertEqual(self.mtime(name), 1_700_000_000 * SECOND + 5)

    def test_changed_rebuilt_files_keep_the_time_they_were_written(self):
        written = self.materialise()
        for name in written:
            self.set_mtime(name, 1_700_000_000 * SECOND)
        mtimes.record(self.root, self.target)
        now = self.rewrite(written)
        source, stamp = written
        (self.root / source).write_bytes(b'pub fn decode() {}\n')
        self.write(stamp, '{"tree": "2"}\n')
        self.write(mtimes.TREE + '/crates/bevy_image/src/new.rs', '')
        for name in (source, stamp, mtimes.TREE + '/crates/bevy_image/src/new.rs'):
            self.set_mtime(name, now)
        self.assertEqual(mtimes.restore(self.root, self.target), (4, 3))
        self.assertEqual(self.mtime(source), now)
        self.assertEqual(self.mtime(stamp), now)

    def test_unchanged_vendored_enet_sources_keep_their_cached_time(self):
        source = 'library/vendor/rusty_enet/src/lib.rs'
        self.set_mtime(source, 1_700_000_000 * SECOND)
        mtimes.record(self.root, self.target)
        self.checkout_now()
        mtimes.restore(self.root, self.target)
        self.assertEqual(self.mtime(source), 1_700_000_000 * SECOND)

    def test_a_tracked_link_is_no_input(self):
        # The id of a link is its text, and its time would be the one of the file it names: a changed
        # file behind an unchanged link must not get an old time back through the link.
        link = self.root / VENDOR / 'boimp/src/alias.rs'
        link.symlink_to('lib.rs')
        self.commit()
        self.assertNotIn(VENDOR + '/boimp/src/alias.rs', mtimes.blobs(self.root))
        self.set_mtime(BOIMP, 1_700_000_000 * SECOND)
        mtimes.record(self.root, self.target)
        manifest = json.loads((self.profiles[0] / '.fingerprint' / mtimes.MANIFEST).read_text())
        self.assertNotIn(VENDOR + '/boimp/src/alias.rs', manifest['files'])
        now = self.checkout_now()
        self.write(BOIMP, 'pub fn import() { /* changed */ }\n')
        self.set_mtime(BOIMP, now)
        self.git('add', '.')
        mtimes.restore(self.root, self.target)
        self.assertEqual(self.mtime(BOIMP), now)

    def test_a_path_of_the_index_that_is_not_on_disk_is_left_out(self):
        self.set_mtime(DUI, 1_700_000_000 * SECOND)
        (self.root / BOIMP).unlink()
        mtimes.record(self.root, self.target)
        manifest = json.loads((self.profiles[0] / '.fingerprint' / mtimes.MANIFEST).read_text())
        self.assertNotIn(BOIMP, manifest['files'])
        self.assertIn(DUI, manifest['files'])
        self.git('checkout', '-q', '--', BOIMP)
        mtimes.record(self.root, self.target)
        self.checkout_now()
        shutil.rmtree(self.root / VENDOR / 'boimp')
        restored, _ = mtimes.restore(self.root, self.target)
        self.assertEqual(restored, 3)
        self.assertEqual(self.mtime(DUI), 1_700_000_000 * SECOND)

    def test_a_directory_of_the_rebuilt_tree_keeps_its_cached_time(self):
        # The rebuilt tree is not tracked: its directories are named by the files on disk.
        self.write(mtimes.TREE + '/crates/bevy_ecs/src/lib.rs', 'pub struct World;\n')
        folder = mtimes.TREE + '/crates/bevy_ecs/src'
        self.set_mtime(folder, 1_700_000_000 * SECOND + 250_000_000)
        mtimes.record(self.root, self.target)
        now = time.time_ns()
        self.set_mtime(folder, now)
        mtimes.restore(self.root, self.target)
        self.assertEqual(self.mtime(folder), 1_700_000_000 * SECOND + 250_000_000)
        self.write(mtimes.TREE + '/crates/bevy_ecs/src/world.rs', 'pub fn spawn() {}\n')
        after = self.mtime(folder)
        mtimes.restore(self.root, self.target)
        self.assertEqual(self.mtime(folder), after)

    def test_the_manifest_survives_the_native_cache_pruning(self):
        # tooling/bevy-target-cache.py prunes the .fingerprint entries of workspace crates, <crate>-<hash>.
        spec = importlib.util.spec_from_file_location('bevy_target_cache', Path(__file__).parents[1] / 'bevy-target-cache.py')
        cache = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cache)
        if shutil.which('cargo') is None:
            self.skipTest('cargo reads the workspace members')
        names, _ = cache.workspace()
        self.assertFalse(cache.owned(mtimes.MANIFEST, names))


if __name__ == '__main__':
    unittest.main()
