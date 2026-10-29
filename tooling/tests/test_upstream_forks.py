"""The fork inventory reads the pinned lock exactly and pins every crate in the patch config."""
import importlib.util
from pathlib import Path
import textwrap
import unittest

spec = importlib.util.spec_from_file_location('upstream_forks', Path(__file__).resolve().parents[1] / 'upstream-forks.py')
forks = importlib.util.module_from_spec(spec)
spec.loader.exec_module(forks)

LOCK = textwrap.dedent('''
    [[package]]
    name = "serde"
    version = "1.0.0"
    source = "registry+https://github.com/rust-lang/crates.io-index"

    [[package]]
    name = "bevy_ecs"
    version = "0.16.1"
    source = "git+https://github.com/robtfm/bevy?branch=release-0.16-dcl#4a9e4a68"

    [[package]]
    name = "bevy_app"
    version = "0.16.1"
    source = "git+https://github.com/robtfm/bevy?branch=release-0.16-dcl#4a9e4a68"

    [[package]]
    name = "draco-oxide-core"
    version = "0.2.0"
    source = "git+https://github.com/robtfm/draco-oxide?branch=fix%2Fwasm32-rans-refill#5acfcf6"

    [[package]]
    name = "gltf"
    version = "1.4.1"
    source = "git+https://github.com/gltf-rs/gltf?rev=12fc1b7c#12fc1b7c"

    [[package]]
    name = "local"
    version = "0.1.0"
''')

MANIFEST = textwrap.dedent('''
    [patch.crates-io]
    # rANS refill never refills on 32-bit targets.
    # fix PR'd upstream; drop when released.
    draco-oxide-core = { git = "https://github.com/robtfm/draco-oxide", branch = "fix/wasm32-rans-refill" }
    bevy_ecs = { git = "https://github.com/robtfm/bevy", branch = "release-0.16-dcl" }
''')


class ForkInventoryTests(unittest.TestCase):
    def test_git_sources_are_grouped_by_url_reference_and_revision(self):
        sources = forks.parse_git_sources(LOCK)
        self.assertEqual(sources[('https://github.com/robtfm/bevy', 'branch=release-0.16-dcl', '4a9e4a68')],
                         [('bevy_app', '0.16.1'), ('bevy_ecs', '0.16.1')])
        self.assertIn(('https://github.com/robtfm/draco-oxide', 'branch=fix/wasm32-rans-refill', '5acfcf6'), sources)
        self.assertEqual(sources[('https://github.com/gltf-rs/gltf', 'rev=12fc1b7c', '12fc1b7c')], [('gltf', '1.4.1')])
        self.assertEqual(len(sources), 3)

    def test_reasons_come_from_the_comment_block_above_the_first_mention(self):
        reasons = forks.harvest_reasons(MANIFEST, {'https://github.com/robtfm/draco-oxide', 'https://github.com/robtfm/bevy'})
        self.assertEqual(reasons['https://github.com/robtfm/draco-oxide'],
                         "rANS refill never refills on 32-bit targets. fix PR'd upstream; drop when released.")
        self.assertNotIn('https://github.com/robtfm/bevy', reasons)
        # a url that is a prefix of another url takes only its own line's comment
        text = '# console reason\nbevy_console = { git = "https://github.com/robtfm/bevy-console" }\nbevy = { git = "https://github.com/robtfm/bevy" }\n'
        self.assertEqual(forks.harvest_reasons(text, {'https://github.com/robtfm/bevy', 'https://github.com/robtfm/bevy-console'}), {'https://github.com/robtfm/bevy-console': 'console reason'})

    def test_patch_config_pins_crates_io_patches_and_templates_direct_git_deps(self):
        import tomllib
        patched = forks.manifest_crates_io_patches(MANIFEST)
        self.assertEqual(patched, {'draco-oxide-core': 'https://github.com/robtfm/draco-oxide', 'bevy_ecs': 'https://github.com/robtfm/bevy'})
        text = forks.render_patches(forks.parse_git_sources(LOCK), patched)
        config = tomllib.loads(text)['patch']['crates-io']
        # only the crates the manifest patches, pinned to the lock's revision of their source
        self.assertEqual(config, {'bevy_ecs': {'git': 'https://github.com/robtfm/bevy', 'rev': '4a9e4a68'},
                                  'draco-oxide-core': {'git': 'https://github.com/robtfm/draco-oxide', 'rev': '5acfcf6'}})
        # a source the manifests name directly can only be patched onto another repository
        self.assertIn('# [patch."https://github.com/gltf-rs/gltf"]', text)
        self.assertIn('# gltf = { git = "MIRROR_URL", rev = "12fc1b7c" }', text)

    def test_upstream_check_reports_parent_distance_and_branch_movement(self):
        calls = []
        def api(path):
            calls.append(path)
            if path == 'repos/robtfm/bevy':
                return {'fork': True, 'default_branch': 'main', 'pushed_at': '2026-10-16T00:00:00Z',
                        'parent': {'full_name': 'bevyengine/bevy', 'default_branch': 'main'}}
            if path.endswith('bevyengine:main...4a9e4a68'):
                return {'ahead_by': 298, 'behind_by': 3872}
            if path.endswith('4a9e4a68...release-0.16-dcl'):
                return {'ahead_by': 2}
            raise AssertionError(path)
        out = forks.check_upstream('https://github.com/robtfm/bevy', 'branch=release-0.16-dcl', '4a9e4a68', api=api)
        self.assertEqual(out, {'default_branch': 'main', 'pushed_at': '2026-10-16', 'parent': 'bevyengine/bevy',
                               'ahead_of_parent': 298, 'behind_parent': 3872, 'branch_commits_since_pin': 2})
        self.assertEqual(forks.check_upstream('https://example.org/x.git', 'branch=b', 'r', api=api), {})

    def test_forks_toml_round_trips(self):
        import tomllib
        sources = forks.parse_git_sources(LOCK)
        text = forks.render_forks(sources, {'https://github.com/robtfm/bevy': 'why'}, {}, '2026-10-18')
        data = tomllib.loads(text)
        self.assertEqual({s['url'] for s in data['source']}, {u for u, _, _ in sources})
        bevy = next(s for s in data['source'] if s['url'].endswith('/bevy'))
        self.assertEqual(bevy['reason'], 'why')
        self.assertEqual(bevy['crates'], ['bevy_app 0.16.1', 'bevy_ecs 0.16.1'])


