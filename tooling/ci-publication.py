#!/usr/bin/env python3
"""Skip superseded main pushes after acquiring a non-cancelable publication slot."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess


def api(path):
    return json.loads(subprocess.check_output(
        ['gh', 'api', f'repos/{os.environ["GITHUB_REPOSITORY"]}/{path}'], text=True, timeout=60))


def consumer_changed(base, head, consumer):
    spec = importlib.util.spec_from_file_location('bevy_ci', Path(__file__).with_name('bevy-ci.py'))
    ci = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(ci)
    return ci.consumer_changed(base, head, consumer)


def current():
    # Tags and explicitly requested manual releases are never superseded by a push.
    event = os.environ['GITHUB_EVENT_NAME']
    if event == 'workflow_run':
        run_id, revision = os.environ['SOURCE_RUN'], os.environ['SOURCE_SHA']
    else:
        run_id, revision = os.environ['GITHUB_RUN_ID'], os.environ['GITHUB_SHA']
    if event not in ('push', 'workflow_run') or os.environ['GITHUB_REF'] != 'refs/heads/main':
        return True
    run = api(f'actions/runs/{run_id}')
    if (run['event'] != 'push' or run['head_branch'] != 'main'
            or run['head_sha'] != revision):
        raise ValueError('Publication run does not match this checkout')
    if event == 'workflow_run' and (run.get('status') != 'completed' or run.get('conclusion') != 'success'
            or run.get('path') != '.github/workflows/bevy-explorer-ci.yml'
            or run.get('head_repository', {}).get('full_name') != os.environ['GITHUB_REPOSITORY']):
        raise ValueError('Publication source is no longer a successful trusted CI run')
    if event == 'workflow_run' and run.get('run_attempt') != int(os.environ['SOURCE_ATTEMPT']):
        raise ValueError('Publication source attempt changed; wait for its new completion')
    # Use the latest triggered push, not branch HEAD: a newer docs-only commit may
    # not trigger this workflow. Failed newer builds must not roll tags backwards.
    runs = api(f'actions/workflows/{run["workflow_id"]}/runs?branch=main&event=push&per_page=100')['workflow_runs']
    if not runs:
        raise ValueError('GitHub returned no main push runs; refusing publication')
    newer = [row for row in runs if row['run_number'] > run['run_number']]
    if event == 'workflow_run':
        # A sites-only push must not discard the last headless release merely
        # because the producer DAG now also schedules image consumers.
        return not any(consumer_changed(revision, row['head_sha'], os.environ['SOURCE_CONSUMER'])
                       for row in newer)
    return not newer


def main():
    allowed = current()
    with Path(os.environ['GITHUB_OUTPUT']).open('a') as output:
        output.write(f'publish={str(allowed).lower()}\n')
    print('Publishing this run.' if allowed else 'Skipping publication: a newer main push supersedes this run.')


if __name__ == '__main__':
    main()
