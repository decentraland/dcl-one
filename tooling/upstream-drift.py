#!/usr/bin/env python3
"""The pinned sources of the Explorer against their upstreams.

The pinned sources are every rendering/bevy-explorer/patches/<name>/source.toml (an upstream commit
and the patch series this repository keeps on it) and every other git source of
rendering/bevy-explorer/source/Cargo.lock. The lock takes each patched source from this repository:
that entry has no upstream of its own, and the source.toml names the one to watch. Nothing here
writes a tracked file.

`check [NAME ...]` asks GitHub, per source (every source, or the named ones), how many commits the
watched ref has past the pinned commit: the `ref` of source.toml, the branch or tag the lock names,
or the default branch for a source pinned by revision alone. A tag does not move, so for a tag it
also counts the newer tags of the same naming scheme. It prints one line per source. It fails when
a source cannot be checked, and when a source that names its ref is not pinned on it: the pinned
commit has commits the branch lacks, or is not the commit the tag names.

`collect` gathers, into .cache/upstream-drift/data.json:
  - every pinned source: the patches it carries, the upstream commits, releases and tags since the
    pinned commit, and how far the watched ref moved
  - RustSec advisories against every crate version in the lock (rustsec/advisory-db)
  - the Explorer's direct crates.io dependencies against the newest release on crates.io
  - the lock's shape: sources, duplicated crates
  - the age of the scene runtime's V8 (the transitive `v8` crate) against deno_core, rusty_v8 and Chrome
  - the age of the catalyst's V8 (its direct `v8` pin, services/catalyst/Cargo.lock) against rusty_v8 and Chrome
`collect` caches every GitHub and crates.io response under .cache/upstream-drift/api so reruns are
cheap; `check` always asks. tooling/upstream-drift-site.py turns the data, the research and the
review into the site.

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
# Where Cargo takes the patched sources from: MIRROR of tooling/patched-sources.py.
REPOSITORY = 'https://github.com/decentraland/dcl-one'
USER_AGENT = f'dcl-one upstream-drift ({REPOSITORY})'
EXPLORER = 'decentraland/bevy-explorer'


# ---------------------------------------------------------------- cached fetchers

def cached(key, fetch):
    API_CACHE.mkdir(parents=True, exist_ok=True)
    path = API_CACHE / (hashlib.sha256(key.encode()).hexdigest()[:24] + '.json')
    if path.exists():
        return json.loads(path.read_text(encoding='utf-8'))
    value = fetch()
    if value is not None:  # a 404 is not cached: a later run may succeed (a new tag, a fixed path)
        path.write_text(json.dumps(value), encoding='utf-8')
    return value


def gh_api(path):
    result = subprocess.run(['gh', 'api', path], capture_output=True, text=True)
    if result.returncode:
        if 'HTTP 404' in result.stderr or 'Not Found' in result.stderr:
            return None
        raise RuntimeError(f'gh api {path}: {result.stderr.strip()[:300]}')
    return json.loads(result.stdout)


def gh(path):
    return cached('gh:' + path, lambda: gh_api(path))


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


# ---------------------------------------------------------------- pinned sources

def lock_packages(cargo_lock_text):
    packages = []
    for block in cargo_lock_text.split('[[package]]')[1:]:
        fields = dict(re.findall(r'^(\w+) = "([^"]*)"$', block, re.M))
        packages.append({'name': fields['name'], 'version': fields['version'], 'source': fields.get('source', '')})
    return packages


def github_repo(url):
    match = re.fullmatch(r'https://github\.com/([^/]+)/([^/]+?)(?:\.git)?', url)
    return match.groups() if match else None


def manifests(source_dir):
    return [source_dir / 'Cargo.toml', *sorted((source_dir / 'crates').glob('*/Cargo.toml'))]


def patch_row(path):
    """The origin commit a patch of a series names in its header. A patch written in this repository
    (tooling/patched-sources.py export) names the all-zero commit and no origin: it is `local`, with
    no commit to show."""
    with path.open('rb') as handle:
        head = handle.read(4096).decode('utf-8', 'replace').split('\n\n')[0]
    fields = dict(re.findall(r'^(From|Subject:|Origin:|Date:) (.*?)\r?$', head, re.M))
    origin = github_repo(fields.get('Origin:', '').partition('/commit/')[0])
    sha = fields.get('From', '').strip('0') and fields['From']
    return {'sha': sha, 'local': not sha, 'url': fields.get('Origin:', '') if sha else '', 'file': path.name,
            'date': fields.get('Date:', '')[:10], 'title': fields.get('Subject:', path.stem),
            'author': '/'.join(origin) if origin else ''}


def patched_sources(patches_dir):
    """Every <name>/source.toml: the upstream commit this repository patches, with its series."""
    sources = []
    for path in sorted(patches_dir.glob('*/source.toml')):
        pin = tomllib.loads(path.read_text(encoding='utf-8'))
        sources.append({'name': pin['name'], 'upstream': pin['upstream'], 'commit': pin['commit'], 'ref': pin.get('ref') or None,
                        'patches': [patch_row(p) for p in sorted((path.parent / 'series').glob('*.patch'))]})
    return sources


def lock_git_sources(cargo_lock_text):
    """Every git source of a Cargo.lock with the crates it supplies. `ref` is the branch or tag
    the manifest named; a source named by revision or by nothing watches the default branch."""
    grouped = {}
    for package in lock_packages(cargo_lock_text):
        if not package['source'].startswith('git+'):
            continue
        url, _, commit = package['source'][4:].partition('#')
        url, _, query = url.partition('?')
        params = dict(urllib.parse.parse_qsl(query))
        grouped.setdefault((url, params.get('branch') or params.get('tag'), commit), []).append((package['name'], package['version']))
    return [{'name': url.rstrip('/').rsplit('/', 1)[-1].removesuffix('.git'), 'upstream': url, 'commit': commit, 'ref': ref, 'crates': sorted(crates)}
            for (url, ref, commit), crates in sorted(grouped.items(), key=lambda item: (item[0][0], item[0][2]))]


def harvest_reasons(manifest_text, urls):
    """The `#` comment block right above the first manifest line naming each git url."""
    lines = manifest_text.splitlines()
    reasons = {}
    for url in urls:
        for index, line in enumerate(lines):
            # the url followed by its closing quote: x/gltf must not match x/gltf-json
            if (url + '"') not in line or line.lstrip().startswith('#'):
                continue
            comments = []
            cursor = index - 1
            while cursor >= 0 and lines[cursor].lstrip().startswith('#'):
                comments.insert(0, lines[cursor].lstrip('# ').rstrip())
                cursor -= 1
            if comments:
                reasons[url] = ' '.join(comments)
            break
    return reasons


def canonical(url):
    """A git URL without what GitHub does not read, as `pins` of tooling/patched-sources.py compares it."""
    url = url.lower()
    while url.endswith(('/', '.git')):
        url = url.removesuffix('/').removesuffix('.git')
    return url


def pinned_sources(pin=PIN):
    """The patched sources, then the other git sources the lock keeps with the manifests' reasons."""
    source_dir = pin / 'source'
    locked = [source for source in lock_git_sources((source_dir / 'Cargo.lock').read_text(encoding='utf-8'))
              if canonical(source['upstream']) != canonical(REPOSITORY)]
    reasons = harvest_reasons('\n'.join(m.read_text(encoding='utf-8') for m in manifests(source_dir) if m.exists()),
                              {s['upstream'] for s in locked})
    for source in locked:
        source['reason'] = reasons.get(source['upstream'])
    return patched_sources(pin / 'patches') + locked


