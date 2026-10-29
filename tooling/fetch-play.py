#!/usr/bin/env python3
"""Import our verified Bevy web build for Sites; never fall back to an npm engine."""
import argparse
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import zipfile

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('play_artifact', ROOT / 'tooling/play-artifact.py')
artifact = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(artifact)
REPO = 'decentraland/dcl-one'
WORKFLOW = '.github/workflows/bevy-explorer-ci.yml'
PRODUCER = 'Web engine, HUD and bridge scene'
UPLOAD_STEP = 'Upload the complete web app'
MAX_BYTES = 2 * 1024**3


def api(path, deadline=None):
    # Retry reads within the caller's existing wait budget. Non-waiting imports
    # and final publication checks get only four retries; no writes are repeated.
    attempt = 0
    while True:
        timeout = 60 if deadline is None else min(60, deadline - time.monotonic())
        if timeout <= 0:
            raise TimeoutError('GitHub read exceeded the configured wait deadline')
        try:
            response = subprocess.check_output(
                ['gh', 'api', '--method', 'GET', f'repos/{REPO}/{path}'],
                text=True, stderr=subprocess.PIPE, timeout=timeout)
        except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as exc:
            transient = isinstance(exc, subprocess.TimeoutExpired) or re.search(
                r'\(HTTP (?:500|502|503|504)\)', exc.stderr or '')
            exhausted = attempt >= 4 if deadline is None else time.monotonic() >= deadline
            if not transient or exhausted:
                if isinstance(exc, subprocess.CalledProcessError) and exc.stderr:
                    print(exc.stderr, file=sys.stderr, end='')
                raise
            delay = min(30, 2 ** min(attempt + 1, 5))
            if deadline is not None:
                delay = min(delay, max(0, deadline - time.monotonic()))
            print(f'Transient GitHub read failure; retry {attempt + 1} in {delay:g}s.', flush=True)
            time.sleep(delay)
            if deadline is not None and time.monotonic() >= deadline:
                raise
            attempt += 1
        else:
            return json.loads(response)


def workflow_runs(deadline=None):
    # Paginate without search filters: branch/head_sha searches cap results at
    # 1,000. Trust and PR identity are checked locally, on every returned run.
    page = 1
    while True:
        rows = api(f'actions/workflows/bevy-explorer-ci.yml/runs?per_page=100&page={page}', deadline)['workflow_runs']
        yield from rows
        if len(rows) < 100:
            return
        page += 1


def poll_runs(pending, deadline=None):
    # Discover recent producers without rescanning history. A pending run may
    # have fallen off page one, so refresh those IDs explicitly as well.
    recent = api('actions/workflows/bevy-explorer-ci.yml/runs?per_page=100&page=1', deadline)['workflow_runs']
    yield from recent
    listed = {run['id'] for run in recent}
    for run_id in sorted(pending - listed):
        yield api(f'actions/runs/{run_id}', deadline)


def current_pr():
    if os.environ.get('GITHUB_EVENT_NAME') != 'pull_request':
        return None
    event = json.loads(Path(os.environ['GITHUB_EVENT_PATH']).read_text())
    return {'number': event['number'], 'head': event['pull_request']['head']['sha'],
            'base': event['pull_request']['base']['sha'], 'merge': os.environ['GITHUB_SHA']}


def inputs_at(revision):
    """The engine-input fingerprint of a commit, fetched shallowly if the checkout lacks it; None when unreadable."""
    try:
        if subprocess.run(['git', 'cat-file', '-e', f'{revision}^{{commit}}'],
                          cwd=ROOT, capture_output=True).returncode:
            subprocess.run(['git', 'fetch', '--no-tags', '--depth=1', 'origin', revision],
                           cwd=ROOT, capture_output=True, check=True, timeout=60)
        return artifact.inputs(ROOT, revision)
    except (OSError, ValueError, subprocess.SubprocessError):
        return None


def engine_changed(pr):
    """Whether the PR changes an engine input (engine source, protos, HUD, build scripts)
    relative to the commit it is based on. A PR that does not has main's engine, so it is
    tested with the newest main build that uploaded its web app and has not failed instead
    of waiting for main to rebuild the same inputs; a PR that does gets its own Bevy run and
    needs that exact build."""
    if not pr or not pr.get('base'):
        return True
    base = inputs_at(pr['base'])
    return base is None or base != artifact.inputs(ROOT)


