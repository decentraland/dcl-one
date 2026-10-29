#!/usr/bin/env python3
"""Decide whether a pull request's benchmark runs, and find its builds and legs in Bevy Explorer CI runs.

Reads the triggering event (GITHUB_EVENT_NAME, GITHUB_EVENT_PATH) and writes the step outputs of
.github/workflows/bevy-explorer-benchmark.yml. A trigger that does not apply (a fork, a failed CI
run, a CI run that built no web app because the PR changes no web input) skips silently; an unfit
build skips with notes, which the workflow writes into the PR's CI status comment.
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
# A leg's Test builds step is cut off after 60 minutes, so runs x 2 x (the 480 s run timeout + 45 s
# to close Chrome and settle) must stay under 3600 (bevy-explorer-benchmark-leg.yml).
MAX_RUNS = (3600 - 1) // (2 * (480 + 45))
SHA = re.compile(r'[0-9a-f]{40}')


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


def artifacts(repo, run_id):
    """A run's artifacts that have not expired: {name: artifact ID}."""
    data = api(f'repos/{repo}/actions/runs/{run_id}/artifacts?per_page=100')
    if data['total_count'] != len(data['artifacts']):
        raise ValueError(f'run {run_id} has more artifacts than one page lists')
    return {row['name']: row['id'] for row in data['artifacts'] if not row['expired']}


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
    if base_sha:
        runs = api(f'repos/{repo}/{RUNS_PATH}?head_sha={base_sha}&status=success&per_page=100')['workflow_runs']
        return [run for run in runs if ci_run(run, repo, ('push', 'workflow_dispatch'))
                and run['head_branch'] == 'main' and run['head_sha'] == base_sha]
    runs = api(f'repos/{repo}/{RUNS_PATH}?branch=main&event=push&status=success&per_page=100')['workflow_runs']
    return [run for run in runs if ci_run(run, repo, ('push',)) and run['head_branch'] == 'main']


def resolve(event_name, event, repo, enabled='', runs=''):
    inputs = {}
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

    change_sha = (inputs.get('change_sha') or '').strip() or pr['head']['sha']
    base_sha = (inputs.get('base_sha') or '').strip()
    for name, value in (('change_sha', change_sha), ('base_sha', base_sha)):
        if value and not SHA.fullmatch(value):
            raise ValueError(f'{name} must be a full 40-character commit hash')
    if event_name == 'workflow_run' and trigger['head_sha'] != change_sha:
        return skip(f'#{number} has moved on to {change_sha}; the CI run of that commit starts its benchmark')
    # The legs' RUNS and the report's EXPECTED_RUNS: a plain integer, which the report requires.
    runs = (inputs.get('runs') or '').strip() or (runs or '').strip() or '3'
    if not re.fullmatch(r'[0-9]+', runs) or not 1 <= int(runs) <= MAX_RUNS:
        raise ValueError(f'runs must be a whole number from 1 to {MAX_RUNS}; the leg is cut off after an hour')
    runs = str(int(runs))

    result = {'run': False, 'pr': str(number), 'change_sha': change_sha, 'base_sha': '', 'runs': runs,
              'legs': [], 'expected_labels': '', 'report_runner_artifact': '', 'report_runner_run_id': '',
              'notes': ''}
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
    # CI builds every web app with wasm-opt, on PRs as on main: an unoptimized build on either side
    # would bias the comparison.
    if not optimized(repo, change_names[change_web]):
        return stop(result, f"The change web build (CI run {change['id']}) is not optimized; CI builds pull "
                            "requests' web apps with wasm-opt, so push again for a build it can compare.")

    fleet = [(label, system) for label, system in FLEET if not enabled or label in enabled]
    if not fleet:
        return stop(result, 'BENCHMARK_ENABLED_RUNNERS enables no laptop of the fleet.')
    # The runner comes from the change run, so a PR that changes it is measured with its own;
    # otherwise from the base run that has it.
    platforms = sorted({PLATFORMS[system] for _, system in fleet} | {PLATFORMS['linux']})
    runners = {}
    for platform in platforms:
        runner = named(change_names, RUNNER + platform)
        if runner:
            runners[platform] = (runner, change)
    wanted = [WEB, *(RUNNER + platform for platform in platforms if platform not in runners)]
    found = search(repo, base_runs(repo, base_sha), wanted)
    base_where = f'the CI run of {base_sha}' if base_sha else 'main'
    if WEB not in found:
        where = f'No CI run of {base_sha}' if base_sha else 'No successful main push run'
        return stop(result, f'{where} still has a web app to compare with.')
    base_web, base, base_web_id = found[WEB]
    for platform in platforms:
        if platform not in runners and RUNNER + platform in found:
            runners[platform] = found[RUNNER + platform][:2]
    result['base_sha'] = base['head_sha']
    if not optimized(repo, base_web_id):
        return stop(result, f"The base web build ({base['head_sha']}, CI run {base['id']}) is not optimized. "
                            "Main's CI builds optimized, so a manual run can name a base_sha whose build is.")

    if PLATFORMS['linux'] not in runners:
        return stop(result, 'No benchmark runner for linux-amd64 to write the report: neither the change run '
                            f'nor {base_where} has one.')
    report_name, report_run = runners[PLATFORMS['linux']]
    notes = []
    for label, system in fleet:
        if PLATFORMS[system] not in runners:
            notes.append(f'{label}-chrome did not run: neither the change run nor {base_where} has a benchmark '
                         f'runner for {PLATFORMS[system]}.')
            continue
        runner_name, runner_run = runners[PLATFORMS[system]]
        result['legs'].append({
            'label': f'{label}-chrome', 'runs_on': label, 'os': system,
            'runner_artifact': runner_name, 'runner_run_id': str(runner_run['id']),
            'change_artifact': change_web, 'change_run_id': str(change['id']),
            'base_artifact': base_web, 'base_run_id': str(base['id']),
        })
    if not result['legs']:
        return stop(result, 'No laptop can run the benchmark.', notes)
    labels = [leg['label'] for leg in result['legs']]
    return {**result, 'run': True, 'expected_labels': ','.join(labels), 'notes': ' '.join(notes),
            'report_runner_artifact': report_name, 'report_runner_run_id': str(report_run['id']),
            'reason': f"#{number} at {change_sha} against main at {base['head_sha']} on {', '.join(labels)}"}


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