# ---------------------------------------------------------------- check

def scheme_tags(repo, ref, api):
    """The repository's tags in the naming scheme of `ref`; a ref that is not among them is a branch.
    GitHub answers with every matching ref at once."""
    prefix = tag_prefix(ref)
    rows = api(f'repos/{repo}/git/matching-refs/tags/{urllib.parse.quote(prefix)}') or []
    return [name for name in (row['ref'].removeprefix('refs/tags/') for row in rows) if tag_prefix(name) == prefix]


def newer_tags(tags, ref):
    """The tags past `ref`, oldest first. Pre-releases count only when `ref` is one."""
    newer = [tag for tag in tags if tag_order(tag) > tag_order(ref) and (tag_order(tag)[1] or not tag_order(ref)[1])]
    return sorted(newer, key=tag_order)


def named_ref(repo, ref, api):
    """('tag' or 'branch', commit id) of the tag or branch with exactly this name, or None. GitHub
    compare also resolves a pull request head and a commit id, and those claim nothing about a pin."""
    for kind, space in (('tag', 'tags'), ('branch', 'heads')):
        rows = api(f'repos/{repo}/git/matching-refs/{space}/{urllib.parse.quote(ref)}') or []
        target = next((row['object'] for row in rows if row['ref'] == f'refs/{space}/{ref}'), None)
        while target and target['type'] == 'tag':  # an annotated tag names its commit through a tag object
            target = (api(f'repos/{repo}/git/tags/{target["sha"]}') or {}).get('object')
        if target:
            return kind, target['sha']
    return None


