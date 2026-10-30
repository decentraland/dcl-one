#!/usr/bin/env python3
import argparse
import base64
import hashlib
import html
import io
import json
import os
from pathlib import Path
import re
import struct
import urllib.error
import urllib.parse
import urllib.request
import zipfile

MARKER = '<!-- scene-cli-journeys -->'
WORKFLOW = '.github/workflows/scene-cli-journeys.yml'
ARTIFACT = 'scene-cli-journeys'


def manifest(raw):
    if len(raw) > 4_000_000:
        raise ValueError('Manifest exceeds 4 MB')
    value = json.loads(raw)
    if value.get('schema') != 1 or not isinstance(value.get('stages'), list) or len(value['stages']) > 100:
        raise ValueError('Invalid journey manifest')
    seen = set()
    for stage in value['stages']:
        key = stage['id']
        if not re.fullmatch(r'[a-z0-9-]{1,80}', key) or key in seen:
            raise ValueError('Invalid or duplicate stage ID')
        seen.add(key)
        if stage.get('screenshot') not in (None, key + '.png') or stage.get('status') not in ('passed', 'failed'):
            raise ValueError('Invalid screenshot path or stage status')
    return value


def png(data):
    if not 24 <= len(data) <= 5_000_000 or data[:16] != b'\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR':
        raise ValueError('Expected a PNG of at most 5 MB')
    width, height = struct.unpack('>II', data[16:24])
    if not (1 <= width <= 4096 and 1 <= height <= 12000):
        raise ValueError('Screenshot dimensions exceed limits')
    return data


def read_results(directory):
    result = {}
    for label in ('base', 'head'):
        file = directory / label / 'manifest.json'
        result[label] = manifest(file.read_bytes()) if file.exists() else {'stages': [], 'errors': ['Build or capture did not complete']}
    return result


def stages(results):
    return {label: {stage['id']: stage for stage in value['stages']} for label, value in results.items()}


def render(directory, output):
    results = read_results(directory)
    indexed = stages(results)
    esc = html.escape
    rows = []
    for key in dict.fromkeys([*indexed['head'], *indexed['base']]):
        pair = [indexed[label].get(key, {}) for label in ('base', 'head')]
        changed = pair[0].get('sha256') != pair[1].get('sha256')
        title = pair[1].get('title', pair[0].get('title', key))
        cells = []
        for label, stage in zip(('base', 'head'), pair):
            image = ''
            if stage.get('screenshot'):
                data = png((directory / label / stage['screenshot']).read_bytes())
                image = '<img loading="lazy" alt="' + esc(label + ': ' + title) + '" src="data:image/png;base64,' + base64.b64encode(data).decode() + '">'
            error = stage.get('error', '')
            cells.append(f'<div><strong>{label}</strong> · {esc(stage.get("status", "missing"))}<p>{esc(error)}</p>{image}</div>')
        rows.append(f'<section><h2>{esc(title)}</h2><p>{esc(key)} · {"Screenshot changed" if changed else "Same screenshot"}</p><div class="pair">{"".join(cells)}</div></section>')
    errors = '<br>'.join(esc(label + ': ' + str(error)) for label, value in results.items() for error in value.get('errors', []))
    document = '''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Scene CLI journeys</title><style>
body{font:16px/1.5 system-ui;margin:32px;background:#f6f5f1;color:#24272a}main{max-width:1500px;margin:auto}section{border-top:1px solid #ccc;padding:24px 0}h1,h2{line-height:1.2}.pair{display:grid;grid-template-columns:1fr 1fr;gap:20px}img{width:100%;cursor:zoom-in;background:white}img:focus{outline:3px solid #a22}p{max-width:85ch}dialog{width:95vw;max-width:1600px;border:0;padding:0}dialog::backdrop{background:#000b}dialog img{width:100%;cursor:zoom-out}@media(max-width:700px){body{margin:16px}.pair{grid-template-columns:1fr}}
</style><main><h1>Scene CLI journeys: before and after</h1><p>Real Chromium screenshots of disposable scenes and local services. Image changes are review evidence, not automatic visual failures. Click an image to enlarge; Escape closes it.</p>'''
    for label, value in results.items():
        document += f'<p><strong>{label}:</strong> {esc(value.get("revision", "unknown"))} · {len(value["stages"])} stages</p>'
    document += f'<p>{errors}</p>' + ''.join(rows)
    document += '<h2>External boundaries</h2><p>' + '<br>'.join(esc(x) for x in results['head'].get('boundaries', [])) + '</p></main>'
    document += '''<dialog aria-label="Full screenshot"><img alt="Enlarged screenshot"></dialog><script>const d=document.querySelector('dialog');document.querySelectorAll('main img').forEach(i=>{i.tabIndex=0;const open=()=>{d.querySelector('img').src=i.src;d.querySelector('img').alt=i.alt;d.showModal()};i.onclick=open;i.onkeydown=e=>{if(e.key==='Enter')open()}});d.onclick=()=>d.close()</script></html>'''
    output.write_text(document)
    return results


class Redirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, *args):
        redirected = super().redirect_request(request, *args)
        if redirected:
            redirected.remove_header('Authorization')
        return redirected


def api(endpoint, data=None, method=None, content_type='application/json', raw=False):
    url = endpoint if endpoint.startswith('https://') else 'https://api.github.com/' + endpoint
    headers = {'Authorization': 'Bearer ' + os.environ['GH_TOKEN'], 'Accept': 'application/vnd.github+json',
               'Content-Type': content_type, 'X-GitHub-Api-Version': '2022-11-28'}
    payload = data if isinstance(data, bytes) else json.dumps(data).encode() if data is not None else None
    request = urllib.request.Request(url, data=payload, headers=headers, method=method)
    with urllib.request.build_opener(Redirect()).open(request, timeout=60) as response:
        body = response.read(100_000_001)
        if len(body) > 100_000_000:
            raise ValueError('GitHub response exceeds 100 MB')
        return body if raw else json.loads(body) if body else None


def pages(endpoint):
    for page in range(1, 11):
        values = api(endpoint + ('&' if '?' in endpoint else '?') + f'per_page=100&page={page}')
        yield from values
        if len(values) < 100:
            return
    raise ValueError('GitHub pagination exceeds limit')


def current_pr(repo, run):
    if run['event'] != 'pull_request' or run['status'] != 'completed' or run['path'] != WORKFLOW:
        raise ValueError('Expected a completed pull-request journey workflow')
    candidates = run.get('pull_requests') or api(f'repos/{repo}/commits/{run["head_sha"]}/pulls')
    matches = []
    for candidate in candidates:
        pr = api(f'repos/{repo}/pulls/{candidate["number"]}')
        if pr['state'] == 'open' and pr['base']['repo']['full_name'] == repo and pr['head']['sha'] == run['head_sha']:
            matches.append(pr)
    if len(matches) > 1:
        raise ValueError('Ambiguous pull-request association')
    return matches[0] if matches else None


def read_archive(data):
    results, images = {}, {}
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        if len(archive.infolist()) > 250 or sum(x.file_size for x in archive.infolist()) > 100_000_000:
            raise ValueError('Journey artifact exceeds limits')
        names = archive.namelist()
        if len(set(names)) != len(names):
            raise ValueError('Duplicate artifact member')
        for label in ('base', 'head'):
            name = label + '/manifest.json'
            results[label] = manifest(archive.read(name)) if name in names else {'stages': [], 'errors': ['Capture unavailable']}
            for stage in results[label]['stages']:
                if stage.get('screenshot'):
                    name = label + '/' + stage['screenshot']
                    image = png(archive.read(name))
                    stage['sha256'] = hashlib.sha256(image).hexdigest()
                    images[label, stage['id']] = image
    return results, images


def comment(repo, run, pr, results, urls, artifact):
    indexed = stages(results)
    body = [MARKER, '## Scene CLI journeys',
            f'Commit `{pr["head"]["sha"][:12]}` · [workflow run]({run["html_url"]}) · **{run["conclusion"]}**',
            f'[Download HTML, all screenshots and logs](https://github.com/{repo}/actions/runs/{run["id"]}/artifacts/{artifact["id"]})',
            'Before = PR base; after = proposed commit. Screenshot differences request review; they do not fail the check.',
            'Wallet signatures and live external services are not exercised. Local fixtures contain no real credentials.']
    for label, value in results.items():
        failed = sum(stage['status'] == 'failed' for stage in value['stages'])
        body.append(f'\n**{label}:** {len(value["stages"])} stages, {failed} failures, {len(value.get("errors", []))} setup errors.')
    groups = {}
    for key in dict.fromkeys([*indexed['head'], *indexed['base']]):
        stage = indexed['head'].get(key, indexed['base'].get(key))
        group = re.sub(r'[^a-z0-9-]', '', stage.get('journey', 'other'))[:50]
        changed = indexed['base'].get(key, {}).get('sha256') != indexed['head'].get(key, {}).get('sha256')
        row = [f'**{key.replace("-", " ")}** · {"changed" if changed else "same screenshot"}', '| Before | After |', '| --- | --- |']
        cells = []
        for label in ('base', 'head'):
            url = urls.get((label, key))
            cells.append(f'![{label}]({url})' if url else 'No screenshot; inspect workflow logs')
        row.append('| ' + ' | '.join(cells) + ' |')
        groups.setdefault(group, []).append('\n'.join(row))
    for group, rows in groups.items():
        body.append(f'\n<details><summary>{group}: {len(rows)} stages</summary>\n\n' + '\n\n'.join(rows) + '\n\n</details>')
    text = '\n\n'.join(body)
    if len(text) > 60000:
        raise ValueError('Screenshot comment exceeds safe length')
    return text


