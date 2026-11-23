#!/usr/bin/env python3
"""Require a trusted key-scan receipt for the UI gallery bytes a Sites image ships.

The Shared UI library run (ui-library.yml) puts its hosted gallery build in the `ui-review-<sha>` artifact as
`ui-gallery/`. The UI library key scan (ui-library-key-scan.yml) runs from the default branch on workflow_run and reads
that artifact. The Sites image carries a separate Nix build of the same gallery (infra/packages/ui-gallery.nix).
Before an image is published, this script requires, for a commit whose gallery bytes are the image's:

- a run of ui-library-key-scan.yml, on the default branch and from a workflow_run, created between the start of the
  latest attempt of the commit's Shared UI library push run and an hour after that run ended, whose key-scan job
  succeeded and whose log names the commit (HEAD_SHA), ran the key check and downloaded that run's `ui-review-<sha>`
  artifact by id. The newest such run decides. The "UI library key scan" check run is not read: any workflow holding
  checks: write can post one, and one posted from a workflow_run lands in the triggering run's check suite, so its
  suite cannot name the scan workflow;
- that artifact holding exactly the bytes of the image's `build/client/ui-gallery`.

The commits tried are the head, then, newest first, the head's ancestors that last changed a gallery input (GALLERY
INPUTS): the gallery bytes only change with them, so the newest such commit built the bytes the head ships. A missing,
stale, foreign or failed scan, a missing artifact, a digest mismatch or any GitHub answer that fails, is empty or is
not JSON fails: nothing is published unscanned.
"""
import argparse
from datetime import datetime, timedelta, timezone
import hashlib
import io
import json
import os
from pathlib import Path
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import zipfile

LIBRARY_WORKFLOW = '.github/workflows/ui-library.yml'
SCAN_WORKFLOW = '.github/workflows/ui-library-key-scan.yml'
PREFIX = 'ui-gallery/'
# What the gallery bytes are built from: infra/packages/ui-gallery.nix's fileset roots, the package itself and the
# flake lock that pins its Node and pnpm. The Shared UI library scope (both workflows' pathspecs) holds all of them.
GALLERY_INPUTS = ('library/ui', 'library/explorer-core', 'infra/packages/ui-gallery.nix', 'flake.lock')
# The newest commits, per input, that changed it. The newest one overall built the bytes the head ships; older ones
# only matter when that one has no usable receipt, so one page per input reaches far past the 90 days a main
# artifact is kept.
CANDIDATES_PER_INPUT = 100
PAGE = 100
MAX_PAGES = 10
# A scan run is created when its Shared UI library run ends; a rerun moves the window with the run's latest attempt.
SCAN_MARGIN = timedelta(hours=1)
RECONCILE = ('The hosted gallery the Shared UI library run scanned and the one Nix built for the image differ. Both '
             'must build with the same Node major and pnpm lockfile: compare setup-node in the library job of '
             '.github/workflows/ui-library.yml with nodejs-slim in infra/packages/ui-gallery.nix, and the fileset '
             'of that package with the files the gallery reads.')


class ReceiptError(Exception):
    pass


class FetchError(Exception):
    """A GitHub API call that failed or answered nothing usable. It always fails the receipt: an unreadable answer is
    never read as "nothing found", so it cannot hide a newer failing scan behind an older passing one."""


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


class _NoAuthOnRedirect(urllib.request.HTTPRedirectHandler):
    """Logs and artifacts redirect to signed storage URLs, which must not receive the token."""

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        new = super().redirect_request(req, fp, code, msg, headers, newurl)
        if new is not None and urllib.parse.urlsplit(newurl).netloc != urllib.parse.urlsplit(req.full_url).netloc:
            new.remove_header('Authorization')
        return new


