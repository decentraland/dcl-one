#!/usr/bin/env python3
"""Decide whether a benchmark runs, and find its builds and legs in Bevy Explorer CI runs.

A pull request's change is measured against main; main's newest web build, on a schedule or by a
manual run without a pull request, against the latest release. Reads the triggering event
(GITHUB_EVENT_NAME, GITHUB_EVENT_PATH) and writes the step outputs of
.github/workflows/bevy-explorer-benchmark.yml. A trigger that does not apply (a fork, a failed CI
run, a CI run that built no web app because the PR changes no web input, main unchanged since the
release) skips silently; an unfit build skips with notes, which the workflow writes into the PR's
CI status comment or the run's summary.

The latest release is the web package of the newest successful rollout of every player at
decentraland.org/play (a GitHub deployment to `org` at 100%); its version, 0.1.0-<CI run>.commit-<sha>,
names the CI run whose web app is measured beside the change and the base. A release that cannot be
measured leaves a note, and the PR is still compared with its base.
"""
import io
import json
import os
from pathlib import Path
import re
import subprocess
import zipfile

WORKFLOW = '.github/workflows/bevy-explorer-ci.yml'
RUNS_PATH = 'actions/workflows/bevy-explorer-ci.yml/runs'
# The laptops by runner label, and the operating system each runs Chrome on.
FLEET = (
    ('intel-i5', 'windows'),
    ('apple-m1', 'macos'),
    ('amd-ryzen-3', 'windows'),
    ('intel-i3', 'windows'),
    ('framework-13-i7', 'windows'),
)
# The runner build each operating system takes; Linux writes the report.
PLATFORMS = {'windows': 'windows-amd64', 'macos': 'macos-arm64', 'linux': 'linux-amd64'}
WEB = 'bevy-explorer-web'
RUNNER = 'bevy-explorer-benchmark-'
# A leg's Test builds step is cut off after 90 minutes, so runs x 3 builds x (the 480 s run timeout
# + 45 s to close Chrome and settle) must stay under 5400 (bevy-explorer-benchmark-leg.yml).
MAX_RUNS = (5400 - 1) // (3 * (480 + 45))
SHA = re.compile(r'[0-9a-f]{40}')
# The latest release is the newest applied rollout of the web package to every player at
# decentraland.org/play: a GitHub deployment that bevy-explorer-rollout-manual.yml records with
# GITHUB_TOKEN as set-rollout-action's task (a record made by hand is not a release). Its own commit
# is where the rollout was dispatched from; the build is the CI run named in its version,
# <semver>-<CI run ID>.commit-<short commit>.
RELEASE_ENVIRONMENT = 'org'
RELEASE_PACKAGE = '@dcl-regenesislabs/bevy-explorer-web'
RELEASE_PATH = 'play'
RELEASE_CREATOR = 'github-actions[bot]'
RELEASE_TASK = 'dcl/set-rollout'
RELEASE_VERSION = re.compile(r'.+-([0-9]+)\.commit-([0-9a-f]{7,40})')
MAIN_ISSUE = 'Bevy Explorer: performance regression on main against the latest release'
# Days a run keeps its leg results and report: a pull request's for its review, main's as history.
RETENTION_DAYS = {'pr': '7', 'main': '90'}
NO_LAPTOP = 'BENCHMARK_ENABLED_RUNNERS enables no laptop of the fleet.'


def api(path, raw=False):
    """A GET of the GitHub REST API through gh: its JSON, or the bytes of an artifact's zip."""
    output = subprocess.check_output(['gh', 'api', '--method', 'GET', path], timeout=300 if raw else 60)
    return output if raw else json.loads(output)


def skip(reason):
    return {'run': False, 'pr': '', 'notes': '', 'reason': reason}


def stop(result, note, notes=()):
    return {**result, 'run': False, 'notes': ' '.join((*notes, note)), 'reason': 'nothing can run'}


def pr_number(text, repo):
    """A pull request given as its number or its URL in this repository."""
    text = (text or '').strip()
    match = (re.fullmatch(r'#?([0-9]+)', text) or
             re.fullmatch(rf'https://github\.com/{re.escape(repo)}/pull/([0-9]+)(?:[/?#].*)?', text, re.I))
    if not match:
        raise ValueError(f'pr must be a pull request number or a pull request URL of {repo}')
    return int(match.group(1))


