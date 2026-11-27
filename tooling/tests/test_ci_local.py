"""The local hooks select the same workflows GitHub would for a change."""
import importlib.util
import errno
import io
import os
import shutil
import subprocess
import sys
import tempfile
from types import SimpleNamespace
from pathlib import Path
import re
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('ci_local', Path(__file__).parents[1] / 'ci-local.py')
ci = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ci)


class CiLocalTest(unittest.TestCase):
    def setUp(self):
        # Standalone runs must inspect this checkout, not a stale hook worktree.
        tree = patch.object(ci, 'TREE', ci.ROOT)
        tree.start()
        self.addCleanup(tree.stop)

    def test_reads_pull_request_path_filters(self):
        # Development runs on every pull request (its CI gate always reports); what it tests is its push.paths.
        self.assertIsNone(ci.workflow_paths('development.yml'))
        self.assertIn('services/catalyst/**', ci.workflow_paths('development.yml', event='push'))
        self.assertIn('tooling/image-*', ci.workflow_paths('apis-sites-images.yml'))
        self.assertIsNone(ci.workflow_paths('structure.yml'))

    def test_globs_follow_github_semantics(self):
        self.assertTrue(ci.glob_regex('services/catalyst/**').match('services/catalyst/db/src/lib.rs'))
        self.assertTrue(ci.glob_regex('tooling/dev*').match('tooling/dev-smoke.sh'))
        self.assertFalse(ci.glob_regex('tooling/dev*').match('tooling/dev/x.sh'))
        self.assertFalse(ci.glob_regex('flake.*').match('apps/flake.nix'))

    def test_selects_triggered_workflows(self):
        self.assertTrue(ci.triggered('development.yml', {'schemas/001.sql'}, event='push'))
        self.assertFalse(ci.triggered('development.yml', {'docs/README.md'}, event='push'))
        self.assertTrue(ci.triggered('apis-sites-images.yml', {'tooling/image-smoke.py'}))
        self.assertTrue(ci.triggered('structure.yml', {'docs/README.md'}))
        for hook in (ci.pre_commit_steps, ci.pre_push_steps):
            for changed in ({'docs/README.md'}, {'.github/workflows/ui-library.yml'}):
                with self.subTest(hook=hook.__name__, changed=changed):
                    self.assertEqual([s.name for s in hook(changed)],
                                     ['ownership catalogue', 'module layers', 'workflow tests'])

    def test_clippy_runs_from_the_catalyst_folder_and_lints_scene_cli_only_changes(self):
        steps = {s.name: s.command for s in ci.pre_push_steps({'services/catalyst/src/lib.rs'})}
        self.assertTrue(steps['catalyst clippy'].startswith('cd services/catalyst && cargo clippy --locked --workspace '))
        self.assertEqual(steps['catalyst clippy (ts)'], 'cd services/catalyst && cargo clippy --locked '
                         '--workspace --all-targets --features catalyst/ts -- -D warnings')
        self.assertNotIn('scene cli clippy', steps)
        names = list(steps)
        self.assertEqual(names.index('unused dependencies'), names.index('catalyst clippy (ts)') + 1)
        self.assertEqual(steps['unused dependencies'], 'nix build --inputs-from . nixpkgs#cargo-machete -o '
                         '.cache/cargo-machete && .cache/cargo-machete/bin/cargo-machete services/catalyst '
                         'library/dcl-* apps/dcl-scene-cli')
        steps = {s.name: s.command for s in ci.pre_push_steps({'apps/dcl-scene-cli/src/main.rs'})}
        self.assertNotIn('catalyst clippy', steps)
        self.assertNotIn('catalyst clippy (ts)', steps)
        self.assertEqual(steps['scene cli clippy'],
                         'cd services/catalyst && cargo clippy --locked -p dcl-scene-cli --all-targets -- -D warnings')
        self.assertEqual(steps['scene cli unused dependencies'], 'nix build --inputs-from . nixpkgs#cargo-machete -o '
                         '.cache/cargo-machete && .cache/cargo-machete/bin/cargo-machete apps/dcl-scene-cli')
        self.assertIn('.cache/cargo-machete/bin/cargo-machete apps/dcl-scene-cli\n',
                      (ci.ROOT / '.github/workflows/sdk.yml').read_text())

    def test_documentation_is_excluded_from_development_and_image_inputs(self):
        docs = {'DATABASE.md', 'services/catalyst/docs/events.md',
                'services/catalyst/docs/example.json', 'library/example/README.md',
                'rendering/bevy-explorer/docs/example.json'}
        for workflow in ('development.yml', 'apis-sites-images.yml'):
            with self.subTest(workflow=workflow):
                self.assertFalse(ci.triggered(workflow, docs, event='push'))
                self.assertTrue(ci.triggered(workflow, docs | {'services/catalyst/src/lib.rs'}, event='push'))

    def test_negative_globs_match_root_files_and_allow_later_inclusions(self):
        self.assertTrue(ci.glob_regex('**/*.md').match('README.md'))
        self.assertTrue(ci.glob_regex('**/docs/**').match('docs/example.json'))
        with patch.object(ci, 'workflow_paths', return_value=['**', '!**/*.md', 'keep.md']):
            self.assertFalse(ci.triggered('example.yml', {'README.md'}))
            self.assertTrue(ci.triggered('example.yml', {'keep.md'}))

    def test_documentation_patterns_match_development_workflow(self):
        paths = ci.workflow_paths('development.yml', event='push')
        expected = ci.paths.DEVELOPMENT_DOCUMENTATION_PATHS
        self.assertEqual(tuple(paths[-len(expected):]), expected)

    def test_fixture_exceptions_stay_within_each_workflows_inputs(self):
        bevy = 'rendering/bevy-explorer/source/local-harness/benchmark/tests/fixtures/report/ok.md'
        creator = 'apps/creator-hub/tests/fixtures/example.md'
        for path in (bevy, creator, 'apps/creator-hub/tests/fixtures/example.json'):
            with self.subTest(path=path):
                self.assertFalse(ci.triggered('development.yml', [path], event='push'))
        self.assertFalse(ci.triggered('apis-sites-images.yml', [creator]))
        self.assertFalse(ci.triggered('apis-sites-images.yml', ['apps/creator-hub/tests/fixtures/example.json']))
        self.assertTrue(ci.triggered('apis-sites-images.yml', [bevy]))
        for path in ('services/catalyst/tests/fixtures/example.md',
                     'services/catalyst/db/tests/fixtures/example.md',
                     'library/example/tests/fixtures/example.md'):
            self.assertTrue(ci.triggered('development.yml', [path], event='push'))

    def test_shared_matcher_triggers_engine_images_and_scenes(self):
        for workflow in ('bevy-explorer-ci.yml', 'bevy-explorer-scenes.yml', 'apis-sites-images.yml'):
            with self.subTest(workflow=workflow):
                self.assertTrue(ci.triggered(workflow, ['tooling/ci_paths.py'], event='push'))

    def test_docs_source_embedded_data_and_published_content_select_jobs(self):
        for path in ('apps/sites/src/features/docs/DocsPage.tsx',
                     'apps/sites/src/data/lib/docs/render.ts',
                     'library/dcl-scene-authoring/docs/composite-component-schemas.json',
                     'apps/sites/content/docs/example.md', 'apps/sites/content/docs/manifest.json',
                     'services/catalyst/tests/fixtures/example.md'):
            for workflow in ('development.yml', 'apis-sites-images.yml'):
                with self.subTest(path=path, workflow=workflow):
                    self.assertTrue(ci.triggered(workflow, [path], event='push'))

    def test_shared_ui_edits_run_the_sites_unit_tests_before_commit(self):
        names = lambda changed: [step.name for step in ci.pre_commit_steps(changed)]
        for path in ('apps/sites/src/routes/root.tsx', 'library/ui/figma/src/cli.ts'):
            with self.subTest(path=path):
                self.assertIn('sites unit tests', names({path}))
                self.assertNotIn('ui3 unit tests', names({path}))
        for path in ('library/ui/apps/shop/ShopChrome.tsx', 'library/ui/apps/hud/Panel.tsx',
                     'library/ui/components/AppChrome/SiteLayout.tsx', 'library/ui/apps/sites/chrome/SitesFooter.tsx',
                     'library/ui/components/NavBar/NavMenuItem.tsx', 'library/ui/components/Button/Button.tsx',
                     'library/ui/apps/sites/shared/Button/Button.tsx', 'library/ui/apps/sites/events/MyEvents.tsx',
                     'library/ui/foundations/icons/index.ts', 'library/ui/index.ts'):
            with self.subTest(path=path):
                self.assertIn('ui3 unit tests', names({path}))
                self.assertIn('sites unit tests', names({path}))
        self.assertNotIn('sites unit tests', names({'docs/README.md'}))

    def test_every_library_ui_module_ui3_imports_selects_the_ui3_tests(self):
        root = Path(__file__).resolve().parents[2]
        # The package specifier, or a relative path into library/ui (browser tests run outside `pnpm test`).
        specifier = re.compile(r"""['"](?:@dcl/one-ui|(?:\.\./)+library/ui)(/[^'"?]*)?['"?]""")
        imported = set()
        for source in (root / 'apps/ui3/src').rglob('*'):
            if source.suffix not in ('.ts', '.tsx') or 'node_modules' in source.parts or '.browser.test.' in source.name:
                continue
            for match in specifier.finditer(source.read_text(encoding='utf-8', errors='ignore')):
                sub = (match.group(1) or '').strip('/')
                imported.add(f'library/ui/{sub}' if sub else 'library/ui/index.ts')
        self.assertTrue(imported)
        uncovered = sorted(path for path in imported if not ci.touches({path}, *ci.UI3_SOURCES))
        self.assertEqual(uncovered, [], 'add the folder to UI3_SOURCES in tooling/ci-local.py')

    def test_engine_edits_select_the_engine_and_its_sites_consumer(self):
        changed = {'rendering/bevy-explorer/source/src/lib.rs'}
        self.assertTrue(ci.triggered('bevy-explorer-ci.yml', changed, event='push'))
        self.assertTrue(ci.triggered('apis-sites-images.yml', changed))
        self.assertFalse(ci.triggered('development.yml', changed, event='push'))

    def test_zip_extract_edits_select_the_scenes_that_install_it(self):
        # The bridge scene and the editor scene copy it (pnpm-workspace.yaml); Creator Hub CI builds neither.
        changed = {'apps/creator-hub/packages/zip-extract/index.js'}
        for event in ('push', 'pull_request'):
            self.assertTrue(ci.triggered('bevy-explorer-ci.yml', changed, event=event))
        self.assertTrue(ci.triggered('apis-sites-images.yml', changed))
        self.assertTrue(ci.triggered('development.yml', changed, event='push'))

    def test_root_cargo_and_toolchain_edits_select_sites_and_its_web_producer(self):
        for path in ('.cargo/config.toml', 'rust-toolchain', 'rust-toolchain.toml'):
            with self.subTest(path=path):
                self.assertTrue(ci.triggered('apis-sites-images.yml', {path}))
                self.assertTrue(ci.triggered('bevy-explorer-ci.yml', {path}, event='push'))

    def test_cargo_only_push_and_pr_select_sites_and_its_web_producer(self):
        for workflow in ('bevy-explorer-ci.yml',):
            source = (ci.ROOT / '.github/workflows' / workflow).read_text()
            section = re.search(r'(?ms)^  push:\n(.*?)(?=^  \S|\Z)', source)
            self.assertIsNotNone(section)
            paths = [line.strip()[2:].strip('\'"') for line in
                     section.group(1).split('    paths:\n', 1)[1].splitlines()
                     if line.strip().startswith('- ')]
            self.assertIn('.cargo/**', paths)
            # Pull requests run on every change so the Explorer CI gate always reports.
            self.assertIsNone(ci.workflow_paths(workflow))

    def test_bevy_selection_changes_run_their_guards_locally_and_in_ci(self):
        for name in ('tooling/bevy-ci.py', 'tooling/tests/test_bevy_ci.py', 'tooling/tests/test_bevy_protoc.py'):
            changed = {name}
            self.assertTrue(ci.triggered('bevy-explorer-ci.yml', changed, event='push'))
            self.assertIn(ci.TOOLING_TESTS, ci.pre_push_steps(changed))

    def test_native_build_inputs_publish_headless_packages(self):
        for name in ('rendering/bevy-explorer/source/.cargo/config.toml', 'rendering/bevy-explorer/source/build.rs',
                     'rendering/bevy-explorer/source/crates/scene_runner/src/lib.rs',
                     'rendering/bevy-explorer/source/Cargo.lock', 'rendering/bevy-explorer/source/deploy/headless/launcher/install.js'):
            self.assertTrue(ci.triggered('bevy-explorer-headless.yml', {name}, event='push'), name)
        # The headless build takes each patched source by the revision that Cargo.lock names.
        for name in ('rendering/bevy-explorer/source/react-web/src/App.tsx', 'rendering/bevy-explorer/source/deploy/web/engine/boot.js',
                     'apps/bevy-explorer/Dockerfile.web', 'tooling/patched-sources.py',
                     'rendering/bevy-explorer/patches/bevy/series/0001-scene-spawner.patch'):
            self.assertFalse(ci.triggered('bevy-explorer-headless.yml', {name}, event='push'), name)

    def test_gateway_edits_select_explorer_contract_but_not_application_images(self):
        changed = {'infra/gateway/gateway.py'}
        self.assertTrue(ci.triggered('gateway-images.yml', changed))
        self.assertFalse(ci.triggered('apis-sites-images.yml', changed))
        self.assertTrue(ci.triggered('development.yml', changed, event='push'))
        for event in ('push', 'pull_request'):
            with self.subTest(event=event):
                self.assertTrue(ci.triggered('bevy-explorer-ci.yml', changed, event=event))
        self.assertIn('gateway configuration', [step.name for step in ci.pre_commit_steps(changed)])


