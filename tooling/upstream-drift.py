#!/usr/bin/env python3
"""Dependency and drift map of the pinned Explorer: raw facts for the drift site.

`collect` gathers, into .cache/upstream-drift/data.json:
  - every git source of the pinned Cargo.lock (rendering/bevy-explorer/forks.toml): the parent
    repository, the release the fork is based on, the commits the fork carries on top of it, the
    upstream releases and commits the fork is missing, and the branch's last activity
  - RustSec advisories against every crate version in the lock (rustsec/advisory-db)
  - the Explorer's direct crates.io dependencies against the newest release on crates.io
  - the lock's shape: sources, duplicated crates
  - the age of the scene runtime's V8 (the transitive `v8` crate) against deno_core, rusty_v8 and Chrome
  - the age of the catalyst's V8 (its direct `v8` pin, services/catalyst/Cargo.lock) against rusty_v8 and Chrome
Every GitHub and crates.io response is cached under .cache/upstream-drift/api so reruns are cheap.
`render` (tooling/upstream-drift-site.py) turns the data, the research and the review into the site.
`v8-report` reads the V8 sections of data.json, prints them and writes the issue body when either V8 is stale.
"""
import argparse
import base64
import datetime
import hashlib
import json
import os
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
CATALYST_LOCK = ROOT / 'services/catalyst/Cargo.lock'
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
    return crates_io_api(f'crates/{urllib.parse.quote(name)}')


def crates_io_api(path):
    """Any crates.io API path ('crates/v8', 'crates/v8/0.105.1/dependencies')."""
    def fetch():
        time.sleep(1.1)  # crates.io asks for one request per second
        request = urllib.request.Request(f'https://crates.io/api/v1/{path}', headers={'User-Agent': USER_AGENT})
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            if error.code == 404:
                return None
            raise
    return cached('crates:' + path, fetch)


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


# ---------------------------------------------------------------- V8 freshness

RUSTY_V8 = 'denoland/rusty_v8'
CHROME_STABLE = 'https://chromiumdash.appspot.com/fetch_releases?channel=Stable&platform=Linux&num=1'
CHROME_GAP_LIMIT = 12  # milestones; Chrome ships one every four weeks, so about a year


def v8_engine(crate_version, readme=None):
    """V8 version and Chrome milestone of a `v8` crate version. rusty_v8's README states the V8
    version at every tag ('V8 Version: 12.9.202.9'); V8 a.b ships in Chrome milestone a*10+b.
    Without the README, 129.0.0 and later map from the crate major (rusty_v8 took Chrome-style
    versions there: V8 M//10.M%10); 0.x releases have no such mapping and stay unknown."""
    match = re.search(r'^V8 Version:\s*((\d+)\.(\d+)[0-9.]*)', readme or '', re.M)
    if match:
        return {'v8': match.group(1), 'milestone': int(match.group(2)) * 10 + int(match.group(3))}
    major = parse_version(crate_version)[0][0]
    if major >= 129:
        return {'v8': f'{major // 10}.{major % 10}', 'milestone': major}
    return {'v8': None, 'milestone': None}


def rusty_v8_readme(version):
    content = gh(f'repos/{RUSTY_V8}/contents/README.md?ref=v{version}')
    return base64.b64decode(content['content']).decode('utf-8', 'replace') if content else None


def newest_matching(versions, req):
    """The newest stable, unyanked version of a crates.io version list matching a requirement."""
    ok = [v['num'] for v in versions if not v.get('yanked') and '-' not in v['num'] and matches_req(v['num'], req)]
    return max(ok, key=lambda v: parse_version(v)[0]) if ok else None


def v8_requirement(name, version, depth=2):
    """The `v8` requirement of a crate version, followed through deno_v8 (deno_core -> deno_v8 -> v8)."""
    deps = [d for d in (crates_io_api(f'crates/{name}/{version}/dependencies') or {}).get('dependencies', []) if d['kind'] == 'normal']
    for dep in deps:
        if dep['crate_id'] == 'v8':
            return dep['req']
    for dep in deps:
        if dep['crate_id'] == 'deno_v8' and depth:
            meta = crates_io('deno_v8')
            resolved = newest_matching(meta['versions'], dep['req']) if meta else None
            return v8_requirement('deno_v8', resolved, depth - 1) if resolved else None
    return None