def trusted(run, pr=None):
    if (run.get('path') != WORKFLOW or run.get('head_repository', {}).get('full_name') != REPO
            or not re.fullmatch('[0-9a-f]{40}', run.get('head_sha', ''))):
        return False
    if run['event'] in ('push', 'workflow_dispatch') and run['head_branch'] == 'main':
        return True
    return bool(pr and run['event'] == 'pull_request' and run['head_sha'] == pr['head']
                and any(row['number'] == pr['number'] for row in run.get('pull_requests', [])))


def extract(archive, output):
    with zipfile.ZipFile(archive) as source:
        members = source.infolist()
        if len(members) > 20000 or sum(m.file_size for m in members) > MAX_BYTES:
            raise ValueError('Explorer archive exceeds extraction limits')
        seen = set()
        for member in members:
            path = PurePosixPath(member.filename)
            mode = member.external_attr >> 16
            if (path.is_absolute() or '..' in path.parts or '.git' in path.parts or '\\' in member.filename
                    or not path.parts or path.as_posix() in seen
                    or stat.S_ISLNK(mode) or stat.S_IFMT(mode) not in (0, stat.S_IFREG, stat.S_IFDIR)):
                raise ValueError(f'Unsafe Explorer archive entry: {member.filename}')
            seen.add(path.as_posix())
        source.extractall(output)


