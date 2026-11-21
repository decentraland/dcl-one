#!/usr/bin/env python3
"""Require a key-scan receipt for the UI gallery bytes a Sites image ships.

The Shared UI library run puts its hosted gallery build in the `ui-review-<sha>` artifact as `ui-gallery/`, and the
UI library key scan (ui-library-key-scan.yml, run from the default branch) reads that artifact and posts the
"UI library key scan" check run on the commit. The Sites image carries a separate Nix build of the same gallery
(infra/packages/ui-gallery.nix). Before an image is published, this script finds a successful scan whose artifact
holds exactly the bytes of the image's `build/client/ui-gallery`:

- the head commit's own scan, when the Shared UI library run for the head built the gallery: it waits for that run
  and its scan, and fails if either fails or the scanned bytes differ;
- otherwise (library/ui unchanged at the head, so no scan ran for it), the nearest first-parent ancestor with a
  successful scan whose artifact holds identical bytes.

A missing scan, a failed scan, a missing artifact or a digest mismatch fails: nothing is published unscanned.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import zipfile

CHECK = 'UI library key scan'
LIBRARY_WORKFLOW = '.github/workflows/ui-library.yml'
PREFIX = 'ui-gallery/'
ANCESTORS = 200


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

    def api(self, path):
        return json.loads(subprocess.check_output(['gh', 'api', f'repos/{self.repo}/{path}'], text=True))

    def download(self, artifact_id):
        return subprocess.check_output(['gh', 'api', f'repos/{self.repo}/actions/artifacts/{artifact_id}/zip'])


def scan(gh, sha):
    """The latest completed key-scan check run on `sha` posted by GitHub Actions, or None."""
    runs = gh.api(f'commits/{sha}/check-runs?check_name={CHECK.replace(" ", "%20")}&per_page=100')['check_runs']
    runs = [run for run in runs if run.get('name') == CHECK and (run.get('app') or {}).get('slug') == 'github-actions']
    if not runs:
        return None
    return max(runs, key=lambda run: (run.get('completed_at') or '', run['id']))


def scanned_digest(gh, sha):
    """The digest of `ui-gallery/` in the newest unexpired `ui-review-<sha>` of a Shared UI library run for `sha`."""
    artifacts = [artifact for artifact in gh.api(f'actions/artifacts?name=ui-review-{sha}&per_page=100')['artifacts']
                 if not artifact.get('expired') and (artifact.get('workflow_run') or {}).get('head_sha') == sha]
    for artifact in sorted(artifacts, key=lambda artifact: artifact['id'], reverse=True):
        run = gh.api(f'actions/runs/{artifact["workflow_run"]["id"]}')
        if run.get('path') == LIBRARY_WORKFLOW:
            return artifact_digest(gh.download(artifact['id']))
    raise ReceiptError(f'no unexpired ui-review-{sha} artifact of a Shared UI library run')


def head_scan_expected(gh, sha):
    """True when the Shared UI library run for `sha` built the gallery, False when it did not, None while it runs."""
    runs = gh.api(f'actions/workflows/ui-library.yml/runs?head_sha={sha}&per_page=100')['workflow_runs']
    runs = [run for run in runs if run.get('head_sha') == sha and run.get('event') == 'push']
    if not runs:
        return False
    run = max(runs, key=lambda run: (run.get('run_attempt', 1), run['id']))
    if run.get('status') != 'completed':
        return None
    if run.get('conclusion') != 'success':
        raise ReceiptError(f'the Shared UI library run for {sha} concluded {run.get("conclusion")}')
    jobs = gh.api(f'actions/runs/{run["id"]}/jobs?per_page=100')['jobs']
    return any(job.get('name') == 'library' and job.get('conclusion') == 'success' for job in jobs)


def require_receipt(gh, head, digest, wait=2700, poll=30, sleep=time.sleep):
    deadline = time.monotonic() + wait
    while True:
        expected = head_scan_expected(gh, head)
        if expected is False:
            break
        found = scan(gh, head) if expected else None
        if found and found.get('status') == 'completed':
            if found.get('conclusion') != 'success':
                raise ReceiptError(f'the key scan of {head} concluded {found.get("conclusion")}')
            scanned = scanned_digest(gh, head)
            if scanned != digest:
                raise ReceiptError(f'the key scan of {head} read {scanned}, but the image ships {digest}')
            return head
        if time.monotonic() >= deadline:
            raise ReceiptError(f'no completed key scan of {head} within {wait}s')
        sleep(poll)
    sha = head
    for _ in range(ANCESTORS):
        parents = gh.api(f'commits/{sha}')['parents']
        if not parents:
            break
        sha = parents[0]['sha']
        found = scan(gh, sha)
        if not found or found.get('status') != 'completed' or found.get('conclusion') != 'success':
            continue
        try:
            if scanned_digest(gh, sha) == digest:
                return sha
        except ReceiptError:
            continue
    raise ReceiptError(f'no successful key scan of {head} or its last {ANCESTORS} ancestors read the bytes {digest}')


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--repo', required=True)
    parser.add_argument('--head', required=True)
    parser.add_argument('--gallery', required=True, help="the image's build/client/ui-gallery")
    args = parser.parse_args(argv)
    try:
        digest = tree_digest(args.gallery)
        receipt = require_receipt(GitHub(args.repo), args.head, digest)
    except (ReceiptError, subprocess.CalledProcessError, zipfile.BadZipFile, KeyError) as error:
        print(f'::error::UI gallery receipt: {error}', file=sys.stderr)
        return 1
    print(f'The Sites image ships UI gallery {digest}, scanned on {receipt}')
    if os.environ.get('GITHUB_STEP_SUMMARY'):
        with open(os.environ['GITHUB_STEP_SUMMARY'], 'a') as summary:
            print(f'UI gallery {digest}: key scan receipt on {receipt}', file=summary)
    return 0


if __name__ == '__main__':
    sys.exit(main())