def check_source(source, api=gh_api):
    """How far the watched ref is from the pinned commit; for a tag, also the newer tags of its
    scheme. None when the repository has no tag or branch of that name, or GitHub cannot compare."""
    repo = github_repo(source['upstream'])
    if repo is None:
        return None
    repo = '/'.join(repo)
    ref = source['ref'] or (api(f'repos/{repo}') or {}).get('default_branch')
    kind, head = (named_ref(repo, ref, api) or (None, None)) if source['ref'] else ('branch', ref)
    compare = api(f'repos/{repo}/compare/{source["commit"]}...{urllib.parse.quote(head, safe="")}') if head else None
    if compare is None:
        return None
    tags = scheme_tags(repo, ref, api) if kind == 'tag' else []
    return {'ref': ref, 'ahead': compare['ahead_by'], 'behind': compare['behind_by'], 'named': bool(source['ref']),
            'tag': kind == 'tag', 'newer_tags': newer_tags(tags, ref)}


def off_ref(result):
    """Why the pinned commit is not on the ref the source names, or None. A source that names no
    ref watches the default branch, which claims nothing about the pin."""
    if not result['named']:
        return None
    if result['behind']:
        return f'the pin has {result["behind"]} commit(s) {result["ref"]} lacks'
    if result['tag'] and result['ahead']:
        return f'the tag moved, or never named the pin: it is {result["ahead"]} commit(s) past it'
    return None


def check_report(sources, api=gh_api):
    """(text, ok): one line per source; ok is false when a source could not be checked or is not
    pinned on the ref it names."""
    lines, moved, tagged, off, failed = [], 0, 0, 0, 0
    for source in sources:
        repo, pin = github_repo(source['upstream']), source['commit'][:10]
        where = '/'.join(repo) if repo else source['upstream']
        result = check_source(source, api)
        if result is None:
            failed += 1
            why = f'GitHub cannot compare {"the tag or branch " + source["ref"] if source["ref"] else "the default branch"} with {pin}' if repo else 'not a GitHub repository'
            lines.append(f'- {source["name"]}: {where} NOT CHECKED: {why}')
            continue
        moved += result['ahead'] > 0
        tagged += bool(result['newer_tags'])
        line = f'- {source["name"]}: {where} {"tag " if result["tag"] else ""}{result["ref"]} '
        line += f'is {result["ahead"]} commit(s) past {pin}' if result['ahead'] else f'has not moved past {pin}'
        wrong = off_ref(result)
        if wrong:
            off += 1
            line += f'; PIN NOT ON ITS REF: {wrong}'
        elif result['behind']:
            line += f' (the pin has {result["behind"]} commit(s) the ref lacks)'
        if 'patches' in source:
            local = sum(1 for patch in source['patches'] if patch.get('local'))
            line += f'; {len(source["patches"])} patch(es)' + (f', {local} local' if local else '')
        if result['newer_tags']:
            line += f'; {len(result["newer_tags"])} newer tag(s), newest {result["newer_tags"][-1]}'
        lines.append(line)
    head = (f'Pinned sources of the Explorer: {len(sources)}; upstream moved past the pin: {moved}; a newer tag: {tagged}; '
            f'pin not on its ref: {off}; not checked: {failed}.')
    return '\n'.join([head, '', *lines]) + '\n', not failed and not off