if __name__ == '__main__':
    unittest.main()


class SplitDetectionTests(unittest.TestCase):
    def test_a_repository_resolved_under_two_forms_is_a_split(self):
        metadata = {'packages': [
            {'name': 'bevy_app', 'source': 'git+https://github.com/robtfm/bevy?branch=release-0.16-dcl#4a9e'},
            {'name': 'bevy_app', 'source': 'git+https://github.com/robtfm/bevy?rev=4a9e#4a9e'},
            {'name': 'deno_core', 'source': 'git+https://github.com/robtfm/deno_core?rev=9b59#9b59'},
            {'name': 'serde', 'source': 'registry+https://github.com/rust-lang/crates.io-index'},
            {'name': 'local', 'source': None},
        ]}
        self.assertEqual(forks.splitting_sources(metadata), {'https://github.com/robtfm/bevy'})

    def test_a_splitting_source_is_left_to_the_lock_and_offered_as_a_mirror(self):
        import tomllib
        sources = forks.parse_git_sources(LOCK)
        patched = forks.manifest_crates_io_patches(MANIFEST)
        text = forks.render_patches(sources, patched, splitting={'https://github.com/robtfm/bevy'})
        config = tomllib.loads(text)['patch']['crates-io']
        self.assertEqual(set(config), {'draco-oxide-core'})
        self.assertIn('https://github.com/robtfm/bevy (branch release-0.16-dcl): NOT pinned here', text)
        self.assertIn('# [patch."https://github.com/robtfm/bevy"]  # branch release-0.16-dcl', text)
        self.assertIn('# bevy_ecs = { git = "MIRROR_URL", rev = "4a9e4a68" }', text)


class RevisionDriftTests(unittest.TestCase):
    def test_drift_names_sources_resolved_to_another_revision(self):
        sources = forks.parse_git_sources(LOCK)
        metadata = {'packages': [
            {'name': 'bevy_ecs', 'source': 'git+https://github.com/robtfm/bevy?rev=4a9e4a68#4a9e4a68'},
            {'name': 'draco-oxide-core', 'source': 'git+https://github.com/robtfm/draco-oxide?rev=other#other'},
            {'name': 'serde', 'source': 'registry+https://github.com/rust-lang/crates.io-index'},
        ]}
        self.assertEqual(forks.revision_drift(metadata, sources), {'https://github.com/robtfm/draco-oxide': ('5acfcf6', 'other')})
