#!/usr/bin/env python3
"""The inputs of Bevy Explorer / E2E: which web app the suite runs against, and from which commit.

    python3 tooling/bevy-e2e-inputs.py                 # resolve the run's inputs
    python3 tooling/bevy-e2e-inputs.py suite-changed   # does the pull request change the suite?

Outputs (stdout as JSON, and GITHUB_OUTPUT when set): `run` (true or false), `reason`, `sha` (the
commit whose suite runs, checked out by the job), `web_artifact` and `web_run_id` (the web app to
download, from tooling/benchmark-inputs.py's artifact search), and `pr` (the pull request number,
or empty). `suite-changed` writes `changed` (true or false) instead.

- `pull_request`: the `labeled` event of the RUN_LABEL label on a pull request of this repository
  that is not a draft: a maintainer's decision to run the pull request's code, as it is at that
  moment, on the GPU runner with the accounts. The label admits that commit alone: a push to the
  pull request (`synchronize`) takes the label off and runs nothing, and the maintainer puts it
  back to run the new commit; any other event, and a `labeled` event for another label, runs
  nothing. The job waits for the commit's Bevy Explorer / CI run (BEVY_E2E_CI_WAIT_SECONDS, polled
  every BEVY_E2E_CI_POLL_SECONDS) and takes the web app it built; a run whose web app has expired
  is skipped, saying so; a pull request whose CI built none, or that started no CI run, takes
  main's newest when it changes the suite, its workflow or its tooling, so its specs still run;
  one that changes neither is skipped.
- `workflow_run` of Bevy Explorer / CI: a successful push to main, with the web app that run
  built. A pull request's CI run is skipped here: its scenarios run from its own event.
- `schedule`: main's newest web app and the commit that built it.
- `workflow_dispatch`: `inputs.sha`'s CI run and web app, else main's newest.
"""
import importlib.util
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path
from urllib.parse import quote

_spec = importlib.util.spec_from_file_location('benchmark_inputs', Path(__file__).resolve().parent / 'benchmark-inputs.py')
bench = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(bench)

# What the workflow's `pull_request` trigger lists for the suite's own checks: the suite, the
# workflow and its tooling.
SUITE_PATHS = ('apps/bevy-explorer/e2e/', '.github/workflows/bevy-explorer-e2e.yml', 'tooling/bevy-e2e-', 'tooling/tests/test_bevy_e2e_')
# The label a maintainer puts on a pull request whose code may run the scenarios. The workflow's
# concurrency group names it too.
RUN_LABEL = 'ci:e2e'
# How long a pull request's job waits for the commit's Bevy Explorer / CI run, and how often it asks.
DEFAULT_WAIT_SECONDS = 5400
DEFAULT_POLL_SECONDS = 30
# How long a CI run may take to appear after the push: past it, the commit started none.
START_SECONDS = 180


def api_delete(path):
    """A DELETE of the GitHub REST API through gh."""
    subprocess.check_output(['gh', 'api', '--method', 'DELETE', path], timeout=60)


def skip(reason):
    return {'run': False, 'reason': reason, 'sha': '', 'web_artifact': '', 'web_run_id': '', 'pr': ''}


def unlabel(repo, pr):
    """Take RUN_LABEL off the pull request: the commit it admitted is no longer the head."""
    api_delete(f'repos/{repo}/issues/{pr}/labels/{quote(RUN_LABEL, safe="")}')


def web_app_of(repo, run_id):
    """The run's web app artifact: (name, 'kept'), (name, 'expired') or (None, 'none'). Expired
    artifacts are told apart from a run that built none: a pull request's web app is kept 7 days."""
    data = bench.api(f'repos/{repo}/actions/runs/{run_id}/artifacts?per_page=100')
    rows = [row for row in data['artifacts'] if re.fullmatch(re.escape(bench.WEB) + '-[0-9a-f]{40}', row['name'])]
    kept = [row for row in rows if not row['expired']]
    if kept:
        return kept[0]['name'], 'kept'
    if rows:
        return rows[0]['name'], 'expired'
    return None, 'none'