# ---------------------------------------------------------------- drift map


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


def version_key(tag):
    nums = re.findall(r'\d+', tag)
    return tuple(int(n) for n in nums[:4]) + (0,) * (4 - len(nums[:4]))


def tag_order(tag):
    """Sort key of a tag within its scheme: (version, 1 for a release and 0 for a pre-release, the
    pre-release label, its numbers), so 1.0.0 follows 1.0.0-rc.2, which follows 1.0.0-beta.9."""
    version, suffix = re.match(r'([0-9.]*)(.*)', tag[len(tag_prefix(tag)):], re.S).groups()
    numbers = [int(n) for n in version.split('.') if n][:4]
    return (tuple(numbers) + (0,) * (4 - len(numbers)), int(not suffix),
            ''.join(re.findall(r'[a-z]+', suffix.lower())), tuple(int(n) for n in re.findall(r'\d+', suffix)))


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


def collect_sources(sources):
    """One entry per pinned source in the shape the site reads: `rev` is the pinned commit, `branch`
    the watched ref, `carried` the patch series (patched sources only), `crates` what the lock takes
    from the source (lock sources only), `upstream_since_pin` what the default branch gained,
    `branch_since_pin` how far the watched ref moved, and for a ref that is a tag `base` and
    `newer_tags`, the tags of its scheme past it."""
    entries = []
    for s in sources:
        url, rev, ref = s['upstream'], s['commit'], s['ref']
        repo = github_repo(url)
        entry = {'name': s['name'], 'url': url, 'repo': '/'.join(repo) if repo else None, 'branch': ref, 'rev': rev,
                 'reason': s.get('reason')}
        if 'crates' in s:
            entry['crates'] = [{'name': name, 'version': version} for name, version in s['crates']]
        if 'patches' in s:
            entry['carried'] = {'commits': s['patches']}
        entries.append(entry)
        if repo is None:
            continue
        repo = entry['repo']
        info = gh(f'repos/{repo}') or {}
        entry.update({'description': info.get('description'), 'stars': info.get('stargazers_count'), 'archived': info.get('archived')})
        head = gh(f'repos/{repo}/commits/{rev}')
        entry['rev_date'] = head['commit']['committer']['date'][:10] if head else None
        entry['upstream_since_pin'] = compare_all(repo, rev, info.get('default_branch', 'main'), cap=1000)
        tags = scheme_tags(repo, ref, gh) if ref else []
        tag = ref if ref in tags else None
        if ref:
            entry['branch_since_pin'] = compare_all(repo, rev, urllib.parse.quote(ref, safe=''))
        if tag:
            entry['base'] = {'tag': tag, 'sha': rev, 'date': entry['rev_date']}
            entry['newer_tags'] = newer_tags(tags, tag)
        entry['releases_since_base'] = releases_since(repo, entry['rev_date'], tag)
        latest = gh(f'repos/{repo}/releases/latest')
        entry['parent_latest_release'] = {'tag': latest['tag_name'], 'date': latest['published_at'][:10], 'url': latest['html_url']} if latest else None
        print(f'  {repo}: patches={len(s.get("patches", []))} upstream_since_pin={(entry["upstream_since_pin"] or {}).get("total_commits")} '
              f'releases={len(entry["releases_since_base"])}', file=sys.stderr)
    return entries


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
    for mf in manifests(source_dir):
        d = tomllib.loads(mf.read_text(encoding='utf-8'))
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


