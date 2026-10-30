#!/usr/bin/env python3
"""Dependency and drift map of the pinned Explorer: raw facts for the drift site.

`collect` gathers, into .cache/upstream-drift/data.json:
  - every git source of the pinned Cargo.lock (rendering/bevy-explorer/forks.toml): the parent
    repository, the release the fork is based on, the commits the fork carries on top of it, the
    upstream releases and commits the fork is missing, and the branch's last activity
  - RustSec advisories against every crate version in the lock (rustsec/advisory-db)
  - the Explorer's direct crates.io dependencies against the newest release on crates.io
  - the lock's shape: sources, duplicated crates
Every GitHub and crates.io response is cached under .cache/upstream-drift/api so reruns are cheap.
`render` (tooling/upstream-drift-site.py) turns the data, the research and the review into the site.
"""
import argparse
import base64
import datetime
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import time
import tomllib
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
PIN = ROOT / 'rendering/bevy-explorer'
CACHE = ROOT / '.cache/upstream-drift'
API_CACHE = CACHE / 'api'
DATA = CACHE / 'data.json'
USER_AGENT = 'dcl-one upstream-drift (https://github.com/decentraland/dcl-one)'
EXPLORER = 'decentraland/bevy-explorer'


# ---------------------------------------------------------------- cached fetchers

def cached(key, fetch):
    API_CACHE.mkdir(parents=True, exist_ok=True)
    path = API_CACHE / (hashlib.sha256(key.encode()).hexdigest()[:24] + '.json')
    if path.exists():
        return json.loads(path.read_text())
    value = fetch()
    if value is not None:  # a 404 is not cached: a later run may succeed (a new tag, a fixed path)
        path.write_text(json.dumps(value))
    return value


def gh(path):
    def fetch():
        result = subprocess.run(['gh', 'api', path], capture_output=True, text=True)
        if result.returncode:
            if 'HTTP 404' in result.stderr or 'Not Found' in result.stderr:
                return None
            raise RuntimeError(f'gh api {path}: {result.stderr.strip()[:300]}')
        return json.loads(result.stdout)
    return cached('gh:' + path, fetch)


def gh_pages(path, per_page=100, limit=10):
    """Concatenate paginated list results."""
    items = []
    for page in range(1, limit + 1):
        sep = '&' if '?' in path else '?'
        chunk = gh(f'{path}{sep}per_page={per_page}&page={page}')
        if not chunk:
            break
        items.extend(chunk)
        if len(chunk) < per_page:
            break
    return items


def crates_io(name):
    def fetch():
        time.sleep(1.1)  # crates.io asks for one request per second
        request = urllib.request.Request(f'https://crates.io/api/v1/crates/{urllib.parse.quote(name)}', headers={'User-Agent': USER_AGENT})
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            if error.code == 404:
                return None
            raise
    return cached('crates:' + name, fetch)


# ---------------------------------------------------------------- lock + manifests

def lock_packages(cargo_lock_text):
    packages = []
    for block in cargo_lock_text.split('[[package]]')[1:]:
        fields = dict(re.findall(r'^(\w+) = "([^"]*)"$', block, re.M))
        packages.append({'name': fields['name'], 'version': fields['version'], 'source': fields.get('source', '')})
    return packages


def github_repo(url):
    match = re.fullmatch(r'https://github\.com/([^/]+)/([^/]+?)(?:\.git)?', url)
    return match.groups() if match else None


def commit_row(c):
    return {'sha': c['sha'], 'date': c['commit']['committer']['date'][:10],
            'title': c['commit']['message'].splitlines()[0][:200],
            'author': (c.get('author') or {}).get('login') or c['commit']['author']['name']}


def compare_all(repo, base, head, cap=600):
    """Every commit of a compare (the API lists 250 per page), capped."""
    first = gh(f'repos/{repo}/compare/{base}...{head}?per_page=250&page=1')
    if first is None:
        return None
    commits = list(first['commits'])
    page = 2
    while len(commits) < min(first['total_commits'], cap) and len(first['commits']) == 250:
        more = gh(f'repos/{repo}/compare/{base}...{head}?per_page=250&page={page}')
        if not more or not more['commits']:
            break
        commits.extend(more['commits'])
        page += 1
    return {'status': first['status'], 'ahead_by': first['ahead_by'], 'behind_by': first['behind_by'],
            'total_commits': first['total_commits'], 'commits': [commit_row(c) for c in commits[:cap]]}


