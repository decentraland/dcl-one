#!/usr/bin/env python3
"""Select web preview builds and publish only validated static build outputs."""

import argparse
import http.client
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
from urllib.error import HTTPError, URLError
from urllib.request import urlopen

WORKFLOW = '.github/workflows/web-previews.yml'
PIPELINE_INPUTS = {
    WORKFLOW,
    '.github/workflows/web-previews-publish.yml',
    'tooling/web-preview.py',
    'tooling/tests/test_web_preview.py',
}
PACKAGE = '@dcl-regenesislabs/dcl-one-ui-review'
MARKER = '<!-- web-pr-previews -->'
STATIC_EXTENSIONS = {
    '.html',
    '.js',
    '.css',
    '.json',
    '.png',
    '.jpg',
    '.jpeg',
    '.gif',
    '.webp',
    '.svg',
    '.ico',
    '.woff',
    '.woff2',
    '.ttf',
    '.otf',
    '.glb',
    '.gltf',
    '.avif',
    '.bin',
    '.wasm',
    '.txt',
}


def products(paths):
    paths = list(paths)
    pipeline = bool(PIPELINE_INPUTS.intersection(paths))
    return [
        name
        for name, prefix in [('sites', 'apps/sites/'), ('reviewer', 'library/ui/')]
        if pipeline or any(path.startswith(prefix) for path in paths)
    ]


def publishable(run, pr, repository):
    return (
        run.get('event') == 'pull_request'
        and run.get('status') == 'completed'
        and run.get('conclusion') in ('success', 'failure')
        and run.get('path') == WORKFLOW
        and all(
            (run.get(field) or {}).get('login') != 'dependabot[bot]'
            for field in ('actor', 'triggering_actor')
        )
        and (run.get('head_repository') or {}).get('full_name') == repository
        and pr.get('state') == 'open'
        and (pr.get('head', {}).get('repo') or {}).get('full_name') == repository
        and pr.get('head', {}).get('sha') == run.get('head_sha')
    )


def reviewer_ready(jobs, artifacts):
    selected = [job for job in jobs if job['name'] == 'UI reviewer']
    outputs = [
        artifact
        for artifact in artifacts
        if artifact['name'] == 'web-preview-reviewer' and not artifact['expired']
    ]
    return (
        len(selected) == 1
        and len(outputs) == 1
        and selected[0]['status'] == 'completed'
        and selected[0]['conclusion'] == 'success'
    )


def require_reviewer(repository, run_id):
    jobs = api(f'repos/{repository}/actions/runs/{run_id}/jobs?per_page=100')['jobs']
    artifacts = api(f'repos/{repository}/actions/runs/{run_id}/artifacts?per_page=100')['artifacts']
    return reviewer_ready(jobs, artifacts)


def package_reviewer(source, target, revision, run_id):
    if not re.fullmatch(r'[0-9a-f]{40}', revision) or run_id < 1:
        raise ValueError('Invalid preview source identity')
    if not (source / 'dcl-ui-review/index.html').is_file():
        raise ValueError('Missing built reviewer entry: dcl-ui-review/index.html')
    files = list(source.rglob('*'))
    total = 0
    for path in files:
        relative = path.relative_to(source)
        if path.is_symlink() or any(part.startswith('.') for part in relative.parts):
            raise ValueError(f'Unsafe static preview path: {relative}')
        if path.is_dir():
            continue
        if (
            not path.is_file()
            or path.suffix.lower() not in STATIC_EXTENSIONS
            or path.name == 'package.json'
        ):
            raise ValueError(f'Unexpected static preview file: {relative}')
        total += path.stat().st_size
    if len(files) > 20000 or total > 512 * 1024**2:
        raise ValueError('Static preview exceeds the publication size limit')
    shutil.copytree(source, target)
    (target / 'package.json').write_text(
        json.dumps(
            {
                'name': PACKAGE,
                'version': '0.1.0',
                'license': 'Apache-2.0',
                'description': 'Static UI reviewer preview for dcl-one',
            }
        )
        + '\n'
    )
    (target / 'preview-source.json').write_text(
        json.dumps({'revision': revision, 'run': run_id}) + '\n'
    )


def api(path, *, method='GET', body=None, paginate=False):
    args = ['gh', 'api', '--method', method, path]
    if paginate:
        args += ['--paginate', '--slurp']
    if body is not None:
        args += ['--input', '-']
    result = subprocess.run(
        args,
        input=json.dumps(body) if body is not None else None,
        capture_output=True,
        text=True,
        check=False,
        timeout=120,
    )
    if result.returncode:
        print(result.stderr, file=sys.stderr)
        result.check_returncode()
    value = json.loads(result.stdout)
    return [item for page in value for item in page] if paginate else value


def resolve(repository, run_id):
    run = api(f'repos/{repository}/actions/runs/{run_id}')
    if (
        run.get('event') != 'pull_request'
        or run.get('path') != WORKFLOW
        or (run.get('head_repository') or {}).get('full_name') != repository
    ):
        return run, None
    candidates = run.get('pull_requests') or api(
        f'repos/{repository}/commits/{run["head_sha"]}/pulls?per_page=100', paginate=True
    )
    for candidate in candidates:
        pr = api(f'repos/{repository}/pulls/{int(candidate["number"])}')
        if publishable(run, pr, repository):
            return run, pr
    return run, None