class GitHub:
    """The REST API over HTTPS with the job's token. Logs come back as bytes, escape sequences included."""

    def __init__(self, repo, token, api='https://api.github.com'):
        self.repo, self.token, self.base = repo, token, api
        self.opener = urllib.request.build_opener(_NoAuthOnRedirect)

    def _url(self, path):
        return f'{self.base}/repos/{self.repo}/{path}' if path else f'{self.base}/repos/{self.repo}'

    def _get(self, path):
        request = urllib.request.Request(self._url(path), headers={
            'Accept': 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28',
            'Authorization': f'Bearer {self.token}', 'User-Agent': 'ui-gallery-receipt'})
        try:
            with self.opener.open(request, timeout=60) as response:
                body = response.read()
        except (urllib.error.URLError, OSError) as error:
            raise FetchError(f'GET {path} failed: {error}') from error
        if not body:
            raise FetchError(f'GET {path} answered nothing')
        return body

    def api(self, path):
        try:
            return json.loads(self._get(path))
        except ValueError as error:
            raise FetchError(f'GET {path} answered no JSON: {error}') from error

    def pages(self, path, key, max_pages=MAX_PAGES):
        """Every item under `key` across pages of `path` (which carries per_page=PAGE); fails past `max_pages`."""
        items = []
        for page in range(1, max_pages + 1):
            answer = self.api(f'{path}&page={page}')
            if not isinstance(answer.get(key), list):
                raise FetchError(f'GET {path} answered no {key}')
            items += answer[key]
            if len(answer[key]) < PAGE:
                return items
        raise FetchError(f'GET {path} holds more than {max_pages} pages')

    def text(self, path):
        return self._get(path).decode('utf-8', errors='replace')

    def download(self, artifact_id):
        return self._get(f'actions/artifacts/{artifact_id}/zip')


def _time(value):
    return datetime.fromisoformat(value.replace('Z', '+00:00'))


