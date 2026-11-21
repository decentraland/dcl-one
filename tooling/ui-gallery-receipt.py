#!/usr/bin/env python3
"""Require a trusted key-scan receipt for the UI gallery bytes a Sites image ships.

The Shared UI library run (ui-library.yml) puts its hosted gallery build in the `ui-review-<sha>` artifact as
`ui-gallery/`. The UI library key scan (ui-library-key-scan.yml) runs from the default branch on workflow_run, reads
that artifact and posts the "UI library key scan" check run on the commit. The Sites image carries a separate Nix
build of the same gallery (infra/packages/ui-gallery.nix). Before an image is published, this script requires:

- a run of ui-library-key-scan.yml, on the default branch and from a workflow_run, created after the latest attempt
  of the commit's Shared UI library push run started, whose key-scan job succeeded and whose log names the commit
  (HEAD_SHA), ran the key check and downloaded the `ui-review-<sha>` artifact by id. The newest such run decides. The
  check run alone is not trusted: any workflow holding checks: write can post one, and a check run posted from a
  workflow_run lands in the triggering run's check suite, so its suite cannot name the scan workflow;
- that artifact, uploaded by that Shared UI library run, holding exactly the bytes of the image's
  `build/client/ui-gallery`.

For the head commit when its Shared UI library run built the gallery (it waits for the run and its scan); otherwise
(library/ui and the gallery's Nix inputs unchanged at the head, so no artifact) for the nearest first-parent ancestor
whose trusted scan read identical bytes. A missing, stale, foreign or failed scan, a missing artifact or a digest
mismatch fails: nothing is published unscanned.
"""
import argparse
from datetime import datetime, timedelta
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
import zipfile

LIBRARY_WORKFLOW = '.github/workflows/ui-library.yml'
SCAN_WORKFLOW = '.github/workflows/ui-library-key-scan.yml'
PREFIX = 'ui-gallery/'
# The gallery bytes change with library/ui, which main changes every few hours, and every such main commit leaves a
# receipt whose artifact is kept 90 days. 200 first-parent commits is weeks of main history: a walk that ends without
# a receipt means the receipt expired or was never made, which a longer walk would not fix.
ANCESTORS = 200
SCAN_RUNS = 100
# A scan starts minutes after its Shared UI library run ends; a rerun moves the window with the run's latest attempt.
SCAN_WINDOW = timedelta(hours=6)
RECONCILE = ('The hosted gallery the Shared UI library run scanned and the one Nix built for the image differ. Both '
             'must build with the same Node major and pnpm lockfile: compare setup-node in the library job of '
             '.github/workflows/ui-library.yml with nodejs-slim in infra/packages/ui-gallery.nix, and the fileset '
             'of that package with the files the gallery reads.')


class ReceiptError(Exception):
    pass


def _digest(files):
    """sha256 over `<sha256>  <path>` lines sorted by path; the same digest as gallery/scripts/hosted-digest.mjs."""
    lines = [f'{hashlib.sha256(data).hexdigest()}  {name}' for name, data in sorted(files)]
    if not any(line.endswith('  index.html') for line in lines):
        raise ReceiptError('the gallery holds no index.html')
    return hashlib.sha256(''.join(f'{line}\n' for line in lines).encode()).hexdigest()


def _check_name(name):
    if any(part.startswith('.') or part == '' for part in name.split('/')):
        raise ReceiptError(f'{name} is a hidden or empty path, which the artifact upload drops unscanned')


def tree_digest(root):
    root = Path(root).resolve()
    files = []
    for path in sorted(root.rglob('*')):
        name = path.relative_to(root).as_posix()
        _check_name(name)
        if path.is_symlink():
            raise ReceiptError(f'{name} is a link')
        if path.is_dir():
            continue
        if not path.is_file():
            raise ReceiptError(f'{name} is not a regular file')
        files.append((name, path.read_bytes()))
    return _digest(files)