def ci_run(run, repo, events):
    return (run['path'] == WORKFLOW and run['status'] == 'completed' and run['conclusion'] == 'success'
            and run['event'] in events and (run.get('head_repository') or {}).get('full_name') == repo
            and SHA.fullmatch(run['head_sha']))


# One resolve reads each list once: the web app and the runners are searched in the same runs.
# None outside resolve, so other callers always read the API.
CACHE = None


def artifacts(repo, run_id):
    """A run's artifacts that have not expired: {name: artifact ID}."""
    return cached(('artifacts', repo, run_id), lambda: list_artifacts(repo, run_id))


def list_artifacts(repo, run_id):
    data = api(f'repos/{repo}/actions/runs/{run_id}/artifacts?per_page=100')
    if data['total_count'] != len(data['artifacts']):
        raise ValueError(f'run {run_id} has more artifacts than one page lists')
    return {row['name']: row['id'] for row in data['artifacts'] if not row['expired']}


def cached(key, read):
    if CACHE is None:
        return read()
    if key not in CACHE:
        CACHE[key] = read()
    return CACHE[key]


def optimized(repo, artifact_id):
    """Whether a web app artifact's build-info.json says its engine went through wasm-opt."""
    try:
        with zipfile.ZipFile(io.BytesIO(api(f'repos/{repo}/actions/artifacts/{artifact_id}/zip', raw=True))) as app:
            info = json.loads(app.read('build-info.json'))
    except (KeyError, ValueError, zipfile.BadZipFile):
        return False
    return isinstance(info, dict) and isinstance(info.get('engine'), dict) and info['engine'].get('optimized') is True


def named(names, prefix):
    """The one artifact called <prefix>-<the commit the run built>."""
    found = [name for name in names if re.fullmatch(re.escape(prefix) + '-[0-9a-f]{40}', name)]
    return found[0] if len(found) == 1 else None


def search(repo, runs, prefixes):
    """For each prefix, the newest run that still has its artifact: {prefix: (name, run, artifact ID)}."""
    found = {}
    for run in runs:
        missing = [prefix for prefix in prefixes if prefix not in found]
        if not missing:
            break
        names = artifacts(repo, run['id'])
        for prefix in missing:
            name = named(names, prefix)
            if name:
                found[prefix] = (name, run, names[name])
    return found


def base_runs(repo, base_sha):
    """Successful main runs, newest first: base_sha's when given, else every push."""
    return cached(('runs', repo, base_sha), lambda: list_base_runs(repo, base_sha))


def list_base_runs(repo, base_sha):
    if base_sha:
        runs = api(f'repos/{repo}/{RUNS_PATH}?head_sha={base_sha}&status=success&per_page=100')['workflow_runs']
        return [run for run in runs if ci_run(run, repo, ('push', 'workflow_dispatch'))
                and run['head_branch'] == 'main' and run['head_sha'] == base_sha]
    runs = api(f'repos/{repo}/{RUNS_PATH}?branch=main&event=push&status=success&per_page=100')['workflow_runs']
    return [run for run in runs if ci_run(run, repo, ('push',)) and run['head_branch'] == 'main']


def deployments(repo, pages=10):
    """The `org` deployments, newest first, a page of 100 at a time: a caller stops at its match."""
    for page in range(1, pages + 1):
        rows = api(f'repos/{repo}/deployments?environment={RELEASE_ENVIRONMENT}&per_page=100&page={page}')
        yield from rows
        if len(rows) < 100:
            return


def latest_release(repo):
    """The newest applied rollout of the web package to every player of decentraland.org:
    (version, CI run ID, short commit), with None for both when its version names no CI run, or
    None. The callers decide what a version without a run means."""
    for deployment in deployments(repo):
        payload = deployment.get('payload')
        if not isinstance(payload, dict) or payload.get('prefix') != RELEASE_PACKAGE:
            continue
        if payload.get('path') != RELEASE_PATH or str(payload.get('percentage')) != '100':
            continue
        if (deployment.get('creator') or {}).get('login') != RELEASE_CREATOR or deployment.get('task') != RELEASE_TASK:
            continue
        if not released(api(f"repos/{repo}/deployments/{deployment['id']}/statuses?per_page=100")):
            continue
        version = str(payload.get('version') or '')
        match = RELEASE_VERSION.fullmatch(version)
        return (version, int(match.group(1)), match.group(2)) if match else (version, None, None)
    return None


