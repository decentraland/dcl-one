"""Engine selection uses committed inputs; publication requires explicit producers."""
import importlib.util
import fnmatch
import shlex
import re
import json
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest
from unittest.mock import patch
from hud_manifest_fixture import seal, SHARED

SPEC = importlib.util.spec_from_file_location('bevy_ci', Path(__file__).parents[1] / 'bevy-ci.py')
ci = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ci)


def source_run(**changes):
    return {'id': 42, 'status': 'completed', 'conclusion': 'success', 'head_branch': 'main',
            'event': 'push', 'head_repository': {'full_name': 'example/project'},
            'path': ci.WORKFLOW, 'head_sha': 'a' * 40, **changes}


def producers(outcome='success'):
    return [{'name': name, 'status': 'completed', 'conclusion': outcome} for name in ci.PRODUCERS.values()]


def path_packages(source):
    """{package name: directory} of each manifest the workspace at `source` reaches: its members and
    every path that a dependency, a workspace dependency or a patch names, followed in turn."""
    workspace = tomllib.loads((source / 'Cargo.toml').read_text(encoding='utf-8'))['workspace']
    queue = [source, *(path for member in workspace.get('members', ()) for path in source.glob(member))]
    found, seen = {}, set()
    while queue:
        directory = queue.pop().resolve()
        if directory in seen or not (directory / 'Cargo.toml').is_file():
            continue
        seen.add(directory)
        manifest = tomllib.loads((directory / 'Cargo.toml').read_text(encoding='utf-8'))
        if 'package' in manifest:
            found[manifest['package']['name']] = directory
        tables = [manifest.get('workspace', {}).get('dependencies', {}), *manifest.get('patch', {}).values()]
        for section in (manifest, *manifest.get('target', {}).values()):
            tables += [section.get(kind, {}) for kind in ('dependencies', 'dev-dependencies', 'build-dependencies')]
        queue += [directory / spec['path'] for table in tables for spec in table.values()
                  if isinstance(spec, dict) and 'path' in spec]
    return found


def locked_path_dependencies(source, crate, root):
    """The directories, relative to `root`, of `crate` and of each package without a registry or git
    source that it reaches in `source`'s Cargo.lock: its path dependencies, for every target and
    feature the lock resolved, a path patch included."""
    packages = tomllib.loads((source / 'Cargo.lock').read_text(encoding='utf-8'))['package']

    def locked(dependency):
        # Cargo.lock names a dependency `name`, `name version` or `name version (source)`.
        name, *rest = dependency.split(' ', 2)
        match = [package for package in packages if package['name'] == name
                 and rest[:1] in ([], [package['version']]) and rest[1:] in ([], [f'({package.get("source")})'])]
        assert len(match) == 1, dependency
        return match[0]

    reached, queue = set(), [locked(crate)]
    while queue:
        package = queue.pop()
        if 'source' not in package and package['name'] not in reached:
            reached.add(package['name'])
            queue += map(locked, package.get('dependencies', ()))
    directories = path_packages(source)
    assert reached <= directories.keys(), f'no manifest found for {sorted(reached - directories.keys())}'
    return {directories[name].relative_to(root).as_posix() + '/' for name in reached}


