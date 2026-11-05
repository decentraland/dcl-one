#!/usr/bin/env python3
"""Select web preview builds and publish only validated static build outputs."""

import argparse
import http.client
from html.parser import HTMLParser
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
from urllib.error import HTTPError
from urllib.request import urlopen
from urllib.parse import urljoin

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


def same_pull_request(candidate, pr):
    base = candidate.get('base') or {}
    head = candidate.get('head') or {}
    return bool(
        candidate.get('number') == pr.get('number')
        and base.get('ref')
        and base.get('sha')
        and head.get('sha')
        and base.get('ref') == (pr.get('base') or {}).get('ref')
        and base.get('sha') == (pr.get('base') or {}).get('sha')
        and head.get('sha') == (pr.get('head') or {}).get('sha')
    )


def newest_run_for_pr(repository, run_id, run, pr):
    runs = api(
        f'repos/{repository}/actions/workflows/web-previews.yml/runs'
        f'?event=pull_request&head_sha={run["head_sha"]}&per_page=100'
    )['workflow_runs']
    matching = []
    for candidate in runs:
        candidate_id = int(candidate['id'])
        if candidate_id == run_id:
            matching.append(candidate_id)
        elif candidate.get('pull_requests'):
            if any(same_pull_request(item, pr) for item in candidate['pull_requests']):
                matching.append(candidate_id)
        else:
            _, candidate_pr = resolve(repository, candidate_id)
            if candidate_pr and same_pull_request(candidate_pr, pr):
                matching.append(candidate_id)
    return bool(matching) and max(matching) == run_id


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
    matches = []
    for candidate in candidates:
        pr = api(f'repos/{repository}/pulls/{int(candidate["number"])}')
        if same_pull_request(candidate, pr) and publishable(run, pr, repository):
            matches.append(pr)
    return run, matches[0] if len(matches) == 1 else None


def cdn_url(version):
    if not re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+-[0-9]+\.commit-[0-9a-f]+', version):
        raise ValueError('Invalid published preview version')
    return f'https://cdn.decentraland.org/{PACKAGE}/{version}'


class ReviewerAssets(HTMLParser):
    def __init__(self):
        super().__init__()
        self.assets = []

    def handle_starttag(self, tag, attrs):
        values = dict(attrs)
        source = (
            values.get('src')
            if tag == 'script'
            else (
                values.get('href') if tag == 'link' and values.get('rel') == 'stylesheet' else None
            )
        )
        if source and re.search(r'/assets/[^/]+-[A-Za-z0-9_-]+\.(js|css)$', source):
            self.assets.append(source)


def verify_reviewer_resources(url, entry, html):
    parser = ReviewerAssets()
    parser.feed(html)
    if not parser.assets:
        raise ValueError('Reviewer entry has no hashed JavaScript or CSS asset')
    asset = urljoin(entry, parser.assets[0])
    if not asset.startswith(url + '/'):
        raise ValueError('Reviewer asset escapes the versioned CDN prefix')
    with urlopen(asset, timeout=15) as response:
        content_type = response.headers.get('Content-Type', '')
        expected = 'text/css' if asset.endswith('.css') else 'javascript'
        if expected not in content_type or not response.read(1):
            raise ValueError('CDN reviewer asset is not JavaScript or CSS')
    with urlopen(f'{url}/dcl-ui-review/fidelity.json', timeout=15) as response:
        fidelity = json.load(response)
        if not isinstance(fidelity, (dict, list)):
            raise ValueError('CDN reviewer fidelity data is not a JSON object or array')


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
                    entry = f'{url}/dcl-ui-review/index.html'
                    verify_reviewer_resources(url, entry, response.read().decode('utf-8'))
                    return entry
        except HTTPError as error:
            error.close()
        except (OSError, http.client.HTTPException, json.JSONDecodeError):
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
    selection = next((job for job in jobs if job['name'] == 'changes'), None)
    if selection and selection['conclusion'] == 'failure':
        rows.append(f'| Preview selection | failure | [Build log]({selection["html_url"]}) |')
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
    if not rows and run.get('conclusion') == 'failure':
        rows.append(f'| Preview selection | failure | [Build run]({run["html_url"]}) |')
    if not rows:
        print('No selected web products to report')
        return
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
    # Recheck after the API reads so an outdated source cannot replace the current preview.
    _, live = resolve(repository, run_id)
    if live is None or not same_pull_request(live, pr):
        return
    if not newest_run_for_pr(repository, run_id, run, live):
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
    if args.command == 'report':
        report(
            repository,
            run_id,
            os.environ.get('REVIEWER_URL', ''),
            os.environ.get('PUBLICATION_RESULT', 'not published'),
        )
        return
    run, pr = resolve(repository, run_id)
    if args.command == 'resolve':
        print(f'pr={pr["number"] if pr else ""}')
        print(f'publish={str(pr is not None).lower()}')
        print('reviewer=' + str(require_reviewer(repository, run_id)).lower())
        return
    if pr is None:
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
