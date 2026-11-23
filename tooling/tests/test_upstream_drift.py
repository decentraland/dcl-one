"""The pinned source list, the upstream check, the drift collector's parsing and version logic,
and the site's markdown rendering. Nothing here reaches the network."""
import base64
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import textwrap
import unittest
from unittest import mock
from unittest.mock import patch

TOOLING = Path(__file__).resolve().parents[1]
WORKFLOW = TOOLING.parent / '.github/workflows/bevy-explorer-upstream.yml'
spec = importlib.util.spec_from_file_location('upstream_drift', TOOLING / 'upstream-drift.py')
drift = importlib.util.module_from_spec(spec)
spec.loader.exec_module(drift)
spec2 = importlib.util.spec_from_file_location('upstream_drift_site', TOOLING / 'upstream-drift-site.py')
site = importlib.util.module_from_spec(spec2)
spec2.loader.exec_module(site)
spec3 = importlib.util.spec_from_file_location('patched_sources', TOOLING / 'patched-sources.py')
patched = importlib.util.module_from_spec(spec3)
spec3.loader.exec_module(patched)


SOURCE_TOML = textwrap.dedent('''
    name       = "winit"
    upstream   = "https://github.com/rust-windowing/winit"
    commit     = "1111111111111111111111111111111111111111"
    ref        = "v0.30.12"
    origin     = "https://github.com/example/winit"
    origin_rev = "2222222222222222222222222222222222222222"
    license    = "Apache-2.0"
    paths      = ["."]
    exclude    = []
    base_tree  = "3333333333333333333333333333333333333333"
    tree       = "4444444444444444444444444444444444444444"
    nix_hash   = ""
''')

# What `tooling/patched-sources.py export` writes: no upstream commit, so the all-zero one.
LOCAL_PATCH = textwrap.dedent('''\
    From 0000000000000000000000000000000000000000
    Subject: resolve the merge by hand
    Date: 2026-03-05T10:00:00+01:00

    ---
    diff --git a/src/lib.rs b/src/lib.rs
''')

PATCH = textwrap.dedent('''\
    From 5555555555555555555555555555555555555555
    Subject: keep the cursor grab on focus loss
    Origin: https://github.com/example/winit/commit/5555555555555555555555555555555555555555
    Date: 2026-03-04T10:00:00+01:00

    The body may name a line that looks like a header.
    Subject: not this one
    ---
    diff --git a/src/lib.rs b/src/lib.rs
''')

LOCK = textwrap.dedent('''
    [[package]]
    name = "serde"
    version = "1.0.0"
    source = "registry+https://github.com/rust-lang/crates.io-index"

    [[package]]
    name = "gltf-json"
    version = "1.4.1"
    source = "git+https://github.com/gltf-rs/gltf?rev=12fc1b7c#12fc1b7ce28c7b4c6d1b881a30df24060dccfbda"

    [[package]]
    name = "gltf"
    version = "1.4.1"
    source = "git+https://github.com/gltf-rs/gltf?rev=12fc1b7c#12fc1b7ce28c7b4c6d1b881a30df24060dccfbda"

    [[package]]
    name = "draco-oxide-core"
    version = "0.2.0"
    source = "git+https://github.com/example/draco-oxide?branch=fix%2Fwasm32-rans-refill#5acfcf6"

    [[package]]
    name = "local"
    version = "0.1.0"
''')

MANIFEST = textwrap.dedent('''
    [dependencies]
    # Pin the merge commit
    # until a release ships it.
    gltf = { git = "https://github.com/gltf-rs/gltf", rev = "12fc1b7c" }
    gltf-extra = { git = "https://github.com/gltf-rs/gltf-extra" }
''')

GLTF = 'https://github.com/gltf-rs/gltf'


def pin_tree(root):
    """A pinned Explorer in the contract's layout: one patched source and a lock with git sources."""
    series = root / 'patches/winit/series'
    series.mkdir(parents=True)
    (root / 'patches/winit/source.toml').write_text(SOURCE_TOML)
    (series / '0001-keep-the-cursor-grab.patch').write_text(PATCH)
    (series / '0002-merge-resolution.patch').write_text(LOCAL_PATCH)
    (series / '0003-no-header.patch').write_text('---\ndiff --git a/x b/x\n')
    (root / 'source').mkdir()
    (root / 'source/Cargo.lock').write_text(LOCK)
    (root / 'source/Cargo.toml').write_text(MANIFEST)
    return root


class PinnedSourceTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.pin = pin_tree(Path(directory.name))

    def test_a_patched_source_is_its_upstream_commit_ref_and_series(self):
        winit, = drift.patched_sources(self.pin / 'patches')
        self.assertEqual({k: v for k, v in winit.items() if k != 'patches'},
                         {'name': 'winit', 'upstream': 'https://github.com/rust-windowing/winit',
                          'commit': '1' * 40, 'ref': 'v0.30.12'})
        self.assertEqual(winit['patches'], [
            {'sha': '5' * 40, 'local': False, 'url': 'https://github.com/example/winit/commit/' + '5' * 40,
             'file': '0001-keep-the-cursor-grab.patch', 'date': '2026-03-04', 'title': 'keep the cursor grab on focus loss',
             'author': 'example/winit'},
            {'sha': '', 'local': True, 'url': '', 'file': '0002-merge-resolution.patch', 'date': '2026-03-05',
             'title': 'resolve the merge by hand', 'author': ''},
            {'sha': '', 'local': True, 'url': '', 'file': '0003-no-header.patch', 'date': '', 'title': '0003-no-header',
             'author': ''}])

    def test_every_text_file_is_read_and_written_as_utf_8(self):
        # The default encoding is the locale's, and a patch subject or a commit title is not ASCII.
        for tool in ('upstream-drift.py', 'upstream-drift-site.py'):
            text = (TOOLING / tool).read_text(encoding='utf-8')
            self.assertEqual(re.findall(r'\b(?:read|write)_text\((?![^\n]*encoding=)[^\n]*', text), [], tool)

    def test_lock_git_sources_are_grouped_and_name_the_ref_to_watch(self):
        draco, gltf = drift.lock_git_sources(LOCK)
        self.assertEqual(draco, {'name': 'draco-oxide', 'upstream': 'https://github.com/example/draco-oxide', 'commit': '5acfcf6',
                                 'ref': 'fix/wasm32-rans-refill', 'crates': [('draco-oxide-core', '0.2.0')]})
        self.assertEqual(gltf, {'name': 'gltf', 'upstream': GLTF, 'commit': '12fc1b7ce28c7b4c6d1b881a30df24060dccfbda',
                                'ref': None, 'crates': [('gltf', '1.4.1'), ('gltf-json', '1.4.1')]})

    def test_reasons_come_from_the_comment_block_above_the_first_mention(self):
        reasons = drift.harvest_reasons(MANIFEST, {GLTF, GLTF + '-extra'})
        self.assertEqual(reasons, {GLTF: 'Pin the merge commit until a release ships it.'})

    def test_pinned_sources_are_the_patched_ones_then_the_lock_with_reasons(self):
        sources = drift.pinned_sources(self.pin)
        self.assertEqual([s['name'] for s in sources], ['winit', 'draco-oxide', 'gltf'])
        self.assertEqual(sources[2]['reason'], 'Pin the merge commit until a release ships it.')
        self.assertIsNone(sources[1]['reason'])

    def test_a_patched_source_the_lock_takes_from_this_repository_is_listed_once(self):
        # Cargo takes a patched source from this repository by revision. That lock entry has no
        # upstream of its own: the source.toml names the upstream to watch.
        for url in (drift.REPOSITORY, drift.REPOSITORY + '.git', drift.REPOSITORY + '/',
                    drift.REPOSITORY.replace('/decentraland/', '/Decentraland/')):
            lock = LOCK + textwrap.dedent(f'''
                [[package]]
                name = "winit"
                version = "0.30.12"
                source = "git+{url}?rev=6666666666666666666666666666666666666666#6666666666666666666666666666666666666666"
            ''')
            (self.pin / 'source/Cargo.lock').write_text(lock, encoding='utf-8')
            self.assertIn(url, [s['upstream'] for s in drift.lock_git_sources(lock)])
            sources = drift.pinned_sources(self.pin)
            self.assertEqual([s['name'] for s in sources], ['winit', 'draco-oxide', 'gltf'])
            self.assertEqual(sources[0]['upstream'], 'https://github.com/rust-windowing/winit')

    def test_this_repository_is_the_one_that_the_patch_tool_names(self):
        self.assertEqual(drift.REPOSITORY, patched.MIRROR)
        # The two tools read the same URLs as this repository.
        same = (drift.REPOSITORY, drift.REPOSITORY + '.git', drift.REPOSITORY + '/', drift.REPOSITORY.upper() + '.git/')
        for url in (*same, drift.REPOSITORY + '-fork', GLTF + '.git'):
            self.assertEqual(drift.canonical(url), patched.canonical(url), url)
            self.assertEqual(drift.canonical(url) == drift.canonical(drift.REPOSITORY), url in same, url)

    def test_the_tracked_sources_follow_the_contract(self):
        sources = drift.patched_sources(drift.PIN / 'patches')
        self.assertIn('bevy', [s['name'] for s in sources])
        for source in sources:
            self.assertRegex(source['commit'], r'^[0-9a-f]{40}$')
            self.assertIsNotNone(drift.github_repo(source['upstream']), source['upstream'])
            self.assertNotIn('crates', source)
            # The check proves a pin against the ref its source names: a source with none proves nothing.
            self.assertTrue(source['ref'], source['name'])
            for patch in source['patches']:
                self.assertTrue(patch['title'], patch['file'])
                # A patch is a commit of a public repository, with its address, or a local patch.
                self.assertEqual(patch['local'], not patch['sha'], patch['file'])
                self.assertEqual(bool(patch['url']), not patch['local'], patch['file'])
                self.assertNotRegex(patch['sha'], r'^0+$', patch['file'])


def fake_api(answers):
    def api(path):
        if path not in answers:
            raise AssertionError(path)
        return answers[path]
    return api


def tag_rows(*names):
    return [{'ref': f'refs/tags/{name}'} for name in names]


def ref_rows(space, name, commit, kind='commit'):
    return [{'ref': f'refs/{space}/{name}', 'object': {'sha': commit, 'type': kind}}]


HEAD = 'f' * 40
WINIT = 'repos/rust-windowing/winit'
WINIT_TAGS = WINIT + '/git/matching-refs/tags/v'
WINIT_TAG = WINIT + '/git/matching-refs/tags/v0.30.12'
WINIT_COMPARE = WINIT + '/compare/' + 'a' * 40 + '...' + HEAD
DRACO = 'repos/example/draco-oxide'