def tag_prefix(tag):
    """The non-numeric lead of a tag ('v', 'livekit-ffi/v', ''): one naming scheme per package."""
    return re.match(r'^[^0-9]*', tag).group(0)


def find_base_tag(parent, fork_repo, rev, crates, branch):
    """The newest release tag of the parent that the fork revision contains: candidates are tags
    naming the crate versions the lock resolved, tags matching the branch's version digits
    ('1_46_hotfix' -> 1.46.*), then the newest tags; each is checked with a compare whose
    behind_by == 0 means the tag is an ancestor of the fork."""
    tags = gh_pages(f'repos/{parent}/tags', limit=15)
    by_name = {t['name']: t['commit']['sha'] for t in tags}
    candidates = []
    for name, version in crates:
        for candidate in (f'v{version}', version, f'{name}-v{version}', f'{name}-{version}', f'{name}@{version}', f'{name}/v{version}'):
            if candidate in by_name:
                candidates.append(candidate)
    digits = re.findall(r'\d+', branch or '')
    if len(digits) >= 2:
        prefix = re.compile(r'^v?' + re.escape(digits[0]) + r'\.' + re.escape(digits[1]) + r'(\.|$)')
        candidates += sorted((t for t in by_name if prefix.match(t)), key=version_key, reverse=True)[:12]
    candidates += [t['name'] for t in tags[:25]]
    owner = parent.split('/')[0]
    # newest first: the first ancestor found is the latest release the fork contains
    for tag in sorted(set(candidates), key=version_key, reverse=True):
        compare = gh(f'repos/{fork_repo}/compare/{owner}:{urllib.parse.quote(tag, safe="")}...{rev}')
        if compare and compare['behind_by'] == 0:
            return tag, by_name[tag]
    return None, None


def tag_date(repo, sha):
    commit = gh(f'repos/{repo}/commits/{sha}')
    return commit['commit']['committer']['date'][:10] if commit else None


def version_key(tag):
    nums = re.findall(r'\d+', tag)
    return tuple(int(n) for n in nums[:4]) + (0,) * (4 - len(nums[:4]))


def releases_since(parent, base_date, base_tag):
    releases = gh_pages(f'repos/{parent}/releases', limit=3)
    out = []
    for r in releases:
        if r.get('draft'):
            continue
        published = (r.get('published_at') or '')[:10]
        if base_date and published and published <= base_date:
            continue
        if base_tag and (tag_prefix(r['tag_name']) != tag_prefix(base_tag) or version_key(r['tag_name']) <= version_key(base_tag)):
            continue
        out.append({'tag': r['tag_name'], 'name': r.get('name') or r['tag_name'], 'date': published,
                    'prerelease': bool(r.get('prerelease')), 'url': r['html_url'], 'body': (r.get('body') or '')[:6000]})
    return sorted(out, key=lambda r: r['date'])