def artifact_digest(archive):
    files = []
    with zipfile.ZipFile(io.BytesIO(archive)) as zipped:
        for info in zipped.infolist():
            if not info.filename.startswith(PREFIX) or info.is_dir():
                continue
            name = info.filename[len(PREFIX):]
            _check_name(name)
            files.append((name, zipped.read(info)))
    if not files:
        raise ReceiptError(f'the artifact holds no {PREFIX}')
    return _digest(files)


class GitHub:
    def __init__(self, repo):
        self.repo = repo

    def _path(self, path):
        return f'repos/{self.repo}/{path}' if path else f'repos/{self.repo}'

    def api(self, path):
        return json.loads(subprocess.check_output(['gh', 'api', self._path(path)], text=True))

    def text(self, path):
        # A job with no log (skipped, or its log expired) reads as empty: it names no commit, so it proves nothing.
        result = subprocess.run(['gh', 'api', self._path(path)], capture_output=True, text=True, errors='replace')
        return result.stdout if result.returncode == 0 else ''

    def download(self, artifact_id):
        return subprocess.check_output(['gh', 'api', self._path(f'actions/artifacts/{artifact_id}/zip')])


def _time(value):
    return datetime.fromisoformat(value.replace('Z', '+00:00'))


def library_run(gh, sha):
    """The commit's Shared UI library push run, latest attempt; None when there is none."""
    runs = gh.api(f'actions/workflows/ui-library.yml/runs?head_sha={sha}&event=push&per_page=100')['workflow_runs']
    runs = [run for run in runs if run.get('head_sha') == sha and run.get('event') == 'push'
            and run.get('path') == LIBRARY_WORKFLOW]
    return max(runs, key=lambda run: run['id']) if runs else None


def built_gallery(gh, run):
    """Whether the run's library job built the gallery artifact; None while the run is not finished."""
    if run.get('status') != 'completed':
        return None
    if run.get('conclusion') != 'success':
        raise ReceiptError(f'the Shared UI library run {run["id"]} concluded {run.get("conclusion")}')
    jobs = gh.api(f'actions/runs/{run["id"]}/jobs?per_page=100')['jobs']
    return any(job.get('name') == 'library' and job.get('conclusion') == 'success' for job in jobs)


def trusted_scan(gh, sha, run, default_branch):
    """The newest run of the trusted scan workflow naming `sha`, created after `run`'s latest attempt started, as
    (conclusion, artifact id); None when none names `sha` yet."""
    started = run['run_started_at']
    until = (_time(started) + SCAN_WINDOW).strftime('%Y-%m-%dT%H:%M:%SZ')
    runs = gh.api(f'actions/workflows/ui-library-key-scan.yml/runs?event=workflow_run&branch={default_branch}'
                  f'&created={started}..{until}&per_page={SCAN_RUNS}')['workflow_runs']
    pending = False
    for scan in sorted(runs, key=lambda scan: scan['id'], reverse=True):
        if (scan.get('path') != SCAN_WORKFLOW or scan.get('event') != 'workflow_run'
                or scan.get('head_branch') != default_branch or _time(scan['created_at']) < _time(started)):
            continue
        jobs = [job for job in gh.api(f'actions/runs/{scan["id"]}/jobs?per_page=100')['jobs']
                if job.get('name') == 'key-scan']
        if not jobs:
            continue
        job = max(jobs, key=lambda job: job['id'])
        if job.get('status') != 'completed':
            pending = True
            continue
        log = gh.text(f'actions/jobs/{job["id"]}/logs')
        if not re.search(rf'^\S*\s+HEAD_SHA: {sha}\r?$', log, re.M):
            continue
        downloaded = re.search(rf'^\S*\s+- ui-review-{sha} \(ID: (\d+),', log, re.M)
        ran = re.search(r'^\S*\s+check key: ran\r?$', log, re.M)
        if job.get('conclusion') == 'success' and not (downloaded and ran):
            return ('success without reading the artifact', None)
        return (job.get('conclusion'), int(downloaded[1]) if downloaded else None)
    return ('in_progress', None) if pending else None