def released(statuses):
    """Whether a deployment's statuses, newest first, end in success: a later `inactive`, which a
    newer deployment can set, leaves it released; a later failure or error does not."""
    for status in statuses:
        state = status.get('state')
        if state in ('failure', 'error'):
            return False
        if state == 'success':
            return True
    return False


def released_by(run, repo, short):
    """Whether a CI run of this repository built the commit a release version names. Not its status
    or conclusion: a release can roll out while its run is still going, later jobs can be cancelled
    or a guard fail after the web app is published, and the web app is checked next."""
    return (run['path'] == WORKFLOW and run['event'] in ('push', 'workflow_dispatch')
            and (run.get('head_repository') or {}).get('full_name') == repo
            and bool(SHA.fullmatch(run['head_sha'])) and run['head_sha'].startswith(short))


def release_build(repo, base_sha):
    """The latest release's web app for a pull request: ({'version', 'sha', 'artifact', 'run_id'},
    None), or (None, why not)."""
    latest = latest_release(repo)
    if latest is None:
        return None, f'No successful rollout of {RELEASE_PACKAGE} to every player of decentraland.org was found.'
    version, run_id, short = latest
    if run_id is None:
        return None, f'The latest release {version!r} does not name the CI run that built it.'
    run = api(f'repos/{repo}/actions/runs/{run_id}')
    if not released_by(run, repo, short):
        return None, f'CI run {run_id} is not a Bevy Explorer / CI run of the latest release ({version}).'
    if run['head_sha'] == base_sha:
        return None, f'The latest release ({version}) is the base build.'
    names = artifacts(repo, run['id'])
    web = named(names, WEB)
    if not web:
        return None, f'The web app of the latest release ({version}, CI run {run_id}) has expired.'
    if not optimized(repo, names[web]):
        return None, f'The web build of the latest release ({version}, CI run {run_id}) is not optimized.'
    return {'version': version, 'sha': run['head_sha'], 'artifact': web, 'run_id': str(run['id'])}, None


def run_count(inputs, runs):
    """The legs' RUNS and the report's EXPECTED_RUNS: a plain integer, which the report requires."""
    runs = (inputs.get('runs') or '').strip() or (runs or '').strip() or '3'
    if not re.fullmatch(r'[0-9]+', runs) or not 1 <= int(runs) <= MAX_RUNS:
        raise ValueError(f'runs must be a whole number from 1 to {MAX_RUNS}; the leg is cut off after 90 minutes')
    return str(int(runs))


def commits(inputs):
    change_sha = (inputs.get('change_sha') or '').strip()
    base_sha = (inputs.get('base_sha') or '').strip()
    for name, value in (('change_sha', change_sha), ('base_sha', base_sha)):
        if value and not SHA.fullmatch(value):
            raise ValueError(f'{name} must be a full 40-character commit hash')
    return change_sha, base_sha


def resolve(event_name, event, repo, enabled='', runs=''):
    global CACHE
    CACHE = {}
    try:
        return resolve_event(event_name, event, repo, enabled, runs)
    finally:
        CACHE = None