class CheckTests(unittest.TestCase):
    def test_a_tag_that_stayed_reports_the_newer_tags_of_its_scheme(self):
        source = {'name': 'winit', 'upstream': 'https://github.com/rust-windowing/winit', 'commit': 'a' * 40, 'ref': 'v0.30.12'}
        answers = {WINIT_COMPARE: {'ahead_by': 0, 'behind_by': 0}, WINIT_TAG: ref_rows('tags', 'v0.30.12', HEAD),
                   WINIT_TAGS: tag_rows('v0.29.0', 'v0.30.12', 'v0.30.13', 'v0.31.0-beta.2', 'v0.31.0', 'v0.30.9', 'vello-1.0')}
        result = drift.check_source(source, fake_api(answers))
        self.assertEqual(result, {'ref': 'v0.30.12', 'ahead': 0, 'behind': 0, 'named': True, 'tag': True,
                                  'newer_tags': ['v0.30.13', 'v0.31.0']})
        self.assertIsNone(drift.off_ref(result))
        answers[WINIT_TAGS] = tag_rows('v0.29.0', 'v0.30.12')
        self.assertEqual(drift.check_source(source, fake_api(answers))['newer_tags'], [])

    def test_a_pre_release_pin_counts_pre_releases_and_a_release_pin_does_not(self):
        tags = ['v0.1.0-alpha.9', 'v0.1.0-alpha.10', 'v0.1.0-rc.1', 'v0.1.0', 'v0.2.0-alpha.1']
        self.assertEqual(drift.newer_tags(tags, 'v0.1.0-alpha.9'), ['v0.1.0-alpha.10', 'v0.1.0-rc.1', 'v0.1.0', 'v0.2.0-alpha.1'])
        self.assertEqual(drift.newer_tags(tags, 'v0.1.0'), [])
        self.assertEqual(drift.newer_tags(['0.18', '0.18.1', '0.19.0-dev', '0.17.3'], '0.18'), ['0.18.1'])

    def test_tags_of_another_scheme_are_not_the_scheme_of_the_ref(self):
        answers = {'repos/livekit/rust-sdks/git/matching-refs/tags/livekit/v':
                   tag_rows('livekit/v0.7.1', 'livekit/v0.7.2', 'livekit/v-odd/v1.0.0')}
        self.assertEqual(drift.scheme_tags('livekit/rust-sdks', 'livekit/v0.7.1', fake_api(answers)), ['livekit/v0.7.1', 'livekit/v0.7.2'])

    def test_a_tag_that_no_longer_names_the_pin_is_off_its_ref(self):
        source = {'name': 'winit', 'upstream': 'https://github.com/rust-windowing/winit', 'commit': 'a' * 40, 'ref': 'v0.30.12'}
        answers = {WINIT_COMPARE: {'ahead_by': 2, 'behind_by': 0}, WINIT_TAG: ref_rows('tags', 'v0.30.12', HEAD),
                   WINIT_TAGS: tag_rows('v0.30.12')}
        self.assertEqual(drift.off_ref(drift.check_source(source, fake_api(answers))),
                         'the tag moved, or never named the pin: it is 2 commit(s) past it')

    def test_a_branch_that_moved_counts_its_commits(self):
        source = {'name': 'draco-oxide', 'upstream': 'https://github.com/example/draco-oxide', 'commit': 'b' * 40, 'ref': 'fix/rans'}
        answers = {DRACO + '/compare/' + 'b' * 40 + '...' + HEAD: {'ahead_by': 4, 'behind_by': 0},
                   DRACO + '/git/matching-refs/tags/fix/rans': [],
                   DRACO + '/git/matching-refs/heads/fix/rans': [*ref_rows('heads', 'fix/rans', HEAD), *ref_rows('heads', 'fix/rans-2', 'e' * 40)]}
        result = drift.check_source(source, fake_api(answers))
        self.assertEqual(result, {'ref': 'fix/rans', 'ahead': 4, 'behind': 0, 'named': True, 'tag': False, 'newer_tags': []})
        self.assertIsNone(drift.off_ref(result))

    def test_a_pin_the_named_branch_lacks_is_off_its_ref(self):
        source = {'name': 'draco-oxide', 'upstream': 'https://github.com/example/draco-oxide', 'commit': 'b' * 40, 'ref': 'main'}
        answers = {DRACO + '/compare/' + 'b' * 40 + '...' + HEAD: {'ahead_by': 4, 'behind_by': 1},
                   DRACO + '/git/matching-refs/tags/main': ref_rows('tags', 'maintenance-1', 'e' * 40),
                   DRACO + '/git/matching-refs/heads/main': ref_rows('heads', 'main', HEAD)}
        self.assertEqual(drift.off_ref(drift.check_source(source, fake_api(answers))), 'the pin has 1 commit(s) main lacks')

    def test_a_source_pinned_by_revision_alone_watches_the_default_branch_and_claims_nothing(self):
        source = {'name': 'gltf', 'upstream': GLTF, 'commit': 'c' * 40, 'ref': None}
        answers = {'repos/gltf-rs/gltf': {'default_branch': 'main'},
                   'repos/gltf-rs/gltf/compare/' + 'c' * 40 + '...main': {'ahead_by': 26, 'behind_by': 2}}
        result = drift.check_source(source, fake_api(answers))
        self.assertEqual(result, {'ref': 'main', 'ahead': 26, 'behind': 2, 'named': False, 'tag': False, 'newer_tags': []})
        self.assertIsNone(drift.off_ref(result))

    def test_a_source_github_cannot_compare_is_not_checked(self):
        gone = {'name': 'gone', 'upstream': 'https://github.com/example/gone', 'commit': 'd' * 40, 'ref': 'v1'}
        answers = {'repos/example/gone/git/matching-refs/tags/v1': ref_rows('tags', 'v1', HEAD),
                   'repos/example/gone/compare/' + 'd' * 40 + '...' + HEAD: None}
        self.assertIsNone(drift.check_source(gone, fake_api(answers)))
        elsewhere = {'name': 'x', 'upstream': 'https://example.org/x.git', 'commit': 'e' * 40, 'ref': 'main'}
        self.assertIsNone(drift.check_source(elsewhere, fake_api({})))

    def test_a_ref_that_is_no_tag_or_branch_of_the_repository_is_not_checked(self):
        # GitHub compare resolves each of these names, so no compare may be asked for.
        for ref in ('pull/123/head', 'refs/pull/123/head', 'c' * 40, 'v0.30'):
            with self.subTest(ref=ref):
                source = {'name': 'winit', 'upstream': 'https://github.com/rust-windowing/winit', 'commit': 'a' * 40, 'ref': ref}
                answers = {f'{WINIT}/git/matching-refs/tags/{ref}': ref_rows('tags', 'v0.30.12', HEAD) if ref == 'v0.30' else [],
                           f'{WINIT}/git/matching-refs/heads/{ref}': []}
                self.assertIsNone(drift.check_source(source, fake_api(answers)))
                text, ok = drift.check_report([source], fake_api(answers))
                self.assertFalse(ok)
                self.assertIn(f'NOT CHECKED: GitHub cannot compare the tag or branch {ref} with aaaaaaaaaa', text)

    def test_an_annotated_tag_is_compared_by_the_commit_it_names(self):
        source = {'name': 'winit', 'upstream': 'https://github.com/rust-windowing/winit', 'commit': 'a' * 40, 'ref': 'v0.30.12'}
        answers = {WINIT_TAG: ref_rows('tags', 'v0.30.12', 'e' * 40, 'tag'),
                   WINIT + '/git/tags/' + 'e' * 40: {'object': {'sha': HEAD, 'type': 'commit'}},
                   WINIT_COMPARE: {'ahead_by': 0, 'behind_by': 0}, WINIT_TAGS: tag_rows('v0.30.12')}
        self.assertEqual(drift.check_source(source, fake_api(answers)),
                         {'ref': 'v0.30.12', 'ahead': 0, 'behind': 0, 'named': True, 'tag': True, 'newer_tags': []})

    def report(self, behind=0):
        sources = [
            {'name': 'winit', 'upstream': 'https://github.com/rust-windowing/winit', 'commit': 'a' * 40, 'ref': 'v0.30.12',
             'patches': [{'local': False}, {'local': True}]},
            {'name': 'gltf', 'upstream': GLTF, 'commit': 'c' * 40, 'ref': None},
        ]
        answers = {WINIT_COMPARE: {'ahead_by': 0, 'behind_by': behind}, WINIT_TAG: ref_rows('tags', 'v0.30.12', HEAD),
                   WINIT_TAGS: tag_rows('v0.30.12', 'v0.30.13', 'v0.31.0'),
                   'repos/gltf-rs/gltf': {'default_branch': 'main'},
                   'repos/gltf-rs/gltf/compare/' + 'c' * 40 + '...main': {'ahead_by': 26, 'behind_by': 2}}
        return sources, answers

    def test_the_report_has_one_line_per_source_with_local_patches_and_tag_drift(self):
        sources, answers = self.report()
        text, ok = drift.check_report(sources, fake_api(answers))
        self.assertTrue(ok)
        self.assertEqual(text.splitlines(), [
            'Pinned sources of the Explorer: 2; upstream moved past the pin: 1; a newer tag: 1; pin not on its ref: 0; not checked: 0.',
            '',
            '- winit: rust-windowing/winit tag v0.30.12 has not moved past aaaaaaaaaa; 2 patch(es), 1 local; 2 newer tag(s), newest v0.31.0',
            '- gltf: gltf-rs/gltf main is 26 commit(s) past cccccccccc (the pin has 2 commit(s) the ref lacks)'])

    def test_the_report_fails_on_a_pin_that_is_not_on_its_ref(self):
        sources, answers = self.report(behind=3)
        text, ok = drift.check_report(sources, fake_api(answers))
        self.assertFalse(ok)
        self.assertIn('pin not on its ref: 1; not checked: 0.', text)
        self.assertIn('- winit: rust-windowing/winit tag v0.30.12 has not moved past aaaaaaaaaa; '
                      'PIN NOT ON ITS REF: the pin has 3 commit(s) v0.30.12 lacks; 2 patch(es), 1 local', text)

    def test_the_report_fails_on_an_unchecked_source(self):
        sources, answers = self.report()
        sources.append({'name': 'x', 'upstream': 'https://example.org/x.git', 'commit': 'e' * 40, 'ref': 'main'})
        text, ok = drift.check_report(sources, fake_api(answers))
        self.assertFalse(ok)
        self.assertIn('not checked: 1.', text)
        self.assertEqual(text.splitlines()[-1], '- x: https://example.org/x.git NOT CHECKED: not a GitHub repository')

    def test_the_command_checks_the_named_sources_and_rejects_an_unknown_name(self):
        sources = [{'name': 'winit'}, {'name': 'gltf'}]
        seen = []
        with mock.patch.object(drift, 'pinned_sources', return_value=sources), \
                mock.patch.object(drift, 'check_report', side_effect=lambda chosen: (seen.append(chosen) or ('', False))), \
                mock.patch.object(drift.sys, 'argv', ['upstream-drift.py', 'check', 'gltf']):
            self.assertEqual(drift.main(), 1)
        self.assertEqual(seen, [[{'name': 'gltf'}]])
        with mock.patch.object(drift, 'pinned_sources', return_value=sources), mock.patch.object(drift.sys, 'stderr'), \
                mock.patch.object(drift.sys, 'argv', ['upstream-drift.py', 'check', 'winnit']), self.assertRaises(SystemExit) as stop:
            drift.main()
        self.assertEqual(stop.exception.code, 2)