def scanned_digest(gh, sha, artifact_id, run):
    """The digest of `ui-gallery/` in the artifact the trusted scan downloaded, which `run` uploaded for `sha`."""
    artifact = gh.api(f'actions/artifacts/{artifact_id}')
    if (artifact.get('name') != f'ui-review-{sha}' or artifact.get('expired')
            or (artifact.get('workflow_run') or {}).get('id') != run['id']):
        raise ReceiptError(f'the scanned artifact {artifact_id} is expired or not the ui-review-{sha} of run {run["id"]}')
    return artifact_digest(gh.download(artifact_id))


def receipt_for(gh, sha, digest, default_branch):
    """'ok' when a trusted scan of `sha` read `digest`, 'none' when `sha` has no gallery to scan, 'wait' while its run
    or scan is not finished; raises when the scan failed or read other bytes."""
    run = library_run(gh, sha)
    if run is None:
        return 'none'
    built = built_gallery(gh, run)
    if built is None:
        return 'wait'
    if not built:
        return 'none'
    scan = trusted_scan(gh, sha, run, default_branch)
    if scan is None or scan[0] == 'in_progress':
        return 'wait'
    conclusion, artifact_id = scan
    if conclusion != 'success':
        raise ReceiptError(f'the trusted key scan of {sha} concluded {conclusion}')
    scanned = scanned_digest(gh, sha, artifact_id, run)
    if scanned != digest:
        raise ReceiptError(f'the key scan of {sha} read {scanned}, but the image ships {digest}. {RECONCILE}')
    return 'ok'


def require_receipt(gh, head, digest, wait=2700, poll=30, sleep=time.sleep):
    default_branch = gh.api('')['default_branch']
    deadline = time.monotonic() + wait
    while True:
        state = receipt_for(gh, head, digest, default_branch)
        if state == 'ok':
            return head
        if state == 'none':
            break
        if time.monotonic() >= deadline:
            raise ReceiptError(f'no completed trusted key scan of {head} within {wait}s')
        sleep(poll)
    sha = head
    for _ in range(ANCESTORS):
        parents = gh.api(f'commits/{sha}')['parents']
        if not parents:
            raise ReceiptError(f'no trusted key scan of {head} or its ancestors read the bytes {digest}')
        sha = parents[0]['sha']
        try:
            if receipt_for(gh, sha, digest, default_branch) == 'ok':
                return sha
        except ReceiptError:
            continue
    raise ReceiptError(
        f'no trusted key scan of {head} or its last {ANCESTORS} first-parent ancestors read the bytes {digest}: the '
        'receipt expired (main artifacts are kept 90 days) or was never made. A main commit that changes library/ui, '
        'library/explorer-core, infra/packages/ui-gallery.nix or flake.lock makes a new one. If the bytes still differ '
        f'from every scan, {RECONCILE}')


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--repo', required=True)
    parser.add_argument('--head', required=True)
    parser.add_argument('--gallery', required=True, help="the image's build/client/ui-gallery")
    args = parser.parse_args(argv)
    try:
        digest = tree_digest(args.gallery)
        receipt = require_receipt(GitHub(args.repo), args.head, digest)
    except (ReceiptError, subprocess.CalledProcessError, zipfile.BadZipFile, KeyError, ValueError) as error:
        print(f'::error::UI gallery receipt: {error}', file=sys.stderr)
        return 1
    print(f'The Sites image ships UI gallery {digest}, scanned on {receipt}')
    if os.environ.get('GITHUB_STEP_SUMMARY'):
        with open(os.environ['GITHUB_STEP_SUMMARY'], 'a') as summary:
            print(f'UI gallery {digest}: key scan receipt on {receipt}', file=summary)
    return 0


if __name__ == '__main__':
    sys.exit(main())