def _stamp(value):
    return value.astimezone(timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')


def library_run(gh, sha):
    """The commit's Shared UI library push run, latest attempt; None when there is none."""
    runs = gh.pages(f'actions/workflows/ui-library.yml/runs?head_sha={sha}&event=push&per_page={PAGE}',
                    'workflow_runs')
    runs = [run for run in runs if run.get('head_sha') == sha and run.get('event') == 'push'
            and run.get('path') == LIBRARY_WORKFLOW]
    return max(runs, key=lambda run: run['id']) if runs else None


def built_gallery(gh, run):
    """Whether the run's library job built the gallery artifact; None while the run is not finished."""
    if run.get('status') != 'completed':
        return None
    if run.get('conclusion') != 'success':
        raise ReceiptError(f'the Shared UI library run {run["id"]} concluded {run.get("conclusion")}')
    jobs = gh.pages(f'actions/runs/{run["id"]}/jobs?per_page={PAGE}', 'jobs')
    return any(job.get('name') == 'library' and job.get('conclusion') == 'success' for job in jobs)


def trusted_scan(gh, sha, run, default_branch):
    """The newest run of the trusted scan workflow that read `run`'s artifact for `sha`, as (conclusion, artifact id);
    ('in_progress', None) while a scan in the window is unfinished, None when none read it."""
    started = _time(run['run_started_at'])
    until = _time(run['updated_at']) + SCAN_MARGIN
    runs = gh.pages(f'actions/workflows/ui-library-key-scan.yml/runs?event=workflow_run&branch={default_branch}'
                    f'&created={_stamp(started)}..{_stamp(until)}&per_page={PAGE}', 'workflow_runs')
    pending = False
    for scan in sorted(runs, key=lambda scan: scan['id'], reverse=True):
        if (scan.get('path') != SCAN_WORKFLOW or scan.get('event') != 'workflow_run'
                or scan.get('head_branch') != default_branch or not started <= _time(scan['created_at']) <= until):
            continue
        jobs = [job for job in gh.pages(f'actions/runs/{scan["id"]}/jobs?per_page={PAGE}', 'jobs')
                if job.get('name') == 'key-scan']
        if not jobs:
            continue
        job = max(jobs, key=lambda job: job['id'])
        if job.get('status') != 'completed':
            pending = True
            continue
        # Only a skipped job has no log, and a skipped scan read nothing. Any other log must be read, or the receipt
        # fails (FetchError): an unreadable log could be the newest scan of this commit, and it may have failed.
        if job.get('conclusion') == 'skipped':
            continue
        log = gh.text(f'actions/jobs/{job["id"]}/logs')
        if not re.search(rf'^\S*\s+HEAD_SHA: {sha}\r?$', log, re.M):
            continue
        downloaded = re.search(rf'^\S*\s+- ui-review-{sha} \(ID: (\d+),', log, re.M)
        if downloaded:
            # A pull request whose head is this commit gets a scan of its own run's artifact: not this run's.
            artifact = gh.api(f'actions/artifacts/{downloaded[1]}')
            if (artifact.get('workflow_run') or {}).get('id') != run['id']:
                continue
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


def receipt_for(gh, sha, digest, default_branch, now):
    """'ok' when a trusted scan of `sha` read `digest`; 'none' when `sha` has no scanned gallery; 'wait' while its run
    or scan is unfinished; raises when the scan failed or read other bytes."""
    run = library_run(gh, sha)
    if run is None:
        return 'none'
    built = built_gallery(gh, run)
    if built is None:
        return 'wait'
    if not built:
        return 'none'
    scan = trusted_scan(gh, sha, run, default_branch)
    if scan is None:
        return 'wait' if now() <= _time(run['updated_at']) + SCAN_MARGIN else 'none'
    if scan[0] == 'in_progress':
        return 'wait'
    conclusion, artifact_id = scan
    if conclusion != 'success':
        raise ReceiptError(f'the trusted key scan of {sha} concluded {conclusion}')
    scanned = scanned_digest(gh, sha, artifact_id, run)
    if scanned != digest:
        raise ReceiptError(f'the key scan of {sha} read {scanned}, but the image ships {digest}. {RECONCILE}')
    return 'ok'


def candidates(gh, head):
    """The head, then its ancestors that changed a gallery input, newest first."""
    commits = {}
    for path in GALLERY_INPUTS:
        answer = gh.api(f'commits?sha={head}&path={urllib.parse.quote(path)}&per_page={CANDIDATES_PER_INPUT}')
        if not isinstance(answer, list):
            raise FetchError(f'GET commits for {path} answered no list')
        for commit in answer:
            commits[commit['sha']] = _time(commit['commit']['committer']['date'])
    commits.pop(head, None)
    return [head] + sorted(commits, key=lambda sha: commits[sha], reverse=True)


def require_receipt(gh, head, digest, wait=2700, poll=30, sleep=time.sleep,
                    now=lambda: datetime.now(timezone.utc)):
    default_branch = gh.api('')['default_branch']
    tried = candidates(gh, head)
    deadline = time.monotonic() + wait
    while True:
        for sha in tried:
            try:
                state = receipt_for(gh, sha, digest, default_branch, now)
            except ReceiptError:
                if sha == head:
                    raise
                continue
            if state == 'ok':
                return sha
            if state == 'wait':
                break
        else:
            raise ReceiptError(
                f'no trusted key scan of {head} or of the {len(tried) - 1} ancestors that last changed '
                f'{", ".join(GALLERY_INPUTS)} read the bytes {digest}: the receipt expired (main artifacts are kept '
                f'90 days) or was never made. A main commit that changes one of them makes a new one. If the bytes '
                f'still differ from every scan, {RECONCILE}')
        if time.monotonic() >= deadline:
            raise ReceiptError(f'the trusted key scan of {sha} did not finish within {wait}s')
        sleep(poll)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--repo', required=True)
    parser.add_argument('--head', required=True)
    parser.add_argument('--gallery', required=True, help="the image's build/client/ui-gallery")
    args = parser.parse_args(argv)
    token = os.environ.get('GH_TOKEN') or os.environ.get('GITHUB_TOKEN')
    try:
        if not token:
            raise FetchError('GH_TOKEN is not set')
        digest = tree_digest(args.gallery)
        receipt = require_receipt(GitHub(args.repo, token), args.head, digest)
    except (ReceiptError, FetchError, zipfile.BadZipFile, KeyError, ValueError, TypeError) as error:
        print(f'::error::UI gallery receipt: {error}', file=sys.stderr)
        return 1
    print(f'The Sites image ships UI gallery {digest}, scanned on {receipt}')
    if os.environ.get('GITHUB_STEP_SUMMARY'):
        with open(os.environ['GITHUB_STEP_SUMMARY'], 'a') as summary:
            print(f'UI gallery {digest}: key scan receipt on {receipt}', file=summary)
    return 0


if __name__ == '__main__':
    sys.exit(main())