class CollectTests(unittest.TestCase):
    def test_a_source_entry_has_the_shape_the_site_reads(self):
        commit = {'sha': 'f' * 40, 'commit': {'committer': {'date': '2026-05-01T00:00:00Z'}, 'author': {'name': 'someone'}, 'message': 'a change\n\nbody'}}
        answers = {
            'repos/rust-windowing/winit': {'description': 'windows', 'stargazers_count': 5, 'archived': False, 'default_branch': 'master'},
            'repos/rust-windowing/winit/commits/' + 'a' * 40: {'commit': {'committer': {'date': '2026-01-02T00:00:00Z'}}},
            'repos/rust-windowing/winit/compare/' + 'a' * 40 + '...master?per_page=250&page=1':
                {'status': 'ahead', 'ahead_by': 1, 'behind_by': 0, 'total_commits': 1, 'commits': [commit]},
            WINIT_TAGS: tag_rows('v0.30.11', 'v0.30.12', 'v0.31.0'),
            'repos/rust-windowing/winit/compare/' + 'a' * 40 + '...v0.30.12?per_page=250&page=1':
                {'status': 'identical', 'ahead_by': 0, 'behind_by': 0, 'total_commits': 0, 'commits': []},
            'repos/rust-windowing/winit/releases?per_page=100&page=1': [
                {'tag_name': 'v0.31.0', 'published_at': '2026-04-01T00:00:00Z', 'html_url': 'https://example.org/r'},
                {'tag_name': 'v0.30.12', 'published_at': '2026-01-03T00:00:00Z', 'html_url': 'https://example.org/q'}],
            'repos/rust-windowing/winit/releases/latest': {'tag_name': 'v0.31.0', 'published_at': '2026-04-01T00:00:00Z', 'html_url': 'https://example.org/r'},
        }
        patch = {'sha': '5' * 40, 'local': False, 'url': 'https://github.com/example/winit/commit/' + '5' * 40,
                 'file': '0001-keep.patch', 'date': '2026-03-04', 'title': 'keep the cursor grab', 'author': 'example/winit'}
        source = {'name': 'winit', 'upstream': 'https://github.com/rust-windowing/winit', 'commit': 'a' * 40, 'ref': 'v0.30.12', 'patches': [patch]}
        with mock.patch.object(drift, 'gh', fake_api(answers)), mock.patch.object(drift.sys, 'stderr'):
            entry, = drift.collect_sources([source])
        self.assertEqual((entry['url'], entry['repo'], entry['rev'], entry['branch']),
                         ('https://github.com/rust-windowing/winit', 'rust-windowing/winit', 'a' * 40, 'v0.30.12'))
        self.assertNotIn('crates', entry)
        self.assertEqual(entry['carried'], {'commits': [patch]})
        self.assertEqual(entry['base'], {'tag': 'v0.30.12', 'sha': 'a' * 40, 'date': '2026-01-02'})
        self.assertEqual(entry['newer_tags'], ['v0.31.0'])
        self.assertEqual(entry['upstream_since_pin']['total_commits'], 1)
        self.assertEqual(entry['upstream_since_pin']['commits'][0]['title'], 'a change')
        self.assertEqual(entry['branch_since_pin']['total_commits'], 0)
        self.assertEqual([r['tag'] for r in entry['releases_since_base']], ['v0.31.0'])
        self.assertEqual(entry['parent_latest_release']['tag'], 'v0.31.0')
        # The page reads no key the collector does not write; `crates` is the key of a lock source.
        script = site.PAGE[site.PAGE.index('<script>'):]
        read = set(re.findall(r'\bs\.([a-z_]+)', script))
        self.assertEqual(read - set(entry), {'crates'})
        self.assertEqual(set(re.findall(r'\bs\.carried\.([a-z_]+)', script)), set(entry['carried']))
        self.assertEqual(set(re.findall(r'\bc\.(sha|local|url|file|date|author|title)\b', script)) - set(patch), set())
        for gone in ('parent_info', 'vs_parent_default', 'config_pin', 'base_note', 'upstream_since_base', 'fork_only'):
            self.assertNotIn(gone, site.PAGE)

    def test_a_lock_source_has_crates_and_no_patch_series(self):
        answers = {'repos/gltf-rs/gltf': {'description': 'gltf', 'stargazers_count': 1, 'archived': False, 'default_branch': 'main'},
                   'repos/gltf-rs/gltf/commits/' + 'c' * 40: {'commit': {'committer': {'date': '2026-01-02T00:00:00Z'}}},
                   'repos/gltf-rs/gltf/compare/' + 'c' * 40 + '...main?per_page=250&page=1':
                       {'status': 'identical', 'ahead_by': 0, 'behind_by': 0, 'total_commits': 0, 'commits': []},
                   'repos/gltf-rs/gltf/releases?per_page=100&page=1': [],
                   'repos/gltf-rs/gltf/releases/latest': None}
        source = {'name': 'gltf', 'upstream': GLTF, 'commit': 'c' * 40, 'ref': None, 'reason': None, 'crates': [('gltf', '1.4.1')]}
        with mock.patch.object(drift, 'gh', fake_api(answers)), mock.patch.object(drift.sys, 'stderr'):
            entry, = drift.collect_sources([source])
        self.assertEqual(entry['crates'], [{'name': 'gltf', 'version': '1.4.1'}])
        for absent in ('carried', 'base', 'newer_tags', 'branch_since_pin'):
            self.assertNotIn(absent, entry)