def resolve_event(event_name, event, repo, enabled, runs):
    inputs = {}
    if event_name == 'schedule':
        return resolve_main({}, repo, enabled, runs)
    # Only an empty pr is a run of main: the concurrency group compares it untrimmed.
    if event_name == 'workflow_dispatch' and ((event.get('inputs') or {}).get('pr') or '') == '':
        return resolve_main(event.get('inputs') or {}, repo, enabled, runs)
    if event_name == 'workflow_run':
        trigger = event['workflow_run']
        if trigger['event'] != 'pull_request' or (trigger.get('head_repository') or {}).get('full_name') != repo:
            return skip('the CI run is not for a pull request from this repository')
        if trigger['conclusion'] != 'success':
            return skip(f"CI run {trigger['id']} did not succeed")
        numbers = [row['number'] for row in trigger.get('pull_requests') or []]
        if not numbers:
            return skip(f"CI run {trigger['id']} belongs to no open pull request")
        number = numbers[0]
    elif event_name == 'workflow_dispatch':
        inputs = event.get('inputs') or {}
        number = pr_number(inputs.get('pr'), repo)
    else:
        return skip(f'{event_name} does not start the benchmark')

    pr = api(f'repos/{repo}/pulls/{number}')
    if ((pr['head'].get('repo') or {}).get('full_name')) != repo:
        return skip(f'#{number} comes from a fork')
    if pr['state'] != 'open':
        return skip(f'#{number} is not open')

    change_sha, base_sha = commits(inputs)
    change_sha = change_sha or pr['head']['sha']
    if event_name == 'workflow_run' and trigger['head_sha'] != change_sha:
        return skip(f'#{number} has moved on to {change_sha}; the CI run of that commit starts its benchmark')
    runs = run_count(inputs, runs)

    result = outcome(str(number), change_sha, runs, 'pr')
    candidates = api(f'repos/{repo}/{RUNS_PATH}?head_sha={change_sha}&status=success&per_page=100')['workflow_runs']
    changes = [run for run in candidates if ci_run(run, repo, ('pull_request', 'workflow_dispatch'))
               and run['head_sha'] == change_sha]
    if not changes:
        return stop(result, f'{change_sha} has no successful Bevy Explorer / CI run yet; the benchmark starts '
                            'when one passes.')
    change = changes[0]
    change_names = artifacts(repo, change['id'])
    change_web = named(change_names, WEB)
    if not change_web:
        # CI builds the web app only when the PR changes a web input: a run without one is not an
        # explorer change to measure, and says so only when asked by hand.
        if event_name == 'workflow_run':
            return skip(f"CI run {change['id']} built no web app: the PR changes no web input")
        return stop(result, f"CI run {change['id']} has no web app: it built none (the PR changes no web "
                            'input) or the artifact expired.')
    if not fleet(enabled):
        return stop(result, NO_LAPTOP)
    # While the fleet is on, CI builds every web app with wasm-opt, on PRs as on main: an
    # unoptimized build on either side would bias the comparison.
    if not optimized(repo, change_names[change_web]):
        return stop(result, f"The change web build (CI run {change['id']}) is not optimized; CI builds pull "
                            "requests' web apps with wasm-opt, so push again for a build it can compare.")
    found = search(repo, base_runs(repo, base_sha), [WEB])
    if WEB not in found:
        where = f'No CI run of {base_sha}' if base_sha else 'No successful main push run'
        return stop(result, f'{where} still has a web app to compare with.')
    base_web, base, base_web_id = found[WEB]
    result['base_sha'] = base['head_sha']
    result['change_description'] = f'#{number} at {change_sha}'
    result['base_description'] = f"main at {base['head_sha']}"
    if not optimized(repo, base_web_id):
        return stop(result, f"The base web build ({base['head_sha']}, CI run {base['id']}) is not optimized. "
                            "Main's CI builds optimized, so a manual run can name a base_sha whose build is.")
    # The runner comes from the change run, so a PR that changes it is measured with its own;
    # otherwise from the base run that has it.
    where = f'the CI run of {base_sha}' if base_sha else 'main'
    return measure(repo, result, enabled, (change, change_names, change_web), (base, base_web),
                   base_runs(repo, base_sha), where, f'#{number} at {change_sha}')