class WorkflowTest(unittest.TestCase):

    def test_avatar_preview_boot_contract_is_selected_for_its_inputs(self):
        workflow = (ci.ROOT / ci.WORKFLOW).read_text()
        patterns = re.findall(r"^ +- '?([^'\n]+)'?$", workflow, re.M)
        for path in (
            'rendering/avatar-preview/web/preview.js',
            'rendering/avatar-preview/build.sh',
            'rendering/avatar-preview/tools/preview-packaging.test.mjs',
            'rendering/avatar-preview/Cargo.toml',
            'rendering/bevy-explorer/source/Cargo.toml',
            'rendering/avatar-preview/tools/preview-boot.test.mjs',
            'rendering/bevy-explorer/source/deploy/web/engine/content_cache.js',
        ):
            with self.subTest(path=path):
                self.assertTrue(any(fnmatch.fnmatchcase(path, pattern) for pattern in patterns))
        source = workflow.split('\n  source:\n', 1)[1].split('\n  changes:\n', 1)[0]
        self.assertIn(
            'run: node --experimental-vm-modules --test rendering/avatar-preview/tools/preview-*.test.mjs',
            source,
        )

    def test_preview_patches_match_the_explorer_pins(self):
        preview = tomllib.loads((ci.ROOT / 'rendering/avatar-preview/Cargo.toml').read_text())
        explorer = tomllib.loads((ci.ROOT / 'rendering/bevy-explorer/source/Cargo.toml').read_text())
        for source, crates in preview['patch'].items():
            for name, pin in crates.items():
                with self.subTest(source=source, crate=name):
                    expected = explorer['patch'][source][name]
                    self.assertEqual(
                        (pin.get('git'), pin.get('rev')),
                        (expected.get('git'), expected.get('rev')),
                    )
        # Cargo writes unused patches to the lockfile in an order that changes between runs, so a
        # --locked build of the preview fails now and then: it repeats only the pins it uses.
        lock = (ci.ROOT / 'rendering/avatar-preview/Cargo.lock').read_text()
        self.assertNotIn('[[patch.unused]]', lock)

    @staticmethod
    def upstream_commit_cache(job):
        """The key command of a job that restores the upstream commits, and the directory, key and fallback of the restore."""
        command = re.search(r'- name: Identify the upstream commits\n(?: +(?:id|if|working-directory): .+\n)* +run: (.+)\n', job)
        restore = re.search(r'uses: actions/cache/restore@\S+ # v[\d.]+\n +with:\n +path: (.+)\n +key: (.+)\n +restore-keys: \|\n +(.+)\n', job)
        return (command.group(1), *restore.groups())

    def test_nix_play_tooling_dependencies_are_watched_by_image_selection(self):
        contract = (ci.ROOT / 'infra/packages/play.nix').read_text()
        paths = set(re.findall(r'/tooling/[a-zA-Z0-9_-]+\.py', contract))
        self.assertTrue(paths)
        for path in paths:
            with self.subTest(path=path):
                self.assertTrue(any(fnmatch.fnmatchcase(path.lstrip('/'), pattern) for pattern in ci.IMAGES_INPUTS))

    def test_every_path_the_play_contract_reads_is_watched_by_image_selection(self):
        # The Sites image carries the Explorer build play.nix verifies, the avatar preview among it: a change
        # to any path that build is checked against has to rebuild the image, or the old build keeps serving.
        contract = (ci.ROOT / 'infra/packages/play.nix').read_text()
        listed = contract.split('lib.fileset.maybeMissing (root + p)) [', 1)[1].split(']', 1)[0]
        paths = re.findall(r'(?<![\w.-])/([\w.-][\w./-]*)', listed)
        self.assertIn('rendering/avatar-preview', paths)
        for path in paths:
            with self.subTest(path=path):
                self.assertTrue(any(fnmatch.fnmatchcase(candidate, pattern)
                                    for candidate in (path, path + '/file') for pattern in ci.IMAGES_INPUTS))

    def test_workflow_filters_watch_the_in_tree_crates_and_only_this_workflow_watches_the_patch_tool(self):
        def watched(name, path):
            patterns = re.findall(r"^ +- '?([^'\n]+)'?$", (ci.ROOT / name).read_text(), re.M)
            return any(fnmatch.fnmatchcase(path, pattern) for pattern in patterns)
        tool, crate = 'tooling/patched-sources.py', 'rendering/bevy-explorer/vendor/bevy_dui/Cargo.toml'
        # The guards, the Changes job and the Test Suite of this workflow run the tool on the series.
        for path in (tool, crate, 'rendering/bevy-explorer/patches/bevy/source.toml',
                     'rendering/bevy-explorer/patches/bevy/series/0001-scene-spawner.patch'):
            with self.subTest(path=path):
                self.assertTrue(watched(ci.WORKFLOW, path))
        # The scene and end-to-end workflows compile the in-tree crates and take each patched source
        # by the revision that Cargo.lock names: no step of them runs the tool.
        for name in (ci.SCENES_WORKFLOW, '.github/workflows/bevy-explorer-e2e.yml'):
            with self.subTest(workflow=name):
                self.assertTrue(watched(name, crate))
                self.assertFalse(watched(name, tool))

    def test_source_job_runs_the_online_guards_for_what_changed_and_the_daily_run_proves_every_series(self):
        workflow = (ci.ROOT / ci.WORKFLOW).read_text()
        source = workflow.split('\n  source:\n', 1)[1].split('\n  changes:\n', 1)[0]
        daily = (ci.ROOT / '.github/workflows/bevy-explorer-upstream.yml').read_text()
        self.assertIn('run: python3 tooling/patched-sources.py verify\n', daily)
        # The names reach the shell through the environment, and the tool reads the restored directory.
        self.assertIn("        if: steps.guards.outputs.verify != ''\n"
                      '        env:\n'
                      '          DCL_PATCHED_SOURCES_CACHE: ${{ github.workspace }}/.cache/patched-sources\n'
                      '          NAMES: ${{ steps.guards.outputs.verify }}\n'
                      '        run: python3 tooling/patched-sources.py verify $NAMES\n', source)
        # The job reads the cache that the daily run saves, with the same key and directory.
        self.assertEqual(self.upstream_commit_cache(source), self.upstream_commit_cache(daily))
        guards, restore, verify = (source.index(step) for step in (
            'run: python3 tooling/bevy-ci.py guards-plan', 'actions/cache/restore@', 'patched-sources.py verify $NAMES'))
        self.assertLess(guards, restore)
        self.assertLess(restore, verify)
        for step in ("python3 -m unittest discover -s tooling/tests -p 'test_upstream_*.py' -v",
                     'run: python3 tooling/bevy-ci.py guards-plan',
                     "if: steps.guards.outputs.pins != ''",
                     'run: python3 tooling/upstream-drift.py check $PINS',
                     "if: steps.guards.outputs.provenance == 'true'\n        run: python3 tooling/vendor-provenance.py"):
            self.assertIn(step, source)
        # A change to only a pin, the upstream check, its tests or a provenance tool starts the run.
        for event, end in (('push', 'pull_request'),):
            block = workflow.split('  ' + event + ':', 1)[1].split('  ' + end + ':', 1)[0]
            patterns = [line.strip()[2:] for line in block.splitlines() if line.strip().startswith('- ')]
            for name in ('rendering/bevy-explorer/patches/winit/source.toml', 'tooling/upstream-drift.py',
                         'tooling/upstream-drift-site.py', 'tooling/tests/test_upstream_drift.py',
                         'tooling/vendor-provenance.py', 'tooling/enet-provenance.py'):
                with self.subTest(event=event, path=name):
                    self.assertTrue(any(fnmatch.fnmatchcase(name, pattern) for pattern in patterns))

    def test_daily_upstream_run_checks_every_pin_and_every_in_tree_crate(self):
        daily = (ci.ROOT / '.github/workflows/bevy-explorer-upstream.yml').read_text()
        self.assertIn('run: python3 tooling/upstream-drift.py check | tee -a "$GITHUB_STEP_SUMMARY"\n', daily)
        self.assertIn('run: python3 tooling/vendor-provenance.py\n', daily)
        # The directory mode moved out of the ENet tool; no workflow or action may still call it.
        for path in sorted((ci.ROOT / '.github').rglob('*.yml')):
            self.assertNotRegex(path.read_text(encoding='utf-8'), r'enet-provenance\.py[ \t]+\S', path.name)

    def test_daily_upstream_run_restores_the_newest_bases_and_saves_them_after_a_proof(self):
        daily = (ci.ROOT / '.github/workflows/bevy-explorer-upstream.yml').read_text()
        # The key names no system and the directory is in the workspace, where the tool is told to
        # look: a job on another system restores the same archive to the same relative path.
        key = re.search(r'^ +key: (bevy-explorer-patch-bases-)\$\{\{ steps\.bases\.outputs\.key \}\}$', daily, re.M)
        self.assertIsNotNone(key)
        self.assertIn(f'restore-keys: |\n            {key.group(1)}\n', daily)
        self.assertEqual(re.findall(r'^ +path: (.+)$', daily.split('- name: Upload the drift site')[0], re.M),
                         ['.cache/patched-sources'] * 2)
        self.assertIn('      DCL_PATCHED_SOURCES_CACHE: ${{ github.workspace }}/.cache/patched-sources\n',
                      daily.split('    steps:\n')[0])
        restore, verify, save = (daily.index(step) for step in ('actions/cache/restore@', 'patched-sources.py verify',
                                                                 'actions/cache/save@'))
        self.assertLess(restore, verify)
        self.assertLess(verify, save)
        self.assertIn("if: ${{ !cancelled() && steps.verify.outcome == 'success' && steps.restore.outputs.cache-hit != 'true' "
                      "&& github.ref == 'refs/heads/main' }}\n", daily)

    def test_the_daily_upstream_run_only_reads_the_repository(self):
        # Publishing a revision writes a ref. The daily run only reads the repository: the publish-sources workflow
        # publishes on a push to main. The one permission to write is for issues, on the job that reports a failure.
        daily = (ci.ROOT / '.github/workflows/bevy-explorer-upstream.yml').read_text()
        self.assertEqual(re.findall(r'^permissions:\n((?:  .+\n)+)', daily, re.M), ['  contents: read\n'])
        jobs = dict(re.findall(r'(?ms)^  ([\w-]+):\n(.*?)(?=^  [\w-]+:\n|\Z)', daily.split('\njobs:\n', 1)[1]))
        self.assertEqual({name: re.findall(r'^    permissions:\n((?:      .+\n)+)', job, re.M) for name, job in jobs.items()},
                         {'drift': [], 'report': ['      issues: write\n']})

    def test_a_failed_daily_run_on_main_opens_one_issue_or_adds_a_comment_to_it(self):
        daily = (ci.ROOT / '.github/workflows/bevy-explorer-upstream.yml').read_text()
        report = daily.split('\n  report:\n', 1)[1]
        # A cancelled run and a run of another branch report nothing.
        self.assertIn("    needs: drift\n    if: ${{ failure() && github.ref == 'refs/heads/main' }}\n", report)
        self.assertIn('          RUN_URL: ${{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }}\n', report)
        # A failed search stops the step: it does not open a second issue.
        self.assertIn('        shell: bash\n', report)
        title = re.search(r"^          TITLE: '(.+)'$", report, re.M).group(1)
        script = re.sub(r'(?m)^ {10}', '', report.split('        run: |\n', 1)[1])
        self.assertNotIn('${{', script)
        with tempfile.TemporaryDirectory() as directory:
            calls, stub = Path(directory, 'calls'), Path(directory, 'gh')
            # Records each call; the search prints the numbers of the open issues with the title.
            stub.write_text('#!/bin/sh\nprintf \'%s\\n\' "$*" >> "$CALLS"\nif [ "$2" = list ]; then printf %s "$FOUND"; fi\n')
            stub.chmod(0o755)

            def run(found):
                calls.write_text('')
                env = {**os.environ, 'PATH': directory + os.pathsep + os.environ['PATH'], 'CALLS': str(calls),
                       'FOUND': found, 'TITLE': title, 'RUN_URL': 'https://example.test/actions/runs/7'}
                subprocess.run(['bash', '--noprofile', '--norc', '-eo', 'pipefail', '-c', script], env=env, check=True)
                search, *writes = calls.read_text().splitlines()
                self.assertIn(f'issue list --state open --search in:title "{title}" ', search)
                return writes

            body = '--body The daily run of the dependency drift workflow failed: https://example.test/actions/runs/7'
            self.assertEqual(run(''), [f'issue create --title {title} {body}'])
            self.assertEqual(run('41\n40\n'), [f'issue comment 41 {body}'])

    def test_the_pinned_revisions_are_checked_before_any_build_job_starts(self):
        workflow = (ci.ROOT / ci.WORKFLOW).read_text(encoding='utf-8')
        jobs = dict(re.findall(r'(?ms)^  ([\w-]+):\n(.*?)(?=^  [\w-]+:\n|\Z)', workflow.split('\njobs:\n', 1)[1]))
        # Offline and fast. Every build job needs `changes`, so none starts when a revision in the
        # manifest or the lock is not the one a source.toml records.
        plan = jobs['changes']
        pins = plan.index('        run: python3 tooling/patched-sources.py pins\n')
        self.assertLess(plan.index('actions/checkout@'), pins)
        self.assertLess(pins, plan.index('- name: Select engine builds from their inputs'))
        step = plan[plan.rfind('- name:', 0, pins):pins]
        self.assertNotIn('if:', step)
        self.assertNotIn('continue-on-error', plan)
        # Cargo fetches each revision from this repository by id, and GitHub serves a fork's commits through
        # it. A revision that neither has fails here, and the error names the command that pushes it to a fork.
        published = plan.index('          python3 tooling/patched-sources.py publish --check --fetchable || {\n')
        self.assertLess(pins, published)
        select = plan.index('- name: Select engine builds from their inputs')
        self.assertLess(published, select)
        step = plan[plan.rfind('- name:', 0, published):select]
        self.assertNotIn('if:', step)
        self.assertRegex(step, r"echo '::error::.+`python3 tooling/patched-sources\.py publish --remote [^`]+`\.'\n +exit 1\n")
        # The plan job's default runner image has the Python the tool needs.
        self.assertRegex(plan, r"(?m)^    runs-on: .*'ubuntu-26\.04'")

    def test_a_push_to_main_publishes_the_pinned_revisions(self):
        workflow = (ci.ROOT / ci.WORKFLOW).read_text(encoding='utf-8')
        # The CI workflow only reads the repository: its pending runs on main can be cancelled.
        self.assertNotIn('contents: write', workflow)
        publish = (ci.ROOT / '.github/workflows/bevy-explorer-publish-sources.yml').read_text()
        # One run for each push to main that changes a source, and none is cancelled: a cancelled run would leave
        # the `rev` of its push without a ref.
        self.assertIn("on:\n  push:\n    branches:\n      - main\n    paths:\n      - rendering/bevy-explorer/patches/**\n", publish)
        self.assertNotIn('concurrency', publish.replace('# No concurrency group', ''))
        self.assertEqual(re.findall(r'^permissions:\n((?:  .+\n)+)', publish, re.M), ['  contents: read\n'])
        jobs = dict(re.findall(r'(?ms)^  ([\w-]+):\n(.*?)(?=^  [\w-]+:\n|\Z)', publish.split('\njobs:\n', 1)[1]))
        self.assertEqual(list(jobs), ['publish'])
        job = jobs['publish']
        self.assertIn("    if: github.ref == 'refs/heads/main'\n", job)
        self.assertEqual(re.findall(r'^    permissions:\n((?:      .+\n)+)', job, re.M), ['      contents: write\n'])
        # The token is in the environment of the publish step alone, not in the checkout's configuration.
        self.assertIn('          persist-credentials: false\n', job)
        self.assertEqual(job.count('github.token'), 1)
        self.assertNotIn('secrets.', job)
        # It rebuilds from the upstream commits that the daily run caches: the same key and directory.
        daily = (ci.ROOT / '.github/workflows/bevy-explorer-upstream.yml').read_text()
        self.assertEqual(self.upstream_commit_cache(job), self.upstream_commit_cache(daily))
        self.assertIn('      DCL_PATCHED_SOURCES_CACHE: ${{ github.workspace }}/.cache/patched-sources\n', job)
        run = job.index('python3 tooling/patched-sources.py publish\n')
        self.assertLess(job.index('actions/cache/restore@'), run)
        self.assertNotIn('--remote', job)
        self.assertNotIn('--check', job)

    def test_only_the_test_suite_and_clippy_rebuild_a_tree_and_only_the_bevy_one(self):
        workflow = (ci.ROOT / ci.WORKFLOW).read_text(encoding='utf-8')
        jobs = dict(re.findall(r'(?ms)^  ([\w-]+):\n(.*?)(?=^  [\w-]+:\n|\Z)', workflow.split('\njobs:\n', 1)[1]))
        self.assertEqual([name for name, job in jobs.items() if 'materialise' in job], ['test', 'clippy'])
        # Clippy compiles `common`'s tests, which include a shader of the Bevy tree.
        clippy = jobs['clippy']
        self.assertIn('        run: python3 "$GITHUB_WORKSPACE/tooling/patched-sources.py" materialise bevy\n', clippy)
        # Written before the file times are restored, so an unchanged tree keeps its cached times.
        self.assertLess(clippy.index('patched-sources.py" materialise bevy'), clippy.index('bevy-mtimes.py" restore'))
        # It reads the same cache as the Test Suite: the key from the root, restored before the rebuild.
        self.assertIn('        id: bases\n        working-directory: .\n', clippy)
        self.assertIn('        env:\n'
                      '          DCL_PATCHED_SOURCES_CACHE: ${{ github.workspace }}/.cache/patched-sources\n'
                      '        run: python3 "$GITHUB_WORKSPACE/tooling/patched-sources.py" materialise bevy\n', clippy)
        self.assertLess(clippy.index('actions/cache/restore@', clippy.index('- name: Restore the upstream commits\n')),
                        clippy.index('patched-sources.py" materialise bevy'))
        job = jobs['test']
        python, rebuild, tests = (job.index(step) for step in (
            "python-version: '3.12'\n", 'run: python "$GITHUB_WORKSPACE/tooling/patched-sources.py" materialise bevy\n',
            '- name: Test the patched Bevy crates\n'))
        # The tool reads TOML with the standard library of Python 3.11; not every runner image has it.
        self.assertLess(python, rebuild)
        self.assertLess(rebuild, tests)
        # The upstream commit comes from the cache that the daily run saves on Linux: the same key
        # and directory, read where the tool looks, on each system of the matrix.
        daily = (ci.ROOT / '.github/workflows/bevy-explorer-upstream.yml').read_text()
        self.assertEqual(self.upstream_commit_cache(job), self.upstream_commit_cache(daily))
        self.assertEqual(self.upstream_commit_cache(jobs['clippy']), self.upstream_commit_cache(daily))
        restore = job.index('actions/cache/restore@', job.index('- name: Restore the upstream commits\n'))
        self.assertLess(restore, rebuild)
        step = job[job.rfind('- name:', 0, restore):job.rfind('- name:', 0, rebuild)]
        self.assertNotIn('if:', step)
        self.assertIn('          enableCrossOsArchive: true\n', step)
        self.assertIn('        env:\n'
                      '          DCL_PATCHED_SOURCES_CACHE: ${{ github.workspace }}/.cache/patched-sources\n'
                      '        run: python "$GITHUB_WORKSPACE/tooling/patched-sources.py" materialise bevy\n', job)
        # The job's commands run in the Explorer workspace; the key command reads paths from the root.
        self.assertIn('        id: bases\n        working-directory: .\n', job)
        # Only the daily run saves the directory: each step of this workflow that names it restores it.
        self.assertEqual(re.findall(r'uses: (actions/cache\S*)@\S+ # v[\d.]+\n +with:\n +path: \.cache/patched-sources\n', workflow),
                         ['actions/cache/restore'] * 3)
        # The rebuilt workspace has no lock file: it starts from the explorer's.
        self.assertIn('          cp Cargo.lock ../bevy/Cargo.lock\n          cargo test --manifest-path ../bevy/Cargo.toml ', job[tests:])

    def test_the_windows_ffmpeg_archive_is_accepted_only_with_its_pinned_digest(self):
        # The desktop package ships the libraries of this archive, and a release tag can get a new file.
        pins = set()
        for name in ('bevy-explorer-ci.yml', 'bevy-explorer-desktop.yml'):
            workflow = (ci.ROOT / '.github/workflows' / name).read_text()
            step = workflow.split('$ffUrl = ', 1)[1].split('shell: pwsh', 1)[0]
            with self.subTest(workflow=name):
                self.assertEqual(workflow.count('$ffUrl = '), 1)
                pin = re.search(r'\$ffSha256 = "([0-9a-f]{64})"\n', step)
                self.assertIsNotNone(pin)
                pins.add(pin.group(1))
                check = step.index('if ($ffActual -ne $ffSha256) { throw ')
                self.assertLess(step.index('Invoke-WebRequest $ffUrl'), step.index('Get-FileHash ffmpeg-6.0-full_build-shared.7z -Algorithm SHA256'))
                self.assertLess(step.index('Get-FileHash ffmpeg-6.0-full_build-shared.7z -Algorithm SHA256'), check)
                self.assertLess(check, step.index('7z x ffmpeg-6.0-full_build-shared.7z'))
        self.assertEqual(len(pins), 1)

    def test_shared_reader_and_fixture_changes_schedule_both_workflow_events(self):
        workflow = (ci.ROOT / ci.WORKFLOW).read_text()
        for event, end in (('push', 'pull_request'),):
            block = workflow.split('  ' + event + ':', 1)[1].split('  ' + end + ':', 1)[0]
            patterns = [line.strip()[2:] for line in block.splitlines() if line.strip().startswith('- ')]
            for name in ('tooling/hud_shared_inputs.py', 'tooling/tests/test_hud_shared_inputs.py',
                         'tooling/tests/hud_manifest_fixture.py', 'tooling/tests/hud_manifest_invalid_cases.json', 'tooling/tests/check_hud_manifest_nix.py'):
                with self.subTest(event=event, path=name):
                    self.assertTrue(any(fnmatch.fnmatchcase(name, pattern) for pattern in patterns))
        self.assertIn("-p 'test_hud_shared_inputs.py' -v", workflow)

    def test_pure_nix_hud_corpus_runs_after_nix_install_and_before_heavy_builds(self):
        workflow = (ci.ROOT / '.github/workflows/apis-sites-images.yml').read_text()
        install = workflow.index('cachix/install-nix-action@')
        corpus = workflow.index('run: python3 tooling/tests/check_hud_manifest_nix.py')
        build = workflow.index('run: nix build')
        self.assertLess(install, corpus)
        self.assertLess(corpus, build)
        for name in ('tooling/tests/check_hud_manifest_nix.py', 'tooling/tests/hud_manifest_invalid_cases.json',
                     'tooling/tests/hud_manifest_fixture.py', 'infra/packages/hud-shared-inputs.nix',
                     'infra/packages/hud-shared-inputs-corpus.nix'):
            self.assertTrue(any(fnmatch.fnmatchcase(name, pattern) for pattern in ci.IMAGES_INPUTS), name)

    def test_generator_suite_is_a_ci_gate_after_build_and_before_artifact_upload(self):
        workflow = (ci.ROOT / ci.WORKFLOW).read_text()
        job = workflow.split('  build-deploy-web:', 1)[1].split('  headless-packages:', 1)[0]
        build = job.index('run: python3 ../../../apps/bevy-explorer/build-web.py')
        tests = job.index('run: cd react-web && pnpm run test:shared-inputs')
        upload = job.index('- name: Upload the complete web app')
        self.assertLess(build, tests)
        self.assertLess(tests, upload)
        test_step = job[job.rfind('- name:', 0, tests):job.index('- name:', tests)]
        self.assertNotIn('continue-on-error', test_step)
        self.assertNotIn('if:', test_step)

    def test_main_runs_are_not_cancelled_by_the_next_push(self):
        workflow = (ci.ROOT / ci.WORKFLOW).read_text(encoding='utf-8')
        lines = workflow.split('\nconcurrency:\n', 1)[1].splitlines(True)
        # The workflow-level block only: it ends at the next top-level key.
        block = ''.join(lines[:next(i for i, line in enumerate(lines) if line[:1] not in (' ', '#'))])
        self.assertIn("  group: bevy-explorer-ci-${{ github.event.pull_request.number && format('pr-{0}', "
                      "github.event.pull_request.number) || github.ref }}\n", block)
        self.assertIn("  cancel-in-progress: ${{ github.event_name == 'pull_request' || "
                      "github.ref != 'refs/heads/main' }}\n", block)

    def test_the_desktop_build_waits_for_the_proof_of_every_patched_source(self):
        workflow = (ci.ROOT / '.github/workflows/bevy-explorer-desktop.yml').read_text(encoding='utf-8')
        jobs = dict(re.findall(r'(?ms)^  ([\w-]+):\n(.*?)(?=^  [\w-]+:\n|\Z)', workflow.split('\njobs:\n', 1)[1]))
        self.assertIn('run: python3 tooling/patched-sources.py pins\n', jobs['source'])
        self.assertIn('run: python3 tooling/patched-sources.py verify\n', jobs['source'])
        self.assertNotIn('secrets.', jobs['source'])
        self.assertIn('    needs: source\n', jobs['build'])
        self.assertIn('cargo build --release --locked', jobs['build'])

    def test_the_build_jobs_start_beside_the_guards_and_a_failed_check_stops_them(self):
        workflow = (ci.ROOT / ci.WORKFLOW).read_text(encoding='utf-8')
        jobs = dict(re.findall(r'(?ms)^  ([\w-]+):\n(.*?)(?=^  [\w-]+:\n|\Z)', workflow.split('\njobs:\n', 1)[1]))
        closed = "    if: github.event.action != 'closed'\n"
        # A closed PR runs only the cleanup: the guards, the format checks and the plan skip it themselves.
        for name in ('source', 'changes', 'fmt', 'toml'):
            self.assertIn(closed, jobs[name], name)
            self.assertNotIn('needs:', jobs[name], name)

        def needs(name):
            found = re.search(r'(?m)^    needs:(?: \[([^\]]*)\]|((?:\n      - \S+)+))', jobs[name])
            return set(re.findall(r'[A-Za-z][\w-]*', found.group(1) or found.group(2)))

        readers = [name for name, job in jobs.items() if 'needs.changes.' in job]
        self.assertGreaterEqual(len(readers), 6)
        for name in readers:
            self.assertIn('changes', needs(name), name)
        # The build jobs do not wait for the guards; the jobs that package or publish still do.
        for name in ('test', 'headless-smoke', 'clippy', 'build-deploy-web'):
            self.assertEqual(needs(name), {'changes'}, name)
        for name in ('headless-packages', 'images', 'publish-web'):
            self.assertIn('source', needs(name), name)
        # The packages compile beside the web build; only the image assembly waits for it.
        self.assertEqual(needs('images-compile'), {'source', 'changes'})
        self.assertEqual(needs('images'), {'source', 'changes', 'build-deploy-web', 'images-compile'})
        self.assertIn("needs.images-compile.result == 'success'", jobs['images'])
        self.assertIn('    uses: ./.github/workflows/apis-sites-compile.yml\n', jobs['images-compile'])
        self.assertIn('    permissions:\n      contents: read\n    uses:', jobs['images-compile'])
        # The plan reads a finished image build from the jobs of this name; the compile job is not one.
        compile_name = re.search(r'(?m)^    name: (.+)$', jobs['images-compile']).group(1)
        self.assertFalse(any(job.startswith(compile_name + ' / ') for job in ci.IMAGES_JOBS))
        self.assertFalse((compile_name + ' / packages (amd64, ubuntu-26.04)').startswith('APIs + Sites / build ('))
        # The rollout reads the published package and waits for the guards itself.
        self.assertEqual(needs('rollout-web'), {'source', 'publish-web'})
        stop = jobs['stop-builds']
        self.assertEqual(needs('stop-builds'), {'source', 'fmt', 'toml'})
        self.assertIn("    if: failure() && github.event_name == 'pull_request' && "
                      "github.event.pull_request.head.repo.full_name == github.repository\n", stop)
        self.assertIn('    permissions:\n      actions: write\n', stop)
        self.assertIn('gh run cancel "${{ github.run_id }}" --repo "${{ github.repository }}"\n', stop)
        # No other job may cancel a run.
        self.assertEqual(workflow.count('actions: write'), 1)
        self.assertEqual(workflow.count('gh run cancel'), 1)

    def test_the_gate_requires_every_check_build_and_test(self):
        workflow = (ci.ROOT / ci.WORKFLOW).read_text(encoding='utf-8')
        jobs = dict(re.findall(r'(?ms)^  ([\w-]+):\n(.*?)(?=^  [\w-]+:\n|\Z)', workflow.split('\njobs:\n', 1)[1]))
        gate = jobs['gate']
        # A path filter on pull requests would leave a required gate waiting for ever.
        pull_request = workflow.split('\n  pull_request:\n', 1)[1].split('\n  workflow_dispatch:\n', 1)[0]
        self.assertNotIn('paths:', pull_request)
        self.assertIn('    name: Explorer CI gate\n', gate)
        needs = set(re.search(r'(?m)^    needs: \[([^\]]*)\]', gate).group(1).replace(' ', '').split(','))
        unchecked = {'gate', 'stop-builds', 'publish-web', 'rollout-web', 'headless-packages', 'web-pr-target',
                     'preview-cleanup'}
        self.assertEqual(needs, set(jobs) - unchecked)
        self.assertIn('RESULTS: ${{ toJSON(needs) }}', gate)
        # An earlier run's gate stands until the next one reports: a held draft or an unmerged close fails.
        self.assertIn("    if: ${{ always() && !(github.event.action == 'closed' && github.event.pull_request.merged) }}\n",
                      gate)
        self.assertIn('      held: ${{ steps.plan.outputs.held }}\n', jobs['changes'])
        self.assertIn('HELD: ${{ needs.changes.outputs.held }}', gate)
        self.assertIn('if [ "$HELD" = true ]; then', gate)
        self.assertIn('if [ "$ACTION" = closed ]; then', gate)
        development = (ci.ROOT / '.github/workflows/development.yml').read_text(encoding='utf-8')
        self.assertIn('    types: [opened, synchronize, reopened, ready_for_review]\n', development)

    def test_the_headless_smoke_runs_for_thirty_seconds(self):
        workflow = (ci.ROOT / ci.WORKFLOW).read_text(encoding='utf-8')
        job = workflow.split('\n  headless-smoke:\n', 1)[1].split('\n  fmt:\n', 1)[0]
        # Scenes tick within 11 s of the start; the assert step needs a first tick and a later, higher one.
        self.assertIn(' --location 52,-52 --server-mode --preview --timeout 30 ', job)

    def test_a_same_repository_pull_request_publishes_a_preview(self):
        workflow = (ci.ROOT / ci.WORKFLOW).read_text(encoding='utf-8')
        job = workflow.split('\n  build-deploy-web:\n', 1)[1].split('\n  headless-packages:\n', 1)[0]
        self.assertIn("      BEVY_WEB_PUBLISH: ${{ (github.event_name == 'push' && github.ref == 'refs/heads/main') || "
                      "(github.event_name == 'workflow_dispatch' && inputs.publish-tag != '') || "
                      "(github.event_name == 'pull_request' && "
                      "github.event.pull_request.head.repo.full_name == github.repository && "
                      "github.head_ref != 'main' && github.head_ref != 'master') }}\n", job)
        self.assertIn('      publish: ${{ env.BEVY_WEB_PUBLISH }}\n', job)
        # The package job and the publish job read only that answer.
        self.assertIn("      - name: Upload the stamped package for publication\n"
                      "        if: needs.build-deploy-web.outputs.publish == 'true'\n", job)
        self.assertIn("\n  publish-web:\n    if: needs.build-deploy-web.outputs.publish == 'true'\n", workflow)
        # No label gates the preview or its removal.
        self.assertNotIn('ci:preview', workflow)
        cleanup = workflow.split('\n  preview-cleanup:\n', 1)[1].split('\n    name:', 1)[0]
        self.assertNotIn('labels', cleanup)

    def test_pr_web_builds_skip_wasm_opt_only_while_the_benchmark_is_off(self):
        workflow = (ci.ROOT / ci.WORKFLOW).read_text(encoding='utf-8')
        job = workflow.split('\n  build-deploy-web:\n', 1)[1].split('\n  headless-packages:\n', 1)[0]
        self.assertIn('          key=$(python3 tooling/play-artifact.py engine-cache-key)\n'
                      '          echo "key=$key" >> "$GITHUB_OUTPUT"\n', job)
        self.assertIn("      - name: Save tested compiled engine\n"
                      "        if: steps.engine-cache.outputs.cache-matched-key == ''\n", job)
        self.assertIn("      BEVY_WEB_OPT: ${{ github.event_name == 'pull_request' && "
                      "vars.BENCHMARK_ENABLED_RUNNERS == 'none' && 'unoptimized' || 'optimized' }}\n", job)
        self.assertIn('          key: bevy-engine-v1-${{ env.BEVY_WEB_OPT }}-${{ steps.engine-inputs.outputs.key }}\n'
                      '          restore-keys: bevy-engine-v1-optimized-${{ steps.engine-inputs.outputs.key }}\n',
                      job)
        self.assertIn("verify-engine .cache/bevy-engine ${{ env.BEVY_WEB_OPT == 'optimized' && '--production' || '' }}\n",
                      job)
        self.assertIn("build-web.py --engine-cache ../../../.cache/bevy-engine --preview-cache ../../../.cache/avatar-preview "
                      "${{ env.BEVY_WEB_OPT == 'unoptimized' && '--no-opt' || '' }} --shared-build\n", job)
        self.assertIn('          path: .cache/bevy-engine\n'
                      '          key: ${{ steps.engine-cache.outputs.cache-primary-key }}\n', job)

    def test_the_avatar_preview_is_compiled_once_per_input_and_only_main_saves_its_target(self):
        workflow = (ci.ROOT / ci.WORKFLOW).read_text(encoding='utf-8')
        job = workflow.split('\n  build-deploy-web:\n', 1)[1].split('\n  package-web:\n', 1)[0]
        self.assertIn('          key=$(python3 tooling/play-artifact.py preview-inputs)\n', job)
        # As the engine: a PR may take main's optimized preview, main never an unoptimized one.
        self.assertIn('          key: avatar-preview-v1-${{ env.BEVY_WEB_OPT }}-${{ steps.preview-inputs.outputs.key }}\n'
                      '          restore-keys: avatar-preview-v1-optimized-${{ steps.preview-inputs.outputs.key }}\n', job)
        self.assertIn("verify-preview .cache/avatar-preview ${{ env.BEVY_WEB_OPT == 'optimized' && '--production' || '' }}\n",
                      job)
        # The compilers are set up when either the engine or the preview is compiled.
        either = ("(steps.engine-cache.outputs.cache-matched-key == '' || "
                  "steps.preview-cache.outputs.cache-matched-key == '')")
        for step in ('Install the web build tools from the flake', 'Install the pinned WASM binding generator and optimizer',
                     'Stamp rust-src with the toolchain date so cached build-std artifacts stay fresh'):
            with self.subTest(step=step):
                self.assertIn(f'      - name: {step}\n        if: {either}\n', job)
        self.assertIn(f"      - uses: dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8 # master 2026-10-16\n"
                      f"        if: {either}\n", job)
        restore = job.index('Restore the avatar preview target cache')
        times = job.index("bevy-mtimes.py\" restore ../../avatar-preview/target")
        build = job.index('      - name: Build and test the local web app')
        record = job.index("bevy-mtimes.py\" record ../../avatar-preview/target")
        save = job.index('      - name: Save the avatar preview target cache\n'
                         "        if: github.ref == 'refs/heads/main' && ")
        self.assertLess(restore, times)
        self.assertLess(times, build)
        self.assertLess(build, record)
        self.assertLess(record, save)
        self.assertIn('          path: .cache/avatar-preview\n'
                      '          key: ${{ steps.preview-cache.outputs.cache-primary-key }}\n', job)

    def test_pr_web_builds_restore_the_unoptimised_target_that_main_saves(self):
        workflow = (ci.ROOT / ci.WORKFLOW).read_text(encoding='utf-8')
        jobs = dict(re.findall(r'(?ms)^  ([a-z-]+):\n(.*?)(?=^  [a-z-]+:\n|\Z)', workflow.split('\njobs:\n', 1)[1]))
        web, main = jobs['build-deploy-web'], jobs['web-pr-target']
        unoptimised = "steps.engine-cache.outputs.cache-matched-key == '' && env.BEVY_WEB_OPT == 'unoptimized'"

        def step(job, name):
            return job.split(f'      - name: {name}\n', 1)[1].split('\n      - ', 1)[0]

        # Both jobs name the dependencies with one expression, or a PR never finds main's entry.
        hashes = [re.search(r'echo "hash=(\$\{\{ hashFiles\(.*?\) \}\})"', step(job, 'Identify the wasm dependencies'),
                            re.S).group(1) for job in (web, main)]
        self.assertEqual(hashes[0], hashes[1])
        optimised = re.search(r'key: bevy-explorer-web-(\$\{\{ hashFiles\(.*?\) \}\})',
                              step(web, 'Restore wasm target cache'), re.S).group(1)
        self.assertEqual(hashes[0], optimised)
        # The new family must not start with the optimised cache's restore prefix.
        self.assertTrue(step(web, 'Restore wasm target cache').endswith('          restore-keys: bevy-explorer-web-'))
        self.assertNotIn('bevy-explorer-web-unopt', workflow)

        main_key = ('bevy-web-unopt-v1-main-${{ steps.wasm-deps.outputs.hash }}-'
                    '${{ steps.engine-inputs.outputs.key }}')
        main_prefix = 'bevy-web-unopt-v1-main-${{ steps.wasm-deps.outputs.hash }}-'
        restore = step(web, 'Restore the pull request wasm target cache')
        self.assertIn(f'        if: {unoptimised}\n', restore)
        # Main's entries in a step that names no PR entry: GitHub searches the PR's own scope with
        # every key of a step before main's, so a PR entry named here would win over a newer entry
        # of main for the same dependencies.
        self.assertTrue((restore + '\n').endswith(f'          key: {main_key}\n          restore-keys: {main_prefix}\n'))
        self.assertNotIn('bevy-web-unopt-v1-pr-', restore)
        own = step(web, "Restore the pull request's own wasm target cache")
        self.assertIn(f"        if: {unoptimised} && steps.wasm-pr-cache.outputs.cache-matched-key == ''\n", own)
        self.assertTrue((own + '\n').endswith('          key: bevy-web-unopt-v1-pr-${{ steps.wasm-deps.outputs.hash }}\n'
                                              '          restore-keys: bevy-web-unopt-v1-main-\n'))
        save = step(web, 'Save the pull request wasm target cache')
        # Once, when neither main nor the PR had an entry for these dependencies, and never from a fork.
        self.assertIn(f"          {unoptimised} &&\n"
                      "          steps.wasm-pr-cache.outputs.cache-matched-key == '' && "
                      "steps.wasm-own-cache.outputs.cache-hit != 'true' &&\n"
                      "          github.event.pull_request.head.repo.full_name == github.repository\n", save)
        self.assertIn('          key: ${{ steps.wasm-own-cache.outputs.cache-primary-key }}', save)
        # Cargo compares file times in both modes.
        for name, verb in (("Restore the path dependencies' cached file times", 'restore'),
                           ("Record the path dependencies' file times for the cache", 'record')):
            self.assertEqual(step(web, name), "        if: steps.engine-cache.outputs.cache-matched-key == ''\n"
                             f'        run: python3 "$GITHUB_WORKSPACE/tooling/bevy-mtimes.py" {verb} target')
        order = [web.index(f'      - name: {name}\n') for name in (
            'Identify the wasm dependencies', 'Restore the pull request wasm target cache',
            "Restore the pull request's own wasm target cache",
            "Restore the path dependencies' cached file times", 'Build and test the local web app',
            "Record the path dependencies' file times for the cache", 'Save the pull request wasm target cache')]
        self.assertEqual(order, sorted(order))

        # Main fills the cache in a job that no job waits for, as the web job would compile it. Only a
        # first attempt runs: the cleanup keeps the newest entry, and a later attempt of an older run
        # would save a newer one.
        self.assertIn("    if: >-\n      needs.changes.outputs.web == 'true' && github.event_name == 'push' && "
                      "github.ref == 'refs/heads/main' &&\n      vars.BENCHMARK_ENABLED_RUNNERS == 'none' && "
                      "github.run_attempt == '1'\n", main)
        self.assertTrue(main.rstrip().endswith('    timeout-minutes: 45\n    needs:\n      - changes'))
        self.assertFalse([name for name, body in jobs.items() if re.search(r'^\s+- web-pr-target$', body, re.M)])
        for line in ('runs-on', 'BEVY_SEGMENT_WRITE_KEY', 'toolchain', 'targets', 'components', 'working-directory: rendering'):
            mine = re.search(rf'^ +{line}.*$', main, re.M).group().strip()
            self.assertIn(mine, [found.strip() for found in re.findall(rf'^ +{line}.*$', web, re.M)], line)
        self.assertIn('    permissions:\n      contents: read\n', main)
        # A push that changes no engine input finds its entry without a download, and every later step is skipped.
        probe = step(main, 'Look for the pull request wasm target cache')
        self.assertTrue((probe + '\n').endswith(f'          lookup-only: true\n          key: {main_key}\n'))
        self.assertNotIn('restore-keys', probe)
        after = main.split('      - name: Look for the pull request wasm target cache\n', 1)[1].split('\n    defaults:', 1)[0]
        later_steps = after.split('\n      - ')[1:]
        self.assertEqual(len(later_steps), 9)
        for later in later_steps:
            self.assertIn("        if: steps.wasm-pr-probe.outputs.cache-hit != 'true'", later, later.splitlines()[0])
        # Entries of the same dependencies only: an entry of other dependencies would carry their artifacts
        # into every later save.
        key = step(main, 'Restore the pull request wasm target cache')
        self.assertTrue((key + '\n').endswith(f'          restore-keys: {main_prefix}\n'))
        self.assertIn(f'          key: {main_key}\n', key)
        self.assertNotIn('bevy-web-unopt-v1-main-\n', key + '\n')
        self.assertIn('run: python3 ../../../apps/bevy-explorer/build-web.py --no-opt --target-only',
                      step(main, 'Compile the web engine as a pull request does'))
        self.assertIn('          key: ${{ steps.wasm-pr-cache.outputs.cache-primary-key }}',
                      step(main, 'Save the pull request wasm target cache'))

    def test_gateway_route_step_requires_the_ignored_fixture_to_pass(self):
        workflow = (ci.ROOT / ci.WORKFLOW).read_text(encoding='utf-8')
        _, _, after = workflow.partition('      - name: Test Explorer routes against the gateway\n')
        step = after.split('      - name:', 1)[0]
        self.assertIn("if: runner.os == 'Linux'", step)
        self.assertIn('set -o pipefail', step)
        self.assertIn('gateway::gateway_tests::gateway_routes_match_deployment -- --ignored --exact', step)
        self.assertIn('| tee "$RUNNER_TEMP/explorer-gateway-routes.log"', step)
        self.assertIn(
            "grep -Fx 'test gateway::gateway_tests::gateway_routes_match_deployment ... ok'"
            ' "$RUNNER_TEMP/explorer-gateway-routes.log"',
            step,
        )


class SelectionTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.root_patch = patch.object(ci, 'ROOT', self.root)
        self.root_patch.start()
        self.addCleanup(self.root_patch.stop)
        self.git('init', '-q')
        # A detached Git maintenance process can recreate .git entries while
        # TemporaryDirectory removes this fixture after the last commit.
        self.git('config', 'maintenance.auto', 'false')
        self.git('config', 'gc.auto', '0')
        self.write('rendering/bevy-explorer/source/src/lib.rs', 'fn main() {}\n')
        self.base = self.commit()

    def git(self, *args):
        return subprocess.check_output(['git', '-c', 'user.name=CI', '-c', 'user.email=ci@example.invalid', '-c', 'commit.gpgsign=false',
                                        *args], cwd=self.root, text=True).strip()

    def write(self, name, value):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(value)

    def commit(self):
        seal(self.root)
        self.git('add', '.')
        self.git('commit', '-qm', 'fixture', '--allow-empty')
        return self.git('rev-parse', 'HEAD')

    def test_reader_changes_select_web_but_test_only_changes_do_not_build(self):
        for name in ('tooling/hud_shared_inputs.py', 'tooling/tests/test_hud_shared_inputs.py',
                     'tooling/tests/hud_manifest_fixture.py', 'tooling/tests/hud_manifest_invalid_cases.json', 'tooling/tests/check_hud_manifest_nix.py'):
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write(name, 'reader input')
                result = ci.select(before, self.commit())
                self.assertEqual(result['web'], name == 'tooling/hud_shared_inputs.py')
                self.assertFalse(any(result[lane] for lane in ('clippy', 'test', 'headless', 'native-matrix', 'scenes')))

    def test_shared_reader_only_change_selects_images_without_engine_lanes(self):
        self.write('tooling/hud_shared_inputs.py', 'changed image verification dependency')
        head = self.commit()
        self.assertTrue(ci.consumer_plan('pull_request', self.base, head, '')['images'])
        selected = ci.select(self.base, head)
        self.assertTrue(selected['web'])
        self.assertFalse(any(selected[lane] for lane in ('clippy', 'test', 'headless', 'native-matrix', 'scenes')))

    def test_source_and_integration_inputs_select_builds(self):
        for name in ('rendering/bevy-explorer/source/src/lib.rs',
                     'rendering/bevy-explorer/source/react-web/pnpm-lock.yaml',
                     'apps/bevy-explorer/build-web.py', 'apps/bevy-explorer/Dockerfile.web',
                     'tooling/bevy-ci.py', 'tooling/ci_paths.py', ci.WORKFLOW,
                     '.github/workflows/bevy-explorer-images.yml'):
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write(name, 'changed')
                result = ci.select(before, self.commit())
                self.assertTrue(any(result[lane] for lane in ci.LANES))

    def test_dependency_toolchain_and_workflow_changes_select_native_matrix(self):
        for name in ('rendering/bevy-explorer/source/Cargo.lock',
                     'rendering/bevy-explorer/source/Cargo.toml',
                     'rendering/bevy-explorer/source/.cargo/config.toml',
                     'rendering/bevy-explorer/source/rust-toolchain.toml',
                     '.cargo/config.toml', 'rust-toolchain.toml', ci.WORKFLOW,
                     'library/vendor/rusty_enet/src/packet.rs',
                     'library/vendor/rusty_enet/checksums.sha256',
                     'tooling/dev-regressions.py', 'tooling/dev-regressions.json'):
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write(name, 'changed')
                self.assertTrue(ci.select(before, self.commit())['native-matrix'])

    def test_platform_inputs_under_source_select_native_matrix(self):
        for name in ('build.rs', 'crates/assets/build.rs', 'crates/dcl_component/build_quant.rs',
                     'local-harness/benchmark/src/main.rs', 'local-harness/web-driver/driver.mjs',
                     'crates/platform/src/native/deeplink/windows.rs', 'crates/platform/src/native/deeplink/macos.rs',
                     'crates/platform/src/native.rs', 'crates/platform/src/native/dm_history.rs',
                     'deploy/macos/Info.plist'):
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write('rendering/bevy-explorer/source/' + name, 'changed')
                self.assertTrue(ci.select(before, self.commit())['native-matrix'])

    def test_platform_neutral_source_and_protos_skip_only_the_native_matrix(self):
        for name in ('rendering/bevy-explorer/source/src/lib.rs', 'rendering/bevy-explorer/source/assets/image.png',
                     'rendering/bevy-explorer/source/crates/example/Cargo.toml',
                     'rendering/bevy-explorer/source/crates/platform/src/wasm.rs',
                     'rendering/bevy-explorer/source/crates/platform/src/native_util.rs',
                     'library/protocol/proto/decentraland/common/vectors.proto'):
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write(name, 'changed')
                selected = ci.select(before, self.commit())
                self.assertEqual({lane for lane in (*ci.LANES, 'native-matrix', 'scenes')
                                  if selected[lane]}, {*ci.LANES, 'scenes'})

    def test_cargo_lock_selects_native_matrix_only_when_the_locked_packages_change(self):
        def lock(*packages, dependencies=()):
            entries = ''.join(f'[[package]]\nname = "{name}"\nversion = "{version}"\n' for name, version in packages)
            return (f'version = 4\n\n{entries}[[package]]\nname = "dcl"\nversion = "0.1.0"\n'
                    f'dependencies = {json.dumps(list(dependencies))}\n')
        name = 'rendering/bevy-explorer/source/Cargo.lock'
        self.write(name, lock(('tokio', '1.0.0')))
        before = self.commit()
        cases = {'a new edge between locked packages': (lock(('tokio', '1.0.0'), dependencies=['tokio']), False),
                 'a new package': (lock(('tokio', '1.0.0'), ('url', '2.5.0')), True),
                 'a version bump': (lock(('tokio', '1.1.0')), True),
                 'an unreadable lockfile': ('not toml [', True)}
        for case, (content, native) in cases.items():
            with self.subTest(case=case):
                self.write(name, content)
                selected = ci.select(before, self.commit())
                self.assertEqual(selected['native-matrix'], native)
                self.assertTrue(all(selected[lane] for lane in (*ci.LANES, 'scenes')))

    def test_main_push_keeps_native_matrix_for_platform_neutral_source(self):
        self.write('rendering/bevy-explorer/source/src/lib.rs', 'native change')
        head = self.commit()
        with patch.object(ci, 'main_baselines', return_value=dict.fromkeys(ci.LANES, self.base)):
            result = ci.plan('push', self.base, head, 'example/project')
        self.assertTrue(result['test'])
        self.assertTrue(result['native-matrix'])

    def test_renaming_or_deleting_source_selects_builds(self):
        path = self.root / 'rendering/bevy-explorer/source/src/lib.rs'
        path.rename(self.root / 'moved.rs')
        result = ci.select(self.base, self.commit())
        self.assertTrue(all(result[lane] for lane in ci.LANES))

    def test_web_assets_validate_web_without_recompiling_native_targets(self):
        for name in ('react-web/src/App.tsx', 'react-web/pnpm-lock.yaml',
                     'react-web/bridge-scene/src/index.ts', 'deploy/web/engine/boot.js',
                     'deploy/web/service_worker.js'):
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write('rendering/bevy-explorer/source/' + name, 'changed')
                selected = ci.select(before, self.commit())
                self.assertEqual({lane for lane in (*ci.LANES, 'native-matrix', 'scenes')
                                  if selected[lane]}, {'web'})

    def test_avatar_preview_changes_build_and_publish_it_with_the_web_app(self):
        # The web lane compiles, packages and publishes the preview (play-artifact.py); the native
        # lanes never build it. Its prose selects nothing.
        for name in ('src/lib.rs', 'Cargo.lock', 'web/preview.js', 'web/index.html', 'tools/package.py'):
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write('rendering/avatar-preview/' + name, 'changed')
                selected = ci.select(before, self.commit())
                self.assertEqual({lane for lane in (*ci.LANES, 'native-matrix', 'scenes')
                                  if selected[lane]}, {'web'})
        before = self.git('rev-parse', 'HEAD')
        self.write('rendering/avatar-preview/README.md', 'prose')
        selected = ci.select(before, self.commit())
        self.assertFalse(any(selected[lane] for lane in (*ci.LANES, 'native-matrix', 'scenes')))

    def test_shared_hud_sources_validate_web_only(self):
        for name in SHARED:
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write(name, 'changed')
                selected = ci.select(before, self.commit())
                self.assertEqual({lane for lane in (*ci.LANES, 'native-matrix', 'scenes')
                                  if selected[lane]}, {'web'})

    def test_local_hashing_and_its_check_validate_web_only(self):
        # play-artifact.py fingerprints these paths with the web app: a change needs a web build
        # with the new fingerprint, or fetch-play.py finds no producer for it. Markdown is in
        # neither the fingerprint nor the selection.
        for name, lanes in (('library/hashing/src/cid.ts', {'web'}), ('library/hashing/package.json', {'web'}),
                            ('tooling/check-local-hashing.mjs', {'web'}), ('library/hashing/README.md', set()),
                            ('apps/creator-hub/packages/zip-extract/index.js', {'web'}), ('apps/creator-hub/packages/zip-extract/README.md', set())):
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write(name, 'changed')
                selected = ci.select(before, self.commit())
                self.assertEqual({lane for lane in (*ci.LANES, 'native-matrix', 'scenes') if selected[lane]}, lanes)

    def test_library_ui_files_outside_the_checked_graph_select_nothing(self):
        for name in ('library/ui/figma/src/cli.ts', 'library/ui/figma/review/src/review.tsx',
                     'library/ui/templates/component/Component.tsx', 'library/ui/upstream.lock.json'):
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write(name, 'changed')
                selected = ci.select(before, self.commit())
                self.assertEqual({lane for lane in (*ci.LANES, 'native-matrix', 'scenes') if selected[lane]}, set())

    def test_web_files_the_harness_tests_read_also_select_the_test_suite(self):
        for name in ci.HARNESS_WEB_READS:
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write(name, 'changed')
                selected = ci.select(before, self.commit())
                self.assertEqual({lane for lane in (*ci.LANES, 'native-matrix', 'scenes')
                                  if selected[lane]}, {'web', 'test'})

    def test_the_end_to_end_suite_selects_no_engine_lane(self):
        for name in ('driver/explorer.ts', 'specs/chat.test.ts', 'scenes/core/scene.json', 'README.md'):
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write('apps/bevy-explorer/e2e/' + name, 'changed')
                selected = ci.select(before, self.commit())
                self.assertEqual({lane for lane in (*ci.LANES, 'native-matrix', 'scenes')
                                  if selected[lane]}, set())

    def test_web_change_cannot_hide_a_native_or_shared_input_change(self):
        self.write('rendering/bevy-explorer/source/react-web/src/App.tsx', 'web edit')
        self.write('rendering/bevy-explorer/source/crates/system_api_types/build.rs', 'build script edit')
        selected = ci.select(self.base, self.commit())
        self.assertTrue(all(selected[lane] for lane in (*ci.LANES, 'native-matrix', 'scenes')))

    def test_headless_packages_select_headless_without_web_native_tests_or_scenes(self):
        for name in ('launcher/package.json', 'launcher/bin/cli.js',
                     'launcher/lib/resolve-binary.js', 'build-platform-package.js'):
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write('rendering/bevy-explorer/source/deploy/headless/' + name, 'changed')
                selected = ci.select(before, self.commit())
                self.assertEqual({lane for lane in (*ci.LANES, 'native-matrix', 'scenes')
                                  if selected[lane]}, {'headless'})

    def test_mixed_packages_keep_both_producers_and_rust_changes_widen_coverage(self):
        self.write('rendering/bevy-explorer/source/deploy/headless/launcher/package.json', 'headless')
        self.write('rendering/bevy-explorer/source/deploy/web/package.json', 'web')
        selected = ci.select(self.base, self.commit())
        self.assertEqual({lane for lane in (*ci.LANES, 'native-matrix', 'scenes')
                          if selected[lane]}, {'web', 'headless'})
        self.write('rendering/bevy-explorer/source/crates/dcl/src/lib.rs', 'changed Rust')
        selected = ci.select(self.base, self.commit())
        self.assertEqual({lane for lane in (*ci.LANES, 'native-matrix', 'scenes')
                          if selected[lane]}, {*ci.LANES, 'scenes'})

    def test_deleted_headless_launcher_still_selects_headless(self):
        name = 'rendering/bevy-explorer/source/deploy/headless/launcher/bin/cli.js'
        self.write(name, 'launcher')
        before = self.commit()
        (self.root / name).unlink()
        selected = ci.select(before, self.commit())
        self.assertTrue(selected['headless'])
        self.assertFalse(selected['test'])
        self.assertFalse(selected['web'])

    def test_deleted_web_assets_still_select_their_producer(self):
        name = 'rendering/bevy-explorer/source/react-web/src/App.tsx'
        self.write(name, 'web asset')
        before = self.commit()
        (self.root / name).unlink()
        selected = ci.select(before, self.commit())
        self.assertTrue(selected['web'])
        self.assertFalse(selected['test'])

    def test_pr_uses_supplied_base_and_merge_tree_without_api(self):
        self.write('apps/sites/only-sites.ts', 'changed')
        with patch.object(ci, 'api', side_effect=AssertionError('PR must not consult main runs')):
            self.assertFalse(ci.plan('pull_request', self.base, self.commit(), '')['test'])

    def pr_merge(self, main_file, pr_file):
        self.write(pr_file, 'pr')
        pr = self.commit()
        self.git('checkout', '-q', '--detach', self.base)
        self.write(main_file, 'main moved')
        tip = self.commit()
        self.git('merge', '-q', '--no-ff', '-m', f'Merge {pr} into {tip}', pr)
        return tip, pr, self.git('rev-parse', 'HEAD')

    def run_main(self, command, head, base, pr_head, **pull_request):
        with tempfile.TemporaryDirectory() as temporary:
            output, event = Path(temporary) / 'output', Path(temporary) / 'event.json'
            event.write_text(json.dumps({'pull_request': {'head': {'sha': pr_head}, **pull_request}}))
            env = {'GITHUB_EVENT_NAME': 'pull_request', 'GITHUB_EVENT_PATH': str(event), 'BASE_SHA': base,
                   'GH_REPO': '', 'GITHUB_OUTPUT': str(output), 'GITHUB_STEP_SUMMARY': ''}
            with patch.dict(os.environ, env), patch('sys.argv', ['bevy-ci.py', command, '--head', head]), \
                    patch.object(ci, 'api', side_effect=AssertionError('PR must not consult main runs')):
                ci.main()
            return dict(line.split('=', 1) for line in output.read_text().splitlines())

    def test_pr_compares_with_the_main_tip_it_was_merged_onto(self):
        tip, pr, merge = self.pr_merge('rendering/bevy-explorer/source/Cargo.toml', 'apps/sites/only-sites.ts')
        self.assertTrue(ci.plan('pull_request', self.base, merge, '')['native-matrix'])
        self.assertEqual(ci.tested_base(merge, pr, self.base), tip)
        values = self.run_main('plan', merge, self.base, pr)
        for lane in ('clippy', 'test', 'headless', 'web', 'native-matrix', 'scenes'):
            self.assertEqual(values[lane], 'false', lane)

    def test_pr_images_ignore_main_changes_since_the_payload_base(self):
        _, pr, merge = self.pr_merge('services/catalyst/src/main.rs', 'docs/only-docs.md')
        self.assertEqual(self.run_main('consumer-plan', merge, self.base, pr)['images'], 'false')

    def test_pr_merged_onto_main_keeps_its_own_engine_change(self):
        _, pr, merge = self.pr_merge('apps/sites/main-only.ts', 'rendering/bevy-explorer/source/src/lib.rs')
        values = self.run_main('plan', merge, self.base, pr)
        self.assertEqual((values['test'], values['web']), ('true', 'true'))

    def test_a_draft_holds_the_test_and_headless_jobs_until_ready_or_labelled(self):
        _, pr, merge = self.pr_merge('apps/sites/main-only.ts', 'rendering/bevy-explorer/source/src/lib.rs')
        held = self.run_main('plan', merge, self.base, pr, draft=True, labels=[{'name': 'ci:macos'}])
        self.assertEqual([held[lane] for lane in ('clippy', 'test', 'headless', 'web', 'native-matrix')],
                         ['true', 'false', 'false', 'true', 'false'])
        self.assertIn('draft: test, headless wait until it is ready for review', held['reason'])
        self.assertEqual(held['held'], 'true')
        for pull_request in ({'draft': False}, {'draft': True, 'labels': [{'name': 'ci:engine'}]}, {},
                             {'draft': 'true'}, {'draft': True, 'labels': 'ci:engine'}):
            values = self.run_main('plan', merge, self.base, pr, **pull_request)
            self.assertEqual([values[lane] for lane in ('clippy', 'test', 'headless', 'web')], ['true'] * 4, pull_request)
            self.assertNotIn('draft', values['reason'])
            self.assertNotIn('held', values)

    def test_a_draft_with_no_held_job_selected_reports_nothing(self):
        _, pr, merge = self.pr_merge('apps/sites/main-only.ts', 'docs/only-docs.md')
        values = self.run_main('plan', merge, self.base, pr, draft=True)
        self.assertNotIn('draft', values['reason'])
        self.assertNotIn('held', values)

    def test_the_draft_hold_names_jobs_on_larger_runners_and_never_the_web_job(self):
        workflow = (Path(__file__).parents[2] / ci.WORKFLOW).read_text(encoding='utf-8')
        jobs = dict(re.findall(r'(?ms)^  ([a-z-]+):\n(.*?)(?=^  [a-z-]+:\n|\Z)', workflow.split('\njobs:\n', 1)[1]))
        larger = {name for name, body in jobs.items() if re.search(r'runs-on: .*(16core|xlarge)', body)}
        conditions = {'test': 'test', 'headless-smoke': 'headless'}
        # web-pr-target runs only on main, where no pull request waits for it.
        self.assertEqual(larger, {*conditions, 'build-deploy-web', 'web-pr-target'})
        self.assertEqual(set(conditions.values()), set(ci.HELD_ON_DRAFT))
        # The development smoke job and the images read the web job's output, so a draft still builds it.
        for name in ('build-deploy-web', 'images'):
            self.assertNotRegex(jobs[name], r'needs\.changes\.outputs\.(test|headless)\b', name)
        for name, lane in conditions.items():
            self.assertRegex(jobs[name], rf"if: .*needs\.changes\.outputs\.{lane} == 'true'")
        # Marking a draft ready starts the run that the hold skipped.
        self.assertIn('    types: [opened, synchronize, reopened, ready_for_review, closed]\n', workflow)

    def test_pr_base_falls_back_unless_github_merged_the_pr_head(self):
        self.write('rendering/bevy-explorer/source/src/lib.rs', 'changed')
        head = self.commit()
        self.assertEqual(ci.tested_base(head, head, self.base), self.base)
        self.assertEqual(self.run_main('plan', head, self.base, head)['test'], 'true')
        # A branch's own merge of main has two parents, but its second is not the PR head.
        self.git('checkout', '-q', '--detach', self.base)
        self.write('apps/sites/main-only.ts', 'main')
        main_tip = self.commit()
        self.git('checkout', '-q', '--detach', head)
        self.git('merge', '-q', '--no-ff', '-m', 'Merge main into feature', main_tip)
        branch_merge = self.git('rev-parse', 'HEAD')
        self.assertEqual(ci.tested_base(branch_merge, branch_merge, self.base), self.base)
        self.assertEqual(ci.tested_base(branch_merge, None, self.base), self.base)
        self.git('commit', '-q', '--allow-empty', '-m', f'notes\n\nparent {head}')
        self.assertEqual(ci.tested_base(self.git('rev-parse', 'HEAD'), head, 'fallback'), 'fallback')
        self.assertEqual(ci.tested_base('0' * 39 + '1', head, 'fallback'), 'fallback')

    def test_pr_base_is_read_from_a_shallow_checkout_of_the_merge_commit(self):
        tip, pr, merge = self.pr_merge('rendering/bevy-explorer/source/Cargo.toml', 'apps/sites/only-sites.ts')
        self.git('config', 'uploadpack.allowAnySHA1InWant', 'true')
        with tempfile.TemporaryDirectory() as temporary:
            clone = Path(temporary)
            run = lambda *args: subprocess.check_output(['git', *args], cwd=clone, text=True).strip()
            run('init', '-q')
            run('config', 'maintenance.auto', 'false')
            run('config', 'gc.auto', '0')
            # What actions/checkout does for a pull_request: the merge commit at depth 1.
            run('remote', 'add', 'origin', self.root.as_uri())
            run('fetch', '-q', '--no-tags', '--depth=1', 'origin', f'+{merge}:refs/remotes/pull/1/merge')
            run('checkout', '-q', '--detach', merge)
            self.assertNotEqual(subprocess.run(['git', 'rev-parse', '--verify', '-q', 'HEAD^1'], cwd=clone).returncode, 0)
            with patch.object(ci, 'ROOT', clone):
                self.assertEqual(ci.tested_base(merge, pr, self.base), tip)
                self.assertEqual(self.run_main('plan', merge, self.base, pr)['native-matrix'], 'false')

    def test_gateway_contract_inputs_select_only_the_linux_test_lane_on_prs(self):
        for name in ('infra/gateway/gateway.py', 'infra/gateway/routes.nix',
                     'apps/ui3/src/play/lib/baseDomain.ts',
                     'library/explorer-core/engine/generated/serviceTable.ts'):
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write(name, 'changed')
                selected = ci.plan('pull_request', before, self.commit(), '')
                self.assertEqual({lane for lane in (*ci.LANES, 'native-matrix', 'scenes')
                                  if selected[lane]}, {'test'})

    def test_upstream_watch_and_manual_packaging_are_not_ci_build_inputs(self):
        for name in ('upstream', 'desktop', 'rollout-manual'):
            self.write(f'.github/workflows/bevy-explorer-{name}.yml', 'changed')
        result = ci.select(self.base, self.commit())
        self.assertFalse(any(result[lane] for lane in ci.LANES))

    def test_the_upstream_and_provenance_tools_run_the_guards_and_build_nothing(self):
        # The workflow filters start the run for these; the `source` job tests and runs them.
        for name in ('tooling/upstream-drift.py', 'tooling/upstream-drift-site.py', 'tooling/tests/test_upstream_drift.py',
                     'tooling/vendor-provenance.py', 'tooling/enet-provenance.py'):
            self.write(name, 'changed')
        result = ci.select(self.base, self.commit())
        self.assertFalse(any(result[lane] for lane in (*ci.LANES, 'native-matrix', 'scenes')))

    def test_platform_workflow_changes_select_the_shared_linux_producer(self):
        for name in ('bevy-explorer-headless.yml', 'bevy-explorer-headless-build.yml'):
            before = self.git('rev-parse', 'HEAD')
            self.write('.github/workflows/' + name, 'changed')
            selected = ci.select(before, self.commit())
            self.assertTrue(selected['headless'])
            self.assertFalse(selected['web'])
            self.assertFalse(selected['test'])

    def test_avatar_preview_page_change_rebuilds_images(self):
        # The page Sites serves at /avatar-preview/ comes from this tree: changing only it must still
        # build the images, or dcl.dev keeps the previous page.
        self.write('rendering/avatar-preview/web/index.html', 'changed')
        head = self.commit()
        self.assertTrue(ci.consumer_changed(self.base, head, 'images'))
        self.assertTrue(ci.select(self.base, head)['web'])

    def test_image_only_change_selects_consumer_without_engine(self):
        self.write('apps/sites/server.ts', 'changed')
        head = self.commit()
        self.assertTrue(ci.consumer_changed(self.base, head, 'images'))
        self.assertFalse(ci.consumer_plan('pull_request', self.base, head, '')['images'])
        self.assertFalse(any(ci.select(self.base, head)[lane] for lane in ci.LANES))

    def test_docs_only_change_skips_image_consumer(self):
        for name in ('DATABASE.md', 'docs/example.md', 'services/catalyst/docs/events.md',
                     'services/catalyst/docs/example.json', 'rendering/bevy-explorer/docs/example.json',
                     'rendering/bevy-explorer/source/README.md',
                     'apps/bevy-explorer/README.md', 'library/vendor/rusty_enet/README.md'):
            self.write(name, 'changed')
        head = self.commit()
        self.assertFalse(ci.consumer_plan('pull_request', self.base, head, '')['images'])
        self.assertFalse(ci.consumer_changed(self.base, head, 'images'))
        result = ci.select(self.base, head)
        self.assertFalse(any(result[lane] for lane in (*ci.LANES, 'native-matrix', 'scenes')))
        self.write('services/catalyst/explore/src/lib.rs', 'source')
        head = self.commit()
        self.assertTrue(ci.consumer_changed(self.base, head, 'images'))
        self.assertFalse(ci.consumer_plan('pull_request', self.base, head, '')['images'])

    def test_docs_source_embedded_data_and_published_content_rebuild_images(self):
        for name in ('apps/sites/src/features/docs/DocsPage.tsx',
                     'apps/sites/src/data/lib/docs/render.ts',
                     'library/dcl-scene-authoring/docs/composite-component-schemas.json',
                     'library/dcl-scene-authoring/docs/composite-tojson-edge-cases.json',
                     'apps/sites/content/docs/example.md', 'apps/sites/content/docs/page.mdx',
                     'apps/sites/content/docs/manifest.json', 'apps/sites/content/docs/search-index.json',
                     'apps/sites/content/docs/_media.json'):
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write(name, 'changed')
                head = self.commit()
                self.assertTrue(ci.consumer_changed(before, head, 'images'))
                self.assertEqual(ci.consumer_plan('pull_request', before, head, '')['images'],
                                 any(fnmatch.fnmatchcase(name, pattern) for pattern in ci.IMAGES_PACKAGING))
                with patch.object(ci, 'api', return_value={'workflow_runs': [{'head_sha': before, 'id': 1}]}), \
                        patch.object(ci, 'trusted_run', return_value=True), \
                        patch.object(ci, 'run_jobs', return_value=[]), \
                        patch.object(ci, 'consumer_ready', return_value=True):
                    result = ci.consumer_plan('push', before, head, 'example/project')
                self.assertTrue(result['images'])
                self.assertEqual(result['reason'], 'Image consumer inputs changed')

    def test_a_pull_request_builds_images_for_packaging_only(self):
        for name, built in (('services/catalyst/src/new.rs', False), ('apps/sites/src/new.ts', False),
                            ('infra/containers/new.nix', True), ('tooling/image-new.py', True),
                            ('services/catalyst/Cargo.lock', True), ('services/catalyst/new/Cargo.toml', True),
                            ('services/catalyst/rust-toolchain.toml', True), ('.cargo/config.toml', True),
                            ('library/new/Cargo.toml', True), ('apps/sites/pnpm-lock.yaml', True),
                            ('apps/sites/package.json', True), ('apps/sites/.npmrc', True),
                            ('library/ui/pnpm-lock.yaml', True), ('library/ui/package.json', True),
                            ('library/ui/pnpm-workspace.yaml', True), ('library/ui/gallery/src/new.tsx', False)):
            with self.subTest(path=name):
                before = self.git('rev-parse', 'HEAD')
                self.write(name, 'changed')
                head = self.commit()
                self.assertEqual(ci.consumer_plan('pull_request', before, head, '')['images'], built)
                self.assertTrue(ci.consumer_changed(before, head, 'images'))

    def test_the_architecture_choice_reads_packaging_inputs_only(self):
        workflow = (Path(__file__).parents[2] / '.github/workflows/apis-sites-images.yml').read_text()
        listed = workflow.split('git diff --quiet HEAD^1 HEAD -- \\\n', 1)[1].split('; then', 1)[0]
        specs = shlex.split(listed.replace('\\\n', ' '))
        self.assertIn('apps/sites/pnpm-lock.yaml', specs)
        for spec in specs:
            sample = spec.replace('*', 'a')
            with self.subTest(spec=spec):
                self.assertTrue(any(fnmatch.fnmatchcase(path, pattern) for pattern in ci.IMAGES_PACKAGING
                                    for path in (sample, sample + '/a')))

    def test_source_renamed_to_documentation_still_selects_builds(self):
        (self.root / 'rendering/bevy-explorer/source/src/lib.rs').rename(
            self.root / 'rendering/bevy-explorer/source/source.md')
        head = self.commit()
        self.assertTrue(ci.consumer_changed(self.base, head, 'images'))
        self.assertTrue(ci.select(self.base, head)['test'])

    def test_markdown_golden_changes_select_tests_and_images(self):
        self.write('rendering/bevy-explorer/source/local-harness/benchmark/tests/fixtures/report/ok.md',
                   'broken report golden')
        head = self.commit()
        self.assertTrue(ci.select(self.base, head)['test'])
        self.assertTrue(ci.consumer_changed(self.base, head, 'images'))

    def test_unwatched_fixtures_do_not_select_images(self):
        self.write('apps/creator-hub/tests/fixtures/example.md', 'fixture')
        self.write('apps/creator-hub/tests/fixtures/example.json', '{}')
        head = self.commit()
        self.assertFalse(ci.consumer_changed(self.base, head, 'images'))
        self.assertFalse(ci.consumer_plan('pull_request', self.base, head, '')['images'])

    def test_deleted_prose_documentation_does_not_select_images(self):
        for path in ('services/catalyst/docs/example.md', 'rendering/bevy-explorer/docs/example.md',
                     'docs/example.md'):
            with self.subTest(path=path):
                self.write(path, 'prose')
                before = self.commit()
                (self.root / path).unlink()
                head = self.commit()
                self.assertFalse(ci.consumer_changed(before, head, 'images'))
                self.assertFalse(ci.consumer_plan('pull_request', before, head, '')['images'])

    def test_removed_or_renamed_embedded_markdown_selects_builds(self):
        for path in ('rendering/bevy-explorer/source/crates/example/README.md',
                     'rendering/bevy-explorer/source/crates/example/docs/conversion.md'):
            for rename in (False, True):
                with self.subTest(path=path, rename=rename):
                    self.write(path, 'original')
                    before = self.commit()
                    self.write(path, 'edited rustdoc')
                    edited = self.commit()
                    self.assertFalse(ci.select(before, edited)['test'])
                    self.assertFalse(ci.consumer_changed(before, edited, 'images'))
                    source = self.root / path
                    if rename:
                        source.rename(source.with_name('renamed.md'))
                    else:
                        source.unlink()
                    removed = self.commit()
                    self.assertTrue(ci.select(edited, removed)['test'])
                    self.assertTrue(ci.consumer_changed(edited, removed, 'images'))

    def test_local_hook_changes_do_not_select_engine_or_images(self):
        self.write('tooling/ci-local.py', 'unrelated local hook change')
        head = self.commit()
        result = ci.select(self.base, head)
        self.assertFalse(any(result[lane] for lane in (*ci.LANES, 'native-matrix', 'scenes')))
        self.assertFalse(ci.consumer_changed(self.base, head, 'images'))

    def test_wildcard_inputs_stay_in_their_directory(self):
        self.write('tooling/dev-regressions-x/y.json', '{}')
        result = ci.select(self.base, self.commit())
        self.assertFalse(any(result[lane] for lane in ci.LANES) or result['scenes'])

    def test_schema_files_rebuild_images_and_independent_crates_or_docs_do_not(self):
        base = self.base
        for name, expected in (('schemas/10_explore.sql', True), ('apps/dcl-scene-cli/src/main.rs', False),
                               ('rendering/bevy-explorer/README.md', False)):
            self.write(name, 'changed')
            head = self.commit()
            self.assertEqual(ci.consumer_changed(base, head, 'images'), expected, name)
            base = head

    def test_consumer_failure_to_find_a_successful_baseline_builds_images(self):
        with patch.object(ci, 'api', return_value={'workflow_runs': []}):
            self.assertTrue(ci.consumer_plan('push', self.base, self.base, 'example/project')['images'])

    def test_push_uses_successful_build_not_previous_canceled_commit(self):
        self.write('rendering/bevy-explorer/source/src/lib.rs', 'engine edit')
        canceled = self.commit()
        self.write('apps/bevy-explorer/README.md', 'docs only')
        with patch.object(ci, 'main_baselines', return_value=dict.fromkeys(ci.LANES, self.base)):
            self.assertTrue(ci.plan('push', canceled, self.commit(), 'example/project')['test'])

    def test_manual_missing_history_and_api_errors_run_full(self):
        with patch.object(ci, 'api', side_effect=OSError('unavailable')):
            for event, base in (('workflow_dispatch', self.base), ('push', self.base),
                                ('pull_request', '0' * 40), ('unknown', self.base)):
                with self.subTest(event=event):
                    result = ci.plan(event, base, self.base, 'example/project')
                    self.assertTrue(all(result[lane] for lane in ci.LANES))
                    self.assertTrue(result['native-matrix'])

    def test_known_integration_inputs_select_only_their_consumers(self):
        cases = {
            'apps/bevy-explorer/Dockerfile.web': {'web'},
            'apps/bevy-explorer/nginx.conf': {'web'},
            'apps/bevy-explorer/build-web.py': {'web'},
            'apps/bevy-explorer/build-headless.py': {'headless'},
            'apps/bevy-explorer/Dockerfile.headless': {'headless'},
            'apps/bevy-explorer/image-smoke.py': {'web', 'headless'},
            '.github/workflows/bevy-explorer-images.yml': {'web', 'headless'},
            'apps/bevy-explorer/prepare-native-cache.py': {'headless', 'native-matrix', 'scenes'},
            'apps/bevy-explorer/install-linux-deps.py': {*ci.LANES, 'scenes'},
            'apps/bevy-explorer/install-cef.py': {'test', 'native-matrix', 'scenes'},
            'apps/bevy-explorer/install-web-tools.py': {'web'},
            ci.SCENES_WORKFLOW: {'scenes'},
            ci.WORKFLOW: {*ci.LANES, 'native-matrix'},
            'apps/bevy-explorer/README.md': set(),
            'tooling/tests/test_bevy_linux_deps.py': set(),
        }
        for path, expected in cases.items():
            with self.subTest(path=path):
                before = self.git('rev-parse', 'HEAD')
                self.write(path, 'changed')
                result = ci.select(before, self.commit())
                self.assertEqual({lane for lane in (*ci.LANES, 'native-matrix', 'scenes') if result[lane]}, expected)

    def test_the_relay_wake_browser_test_follows_its_crates_bevy_fork_tools_and_workflow(self):
        cases = {
            'rendering/bevy-explorer/source/crates/dcl_wasm/src/relay_wake.rs': True,
            'rendering/bevy-explorer/source/crates/dcl_wasm/Cargo.toml': True,
            'rendering/bevy-explorer/source/crates/common/src/lib.rs': True,
            'rendering/bevy-explorer/source/crates/console/src/lib.rs': True,
            'rendering/bevy-explorer/source/crates/dcl_component/build.rs': True,
            'rendering/bevy-explorer/source/crates/dcl_js/src/modules/EngineApi.js': True,
            'rendering/bevy-explorer/source/crates/dcl_types/src/lib.rs': True,
            'rendering/bevy-explorer/source/crates/platform/src/lib.rs': True,
            'rendering/bevy-explorer/source/crates/system_api_types/src/lib.rs': True,
            'rendering/bevy-explorer/source/crates/system_bridge/src/lib.rs': True,
            'rendering/bevy-explorer/vendor/rpc-rust/rpc/src/lib.rs': True,
            'library/protocol/proto/decentraland/sdk/components/transform.proto': True,
            'rendering/bevy-explorer/patches/bevy/series/0080-web-worker.patch': True,
            'rendering/bevy-explorer/patches/bevy/source.toml': True,
            'rendering/bevy-explorer/source/Cargo.toml': True,
            'rendering/bevy-explorer/source/.cargo/config.toml': True,
            'apps/bevy-explorer/install-web-tools.py': True,
            'apps/bevy-explorer/install-tools.sh': True,
            'tooling/bevy-target-cache.py': True,
            'tooling/bevy-mtimes.py': True,
            'flake.lock': True,
            ci.WORKFLOW: True,
            'rendering/bevy-explorer/source/crates/scene_runner/src/lib.rs': False,
            'rendering/bevy-explorer/source/crates/dcl_deno/src/lib.rs': False,
            'rendering/bevy-explorer/vendor/bevy_dui/src/lib.rs': False,
            'rendering/bevy-explorer/patches/wgpu/source.toml': False,
            'rendering/bevy-explorer/source/deploy/web/engine/sandbox_worker.js': False,
            'apps/bevy-explorer/build-web.py': False,
        }
        for path, expected in cases.items():
            with self.subTest(path=path):
                before = self.git('rev-parse', 'HEAD')
                self.write(path, 'changed')
                self.assertEqual(ci.select(before, self.commit())['relay-wake'], expected)
        self.assertTrue(ci.full_selection('Manual full run')['relay-wake'])

    def test_on_main_the_relay_wake_browser_test_follows_the_web_baseline(self):
        self.write('rendering/bevy-explorer/source/crates/dcl_wasm/src/lib.rs', 'relay change')
        changed = self.commit()
        self.write('apps/bevy-explorer/nginx.conf', 'web change')
        head = self.commit()
        for web, expected in ((self.base, True), (changed, False)):
            with self.subTest(web=web):
                baselines = {**dict.fromkeys(ci.LANES, self.base), 'web': web}
                with patch.object(ci, 'main_baselines', return_value=baselines):
                    self.assertEqual(ci.plan('push', self.base, head, 'example/project')['relay-wake'], expected)

    def test_the_movement_scene_checks_follow_the_scene_and_the_workflow(self):
        cases = {
            'rendering/bevy-explorer/source/movement-scene/src/vertical.ts': True,
            'rendering/bevy-explorer/source/movement-scene/test/run.mjs': True,
            'rendering/bevy-explorer/source/movement-scene/package.json': True,
            'rendering/bevy-explorer/source/movement-scene/pnpm-lock.yaml': True,
            ci.WORKFLOW: True,
            'rendering/bevy-explorer/source/movement-scene/README.md': False,
            'rendering/bevy-explorer/source/crates/avatar/src/animate.rs': False,
            'rendering/bevy-explorer/source/react-web/bridge-scene/src/index.ts': False,
        }
        for path, expected in cases.items():
            with self.subTest(path=path):
                before = self.git('rev-parse', 'HEAD')
                self.write(path, 'changed')
                self.assertEqual(ci.select(before, self.commit())['movement-scene'], expected)
        self.assertTrue(ci.full_selection('Manual full run')['movement-scene'])

    def test_on_main_the_movement_scene_checks_follow_the_web_baseline(self):
        self.write('rendering/bevy-explorer/source/movement-scene/src/index.ts', 'scene change')
        changed = self.commit()
        self.write('apps/bevy-explorer/nginx.conf', 'web change')
        head = self.commit()
        for web, expected in ((self.base, True), (changed, False)):
            with self.subTest(web=web):
                baselines = {**dict.fromkeys(ci.LANES, self.base), 'web': web}
                with patch.object(ci, 'main_baselines', return_value=baselines):
                    self.assertEqual(ci.plan('push', self.base, head, 'example/project')['movement-scene'], expected)

    def test_unknown_inputs_and_shared_protoc_changes_run_all_platforms_and_scenes(self):
        for path in ('apps/bevy-explorer/future-build-input', 'apps/bevy-explorer/install-protoc.py',
                     'tooling/bevy-ci.py', 'tooling/bevy-mtimes.py', 'tooling/bevy-target-cache.py'):
            before = self.git('rev-parse', 'HEAD')
            self.write(path, 'changed')
            result = ci.select(before, self.commit())
            self.assertTrue(all(result[lane] for lane in (*ci.LANES, 'native-matrix', 'scenes')))

    def test_flake_tool_changes_select_the_linux_lanes_and_the_native_matrix_without_scenes(self):
        # The Linux lanes take protoc, taplo and wasm-pack from the flake's bevy-tools package and
        # the macOS Test Suite its ffmpeg; the graphical scenes still run install-protoc.py.
        for path in ('flake.lock', 'flake.nix', 'infra/packages/default.nix', 'infra/packages/tools.nix',
                     '.github/actions/bevy-explorer-tools/action.yml', 'apps/bevy-explorer/install-tools.sh'):
            with self.subTest(path=path):
                before = self.git('rev-parse', 'HEAD')
                self.write(path, 'changed')
                result = ci.select(before, self.commit())
                self.assertEqual({lane for lane in (*ci.LANES, 'native-matrix', 'scenes') if result[lane]}, {*ci.LANES, 'native-matrix'})
        # The other package definitions feed the images, not the engine lanes.
        before = self.git('rev-parse', 'HEAD')
        self.write('infra/packages/play.nix', 'changed')
        result = ci.select(before, self.commit())
        self.assertFalse(any(result[lane] for lane in (*ci.LANES, 'native-matrix', 'scenes')))

    def test_a_bevy_patch_and_the_patch_tool_select_the_test_suite_on_each_system(self):
        # The Test Suite rebuilds the Bevy tree from its series with the tool and tests its crates on
        # each system; Clippy rebuilds it for a shader that `common`'s tests include. Every build takes
        # a patched source by the revision that Cargo.lock names, so no other job and no image reads a
        # series or the tool.
        cases = (('rendering/bevy-explorer/patches/bevy/series/0001-scene-spawner.patch', {'test', 'clippy', 'native-matrix'}),
                 ('rendering/bevy-explorer/patches/bevy/source.toml', {'test', 'clippy', 'native-matrix'}),
                 ('tooling/patched-sources.py', {'test', 'clippy', 'native-matrix'}),
                 ('rendering/bevy-explorer/patches/winit/series/0001-one.patch', set()))
        for path, expected in cases:
            with self.subTest(path=path):
                before = self.git('rev-parse', 'HEAD')
                self.write(path, 'changed')
                head = self.commit()
                result = ci.select(before, head)
                self.assertEqual({lane for lane in (*ci.LANES, 'native-matrix', 'scenes') if result[lane]}, expected)
                self.assertFalse(ci.consumer_changed(before, head, 'images'))
                self.assertFalse(ci.consumer_changed(before, head, 'headless'))

    def test_in_tree_crates_select_every_lane_and_rebuild_the_images(self):
        # Cargo compiles the crates tracked as exact bytes from their paths.
        self.write('rendering/bevy-explorer/vendor/bevy_dui/src/lib.rs', 'changed')
        head = self.commit()
        result = ci.select(self.base, head)
        self.assertTrue(all(result[lane] for lane in (*ci.LANES, 'native-matrix', 'scenes')))
        self.assertTrue(ci.consumer_changed(self.base, head, 'images'))

    def test_online_guards_follow_the_changed_pins_and_the_in_tree_crates(self):
        pins, vendor = 'rendering/bevy-explorer/patches', 'rendering/bevy-explorer/vendor'
        for name in ('bevy', 'winit', 'old'):
            self.write(f'{pins}/{name}/source.toml', f'name = "{name}"\ncommit = "1"\n')
        self.write(f'{pins}/bevy/series/0001-one.patch', 'one')
        self.write(f'{vendor}/bevy_dui/src/lib.rs', 'one')
        before = self.commit()
        cases = [
            # A patch, a document and a file beside the pin change no pin.
            ({f'{pins}/bevy/series/0001-one.patch': 'two', f'{pins}/README.md': 'text', f'{pins}/winit/notes.toml': 'x'}, '', False),
            ({f'{pins}/winit/source.toml': 'name = "winit"\ncommit = "2"\n'}, 'winit', False),
            ({f'{pins}/bevy/source.toml': 'commit = "3"\n', f'{pins}/new/source.toml': 'commit = "1"\n'}, 'bevy new', False),
            ({f'{vendor}/bevy_dui/src/lib.rs': 'two'}, '', True),
            ({f'{vendor}/bevy_dui/upstream.lock.json': '{}'}, '', True),
            ({'tooling/vendor-provenance.py': 'changed'}, '', True),
            ({'tooling/enet-provenance.py': 'changed'}, '', True),
            ({'tooling/upstream-drift.py': 'changed', 'rendering/bevy-explorer/source/vendor/basisu/x.c': 'x'}, '', False),
        ]
        for files, expected, provenance in cases:
            with self.subTest(files=sorted(files)):
                for name, value in files.items():
                    self.write(name, value)
                head = self.commit()
                result = ci.guards_plan('pull_request', before, head)
                self.assertEqual((result['pins'], result['provenance']), (expected, provenance))
                before = head
        # A removed source has no pin to prove.
        (self.root / pins / 'old/source.toml').unlink()
        self.assertEqual(ci.guards_plan('push', before, self.commit())['pins'], '')

    def test_a_changed_series_or_tool_selects_the_sources_to_rebuild_and_prove(self):
        pins = 'rendering/bevy-explorer/patches'
        for name in ('bevy', 'winit', 'old'):
            self.write(f'{pins}/{name}/source.toml', 'rev = "1"\n')
            self.write(f'{pins}/{name}/series/0001-one.patch', 'one')
        before = self.commit()
        cases = [
            ({f'{pins}/bevy/series/0001-one.patch': 'two'}, 'bevy'),
            ({f'{pins}/winit/source.toml': 'rev = "2"\n', f'{pins}/new/source.toml': 'rev = "1"\n'}, 'new winit'),
            # A document beside the sources and the lock that names the revisions belong to no series.
            ({f'{pins}/README.md': 'text', 'rendering/bevy-explorer/source/Cargo.lock': 'changed'}, ''),
            # The tool decides every tree and revision.
            ({'tooling/patched-sources.py': 'changed'}, 'bevy new old winit'),
        ]
        for files, expected in cases:
            with self.subTest(files=sorted(files)):
                for name, value in files.items():
                    self.write(name, value)
                head = self.commit()
                self.assertEqual(ci.guards_plan('pull_request', before, head)['verify'], expected)
                before = head
        # A removed patch changes its series; a removed source has none to prove.
        for name in ('bevy/series/0001-one.patch', 'old/series/0001-one.patch', 'old/source.toml'):
            (self.root / pins / name).unlink()
        self.assertEqual(ci.guards_plan('push', before, self.commit())['verify'], 'bevy')

    def test_online_guards_all_run_without_a_comparison(self):
        for name in ('bevy', 'winit'):
            self.write(f'rendering/bevy-explorer/patches/{name}/source.toml', 'commit = "1"\n')
        self.write('rendering/bevy-explorer/patches/winit/series/0001-one.patch', 'one')
        head = self.commit()
        everything = {'pins': 'bevy winit', 'verify': 'bevy winit', 'provenance': True,
                      'reason': 'Comparison unavailable; every online source guard runs'}
        with patch('builtins.print'):
            self.assertEqual(ci.guards_plan('workflow_dispatch', self.base, head), everything)
            self.assertEqual(ci.guards_plan('push', '0' * 40, head), everything)
            self.assertEqual(ci.guards_plan('pull_request', '', head), everything)

    def test_newer_web_pass_cannot_hide_an_unvalidated_native_change_on_main(self):
        self.write('rendering/bevy-explorer/source/src/lib.rs', 'native change')
        later_web = self.commit()
        self.write('apps/bevy-explorer/nginx.conf', 'web change')
        head = self.commit()
        baselines = {**dict.fromkeys(ci.LANES, self.base), 'web': head}
        with patch.object(ci, 'main_baselines', return_value=baselines):
            result = ci.plan('push', later_web, head, 'example/project')
        self.assertFalse(result['web'])
        self.assertTrue(result['test'])
        self.assertTrue(result['headless'])
        self.assertTrue(result['native-matrix'])

    def test_missing_main_lane_baseline_runs_only_that_unproven_lane(self):
        baselines = dict.fromkeys(('clippy', 'test', 'web'), self.base)
        with patch.object(ci, 'main_baselines', return_value=baselines):
            result = ci.plan('push', self.base, self.base, 'example/project')
        self.assertTrue(result['headless'])
        self.assertFalse(result['web'])
        self.assertFalse(result['test'])
        self.assertFalse(result['native-matrix'])

    def test_graphical_selection_uses_event_diff_and_manual_run_is_full(self):
        self.write('apps/bevy-explorer/Dockerfile.web', 'web image change')
        head = self.commit()
        self.assertFalse(ci.scenes_plan('pull_request', self.base, head)['scenes'])
        self.assertFalse(ci.scenes_plan('push', self.base, head)['scenes'])
        self.assertTrue(ci.scenes_plan('workflow_dispatch', self.base, head)['scenes'])
        self.write('rendering/bevy-explorer/source/src/lib.rs', 'engine change')
        head = self.commit()
        self.assertTrue(ci.scenes_plan('pull_request', self.base, head)['scenes'])
        self.assertEqual(ci.scenes_plan('push', self.base, head)['sha'], head)
        self.assertTrue(ci.scenes_plan('push', '0' * 40, head)['scenes'])

    def test_allowed_failure_list_changes_select_only_the_graphical_suite(self):
        self.write('apps/bevy-explorer/scene-tests.txt', '52,-52\n')
        selected = ci.select(self.base, self.commit())
        self.assertTrue(selected['scenes'])
        self.assertFalse(any(selected[lane] for lane in (*ci.LANES, 'native-matrix')))