def collect_sources(forks, lock_pkgs):
    sources = []
    for s in forks['source']:
        url, rev = s['url'], s['rev']
        repo = github_repo(url)
        entry = {'url': url, 'repo': '/'.join(repo) if repo else None, 'branch': s.get('branch'), 'manifest_rev': s.get('manifest_rev'),
                 'rev': rev, 'parent': s.get('parent'), 'default_branch': s.get('default_branch'), 'pushed_at': s.get('pushed_at'),
                 'reason': s.get('reason'), 'config_pin': s.get('config_pin'),
                 'crates': [{'name': c.split(' ')[0], 'version': c.split(' ')[1]} for c in s['crates']]}
        if repo is None:
            sources.append(entry)
            continue
        owner_repo = '/'.join(repo)
        info = gh(f'repos/{owner_repo}') or {}
        entry.update({'description': info.get('description'), 'stars': info.get('stargazers_count'), 'archived': info.get('archived')})
        head = gh(f'repos/{owner_repo}/commits/{rev}')
        entry['rev_date'] = head['commit']['committer']['date'][:10] if head else None
        if s.get('branch'):
            tip = gh(f'repos/{owner_repo}/commits/{urllib.parse.quote(s["branch"], safe="")}')
            entry['branch_tip'] = commit_row(tip) if tip else None
            moved = compare_all(owner_repo, rev, urllib.parse.quote(s['branch'], safe=''))
            entry['branch_since_pin'] = moved
        parent = s.get('parent')
        if parent:
            pinfo = gh(f'repos/{parent}') or {}
            entry['parent_info'] = {'description': pinfo.get('description'), 'stars': pinfo.get('stargazers_count'),
                                    'default_branch': pinfo.get('default_branch'), 'pushed_at': (pinfo.get('pushed_at') or '')[:10],
                                    'archived': pinfo.get('archived')}
            crates = [(c['name'], c['version']) for c in entry['crates']]
            base_tag, base_sha = find_base_tag(parent, owner_repo, rev, crates, s.get('branch'))
            entry['base'] = {'tag': base_tag, 'sha': base_sha, 'date': tag_date(parent, base_sha) if base_sha else None}
            default = pinfo.get('default_branch', 'main')
            if base_tag:
                carried = compare_all(repo[0] + '/' + repo[1], f'{parent.split("/")[0]}:{base_tag}', rev)
                entry['carried'] = carried  # the fork's own commits on top of the release
                missing_from_base = compare_all(parent, base_tag, default, cap=1000)
                entry['upstream_since_base'] = missing_from_base
            else:
                # no release is an ancestor of the fork: measure against the parent's default branch
                entry['carried'] = compare_all(owner_repo, f'{parent.split("/")[0]}:{default}', rev)
                entry['upstream_since_base'] = compare_all(owner_repo, rev, f'{parent.split("/")[0]}:{default}', cap=1000)
                entry['base_note'] = 'no upstream release tag is an ancestor of the fork; carried and missing are measured against the parent default branch'

            behind = gh(f'repos/{owner_repo}/compare/{parent.split("/")[0]}:{default}...{rev}')
            entry['vs_parent_default'] = {'ahead_by': behind['ahead_by'], 'behind_by': behind['behind_by']} if behind else None
            # commits only the fork has: drop the upstream commits between the base tag and the fork
            # point (an old base tag puts upstream history into the base-relative compare)
            if entry.get('carried') and entry.get('upstream_since_base'):
                upstream_shas = {c['sha'] for c in entry['upstream_since_base']['commits']}
                fork_only = [c for c in entry['carried']['commits'] if c['sha'] not in upstream_shas]
                entry['carried']['fork_only'] = fork_only
                entry['carried']['fork_only_count'] = len(fork_only) if entry['upstream_since_base']['total_commits'] <= len(upstream_shas) else min(len(fork_only), behind['ahead_by'] if behind else len(fork_only))
            entry['releases_since_base'] = releases_since(parent, entry['base']['date'], base_tag)
            latest = gh(f'repos/{parent}/releases/latest')
            entry['parent_latest_release'] = {'tag': latest['tag_name'], 'date': latest['published_at'][:10], 'url': latest['html_url']} if latest else None
        else:
            # not a fork: an upstream repository used at a branch or revision
            default = info.get('default_branch', 'main')
            since = compare_all(owner_repo, rev, default)
            entry['upstream_since_pin'] = since
            latest = gh(f'repos/{owner_repo}/releases/latest')
            entry['parent_latest_release'] = {'tag': latest['tag_name'], 'date': latest['published_at'][:10], 'url': latest['html_url']} if latest else None
        sources.append(entry)
        print(f'  {owner_repo}: base={entry.get("base", {}).get("tag")} carried={len((entry.get("carried") or {}).get("commits", []))} '
              f'upstream_since_base={(entry.get("upstream_since_base") or {}).get("total_commits")} releases={len(entry.get("releases_since_base", []))}', file=sys.stderr)
    return sources


# ---------------------------------------------------------------- advisories

def parse_version(v):
    parts = re.split(r'[.+-]', v)
    nums = []
    for p in parts[:3]:
        nums.append(int(p) if p.isdigit() else 0)
    while len(nums) < 3:
        nums.append(0)
    return tuple(nums), ('-' in v)


