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
from urllib.request import Request, urlopen
from urllib.parse import urljoin

WORKFLOW = '.github/workflows/web-previews.yml'
PIPELINE_INPUTS = {
    WORKFLOW,
    '.github/workflows/web-previews-publish.yml',
    'tooling/web-preview.py',
    'tooling/tests/test_web_preview.py',
    'tooling/web-preview-browser.mjs',
}
PACKAGE = '@dcl-regenesislabs/dcl-one-ui-review'
MARKER = '<!-- web-pr-previews -->'
CDN_USER_AGENT = 'dcl-one-web-preview/1.0 (+https://github.com/decentraland/dcl-one)'
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


def bot_account(account):
    account = account or {}
    return account.get('type') == 'Bot' or str(account.get('login', '')).endswith('[bot]')


def publishable(run, pr, repository):
    return (
        run.get('event') == 'pull_request'
        and run.get('status') == 'completed'
        and run.get('conclusion') in ('success', 'failure')
        and run.get('path') == WORKFLOW
        and not any(bot_account(run.get(field)) for field in ('actor', 'triggering_actor'))
        and not bot_account(pr.get('user'))
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


def package_reviewer(source, target, revision, run_id, attempt=1):
    if (
        not re.fullmatch(r'[0-9a-f]{40}', revision)
        or run_id < 1
        or type(attempt) is not int
        or attempt < 1
    ):
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
                'repository': {
                    'type': 'git',
                    'url': 'git+https://github.com/decentraland/dcl-one.git',
                },
            }
        )
        + '\n'
    )
    (target / 'preview-source.json').write_text(
        json.dumps({'revision': revision, 'run': run_id, 'attempt': attempt}) + '\n'
    )


def api(path, *, method='GET', body=None, paginate=False, collection=None):
    args = ['gh', 'api', '--method', method, path]
    if paginate:
        args += ['--paginate', '--slurp']
    if body is not None:
        args += ['--input', '-']
    attempts = 3 if method in ('GET', 'PATCH') else 1
    for attempt in range(attempts):
        try:
            result = subprocess.run(
                args,
                input=json.dumps(body) if body is not None else None,
                capture_output=True,
                text=True,
                check=False,
                timeout=30,
            )
        except subprocess.TimeoutExpired:
            if attempt == attempts - 1:
                raise
        else:
            if result.returncode:
                status = re.search(r'HTTP[ /](\d{3})', result.stderr or '')
                limited = status and (
                    int(status[1]) == 429
                    or (
                        int(status[1]) == 403
                        and re.search(r'rate.?limit|secondary limit|abuse', result.stderr, re.I)
                    )
                )
                if (status and int(status[1]) < 500 and not limited) or attempt == attempts - 1:
                    print(result.stderr, file=sys.stderr)
                    result.check_returncode()
            else:
                try:
                    value = json.loads(result.stdout)
                except json.JSONDecodeError:
                    if attempt == attempts - 1:
                        raise
                else:
                    if paginate:
                        return [
                            item
                            for page in value
                            for item in (page[collection] if collection else page)
                        ]
                    return value
        print(f'Transient GitHub API failure; retrying ({attempt + 1}/{attempts})', file=sys.stderr)
        time.sleep(2**attempt)


def require_publication_environment(repository):
    environment = api(f'repos/{repository}/environments/web-preview')
    reviewers = [
        item
        for rule in environment.get('protection_rules', [])
        if rule.get('type') == 'required_reviewers'
        for item in rule.get('reviewers', [])
        if item.get('type') == 'User'
        and (item.get('reviewer') or {}).get('type') == 'User'
        and not bot_account(item.get('reviewer'))
    ]
    if not reviewers or environment.get('can_admins_bypass') is not False:
        raise ValueError('web-preview must require a human reviewer and disable admin bypass')
    policy = environment.get('deployment_branch_policy') or {}
    if (
        policy.get('custom_branch_policies') is not True
        or policy.get('protected_branches') is not False
    ):
        raise ValueError('web-preview must restrict deployments to the default branch')
    default = api(f'repos/{repository}')['default_branch']
    policies = api(
        f'repos/{repository}/environments/web-preview/deployment-branch-policies?per_page=100'
    )
    branches = policies['branch_policies']
    if (
        policies['total_count'] != 1
        or len(branches) != 1
        or branches[0].get('name') != default
        or branches[0].get('type') != 'branch'
    ):
        raise ValueError(
            'web-preview must allow only the default branch, without tags or wildcards'
        )