class RelayWakeCrateDirsTest(unittest.TestCase):
    """RELAY_WAKE_CRATE_DIRS lists by hand the directories of the crates the relay wake browser test
    compiles. A path dependency that dcl_wasm gains, directly or through another crate, must fail here
    until the list names it."""

    def test_the_list_names_dcl_wasm_and_every_path_dependency_it_reaches(self):
        self.assertEqual(sorted(locked_path_dependencies(ci.ROOT / ci.SOURCE, 'dcl_wasm', ci.ROOT.resolve())),
                         sorted(ci.RELAY_WAKE_CRATE_DIRS))

    def test_a_transitive_vendored_or_patched_path_dependency_is_found(self):
        files = {
            'source/Cargo.toml': '[workspace]\nmembers = ["crates/*"]\n'
                                 '[workspace.dependencies]\na = { path = "crates/a" }\nb = { path = "../vendor/b" }\n'
                                 '[patch.crates-io]\nc = { path = "../vendor/c" }\n',
            'source/crates/top/Cargo.toml': '[package]\nname = "top"\n[dependencies]\na = { workspace = true }\n',
            'source/crates/a/Cargo.toml': '[package]\nname = "a"\n'
                                          "[target.'cfg(target_arch = \"wasm32\")'.dependencies]\nb = { workspace = true }\n",
            'source/crates/other/Cargo.toml': '[package]\nname = "other"\n',
            'vendor/b/Cargo.toml': '[package]\nname = "b"\n[dependencies]\nc = "1"\n',
            'vendor/c/Cargo.toml': '[package]\nname = "c"\n',
            'source/Cargo.lock': ''.join(
                f'[[package]]\nname = "{name}"\nversion = "{version}"\n{extra}\n' for name, version, extra in (
                    ('top', '0.1.0', 'dependencies = ["a", "serde"]'), ('a', '0.1.0', 'dependencies = ["b"]'),
                    ('b', '0.1.0', 'dependencies = ["c 1.0.0"]'), ('c', '1.0.0', ''), ('other', '0.1.0', ''),
                    ('serde', '1.0.0', 'source = "registry+https://github.com/rust-lang/crates.io-index"'))),
        }
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            for path, text in files.items():
                (root / path).parent.mkdir(parents=True, exist_ok=True)
                (root / path).write_text(text, encoding='utf-8')
            self.assertEqual(locked_path_dependencies(root / 'source', 'top', root),
                             {'source/crates/top/', 'source/crates/a/', 'vendor/b/', 'vendor/c/'})