def chrome_stable():
    def fetch():
        request = urllib.request.Request(CHROME_STABLE, headers={'User-Agent': USER_AGENT})
        with urllib.request.urlopen(request, timeout=60) as response:
            return json.load(response)
    release = cached('chrome-stable:' + datetime.date.today().isoformat(), fetch)[0]
    return {'milestone': release['milestone'], 'version': release['version'],
            'date': datetime.datetime.fromtimestamp(release['time'] / 1000, datetime.UTC).date().isoformat()}


def v8_lock_findings(v8):
    """Findings on the lock itself, and whether the locked V8 is known well enough to age-check."""
    locked = v8['locked']
    findings = []
    if v8.get('unregistered'):
        findings.append(f"v8 outside crates.io, its age is not checked: {', '.join(v8['unregistered'])}")
    if locked is None:
        return findings, False
    if locked['milestone'] is None:
        findings.append(f"cannot tell the V8 version of the locked v8 {locked['crate']}")
        return findings, False
    if len(v8['locked_versions']) > 1:
        findings.append(f"the lock has more than one v8: {', '.join(v8['locked_versions'])}")
    return findings, True


def chrome_gap_findings(v8):
    locked, chrome = v8['locked'], v8['chrome']
    if chrome and chrome['milestone'] - locked['milestone'] >= CHROME_GAP_LIMIT:
        return [f"M{locked['milestone']} is {chrome['milestone'] - locked['milestone']} milestones behind Chrome stable "
                f"M{chrome['milestone']} (limit {CHROME_GAP_LIMIT})"]
    return []


def v8_findings(v8):
    """Why the scene runtime's V8 needs attention; empty when it is current enough."""
    locked, accepted = v8['locked'], v8['accepted']
    findings, checkable = v8_lock_findings(v8)
    if not checkable:
        return findings
    if not (accepted and accepted['milestone']):
        # Degraded data must keep the issue open, not close it.
        findings.append(f"unable to resolve which v8 deno_core {v8['deno_core']['version']} accepts "
                        f"(requirement: {v8['deno_core']['v8_req'] or 'not found'})")
    elif locked['milestone'] < accepted['milestone']:
        findings.append(f"behind deno_core: v8 {locked['crate']} (V8 {locked['v8']}, M{locked['milestone']}) is locked while "
                        f"deno_core {v8['deno_core']['version']} accepts v8 {accepted['crate']} (V8 {accepted['v8']}, M{accepted['milestone']}); "
                        'bump deno_core, deno_web and deno_webidl together')
    return findings + chrome_gap_findings(v8)


def catalyst_v8_findings(v8):
    """The catalyst pins v8 directly, so any newer rusty_v8 is a bump it can take."""
    locked, newest = v8['locked'], v8['newest']
    findings, checkable = v8_lock_findings(v8)
    if not checkable:
        return findings
    if not (newest and newest['milestone']):
        findings.append('unable to resolve the newest rusty_v8 on crates.io')
    elif parse_version(locked['crate'])[0] < parse_version(newest['crate'])[0]:
        findings.append(f"behind the newest rusty_v8: v8 {locked['crate']} (V8 {locked['v8']}, M{locked['milestone']}) is pinned while "
                        f"v8 {newest['crate']} (V8 {newest['v8']}, M{newest['milestone']}) is out; bump the `=` pin in services/catalyst/Cargo.toml")
    return findings + chrome_gap_findings(v8)