def chosen(sha, artifact, run_id, pr, reason):
    return {'run': True, 'reason': reason, 'sha': sha, 'web_artifact': artifact, 'web_run_id': str(run_id), 'pr': pr}


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


def ci_runs_of(repo, sha, events):
    """The commit's Bevy Explorer / CI runs from this repository, newest first, whatever their state."""
    runs = bench.api(f'repos/{repo}/{bench.RUNS_PATH}?head_sha={sha}&per_page=100')['workflow_runs']
    return [run for run in runs if run['path'] == bench.WORKFLOW and run['event'] in events and run['head_sha'] == sha
            and (run.get('head_repository') or {}).get('full_name') == repo]


class StillRunning(Exception):
    """The commit's CI run had not completed when the wait ran out; `args[0]` is its id."""


def wait_for_ci(repo, sha, wait_seconds, poll_seconds=DEFAULT_POLL_SECONDS, sleep=time.sleep, clock=time.monotonic):
    """The commit's newest Bevy Explorer / CI pull request run once it has completed (any conclusion).
    None when the commit started no run within START_SECONDS; StillRunning past `wait_seconds`."""
    started = clock()
    while True:
        runs = ci_runs_of(repo, sha, ('pull_request',))
        if runs and runs[0]['status'] == 'completed':
            return runs[0]
        now = clock()
        if not runs and now - started >= START_SECONDS:
            return None
        if now - started >= wait_seconds:
            raise StillRunning(runs[0]['id'] if runs else None)
        sleep(poll_seconds)


def against_main(repo, sha, pr, why):
    """The pull request's suite against main's newest web app, when the pull request changes the suite."""
    if not changes_suite(repo, pr):
        return skip(f"#{pr} changes neither the explorer's web app nor the suite")
    main = newest_main(repo)
    if not main:
        return skip(f"#{pr} {why} and no main run still has a web app")
    name, run = main
    return chosen(sha, name, run['id'], pr, f"#{pr} at {sha} ({why}) against main's web app from {run['head_sha']}")


def resolve_pull_request(event, repo, wait):
    pull = event['pull_request']
    pr = str(pull['number'])
    sha = pull['head']['sha']
    action = event.get('action')
    labelled = RUN_LABEL in [label['name'] for label in pull.get('labels') or []]
    if action == 'synchronize':
        if labelled:
            unlabel(repo, pr)
            return skip(f"#{pr}: the `{RUN_LABEL}` label admitted an earlier commit and is taken off; a maintainer puts it back to run {sha}")
        return skip(f"#{pr} has no `{RUN_LABEL}` label: a maintainer applies it to run the scenarios on {sha}")
    if action != 'labeled':
        return skip(f"#{pr}: the scenarios run when a maintainer applies the `{RUN_LABEL}` label, not on {action}")
    if (event.get('label') or {}).get('name') != RUN_LABEL:
        return skip(f"#{pr}: the label `{(event.get('label') or {}).get('name')}` is not `{RUN_LABEL}`")
    if ((pull.get('head') or {}).get('repo') or {}).get('full_name') != repo:
        return skip(f'#{pr} is not from this repository')
    if pull.get('draft'):
        return skip(f'#{pr} is a draft')
    try:
        run = wait(repo, sha)
    except StillRunning as exc:
        return skip(f"#{pr} at {sha}: CI run {exc.args[0]} has not completed in time")
    if run is None:
        return against_main(repo, sha, pr, 'started no CI run')
    if run['conclusion'] != 'success':
        return skip(f"#{pr} at {sha}: CI run {run['id']} ended {run['conclusion']}")
    web, state = web_app_of(repo, run['id'])
    if state == 'kept':
        return chosen(sha, web, run['id'], pr, f"#{pr} at {sha}: the web app CI run {run['id']} built")
    if state == 'expired':
        return skip(f"#{pr} at {sha}: the web app CI run {run['id']} built has expired; re-run that CI run or push a new commit, then apply the label again")
    return against_main(repo, sha, pr, 'no web input changed')