def cdn_url(version):
    if not re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+-[0-9]+\.commit-[0-9a-f]+', version):
        raise ValueError('Invalid published preview version')
    return f'https://cdn.decentraland.org/{PACKAGE}/{version}'


def verify_cdn(version, revision, run_id, timeout=300):
    url = cdn_url(version)
    deadline = time.monotonic() + timeout
    while True:
        try:
            with urlopen(f'{url}/preview-source.json', timeout=15) as response:
                identity = json.load(response)
            if identity != {'revision': revision, 'run': run_id}:
                raise ValueError('CDN preview belongs to a different source build')
            with urlopen(f'{url}/dcl-ui-review/index.html', timeout=15) as response:
                if response.status == 200 and 'text/html' in response.headers.get(
                    'Content-Type', ''
                ):
                    return f'{url}/dcl-ui-review/index.html'
        except (OSError, HTTPError, URLError, http.client.HTTPException, json.JSONDecodeError):
            pass
        if time.monotonic() >= deadline:
            raise TimeoutError(
                'Reviewer is not available on the CDN; no preview link will be posted'
            )
        time.sleep(5)


def report(repository, run_id, reviewer_url, publication_result):
    run, pr = resolve(repository, run_id)
    if pr is None:
        print('No current same-repository pull request to report into')
        return
    jobs = api(f'repos/{repository}/actions/runs/{run_id}/jobs?per_page=100')['jobs']
    rows = []
    for name, label in [('UI reviewer', 'UI reviewer'), ('Sites', 'Sites')]:
        job = next((job for job in jobs if job['name'] == name), None)
        if not job or job['conclusion'] == 'skipped':
            continue
        result = job['conclusion'] or job['status']
        if result != 'success':
            link = f'[Build log]({job["html_url"]})'
        elif name == 'UI reviewer':
            link = (
                f'[Preview]({reviewer_url})'
                if reviewer_url
                else f'Publication: {publication_result}'
            )
        else:
            link = 'SSR preview pending: shared host deployment is not wired yet'
        rows.append(f'| {label} | {result} | {link} |')
    if not rows:
        rows.append('| Web previews | failed | See the build run |')
    receipt = f'<!-- source-run:{run_id}:{run["run_attempt"]} -->'
    body = '\n'.join(
        [
            MARKER,
            receipt,
            '### Web previews',
            '',
            f'Commit `{run["head_sha"][:7]}`. [Build run]({run["html_url"]}).',
            '',
            '| Product | Build | Preview |',
            '| --- | --- | --- |',
            *rows,
        ]
    )
    comments = api(f'repos/{repository}/issues/{pr["number"]}/comments?per_page=100', paginate=True)
    current = next(
        (
            comment
            for comment in comments
            if comment['user']['login'] == 'github-actions[bot]'
            and comment['body'].startswith(MARKER)
        ),
        None,
    )
    if current:
        previous = re.search(r'<!-- source-run:(\d+):(\d+) -->', current['body'])
        if previous and tuple(map(int, previous.groups())) > (run_id, run['run_attempt']):
            return
    # Recheck after the API reads so an outdated source cannot replace the current preview.
    _, live = resolve(repository, run_id)
    if live is None:
        return
    path = (
        f'repos/{repository}/issues/comments/{current["id"]}'
        if current
        else f'repos/{repository}/issues/{pr["number"]}/comments'
    )
    api(path, method='PATCH' if current else 'POST', body={'body': body})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    select = sub.add_parser('select')
    select.add_argument('--base', required=True)
    select.add_argument('--head', required=True)
    sub.add_parser('resolve')
    sub.add_parser('check-publication')
    package = sub.add_parser('package-reviewer')
    package.add_argument('source', type=Path)
    package.add_argument('target', type=Path)
    verify = sub.add_parser('verify-cdn')
    verify.add_argument('version')
    sub.add_parser('report')
    args = parser.parse_args()
    if args.command == 'select':
        paths = subprocess.check_output(
            ['git', 'diff', '--name-only', '--no-renames', '-z', f'{args.base}...{args.head}'],
            text=True,
        ).split('\0')
        selected = products(paths)
        for name in ('sites', 'reviewer'):
            print(f'{name}={str(name in selected).lower()}')
        return
    repository = os.environ['GITHUB_REPOSITORY']
    run_id = int(os.environ['SOURCE_RUN_ID'])
    run, pr = resolve(repository, run_id)
    if args.command == 'resolve':
        print(f'pr={pr["number"] if pr else ""}')
        print(f'publish={str(pr is not None and publishable(run, pr, repository)).lower()}')
        print('reviewer=' + str(require_reviewer(repository, run_id)).lower())
        return
    if args.command == 'report':
        report(
            repository,
            run_id,
            os.environ.get('REVIEWER_URL', ''),
            os.environ.get('PUBLICATION_RESULT', 'not published'),
        )
        return
    if pr is None or not publishable(run, pr, repository):
        raise ValueError('Source run is no longer eligible for publication')
    if not require_reviewer(repository, run_id):
        raise ValueError('Reviewer did not produce one successful job and static artifact')
    if args.command == 'check-publication':
        return
    if args.command == 'package-reviewer':
        package_reviewer(args.source, args.target, run['head_sha'], run_id)
    elif args.command == 'verify-cdn':
        print('url=' + verify_cdn(args.version, run['head_sha'], run_id))


if __name__ == '__main__':
    main()