def build_summary(selection, reviewer, sites, enabled, repository, run_id):
    run_url = f'https://github.com/{repository}/actions/runs/{run_id}'
    rows = [
        '## Web previews',
        '',
        '| Product | Build | Preview |',
        '| --- | --- | --- |',
    ]
    if selection != 'success':
        rows.append(f'| Preview selection | {selection} | See the changes job |')
    for label, result in [('UI reviewer', reviewer), ('Sites', sites)]:
        if result == 'skipped':
            continue
        if result != 'success':
            state = 'Unavailable: build or CDN-path validation did not succeed'
        elif label == 'Sites':
            state = 'Build only. SSR hosting integration is pending'
        elif not enabled:
            state = 'CDN publication disabled: source CDN-path verification and activation pending'
        else:
            state = 'Pending the separate publisher, source checks and human approval'
        rows.append(f'| {label} | {result} | {state} |')
    if reviewer == sites == 'skipped' and selection == 'success':
        rows.append('| Web previews | not selected | No web products in the cumulative PR diff |')
    rows += [
        '',
        f'[Build logs and downloadable artifacts]({run_url}). Artifacts expire after seven days.',
        '',
        'The publisher workflow must be installed on the default branch. '
        'A successful build does not confirm a deployed preview. '
        'The publisher posts the verified CDN URL in the PR comment after human approval.',
    ]
    return '\n'.join(rows) + '\n'


def same_pull_request(candidate, pr):
    base = candidate.get('base') or {}
    head = candidate.get('head') or {}
    return bool(
        candidate.get('number') == pr.get('number')
        and base.get('ref')
        and head.get('sha')
        and base.get('ref') == (pr.get('base') or {}).get('ref')
        and head.get('sha') == (pr.get('head') or {}).get('sha')
    )


def newest_run_for_pr(repository, run_id, run, pr):
    runs = api(
        f'repos/{repository}/actions/workflows/web-previews.yml/runs'
        f'?event=pull_request&head_sha={run["head_sha"]}&per_page=100',
        paginate=True,
        collection='workflow_runs',
    )
    if not any(int(candidate['id']) == run_id for candidate in runs):
        return False
    for candidate in runs:
        candidate_id = int(candidate['id'])
        if candidate_id <= run_id or candidate.get('conclusion') in ('skipped', 'cancelled'):
            continue
        associations = candidate.get('pull_requests')
        if associations and not any(same_pull_request(item, pr) for item in associations):
            continue
        jobs = api(f'repos/{repository}/actions/runs/{candidate_id}/jobs?per_page=100')['jobs']
        if not jobs:
            continue
        selection = selection_job(jobs)
        if selection and selection.get('conclusion') == 'skipped':
            continue
        if associations:
            return False
        _, candidate_pr = resolve(repository, candidate_id)
        if candidate_pr and same_pull_request(candidate_pr, pr):
            return False
    return True


def selection_job(jobs):
    return next((job for job in jobs if job['name'] == 'changes'), None)


def resolve(repository, run_id, *, expected_attempt=None):
    run = api(f'repos/{repository}/actions/runs/{run_id}')
    if expected_attempt is not None and str(run.get('run_attempt')) != str(expected_attempt):
        return run, None
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
    if not isinstance(version, str) or not re.fullmatch(
        r'[0-9]+\.[0-9]+\.[0-9]+-[0-9]+\.commit-[0-9a-f]+', version
    ):
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


class CdnResourceNotReady(ValueError):
    pass


def cdn_open(url):
    return urlopen(Request(url, headers={'User-Agent': CDN_USER_AGENT}), timeout=15)


def verify_reviewer_resources(url, entry, html):
    parser = ReviewerAssets()
    parser.feed(html)
    if not parser.assets:
        raise CdnResourceNotReady('Reviewer entry has no hashed JavaScript or CSS asset')
    asset = urljoin(entry, parser.assets[0])
    if not asset.startswith(url + '/'):
        raise ValueError('Reviewer asset escapes the versioned CDN prefix')
    with cdn_open(asset) as response:
        content_type = response.headers.get('Content-Type', '')
        expected = 'text/css' if asset.endswith('.css') else 'javascript'
        if expected not in content_type or not response.read(1):
            raise CdnResourceNotReady('CDN reviewer asset is not JavaScript or CSS')
    with cdn_open(f'{url}/dcl-ui-review/fidelity.json') as response:
        fidelity = json.load(response)
        if not isinstance(fidelity, (dict, list)):
            raise CdnResourceNotReady('CDN reviewer fidelity data is not a JSON object or array')