class SceneSuiteTest(unittest.TestCase):
    SCENES = ['52,-52', '52,-54', '52,-56', '52,-58', '52,-60', '52,-62', '52,-64', '52,-66', '52,-68',
              '54,-52', '54,-54', '54,-56', '54,-58', '54,-60']

    def test_suite_file_lists_the_fourteen_scenes_and_the_snapshot_subset(self):
        full = ci.scene_suite('full').split(';')
        self.assertEqual([scene.split('/')[0] for scene in full], self.SCENES)
        self.assertEqual([scene.split('/')[0] for scene in ci.scene_suite('snapshots').split(';')],
                         list(ci.SNAPSHOT_SCENES))
        names = [name for scene in full for name in scene.split('/')[1:]]
        self.assertTrue(all(name == name.strip() and ';' not in name for name in names))
        self.assertEqual(len(names), len(set(names)))

    def test_malformed_suite_lines_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(ci, 'ROOT', Path(temporary)):
            suite = Path(temporary) / ci.SCENE_SUITE
            suite.parent.mkdir(parents=True)
            for bad in ('52,-52/ trailing space ', '52,-52//', 'a,b', '52,-52/x;y'):
                suite.write_text(f'# comment\n{bad}\n')
                with self.assertRaises(ValueError, msg=bad):
                    ci.scene_suite('full')
            suite.write_text('52,-52/a: b\n52,-62\n')
            self.assertEqual(ci.scene_suite('full'), '52,-52/a: b;52,-62')
            self.assertEqual(ci.scene_suite('snapshots'), '52,-62')

    def test_summary_reports_listed_unlisted_and_stale_failures(self):
        log = ('INFO scene_runner::automatic_testing: test a: b: true [1 remaining]\n'
               'INFO scene_runner::automatic_testing: test c: d: false [0 remaining]\n'
               'INFO scene_runner::automatic_testing: test e: f: false [0 remaining]\n'
               'INFO scene_runner::automatic_testing: some tests failed:\n [\n'
               '    (\n        "[(52,-52) : c: d]",\n        "Error: image not found (it has been created)",\n'
               '        true,\n    ),\n    (\n        "[(52,-62) : e: f]",\n'
               '        "Error: method grey-diff was specified and greyPixelDiff similarity (0.97) is lower than threshold 0.9995",\n'
               '        false,\n    ),\n]\n')
        with tempfile.TemporaryDirectory() as temporary, patch.object(ci, 'ROOT', Path(temporary)):
            suite = Path(temporary) / ci.SCENE_SUITE
            suite.parent.mkdir(parents=True)
            suite.write_text('52,-52/c: d/g: h\n52,-62\n')
            text = ci.scene_summary('full', 'failure', log)
            self.assertIn('1 assertion passed, 2 failed; 1 of them are listed', text)
            self.assertIn('Not listed: (52,-62) e: f', text)
            self.assertIn('Listed but passing, remove them: (52,-52) g: h', text)
            self.assertNotIn('discarded', text)
            self.assertIn('no assertion ran', ci.scene_summary('full', 'failure', ''))
            self.assertIn('acceptance holds', ci.scene_summary('snapshots', 'success', 'INFO scene_runner::automatic_testing: test a: b: true [0 remaining]\nall tests passed!'))

    def test_summary_of_a_run_that_stopped_early_determines_no_list(self):
        log = ('INFO scene_runner::automatic_testing: test a: b: true [1 remaining]\n'
               'INFO scene_runner::automatic_testing: test c: d: false [0 remaining]\n'
               'INFO scene_runner::automatic_testing: test e: f: false [0 remaining]\n')
        with tempfile.TemporaryDirectory() as temporary, patch.object(ci, 'ROOT', Path(temporary)):
            suite = Path(temporary) / ci.SCENE_SUITE
            suite.parent.mkdir(parents=True)
            suite.write_text('52,-52/c: d/g: h\n52,-62\n')
            text = ci.scene_summary('full', 'failure', log)
            self.assertIn('1 assertion passed, 2 failed; the run stopped before its end', text)
            self.assertIn('not determined', text)
            for absent in ('of them are listed', 'Not listed', 'remove them'):
                self.assertNotIn(absent, text)

    def test_summary_matches_escaped_names_and_skips_discarded_failures(self):
        log = ('INFO scene_runner::automatic_testing: test say "hi"\\now: false [0 remaining]\n'
               'INFO scene_runner::automatic_testing: test outside: false [0 remaining]\n'
               'WARN scene_runner::automatic_testing: location (52,-60) wasn\'t part of the required set, ignoring this failure\n'
               'INFO scene_runner::automatic_testing: some tests failed:\n [\n'
               '    (\n        "[(52,-52) : say \\"hi\\"\\\\now]",\n        "Error: image not found (it has been created)",\n'
               '        true,\n    ),\n]\n'
               'INFO scene_runner::automatic_testing: all failures were allowed\n')
        with tempfile.TemporaryDirectory() as temporary, patch.object(ci, 'ROOT', Path(temporary)):
            suite = Path(temporary) / ci.SCENE_SUITE
            suite.parent.mkdir(parents=True)
            suite.write_text('52,-52/say "hi"\\now\n')
            text = ci.scene_summary('full', 'success', log)
            self.assertIn('0 assertions passed, 1 failed; 1 of them are listed', text)
            self.assertIn('the runner discarded 1 failure outside the suite', text)
            for absent in ('Not listed', 'remove them'):
                self.assertNotIn(absent, text)
            suite.write_text('52,-52/say "hi"\\now/other\n')
            self.assertIn('remove them: (52,-52) other', ci.scene_summary('full', 'success', log))
            unlisted = log.replace('        true,', '        false,')
            self.assertIn('Not listed: (52,-52) say "hi"\\now', ci.scene_summary('full', 'failure', unlisted))
            suite.write_text('# nothing\n')
            with self.assertRaisesRegex(ValueError, 'Empty full scene suite'):
                ci.scene_suite('full')

    def test_summary_reads_the_runner_log_with_its_colour_codes(self):
        line = ('\x1b[2m2026-10-31T23:51:03.032696Z\x1b[0m \x1b[32m INFO\x1b[0m '
                '\x1b[2mscene_runner::automatic_testing\x1b[0m\x1b[2m:\x1b[0m ')
        log = line + 'test a: b: true [0 remaining]\n' + line + 'all tests passed!\n'
        with tempfile.TemporaryDirectory() as temporary, patch.object(ci, 'ROOT', Path(temporary)):
            suite = Path(temporary) / ci.SCENE_SUITE
            suite.parent.mkdir(parents=True)
            suite.write_text('52,-52\n')
            self.assertIn('1 assertion passed, 0 failed; graphical acceptance holds',
                          ci.scene_summary('full', 'success', log))

    def test_scenes_workflow_builds_its_suite_from_the_listed_file(self):
        text = (Path(__file__).parents[2] / ci.SCENES_WORKFLOW).read_text()
        self.assertIn('bevy-ci.py scene-suite --suite "$SCENE_SUITE"', text)
        self.assertIn('bevy-ci.py scene-summary --suite "$SCENE_SUITE" --outcome "$SCENE_OUTCOME"', text)
        self.assertIn('--log scene-tests.log | tee -a "$GITHUB_STEP_SUMMARY"', text)
        self.assertNotIn('52,-52;', text)


