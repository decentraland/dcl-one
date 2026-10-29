#!/usr/bin/env python3
"""Select Explorer jobs from their inputs and validate image-producing runs."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = '.github/workflows/bevy-explorer-ci.yml'
SCENES_WORKFLOW = '.github/workflows/bevy-explorer-scenes.yml'
PRODUCERS = {'web': 'Web engine, HUD and bridge scene', 'headless': 'Headless Smoke'}
LANES = ('clippy', 'test', 'headless', 'web')
NATIVE_JOBS = {'Test Suite (ubuntu-24.04)', 'Test Suite (windows-2025)', 'Test Suite (macos-15)'}
INPUTS = ('rendering/bevy-explorer/source', 'library/vendor/rusty_enet', 'tooling/dev-regressions*', 'library/protocol/proto', 'apps/bevy-explorer',
          'tooling/bevy-ci.py', 'tooling/play-artifact.py', 'tooling/tests/test_play_*.py',
          'tooling/tests/test_bevy_*.py', WORKFLOW, SCENES_WORKFLOW,
          '.github/workflows/bevy-explorer-images.yml',
          '.github/workflows/bevy-explorer-headless.yml',
          '.github/workflows/bevy-explorer-headless-build.yml',
          'tooling/ci-publication.py', '.cargo', 'rust-toolchain*')
# Web files that the benchmark runner's own tests (rendering/bevy-explorer/source/local-harness/
# benchmark/tests) read: a change to one is also an input of the Test Suite.
BROWSER_LEG_READS = ('rendering/bevy-explorer/source/deploy/web/engine/perf_stream.js',
                     'rendering/bevy-explorer/source/react-web/src/features/auth/sso.ts')
IMAGES_INPUTS = (
    'flake.*',
    'rendering/bevy-explorer/**',
    'apps/bevy-explorer/build-web.py',
    'apps/bevy-explorer/install-*.py',
    '.github/workflows/bevy-explorer-ci.yml',
    'tooling/fetch-play.py',
    'tooling/play-artifact.py',
    'tooling/tests/test_play_*.py',
    'rust-toolchain*',
    'infra/packages/**',
    'infra/containers/**',
    'services/catalyst/**',
    'library/**',
    '.cargo/**',
    'apps/sites/**',
    'apps/ui3/**',
    'tooling/image-*',
    'tooling/ci-publication.py',
    'tooling/tests/test_ci_publication.py',
    'tooling/tests/test_image_publish.py',
    '.github/workflows/apis-sites-images.yml',
)
IMAGES_JOBS = {'APIs + Sites / build (amd64, ubuntu-26.04)',
               'APIs + Sites / build (arm64, ubuntu-26.04-arm)'}
HEADLESS_JOBS = {
    'Headless packages / build (macos-arm64, darwin-arm64, macos-15, aarch64-apple-darwin)',
    'Headless packages / build (macos-amd64, darwin-x64, macos-15, x86_64-apple-darwin)',
    'Headless packages / build (windows-amd64, win32-x64, windows-2025, x86_64-pc-windows-msvc)',
}
# Unknown app inputs remain conservative. Only named integration boundaries narrow work.
APP_INPUTS = {
    'README.md': (),
    'Dockerfile.web': ('web',),
    'nginx.conf': ('web',),
    'Dockerfile.headless': ('headless',),
    'image-smoke.py': ('web', 'headless'),
    'Makefile': ('web', 'headless'),
    # Each producer has its own build script; web edits must not rebuild headless.
    'build-web.py': ('web',),
    'build-headless.py': ('headless',),
    'install-linux-deps.py': (*LANES, 'scenes'),
    'prepare-native-cache.py': ('clippy', 'test', 'headless', 'native-matrix', 'scenes'),
}


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT, text=True)


def api(path):
    return json.loads(subprocess.check_output(['gh', 'api', path], text=True, timeout=60))


def trusted_run(run, repo):
    return (run['status'] == 'completed' and run['conclusion'] == 'success'
            and run['head_branch'] == 'main' and run['event'] in ('push', 'workflow_dispatch')
            and run['head_repository']['full_name'] == repo and run['path'] == WORKFLOW
            and re.fullmatch(r'[0-9a-f]{40}', run['head_sha']))


def produced_artifacts(jobs):
    """Each product needs its own successful producer; explicit skips preserve its tags."""
    products = []
    for product, name in PRODUCERS.items():
        selected = [job for job in jobs if job['name'] == name]
        if len(selected) != 1:
            raise ValueError(f'Missing or duplicate Explorer artifact producer: {name}')
        job = selected[0]
        if job['status'] != 'completed' or job['conclusion'] not in ('success', 'skipped'):
            raise ValueError(f'Explorer artifact producer must succeed or explicitly skip: {name}')
        if job['conclusion'] == 'success':
            products.append(product)
    return products


def run_jobs(repo, run_id):
    data = api(f'repos/{repo}/actions/runs/{run_id}/jobs?per_page=100')
    if data['total_count'] != len(data['jobs']):
        raise ValueError('Incomplete source job list')
    return data['jobs']


def passed_lane(jobs, lane):
    if lane == 'test':
        selected = [job for job in jobs if job['name'].startswith('Test Suite (')]
        names = {job['name'] for job in selected}
        if names not in ({'Test Suite (ubuntu-24.04)'}, NATIVE_JOBS) or len(names) != len(selected):
            return False
    else:
        name = 'Clippy' if lane == 'clippy' else PRODUCERS[lane]
        selected = [job for job in jobs if job['name'] == name]
        if len(selected) != 1:
            return False
    return all(job['status'] == 'completed' and job['conclusion'] == 'success' for job in selected)


def main_baselines(repo):
    """A web-only pass cannot advance the baseline of unexecuted native jobs."""
    runs = api(f'repos/{repo}/actions/workflows/bevy-explorer-ci.yml/runs'
               '?branch=main&status=success&per_page=20')['workflow_runs']
    baselines = {}
    for run in runs:
        if not trusted_run(run, repo):
            continue
        jobs = run_jobs(repo, run['id'])
        for lane in LANES:
            if lane not in baselines and passed_lane(jobs, lane):
                baselines[lane] = run['head_sha']
        if len(baselines) == len(LANES):
            break
    return baselines


def ensure_commit(sha):
    if not sha or not re.fullmatch(r'[0-9a-f]{40}', sha) or sha == '0' * 40:
        raise ValueError('Missing comparison commit')
    found = subprocess.run(['git', 'cat-file', '-e', f'{sha}^{{commit}}'], cwd=ROOT,
                           capture_output=True).returncode == 0
    if not found:
        subprocess.run(['git', 'fetch', '--no-tags', '--depth=1', 'origin', sha], cwd=ROOT,
                       check=True, timeout=60)


def full_selection(reason):
    return {**dict.fromkeys((*LANES, 'native-matrix', 'scenes'), True), 'reason': reason}


def select(base, head):
    ensure_commit(base)
    ensure_commit(head)
    paths = set(filter(None, git('diff', '--name-only', '--no-renames', '-z', base, head,
                                 '--', *INPUTS).split('\0')))
    selected = dict.fromkeys((*LANES, 'native-matrix', 'scenes'), False)
    for path in paths:
        if path == SCENES_WORKFLOW:
            affected = ('scenes',)
        elif path in ('.github/workflows/bevy-explorer-headless.yml',
                      '.github/workflows/bevy-explorer-headless-build.yml', 'tooling/ci-publication.py'):
            affected = ('headless',)
        elif path == WORKFLOW:
            affected = (*LANES, 'native-matrix')
        elif path == '.github/workflows/bevy-explorer-images.yml':
            affected = ('web', 'headless')
        elif path == 'tooling/play-artifact.py':
            affected = ('web',)
        elif path.startswith(('tooling/tests/test_bevy_', 'tooling/tests/test_play_')):
            # The always-run source job executes these tests; they aren't build inputs.
            affected = ()
        elif path in BROWSER_LEG_READS:
            # The web lane builds and tests these files; the Test Suite runs the benchmark runner's
            # tests, which read them (the sender the runner's receiver must accept, the guest's
            # root-key name its reset page keeps).
            affected = ('web', 'test')
        elif path.startswith(('rendering/bevy-explorer/source/react-web/',
                              'rendering/bevy-explorer/source/deploy/web/')):
            # These are runtime web assets, not inputs to Cargo's native tests
            # or headless engine. The web producer builds/tests the HUD and bridge.
            affected = ('web',)
        elif path.startswith('rendering/bevy-explorer/source/deploy/headless/'):
            # Launcher/package assembly cannot affect web or graphical/native tests.
            # Keep the headless build/smoke; main also packages all four platforms.
            affected = ('headless',)
        elif path.startswith('apps/bevy-explorer/') and path.removeprefix('apps/bevy-explorer/') in APP_INPUTS:
            affected = APP_INPUTS[path.removeprefix('apps/bevy-explorer/')]
        else:
            # Source, selector and unknown input changes validate all jobs.
            affected = (*LANES, 'native-matrix', 'scenes')
        for lane in affected:
            selected[lane] = True
    names = [name for name, enabled in selected.items() if enabled]
    return {**selected, 'base': base,
            'reason': 'Selected: ' + ', '.join(names) if names else 'Inputs unchanged; integrity checks only'}


def plan(event, base, head, repo):
    if event == 'workflow_dispatch':
        return full_selection('Manual full run')
    try:
        if event == 'push':
            baselines = main_baselines(repo)
            selections = {}
            result = {}
            for lane in LANES:
                previous = baselines.get(lane)
                if previous not in selections:
                    selections[previous] = select(previous, head) if previous else full_selection('Missing lane baseline')
                result[lane] = selections[previous][lane]
            result['native-matrix'] = selections[baselines.get('test')]['native-matrix']
            result['reason'] = 'Compared each job with its latest successful main execution'
            return result
        if event not in ('pull_request', 'local'):
            raise ValueError('Unknown event')
        return select(base, head)
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as exc:
        print(f'Conservative full build: {exc}')
        return full_selection('Comparison unavailable; full run')


def scenes_plan(event, base, head):
    if event == 'workflow_dispatch':
        return {'scenes': True, 'sha': head, 'reason': 'Manual or nightly graphical run'}
    try:
        if event not in ('push', 'pull_request', 'local'):
            raise ValueError('Unknown graphical event')
        selected = select(base, head)
        return {'scenes': selected['scenes'], 'sha': head, 'reason': selected['reason']}
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as exc:
        print(f'Conservative graphical run: {exc}')
        return {'scenes': True, 'sha': head, 'reason': 'Comparison unavailable; run graphical tests'}


def image_matrix(products):
    return {'include': [
        {'product': product, 'arch': arch, 'runner': 'ubuntu-24.04' + ('-arm' if arch == 'arm64' else '')}
        for product in products for arch in (('amd64', 'arm64') if product == 'web' else ('amd64',))]}


def consumer_ready(jobs, consumer):
    expected = IMAGES_JOBS if consumer == 'images' else HEADLESS_JOBS
    prefix = 'APIs + Sites / build (' if consumer == 'images' else 'Headless packages / build ('
    selected = [job for job in jobs if job['name'].startswith(prefix)]
    if not selected or all(job['conclusion'] == 'skipped' for job in selected):
        return False
    if {job['name'] for job in selected} != expected or len(selected) != len(expected):
        raise ValueError(f'Incomplete or duplicate {consumer} build matrix')
    if consumer == 'headless':
        selected += [job for job in jobs if job['name'] == PRODUCERS['headless']]
        if len(selected) != len(expected) + 1:
            raise ValueError('Missing Linux headless producer')
    if not all(job['status'] == 'completed' and job['conclusion'] == 'success' for job in selected):
        raise ValueError(f'Every {consumer} platform must pass before publication')
    return True


def consumer_changed(base, head, consumer):
    if consumer == 'headless':
        return select(base, head)['headless']
    if consumer != 'images':
        raise ValueError('Unknown publication consumer')
    ensure_commit(base)
    ensure_commit(head)
    return bool(git('diff', '--name-only', '--no-renames', base, head, '--', *IMAGES_INPUTS))


def consumer_plan(event, base, head, repo):
    if event == 'workflow_dispatch':
        return {'images': False, 'reason': 'Manual Explorer run; consumers have their own manual entrypoints'}
    try:
        if event == 'push':
            # Compare against a successful image build, never a canceled push
            # whose changes have not been validated. Pre-migration runs contain
            # no image jobs, so the first coordinated push builds both images.
            base = None
            runs = api(f'repos/{repo}/actions/workflows/bevy-explorer-ci.yml/runs'
                       '?branch=main&status=success&per_page=20')['workflow_runs']
            for run in runs:
                if trusted_run(run, repo) and consumer_ready(run_jobs(repo, run['id']), 'images'):
                    base = run['head_sha']
                    break
        elif event not in ('pull_request', 'local'):
            raise ValueError('Unknown image selection event')
        ensure_commit(base)
        ensure_commit(head)
        changed = git('diff', '--name-only', '--no-renames', base, head, '--', *IMAGES_INPUTS)
        return {'images': bool(changed), 'reason': 'Image consumer inputs changed' if changed else 'Image consumer inputs unchanged'}
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as exc:
        print(f'Conservative image build: {exc}')
        return {'images': True, 'reason': 'Image comparison unavailable; build both architectures'}


def consumer_source(run, repo, consumer):
    ready = consumer_ready(run_jobs(repo, run['id']), consumer)
    if ready:
        if consumer == 'images':
            names = {f'apis-sites-{kind}-{arch}' for kind in ('images', 'producer') for arch in ('amd64', 'arm64')}
        else:
            names = {f'bevy-headless-{platform}-{run["head_sha"]}' for platform in
                     ('linux-x64', 'darwin-x64', 'darwin-arm64', 'win32-x64')}
        data = api(f'repos/{repo}/actions/runs/{run["id"]}/artifacts?per_page=100')
        found = [row['name'] for row in data['artifacts'] if row['name'] in names and not row['expired']]
        if data['total_count'] != len(data['artifacts']) or set(found) != names or len(found) != len(names):
            raise ValueError(f'Missing, expired or duplicate {consumer} artifacts')
    return {'sha': run['head_sha'], 'run_id': str(run['id']), 'attempt': str(run['run_attempt']), 'build': ready,
            'reason': f'{consumer} builds passed' if ready else f'{consumer} builds were not selected'}


def write_outputs(values):
    print(json.dumps(values, indent=2))
    if os.environ.get('GITHUB_OUTPUT'):
        with open(os.environ['GITHUB_OUTPUT'], 'a') as output:
            for key, value in values.items():
                encoded = json.dumps(value, separators=(',', ':')) if not isinstance(value, str) else value
                output.write(f'{key}={encoded}\n')
    if os.environ.get('GITHUB_STEP_SUMMARY'):
        with open(os.environ['GITHUB_STEP_SUMMARY'], 'a') as summary:
            summary.write(f"Explorer CI: {values['reason']}.\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=('plan', 'source', 'scenes-plan', 'consumer-plan'))
    parser.add_argument('--consumer', choices=('images', 'headless'))
    parser.add_argument('--base', default=os.environ.get('BASE_SHA', ''))
    parser.add_argument('--head', default=os.environ.get('GITHUB_SHA', 'HEAD'))
    args = parser.parse_args()
    if args.command != 'source':
        head = git('rev-parse', args.head).strip()
        event = os.environ.get('GITHUB_EVENT_NAME', 'local')
        if args.command == 'consumer-plan':
            write_outputs(consumer_plan(event, args.base, head, os.environ.get('GH_REPO', '')))
            return
        result = (scenes_plan(event, args.base, head) if args.command == 'scenes-plan' else
                  plan(event, args.base, head, os.environ.get('GH_REPO', '')))
        write_outputs(result)
    else:
        repo, run_id = os.environ['GH_REPO'], os.environ['SOURCE_RUN']
        if not re.fullmatch(r'[0-9]+', run_id):
            raise ValueError('Invalid run ID')
        run = api(f'repos/{repo}/actions/runs/{run_id}')
        if not trusted_run(run, repo):
            raise ValueError('Source must be a successful main Explorer CI run in this repository')
        if args.consumer:
            write_outputs(consumer_source(run, repo, args.consumer))
            return
        products = produced_artifacts(run_jobs(repo, run_id))
        write_outputs({'sha': run['head_sha'], 'run_id': run_id, 'build': bool(products),
                       'products': products, 'build-matrix': image_matrix(products),
                       'reason': 'Build images from successful producers: ' + ', '.join(products) if products else
                       'Both producers explicitly skipped; image tags stay unchanged'})


if __name__ == '__main__':
    main()