def verify_cdn(version, revision, run_id, timeout=300, *, attempt=1):
    url = cdn_url(version)
    deadline = time.monotonic() + timeout
    while True:
        try:
            with cdn_open(f'{url}/preview-source.json') as response:
                identity = json.load(response)
            if identity != {'revision': revision, 'run': run_id, 'attempt': attempt}:
                raise ValueError('CDN preview belongs to a different source build')
            with cdn_open(f'{url}/dcl-ui-review/index.html') as response:
                if response.status == 200 and 'text/html' in response.headers.get(
                    'Content-Type', ''
                ):
                    entry = f'{url}/dcl-ui-review/index.html'
                    verify_reviewer_resources(url, entry, response.read().decode('utf-8'))
                    return entry
        except HTTPError as error:
            error.close()
        except (
            OSError,
            http.client.HTTPException,
            json.JSONDecodeError,
            UnicodeDecodeError,
            CdnResourceNotReady,
        ):
            pass
        if time.monotonic() >= deadline:
            raise TimeoutError(
                'Reviewer is not available on the CDN; no preview link will be posted'
            )
        time.sleep(5)


def complete_cdn(repository, event):
    deployment = event.get('deployment') or {}
    payload = deployment.get('payload') or {}
    if (
        not isinstance(payload, dict)
        or deployment.get('environment') != 'cdn'
        or deployment.get('task') != 'upload-to-cdn'
        or payload.get('packageName') != PACKAGE
        or (event.get('deployment_status') or {}).get('state') != 'success'
    ):
        print('Not a successful reviewer CDN upload')
        return
    version = payload.get('packageVersion', '')
    with cdn_open(f'{cdn_url(version)}/preview-source.json') as response:
        source = json.load(response)
    if (
        not isinstance(source, dict)
        or not isinstance(source.get('revision'), str)
        or not re.fullmatch(r'[0-9a-f]{40}', source['revision'])
        or type(source.get('run')) is not int
        or source['run'] < 1
        or type(source.get('attempt')) is not int
        or source['attempt'] < 1
    ):
        raise ValueError('Invalid CDN source receipt')
    run, pr = resolve(repository, source['run'], expected_attempt=source['attempt'])
    if pr is None:
        print('CDN upload no longer matches a current open PR build')
        return
    if run['head_sha'] != source['revision']:
        raise ValueError('CDN receipt does not match the source build revision')
    # Upload readiness comes from deployment_status. A retry of this job probes the same version.
    url = verify_cdn(
        version, source['revision'], source['run'], timeout=0, attempt=source['attempt']
    )
    report(repository, source['run'], url, 'success', expected_attempt=source['attempt'])


def report(repository, run_id, reviewer_url, publication_result, *, expected_attempt=None):
    run, pr = resolve(repository, run_id, expected_attempt=expected_attempt)
    if pr is None:
        print('No current same-repository pull request to report into')
        return
    jobs = api(f'repos/{repository}/actions/runs/{run_id}/jobs?per_page=100')['jobs']
    rows = []
    selection = selection_job(jobs)
    if selection and selection.get('conclusion') == 'skipped':
        print('Preview selection was skipped; keep the existing preview status')
        return
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
    no_products = not rows
    if no_products:
        rows.append('| Web previews | not selected | No web products in the cumulative PR diff |')
    body = '\n'.join(
        [
            MARKER,
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
    if no_products and current is None:
        print('No selected web products to report')
        return
    # Recheck after the API reads so an outdated source cannot replace the current preview.
    _, live = resolve(repository, run_id, expected_attempt=expected_attempt)
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
    sub.add_parser('check-environment')
    sub.add_parser('build-summary')
    sub.add_parser('complete-cdn')
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
    if args.command == 'build-summary':
        summary = build_summary(
            os.environ['SELECTION_RESULT'],
            os.environ['REVIEWER_RESULT'],
            os.environ['SITES_RESULT'],
            os.environ.get('PUBLICATION_ENABLED') == 'true',
            repository,
            int(os.environ['GITHUB_RUN_ID']),
        )
        with Path(os.environ['GITHUB_STEP_SUMMARY']).open('a') as output:
            output.write(summary)
        return
    if args.command == 'check-environment':
        require_publication_environment(repository)
        return
    if args.command == 'complete-cdn':
        complete_cdn(repository, json.loads(Path(os.environ['GITHUB_EVENT_PATH']).read_text()))
        return
    run_id = int(os.environ['SOURCE_RUN_ID'])
    expected_attempt = os.environ.get('SOURCE_RUN_ATTEMPT') or None
    if args.command == 'report':
        report(
            repository,
            run_id,
            os.environ.get('REVIEWER_URL', ''),
            os.environ.get('PUBLICATION_RESULT', 'not published'),
            expected_attempt=expected_attempt,
        )
        return
    run, pr = resolve(repository, run_id, expected_attempt=expected_attempt)
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
        require_publication_environment(repository)
        return
    if args.command == 'package-reviewer':
        package_reviewer(args.source, args.target, run['head_sha'], run_id, run['run_attempt'])
    elif args.command == 'verify-cdn':
        print(
            'url=' + verify_cdn(args.version, run['head_sha'], run_id, attempt=run['run_attempt'])
        )


if __name__ == '__main__':
    main()