class VersionLogicTests(unittest.TestCase):
    def test_requirements_follow_cargo_semantics(self):
        self.assertTrue(drift.matches_req('0.23.37', '>= 0.23.0'))
        self.assertFalse(drift.matches_req('0.23.37', '>= 0.23.38'))
        self.assertTrue(drift.matches_req('1.2.3', '^1.0.0'))
        self.assertFalse(drift.matches_req('2.0.0', '^1.0.0'))
        self.assertTrue(drift.matches_req('0.3.3', '^0.3.1'))
        self.assertFalse(drift.matches_req('0.4.0', '^0.3.1'))
        self.assertTrue(drift.matches_req('0.9.18', '>= 0.9.0, < 0.9.19'))
        self.assertFalse(drift.matches_req('0.9.19', '>= 0.9.0, < 0.9.19'))

    def test_affected_means_neither_patched_nor_unaffected(self):
        advisory = {'patched': ['>= 0.23.40'], 'unaffected': ['< 0.20.0']}
        self.assertTrue(drift.affected('0.23.37', advisory))
        self.assertFalse(drift.affected('0.23.40', advisory))
        self.assertFalse(drift.affected('0.19.0', advisory))

    def test_advisory_markdown_parses_front_matter_and_title(self):
        text = textwrap.dedent('''
            ```toml
            [advisory]
            id = "RUSTSEC-2026-0001"
            package = "demo"
            date = "2026-01-02"
            aliases = ["CVE-2026-1"]
            informational = "unmaintained"

            [versions]
            patched = []
            unaffected = ["< 0.1.0"]
            ```

            # demo is unmaintained

            Body text.
        ''')
        advisory = drift.parse_advisory(text)
        self.assertEqual(advisory['id'], 'RUSTSEC-2026-0001')
        self.assertEqual(advisory['title'], 'demo is unmaintained')
        self.assertEqual(advisory['informational'], 'unmaintained')
        self.assertEqual(advisory['unaffected'], ['< 0.1.0'])
        self.assertIsNone(drift.parse_advisory('no toml here'))

    def test_v8_engine_reads_the_readme_then_the_crate_major(self):
        self.assertEqual(drift.v8_engine('0.105.1', '# rusty_v8\n\nV8 Version: 12.9.202.9\n'), {'v8': '12.9.202.9', 'milestone': 129})
        self.assertEqual(drift.v8_engine('150.4.0'), {'v8': '15.0', 'milestone': 150})
        self.assertEqual(drift.v8_engine('129.0.0'), {'v8': '12.9', 'milestone': 129})
        self.assertEqual(drift.v8_engine('0.105.1'), {'v8': None, 'milestone': None})

    def test_newest_matching_skips_yanked_and_prereleases(self):
        versions = [{'num': '152.2.0'}, {'num': '150.5.0', 'yanked': True}, {'num': '150.4.0'}, {'num': '150.9.0-rc.1'}, {'num': '149.4.0'}]
        self.assertEqual(drift.newest_matching(versions, '^150.4.0'), '150.4.0')
        self.assertIsNone(drift.newest_matching(versions, '^153'))

    def v8_state(self, locked, accepted, chrome):
        return {'locked_versions': [locked['crate']] if locked else [], 'locked': locked, 'accepted': accepted,
                'deno_core': {'version': '0.413.0', 'v8_req': '^150.4.0'}, 'chrome': {'milestone': chrome}}

    def test_v8_findings_fire_behind_deno_core_and_far_from_chrome(self):
        findings = drift.v8_findings(self.v8_state({'crate': '0.105.1', 'v8': '12.9.202.9', 'milestone': 129},
                                                   {'crate': '150.4.0', 'v8': '15.0', 'milestone': 150}, 157))
        self.assertEqual(len(findings), 2)
        self.assertTrue(findings[0].startswith('behind deno_core'))
        self.assertIn('28 milestones behind Chrome stable M157', findings[1])

    def test_v8_findings_quiet_when_on_what_deno_core_accepts(self):
        self.assertEqual(drift.v8_findings(self.v8_state({'crate': '150.4.0', 'v8': '15.0', 'milestone': 150},
                                                         {'crate': '150.4.0', 'v8': '15.0', 'milestone': 150}, 157)), [])

    def test_v8_findings_flag_a_newer_v8_within_deno_cores_requirement(self):
        findings = drift.v8_findings(self.v8_state({'crate': '150.4.0', 'v8': '15.0.245.2', 'milestone': 150},
                                                   {'crate': '150.5.0', 'v8': '15.0.250.1', 'milestone': 150}, 151))
        self.assertEqual(findings, ["a newer v8 within deno_core's requirement: v8 150.4.0 (V8 15.0.245.2) is locked while "
                                    "deno_core 0.413.0 accepts v8 150.5.0 (V8 15.0.250.1); a lock-only bump of v8 takes it"])

    def test_v8_findings_flag_an_unknown_version(self):
        findings = drift.v8_findings(self.v8_state({'crate': '0.1.0', 'v8': None, 'milestone': None}, None, 157))
        self.assertEqual(findings, ['cannot tell the V8 version of the locked v8 0.1.0'])

    def test_v8_findings_flag_an_unresolved_deno_core_requirement(self):
        state = self.v8_state({'crate': '150.4.0', 'v8': '15.0', 'milestone': 150}, None, 151)
        state['deno_core']['v8_req'] = None
        self.assertEqual(drift.v8_findings(state), ['unable to resolve which v8 deno_core 0.413.0 accepts (requirement: not found)'])

    def test_v8_findings_flag_a_v8_outside_crates_io(self):
        state = self.v8_state(None, {'crate': '150.4.0', 'v8': '15.0', 'milestone': 150}, 157)
        state['unregistered'] = ['150.4.0 (git+https://github.com/example/rusty_v8#abc)']
        findings = drift.v8_findings(state)
        self.assertEqual(len(findings), 1)
        self.assertIn('v8 outside crates.io', findings[0])

    def test_collect_v8_keeps_a_v8_outside_crates_io(self):
        meta = {'v8': {'versions': [{'num': '150.4.0', 'created_at': '2026-06-01T00:00:00Z'}], 'crate': {'max_stable_version': '150.4.0'}},
                'deno_core': {'crate': {'max_stable_version': '0.413.0'}}}
        with patch.object(drift, 'crates_io', side_effect=meta.get), \
             patch.object(drift, 'v8_requirement', return_value='^150.4.0'), \
             patch.object(drift, 'rusty_v8_readme', return_value=None), \
             patch.object(drift, 'chrome_stable', return_value={'milestone': 157, 'version': '157.0.1', 'date': '2026-11-01'}):
            v8 = drift.collect_v8([{'name': 'v8', 'version': '150.4.0', 'source': ''}])
        self.assertIsNone(v8['locked'])
        self.assertEqual(v8['findings'], ['v8 outside crates.io, its age is not checked: 150.4.0 (path)'])
        self.assertIn('v8 outside crates.io', drift.v8_report_markdown(v8))

    def test_collect_v8_keeps_the_crates_io_comparisons_without_chrome(self):
        meta = {'v8': {'versions': [{'num': '0.105.1', 'created_at': '2024-10-01T00:00:00Z'}, {'num': '150.4.0', 'created_at': '2026-06-01T00:00:00Z'}],
                       'crate': {'max_stable_version': '150.4.0'}},
                'deno_core': {'crate': {'max_stable_version': '0.413.0'}}}
        lock = [{'name': 'v8', 'version': '0.105.1', 'source': 'registry+https://github.com/rust-lang/crates.io-index'}]
        readmes = {'0.105.1': 'V8 Version: 12.9.202.9\n', '150.4.0': 'V8 Version: 15.0.245.2\n'}
        with patch.object(drift, 'crates_io', side_effect=meta.get), \
             patch.object(drift, 'v8_requirement', return_value='^150.4.0'), \
             patch.object(drift, 'rusty_v8_readme', side_effect=readmes.get), \
             patch.object(drift, 'chrome_stable', side_effect=OSError('chromiumdash down')), \
             patch('sys.stderr'):
            v8 = drift.collect_v8(lock)
        self.assertIsNone(v8['chrome'])
        self.assertEqual(len(v8['findings']), 2)
        self.assertTrue(v8['findings'][0].startswith('behind deno_core: v8 0.105.1'))
        self.assertEqual(v8['findings'][1], 'Chrome stable could not be fetched, so the milestone gap is not checked')
        self.assertIn('| Chrome stable | – | – | ? | unavailable |', drift.v8_report_markdown(v8))

    def test_collect_v8_or_error_stores_the_failure(self):
        lock = [{'name': 'v8', 'version': '0.105.1', 'source': 'registry+https://github.com/rust-lang/crates.io-index'}]
        with patch.object(drift, 'crates_io', side_effect=OSError('crates.io down')):
            self.assertEqual(drift.collect_v8_or_error(lock), {'error': "OSError('crates.io down')"})

    def test_v8_report_fails_and_keeps_the_body_on_collect_errors(self):
        with tempfile.TemporaryDirectory() as tmp:
            data, body = Path(tmp, 'data.json'), Path(tmp, 'body.md')
            data.write_text('{"v8": {"error": "boom"}}')
            body.write_text('previous body')
            with patch.object(drift, 'DATA', data):
                self.assertEqual(drift.v8_report(str(body)), 1)
            self.assertEqual(body.read_text(), 'previous body')

    def catalyst_state(self, locked, newest, chrome):
        return {'locked_versions': [locked['crate']] if locked else [], 'locked': locked, 'newest': newest,
                'chrome': {'milestone': chrome, 'version': f'{chrome}.0.1', 'date': '2026-11-01'}, 'chrome_gap_limit': 12}

    def test_catalyst_v8_findings_flag_any_newer_rusty_v8(self):
        findings = drift.catalyst_v8_findings(self.catalyst_state({'crate': '152.2.0', 'v8': '15.2.124.1', 'milestone': 152},
                                                                  {'crate': '152.3.0', 'v8': '15.2.130.0', 'milestone': 152}, 157))
        self.assertEqual(len(findings), 1)
        self.assertTrue(findings[0].startswith('behind the newest rusty_v8: v8 152.2.0'))
        self.assertIn('services/catalyst/Cargo.toml', findings[0])

    def test_catalyst_v8_findings_quiet_on_the_newest_rusty_v8(self):
        entry = {'crate': '152.2.0', 'v8': '15.2.124.1', 'milestone': 152}
        self.assertEqual(drift.catalyst_v8_findings(self.catalyst_state(entry, entry, 157)), [])

    def test_catalyst_v8_findings_flag_the_chrome_gap_and_an_unresolved_newest(self):
        findings = drift.catalyst_v8_findings(self.catalyst_state({'crate': '140.0.0', 'v8': '14.0', 'milestone': 140}, None, 157))
        self.assertEqual(findings, ['unable to resolve the newest rusty_v8 on crates.io',
                                    'M140 is 17 milestones behind Chrome stable M157 (limit 12)'])

    def test_catalyst_v8_report_renders_without_chrome(self):
        entry = {'crate': '152.2.0', 'v8': '15.2.124.1', 'milestone': 152}
        state = {**self.catalyst_state(entry, entry, 157), 'chrome': None}
        state['findings'] = drift.catalyst_v8_findings(state)
        self.assertEqual(state['findings'], ['Chrome stable could not be fetched, so the milestone gap is not checked'])
        self.assertIn('| Chrome stable | – | – | ? | unavailable |', drift.catalyst_v8_report_markdown(state))

    def test_collect_catalyst_v8_reads_the_direct_pin(self):
        meta = {'v8': {'versions': [{'num': '152.2.0', 'created_at': '2026-10-01T00:00:00Z'}, {'num': '153.0.0', 'created_at': '2026-11-02T00:00:00Z'}],
                       'crate': {'max_stable_version': '153.0.0'}}}
        lock = [{'name': 'v8', 'version': '152.2.0', 'source': 'registry+https://github.com/rust-lang/crates.io-index'}]
        with patch.object(drift, 'crates_io', side_effect=meta.get), \
             patch.object(drift, 'rusty_v8_readme', return_value=None), \
             patch.object(drift, 'chrome_stable', return_value={'milestone': 157, 'version': '157.0.1', 'date': '2026-11-01'}):
            v8 = drift.collect_catalyst_v8(lock)
        self.assertEqual((v8['locked']['crate'], v8['locked']['date'], v8['newest']['milestone']), ('152.2.0', '2026-10-01', 153))
        self.assertEqual(len(v8['findings']), 1)
        self.assertIn('| pinned | 152.2.0 | 15.2 | M152 | 2026-10-01 |', drift.catalyst_v8_report_markdown(v8))

    def test_collect_v8_or_error_covers_a_missing_catalyst_lock(self):
        missing = Path(tempfile.gettempdir(), 'no-such-dir', 'Cargo.lock')
        error = drift.collect_v8_or_error(missing, lambda path: drift.collect_catalyst_v8(drift.lock_packages(path.read_text())))
        self.assertIn('FileNotFoundError', error['error'])

    def write_report(self, data):
        with tempfile.TemporaryDirectory() as tmp:
            path, body = Path(tmp, 'data.json'), Path(tmp, 'body.md')
            path.write_text(drift.json.dumps(data))
            with patch.object(drift, 'DATA', path), patch('sys.stdout'):
                code = drift.v8_report(str(body))
            return code, body.read_text() if body.exists() else None

    def explorer_current(self):
        entry = {'crate': '150.4.0', 'v8': '15.0', 'milestone': 150}
        state = self.v8_state(entry, entry, 151)
        state.update(newest=entry, chrome={'milestone': 151, 'version': '151.0.1', 'date': '2026-11-01'}, findings=[])
        return state

    def test_v8_report_opens_the_issue_for_the_catalyst_alone(self):
        catalyst = self.catalyst_state({'crate': '152.2.0', 'v8': '15.2', 'milestone': 152}, {'crate': '153.0.0', 'v8': '15.3', 'milestone': 153}, 157)
        catalyst['findings'] = drift.catalyst_v8_findings(catalyst)
        code, body = self.write_report({'v8': self.explorer_current(), 'catalyst_v8': catalyst})
        self.assertEqual(code, 0)
        self.assertIn('## Bevy Explorer scene runtime', body)
        self.assertIn(drift.v8_report_markdown(self.explorer_current()), body)
        self.assertIn('## Catalyst SDK7 scene host', body)
        self.assertIn('behind the newest rusty_v8', body)

    def test_v8_report_keeps_the_other_findings_when_one_workspace_fails(self):
        catalyst = self.catalyst_state({'crate': '152.2.0', 'v8': '15.2', 'milestone': 152}, {'crate': '152.2.0', 'v8': '15.2', 'milestone': 152}, 157)
        catalyst['findings'] = []
        code, body = self.write_report({'v8': {'error': 'boom'}, 'catalyst_v8': catalyst})
        self.assertEqual(code, 0)
        self.assertIn("- unable to resolve the V8 freshness data: boom", body)
        self.assertIn('| pinned | 152.2.0 |', body)

    def test_v8_report_flags_an_explorer_lock_without_v8(self):
        entry = {'crate': '152.2.0', 'v8': '15.2', 'milestone': 152}
        catalyst = {**self.catalyst_state(entry, entry, 157), 'findings': []}
        code, body = self.write_report({'v8': None, 'catalyst_v8': catalyst})
        self.assertEqual(code, 0)
        self.assertIn('## Bevy Explorer scene runtime\n\nFindings:\n\n- no v8 crate in the lock', body)

    def test_v8_report_removes_the_body_when_both_are_current(self):
        entry = {'crate': '152.2.0', 'v8': '15.2', 'milestone': 152}
        catalyst = {**self.catalyst_state(entry, entry, 157), 'findings': []}
        self.assertEqual(self.write_report({'v8': self.explorer_current(), 'catalyst_v8': catalyst}), (0, None))

    def test_v8_report_fails_when_neither_workspace_was_collected(self):
        self.assertEqual(self.write_report({'v8': {'error': 'a'}, 'catalyst_v8': {'error': 'b'}}), (1, None))

    def test_v8_issue_job_writes_issues_without_running_repository_code(self):
        text = WORKFLOW.read_text()
        job = text[text.index('  v8-issue:'):text.index('  report:')]
        self.assertIn('permissions:\n      issues: write\n', job)
        self.assertNotIn('actions/checkout', job)
        self.assertNotIn('python3', job)
        self.assertNotIn('--search', job)
        self.assertIn('select(.title == env.TITLE and .author.login == "app/github-actions")', job)
        self.assertIn('gh issue close "$extra" --comment "Duplicate of #$keep."', job)
        drift_job = text[text.index('  drift:'):text.index('  v8-issue:')]
        self.assertNotIn('issues: write', drift_job)
        self.assertIn('python3 tooling/upstream-drift.py v8-report --body "$body"', drift_job)

    def test_advisory_files_reads_every_crate_of_the_tree(self):
        tree = {'truncated': False, 'tree': [
            {'path': 'crates', 'type': 'tree', 'sha': 't0'},
            {'path': 'crates/zvariant_derive', 'type': 'tree', 'sha': 't1'},
            {'path': 'crates/zvariant_derive/RUSTSEC-2026-0002.md', 'type': 'blob', 'sha': 'b1'},
            {'path': 'crates/rustls/RUSTSEC-2026-0004.md', 'type': 'blob', 'sha': 'b3'},
            {'path': 'crates/rustls/RUSTSEC-2026-0003.md', 'type': 'blob', 'sha': 'b2'},
            {'path': 'crates/rustls/notes.txt', 'type': 'blob', 'sha': 'b4'},
            {'path': 'rust/std/RUSTSEC-2026-0005.md', 'type': 'blob', 'sha': 'b5'},
            {'path': 'README.md', 'type': 'blob', 'sha': 'b6'},
        ]}
        self.assertEqual(drift.advisory_files(tree), {
            'zvariant_derive': ['b1'],
            'rustls': ['b2', 'b3'],
        })

    def test_advisory_files_refuses_a_truncated_tree(self):
        with self.assertRaises(RuntimeError):
            drift.advisory_files({'truncated': True, 'tree': []})

    def test_advisory_files_refuses_a_missing_tree(self):
        with self.assertRaisesRegex(RuntimeError, 'could not be fetched'):
            drift.advisory_files(None)

    def test_collect_advisories_skips_withdrawn_and_reports_unreadable_entries(self):
        def advisory(id_, extra=''):
            text = f'```toml\n[advisory]\nid = "{id_}"\npackage = "ring"\ndate = "2025-01-01"\n{extra}\n[versions]\npatched = [">= 0.17.12"]\n```\n# {id_}\n'
            return {'content': base64.b64encode(text.encode()).decode()}
        tree = {'truncated': False, 'tree': [
            {'path': f'crates/ring/RUSTSEC-2025-000{i}.md', 'type': 'blob', 'sha': f'b{i}'} for i in range(1, 5)]}
        responses = {
            'repos/rustsec/advisory-db/git/trees/main?recursive=1': tree,
            'repos/rustsec/advisory-db/git/blobs/b1': advisory('RUSTSEC-2025-0001'),
            'repos/rustsec/advisory-db/git/blobs/b2': advisory('RUSTSEC-2025-0002', 'withdrawn = "2025-02-22"'),
            'repos/rustsec/advisory-db/git/blobs/b3': None,
            'repos/rustsec/advisory-db/git/blobs/b4': {'content': base64.b64encode(b'no front matter').decode()},
        }
        with mock.patch.object(drift, 'gh', side_effect=responses.__getitem__):
            result = drift.collect_advisories([{'name': 'ring', 'version': '0.17.8'}])
        self.assertEqual([f['id'] for f in result['findings']], ['RUSTSEC-2025-0001'])
        self.assertEqual(result['withdrawn'], ['RUSTSEC-2025-0002'])
        self.assertEqual(result['unreadable'], [
            {'crate': 'ring', 'sha': 'b3', 'reason': 'blob not found'},
            {'crate': 'ring', 'sha': 'b4', 'reason': 'front matter does not parse'},
        ])

    def test_tag_helpers(self):
        self.assertEqual(drift.version_key('v0.16.1'), (0, 16, 1, 0))
        self.assertGreater(drift.version_key('v1.46.3'), drift.version_key('v0.28.0'))
        self.assertEqual(drift.tag_prefix('livekit-ffi/v0.12.83'), 'livekit-ffi/v')
        self.assertEqual(drift.tag_prefix('0.307.0'), '')
        ordered = ['v0.9.9', 'v0.10.0-alpha.9', 'v0.10.0-alpha.10', 'v0.10.0-beta.1', 'v0.10.0-rc.2', 'v0.10.0', 'v0.10.1']
        self.assertEqual(sorted(reversed(ordered), key=drift.tag_order), ordered)

    def test_lock_packages_and_shape(self):
        lock = textwrap.dedent('''
            [[package]]
            name = "a"
            version = "1.0.0"
            source = "registry+https://github.com/rust-lang/crates.io-index"

            [[package]]
            name = "a"
            version = "2.0.0"
            source = "registry+https://github.com/rust-lang/crates.io-index"

            [[package]]
            name = "b"
            version = "0.1.0"
            source = "git+https://github.com/x/b?branch=main#abc"

            [[package]]
            name = "ws"
            version = "0.0.0"
        ''')
        pkgs = drift.lock_packages(lock)
        shape = drift.lock_shape(pkgs)
        self.assertEqual(shape['packages'], 4)
        self.assertEqual(shape['by_source'], {'crates.io': 2, 'git': 1, 'workspace': 1})
        self.assertEqual(shape['duplicates'], [{'name': 'a', 'versions': ['1.0.0', '2.0.0']}])