def chrome_stable_or_none():
    """Chrome stable, or None: only the milestone gap needs it, so its outage keeps the crates.io comparisons."""
    try:
        return chrome_stable()
    except Exception as error:
        print(f'  chrome stable: {error!r}', file=sys.stderr)
        return None


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
    if not chrome:
        return ['Chrome stable could not be fetched, so the milestone gap is not checked']
    if chrome['milestone'] - locked['milestone'] >= CHROME_GAP_LIMIT:
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
    elif parse_version(locked['crate'])[0] < parse_version(accepted['crate'])[0]:
        # Patch releases inside a milestone carry V8's security fixes.
        findings.append(f"a newer v8 within deno_core's requirement: v8 {locked['crate']} (V8 {locked['v8']}) is locked while "
                        f"deno_core {v8['deno_core']['version']} accepts v8 {accepted['crate']} (V8 {accepted['v8']}); "
                        'a lock-only bump of v8 takes it')
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
          'newest': describe(v8_meta['crate']['max_stable_version']), 'chrome': chrome_stable_or_none(),
          'chrome_gap_limit': CHROME_GAP_LIMIT}
    v8['findings'] = v8_findings(v8)
    print(f"  v8: locked {(v8['locked'] or {}).get('crate')} (M{(v8['locked'] or {}).get('milestone')}), deno_core {deno_core} accepts "
          f"{(v8['accepted'] or {}).get('crate')}, newest {v8['newest']['crate']}, Chrome stable M{(v8['chrome'] or {}).get('milestone')}: "
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
          'newest': describe(v8_meta['crate']['max_stable_version']), 'chrome': chrome_stable_or_none(),
          'chrome_gap_limit': CHROME_GAP_LIMIT}
    v8['findings'] = catalyst_v8_findings(v8)
    print(f"  catalyst v8: pinned {(v8['locked'] or {}).get('crate')} (M{(v8['locked'] or {}).get('milestone')}), "
          f"newest {(v8['newest'] or {}).get('crate')}, Chrome stable M{(v8['chrome'] or {}).get('milestone')}: "
          f"{len(v8['findings'])} finding(s)", file=sys.stderr)
    return v8


def collect_v8_or_error(lock_pkgs, collect=None):
    """collect_v8 (or `collect`), or {'error': ...}: an outage must not cost the night its site and fork PR."""
    try:
        return (collect or collect_v8)(lock_pkgs)
    except Exception as error:
        print(f'  v8: {error!r}', file=sys.stderr)
        return {'error': repr(error)}


def chrome_row(chrome):
    if not chrome:
        return '| Chrome stable | – | – | ? | unavailable |'
    return f"| Chrome stable | – | – | M{chrome['milestone']} | {chrome['date']} ({chrome['version']}) |"


def v8_report_markdown(v8):
    def row(label, entry, note=''):
        if not entry:
            return f'| {label} | – | – | – | {note} |'
        return f"| {label} | {entry.get('crate', '–')} | {entry.get('v8') or '?'} | M{entry.get('milestone') or '?'} | {entry.get('date') or ''} {note}".rstrip() + ' |'
    lines = ['The native scene runtime embeds V8 through the `v8` crate (rusty_v8), locked transitively by deno_core in '
             '`rendering/bevy-explorer/source/Cargo.lock`. RustSec has no V8 advisories, so this is the only check on its age.', '',
             '| | v8 crate | V8 | Chrome | released |', '| --- | --- | --- | --- | --- |',
             row('locked', v8['locked']),
             row(f"newest deno_core ({v8['deno_core']['version']}) accepts", v8['accepted'], f"(`{v8['deno_core']['v8_req'] or '?'}`)"),
             row('newest rusty_v8', v8['newest']), chrome_row(v8['chrome']), '']
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
    lines = ['The catalyst hosts SDK7 world scenes on V8 through a direct `v8` pin (rusty_v8) in '
             '`services/catalyst/Cargo.toml`, locked in `services/catalyst/Cargo.lock`. No deno_core bounds it, so any '
             'newer rusty_v8 is a bump to take.', '',
             '| | v8 crate | V8 | Chrome | released |', '| --- | --- | --- | --- | --- |',
             row('pinned', v8['locked']), row('newest rusty_v8', v8['newest']), chrome_row(v8['chrome']), '']
    if v8['findings']:
        lines += ['Findings:', ''] + [f'- {f}' for f in v8['findings']]
    else:
        lines.append('No findings.')
    return '\n'.join(lines) + '\n'


