#!/usr/bin/env python3
"""Check mechanically detectable violations in changed web UI files."""
import argparse
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EXTENSIONS = ('.ts', '.tsx', '.css')
STYLED_IMPORT = re.compile(r"(?:from\s*|import\s*|require\s*\(\s*)['\"](?:styled-components|@emotion/(?:styled|react))['\"]|(?:import|export)\s*\{[^}]*\bstyled\b[^}]*\}\s*from\s*['\"](?:@mui/material(?:/styles)?|@mui/system|decentraland-ui2)['\"]")


def is_ui_source(path):
    return path.endswith(EXTENSIONS) and path.startswith(('apps/', 'library/ui/', 'rendering/bevy-explorer/source/react-web/'))


def violations_in_added_lines(line):
    return bool(STYLED_IMPORT.search(line))


def is_component_stylesheet(path, paths):
    if not path.endswith('.css') or path.endswith('.module.css'):
        return False
    base = path[:-4].casefold()
    normalized = {name.casefold() for name in paths}
    return any(base + extension in normalized for extension in ('.tsx', '.jsx'))


def git(*args):
    return subprocess.run(['git', *args], cwd=ROOT, check=True, capture_output=True, text=True).stdout


def check(base='HEAD'):
    tracked = set(git('ls-files', '-co', '--exclude-standard').splitlines())
    changed = set(git('diff', '--name-only', '--diff-filter=ACMR', base).splitlines())
    untracked = set(git('ls-files', '--others', '--exclude-standard').splitlines())
    changed |= untracked
    added = set(git('diff', '--name-only', '--diff-filter=A', base).splitlines()) | untracked
    issues = []
    for name in sorted(added):
        if is_ui_source(name) and is_component_stylesheet(name, tracked):
            issues.append(f'{name}: new component styles must use .module.css')
    for name in sorted(changed):
        if not is_ui_source(name) or not name.endswith(('.ts', '.tsx')):
            continue
        if name in untracked:
            lines = (ROOT / name).read_text(encoding='utf-8').splitlines()
        else:
            diff = git('diff', '--no-ext-diff', '--unified=0', base, '--', name)
            lines = [line[1:] for line in diff.splitlines() if line.startswith('+') and not line.startswith('+++')]
        for line in lines:
            if violations_in_added_lines(line):
                issues.append(f'{name}: new CSS-in-JS imports are not allowed')
    return issues


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base', default='HEAD', help='Compare changes with this Git revision')
    args = parser.parse_args()
    try:
        issues = check(args.base)
    except (OSError, subprocess.CalledProcessError) as error:
        print(f'UI standards check could not run: {error}', file=sys.stderr)
        return 2
    if issues:
        print('\n'.join(issues), file=sys.stderr)
        return 1
    print('UI standards: checked changed files for CSS-in-JS imports and new component stylesheets')
    return 0


if __name__ == '__main__':
    sys.exit(main())