def resolve(event_name, event, repo, wait=None):
    if event_name == 'pull_request':
        return resolve_pull_request(event, repo, wait or (lambda repo_, sha: wait_for_ci(repo_, sha, DEFAULT_WAIT_SECONDS)))
    if event_name == 'workflow_run':
        trigger = event['workflow_run']
        if (trigger.get('head_repository') or {}).get('full_name') != repo:
            return skip('the CI run is not from this repository')
        if trigger['event'] == 'pull_request':
            return skip(f"CI run {trigger['id']} is a pull request's: its scenarios run from its own event once it is labelled `{RUN_LABEL}`")
        if trigger['conclusion'] != 'success':
            return skip(f"CI run {trigger['id']} did not succeed")
        if trigger['event'] != 'push':
            return skip(f"CI run {trigger['id']} was started by {trigger['event']}, not a push")
        if trigger.get('head_branch') != 'main':
            return skip(f"CI run {trigger['id']} is a push to {trigger.get('head_branch')}, not main")
        sha = trigger['head_sha']
        web = bench.named(bench.artifacts(repo, trigger['id']), bench.WEB)
        if not web:
            return skip(f"main's CI run {trigger['id']} built no web app")
        return chosen(sha, web, trigger['id'], '', f"main at {sha}: the web app CI run {trigger['id']} built")
    if event_name in ('schedule', 'workflow_dispatch'):
        sha = ((event.get('inputs') or {}).get('sha') or '').strip()
        if sha:
            if not bench.SHA.fullmatch(sha):
                raise ValueError('sha must be a full 40-character commit hash')
            run = run_of(repo, sha)
            if not run:
                return skip(f'{sha} has no successful Bevy Explorer / CI run')
            web, state = web_app_of(repo, run['id'])
            if state == 'expired':
                return skip(f"CI run {run['id']} of {sha} built a web app that has expired: re-run that CI run")
            if state == 'none':
                return skip(f"CI run {run['id']} of {sha} built no web app")
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
            if values['run']:
                # What an approver of the `e2e` deployment is approving: the code the job checks out.
                where = f"pull request #{values['pr']}, " if values['pr'] else ''
                summary.write(f"Code under test: {where}commit {values['sha']}.\n")


def suite_changed(event, repo):
    """`suite-changed`: whether the pull request of the event changes the suite, as `changed`."""
    changed = changes_suite(repo, event['pull_request']['number'])
    print(json.dumps({'changed': changed}))
    if os.environ.get('GITHUB_OUTPUT'):
        with open(os.environ['GITHUB_OUTPUT'], 'a', encoding='utf-8') as output:
            output.write(f'changed={json.dumps(changed)}\n')
    return 0


def main(argv):
    event = json.loads(Path(os.environ['GITHUB_EVENT_PATH']).read_text(encoding='utf-8'))
    repo = os.environ['GH_REPO']
    if argv[1:] == ['suite-changed']:
        return suite_changed(event, repo)
    if argv[1:]:
        print(__doc__, file=sys.stderr)
        return 2
    wait_seconds = int(os.environ.get('BEVY_E2E_CI_WAIT_SECONDS') or DEFAULT_WAIT_SECONDS)
    poll_seconds = int(os.environ.get('BEVY_E2E_CI_POLL_SECONDS') or DEFAULT_POLL_SECONDS)
    try:
        values = resolve(os.environ['GITHUB_EVENT_NAME'], event, repo,
                         wait=lambda repo_, sha: wait_for_ci(repo_, sha, wait_seconds, poll_seconds))
    except (ValueError, subprocess.CalledProcessError) as exc:
        print(f'::error::{exc}')
        return 1
    write_outputs(values)
    return 0


if __name__ == '__main__':
    raise SystemExit(main(sys.argv))