# data.json key, section title, renderer, whether the lock must have a v8 (the Explorer always embeds V8)
V8_WORKSPACES = [('v8', 'Bevy Explorer scene runtime', v8_report_markdown, True),
                 ('catalyst_v8', 'Catalyst SDK7 scene host', catalyst_v8_report_markdown, False)]


def v8_report(body_path):
    data = json.loads(DATA.read_text(encoding='utf-8'))
    sections, findings, collected, errors = [], [], 0, 0
    for key, title, render, required in V8_WORKSPACES:
        v8 = data.get(key)
        if not v8 and required:
            # A missing v8 is a lock or collector problem, not a current V8.
            collected += 1
            finding = 'no v8 crate in the lock, so its V8 is not checked'
            sections.append(f'## {title}\n\nFindings:\n\n- {finding}\n')
            findings.append((title, finding))
            continue
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
            body.write_text(text + '\nUpdated nightly by `.github/workflows/bevy-explorer-upstream.yml`.\n', encoding='utf-8')
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
    parser.add_argument('command', choices=['check', 'collect', 'v8-report'])
    parser.add_argument('names', nargs='*', metavar='NAME', help='check: only these sources (default: every source)')
    parser.add_argument('--body', help='v8-report: write the issue body here when V8 is stale, remove it otherwise')
    args = parser.parse_args()
    if args.command == 'v8-report':
        if args.names:
            parser.error('v8-report takes no source names')
        return v8_report(args.body)
    sources = pinned_sources()
    if args.command == 'check':
        unknown = sorted(set(args.names) - {source['name'] for source in sources})
        if unknown:
            parser.error(f'no pinned source named: {", ".join(unknown)}')
        text, ok = check_report([source for source in sources if not args.names or source['name'] in args.names])
        sys.stdout.write(text)
        return 0 if ok else 1
    if args.names:
        parser.error('collect takes no source names')
    source_dir = PIN / 'source'
    revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    lock_pkgs = lock_packages((source_dir / 'Cargo.lock').read_text(encoding='utf-8'))
    explorer = gh(f'repos/{EXPLORER}')
    data = {'generated': datetime.date.today().isoformat(), 'explorer': {'repo': EXPLORER, 'revision': revision,
            'description': explorer.get('description') if explorer else None},
            'lock': lock_shape(lock_pkgs)}
    print('sources', file=sys.stderr)
    data['sources'] = collect_sources(sources)
    print('advisories', file=sys.stderr)
    data['advisories'] = collect_advisories(lock_pkgs)
    print('registry drift', file=sys.stderr)
    data['registry'] = collect_registry_drift(source_dir, lock_pkgs)
    print('v8', file=sys.stderr)
    data['v8'] = collect_v8_or_error(lock_pkgs)
    print('catalyst v8', file=sys.stderr)
    # Read inside the guard: a missing catalyst lock is a collect error, not a crash.
    data['catalyst_v8'] = collect_v8_or_error(CATALYST_LOCK, lambda path: collect_catalyst_v8(lock_packages(path.read_text(encoding='utf-8'))))
    CACHE.mkdir(parents=True, exist_ok=True)
    DATA.write_text(json.dumps(data, indent=1), encoding='utf-8')
    # one file per source for the research agents, with the advisories against its crates
    slices = CACHE / 'sources'
    slices.mkdir(exist_ok=True)
    for src in data['sources']:
        names = {c['name'] for c in src.get('crates', [])}
        payload = {**src, 'advisories': [f for f in data['advisories']['findings'] if f['crate'] in names],
                   'slug': re.sub(r'[^a-z0-9]+', '-', src['url'].lower().removeprefix('https://github.com/')).strip('-')}
        (slices / f"{payload['slug']}.json").write_text(json.dumps(payload, indent=1), encoding='utf-8')
    print(f'wrote {len(data["sources"])} source slices to {slices.relative_to(ROOT)}')
    print(f'wrote {DATA.relative_to(ROOT)} ({DATA.stat().st_size // 1024} KiB)')
    return 0


if __name__ == '__main__':
    sys.exit(main())