def matches_req(version, req):
    """A conservative subset of Cargo version requirements."""
    v, _pre = parse_version(version)
    ok = True
    for part in [p.strip() for p in req.split(',') if p.strip()]:
        m = re.match(r'(>=|<=|>|<|=|\^|~)?\s*([0-9][0-9A-Za-z.+-]*)', part)
        if not m:
            return False
        op, target = m.group(1) or '^', m.group(2)
        t, _ = parse_version(target)
        if op == '>=': ok &= v >= t
        elif op == '>': ok &= v > t
        elif op == '<=': ok &= v <= t
        elif op == '<': ok &= v < t
        elif op == '=': ok &= v == t
        elif op == '~': ok &= v[:2] == t[:2] and v >= t
        else:  # caret
            digits = len([p for p in target.split('.') if p.isdigit()])
            if t[0] > 0: ok &= v[0] == t[0] and v >= t
            elif digits >= 2 and t[1] > 0: ok &= v[:2] == t[:2] and v >= t
            else: ok &= v == t
    return ok


def parse_advisory(markdown):
    m = re.search(r'```toml\n(.*?)```', markdown, re.S)
    if not m:
        return None
    try:
        data = tomllib.loads(m.group(1))
    except tomllib.TOMLDecodeError:
        return None
    adv, versions = data.get('advisory', {}), data.get('versions', {})
    title = re.search(r'^#\s+(.+)$', markdown.split('```')[-1], re.M)
    return {'id': adv.get('id'), 'package': adv.get('package'), 'date': adv.get('date'), 'aliases': adv.get('aliases', []),
            'categories': adv.get('categories', []), 'keywords': adv.get('keywords', []), 'informational': adv.get('informational'),
            'url': adv.get('url'), 'title': (title.group(1).strip() if title else adv.get('id')),
            'patched': versions.get('patched', []), 'unaffected': versions.get('unaffected', [])}


def affected(version, advisory):
    if any(matches_req(version, r) for r in advisory['patched']):
        return False
    if any(matches_req(version, r) for r in advisory['unaffected']):
        return False
    return True


def collect_advisories(lock_pkgs):
    listing = gh_pages('repos/rustsec/advisory-db/contents/crates', limit=12)
    advised = {entry['name'] for entry in listing if entry['type'] == 'dir'}
    by_name = {}
    for p in lock_pkgs:
        by_name.setdefault(p['name'], set()).add(p['version'])
    hits = sorted(advised & set(by_name))
    findings = []
    for name in hits:
        files = gh(f'repos/rustsec/advisory-db/contents/crates/{name}') or []
        for f in files:
            if not f['name'].endswith('.md'):
                continue
            content = gh(f'repos/rustsec/advisory-db/contents/crates/{name}/{f["name"]}')
            if not content:
                continue
            advisory = parse_advisory(base64.b64decode(content['content']).decode('utf-8', 'replace'))
            if not advisory:
                continue
            for version in sorted(by_name[name]):
                findings.append({**advisory, 'crate': name, 'version': version, 'affected': affected(version, advisory),
                                 'rustsec_url': f'https://rustsec.org/advisories/{advisory["id"]}.html'})
    print(f'  advisories: {len(hits)} crate(s) in the lock have entries, {sum(1 for f in findings if f["affected"])} affecting the pinned versions', file=sys.stderr)
    return {'crates_with_entries': hits, 'findings': findings}


# ---------------------------------------------------------------- direct deps vs crates.io

def direct_registry_deps(source_dir):
    deps = {}
    for mf in [source_dir / 'Cargo.toml', *sorted((source_dir / 'crates').glob('*/Cargo.toml'))]:
        d = tomllib.loads(mf.read_text())
        tables = [d.get('dependencies', {}), d.get('build-dependencies', {}), d.get('workspace', {}).get('dependencies', {})]
        tables += [t.get('dependencies', {}) for t in d.get('target', {}).values()]
        for table in tables:
            for name, spec in table.items():
                if isinstance(spec, str):
                    deps.setdefault(name, spec)
                elif isinstance(spec, dict) and 'git' not in spec and 'path' not in spec and 'version' in spec:
                    deps.setdefault(spec.get('package', name), spec['version'])
    return deps