class HookQueueTest(unittest.TestCase):
    def test_a_legitimate_run_holding_the_lock_longer_than_120_seconds_is_queued(self):
        attempts = []
        sleeps = []
        def locking(fd, mode, length):
            attempts.append((fd, mode, length))
            if len(attempts) <= 70:
                raise OSError(errno.EACCES, 'lock held')
        module = SimpleNamespace(LK_NBLCK=1, locking=locking)
        notice = io.StringIO()
        with tempfile.TemporaryDirectory() as folder, patch.object(ci, 'STATE', Path(folder)), \
                patch.object(ci, 'WINDOWS', True), patch.dict(sys.modules, {'msvcrt': module}), \
                patch.object(ci.time, 'sleep', side_effect=sleeps.append), patch('sys.stderr', notice):
            with ci.locked():
                self.assertEqual(len(attempts), 71)
        self.assertGreater(sum(sleeps), 120)
        self.assertEqual(sleeps[:5], [.1, .2, .4, .8, 1.6])
        self.assertTrue(all(delay == 2 for delay in sleeps[5:]))
        self.assertEqual(notice.getvalue().count('Waiting for another local CI run'), 1)
        self.assertIn('Ctrl-C', notice.getvalue())

    def test_non_contention_windows_errors_fail_immediately(self):
        def locking(*args):
            raise OSError(errno.EBADF, 'invalid handle')
        module = SimpleNamespace(LK_NBLCK=1, locking=locking)
        with tempfile.TemporaryDirectory() as folder, patch.object(ci, 'STATE', Path(folder)), \
                patch.object(ci, 'WINDOWS', True), patch.dict(sys.modules, {'msvcrt': module}), \
                patch.object(ci.time, 'sleep') as sleeping:
            with self.assertRaises(OSError) as raised:
                with ci.locked():
                    self.fail('invalid handle acquired')
            self.assertEqual(raised.exception.errno, errno.EBADF)
        sleeping.assert_not_called()

    def test_a_held_lock_is_announced_on_posix(self):
        for held, expected in ((False, [6]), (True, [6, 2])):
            calls = []
            def flock(handle, mode):
                calls.append(mode)
                if held and mode & 4:
                    raise BlockingIOError
            notice = io.StringIO()
            with self.subTest(held=held), tempfile.TemporaryDirectory() as folder, \
                    patch.object(ci, 'STATE', Path(folder)), patch.object(ci, 'WINDOWS', False), \
                    patch.dict(sys.modules, {'fcntl': SimpleNamespace(LOCK_EX=2, LOCK_NB=4, flock=flock)}), \
                    patch('sys.stderr', notice):
                with ci.locked():
                    self.assertEqual(calls, expected)
                self.assertEqual(notice.getvalue().count('Waiting for another local CI run'), int(held))


