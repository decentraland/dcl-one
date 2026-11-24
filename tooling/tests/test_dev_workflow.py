"""Development CI keeps platform coverage and reusable, isolated build snapshots."""
from pathlib import Path
import json
import os
import re
import runpy
import subprocess
import tempfile
import textwrap
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from tooling.tests.cache_cleanup_fixture import cache_cleanup_api


WORKFLOW = Path(__file__).parents[2] / '.github/workflows/development.yml'
CLEANUP = WORKFLOW.with_name('closed-pr-cache.yml')
CREATOR_HUB = WORKFLOW.with_name('creator-hub-ci.yml')


def jobs():
    source = WORKFLOW.read_text().split('jobs:\n', 1)[1]
    return dict(re.findall(r'(?ms)^  (\w+):\n(.*?)(?=^  \w+:|\Z)', source))


def disk_steps():
    """The script of each disk step in the three workflows' Rust jobs, by job."""
    found = {}
    sources = {name: body for name, body in jobs().items() if name in ('rust', 'smoke')}
    sources['social-voice-release'] = WORKFLOW.with_name('social-voice-release.yml').read_text()
    for name, body in sources.items():
        step = re.search(r"(?ms)^      - name: Reserve disk space for the Rust test build\n"
                         r"        if: runner\.os == 'Linux' && runner\.environment == 'github-hosted'\n        run: \|\n(.*?)(?=^      - )", body)
        found[name] = (textwrap.dedent(step.group(1)), body.index(step.group(0)) < body.index('uses: actions/checkout@'))
    return found


def runner_jobs(path):
    """Each job of a workflow: its text, the variables its runner reads, whether it is privileged."""
    text = path.read_text()
    shared = re.search(r"(?ms)^permissions:(.*?)(?=^[\w']|\Z)", text)
    body = re.search(r"(?ms)^jobs:\n(.*?)(?=^[\w']|\Z)", text)[1]
    found = {}
    for job in re.split(r'(?m)^(?=  [\w-]+:\s*(?:#.*)?\n)', body):
        runners = re.findall(r'(?ms)^    runs-on:(.*?)(?=^    \S|\Z)', job)
        if any('matrix.' in runner for runner in runners):
            runners += re.findall(r'(?ms)^    strategy:(.*?)(?=^    \S|\Z)', job)
        own = re.search(r'(?ms)^    permissions:(.*?)(?=^    \S|\Z)', job) or shared
        privileged = (own is None or re.search(r':\s*write\b|write-all', own[1])
                      or re.search(r'(?m)^    environment:', job)
                      or re.search(r'secrets\.(?!GITHUB_TOKEN\b)\w', job))
        names = [name for runner in runners for name in re.findall(r'\bvars\W*(\w*)', runner)]
        found[job.split(':', 1)[0].strip()] = (job, names, bool(privileged))
    return found