def v8_lock(lock_pkgs):
    """The lock's registry v8 versions and any v8 from git or a path, or None without a v8."""
    v8_pkgs = [p for p in lock_pkgs if p['name'] == 'v8']
    if not v8_pkgs:
        return None
    return (sorted({p['version'] for p in v8_pkgs if 'crates.io' in p['source']}, key=lambda v: parse_version(v)[0]),
            sorted(f"{p['version']} ({p['source'] or 'path'})" for p in v8_pkgs if 'crates.io' not in p['source']))


def v8_describer(v8_meta):
    released = {v['num']: v['created_at'][:10] for v in v8_meta['versions']}

    def describe(version):
        return {'crate': version, 'date': released.get(version), **v8_engine(version, rusty_v8_readme(version))} if version else None
    return describe


def collect_v8(lock_pkgs):
    lock = v8_lock(lock_pkgs)
    if not lock:
        return None
    locked_versions, unregistered = lock
    v8_meta = crates_io('v8')
    describe = v8_describer(v8_meta)
    deno_core = crates_io('deno_core')['crate']['max_stable_version']
    req = v8_requirement('deno_core', deno_core)
    v8 = {'locked_versions': locked_versions, 'unregistered': unregistered,
          'locked': describe(locked_versions[0] if locked_versions else None),
          'deno_core': {'version': deno_core, 'v8_req': req},
          'accepted': describe(newest_matching(v8_meta['versions'], req) if req else None),
          'newest': describe(v8_meta['crate']['max_stable_version']), 'chrome': chrome_stable(),
          'chrome_gap_limit': CHROME_GAP_LIMIT}
    v8['findings'] = v8_findings(v8)
    print(f"  v8: locked {(v8['locked'] or {}).get('crate')} (M{(v8['locked'] or {}).get('milestone')}), deno_core {deno_core} accepts "
          f"{(v8['accepted'] or {}).get('crate')}, newest {v8['newest']['crate']}, Chrome stable M{v8['chrome']['milestone']}: "
          f"{len(v8['findings'])} finding(s)", file=sys.stderr)
    return v8


def collect_catalyst_v8(lock_pkgs):
    lock = v8_lock(lock_pkgs)
    if not lock:
        return None
    locked_versions, unregistered = lock
    v8_meta = crates_io('v8')
    describe = v8_describer(v8_meta)
    v8 = {'locked_versions': locked_versions, 'unregistered': unregistered,
          'locked': describe(locked_versions[0] if locked_versions else None),
          'newest': describe(v8_meta['crate']['max_stable_version']), 'chrome': chrome_stable(),
          'chrome_gap_limit': CHROME_GAP_LIMIT}
    v8['findings'] = catalyst_v8_findings(v8)
    print(f"  catalyst v8: pinned {(v8['locked'] or {}).get('crate')} (M{(v8['locked'] or {}).get('milestone')}), "
          f"newest {(v8['newest'] or {}).get('crate')}, Chrome stable M{v8['chrome']['milestone']}: "
          f"{len(v8['findings'])} finding(s)", file=sys.stderr)
    return v8


def collect_v8_or_error(lock_pkgs, collect=None):
    """collect_v8 (or `collect`), or {'error': ...}: an outage must not cost the night its site and fork PR."""
    try:
        return (collect or collect_v8)(lock_pkgs)
    except Exception as error:
        print(f'  v8: {error!r}', file=sys.stderr)
        return {'error': repr(error)}


def v8_report_markdown(v8):
    def row(label, entry, note=''):
        if not entry:
            return f'| {label} | – | – | – | {note} |'
        return f"| {label} | {entry.get('crate', '–')} | {entry.get('v8') or '?'} | M{entry.get('milestone') or '?'} | {entry.get('date') or ''} {note}".rstrip() + ' |'
    chrome = v8['chrome']
    lines = ['The native scene runtime embeds V8 through the `v8` crate (rusty_v8), locked transitively by deno_core in '
             '`rendering/bevy-explorer/source/Cargo.lock`. RustSec has no V8 advisories, so this is the only check on its age.', '',
             '| | v8 crate | V8 | Chrome | released |', '| --- | --- | --- | --- | --- |',
             row('locked', v8['locked']),
             row(f"newest deno_core ({v8['deno_core']['version']}) accepts", v8['accepted'], f"(`{v8['deno_core']['v8_req'] or '?'}`)"),
             row('newest rusty_v8', v8['newest']),
             f"| Chrome stable | – | – | M{chrome['milestone']} | {chrome['date']} ({chrome['version']}) |", '']
    if v8['findings']:
        lines += ['Findings:', ''] + [f'- {f}' for f in v8['findings']]
    else:
        lines.append('No findings.')
    return '\n'.join(lines) + '\n'