def collect_registry_drift(source_dir, lock_pkgs):
    deps = direct_registry_deps(source_dir)
    locked = {}
    for p in lock_pkgs:
        if 'crates.io' in p['source']:
            locked.setdefault(p['name'], []).append(p['version'])
    rows = []
    for name in sorted(deps):
        if name not in locked:
            continue
        meta = crates_io(name)
        if not meta:
            continue
        crate = meta['crate']
        latest = crate.get('max_stable_version') or crate.get('max_version')
        current = sorted(locked[name], key=lambda v: parse_version(v)[0])[-1]
        cur, lat = parse_version(current)[0], parse_version(latest)[0] if latest else (0, 0, 0)
        if lat[0] != cur[0] or (cur[0] == 0 and lat[1] != cur[1]):
            gap = 'major'
        elif lat[1] != cur[1]:
            gap = 'minor'
        elif lat[2] != cur[2]:
            gap = 'patch'
        else:
            gap = 'current'
        rows.append({'name': name, 'requirement': deps[name], 'locked': sorted(locked[name]), 'latest': latest,
                     'latest_date': (crate.get('updated_at') or '')[:10], 'gap': gap, 'description': (crate.get('description') or '')[:160],
                     'repository': crate.get('repository'), 'downloads': crate.get('downloads')})
    print(f'  registry: {len(rows)} direct crates.io dependencies, {sum(1 for r in rows if r["gap"] == "major")} a major behind', file=sys.stderr)
    return rows


# ---------------------------------------------------------------- lock shape

def lock_shape(lock_pkgs):
    by_source = {}
    for p in lock_pkgs:
        kind = 'crates.io' if 'crates.io' in p['source'] else 'git' if p['source'].startswith('git+') else 'workspace'
        by_source[kind] = by_source.get(kind, 0) + 1
    names = {}
    for p in lock_pkgs:
        names.setdefault(p['name'], set()).add(p['version'])
    duplicates = sorted((n, sorted(v)) for n, v in names.items() if len(v) > 1)
    return {'packages': len(lock_pkgs), 'by_source': by_source,
            'duplicates': [{'name': n, 'versions': v} for n, v in duplicates]}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('command', choices=['collect'])
    args = parser.parse_args()
    forks = tomllib.loads((PIN / 'forks.toml').read_text())
    source_dir = PIN / 'source'
    revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    lock_pkgs = lock_packages((source_dir / 'Cargo.lock').read_text())
    explorer = gh(f'repos/{EXPLORER}')
    data = {'generated': datetime.date.today().isoformat(), 'explorer': {'repo': EXPLORER, 'revision': revision,
            'description': explorer.get('description') if explorer else None},
            'lock': lock_shape(lock_pkgs)}
    print('sources', file=sys.stderr)
    data['sources'] = collect_sources(forks, lock_pkgs)
    print('advisories', file=sys.stderr)
    data['advisories'] = collect_advisories(lock_pkgs)
    print('registry drift', file=sys.stderr)
    data['registry'] = collect_registry_drift(source_dir, lock_pkgs)
    CACHE.mkdir(parents=True, exist_ok=True)
    DATA.write_text(json.dumps(data, indent=1))
    # one file per source for the research agents, with the advisories against its crates
    slices = CACHE / 'sources'
    slices.mkdir(exist_ok=True)
    for src in data['sources']:
        names = {c['name'] for c in src['crates']}
        payload = {**src, 'advisories': [f for f in data['advisories']['findings'] if f['crate'] in names],
                   'slug': re.sub(r'[^a-z0-9]+', '-', src['url'].lower().removeprefix('https://github.com/')).strip('-')}
        (slices / f"{payload['slug']}.json").write_text(json.dumps(payload, indent=1))
    print(f'wrote {len(data["sources"])} source slices to {slices.relative_to(ROOT)}')
    print(f'wrote {DATA.relative_to(ROOT)} ({DATA.stat().st_size // 1024} KiB)')
    return 0


if __name__ == '__main__':
    sys.exit(main())