class DevelopmentWorkflowTest(unittest.TestCase):
    def test_the_disk_step_deletes_the_unused_sdks_only_at_25_gb_free_or_less(self):
        steps = disk_steps()
        self.assertEqual(set(steps), {'rust', 'smoke', 'social-voice-release'})
        with tempfile.TemporaryDirectory() as temp:
            stubs = Path(temp)
            # df prints the header and the free space as the real one does; sudo records its arguments.
            (stubs / 'df').write_text('#!/bin/sh\n[ "$1" = --output=avail ] && printf \' Avail\\n%5sG\\n\' "$FREE_GB"\nexit 0\n')
            (stubs / 'sudo').write_text('#!/bin/sh\necho "$@" >> "$SUDO_LOG"\n')
            for stub in stubs.iterdir():
                stub.chmod(0o755)
            for name, (script, first) in steps.items():
                self.assertTrue(first, f'{name}: the step runs before the checkout fills the disk')
                for free, deletes in (('5', True), ('25', True), ('26', False), ('112', False), ('', True)):
                    log = stubs / f'{name}-{free or "unknown"}.log'
                    env = {'PATH': f'{stubs}{os.pathsep}{os.environ["PATH"]}', 'FREE_GB': free, 'SUDO_LOG': str(log)}
                    subprocess.run(['bash', '-euc', script], env=env, check=True, capture_output=True)
                    self.assertEqual(log.exists(), deletes, f'{name} with {free or "unknown"} GB free')
                    if deletes:
                        self.assertEqual(log.read_text(), 'rm -rf /usr/local/lib/android /usr/share/dotnet '
                                         '/usr/share/swift /opt/ghc /usr/local/.ghcup\n')

    def test_every_workflow_deletes_the_sdk_folders_only_below_a_free_space_limit(self):
        delete = 'sudo rm -rf /usr/local/lib/android /usr/share/dotnet /usr/share/swift /opt/ghc /usr/local/.ghcup'
        seen = 0
        for path in sorted(WORKFLOW.parent.glob('*.y*ml')):
            lines = [line.strip() for line in path.read_text().splitlines()]
            for number, line in enumerate(lines):
                if line == delete:
                    seen += 1
                    with self.subTest(workflow=path.name, line=number + 1):
                        self.assertRegex(lines[number - 1], r'^if \[ "\$(\{avail:-0\}|available)" -l[te] \S.* \]; then$')
        self.assertGreaterEqual(seen, 12)

    def test_rust_caches_dependencies_only_and_smoke_keeps_a_snapshot_on_both_platforms(self):
        rust, smoke = jobs()['rust'], jobs()['smoke']
        for job in (rust, smoke):
            # Both platforms on main; PRs add macOS with the ci:macos label.
            self.assertRegex(job, r'os: (\[ubuntu-26\.04, macos-15\]|.*"ubuntu-26\.04", "macos-15")')
        # The shards cover every test executable: the matrix lists 1 to n and the script gets i/n.
        self.assertIn('        shard: [1, 2, 3]\n', rust)
        self.assertIn('      CATALYST_TEST_SHARD: ${{ matrix.shard }}/3\n', rust)
        # One of those shards runs the provisioned Archipelago tests; tooling/dev-regressions.py reads the setting.
        self.assertIn("      CATALYST_TEST_PROVISIONED_SHARD: '1'\n", rust)
        self.assertEqual(rust.count('CATALYST_TEST_PROVISIONED_SHARD:'), 1)
        # One shard runs the doc-tests; tooling/catalyst-test.sh rejects a shard outside 1 to n.
        self.assertIn("      CATALYST_TEST_DOC_SHARD: '2'\n", rust)
        self.assertEqual(rust.count('CATALYST_TEST_DOC_SHARD:'), 1)
        # The dependency cache: the action prunes workspace output before saving, main writes, PRs read.
        self.assertNotIn('actions/cache', rust)
        cache = re.search(r'(?ms)^      - name: Restore the Rust dependency cache\n(.*?)(?=^      - |\Z)', rust).group(1)
        self.assertIn('uses: Swatinem/rust-cache@', cache)
        self.assertIn('workspaces: services/catalyst', cache)
        self.assertIn("save-if: ${{ github.ref == 'refs/heads/main' && github.run_attempt == '1' && matrix.shard == 1 }}", cache)
        self.assertIn('cache-on-failure: true', cache)
        self.assertLess(rust.index('Put the Nix Rust toolchain on PATH'), rust.index('Swatinem/rust-cache@'))
        self.assertLess(rust.index('Swatinem/rust-cache@'), rust.index('bash tooling/dev-test.sh rust'))
        # The compiler cache: optional, pinned, and on only when the dependency cache restored an entry
        # (exact or partial key) and the server started, so it holds what that entry does not and a
        # failure of the cache does not fail the tests. The wrapper is set in the test script only,
        # because the dependency cache key hashes the job environment.
        self.assertIn('        id: deps\n        uses: Swatinem/rust-cache@', rust)
        tool = re.search(r'(?ms)^      - name: Install the optional compiler cache\n(.*?)(?=^      - |\Z)', rust).group(1)
        self.assertIn('continue-on-error: true', tool)
        self.assertRegex(tool, r'uses: Mozilla-Actions/sccache-action@[0-9a-f]{40} # v')
        self.assertNotRegex(rust, r'(?m)^ +RUSTC_WRAPPER: ')
        tests = re.search(r'(?ms)^      - name: Run backend tests with PostgreSQL and isolated NATS\n(.*?)(?=^      - |\Z)', rust).group(1)
        restored = re.search(r'(?ms)^      - name: Check whether the dependency cache restored an entry\n(.*?)(?=^      - |\Z)', rust).group(1)
        self.assertIn('id: deps-restored', restored)
        self.assertIn('[ -d services/catalyst/target/debug/deps ]', restored)
        self.assertLess(rust.index('Swatinem/rust-cache@'), rust.index('id: deps-restored'))
        self.assertIn("USE_COMPILER_CACHE: ${{ steps.deps-restored.outputs.restored == 'true' && steps.compiler-cache.outcome == 'success' }}", tests)
        self.assertIn('if [ "$USE_COMPILER_CACHE" = true ] && sccache --start-server; then\n            export RUSTC_WRAPPER=sccache\n', tests)
        self.assertEqual(tests.count('RUSTC_WRAPPER=sccache'), 1)
        self.assertLess(tests.index('export RUSTC_WRAPPER=sccache'), tests.index('bash tooling/dev-test.sh rust'))
        self.assertIn('nix develop .#ci --command bash tooling/dev-test.sh rust || status=$?', tests)
        self.assertIn('exit "$status"', tests)
        self.assertLess(rust.index('Swatinem/rust-cache@'), rust.index('Mozilla-Actions/sccache-action@'))
        self.assertLess(rust.index('Mozilla-Actions/sccache-action@'), rust.index('bash tooling/dev-test.sh rust'))
        keys = re.findall(r'^          key: (foundation-dev-v2-[^\n]+)', smoke, re.M)
        self.assertEqual(len(keys), 1)
        self.assertTrue(keys[0].startswith('foundation-dev-v2-smoke-'))
        self.assertIn('steps.rust-inputs.outputs.source', keys[0])
        self.assertIn("hashFiles('services/catalyst/Cargo.lock')", keys[0])
        saves = re.findall(r'(?ms)^      - name: Save successful[^\n]*\n(.*?)(?=^      - |\Z)', smoke)
        self.assertEqual(len(saves), 1)
        self.assertIn('if: success()', saves[0])
        self.assertIn("github.event_name == 'push'", saves[0])
        self.assertIn("github.ref == 'refs/heads/main'", saves[0])
        self.assertIn('steps.rust-cache.outputs.cache-primary-key', saves[0])
        self.assertIn('bash tooling/dev-smoke.sh', smoke)
        self.assertIn('bash tooling/dev-test.sh e2e', smoke)

    def test_header_geometry_control_requires_the_original_missing_port_evidence(self):
        web = jobs()['web']
        original = re.search(r'(?ms)^      - name: Run Sites unit tests\n(.*?)(?=^      - |\Z)', web).group(1)
        self.assertIn('id: sites-tests', original)
        self.assertIn('run: nix develop --command pnpm --dir apps/sites test\n', original)
        self.assertNotIn('continue-on-error', original)
        control = re.findall(r'(?ms)^      - name: Diagnose isolated header geometry startup\n(.*?)(?=^      - |\Z)', web)
        self.assertEqual(len(control), 1)
        control = control[0]
        self.assertIn('timeout-minutes: 4', control)
        self.assertNotIn('continue-on-error', control)
        self.assertNotIn('|', control)
        self.assertIn('run: nix develop --command pnpm --dir apps/sites test src/features/components/chrome/header-geometry.test.ts\n', control)
        self.assertLess(web.index('name: Run Sites unit tests'), web.index('name: Diagnose isolated header geometry startup'))
        expression = re.search(r'^        if: \$\{\{ (.*?) \}\}$', control, re.M).group(1)
        expression = expression.replace('&&', 'and')
        expression = expression.replace('steps.sites-tests.outcome', 'outcome')
        expression = expression.replace('steps.sites-tests.outputs.capture-missing-port', 'evidence')
        for failed, outcome, evidence, expected in (
            (True, 'failure', 'true', True),
            (True, 'failure', '', False),
            (True, 'success', 'true', False),
            (False, 'cancelled', 'true', False),
            (False, 'success', '', False),
        ):
            with self.subTest(failed=failed, outcome=outcome, evidence=evidence):
                self.assertEqual(bool(eval(expression, {'__builtins__': {}}, {
                    'failure': lambda: failed, 'outcome': outcome, 'evidence': evidence,
                })), expected)

    def test_no_privileged_job_takes_its_runner_from_a_variable(self):
        # The RUNNER_* variables can name a fork's own runners. A job with a write permission, an
        # environment or a secret names its hosted label instead, so its token and its secrets
        # stay off those runners. A job with no permissions block counts as privileged: its
        # token is the repository default or its caller's. The GPU scenarios job is the one
        # exception, on its own switch. The jobs on the variables still supply what privileged
        # jobs publish: docs/development.md, "CI runners", states what their runners must be.
        found, unset = set(), set()
        for path in sorted(WORKFLOW.parent.glob('*.y*ml')):
            # A called workflow names each secret it reads, so the secret check finds it.
            self.assertNotIn('secrets: inherit', path.read_text())
            for name, (job, names, privileged) in runner_jobs(path).items():
                if names and privileged:
                    found.add(f'{path.name}:{name}')
                # A job on a variable names the hosted label as the fallback of each variable.
                for runner in re.findall(r'(?ms)^    runs-on:(.*?)(?=^    \S|\Z)', job):
                    if len(re.findall(r'\bvars\.RUNNER_\w+', runner)) != len(re.findall(r"\bvars\.RUNNER_\w+ \|\| '[\w.-]+'", runner)):
                        unset.add(f'{path.name}:{name}')
        self.assertEqual(found, {'bevy-explorer-e2e.yml:scenarios'})
        self.assertEqual(unset, set())

    def test_the_tooling_tests_run_beside_smoke_on_the_same_platforms(self):
        # They need only the dev shell, so smoke does not wait for them and they skip its caches.
        found = jobs()
        smoke, tooling = found['smoke'], found['tooling']
        suite = 'nix develop --command python3 -m unittest discover -s tooling/tests -v\n'
        self.assertEqual(sum(body.count('unittest discover -s tooling/tests -v') for body in found.values()), 1)
        self.assertIn(suite, tooling)
        self.assertIn('    name: Tooling tests\n', tooling)
        self.assertIn('    needs: changes\n', tooling)
        head = lambda body: body.split('    timeout-minutes:', 1)[0].split('    if:', 1)[1]
        self.assertEqual(head(tooling), head(smoke))
        for cache in ('Restore Rust build cache', 'Restore the pnpm store', 'Reserve disk space', 'cargo-mtimes.py'):
            self.assertNotIn(cache, tooling)
        self.assertNotRegex(tooling, r'(?m)^    (env|permissions):')

    def test_smoke_snapshot_preserves_all_active_library_variants(self):
        prune = runpy.run_path(str(WORKFLOW.parents[2] / "tooling/catalyst-smoke-cache.py"))["prune"]
        with tempfile.TemporaryDirectory() as temporary:
            deps = Path(temporary)
            stems = ["libcrate-" + str(index) * 16 for index in range(3)]
            for stem in stems:
                for suffix in (".rlib", ".rmeta"):
                    (deps / (stem + suffix)).write_bytes(b"data")
            other = deps / ("libother-" + "3" * 16 + ".rlib")
            other.touch()
            def message(stem):
                return json.dumps({"reason": "compiler-artifact", "filenames": [str(deps / (stem + ".rlib"))]})
            receipts = "\n".join(message(stem) for stem in stems[:2])
            self.assertEqual(prune(deps, receipts), 8)
            self.assertEqual(prune(deps, receipts), 0)
            self.assertEqual({path.name for path in deps.iterdir()},
                             {stem + suffix for stem in stems[:2] for suffix in (".rlib", ".rmeta")} | {other.name})
            with self.assertRaises(ValueError):
                prune(deps, "")

    def test_smoke_prunes_verified_binary_and_dynamic_library_variants(self):
        prune = runpy.run_path(str(WORKFLOW.parents[2] / "tooling/catalyst-smoke-cache.py"))["prune"]
        with tempfile.TemporaryDirectory() as temporary:
            deps = Path(temporary) / 'deps'
            deps.mkdir()
            app = deps.parent / 'catalyst'
            app.write_bytes(b'current')
            current = deps / ('catalyst-' + 'a' * 16)
            old = deps / ('catalyst-' + 'b' * 16)
            old.write_bytes(b'old')
            other = deps / ('other-' + 'b' * 16)
            other.touch()
            libraries = []
            for suffix in ('.so', '.dylib'):
                active = deps / ('libmacro-' + 'a' * 16 + suffix)
                stale = deps / ('libmacro-' + 'b' * 16 + suffix)
                active.touch()
                libraries.append(str(active))
                stale.write_bytes(b'old')
            library_receipts = json.dumps({'reason': 'compiler-artifact', 'filenames': libraries})
            os.link(app, current)
            # The smoke profile disables debug info, so this receipt lists only the executable.
            bin_receipt = json.dumps({'reason': 'compiler-artifact', 'target': {'kind': ['bin']},
                                      'filenames': [str(app)], 'executable': str(app)})
            receipts = library_receipts + '\n' + bin_receipt
            self.assertEqual(prune(deps, receipts), 9)
            self.assertTrue(all(Path(lib).exists() for lib in libraries))
            self.assertFalse(old.exists())
            self.assertTrue(current.samefile(app))
            self.assertTrue(other.exists())
            self.assertEqual(prune(deps, receipts), 0)

    def test_smoke_reports_when_hashed_binaries_are_kept(self):
        prune = runpy.run_path(str(WORKFLOW.parents[2] / "tooling/catalyst-smoke-cache.py"))["prune"]
        for reported in (False, True):
            with self.subTest(app_reported=reported), tempfile.TemporaryDirectory() as temporary:
                deps = Path(temporary) / 'deps'
                deps.mkdir()
                app = deps.parent / 'catalyst'
                app.write_bytes(b'current')
                candidate = deps / ('catalyst-' + 'a' * 16)
                lib = deps / ('libcrate-' + 'a' * 16 + '.rlib')
                lib.touch()
                stale = deps / ('libcrate-' + 'b' * 16 + '.rlib')
                stale.write_bytes(b'old')
                filenames = [str(lib)]
                if reported:
                    filenames.append(str(app))
                    candidate.write_bytes(b'unmatched')
                else:
                    os.link(app, candidate)
                receipt = json.dumps({'reason': 'compiler-artifact', 'filenames': filenames})
                # Keeping the binaries does not stop the superseded library from being pruned.
                with patch('builtins.print') as diagnostic:
                    self.assertEqual(prune(deps, receipt), 3)
                    diagnostic.assert_called_once()
                    self.assertIn('Kept all 1 hashed Catalyst', diagnostic.call_args.args[0])
                self.assertTrue(candidate.exists())
                self.assertTrue(lib.exists())
                self.assertFalse(stale.exists())

    def test_smoke_prunes_a_dangling_library_symlink(self):
        prune = runpy.run_path(str(WORKFLOW.parents[2] / "tooling/catalyst-smoke-cache.py"))["prune"]
        with tempfile.TemporaryDirectory() as temporary:
            deps = Path(temporary)
            active = deps / ('libcrate-' + 'a' * 16 + '.so')
            active.touch()
            obsolete = deps / ('libcrate-' + 'b' * 16 + '.so')
            obsolete.symlink_to('missing-library')
            link_size = obsolete.lstat().st_size
            receipt = json.dumps({'reason': 'compiler-artifact', 'filenames': [str(active)]})
            self.assertEqual(prune(deps, receipt), link_size)
            self.assertFalse(obsolete.is_symlink())
            self.assertTrue(active.exists())

    def test_smoke_counts_only_the_last_unlinked_name(self):
        prune = runpy.run_path(str(WORKFLOW.parents[2] / "tooling/catalyst-smoke-cache.py"))["prune"]
        with tempfile.TemporaryDirectory() as temporary:
            deps = Path(temporary) / 'deps'
            deps.mkdir()
            active, obsolete, linked = [deps / ('libcrate-' + c * 16 + '.rlib') for c in 'abc']
            active.touch()
            obsolete.write_bytes(b'old')
            os.link(obsolete, obsolete.with_suffix('.rmeta'))
            linked.write_bytes(b'shared')
            survivor = deps.parent / 'retained'
            os.link(linked, survivor)
            receipt = json.dumps({'reason': 'compiler-artifact', 'filenames': [str(active)]})
            self.assertEqual(prune(deps, receipt), 3)
            self.assertEqual(set(deps.iterdir()), {active})
            self.assertEqual(survivor.read_bytes(), b'shared')

    def test_smoke_recognizes_copied_binaries_by_contents(self):
        prune = runpy.run_path(str(WORKFLOW.parents[2] / "tooling/catalyst-smoke-cache.py"))["prune"]
        with tempfile.TemporaryDirectory() as temporary:
            deps = Path(temporary) / 'deps'
            deps.mkdir()
            app = deps.parent / 'catalyst'
            app.write_bytes(b'current')
            first, second, stale = [deps / ('catalyst-' + c * 16) for c in 'abc']
            for copy in (first, second):
                copy.write_bytes(app.read_bytes())
            stale.write_bytes(b'expired')
            stamp = app.stat().st_mtime_ns
            os.utime(stale, ns=(stamp, stamp))
            lib = deps / ('libcrate-' + 'a' * 16 + '.rlib')
            lib.touch()
            receipts = json.dumps({'reason': 'compiler-artifact', 'filenames': [str(lib), str(app)]})
            self.assertEqual(prune(deps, receipts), 7)
            self.assertEqual(set(deps.iterdir()), {first, second, lib})

    def test_smoke_gives_unchanged_sources_the_snapshot_file_times(self):
        # Cargo decides by file time. The snapshot is of no use to Cargo unless the checkout's
        # unchanged files get the times they had when the snapshot was built.
        smoke = jobs()['smoke']
        restore = '          python3 tooling/cargo-mtimes.py restore services/catalyst/target\n'
        record = '        run: python3 tooling/cargo-mtimes.py record services/catalyst/target\n'
        self.assertEqual(smoke.count(restore), 1)
        self.assertEqual(smoke.count(record), 1)
        self.assertLess(smoke.index('- name: Restore Rust build cache\n'), smoke.index(restore))
        self.assertLess(smoke.index(restore), smoke.index('bash tooling/dev-smoke.sh'))
        self.assertLess(smoke.index('bash tooling/dev-test.sh e2e'), smoke.index(record))
        self.assertLess(smoke.index(record), smoke.index('- name: Save successful smoke build snapshot\n'))
        # The manifest is written only into a snapshot that the next step saves.
        steps = {step.split('\n', 1)[0]: step for step in re.split(r'(?m)^      - name: ', smoke)}
        condition = lambda name: re.search(r'(?m)^        if: (.+)$', steps[name]).group(1)
        self.assertEqual(condition('Record the source file times in the snapshot'),
                         condition('Save successful smoke build snapshot'))
        self.assertEqual(condition('Prune superseded Rust build artifacts from the smoke snapshot'),
                         condition('Save successful smoke build snapshot'))
        self.assertLess(smoke.index('python3 tooling/catalyst-smoke-cache.py'), smoke.index(record))
        self.assertNotIn('        if:', steps['Give unchanged sources the file times of the snapshot'])

    def test_every_nix_job_restores_the_store_snapshot_before_its_first_shell(self):
        policy = WORKFLOW.parents[2] / 'tooling/repo-actions-policy.sh'
        self.assertIn("patterns_allowed[]=nix-community/cache-nix-action@*", policy.read_text())
        installs = 0
        for name in ('development.yml', 'social-voice-release.yml'):
            jobs = WORKFLOW.with_name(name).read_text().split('\njobs:\n', 1)[1]
            for job in re.split(r'(?m)^  (?=[\w-]+:\n)', jobs):
                steps = re.split(r'(?m)^      - ', job)
                for i, step in enumerate(steps):
                    if not step.startswith('uses: cachix/install-nix-action@'):
                        continue
                    installs += 1
                    snapshot, roots = steps[i + 1], steps[i + 2]
                    shell = 'ci' if 'dev-test.sh rust' in job or 'social-voice-release-test.sh' in job else 'default'
                    key = 'nix-store-${{ runner.os }}-${{ runner.arch }}-' + shell + '-'
                    self.assertTrue(snapshot.startswith('uses: nix-community/cache-nix-action@'), name)
                    self.assertIn('primary-key: ' + key + "${{ hashFiles('flake.nix', 'flake.lock', ", snapshot)
                    self.assertIn('restore-prefixes-first-match: ' + key + '\n', snapshot)
                    # One Rust test shard and the web job's ui3 leg save; the other jobs save from every main run.
                    self.assertIn("save: ${{ github.ref == 'refs/heads/main' && github.run_attempt == '1'"
                                  + (' && matrix.shard == 1 }}' if 'shard: [' in job else
                                     " && matrix.app == 'ui3' }}" if 'app: [sites, ui3]' in job else ' }}'), snapshot)
                    self.assertIn('gc-max-store-size: 0', snapshot)
                    self.assertIn('mkdir -p .cache/nix-roots\n', roots)
                    self.assertIn('nix develop ' + ('.#ci ' if shell == 'ci' else '') + '--profile .cache/nix-roots/env --command true', roots)
                    self.assertIn('nix flake archive --json', roots)
                    for command in re.findall(r'nix develop(?: \.#\w+)?', job):
                        self.assertEqual(command, 'nix develop .#ci' if shell == 'ci' else 'nix develop', name)
        self.assertEqual(installs, 5)

    def test_smoke_snapshots_are_saved_only_from_passing_main_pushes(self):
        def allowed(event, ref, platform, passed=True, hit=False):
            save = re.search(r'(?ms)^      - name: Save successful[^\n]*\n(.*?)(?=^      - |\Z)',
                             jobs()['smoke']).group(1)
            expression = re.search(r'^        if: (.+)$', save, re.M).group(1)
            expression = expression.replace('&&', 'and').replace('||', 'or')
            expression = expression.replace('steps.rust-cache.outputs.cache-hit', 'cache_hit')
            github = SimpleNamespace(event_name=event, ref=ref, repository='decentraland/dcl-one')
            return eval(expression, {'__builtins__': {}}, {
                'github': github, 'runner': SimpleNamespace(os=platform),
                'success': lambda: passed, 'cache_hit': 'true' if hit else 'false',
            })

        for platform in ('Linux', 'macOS'):
            with self.subTest(platform=platform):
                self.assertTrue(allowed('push', 'refs/heads/main', platform))
                self.assertFalse(allowed('push', 'refs/heads/feature', platform))
                self.assertFalse(allowed('workflow_dispatch', 'refs/heads/main', platform))
                self.assertFalse(allowed('push', 'refs/heads/main', platform, passed=False))
                self.assertFalse(allowed('push', 'refs/heads/main', platform, hit=True))
                self.assertFalse(allowed('pull_request', 'refs/pull/150/merge', platform))

    def test_closed_pr_cleanup_deletes_only_exact_merge_ref_from_every_page(self):
        source = CLEANUP.read_text()
        self.assertIn('pull_request:\n    types: [closed]', source)
        self.assertIn('permissions: {}', source)
        self.assertNotIn('checkout', source.split('steps:', 1)[1].replace('checks out', ''))
        self.assertNotIn('uses:', source)
        script = textwrap.dedent(source.split("python3 - <<'PY'\n", 1)[1].rsplit('\n          PY', 1)[0])
        self.assertNotIn('${{', script)
        pages = [{'actions_caches': [
            {'id': 1, 'ref': 'refs/heads/main'},
            {'id': 2, 'ref': 'refs/pull/151/merge'},
            {'id': 3, 'ref': 'refs/pull/150/merge'},
        ]}, {'actions_caches': [
            {'id': 4, 'ref': 'refs/pull/150/head'},
            {'id': 5, 'ref': 'refs/pull/150/merge'},
            {'id': 'unsafe', 'ref': 'refs/pull/150/merge'},
        ]}]
        with patch.dict(os.environ, {'PR_NUMBER': '150', 'GITHUB_REPOSITORY': 'decentraland/dcl-one',
                                     'GITHUB_EVENT_NAME': 'pull_request'}), \
                patch('subprocess.run', side_effect=cache_cleanup_api(pages)) as cli, patch('builtins.print'):
            exec(compile(script, str(CLEANUP), 'exec'), {})
            request = next(call.args[0] for call in cli.call_args_list if '--paginate' in call.args[0])
            self.assertIn('ref=refs/pull/150/merge', request)
            self.assertIn('--paginate', request)
            self.assertEqual([call.args[0][-1] for call in cli.call_args_list if 'DELETE' in call.args[0]], [
                'repos/decentraland/dcl-one/actions/caches/3',
                'repos/decentraland/dcl-one/actions/caches/5',
            ])
            for number in ('', '0', '-1', '../main', '150; echo unsafe'):
                with self.subTest(number=number), patch.dict(os.environ, {'PR_NUMBER': number}):
                    cli.reset_mock()
                    with self.assertRaises(ValueError):
                        exec(compile(script, str(CLEANUP), 'exec'), {})
                    cli.assert_not_called()

    def test_scheduled_sweep_catches_late_uploads_and_bounds_dev_and_rust_caches(self):
        source = CLEANUP.read_text()
        self.assertIn("cron: '17 * * * *'", source)
        script = textwrap.dedent(source.split("python3 - <<'PY'\n", 1)[1].rsplit('\n          PY', 1)[0])
        prefix = 'foundation-dev-v2-smoke-decentraland/dcl-one-Linux-X64-'
        deps = 'v0-rust-dev-tests-'
        bevy = 'v0-rust-bevy-explorer-native-ci-test-test-Linux-x64-'
        caches = [
            {'id': 1, 'ref': 'refs/heads/main', 'key': prefix + 'old'},
            {'id': 2, 'ref': 'refs/heads/main', 'key': prefix + 'new'},
            {'id': 3, 'ref': 'refs/heads/main', 'key': 'images-v2-main-exact'},
            {'id': 4, 'ref': 'refs/pull/10/merge', 'key': prefix + 'late'},
            {'id': 5, 'ref': 'refs/pull/10/merge', 'key': 'images-v2-pr-10-late'},
            {'id': 6, 'ref': 'refs/pull/11/merge', 'key': prefix + 'old'},
            {'id': 7, 'ref': 'refs/pull/11/merge', 'key': prefix + 'new'},
            {'id': 8, 'ref': 'refs/pull/11/merge', 'key': 'images-v2-pr-11-exact'},
            {'id': 9, 'ref': 'refs/pull/10/head', 'key': prefix + 'ignored'},
            {'id': 10, 'ref': 'refs/heads/other', 'key': prefix + 'ignored'},
            {'id': 11, 'ref': 'refs/heads/main', 'key': 'bevy-cache'},
            {'id': 12, 'ref': 'refs/heads/main', 'key': deps + '0' * 64 + '-Linux-x64-previous-flake'},
            {'id': 13, 'ref': 'refs/heads/main', 'key': deps + 'f' * 64 + '-Linux-x64-new'},
            {'id': 14, 'ref': 'refs/heads/main', 'key': deps + 'f' * 64 + '-Darwin-arm64-only'},
            {'id': 15, 'ref': 'refs/heads/main', 'key': bevy + 'old'},
            {'id': 16, 'ref': 'refs/heads/main', 'key': bevy + 'new'},
            {'id': 17, 'ref': 'refs/pull/11/merge', 'key': bevy + 'old'},
            {'id': 18, 'ref': 'refs/pull/11/merge', 'key': bevy + 'new'},
            {'id': 19, 'ref': 'refs/heads/main', 'key': 'v0-rust-e2e-scene-cli-scene-cli-Windows_NT-x64-only'},
            {'id': 20, 'ref': 'refs/heads/main', 'key': 'bevy-web-unopt-v1-main-' + 'a' * 64 + '-' + '1' * 64},
            {'id': 21, 'ref': 'refs/heads/main', 'key': 'bevy-web-unopt-v1-main-' + 'b' * 64 + '-' + '2' * 64},
            {'id': 22, 'ref': 'refs/pull/11/merge', 'key': 'bevy-web-unopt-v1-pr-' + 'a' * 64},
            {'id': 23, 'ref': 'refs/pull/11/merge', 'key': 'bevy-web-unopt-v1-pr-' + 'c' * 64},
            {'id': 24, 'ref': 'refs/pull/10/merge', 'key': 'bevy-web-unopt-v1-pr-' + 'a' * 64},
        ]
        with patch.dict(os.environ, {'GITHUB_EVENT_NAME': 'schedule', 'PR_NUMBER': '',
                                     'GITHUB_REPOSITORY': 'decentraland/dcl-one'}), \
                patch('subprocess.run', side_effect=cache_cleanup_api([{'actions_caches': caches}], {'10': 'MERGED'})) as cli, patch('builtins.print'):
            exec(compile(script, str(CLEANUP), 'exec'), {})
        self.assertEqual([call.args[0][-1] for call in cli.call_args_list if 'DELETE' in call.args[0]], [
            'repos/decentraland/dcl-one/actions/caches/1',
            'repos/decentraland/dcl-one/actions/caches/4',
            'repos/decentraland/dcl-one/actions/caches/5',
            'repos/decentraland/dcl-one/actions/caches/6',
            'repos/decentraland/dcl-one/actions/caches/12',
            'repos/decentraland/dcl-one/actions/caches/15',
            'repos/decentraland/dcl-one/actions/caches/17',
            'repos/decentraland/dcl-one/actions/caches/20',
            'repos/decentraland/dcl-one/actions/caches/22',
            'repos/decentraland/dcl-one/actions/caches/24',
        ])
        self.assertEqual(sum('graphql' in call.args[0] for call in cli.call_args_list), 1)

    def test_source_key_tracks_embedded_inputs_but_not_unrelated_commits(self):
        command = re.search(r'^          source=(.*)$', jobs()['smoke'], re.M).group(1)
        inputs = ('services/catalyst/worlds/src/storage/export.rs',
                  'library/protocol/proto/message.proto', 'schemas/20_worlds.sql',
                  'apps/dcl-scene-cli/src/vendor/node_modules.zip', '.cargo/config.toml')
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            def git(*args):
                return subprocess.run(['git', *args], cwd=root, check=True, capture_output=True)

            def commit():
                git('add', '.')
                git('-c', 'user.name=CI test', '-c', 'user.email=ci@example.invalid',
                    '-c', 'commit.gpgsign=false', 'commit', '-qm', 'fixture')

            def fingerprint():
                result = subprocess.run(['bash', '-eo', 'pipefail', '-c',
                                         'source=' + command + '\nprintf "%s" "$source"'],
                                        cwd=root, check=True, capture_output=True, text=True)
                return result.stdout

            git('init', '-q')
            for name in (*inputs, 'docs/unrelated.md'):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text('original')
            commit()
            previous = fingerprint()
            (root / 'docs/unrelated.md').write_text('changed')
            commit()
            self.assertEqual(previous, fingerprint())
            for name in inputs:
                with self.subTest(name=name):
                    (root / name).write_text('changed')
                    commit()
                    current = fingerprint()
                    self.assertNotEqual(previous, current)
                    previous = current

    def test_creator_hub_desktop_builds_need_the_label_on_pull_requests(self):
        source = CREATOR_HUB.read_text().split('jobs:\n', 1)[1]
        found = dict(re.findall(r'(?ms)^  ([\w-]+):\n(.*?)(?=^  [\w-]+:|\Z)', source))
        label = "contains(github.event.pull_request.labels.*.name, 'ci:desktop')"
        linux = "contains(github.event.pull_request.labels.*.name, 'ci:linux')"
        # main always builds; a skipped build is not a failure for the required check.
        self.assertIn(f"    if: github.event_name != 'pull_request' || {label} || {linux}\n", found['creator-hub'])
        # ci:desktop builds macOS, Windows and Linux, ci:linux builds Linux alone; main builds all three.
        self.assertIn(f"      macos-windows: ${{{{ github.event_name != 'pull_request' || {label} }}}}\n", found['creator-hub'])
        self.assertIn(f"      linux: ${{{{ github.ref == 'refs/heads/main' || {label} || {linux} }}}}\n", found['creator-hub'])
        self.assertIn('    needs: [ changes, creator-hub ]\n', found['prune_pre_releases'])
        self.assertIn('creator-hub, prune_pre_releases ]', found['result'])
        self.assertIn('any(.value.result == "failure" or .value.result == "cancelled")', found['result'])
        # No other job reads the label: the checks every pull request needs stay unconditional.
        conditions = {name: re.findall(r'(?m)^    if: (.*)$', job) for name, job in found.items()}
        self.assertEqual([name for name, lines in conditions.items() if any('ci:desktop' in line for line in lines)],
                         ['creator-hub'])
        self.assertIn(f'          DESKTOP: ${{{{ {label} }}}}\n', found['changes'])
        self.assertIn('Add the ci:desktop label', found['changes'])

    def test_creator_hub_installers_release_only_when_asked(self):
        source = CREATOR_HUB.read_text().split('jobs:\n', 1)[1]
        found = dict(re.findall(r'(?ms)^  ([\w-]+):\n(.*?)(?=^  [\w-]+:|\Z)', source))
        # One decision, on main only: the merged pull request's label, or the manual run's input.
        self.assertIn('if [ "$GITHUB_REF" = refs/heads/main ]; then', found['changes'])
        self.assertIn('any(.labels[]; .name == "release:creator-hub")', found['changes'])
        self.assertIn('          MANUAL: ${{ inputs.release }}\n', found['changes'])
        # Every job that publishes, or deletes a published pre-release, follows that decision.
        dry_run = "      dry-run: ${{ needs.changes.outputs.release != 'true' }}\n"
        self.assertEqual([name for name, job in found.items() if 'dry-run:' in job],
                         ['drop_pre_release', 'creator-hub', 'prune_pre_releases'])
        for name in ('drop_pre_release', 'creator-hub', 'prune_pre_releases'):
            with self.subTest(name=name):
                self.assertIn(dry_run, found[name])
                self.assertRegex(found[name], r'(?m)^    needs: \[ changes, ')

    def test_only_unit_suites_are_gated_by_web_selection(self):
        web = jobs()['web']
        changes = jobs()['changes']
        self.assertIn('run: python3 tooling/dev-ci.py\n', changes)
        self.assertIn('BASE_SHA: ${{ github.event.pull_request.base.sha }}', changes)
        self.assertIn('web-units: ${{ steps.web-coverage.outputs.run-web-unit-tests }}', changes)
        self.assertIn('fail-fast: false', web)
        self.assertIn('app: [sites, ui3]', web)
        steps = re.split(r'^      - ', web, flags=re.M)[1:]
        selected = [step for step in steps if 'needs.changes.outputs.web-units' in step]
        self.assertEqual(len(selected), 4)
        for app, command in [('ui3', 'ui3 test'), ('ui3', 'ui3 test:browser'),
                             ('ui3', 'ui3/editor-scene test'), ('sites', 'sites test')]:
            step = next(step for step in selected if f'pnpm --dir apps/{command}\n' in step)
            self.assertIn(f"if: matrix.app == '{app}' && needs.changes.outputs.web-units != 'false'", step)
        for app in ('ui3', 'sites'):
            contracts = next(step for step in steps if f'pnpm --dir apps/{app} typecheck\n' in step)
            self.assertIn(f"if: matrix.app == '{app}'\n", contracts)
        self.assertIn('typecheck:scripts', contracts)
        self.assertIn('gen:schemas:check', contracts)
        # Both legs share the pnpm cache key, so only the UI3 leg saves it, after the editor scene
        # install: a store saved from the Sites leg would lack the UI3-only packages.
        save = next(step for step in steps if step.startswith('name: Save the pnpm store from main'))
        self.assertIn("if: matrix.app == 'ui3' && github.ref == 'refs/heads/main'", save)
        installs = [i for i, step in enumerate(steps)
                    if 'pnpm --dir apps/sites install' in step or 'pnpm --dir apps/ui3/editor-scene install' in step]
        self.assertEqual(len(installs), 2)
        self.assertLess(max(installs), steps.index(save))


if __name__ == '__main__':
    unittest.main()
