#!/usr/bin/env python3
"""Decide which Creator Hub e2e suites a pull request requires, and whether its label runs them."""
import argparse
from dataclasses import dataclass, field
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SELECTION = ROOT / 'apps/creator-hub/e2e-selection.json'
STATUS_LIMIT = 140
ALL = frozenset({'*'})
SAFE_SPEC = re.compile(r'[\w./-]+')

_PATHS = importlib.util.spec_from_file_location('ci_paths', Path(__file__).with_name('ci_paths.py'))
paths = importlib.util.module_from_spec(_PATHS)
_PATHS.loader.exec_module(paths)


@dataclass
class Required:
    creator_hub: bool = False
    inspector: set = field(default_factory=set)
    offline: bool = False
    live: bool = False
    rows: list = field(default_factory=list)


def load_selection(path=SELECTION):
    return json.loads(Path(path).read_text())


def patterns(selection):
    """Every glob list in the selection file, named."""
    inspector = selection['inspector']
    return [('creator-hub', selection['creator-hub']), ('skip', selection['skip']),
            ('every-suite', selection['every-suite']), ('inspector.source', inspector['source']),
            ('inspector.all', inspector['all']), ('creator-hub-live', selection['creator-hub-live']),
            ('creator-hub-offline', selection['creator-hub-offline'])]


def matches(path, globs):
    return bool(paths.matching_paths([path], globs))


def e2e_importers(selection=None):
    """Map each file of the inspector e2e tree to the specs that import it, directly or through others."""
    e2e = ROOT / (selection or load_selection())['inspector']['e2e']
    package = e2e.parents[1]
    files = [p for p in e2e.rglob('*.ts') if p.is_file()]
    imports = {}
    for file in files:
        found = set()
        for target in re.findall(r"from '(\.{1,2}/[^']+)'", file.read_text()):
            resolved = (file.parent / target).resolve()
            for candidate in (resolved.with_suffix('.ts'), resolved / 'index.ts'):
                if candidate in files:
                    found.add(candidate)
        imports[file] = found
    importers = {}
    for spec in (f for f in files if f.name.endswith('.spec.ts')):
        name = spec.relative_to(package).as_posix()
        seen, stack = set(), [spec]
        while stack:
            current = stack.pop()
            if current not in seen:
                seen.add(current)
                stack.extend(imports[current])
        for reached in seen:
            importers.setdefault(reached.relative_to(ROOT).as_posix(), set()).add(name)
    return importers


def _inspector(path, selection, impact, importers):
    if path in importers:
        return set(importers[path]), 'e2e import'
    if path.startswith(selection['inspector']['e2e'] + '/'):
        return ALL, 'e2e file no spec imports'
    if matches(path, selection['inspector']['source']):
        if impact is None or path not in impact:
            return ALL, 'not in the impact map'
        specs = set(impact[path])
        known = {spec for names in importers.values() for spec in names}
        if not specs <= known:
            return ALL, 'impact map names a missing spec'
        return specs, 'impact map'
    if matches(path, selection['inspector']['all']):
        return ALL, 'inspector bundle input'
    return set(), ''


def resolve(changed, selection, impact, importers):
    """What the changed files require of each suite, with one summary row per file."""
    required = Required()
    for path in changed:
        if not matches(path, selection['creator-hub']):
            continue
        required.creator_hub = True
        if matches(path, selection['skip']):
            required.rows.append((path, 'no e2e'))
            continue
        outcomes = []
        if matches(path, selection['every-suite']):
            inspector, reason, offline, live = ALL, 'every suite', True, True
        else:
            inspector, reason = _inspector(path, selection, impact, importers)
            live = matches(path, selection['creator-hub-live'])
            offline = live or matches(path, selection['creator-hub-offline'])
        if inspector:
            outcomes.append(f"inspector {'all' if inspector == ALL else ', '.join(sorted(inspector))} ({reason})")
            required.inspector = ALL if ALL in (inspector, required.inspector) else required.inspector | inspector
        if offline:
            outcomes.append('creator-hub offline')
            required.offline = True
        if live:
            outcomes.append('creator-hub live')
            required.live = True
        required.rows.append((path, '; '.join(outcomes) or 'no e2e'))
    return required