def catalyst_v8_report_markdown(v8):
    def row(label, entry):
        if not entry:
            return f'| {label} | – | – | – | |'
        return f"| {label} | {entry['crate']} | {entry.get('v8') or '?'} | M{entry.get('milestone') or '?'} | {entry.get('date') or ''} |"
    chrome = v8['chrome']
    lines = ['The catalyst hosts SDK7 world scenes on V8 through a direct `v8` pin (rusty_v8) in '
             '`services/catalyst/Cargo.toml`, locked in `services/catalyst/Cargo.lock`. No deno_core bounds it, so any '
             'newer rusty_v8 is a bump to take.', '',
             '| | v8 crate | V8 | Chrome | released |', '| --- | --- | --- | --- | --- |',
             row('pinned', v8['locked']), row('newest rusty_v8', v8['newest']),
             f"| Chrome stable | – | – | M{chrome['milestone']} | {chrome['date']} ({chrome['version']}) |", '']
    if v8['findings']:
        lines += ['Findings:', ''] + [f'- {f}' for f in v8['findings']]
    else:
        lines.append('No findings.')
    return '\n'.join(lines) + '\n'


# data.json key, section title, renderer
V8_WORKSPACES = [('v8', 'Bevy Explorer scene runtime', v8_report_markdown),
                 ('catalyst_v8', 'Catalyst SDK7 scene host', catalyst_v8_report_markdown)]


def v8_report(body_path):
    data = json.loads(DATA.read_text())
    sections, findings, collected, errors = [], [], 0, 0
    for key, title, render in V8_WORKSPACES:
        v8 = data.get(key)
        if not v8:
            sections.append(f'## {title}\n\nNo v8 crate in the lock.\n')
            continue
        if 'error' in v8:
            # The other workspace's findings still count; this one keeps the issue open.
            errors += 1
            finding = f"unable to resolve the V8 freshness data: {v8['error']}"
            print(f'{title}: {finding}', file=sys.stderr)
            sections.append(f'## {title}\n\nFindings:\n\n- {finding}\n')
            findings.append((title, finding))
            continue
        collected += 1
        sections.append(f'## {title}\n\n' + render(v8))
        findings += [(title, f) for f in v8['findings']]
    if errors and not collected:
        # Nothing usable: leave the issue as it is and fail without touching the body.
        return 1
    text = '\n'.join(sections)
    print(text)
    if os.environ.get('GITHUB_ACTIONS'):
        for title, finding in findings:
            print(f'::warning title=V8 freshness ({title})::{finding}')
    if body_path:
        body = Path(body_path)
        if findings:
            body.write_text(text + '\nUpdated nightly by `.github/workflows/bevy-explorer-upstream.yml`.\n')
        else:
            body.unlink(missing_ok=True)
    return 0


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
    parser.add_argument('command', choices=['collect', 'v8-report'])
    parser.add_argument('--body', help='v8-report: write the issue body here when V8 is stale, remove it otherwise')
    args = parser.parse_args()
    if args.command == 'v8-report':
        return v8_report(args.body)
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
    print('v8', file=sys.stderr)
    data['v8'] = collect_v8_or_error(lock_pkgs)
    print('catalyst v8', file=sys.stderr)
    # Read inside the guard: a missing catalyst lock is a collect error, not a crash.
    data['catalyst_v8'] = collect_v8_or_error(CATALYST_LOCK, lambda path: collect_catalyst_v8(lock_packages(path.read_text())))
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