class PublicationTest(unittest.TestCase):
    def test_only_successful_same_repository_main_ci_is_trusted(self):
        self.assertTrue(ci.trusted_run(source_run(), 'example/project'))
        for changed in ({'status': 'in_progress'}, {'conclusion': 'failure'}, {'event': 'pull_request'},
                        {'head_branch': 'dev'}, {'head_repository': {'full_name': 'fork/project'}},
                        {'path': '.github/workflows/unrelated.yml'}, {'head_sha': 'not-a-sha'}):
            with self.subTest(changed=changed):
                self.assertFalse(ci.trusted_run(source_run(**changed), 'example/project'))

    def test_products_are_selected_independently_from_successful_producers(self):
        self.assertEqual(ci.produced_artifacts(producers('skipped')), [])
        self.assertEqual(ci.produced_artifacts(producers()), ['web', 'headless'])
        for product in ci.PRODUCERS:
            jobs = producers('skipped')
            next(job for job in jobs if job['name'] == ci.PRODUCERS[product])['conclusion'] = 'success'
            self.assertEqual(ci.produced_artifacts(jobs), [product])
            entries = ci.image_matrix([product])['include']
            self.assertEqual({row['product'] for row in entries}, {product})
            self.assertEqual({row['arch'] for row in entries},
                             {'amd64', 'arm64'} if product == 'web' else {'amd64'})

    def test_missing_failed_duplicate_or_running_producer_never_allows_publication(self):
        invalid = [[], producers()[:1], producers() * 2, producers('failure'), producers('cancelled')]
        for outcome in ('failure', 'cancelled', 'timed_out'):
            mixed = producers()
            mixed[0]['conclusion'] = outcome
            invalid.append(mixed)
        running = producers()
        running[0]['status'] = 'in_progress'
        invalid.append(running)
        for jobs in invalid:
            with self.subTest(jobs=jobs), self.assertRaises(ValueError):
                ci.produced_artifacts(jobs)

    def test_baselines_are_per_lane_and_ignore_untrusted_or_skipped_jobs(self):
        runs = [source_run(id=44, event='pull_request'), source_run(id=43),
                source_run(id=42, head_sha='b' * 40), source_run(id=41, head_sha='c' * 40)]
        web_only = producers('skipped')
        web_only[0]['conclusion'] = 'success'
        native = producers() + [
            {'name': name, 'status': 'completed', 'conclusion': 'success'}
            for name in ('Clippy', *ci.NATIVE_JOBS)]
        with patch.object(ci, 'api', return_value={'workflow_runs': runs}), \
                patch.object(ci, 'run_jobs', side_effect=[producers('skipped'), web_only, native]) as jobs:
            expected = {**dict.fromkeys(ci.LANES, 'c' * 40), 'web': 'b' * 40}
            self.assertEqual(ci.main_baselines('example/project'), expected)
            self.assertEqual([call.args[1] for call in jobs.call_args_list], [43, 42, 41])

    def test_partial_or_duplicate_native_matrix_is_not_a_passing_baseline(self):
        jobs = [{'name': name, 'status': 'completed', 'conclusion': 'success'} for name in sorted(ci.NATIVE_JOBS)]
        self.assertTrue(ci.passed_lane(jobs, 'test'))
        linux = next(job for job in jobs if job['name'] == 'Test Suite (ubuntu-24.04)')
        self.assertTrue(ci.passed_lane([linux], 'test'))
        for invalid in ([], jobs[:2], jobs + [linux], [{**linux, 'conclusion': 'skipped'}]):
            self.assertFalse(ci.passed_lane(invalid, 'test'))

    def test_missing_baseline_or_incomplete_job_list_cannot_prove_a_pass(self):
        with patch.object(ci, 'api', return_value={'workflow_runs': []}):
            self.assertEqual(ci.main_baselines('example/project'), {})
        with patch.object(ci, 'api', return_value={'total_count': 3, 'jobs': producers()}), \
                self.assertRaises(ValueError):
            ci.run_jobs('example/project', 42)

    def test_source_command_reports_skip_without_relabeling_another_commit(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / 'output'
            env = {'GH_REPO': 'example/project', 'SOURCE_RUN': '42', 'GITHUB_REF': 'refs/heads/dev',
                   'GITHUB_SHA': 'b' * 40, 'GITHUB_OUTPUT': str(output), 'GITHUB_STEP_SUMMARY': ''}
            with patch.dict(os.environ, env), patch('sys.argv', ['bevy-ci.py', 'source']), \
                    patch.object(ci, 'api', return_value=source_run()), \
                    patch.object(ci, 'run_jobs', return_value=producers('skipped')):
                ci.main()
            text = output.read_text()
            self.assertIn('build=false\n', text)
            self.assertIn(f"sha={'a' * 40}\n", text)
            self.assertNotIn('b' * 40, text)

    def test_source_command_exports_only_the_successful_product_and_exact_source_sha(self):
        jobs = producers('skipped')
        jobs[0]['conclusion'] = 'success'
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / 'output'
            env = {'GH_REPO': 'example/project', 'SOURCE_RUN': '42',
                   'GITHUB_OUTPUT': str(output), 'GITHUB_STEP_SUMMARY': ''}
            with patch.dict(os.environ, env), patch('sys.argv', ['bevy-ci.py', 'source']), \
                    patch.object(ci, 'api', return_value=source_run()), \
                    patch.object(ci, 'run_jobs', return_value=jobs):
                ci.main()
            values = dict(line.split('=', 1) for line in output.read_text().splitlines())
            self.assertEqual(values['sha'], 'a' * 40)
            self.assertEqual(values['build'], 'true')
            self.assertEqual(json.loads(values['products']), ['web'])
            self.assertEqual(json.loads(values['build-matrix']), ci.image_matrix(['web']))


def load_tool(name):
    spec = importlib.util.spec_from_file_location(name.replace('-', '_'), Path(__file__).parents[1] / f'{name}.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def workflow_patterns(text):
    """Every entry of a `paths:` list and every argument of a hashFiles() call of a workflow or an action."""
    patterns, indent = set(), None
    for line in text.splitlines():
        stripped = line.strip()
        if stripped in ('paths:', 'paths-ignore:'):
            indent = len(line) - len(line.lstrip())
        elif indent is not None and stripped.startswith('- ') and len(line) - len(line.lstrip()) >= indent:
            patterns.add(stripped[2:].strip().strip('\'"'))
        elif stripped and not stripped.startswith('#'):
            indent = None
    for call in re.findall(r'hashFiles\(([^)]*)\)', text, re.S):
        patterns.update(re.findall(r"'([^']+)'", call))
    return {pattern.lstrip('!') for pattern in patterns}


class PathListTest(unittest.TestCase):
    """The Explorer's patch, in-tree crate and tool paths are repeated in workflow filters, cache
    keys, the labeler, the daily upstream run and the selection tools. A list that names a path the
    repository does not have watches nothing: a moved directory must fail here."""
    EXPLORER = 'rendering/bevy-explorer/'
    # Watched so that the file selects the builds when it is added; the repository has none today.
    WATCH_ONLY = {'rust-toolchain*'}

    def tracked(self, pattern):
        return bool(subprocess.run(['git', 'ls-files', '-z', '--', pattern], cwd=ci.ROOT, capture_output=True, check=True,
                                   env={**os.environ, 'GIT_GLOB_PATHSPECS': '1'}).stdout)

    def lists(self):
        found = {}
        for path in sorted((ci.ROOT / '.github').rglob('*.yml')):
            text = path.read_text(encoding='utf-8')
            patterns = workflow_patterns(text)
            if path.name == 'labeler.yml':
                patterns = set(re.findall(r"^ +- '([^']+)'$", text, re.M))
            patterns.update(re.findall(r'\S*/source\.toml', text))
            if any(pattern.startswith(self.EXPLORER) for pattern in patterns):
                found[path.relative_to(ci.ROOT).as_posix()] = patterns
        artifact, local = load_tool('play-artifact'), load_tool('ci-local')
        found['tooling/bevy-ci.py'] = {*ci.INPUTS, *ci.IMAGES_INPUTS, *ci.TOOLS_INPUTS, *ci.GATEWAY_HUD_INPUTS,
                                       *ci.HARNESS_WEB_READS, *ci.PROVENANCE_INPUTS, ci.PATCHES, ci.SOURCE,
                                       *('apps/bevy-explorer/' + name for name in ci.APP_INPUTS)}
        found['tooling/play-artifact.py'] = {*artifact.INPUTS, artifact.SOURCE, artifact.VENDOR,
                                             artifact.PROTO, artifact.HUD, artifact.BINDINGS}
        found['tooling/ci-local.py'] = set(local.workflow_paths('bevy-explorer-headless.yml', source=ci.ROOT))
        return found

    def missing(self, lists):
        return sorted((name, pattern) for name, patterns in lists.items() for pattern in patterns
                      if pattern not in self.WATCH_ONLY and not self.tracked(pattern))

    def test_every_path_list_names_paths_of_the_repository(self):
        lists = self.lists()
        for name in (ci.WORKFLOW, ci.SCENES_WORKFLOW, '.github/workflows/bevy-explorer-e2e.yml', '.github/labeler.yml',
                     '.github/workflows/bevy-explorer-upstream.yml'):
            self.assertIn(name, lists)
            self.assertTrue(any('bevy-explorer/patches' in pattern or pattern == 'rendering/bevy-explorer/**' for pattern in lists[name]), name)
        self.assertEqual(self.missing(lists), [])

    def test_a_watch_only_entry_names_nothing_yet_and_is_in_use(self):
        used = set().union(*self.lists().values())
        for pattern in self.WATCH_ONLY:
            self.assertIn(pattern, used)
            self.assertFalse(self.tracked(pattern), f'{pattern} exists now: remove it from WATCH_ONLY')

    def test_a_list_that_names_a_moved_directory_fails(self):
        lists = {'workflow': workflow_patterns("    paths:\n      - 'rendering/bevy-explorer/forks/**'\n      - rendering/bevy-explorer/patches/**\n"
                                              "    steps:\n      - run: echo ${{ hashFiles('rendering/bevy-explorer/source/vendor/enet/**', '!flake.lock') }}\n")}
        self.assertEqual(self.missing(lists), [('workflow', 'rendering/bevy-explorer/forks/**'),
                                               ('workflow', 'rendering/bevy-explorer/source/vendor/enet/**')])


if __name__ == '__main__':
    unittest.main()