def _suites(required):
    names = []
    if required.inspector:
        count = len(required.inspector)
        names.append('inspector' if required.inspector == ALL else f"inspector ({count} spec{'s' * (count > 1)})")
    names += ['Creator Hub offline'] * required.offline + ['Creator Hub live'] * required.live
    return ' + '.join(names)


def _spec_args(specs):
    if specs == ALL or not all(SAFE_SPEC.fullmatch(spec) for spec in specs):
        return ''
    return ' '.join(sorted(specs))


def plan(event, action, attempt, changed, labels, selection, impact, importers):
    """Outputs for the workflow: what to run and what the `Creator Hub e2e` status says."""
    label = selection['label']
    if event != 'pull_request':
        return {'creator-hub': 'true', 'required': 'true', 'run': 'true', 'inspector': 'true',
                'inspector-specs': '', 'creator-hub-offline': 'true', 'creator-hub-live': 'true',
                'remove-label': 'false', 'state': 'pending', 'description': 'Running every suite', 'rows': []}
    required = resolve(changed, selection, impact, importers)
    pushed = action in ('synchronize', 'reopened') and int(attempt) == 1
    labelled = label in labels and not pushed
    needed = bool(required.inspector or required.offline or required.live)
    run = needed and labelled
    if not needed:
        description = 'Not required for these changes'
    elif run:
        description = f'Running {_suites(required)}'
    else:
        description = f'Required: add label {label} to run {_suites(required)}'
    flag = lambda value: 'true' if value else 'false'
    return {
        'creator-hub': flag(required.creator_hub), 'required': flag(needed), 'run': flag(run),
        'inspector': flag(run and required.inspector),
        'inspector-specs': _spec_args(required.inspector),
        'creator-hub-offline': flag(run and required.offline), 'creator-hub-live': flag(run and required.live),
        'remove-label': flag(pushed and label in labels),
        'state': 'pending' if needed else 'success', 'description': description[:STATUS_LIMIT],
        'rows': required.rows,
    }


def _changed(base, head):
    out = subprocess.check_output(['git', 'diff', '--name-only', '--no-renames', '-z', f'{base}...{head}'],
                                  cwd=ROOT, text=True)
    return [path for path in out.split('\0') if path]


def _labels(repo, number):
    out = subprocess.check_output(['gh', 'api', f'repos/{repo}/issues/{number}/labels', '--jq', '.[].name'],
                                  text=True)
    return out.split()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['plan'])
    parser.add_argument('--impact-map', type=Path, help='JSON map of source file to the specs that execute it')
    args = parser.parse_args()
    env = os.environ
    event = env['GITHUB_EVENT_NAME']
    pull_request = event == 'pull_request'
    impact = json.loads(args.impact_map.read_text()) if args.impact_map and args.impact_map.is_file() else None
    selection = load_selection()
    out = plan(event, env.get('ACTION', ''), env.get('GITHUB_RUN_ATTEMPT', '1'),
               _changed(env['BASE'], env['HEAD']) if pull_request else [],
               _labels(env['GITHUB_REPOSITORY'], env['PR']) if pull_request else [],
               selection, impact, e2e_importers(selection))
    rows = out.pop('rows')
    with open(env['GITHUB_OUTPUT'], 'a') as output:
        output.writelines(f'{key}={value}\n' for key, value in out.items())
    if pull_request and env.get('GITHUB_STEP_SUMMARY'):
        with open(env['GITHUB_STEP_SUMMARY'], 'a') as summary:
            summary.write(f"### Creator Hub e2e\n\n{out['description']}\n\n")
            if rows:
                summary.write('| changed file | requires |\n|---|---|\n')
                summary.writelines(f'| `{path}` | {outcome} |\n' for path, outcome in rows)
    print(out['description'])


if __name__ == '__main__':
    main()