class SiteTests(unittest.TestCase):
    def test_markdown_covers_the_review_constructs(self):
        html = site.markdown_to_html('# Title\n\nA **bold** `code` [link](https://x.y) line.\n\n- one\n- two\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n')
        self.assertIn('<h1 id="title">Title</h1>', html)
        self.assertIn('<strong>bold</strong> <code>code</code> <a href="https://x.y"', html)
        self.assertIn('<ul>\n<li>one</li>\n<li>two</li>\n</ul>', html)
        self.assertIn('<table><thead><tr><th>a</th><th>b</th></tr></thead><tbody><tr><td>1</td><td>2</td></tr></tbody></table>', html)

    def test_slug(self):
        self.assertEqual(site.slug('https://github.com/robtfm/bevy_kira_audio'), 'robtfm-bevy-kira-audio')

    # The advisories view is JavaScript inside the page, so these run its `esc`, `advisoryTable`
    # and `advisories` under Node with a stand-in for the collected data.
    def render_advisories(self, advisories, q=''):
        node = shutil.which('node')
        if not node:
            if os.environ.get('CI'):
                self.fail('node is needed to render the advisories view')
            self.skipTest('node is not installed')
        esc = re.search(r'^const esc = .*$', site.PAGE, re.M).group(0)
        functions = [re.search(rf'^function {name}\(.*?^\}}$', site.PAGE, re.M | re.S).group(0)
                     for name in ('advisoryTable', 'advisories')]
        script = '\n'.join([f'const D = {{advisories: {json.dumps(advisories)}}};',
                            f'const state = {{q: {json.dumps(q)}}};', esc, *functions,
                            'process.stdout.write(advisories());'])
        return subprocess.run([node, '-'], input=script, capture_output=True, text=True, check=True).stdout

    FINDINGS = [
        {'crate': 'ring', 'version': '0.17.8', 'id': 'RUSTSEC-2025-0009', 'title': 'AES panic',
         'date': '2025-03-06', 'patched': ['>= 0.17.12'], 'affected': True, 'rustsec_url': 'https://rustsec.org/a'},
        {'crate': 'paste', 'version': '1.0.15', 'id': 'RUSTSEC-2024-0436', 'title': 'unmaintained',
         'date': '2024-10-07', 'patched': [], 'affected': False, 'rustsec_url': 'https://rustsec.org/b'},
    ]

    def test_advisories_view_reports_withdrawn_and_unreadable_entries(self):
        html = self.render_advisories({
            'crates_with_entries': ['ring', 'paste'], 'findings': self.FINDINGS,
            'withdrawn': ['RUSTSEC-2025-0002', 'RUSTSEC-2025-0003'],
            'unreadable': [{'crate': 'ring', 'sha': 'b3', 'reason': 'blob not found'},
                           {'crate': '<img src=x>', 'sha': 'b4', 'reason': 'a & "b"'}],
        })
        self.assertIn(' 2 withdrawn entries skipped.', html)
        self.assertIn('<span class="badge b-security">2 entries could not be read</span>: '
                      'ring (blob not found), &lt;img src=x&gt; (a &amp; &quot;b&quot;).', html)
        self.assertNotIn('<img', html)

    def test_advisories_view_without_withdrawn_or_unreadable_entries(self):
        # Data collected before these fields existed has neither key.
        for extra in ({}, {'withdrawn': [], 'unreadable': []}):
            with self.subTest(extra=extra):
                html = self.render_advisories({'crates_with_entries': ['ring', 'paste'], 'findings': self.FINDINGS, **extra})
                self.assertIn('2 crates in the lock have advisory-db entries; 1 entries affect the pinned versions', html)
                self.assertNotIn('withdrawn', html)
                self.assertNotIn('could not be read', html)
                self.assertIn('<tr class="aff">', html)
                self.assertLess(html.index('RUSTSEC-2025-0009'), html.index('RUSTSEC-2024-0436'))

    def test_advisories_view_filters_the_table(self):
        html = self.render_advisories({'crates_with_entries': ['ring', 'paste'], 'findings': self.FINDINGS,
                                       'withdrawn': ['RUSTSEC-2025-0002']}, q='paste')
        self.assertIn('RUSTSEC-2024-0436', html)
        self.assertNotIn('RUSTSEC-2025-0009', html)
        self.assertIn(' 1 withdrawn entries skipped.', html)


if __name__ == '__main__':
    unittest.main()