def resolve_main(inputs, repo, enabled, runs):
    """Main's newest web build, or a given commit's, against the latest release or a given base."""
    change_sha, base_sha = commits(inputs)
    runs = run_count(inputs, runs)
    # The fleet switched off on purpose is not a failure of main's nightly. As the workflow's
    # `vars.BENCHMARK_ENABLED_RUNNERS == 'none'`, case does not matter.
    if (enabled or '').strip().lower() == 'none':
        return skip('BENCHMARK_ENABLED_RUNNERS is none: the fleet is off')
    result = outcome('', change_sha, runs, 'main')
    if change_sha:
        candidates = api(f'repos/{repo}/{RUNS_PATH}?head_sha={change_sha}&status=success&per_page=100')['workflow_runs']
        changes = [run for run in candidates if run['head_sha'] == change_sha
                   and ci_run(run, repo, ('push', 'pull_request', 'workflow_dispatch'))
                   and named(artifacts(repo, run['id']), WEB)]
        if not changes:
            return stop(result, f'{change_sha} has no successful Bevy Explorer / CI run that still has a web app.')
        change = changes[0]
        change_description = change_sha
    else:
        found = search(repo, base_runs(repo, ''), [WEB])
        if WEB not in found:
            return stop(result, 'No successful main push run still has a web app to measure.')
        change = found[WEB][1]
        change_description = f"main at {change['head_sha']}"
    change_names = artifacts(repo, change['id'])
    change_web = named(change_names, WEB)
    result['change_sha'] = change['head_sha']
    result['change_description'] = change_description

    if base_sha:
        found = search(repo, base_runs(repo, base_sha), [WEB])
        if WEB not in found:
            return stop(result, f'No successful main CI run of {base_sha} still has a web app to compare with.')
        base_web, base, base_web_id = found[WEB]
        base_description = base_sha
    else:
        latest = latest_release(repo)
        if not latest:
            return stop(result, f'No complete, successful {RELEASE_PACKAGE} rollout to {RELEASE_ENVIRONMENT} is '
                                'recorded as a deployment.')
        version, run_id, short = latest
        # An older release must not become main's base.
        if run_id is None:
            raise ValueError(f'The latest release, {version or "a rollout without a version"}, names no CI run: '
                             'its version lacks the -<CI run ID>.commit-<short commit> suffix.')
        not_ci = (f'Release {version} names CI run {run_id}, which is not a Bevy Explorer / CI run of commit '
                  f'{short} in {repo}.')
        try:
            base = api(f'repos/{repo}/actions/runs/{run_id}')
        except subprocess.CalledProcessError:
            return stop(result, not_ci)
        if not released_by(base, repo, short):
            return stop(result, not_ci)
        base_names = artifacts(repo, run_id)
        base_web = named(base_names, WEB)
        if not base_web:
            return stop(result, f'The web app of release {version} (CI run {run_id}) has expired.')
        base_web_id = base_names[base_web]
        base_description = f"release {version} at {base['head_sha']}"
    if base['head_sha'] == result['change_sha']:
        if change_sha or base_sha:
            return stop(result, f"{base['head_sha']} is both the change and the base: there is nothing to compare.")
        return skip(f"{base['head_sha']} is both the change and the base: main has no newer web build")
    # Run IDs grow with time: main's newest passed web build may predate a release whose CI run
    # failed a guard, and measuring it against the release would read backwards.
    if not change_sha and not base_sha and change['id'] < base['id']:
        return skip(f"main's newest passed web build (CI run {change['id']}) predates the release's "
                    f"(CI run {base['id']}): main has no newer web build")
    result['base_sha'] = base['head_sha']
    result['base_description'] = base_description
    # The issue a regression opens or updates: main's one issue, or one per overridden comparison.
    result['issue_title'] = (MAIN_ISSUE if not change_sha and not base_sha else
                             f"Bevy Explorer: performance regression of {change_description} against {base_description}")

    if not fleet(enabled):
        return stop(result, NO_LAPTOP)
    if not optimized(repo, change_names[change_web]):
        return stop(result, f"The change web build (CI run {change['id']}) is not optimized.")
    if not optimized(repo, base_web_id):
        return stop(result, f"The base web build ({base['head_sha']}, CI run {base['id']}) is not optimized.")
    # The runner comes from main's newest push run that has it, which reads files of the newest
    # format; a given commit's run is never searched, so its runner never writes the report.
    own = change['event'] == 'push' and change['head_branch'] == 'main'
    return measure(repo, result, enabled, (change, change_names if own else {}, change_web), (base, base_web),
                   base_runs(repo, ''), 'main', change_description)


def outcome(pr, change_sha, runs, kind):
    return {'run': False, 'pr': pr, 'change_sha': change_sha, 'base_sha': '', 'runs': runs, 'legs': [],
            'expected_labels': '', 'report_runner_artifact': '', 'report_runner_run_id': '', 'notes': '',
            'change_description': '', 'base_description': '', 'retention_days': RETENTION_DAYS[kind],
            'issue_title': '', 'release_sha': '', 'release_version': ''}


