#!/usr/bin/env python3
"""The inputs of Bevy Explorer / E2E: which web app the suite runs against, and from which commit.

Outputs (stdout as JSON, and GITHUB_OUTPUT when set): `run` (true or false), `reason`, `sha` (the
commit whose suite runs, checked out by the job), `web_artifact` and `web_run_id` (the web app to
download, from tooling/benchmark-inputs.py's artifact search), and `pr` (the pull request number,
or empty).

- `workflow_run` of Bevy Explorer / CI: a successful run of this repository, a push to main or a
  pull request from this repository. Its own web app when it built one; a pull request run that
  built none (the PR changes no web input) takes main's newest when the PR changes the suite, its
  workflow or its tooling, so the PR's specs still run; a PR that changes neither is skipped.
- `schedule`: main's newest web app and the commit that built it.
- `workflow_dispatch`: `inputs.sha`'s CI run and web app, else main's newest.
"""
import importlib.util
import json
import os
import subprocess
from pathlib import Path

_spec = importlib.util.spec_from_file_location('benchmark_inputs', Path(__file__).resolve().parent / 'benchmark-inputs.py')
bench = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(bench)


# What the workflow's `pull_request` trigger lists: the suite, the workflow and its tooling.
SUITE_PATHS = ('apps/bevy-explorer/e2e/', '.github/workflows/bevy-explorer-e2e.yml', 'tooling/bevy-e2e-', 'tooling/tests/test_bevy_e2e_')


def skip(reason):
    return {'run': False, 'reason': reason, 'sha': '', 'web_artifact': '', 'web_run_id': '', 'pr': ''}


def changes_suite(repo, pr):
    """Whether the pull request changes a file under SUITE_PATHS (its files, a page at a time)."""
    page = 1
    while True:
        files = bench.api(f'repos/{repo}/pulls/{pr}/files?per_page=100&page={page}')
        if any(entry['filename'].startswith(SUITE_PATHS) for entry in files):
            return True
        if len(files) < 100:
            return False
        page += 1


def chosen(sha, artifact, run_id, pr, reason):
    return {'run': True, 'reason': reason, 'sha': sha, 'web_artifact': artifact, 'web_run_id': str(run_id), 'pr': pr}


def newest_main(repo):
    """Main's newest successful push run that still has its web app: (artifact name, run), or None."""
    found = bench.search(repo, bench.base_runs(repo, ''), (bench.WEB,))
    if bench.WEB not in found:
        return None
    name, run, _ = found[bench.WEB]
    return name, run


def run_of(repo, sha):
    runs = bench.api(f'repos/{repo}/{bench.RUNS_PATH}?head_sha={sha}&status=success&per_page=100')['workflow_runs']
    runs = [run for run in runs if bench.ci_run(run, repo, ('push', 'pull_request', 'workflow_dispatch'))
            and run['head_sha'] == sha]
    return runs[0] if runs else None


def resolve(event_name, event, repo):
    if event_name == 'workflow_run':
        trigger = event['workflow_run']
        if (trigger.get('head_repository') or {}).get('full_name') != repo:
            return skip('the CI run is not from this repository')
        if trigger['conclusion'] != 'success':
            return skip(f"CI run {trigger['id']} did not succeed")
        if trigger['event'] not in ('push', 'pull_request'):
            return skip(f"CI run {trigger['id']} was started by {trigger['event']}, not a push or a pull request")
        if trigger['event'] == 'push' and trigger.get('head_branch') != 'main':
            return skip(f"CI run {trigger['id']} is a push to {trigger.get('head_branch')}, not main")
        sha = trigger['head_sha']
        numbers = [row['number'] for row in trigger.get('pull_requests') or []]
        pr = str(numbers[0]) if numbers else ''
        if trigger['event'] == 'pull_request' and not pr:
            return skip(f"CI run {trigger['id']} belongs to no open pull request")
        web = bench.named(bench.artifacts(repo, trigger['id']), bench.WEB)
        if web:
            where = f'#{pr}' if pr else 'main'
            return chosen(sha, web, trigger['id'], pr, f"{where} at {sha}: the web app CI run {trigger['id']} built")
        if trigger['event'] == 'push':
            return skip(f"main's CI run {trigger['id']} built no web app")
        if not changes_suite(repo, pr):
            return skip(f"#{pr} changes neither the explorer's web app nor the suite")
        main = newest_main(repo)
        if not main:
            return skip(f"#{pr} changes no web input and no main run still has a web app")
        name, run = main
        return chosen(sha, name, run['id'], pr,
                      f"#{pr} at {sha} (no web input changed) against main's web app from {run['head_sha']}")
    if event_name in ('schedule', 'workflow_dispatch'):
        sha = ((event.get('inputs') or {}).get('sha') or '').strip()
        if sha:
            if not bench.SHA.fullmatch(sha):
                raise ValueError('sha must be a full 40-character commit hash')
            run = run_of(repo, sha)
            if not run:
                return skip(f'{sha} has no successful Bevy Explorer / CI run')
            web = bench.named(bench.artifacts(repo, run['id']), bench.WEB)
            if not web:
                return skip(f"CI run {run['id']} of {sha} has no web app: it built none or the artifact expired")
            return chosen(sha, web, run['id'], '', f"{sha}: the web app CI run {run['id']} built")
        main = newest_main(repo)
        if not main:
            return skip('no successful main push run still has a web app')
        name, run = main
        return chosen(run['head_sha'], name, run['id'], '', f"main at {run['head_sha']}: the newest web app")
    return skip(f'{event_name} does not start the suite')


def write_outputs(values):
    print(json.dumps(values, indent=2))
    if os.environ.get('GITHUB_OUTPUT'):
        with open(os.environ['GITHUB_OUTPUT'], 'a', encoding='utf-8') as output:
            for key, value in values.items():
                output.write(f'{key}={value if isinstance(value, str) else json.dumps(value)}\n')
    if os.environ.get('GITHUB_STEP_SUMMARY'):
        with open(os.environ['GITHUB_STEP_SUMMARY'], 'a', encoding='utf-8') as summary:
            summary.write(f"E2E suite {'runs' if values['run'] else 'skipped'}: {values['reason']}.\n")


def main():
    event = json.loads(Path(os.environ['GITHUB_EVENT_PATH']).read_text(encoding='utf-8'))
    try:
        values = resolve(os.environ['GITHUB_EVENT_NAME'], event, os.environ['GH_REPO'])
    except (ValueError, subprocess.CalledProcessError) as exc:
        print(f'::error::{exc}')
        return 1
    write_outputs(values)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