def stock_lfs_hook(name):
    """What git-lfs 3.x writes."""
    return ('#!/bin/sh\ncommand -v git-lfs >/dev/null 2>&1 || { printf >&2 "\\n%s\\n\\n" "This repository is '
            "configured for Git LFS but 'git-lfs' was not found on your path. If you no longer wish to use Git LFS, "
            f"remove this hook by deleting the '{name}' file in the hooks directory (set by 'core.hookspath'; usually "
            f"'.git/hooks').\"; exit 2; }}\ngit lfs {name} \"$@\"\n")


def old_lfs_hook(name):
    """What git-lfs 2.x writes."""
    return ('#!/bin/sh\ncommand -v git-lfs >/dev/null 2>&1 || { echo >&2 "\\nThis repository is configured for Git LFS '
            "but 'git-lfs' was not found on your path. If you no longer wish to use Git LFS, remove this hook by "
            f'deleting .git/hooks/{name}.\\n"; exit 2; }}\ngit lfs {name} "$@"\n')


@unittest.skipIf(os.name == 'nt', 'the hooks are POSIX shell scripts, and symlinks need extra rights on Windows')
class HookTest(unittest.TestCase):
    """install and the hook entry points, each in its own scratch repository."""

    def setUp(self):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        self.folder = Path(folder.name)
        config = self.folder / 'gitconfig'
        config.write_text('[user]\n\tname = CI test\n\temail = ci@example.invalid\n[commit]\n\tgpgsign = false\n'
                          '[tag]\n\tgpgsign = false\n[init]\n\tdefaultBranch = main\n')
        environment = patch.dict(os.environ)
        environment.start()
        self.addCleanup(environment.stop)
        for name in [name for name in os.environ if name.startswith('GIT_')] + ['CI_LOCAL_SKIP']:
            os.environ.pop(name, None)
        os.environ.update(GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL=str(config), HOME=str(self.folder),
                          XDG_CONFIG_HOME=str(self.folder), GIT_CEILING_DIRECTORIES=str(self.folder.parent))
        # The ceiling stops git searching above the temp folder, so until repo() makes one ROOT is no
        # repository and an unisolated git call fails here.
        for name, value in (('ROOT', self.folder), ('STATE', self.folder / 'state'), ('TREE', self.folder / 'state/tree')):
            patcher = patch.object(ci, name, value)
            patcher.start()
            self.addCleanup(patcher.stop)

    def repo(self):
        root = Path(tempfile.mkdtemp(dir=self.folder))
        ci.git('init', '-q', cwd=root)
        patcher = patch.object(ci, 'ROOT', root)
        patcher.start()
        self.addCleanup(patcher.stop)
        return root

    def hooks_path(self, root):
        return ci.git('config', '--get', 'core.hooksPath', cwd=root, check=False)

    def install(self):
        with patch('builtins.print') as printed:
            ok = ci.install()
        self.output = ' '.join(str(arg) for call in printed.call_args_list for arg in call.args)
        return ok

    def test_install_replaces_the_stock_lfs_pre_push_and_undoes_the_old_hooks_path(self):
        root = self.repo()
        hooks = root / '.git/hooks'
        for name in ('pre-push', 'post-checkout'):
            (hooks / name).write_text(stock_lfs_hook(name))
        for attempt in ('first', 'again', 'after the old install set .githooks again'):
            with self.subTest(attempt=attempt):
                if attempt != 'again':
                    ci.git('config', 'core.hooksPath', '.githooks', cwd=root)
                self.assertTrue(self.install())
                self.assertNotIn('runs these hooks again', self.output)
                self.assertEqual(self.hooks_path(root), '')
                for name, text in ci.HOOKS.items():
                    self.assertEqual((hooks / name).read_bytes(), text.encode())
                    self.assertTrue(os.access(hooks / name, os.X_OK))
                self.assertEqual((hooks / 'post-checkout').read_text(), stock_lfs_hook('post-checkout'))

    def test_install_from_a_linked_worktree_writes_the_shared_hooks(self):
        root = self.repo()
        ci.git('commit', '-q', '--allow-empty', '-m', 'first', cwd=root)
        ci.git('worktree', 'add', '-q', str(self.folder / 'linked'), cwd=root)
        with patch.object(ci, 'ROOT', self.folder / 'linked'):
            self.assertTrue(self.install())
        self.assertEqual((root / '.git/hooks/pre-commit').read_bytes(), ci.HOOKS['pre-commit'].encode())

    def test_install_changes_nothing_over_another_hook(self):
        for name, text in (('pre-commit', '#!/bin/sh\nnpx nano-staged\n'),
                           ('pre-commit', '#!/bin/sh\n./lint.sh && exec python3 tooling/ci-local.py pre-commit\n'),
                           ('pre-push', '#!/bin/sh\n./check.sh\ngit lfs pre-push "$@"\n'),
                           ('pre-push', '#!/bin/sh\ncommand -v git-lfs >/dev/null || exit 2; ./lint.sh\n'
                                        'git lfs pre-push "$@"\n'),
                           ('pre-push', '#!/bin/sh\n./lint.sh || { echo failed; exit 2; }\ngit lfs pre-push "$@"\n'),
                           ('pre-push', '#!/bin/sh\ncommand -v git-lfs >/dev/null 2>&1 && ./lint.sh || { echo x; exit 2; }\n'
                                        'git lfs pre-push "$@"\n'),
                           ('pre-push', stock_lfs_hook('pre-push').replace('#!/bin/sh', '#!/usr/bin/env bash')),
                           ('pre-push', stock_lfs_hook('pre-push') + './extra.sh\n'),
                           ('pre-push', stock_lfs_hook('pre-push').replace('git lfs pre-push', './other.sh')),
                           ('pre-push', '#!/bin/sh\ncommand -v git-lfs >/dev/null 2>&1 || { :; }; ./check.sh; { exit 2; }\n'
                                        'git lfs pre-push "$@"\n'),
                           *(('pre-push', hook('pre-push').replace('This repository', injected))
                             for hook in (stock_lfs_hook, old_lfs_hook)
                             for injected in ('$(./check.sh)', '`./check.sh`', '"; ./check.sh; echo "')),
                           ('pre-push', stock_lfs_hook('pre-push').replace('exit 2; }', 'exit 2; }; ./check.sh')),
                           ('pre-commit', None)):
            with self.subTest(hook=name, text=text):
                root = self.repo()
                hooks = root / '.git/hooks'
                ci.git('config', 'core.hooksPath', '.githooks', cwd=root)
                if text is None:
                    (hooks / name).symlink_to(self.folder / 'missing')
                else:
                    (hooks / name).write_text(text)
                self.assertFalse(self.install())
                if text is None:
                    self.assertTrue((hooks / name).is_symlink())
                else:
                    self.assertEqual((hooks / name).read_text(), text)
                self.assertEqual([path.name for path in hooks.iterdir() if path.name in ci.HOOKS], [name])
                self.assertEqual(self.hooks_path(root), '.githooks')

    def test_install_refuses_a_hooks_path_it_did_not_set(self):
        extra = self.folder / 'extra.gitconfig'
        extra.write_text('[core]\n\thooksPath = .githooks\n')
        for scope, key, value in (('local', 'core.hooksPath', '.husky'),
                                  ('global', 'core.hooksPath', str(self.folder / 'elsewhere')),
                                  ('global', 'core.hooksPath', ''),
                                  ('local', 'core.hooksPath', ''),
                                  ('local', 'include.path', str(extra))):
            with self.subTest(scope=scope, key=key, value=value):
                root = self.repo()
                ci.git('config', f'--{scope}', key, value, cwd=root)
                before = self.hooks_path(root)
                self.assertFalse(self.install())
                self.assertFalse((root / '.git/hooks/pre-commit').exists())
                self.assertEqual(self.hooks_path(root), before)
                ci.git('config', f'--{scope}', '--unset', key, cwd=root)

    def test_install_needs_a_python3_that_runs(self):
        run = subprocess.run
        for failure in (subprocess.CompletedProcess(['python3'], 9009), FileNotFoundError('python3')):
            def fake(command, *args, **kwargs):
                if command[0] != 'python3':
                    return run(command, *args, **kwargs)
                if isinstance(failure, OSError):
                    raise failure
                return failure
            with self.subTest(failure=failure):
                root = self.repo()
                with patch('subprocess.run', side_effect=fake):
                    self.assertFalse(self.install())
                self.assertFalse((root / '.git/hooks/pre-commit').exists())

    def test_install_keeps_githooks_hooks_running_and_names_the_shared_ones_it_turns_back_on(self):
        for tree, entry in (('main', 'commit-msg'), ('linked', 'post-commit'), ('main', 'lib')):
            with self.subTest(tree=tree, entry=entry):
                root = self.repo()
                ci.git('commit', '-q', '--allow-empty', '-m', 'first', cwd=root)
                linked = Path(tempfile.mkdtemp(dir=self.folder)) / 'linked'
                ci.git('worktree', 'add', '-q', str(linked), cwd=root)
                ci.git('config', 'core.hooksPath', '.githooks', cwd=root)
                shared = root / '.git/hooks'
                for folder in (root / '.githooks', linked / '.githooks', shared):
                    folder.mkdir(exist_ok=True)
                    (folder / 'post-commit').write_text(stock_lfs_hook('post-commit'))
                    (folder / 'post-merge').write_text(old_lfs_hook('post-merge'))
                (shared / 'commit-msg.sample').write_text('#!/bin/sh\nexit 1\n')
                (shared / 'commit-msg').write_text('#!/bin/sh\n./check.sh "$1"\n')
                custom = (root if tree == 'main' else linked) / '.githooks' / entry
                if entry == 'lib':
                    custom.mkdir()
                else:
                    custom.write_text('#!/bin/sh\n./check.sh "$1"\n')
                self.assertFalse(self.install())
                self.assertIn(str(custom), self.output)
                self.assertNotIn(str(shared / 'commit-msg'), self.output)
                self.assertTrue(custom.exists())
                self.assertFalse((shared / 'pre-commit').exists())
                self.assertEqual(self.hooks_path(root), '.githooks')
                custom.rmdir() if entry == 'lib' else custom.unlink()
                self.assertTrue(self.install())
                self.assertEqual(self.hooks_path(root), '')
                self.assertIn(f'runs these hooks again: {shared / "commit-msg"}', self.output)
                for name in ('post-commit', 'post-merge', 'commit-msg.sample'):
                    self.assertNotIn(str(shared / name), self.output)

    def test_the_installed_hooks_run_ci_local_or_git_lfs_and_need_git_lfs(self):
        root = self.repo()
        (root / 'tooling').mkdir()
        for name in ('ci-local.py', 'ci_paths.py'):
            shutil.copy(Path(__file__).parents[1] / name, root / 'tooling' / name)
        self.assertTrue(self.install())
        stub, log, remote = self.folder / 'bin', self.folder / 'lfs.log', self.folder / 'remote.git'
        stub.mkdir()
        (stub / 'git-lfs').write_text('#!/bin/sh\n{ printf "[%s]" "$@"; echo; cat; } >> "$LFS_LOG"\nexit "${LFS_EXIT:-0}"\n')
        (stub / 'git-lfs').chmod(0o755)
        ci.git('init', '-q', '--bare', str(remote), cwd=root)
        ci.git('remote', 'add', 'origin', str(remote), cwd=root)
        environment = {**os.environ, 'PATH': f'{stub}{os.pathsep}{os.environ["PATH"]}', 'LFS_LOG': str(log),
                       'CI_LOCAL_SKIP': '1'}
        def git(*args, **extra):
            return subprocess.run(['git', *args], cwd=root, env={**environment, **extra}, capture_output=True, text=True)
        def pushed(old, new):
            return f'[pre-push][origin][{remote}]\nrefs/heads/main {new} refs/heads/main {old}\n'
        def refused_without_git_lfs():
            result = subprocess.run([str(root / '.git/hooks/pre-push'), 'origin', str(remote)], cwd=root, input='',
                                    env={**environment, 'PATH': str(self.folder / 'no-tools')}, capture_output=True,
                                    text=True)
            self.assertEqual(result.returncode, 2)
            self.assertIn('git-lfs is not on PATH', result.stderr)
        skipped = 'CI_LOCAL_SKIP=1, checks skipped'
        ci.git('add', 'tooling', cwd=root)
        result = git('commit', '-q', '-m', 'first')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(skipped, result.stderr)
        first = ci.git('rev-parse', 'HEAD', cwd=root)
        result = git('push', '-q', 'origin', 'main', LFS_EXIT='1')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(skipped, result.stderr)
        self.assertEqual(ci.git('rev-parse', '--verify', '-q', 'main', cwd=remote, check=False), '')
        result = git('push', '-q', 'origin', 'main')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(log.read_text(), pushed(ci.ZERO, first) * 2)
        refused_without_git_lfs()
        ci.git('rm', '-q', 'tooling/ci-local.py', cwd=root)
        result = git('commit', '-q', '-m', 'second')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn(skipped, result.stderr)
        second = ci.git('rev-parse', 'HEAD', cwd=root)
        result = git('push', '-q', 'origin', 'main')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn(skipped, result.stderr)
        self.assertEqual(log.read_text(), pushed(ci.ZERO, first) * 2 + pushed(first, second))
        self.assertEqual(ci.git('rev-parse', 'main', cwd=remote), second)
        refused_without_git_lfs()

    def pre_push(self, line, passed=True):
        uploads = []
        run = subprocess.run
        def fake(command, *args, **kwargs):
            if command[:3] == ['git', 'lfs', 'pre-push']:
                uploads.append((command[3:], kwargs.get('input')))
                return subprocess.CompletedProcess(command, 0)
            return run(command, *args, **kwargs)
        with patch('subprocess.run', side_effect=fake), \
                patch.object(ci, 'check_commit', return_value=passed) as check, \
                patch.object(ci, 'changes_for_push', return_value=set()), \
                patch('sys.stdin', io.StringIO(line)), patch('builtins.print'):
            ok = ci.pre_push(['origin', 'https://example.invalid/repo.git'])
        return ok, check, uploads

    def test_pre_push_checks_each_pushed_commit_once_and_uploads_only_after_the_checks_pass(self):
        root = self.repo()
        ci.git('commit', '-q', '--allow-empty', '-m', 'first', cwd=root)
        ci.git('tag', '-a', 'v1', '-m', 'v1', cwd=root)
        tag, commit = ci.git('rev-parse', 'v1', cwd=root), ci.git('rev-parse', 'v1^{commit}', cwd=root)
        self.assertNotEqual(tag, commit)
        line = (f'(delete) {ci.ZERO} refs/heads/old {commit}\nrefs/tags/v1 {tag} refs/tags/v1 {ci.ZERO}\n'
                f'refs/heads/main {commit} refs/heads/main {ci.ZERO}\n')
        ok, check, uploads = self.pre_push(line)
        self.assertTrue(ok)
        check.assert_called_once_with(commit, set(), f'pre-push {commit[:10]}')
        self.assertEqual(uploads, [(['origin', 'https://example.invalid/repo.git'], line)])
        ok, check, uploads = self.pre_push(line, passed=False)
        self.assertFalse(ok)
        self.assertEqual(uploads, [])

    def test_ci_local_skip_skips_the_pre_commit_checks(self):
        with patch.dict(os.environ, {'CI_LOCAL_SKIP': '1'}), patch.object(ci, 'locked') as locked, \
                patch('sys.stderr', io.StringIO()):
            self.assertTrue(ci.pre_commit())
        locked.assert_not_called()

    def test_clean_env_keeps_lfs_and_config_settings_and_drops_what_the_hooks_git_sets(self):
        hook = {'GIT_DIR': '/hook/.git', 'GIT_INDEX_FILE': '/hook/index', 'GIT_EXEC_PATH': '/hook/libexec',
                'GIT_AUTHOR_NAME': 'someone', 'GIT_EDITOR': ':'}
        with patch.dict(os.environ, {**hook, 'GIT_LFS_SKIP_SMUDGE': '1', 'GIT_CONFIG_SYSTEM': '/dev/null'}):
            env = ci.clean_env()
        self.assertEqual(env['GIT_LFS_SKIP_SMUDGE'], '1')
        self.assertEqual(env['GIT_CONFIG_SYSTEM'], '/dev/null')
        self.assertEqual(env['GIT_CONFIG_GLOBAL'], os.environ['GIT_CONFIG_GLOBAL'])
        self.assertFalse(set(hook) & set(env))


if __name__ == '__main__':
    unittest.main()