def fleet(enabled):
    return [(label, system) for label, system in FLEET if not enabled or label in enabled]


def measure(repo, result, enabled, change_build, base_build, runner_runs, runner_where, description):
    """The legs of every enabled laptop that has a runner, or the note why none can run."""
    change, change_names, change_web = change_build
    base, base_web = base_build
    laptops = fleet(enabled)
    platforms = sorted({PLATFORMS[system] for _, system in laptops} | {PLATFORMS['linux']})
    runners = {}
    for platform in platforms:
        runner = named(change_names, RUNNER + platform)
        if runner:
            runners[platform] = (runner, change)
    wanted = [RUNNER + platform for platform in platforms if platform not in runners]
    if wanted:
        for prefix, (name, run, _) in search(repo, runner_runs, wanted).items():
            runners[prefix[len(RUNNER):]] = (name, run)

    # A change run whose artifacts were not given was not searched, so the notes leave it out.
    lacking = f'neither the change run nor {runner_where} has' if change_names else f'{runner_where} has no run with'
    if PLATFORMS['linux'] not in runners:
        return stop(result, f'No benchmark runner for linux-amd64 to write the report: {lacking} one.')
    report_name, report_run = runners[PLATFORMS['linux']]
    notes = []
    release = None
    if result['pr']:
        try:
            release, why_not = release_build(repo, base['head_sha'])
        except (subprocess.SubprocessError, ValueError):
            release, why_not = None, 'it could not be read from the GitHub API.'
        if release:
            result['release_sha'], result['release_version'] = release['sha'], release['version']
        else:
            notes.append(f'Not compared with the latest release: {why_not}')
    for label, system in laptops:
        if PLATFORMS[system] not in runners:
            notes.append(f'{label}-chrome did not run: {lacking} a benchmark runner for {PLATFORMS[system]}.')
            continue
        runner_name, runner_run = runners[PLATFORMS[system]]
        result['legs'].append({
            'label': f'{label}-chrome', 'runs_on': label, 'os': system,
            'runner_artifact': runner_name, 'runner_run_id': str(runner_run['id']),
            'change_artifact': change_web, 'change_run_id': str(change['id']),
            'base_artifact': base_web, 'base_run_id': str(base['id']),
            'release_artifact': release['artifact'] if release else '',
            'release_run_id': release['run_id'] if release else '',
        })
    if not result['legs']:
        return stop(result, 'No laptop can run the benchmark.', notes)
    labels = [leg['label'] for leg in result['legs']]
    return {**result, 'run': True, 'expected_labels': ','.join(labels), 'notes': ' '.join(notes),
            'report_runner_artifact': report_name, 'report_runner_run_id': str(report_run['id']),
            'reason': f"{description} against {result['base_description']}"
                      f"{' and release ' + release['version'] if release else ''} on {', '.join(labels)}"}


def write_outputs(values):
    print(json.dumps(values, indent=2))
    if os.environ.get('GITHUB_OUTPUT'):
        with open(os.environ['GITHUB_OUTPUT'], 'a', encoding='utf-8') as output:
            for key, value in values.items():
                encoded = value if isinstance(value, str) else json.dumps(value, separators=(',', ':'))
                output.write(f'{key}={encoded}\n')
    if os.environ.get('GITHUB_STEP_SUMMARY'):
        with open(os.environ['GITHUB_STEP_SUMMARY'], 'a', encoding='utf-8') as summary:
            summary.write(f"Benchmark {'runs' if values['run'] else 'skipped'}: {values['reason']}.\n")
            if values['notes']:
                summary.write(f"\n{values['notes']}\n")


def main():
    event = json.loads(Path(os.environ['GITHUB_EVENT_PATH']).read_text(encoding='utf-8'))
    try:
        values = resolve(os.environ['GITHUB_EVENT_NAME'], event, os.environ['GH_REPO'],
                         os.environ.get('BENCHMARK_ENABLED_RUNNERS', ''), os.environ.get('BENCHMARK_RUNS', ''))
    except (ValueError, subprocess.CalledProcessError) as exc:
        print(f'::error::{exc}')
        return 1
    write_outputs(values)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
