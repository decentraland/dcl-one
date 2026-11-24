#!/usr/bin/env python3
"""Publish the hosted UI gallery of a pull request once the key scan has cleared it.

ui-gallery-publish.yml runs this from the default branch when the Shared UI library key scan completes. Nothing it
reads from a pull request is executed: the Review artifact is data, bound to the digest the scan downloaded. What it
publishes is the artifact's `ui-gallery/`: the hosted gallery build the Shared UI library run adds to it, the one Sites
serves, under the digest gallery/scripts/hosted-digest.mjs and tooling/ui-gallery-receipt.py compute.

  verify   the key scan that triggered the run downloaded and scanned the Review artifact of an open pull request
           from this repository at its current head, and that pull request changes a file under library/ui; prints
           the pull request, the head, the artifact id and digest, and the repository variables the later jobs use
  fetch    downloads that artifact by id, checks its digest and layout and unpacks ui-gallery/ (before any
           credential); prints the gallery digest of the unpacked files, which must equal the artifact's
  upload   copies those files, still at that gallery digest, to s3://$UI_GALLERY_BUCKET/ui-gallery/pr/<n>/<sha>/
           with explicit content types
  probe    waits until the preview answers 200 through $UI_GALLERY_BASE_URL
  comment  creates or updates the one gallery comment on the pull request
  check    checks the hosted gallery in $SITE_DIR as upload would, for ui-library.yml on the pull request's build
  csp      prints the Content-Security-Policy a preview is served with, which ui-library.yml opens that build under

A run that is not eligible (publication not configured, a fork, a closed or moved-on pull request, a scan that read no
Review artifact) ends with a notice and publishes nothing. Evidence that does not match fails the command.
"""
import datetime
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import struct
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import zipfile
import zlib