def install(source, production=False, producer=None):
    # producer: the commit whose inputs the build must match instead of the tree's (a
    # PR that changes no engine input importing main's newest build).
    meta = artifact.verify(source, ROOT, not production, producer)
    cache = ROOT / '.cache/play'
    cache.mkdir(parents=True, exist_ok=True)
    # Different optimization modes and producer commits never overwrite each other.
    key = artifact.digest(json.dumps(meta, sort_keys=True).encode())
    dest = cache / key
    with (cache / '.lock').open('w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if not dest.exists():
            with tempfile.TemporaryDirectory(prefix='.import-', dir=cache) as temporary:
                stage = Path(temporary) / 'build'
                stage.mkdir()
                for part in (*artifact.PARTS, 'bindings', artifact.MANIFEST):
                    src = source / part
                    if src.is_dir():
                        shutil.copytree(src, stage / part)
                    else:
                        shutil.copy2(src, stage / part)
                artifact.verify(stage, ROOT, not production, producer)
                stage.rename(dest)
        if artifact.verify(dest, ROOT, not production, producer) != meta:
            raise ValueError('Cached Explorer manifest differs from the validated import')
        if producer:
            artifact.record_producer_validation(dest, producer, ROOT)
        link = cache / '.current-next'
        link.unlink(missing_ok=True)
        link.symlink_to(dest.name, target_is_directory=True)
        link.replace(cache / 'current')
    print(f"Sites uses our Explorer build {meta['build_revision']} at {dest}", flush=True)
    return dest


def producer_ready(job):
    # Upload runs only after every earlier step passed; later steps rebuild deploy/web for npm, not the artifact.
    if job['status'] == 'completed':
        return job['conclusion'] == 'success'
    uploads = [step for step in job.get('steps') or [] if step.get('name') == UPLOAD_STEP]
    return (job['status'] == 'in_progress' and len(uploads) == 1
            and uploads[0].get('status') == 'completed' and uploads[0].get('conclusion') == 'success')


def producer_record(run, pr=None, deadline=None):
    jobs = api(f'actions/runs/{run["id"]}/jobs?per_page=100', deadline)
    selected = [job for job in jobs['jobs'] if job['name'] == PRODUCER]
    if jobs['total_count'] != len(jobs['jobs']) or len(selected) != 1 or not producer_ready(selected[0]):
        return None
    revision = pr['merge'] if run['event'] == 'pull_request' else run['head_sha']
    data = api(f'actions/runs/{run["id"]}/artifacts?per_page=100', deadline)
    name = f'bevy-explorer-web-{revision}'
    found = [row for row in data['artifacts'] if row['name'] == name and not row['expired']]
    if data['total_count'] != len(data['artifacts']) or len(found) != 1:
        return None
    record = found[0]
    checksum = record.get('digest') or ''
    if not checksum and run['status'] != 'completed':
        return None
    if not re.fullmatch(r'sha256:[0-9a-f]{64}', checksum):
        raise ValueError('GitHub did not provide a verifiable artifact digest')
    if record['size_in_bytes'] > MAX_BYTES:
        raise ValueError('Explorer download exceeds size limit')
    return record


def download_run(run, pr=None, production=False, receipt=None, deadline=None, *, producer=None):
    # Preparation can overlap native tests. Only --production or a verified
    # receipt at publication authorizes delivery after the COMPLETE run passes.
    if production and (not trusted(run) or (run['status'] == 'completed' and run['conclusion'] != 'success') or (receipt is None and
            (run['status'] != 'completed' or run['conclusion'] != 'success'))):
        return None
    record = producer_record(run, pr, deadline)
    if record is None:
        return None
    revision = pr['merge'] if run['event'] == 'pull_request' else run['head_sha']
    checksum = record['digest']
    with tempfile.TemporaryDirectory(prefix='play-ci-') as temporary:
        tmp = Path(temporary)
        archive = tmp / 'web.zip'
        with archive.open('wb') as output:
            subprocess.run(['gh', 'api', f'repos/{REPO}/actions/artifacts/{record["id"]}/zip'],
                           stdout=output, check=True, timeout=300)
        with archive.open('rb') as stream:
            if hashlib.file_digest(stream, 'sha256').hexdigest() != checksum.split(':')[1]:
                raise ValueError('GitHub Explorer archive digest mismatch')
        extract(archive, tmp / 'web')
        meta = artifact.verify(tmp / 'web', ROOT, not production, producer)
        if meta['build_revision'] != revision:
            raise ValueError('Explorer metadata does not identify the validated producer revision')
        destination = install(tmp / 'web', production, producer)
        if receipt is not None:
            if not production:
                raise ValueError('Only optimized trusted-main imports can issue a production receipt')
            receipt.parent.mkdir(parents=True, exist_ok=True)
            receipt.write_text(json.dumps({'version': 1, 'run_id': run['id'], 'run_attempt': run['run_attempt'],
                'revision': revision, 'artifact_id': record['id'], 'digest': checksum,
                'inputs': artifact.inputs(ROOT)}, indent=2) + '\n')
        return destination


def verify_production(receipt, wait=0):
    """Recheck the exact producer used by a trusted image build, without downloading it again."""
    proof = json.loads(receipt.read_text())
    if (proof.get('version') != 1 or proof.get('inputs') != artifact.inputs(ROOT)
            or not re.fullmatch('[0-9a-f]{40}', proof.get('revision', ''))
            or not re.fullmatch('sha256:[0-9a-f]{64}', proof.get('digest', ''))
            or any(type(proof.get(k)) is not int or proof[k] <= 0 for k in ('run_id', 'run_attempt', 'artifact_id'))):
        raise ValueError('Invalid production receipt or changed Explorer inputs')
    deadline = time.monotonic() + wait
    read_deadline = deadline if wait > 0 else None
    while True:
        run = api(f'actions/runs/{proof["run_id"]}', read_deadline)
        if (not trusted(run) or run['id'] != proof['run_id'] or run['head_sha'] != proof['revision']
                or run['run_attempt'] != proof['run_attempt']):
            raise ValueError('Production producer identity or attempt changed; rebuild the images')
        if run['status'] == 'completed':
            if run['conclusion'] != 'success':
                raise ValueError('Complete Bevy CI did not pass; refusing publication')
            record = producer_record(run, deadline=read_deadline)
            if not record or (record['id'], record['digest']) != (proof['artifact_id'], proof['digest']):
                raise ValueError('Production artifact is missing, expired or changed; rebuild the images')
            print(f'Production producer {run["id"]}, attempt {run["run_attempt"]}, artifact {record["id"]}: complete CI passed.')
            return
        if time.monotonic() >= deadline:
            raise RuntimeError('Complete Bevy CI is still pending; refusing publication')
        print(f'Waiting for complete Bevy CI run {run["id"]} before publication.', flush=True)
        time.sleep(min(30, max(0, deadline - time.monotonic())))


def fetch(wait=0, production=False, receipt=None):
    expected = artifact.inputs(ROOT)
    pr = None if production else current_pr()
    # Production and engine-changing PRs need the build of exactly these inputs.
    match_inputs = production or engine_changed(pr)
    deadline = time.monotonic() + wait
    read_deadline = deadline if wait > 0 else None
    checked = set()
    keys = {}
    candidates = workflow_runs(read_deadline)
    while True:
        pending = set()
        for run in candidates:
            # GitHub reuses the run ID on rerun; only this attempt was checked.
            attempt = (run['id'], run['run_attempt'])
            if not trusted(run, pr) or attempt in checked:
                continue
            if run['status'] == 'completed' and run['conclusion'] != 'success':
                checked.add(attempt)
                continue
            revision = pr['merge'] if run['event'] == 'pull_request' else run['head_sha']
            if revision not in keys:
                keys[revision] = inputs_at(revision)
            if keys[revision] is None or (match_inputs and keys[revision] != expected):
                checked.add(attempt)
                continue
            # The tree's own inputs are verified as such; another main build (a change
            # that touches no engine input) against the commit that built it.
            producer = None if keys[revision] == expected else revision
            try:
                if run['status'] != 'completed':
                    if not production or receipt is not None:
                        result = download_run(run, pr, production, receipt, read_deadline, producer=producer)
                        if result:
                            print(f'Preparation import: web artifact uploaded in Bevy run {run["id"]}; '
                                  'remaining CI jobs are still running.', flush=True)
                            return result
                    pending.add(run['id'])
                    continue
                checked.add(attempt)
                if run['conclusion'] == 'success':
                    result = download_run(run, pr, production, receipt, read_deadline, producer=producer)
                    if result:
                        return result
            except ValueError as exc:
                if producer is None:
                    raise  # the build of these very inputs is corrupt or tampered with
                # Usually main moved the HUD contract after this build and an older build
                # still fits; a digest or archive error on a trusted artifact must be seen.
                print(f'::warning::Skipping Bevy run {run["id"]} ({revision[:8]}): {exc}', flush=True)
                checked.add(attempt)
        if not pending or time.monotonic() >= deadline:
            recovery = ('Refresh main with: gh workflow run bevy-explorer-ci.yml --ref main. '
                        'Wait for its successful web build, then retry with matching inputs. ')
            if not production:
                recovery += ('For local development, build with make bevy-web and use '
                             '--from-build apps/bevy-explorer/dist/web. ')
            raise RuntimeError('No successful matching dcl-one Bevy CI artifact. ' + recovery +
                               'Sites will not load a different engine.')
        gate = 'complete Bevy CI run' if production and receipt is None else 'Bevy web producer'
        wanted = f'with inputs {expected[:12]}' if match_inputs else 'on main (this PR changes no engine input)'
        print(f'Waiting for a successful {gate} {wanted} '
              f'({"matching run in progress" if pending else "no matching uploaded producer"})', flush=True)
        time.sleep(min(30, max(0, deadline - time.monotonic())))
        candidates = poll_runs(pending, read_deadline)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--from-build', type=Path, help='Import a verified local build instead of a CI artifact')
    parser.add_argument('--wait', type=int, default=int(os.environ.get('FOUNDATION_PLAY_WAIT_SECONDS', '0')))
    mode.add_argument('--production', action='store_true', help='Revalidate and import optimized trusted-main output')
    mode.add_argument('--prepare-production', type=Path, metavar='RECEIPT',
                      help='Prepare optimized trusted-main output before full CI; write a publication receipt')
    mode.add_argument('--verify-production', type=Path, metavar='RECEIPT',
                      help='Require complete CI for the exact artifact in a trusted build receipt')
    mode.add_argument('--print-directory', action='store_true')
    args = parser.parse_args(argv)
    if args.verify_production:
        verify_production(args.verify_production, args.wait)
        return 0
    if args.prepare_production:
        # Never leave a receipt from an earlier successful import on failure.
        args.prepare_production.unlink(missing_ok=True)
        fetch(args.wait, production=True, receipt=args.prepare_production)
        return 0
    current = ROOT / '.cache/play/current'
    if args.print_directory:
        print(current.resolve(strict=True))
        return 0
    if args.from_build:
        install(args.from_build)
        return 0
    # A local manifest proves byte integrity, not a successful trusted-main run.
    # Production always rechecks GitHub and its archive digest, even on cache hits.
    if not args.production:
        try:
            trust = (current / artifact.PRODUCER_VALIDATION).is_file()
            artifact.verify(current, ROOT, trust_inputs=trust)
            print(f'Sites Explorer already verified at {current}')
            return 0
        except (OSError, ValueError, KeyError):
            pass
    fetch(args.wait, args.production)
    return 0


if __name__ == '__main__':
    main()