def publish(repo, run_id):
    if not re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repo):
        raise ValueError('Invalid repository')
    prefix = f'repos/{repo}'
    run = api(f'{prefix}/actions/runs/{run_id}')
    pr = current_pr(repo, run)
    if not pr:
        print('No current open pull request; skipping stale results')
        return
    artifacts = api(f'{prefix}/actions/runs/{run_id}/artifacts?per_page=100')['artifacts']
    matching = [a for a in artifacts if a['name'] == ARTIFACT and not a['expired']]
    if len(matching) != 1 or matching[0]['size_in_bytes'] > 100_000_000:
        raise ValueError('Expected one unexpired journey artifact of at most 100 MB')
    artifact = matching[0]
    results, images = read_archive(api(f'{prefix}/actions/artifacts/{artifact["id"]}/zip', raw=True))
    if results['head'].get('revision') not in (None, pr['head']['sha']) or results['base'].get('revision') not in (None, pr['base']['sha']):
        raise ValueError('Artifact revisions differ from current PR head/base')
    tag = f'scene-cli-journeys-pr-{pr["number"]}'
    try:
        release = api(f'{prefix}/releases/tags/{tag}')
    except urllib.error.HTTPError as error:
        if error.code != 404:
            raise
        default = api(prefix)['default_branch']
        release = api(f'{prefix}/releases', {'tag_name': tag, 'target_commitish': default,
            'name': f'Scene CLI journey screenshots for PR #{pr["number"]}', 'prerelease': True, 'make_latest': 'false',
            'body': 'Latest before/after journey screenshots. Full reports and logs are GitHub Actions artifacts; these are not product binaries.'})
    if not release['prerelease'] or release['draft']:
        raise ValueError('Screenshot tag must name a published prerelease')
    assets = list(pages(f'{prefix}/releases/{release["id"]}/assets'))
    urls = {}
    run_prefix = f'run-{run_id}-{run["run_attempt"]}-'
    for (label, key), image in images.items():
        name = run_prefix + label + '-' + key + '.png'
        asset = next((a for a in assets if a['name'] == name), None)
        if asset is None:
            asset = api(f'https://uploads.github.com/{prefix}/releases/{release["id"]}/assets?name={urllib.parse.quote(name)}',
                        image, content_type='image/png')
        urls[label, key] = asset['browser_download_url']
    if api(f'{prefix}/pulls/{pr["number"]}')['head']['sha'] != pr['head']['sha']:
        print('PR advanced during image upload; not replacing its comment')
        return
    body = comment(repo, run, pr, results, urls, artifact)
    previous = [c for c in pages(f'{prefix}/issues/{pr["number"]}/comments')
                if c['user']['login'] == 'github-actions[bot]' and c['body'].startswith(MARKER)]
    if previous:
        api(f'{prefix}/issues/comments/{previous[-1]["id"]}', {'body': body}, method='PATCH')
    else:
        api(f'{prefix}/issues/{pr["number"]}/comments', {'body': body})
    for asset in assets:
        if re.fullmatch(r'run-\d+-\d+-(base|head)-[a-z0-9-]+\.png', asset['name']) and not asset['name'].startswith(run_prefix):
            api(f'{prefix}/releases/assets/{asset["id"]}', method='DELETE')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest='command', required=True)
    report = commands.add_parser('render')
    report.add_argument('directory', type=Path)
    publication = commands.add_parser('publish')
    publication.add_argument('--repo', required=True)
    publication.add_argument('--run-id', required=True, type=int)
    args = parser.parse_args()
    if args.command == 'render':
        render(args.directory, args.directory / 'report.html')
    else:
        publish(args.repo, args.run_id)