def _receipt():
    """tooling/ui-gallery-receipt.py, the Sites image's receipt check: its digest of a hosted gallery, over files and
    over an artifact's ui-gallery/, is the one this publisher binds to."""
    path = Path(__file__).with_name('ui-gallery-receipt.py')
    spec = importlib.util.spec_from_file_location('ui_gallery_receipt', path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


receipt = _receipt()

SCAN_WORKFLOW = '.github/workflows/ui-library-key-scan.yml'
SCANNED_WORKFLOW = '.github/workflows/ui-library.yml'
ENVIRONMENT = 'ui-gallery'
VARIABLES = ('UI_GALLERY_ROLE_ARN', 'UI_GALLERY_BUCKET', 'UI_GALLERY_AWS_REGION', 'UI_GALLERY_BASE_URL')
FORMATS = {
    'UI_GALLERY_ROLE_ARN': re.compile(r'arn:aws(?:-[a-z]+)*:iam::\d{12}:role/[\w+=,.@/-]{1,512}'),
    'UI_GALLERY_BUCKET': re.compile(r'(?!.*\.\.)[a-z0-9][a-z0-9.-]{1,61}[a-z0-9]'),
    'UI_GALLERY_AWS_REGION': re.compile(r'[a-z]{2}(?:-[a-z]+)+-\d{1,2}'),
    # The distribution's own CloudFront domain, never a host under a site of ours: previews run pull request scripts,
    # which must share no cookie with dcl.dev. An origin without a trailing slash: the preview is
    # <origin>/ui-gallery/pr/<n>/<sha>/index.html.
    'UI_GALLERY_BASE_URL': re.compile(r'https://[a-z0-9]+\.cloudfront\.net'),
}
# The bucket's lifecycle rule expires ui-gallery/pr/ objects this many days after their upload.
RETENTION_DAYS = 5
MARKER = '<!-- ui-gallery-preview -->'
BOT = 'github-actions[bot]'
SHA = re.compile(r'[0-9a-f]{40}')
DIGEST = re.compile(r'sha256:[0-9a-f]{64}')
GALLERY_DIGEST = re.compile(r'[0-9a-f]{64}')
CONTROL = re.compile(r'[\x00-\x1f\x7f]')
# Only a pull request that changes a file under this tree gets a preview. The library job also runs for the other trees
# the gallery compiles (library/explorer-core) and for its build inputs, which alone get no link.
LIBRARY = 'library/ui/'
# GitHub lists at most this many files of a pull request.
LISTED_FILES = 3000
GALLERY = receipt.PREFIX
# Every file type the hosted gallery may hold and the type it is served with; any other file fails the fetch.
TYPES = {
    'html': 'text/html; charset=utf-8',
    'js': 'text/javascript; charset=utf-8',
    'css': 'text/css; charset=utf-8',
    'json': 'application/json',
    'map': 'application/json',
    'txt': 'text/plain; charset=utf-8',
    'svg': 'image/svg+xml',
    'png': 'image/png',
    'jpg': 'image/jpeg',
    'jpeg': 'image/jpeg',
    'gif': 'image/gif',
    'webp': 'image/webp',
    'avif': 'image/avif',
    'ico': 'image/x-icon',
    'woff2': 'font/woff2',
    'woff': 'font/woff',
    'mp4': 'video/mp4',
    'webm': 'video/webm',
}
# Files Vite names by their content hash under assets/ never change, so they are cached for a day. Every other file
# keeps its name across builds and a later build of the same head replaces it, so it is re-read soon.
PAGE_CACHE = 'public, max-age=300'
ASSET_CACHE = 'public, max-age=86400, immutable'
# Its characters are also literal in an aws s3 cp filter.
HASHED = re.compile(r'assets/[\w.-]+-[\w-]{8}\.[a-z0-9]+')
# The key guard's limits for an unpacked artifact, applied to ui-gallery/; the download itself is capped too.
LIMITS = {'files': 5000, 'total_bytes': 256 * 1024 * 1024, 'file_bytes': 16 * 1024 * 1024, 'depth': 32}
ARCHIVE_BYTES = 512 * 1024 * 1024
# The Content-Security-Policy dcl-one-infra's distribution adds to every preview response. ui-library.yml opens the pull
# request's build under it, and the probe links no preview served with other directives.
PREVIEW_CSP = (
    "default-src 'none'", "script-src 'self'", "style-src 'self'", "style-src-attr 'unsafe-inline'",
    "img-src 'self' data:", "font-src 'self'", "media-src 'self'", "connect-src 'self'", "frame-src 'self'",
    "frame-ancestors 'self'", "base-uri 'none'", "form-action 'none'", "object-src 'none'",
)
PROBE_DELAYS = (5, 10, 15, 20, 20, 20, 20)
USER_AGENT = 'dcl-one-ui-gallery-publish'


class Skip(Exception):
    """The run is not eligible: publish nothing, report a notice, succeed."""


class Reject(Exception):
    """The evidence does not match: publish nothing, fail."""


class ApiError(Exception):
    def __init__(self, path, status, message):
        super().__init__(f'GitHub API {path}: {message}')
        self.status = status


def query_fields(query):
    return [argument for key, value in query.items() for argument in ('-f', f'{key}={value}')]


class GitHub:
    """The REST API through the gh CLI, which authenticates with GH_TOKEN and follows download redirects."""

    def _call(self, arguments, path):
        try:
            result = subprocess.run(['gh', 'api', *arguments, path], capture_output=True, timeout=180)
        except subprocess.TimeoutExpired:
            raise ApiError(path, None, 'no answer within 180 seconds') from None
        if result.returncode:
            message = result.stderr.decode(errors='replace').strip()
            status = re.search(r'HTTP (\d{3})', message)
            raise ApiError(path, int(status.group(1)) if status else None, message or 'gh api failed')
        return result.stdout

    def _json(self, arguments, path):
        try:
            return json.loads(self._call(arguments, path) or b'null')
        except json.JSONDecodeError as error:
            raise ApiError(path, None, f'answered with invalid JSON: {error}') from None

    def get(self, path, **query):
        return self._json(['--method', 'GET', *query_fields(query)], path)

    def pages(self, path, key=None, **query):
        """Every item of a paginated list: the pages themselves, or each page's `key` array."""
        pages = self._json(['--paginate', '--slurp', '--method', 'GET', *query_fields(query)], path)
        return [item for page in pages for item in (page if key is None else page.get(key, []))]

    def text(self, path):
        """A plain text answer, such as a job log. Logs carry ANSI colour codes, which gh 2.97 and later refuses to
        print without --allow-escape-sequences; an older gh does not know the flag. The text only reaches this
        process, never a terminal."""
        try:
            answer = self._call(['--allow-escape-sequences'], path)
        except ApiError as error:
            if 'unknown flag: --allow-escape-sequences' not in str(error):
                raise
            answer = self._call([], path)
        return answer.decode('utf-8', errors='replace')

    def send(self, method, path, **fields):
        return self._json(['--method', method, *query_fields(fields)], path)

    def download(self, path, target, limit):
        size = 0
        with open(target, 'xb') as sink, subprocess.Popen(['gh', 'api', path], stdout=subprocess.PIPE,
                                                          stderr=subprocess.PIPE) as process:
            for chunk in iter(lambda: process.stdout.read(1 << 20), b''):
                size += len(chunk)
                if size > limit:
                    process.kill()
                    raise Reject(f'{path} is larger than {limit} bytes')
                sink.write(chunk)
            error = process.stderr.read().decode(errors='replace').strip()
            try:
                status = process.wait(timeout=60)
            except subprocess.TimeoutExpired:
                process.kill()
                raise ApiError(path, None, 'the download did not finish within 60 seconds') from None
            if status:
                raise ApiError(path, None, error or 'download failed')


def configuration(env, names=VARIABLES, required=False):
    """The repository variables, used exactly as the workflow passes them on: a blank one is missing, and one with
    surrounding spaces is invalid."""
    values = {name: env.get(name) or '' for name in names}
    missing = [name for name, value in values.items() if not value.strip()]
    if missing:
        message = 'publication is not configured: set the repository variables ' + ', '.join(missing)
        raise (Reject if required else Skip)(message)
    for name, value in values.items():
        if not FORMATS[name].fullmatch(value):
            raise Reject(f'{name} is not a valid value: {value!r}')
    return values


def number(env, name):
    value = env.get(name) or ''
    if not re.fullmatch(r'[1-9]\d{0,19}', value):
        raise Reject(f'{name} is not a positive integer: {value!r}')
    return int(value)


def head_sha(env, name='HEAD_SHA'):
    value = env.get(name) or ''
    if not SHA.fullmatch(value):
        raise Reject(f'{name} is not a commit SHA: {value!r}')
    return value


def preview_prefix(pull, head):
    return f'ui-gallery/pr/{pull}/{head}'


def preview_url(base, pull, head):
    return f'{base}/{preview_prefix(pull, head)}/index.html'


# --- verify ---------------------------------------------------------------------------------------------------------

def environment_ready(github, repository, default_branch):
    """The environment the writer role trusts exists and deploys from the default branch only, administrators
    included. A job naming an environment that does not exist would create it without that rule."""
    try:
        environment = github.get(f'repos/{repository}/environments/{ENVIRONMENT}')
    except ApiError as error:
        if error.status == 404:
            raise Skip(f'the {ENVIRONMENT} environment does not exist yet; create it with {default_branch} as its '
                       'only deployment branch') from None
        raise
    if environment.get('can_admins_bypass') is not False:
        raise Skip(f'the {ENVIRONMENT} environment must not let administrators bypass its rules')
    policy = environment.get('deployment_branch_policy') or {}
    if policy.get('custom_branch_policies') is not True or policy.get('protected_branches') is not False:
        raise Skip(f'the {ENVIRONMENT} environment must allow {default_branch} only')
    branches = github.get(f'repos/{repository}/environments/{ENVIRONMENT}/deployment-branch-policies')
    allowed = sorted((item.get('name'), item.get('type', 'branch')) for item in branches.get('branch_policies', []))
    if allowed != [(default_branch, 'branch')]:
        raise Skip(f'the {ENVIRONMENT} environment must allow {default_branch} only, not {allowed}')


def workflow_id(github, repository, path):
    return github.get(f'repos/{repository}/actions/workflows/{PurePosixPath(path).name}').get('id')


def check_scan_run(github, run, repository, default_branch, attempt):
    """The triggering run is main's key scan workflow, by path and by the id GitHub gave that file, never by name."""
    if run.get('path') != SCAN_WORKFLOW or run.get('workflow_id') != workflow_id(github, repository, SCAN_WORKFLOW):
        raise Skip(f'the triggering run {run.get("id")} is not {SCAN_WORKFLOW}')
    if (run.get('event') != 'workflow_run' or run.get('head_branch') != default_branch
            or (run.get('head_repository') or {}).get('full_name') != repository):
        raise Skip('the triggering key scan did not run from the default branch of this repository')
    if run.get('status') != 'completed' or run.get('run_attempt') != attempt:
        raise Skip(f'the key scan is no longer the completed attempt {attempt}; its newer attempt publishes on its '
                   'own completion')
    if run.get('conclusion') != 'success':
        raise Skip(f'the key scan concluded {run.get("conclusion")}')


def scan_job(jobs):
    """The scan's key-scan job, which ran every step: its one conditional step downloads the Review artifact, and it
    runs only when the scanned range changes the UI library. An attempt that re-ran only some jobs may list key-scan
    alone."""
    names = [job.get('name') for job in jobs]
    if set(names) - {'prepare', 'key-scan'} or any(names.count(name) > 1 for name in names):
        raise Reject(f'the key scan ran the jobs {sorted(map(str, names))}, not prepare and key-scan')
    if 'key-scan' not in names:
        raise Skip('the key-scan job did not run in this attempt: nothing was scanned')
    job = next(job for job in jobs if job.get('name') == 'key-scan')
    if job.get('conclusion') != 'success':
        raise Skip(f'the key-scan job concluded {job.get("conclusion")}: nothing was scanned')
    idle = [step.get('name') for step in job.get('steps', []) if step.get('conclusion') != 'success']
    if idle:
        raise Skip('the key scan read no Review artifact (steps not run: ' + ', '.join(map(str, idle)) + ')')
    return job


TIMESTAMP = re.compile(r'^\ufeff?\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z ')
COLOR = re.compile(r'\x1b\[[0-9;]*m')
STEP = '##[group]Run '
ACTION_STEP = re.compile(r'##\[group\]Run (actions/(?:checkout|setup-node|download-artifact))@[0-9a-f]{40}')
LISTED = re.compile(r'- (\S+) \(ID: (\d+), Size: (\d+), Expected Digest: (sha256:[0-9a-f]{64})\)')
DOWNLOADED = re.compile(r'SHA256 digest of downloaded artifact is ([0-9a-f]{64})')


def scanned_artifact(log, repository):
    """The Review artifact the key-scan job downloaded: the runner's echo of the download step's inputs and the
    action's own report of the id and digest it fetched. Only the action steps before the first run step are read.
    The first run step is the key step, which prints scanner output about pull request files, so nothing from it on
    counts as evidence."""
    sections = []
    for raw in log.splitlines():
        line = COLOR.sub('', TIMESTAMP.sub('', raw, count=1))
        if line.startswith(STEP):
            sections.append([line])
        elif sections:
            sections[-1].append(line)
    reviews = []
    for section in sections:
        action = ACTION_STEP.fullmatch(section[0])
        if not action:
            break
        if action.group(1) != 'actions/download-artifact':
            continue
        # The runner echoes the step's inputs under `with:` and its environment under `env:`, in the same shape: only
        # the lines under `with:` are inputs.
        inputs, block = {}, None
        for line in section[1:]:
            if line.startswith('##[endgroup]'):
                break
            if not line.startswith(' '):
                block = line
                continue
            match = re.fullmatch(r'  ([\w-]+): (.*)', line)
            if match and block == 'with:':
                inputs[match.group(1)] = match.group(2)
        if inputs.get('name', '').startswith('ui-review-'):
            reviews.append((inputs, section))
    if len(reviews) != 1:
        raise Reject(f'the key scan log shows {len(reviews)} Review downloads before the key step, not one')
    inputs, section = reviews[0]
    name, head = inputs['name'], inputs['name'].removeprefix('ui-review-')
    if not SHA.fullmatch(head) or not re.fullmatch(r'\d+', inputs.get('run-id', '')):
        raise Reject(f'the key scan downloaded {name!r} from run {inputs.get("run-id")!r}')
    if inputs.get('repository', repository) != repository:
        raise Reject(f'the key scan downloaded the Review artifact from {inputs.get("repository")}')
    listed = [match for match in map(LISTED.fullmatch, section) if match]
    downloaded = [match.group(1) for match in map(DOWNLOADED.fullmatch, section) if match]
    if len(listed) != 1 or listed[0].group(1) != name:
        raise Reject(f'the key scan log names no single artifact {name}')
    if len(downloaded) != 1 or f'sha256:{downloaded[0]}' != listed[0].group(4):
        raise Reject('digest mismatch: the key scan log does not show the expected digest downloaded')
    if 'Artifact download completed successfully.' not in section:
        raise Reject('the key scan log does not show the Review download completing')
    return {'head': head, 'run': int(inputs['run-id']), 'artifact': int(listed[0].group(2)),
            'digest': listed[0].group(4)}


def check_scanned_run(github, run, repository, head):
    if run.get('path') != SCANNED_WORKFLOW or run.get('workflow_id') != workflow_id(github, repository,
                                                                                     SCANNED_WORKFLOW):
        raise Reject(f'the scanned run {run.get("id")} is not {SCANNED_WORKFLOW}')
    if run.get('head_sha') != head:
        raise Reject(f'the scanned run {run.get("id")} built {run.get("head_sha")}, not {head}')
    if run.get('event') != 'pull_request':
        raise Skip(f'the scanned run was triggered by {run.get("event")}, not by a pull request')
    if (run.get('head_repository') or {}).get('full_name') != repository:
        raise Skip('the pull request comes from a fork, which never publishes a preview')
    if run.get('status') != 'completed' or run.get('conclusion') != 'success':
        raise Skip(f'the scanned run is {run.get("status")} with conclusion {run.get("conclusion")}')


def library_built(jobs):
    """The library job runs only when the ui_changed gate of ui-library.yml is true."""
    library = [job for job in jobs if job.get('name') == 'library']
    if len(library) != 1 or library[0].get('conclusion') != 'success':
        raise Skip('the scanned run did not build the UI library: no UI change')


def open_pull_request(github, repository, run, head):
    """The one open pull request from the scanned branch of this repository, whose head is still the scanned SHA."""
    branch = run.get('head_branch') or ''
    owner = repository.split('/')[0]
    pulls = [pull for pull in github.pages(f'repos/{repository}/pulls', head=f'{owner}:{branch}', state='all',
                                           per_page=100)
             if (pull.get('base', {}).get('repo') or {}).get('full_name') == repository
             and (pull.get('head', {}).get('repo') or {}).get('full_name') == repository]
    current = [pull for pull in pulls if pull.get('state') == 'open']
    if not current:
        closed = f' (#{pulls[0]["number"]} is closed)' if pulls else ''
        raise Skip(f'no open pull request from {branch}{closed}')
    if len(current) > 1:
        raise Skip(f'{len(current)} open pull requests come from {branch}')
    pull = current[0]
    if pull['head']['sha'] != head:
        raise Skip(f'pull request #{pull["number"]} moved on to {pull["head"]["sha"]}; its own scan publishes it')
    return pull['number']


def changes_library(github, repository, pull):
    """The pull request adds, changes, removes or renames a file under library/ui."""
    files = github.pages(f'repos/{repository}/pulls/{pull}/files', per_page=100)
    if any(str(item.get(key) or '').startswith(LIBRARY) for item in files for key in ('filename', 'previous_filename')):
        return
    if len(files) >= LISTED_FILES:
        raise Skip(f'pull request #{pull} lists {len(files)} files, the most GitHub returns, none under {LIBRARY}')
    raise Skip(f'pull request #{pull} changes no file under {LIBRARY}: only a library/ui change gets a preview')


def artifact_record(github, repository, artifact_id):
    """The artifact API's answer for an artifact. A re-run of the Shared UI library run replaces its artifact, and
    that run's own key scan publishes the new one."""
    try:
        return github.get(f'repos/{repository}/actions/artifacts/{artifact_id}')
    except ApiError as error:
        if error.status == 404:
            raise Skip(f'artifact {artifact_id} no longer exists; a newer run replaced it') from None
        raise


def check_artifact(github, repository, repository_id, scanned):
    name = f'ui-review-{scanned["head"]}'
    artifact = artifact_record(github, repository, scanned['artifact'])
    origin = artifact.get('workflow_run') or {}
    if (artifact.get('name') != name or origin.get('id') != scanned['run'] or origin.get('head_sha') != scanned['head']
            or origin.get('repository_id') != repository_id or origin.get('head_repository_id') != repository_id):
        raise Reject(f'artifact ID mismatch: artifact {scanned["artifact"]} is not {name} of run {scanned["run"]}')
    if artifact.get('expired'):
        raise Skip(f'the Review artifact {scanned["artifact"]} has expired')
    if artifact.get('digest') != scanned['digest']:
        raise Reject(f'digest mismatch: the key scan downloaded {scanned["digest"]}, the artifact API reports '
                     f'{artifact.get("digest")}')
    held = [item.get('id') for item in github.pages(f'repos/{repository}/actions/runs/{scanned["run"]}/artifacts',
                                                    'artifacts', name=name, per_page=100)
            if item.get('name') == name and not item.get('expired')]
    if held != [scanned['artifact']]:
        raise Reject(f'artifact ID mismatch: the key scan downloaded {scanned["artifact"]}, run {scanned["run"]} '
                     f'holds {held}')


def verify(env, github=None):
    github = github or GitHub()
    # The later jobs use these values from this job's outputs: the publish job names the ui-gallery environment, where
    # vars.* would read environment variables first, so it must not read them again.
    values = configuration(env)
    repository, default_branch = env['REPOSITORY'], env['DEFAULT_BRANCH']
    repository_id, scan_id, attempt = (number(env, name) for name in ('REPOSITORY_ID', 'SCAN_RUN_ID',
                                                                      'SCAN_RUN_ATTEMPT'))
    environment_ready(github, repository, default_branch)
    check_scan_run(github, github.get(f'repos/{repository}/actions/runs/{scan_id}'), repository, default_branch,
                   attempt)
    job = scan_job(github.pages(f'repos/{repository}/actions/runs/{scan_id}/attempts/{attempt}/jobs', 'jobs'))
    scanned = scanned_artifact(github.text(f'repos/{repository}/actions/jobs/{job["id"]}/logs'), repository)
    run = github.get(f'repos/{repository}/actions/runs/{scanned["run"]}')
    check_scanned_run(github, run, repository, scanned['head'])
    library_built(github.pages(f'repos/{repository}/actions/runs/{scanned["run"]}/jobs', 'jobs'))
    pull = open_pull_request(github, repository, run, scanned['head'])
    changes_library(github, repository, pull)
    check_artifact(github, repository, repository_id, scanned)
    print(f'Pull request #{pull} at {scanned["head"]}: the key scan cleared artifact {scanned["artifact"]} '
          f'({scanned["digest"]}) of run {scanned["run"]}')
    return {'publish': 'true', 'pr': pull, 'head': scanned['head'], 'run': scanned['run'],
            'artifact': scanned['artifact'], 'digest': scanned['digest'], 'role': values['UI_GALLERY_ROLE_ARN'],
            'bucket': values['UI_GALLERY_BUCKET'], 'region': values['UI_GALLERY_AWS_REGION'],
            'base': values['UI_GALLERY_BASE_URL']}


# --- fetch ----------------------------------------------------------------------------------------------------------

def sha256(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as source:
        for chunk in iter(lambda: source.read(1 << 20), b''):
            digest.update(chunk)
    return f'sha256:{digest.hexdigest()}'


LOCAL_HEADER = struct.Struct('<IHHHHHIIIHH')  # signature, version, flags, method, time, date, crc, sizes, lengths
LOCAL_SIGNATURE = 0x04034b50
DESCRIPTOR_SIGNATURE = b'PK\x07\x08'


def check_layout(data, bundle):
    """The entries lie back to back from the archive's first byte to its central directory, in the directory's order,
    as upload-artifact writes them: each local header names the directory's file and method, each deflate stream ends
    exactly at its recorded size, and each data descriptor matches. This reader extracts by the central directory; the
    key scan's download reads the local headers from the front. With this layout both meet the same files."""
    position = 0
    for info in bundle.infolist():
        name = info.filename.encode('utf-8' if info.flag_bits & 0x800 else 'cp437')
        if info.header_offset != position or position + LOCAL_HEADER.size > len(data):
            raise Reject(f'the artifact holds bytes outside its entries before {info.filename!r}')
        signature, _, flags, method, _, _, crc, compressed, size, name_length, extra_length = \
            LOCAL_HEADER.unpack_from(data, position)
        start = position + LOCAL_HEADER.size + name_length + extra_length
        if (signature != LOCAL_SIGNATURE or flags != info.flag_bits or method != info.compress_type
                or data[position + LOCAL_HEADER.size:position + LOCAL_HEADER.size + name_length] != name):
            raise Reject(f'the local header of {info.filename!r} differs from the central directory')
        if method != zipfile.ZIP_DEFLATED:
            raise Reject(f'{info.filename!r} is not deflated, as upload-artifact writes every file')
        end = start + info.compress_size
        inflate, pending, produced, chunk = zlib.decompressobj(-zlib.MAX_WBITS), data[start:end], 0, 1 << 20
        while True:
            output = inflate.decompress(pending, chunk)
            produced += len(output)
            pending = inflate.unconsumed_tail
            if produced > info.file_size or (not pending and len(output) < chunk):
                break
        if end > len(data) or produced != info.file_size or not inflate.eof or inflate.unused_data:
            raise Reject(f'the compressed data of {info.filename!r} does not end at its recorded size')
        position = end
        if flags & 0x08:
            position += 4 if data[position:position + 4] == DESCRIPTOR_SIGNATURE else 0
            recorded = (info.CRC, info.compress_size, info.file_size)
            if position + 12 <= len(data) and struct.unpack_from('<III', data, position) == recorded:
                position += 12
            elif position + 20 <= len(data) and struct.unpack_from('<IQQ', data, position) == recorded:
                position += 20
            else:
                raise Reject(f'the data descriptor of {info.filename!r} differs from the central directory')
        elif (crc, compressed, size) != (info.CRC, info.compress_size, info.file_size):
            raise Reject(f'the local header of {info.filename!r} differs from the central directory')
    if position != bundle.start_dir:
        raise Reject('the artifact holds bytes between its last entry and its central directory')


def unpack_gallery(archive, site):
    """Writes the regular files under ui-gallery/ into the empty directory site and returns their paths and their
    gallery digest, which must equal the digest of ui-gallery/ in the archive. The archive is pull request output: a
    layout check_layout refuses, a path that is absolute, climbs, repeats or is hidden, a link, an unknown file type
    or a breach of a limit fails the whole archive."""
    site.mkdir(parents=True)
    files, total, seen = [], 0, set()
    root = GALLERY.rstrip('/')
    try:
        data = Path(archive).read_bytes()
        with zipfile.ZipFile(archive) as bundle:
            check_layout(data, bundle)
            for info in bundle.infolist():
                name = info.filename
                parts = name.rstrip('/').split('/')
                if (not name or name.startswith('/') or '\\' in name or CONTROL.search(name)
                        or any(part in ('', '.', '..') for part in parts)):
                    raise Reject(f'unsafe path in the artifact: {name!r}')
                if name in seen:
                    raise Reject(f'the artifact repeats {name!r}')
                seen.add(name)
                if parts[0] != root or info.is_dir():
                    continue
                kind = stat.S_IFMT(info.external_attr >> 16)
                if kind not in (0, stat.S_IFREG):
                    raise Reject(f'{name} is not a regular file')
                relative = parts[1:]
                if not relative or len(relative) > LIMITS['depth']:
                    raise Reject(f'{name} is not a file within {LIMITS["depth"]} levels of {GALLERY}')
                extension = PurePosixPath(relative[-1]).suffix[1:]
                if extension not in TYPES:
                    raise Reject(f'{name} has no content type: {sorted(TYPES)} are served')
                total += info.file_size
                if info.file_size > LIMITS['file_bytes'] or total > LIMITS['total_bytes']:
                    raise Reject(f'{name} exceeds the size limits of the hosted gallery')
                if len(files) >= LIMITS['files']:
                    raise Reject(f'{GALLERY} holds too many files')
                target = site.joinpath(*relative)
                target.parent.mkdir(parents=True, exist_ok=True)
                with bundle.open(info) as source, open(target, 'xb') as sink:
                    shutil.copyfileobj(source, sink)
                files.append('/'.join(relative))
        if 'index.html' not in files:
            raise Reject(f'the artifact has no {GALLERY}index.html: its run built no hosted gallery')
        scanned, unpacked = receipt.artifact_digest(data), receipt.tree_digest(site)
    except receipt.ReceiptError as error:
        raise Reject(f'the artifact holds no publishable {GALLERY}: {error}') from None
    except (zipfile.BadZipFile, zlib.error, struct.error, OSError, RuntimeError, EOFError) as error:
        raise Reject(f'the artifact cannot be unpacked safely: {error}') from None
    if unpacked != scanned:
        raise Reject(f'digest mismatch: the unpacked gallery is {unpacked}, the artifact holds {scanned}')
    return files, scanned


def fetch(env, github=None):
    github = github or GitHub()
    repository, head = env['REPOSITORY'], head_sha(env)
    run, artifact_id = number(env, 'SCANNED_RUN_ID'), number(env, 'ARTIFACT_ID')
    expected = env.get('ARTIFACT_DIGEST') or ''
    if not DIGEST.fullmatch(expected):
        raise Reject(f'ARTIFACT_DIGEST is not a sha256 digest: {expected!r}')
    artifact = artifact_record(github, repository, artifact_id)
    if artifact.get('expired'):
        raise Skip(f'artifact {artifact_id} has expired')
    if artifact.get('name') != f'ui-review-{head}' or (artifact.get('workflow_run') or {}).get('id') != run:
        raise Reject(f'artifact ID mismatch: artifact {artifact_id} is not ui-review-{head} of run {run}')
    if artifact.get('digest') != expected:
        raise Reject(f'digest mismatch: the artifact API reports {artifact.get("digest")}, verified {expected}')
    work = Path(env['WORK_DIR'])
    try:
        work.mkdir(parents=True)
    except FileExistsError:
        raise Reject(f'{work} already exists: the artifact is unpacked into a new directory only') from None
    archive = work / 'artifact.zip'
    github.download(f'repos/{repository}/actions/artifacts/{artifact_id}/zip', archive, ARCHIVE_BYTES)
    actual = sha256(archive)
    if actual != expected:
        raise Reject(f'digest mismatch: the downloaded artifact is {actual}, the key scan cleared {expected}')
    files, gallery = unpack_gallery(archive, work / 'site')
    print(f'{GALLERY} of artifact {artifact_id} ({expected}): {len(files)} files, gallery digest {gallery}')
    return {'gallery': gallery}


# --- upload and probe -----------------------------------------------------------------------------------------------

def site_files(site):
    return sorted(path.relative_to(site).as_posix() for path in site.rglob('*') if path.is_file())


def check_site(site):
    """The files of a hosted gallery directory and its gallery digest. Every file has a served type, and the directory
    keeps the artifact limits, an index.html, no hidden path and no link."""
    files = site_files(site)
    unknown = sorted({path for path in files if PurePosixPath(path).suffix[1:] not in TYPES})
    if 'index.html' not in files or unknown:
        raise Reject(f'{site} is not a hosted gallery: ' + (f'{unknown} have no content type; {sorted(TYPES)} are '
                                                             'served' if unknown else 'it has no index.html'))
    sizes = [(site / path).stat().st_size for path in files]
    if (len(files) > LIMITS['files'] or max(sizes) > LIMITS['file_bytes'] or sum(sizes) > LIMITS['total_bytes']
            or any(path.count('/') >= LIMITS['depth'] for path in files)):
        raise Reject(f'{site} exceeds the limits of a hosted gallery: {LIMITS}')
    try:
        return files, receipt.tree_digest(site)
    except receipt.ReceiptError as error:
        raise Reject(f'{site} is not a hosted gallery: {error}') from None


def check(env):
    """Run by ui-library.yml on the pull request's own build, so a file the publisher would refuse fails there and not
    on main."""
    files, digest = check_site(Path(env.get('SITE_DIR') or '.'))
    print(f'{len(files)} files a preview can serve, gallery digest {digest}')
    return {}


def csp(env):
    print('; '.join(PREVIEW_CSP))
    return {}


def utc_today():
    return datetime.datetime.now(datetime.timezone.utc).date()


def upload(env, run=subprocess.run, today=utc_today):
    values = configuration(env, ('UI_GALLERY_BUCKET', 'UI_GALLERY_AWS_REGION'), required=True)
    prefix = preview_prefix(number(env, 'PR_NUMBER'), head_sha(env))
    expected = env.get('GALLERY_DIGEST') or ''
    if not GALLERY_DIGEST.fullmatch(expected):
        raise Reject(f'GALLERY_DIGEST is not a sha256 digest: {expected!r}')
    site = Path(env['WORK_DIR']) / 'site'
    files, actual = check_site(site)
    extensions = sorted({PurePosixPath(path).suffix[1:] for path in files})
    if actual != expected:
        raise Reject(f'digest mismatch: {site} is {actual}, the fetch unpacked {expected}')
    destination = f's3://{values["UI_GALLERY_BUCKET"]}/{prefix}/'
    # The lifecycle rule counts from each object's creation, and every object is created on or after this date, even
    # when the upload crosses midnight UTC.
    expires = today() + datetime.timedelta(RETENTION_DAYS)
    # One copy per type and cache header, so each object gets both. cp never lists or deletes in the bucket and sets no
    # ACL: the writer role may only put objects, and the bucket blocks public access.
    hashed = {path for path in files if HASHED.fullmatch(path)}
    for extension in extensions:
        typed = [path for path in files if PurePosixPath(path).suffix[1:] == extension]
        named = sorted(path for path in typed if path in hashed)
        groups = []
        if named:
            groups.append((ASSET_CACHE, [argument for path in named for argument in ('--include', path)]))
        if len(named) < len(typed):
            groups.append((PAGE_CACHE, ['--include', f'*.{extension}',
                                        *[argument for path in named for argument in ('--exclude', path)]]))
        for cache, filters in groups:
            command = ['aws', 's3', 'cp', str(site), destination, '--recursive', '--exclude', '*', *filters,
                       '--content-type', TYPES[extension], '--cache-control', cache,
                       '--region', values['UI_GALLERY_AWS_REGION'], '--only-show-errors']
            if run(command).returncode:
                raise Reject(f'the upload of the .{extension} files to {destination} failed')
    print(f'Uploaded {len(files)} files to {destination}; the bucket keeps them until at least {expires.isoformat()}')
    return {'expires': expires.isoformat()}


# The headers dcl-one-infra's distribution adds to every response; a missing one means the distribution drifted.
EXPECTED_HEADERS = {'X-Robots-Tag': 'noindex', 'Content-Security-Policy': '; '.join(PREVIEW_CSP),
                    'X-Content-Type-Options': 'nosniff'}


def directives(policy):
    return sorted(' '.join(directive.split()) for directive in policy.split(';') if directive.strip())


def carries(name, expected, value):
    """Whether a response header holds what the distribution must add: the policy's exact directives, in any order,
    since the pull request's build was checked under those and no others."""
    if name == 'Content-Security-Policy':
        return directives(value) == directives(expected)
    return expected.lower() in value.lower()


def answer(url):
    """The status and the expected headers of a GET."""
    request = urllib.request.Request(url, headers={'User-Agent': USER_AGENT})
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            response.read()
            status, headers = response.status, response.headers
    except urllib.error.HTTPError as error:
        status, headers = error.code, error.headers
    except (urllib.error.URLError, OSError):
        return 0, {}
    return status, {name: (headers.get(name, '') if headers else '') for name in EXPECTED_HEADERS}


def probe(env, request=answer, sleep=time.sleep):
    base = configuration(env, ('UI_GALLERY_BASE_URL',), required=True)['UI_GALLERY_BASE_URL']
    pull, head = number(env, 'PR_NUMBER'), head_sha(env)
    files = site_files(Path(env['WORK_DIR']) / 'site')
    # A built asset as well as the page: both come through the gallery host from the bucket.
    assets = [path for path in files if path.endswith('.js')] or [path for path in files if path != 'index.html']
    page = preview_url(base, pull, head)
    targets = [page] + [f'{base}/{preview_prefix(pull, head)}/{urllib.parse.quote(path)}' for path in assets[:1]]
    for attempt in range(len(PROBE_DELAYS) + 1):
        answers = [request(url) for url in targets]
        if all(status == 200 for status, _ in answers):
            headers = answers[0][1]
            missing = [f'{name}: {expected}' for name, expected in EXPECTED_HEADERS.items()
                       if not carries(name, expected, headers.get(name, ''))]
            if missing:
                # The preview runs pull request scripts: no link to it without the distribution's protections.
                raise Reject(f'{page} is served without {", ".join(missing)}: the distribution\'s headers policy '
                             'drifted from dcl-one-infra')
            print(f'The preview answers at {page}')
            return {}
        if attempt < len(PROBE_DELAYS):
            sleep(PROBE_DELAYS[attempt])
    statuses = ', '.join(f'{url} answered {status}' for url, (status, _) in zip(targets, answers))
    raise Reject(f'the preview did not answer 200 within {sum(PROBE_DELAYS)} seconds: {statuses}')


# --- comment --------------------------------------------------------------------------------------------------------

# No response header keeps a page from a wallet extension, so the comment itself warns the reviewer.
WALLET_WARNING = ("This preview runs this pull request's code outside Decentraland's sites: never connect a wallet "
                  'or sign anything there.')


def render(url, head, expires):
    return (f'### [Open the UI gallery]({url})\n\n'
            f'| Built from | Available until |\n| --- | --- |\n| `{head[:7]}` | {expires} (UTC) |\n\n'
            f'> [!WARNING]\n> {WALLET_WARNING}\n\n{MARKER}\n')


def comment(env, github=None):
    github = github or GitHub()
    repository, pull, head = env['REPOSITORY'], number(env, 'PR_NUMBER'), head_sha(env)
    base = configuration(env, ('UI_GALLERY_BASE_URL',), required=True)['UI_GALLERY_BASE_URL']
    expires = env.get('EXPIRES') or ''
    if not re.fullmatch(r'\d{4}-\d{2}-\d{2}', expires):
        raise Reject(f'EXPIRES is not a date: {expires!r}')
    current = github.get(f'repos/{repository}/pulls/{pull}')
    if current.get('state') != 'open':
        raise Skip(f'pull request #{pull} is closed')
    if current['head']['sha'] != head:
        raise Skip(f'pull request #{pull} moved on to {current["head"]["sha"]}; its own run comments')
    body = render(preview_url(base, pull, head), head, expires)
    ours = [item for item in github.pages(f'repos/{repository}/issues/{pull}/comments', per_page=100)
            if (item.get('user') or {}).get('login') == BOT and MARKER in (item.get('body') or '')]
    if not ours:
        github.send('POST', f'repos/{repository}/issues/{pull}/comments', body=body)
        print(f'Linked the preview from pull request #{pull}')
    elif ours[0].get('body') != body:
        github.send('PATCH', f'repos/{repository}/issues/comments/{ours[0]["id"]}', body=body)
        print(f'Updated the preview link on pull request #{pull}')
    return {}


# --- entry point ----------------------------------------------------------------------------------------------------

def annotate(level, message):
    escaped = message.replace('%', '%25').replace('\r', '%0D').replace('\n', '%0A')
    print(f'::{level} title=UI gallery::{escaped}', flush=True)


def write_outputs(env, outputs):
    if not outputs:
        return
    lines = []
    for key, value in outputs.items():
        value = str(value)
        if '\n' in value or '\r' in value:
            raise Reject(f'output {key} spans lines')
        lines.append(f'{key}={value}\n')
    with open(env['GITHUB_OUTPUT'], 'a', encoding='utf-8') as sink:
        sink.writelines(lines)


COMMANDS = {'verify': verify, 'fetch': fetch, 'upload': upload, 'probe': probe, 'comment': comment, 'check': check,
            'csp': csp}


def main(argv=None, env=None):
    argv = sys.argv[1:] if argv is None else argv
    env = os.environ if env is None else env
    if len(argv) != 1 or argv[0] not in COMMANDS:
        print(f'usage: ui-gallery-publish.py {{{",".join(COMMANDS)}}}', file=sys.stderr)
        return 2
    try:
        outputs = COMMANDS[argv[0]](env)
    except Skip as reason:
        annotate('notice', f'No UI gallery preview: {reason}')
        write_outputs(env, {'publish': 'false'} if argv[0] == 'verify' else {})
        return 0
    except (Reject, ApiError) as problem:
        annotate('error', f'UI gallery preview refused: {problem}')
        return 1
    except (KeyError, TypeError, AttributeError, ValueError) as problem:
        # An answer or input of an unexpected shape: still the error annotation, with what was missing.
        annotate('error', f'UI gallery preview failed on an unexpected value: {type(problem).__name__}: {problem}')
        return 1
    write_outputs(env, outputs)
    return 0


if __name__ == '__main__':
    sys.exit(main())
