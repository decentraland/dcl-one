#!/usr/bin/env python3
"""Check mechanically detectable violations in changed web UI files.

Best effort: a pass means no CSS-in-JS import or new plain component
stylesheet was detected in the changed files, not that they follow every rule
in docs/ui-standards.md.
"""
import argparse
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT_EXTENSIONS = ('.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs')
EXTENSIONS = SCRIPT_EXTENSIONS + ('.css',)
COMPONENT_EXTENSIONS = ('.tsx', '.jsx')
UI_ROOTS = ('apps/', 'library/ui/', 'rendering/bevy-explorer/source/react-web/')
# Under apps/ but not web UI: the scene SDK toolchain and the Explorer build tooling.
NON_WEB_ROOTS = ('apps/dcl-scene-cli/', 'apps/bevy-explorer/')
ZERO_SHA = re.compile(r'^0+$')
COMMIT_SHA = re.compile(r'^[0-9a-f]{40}$')
COMMENTS = re.compile(r'/\*.*?\*/|(?<![:\'"])//[^\n]*', re.S)
# An import or require at the start of a statement, possibly spanning lines.
FORBIDDEN_IMPORT = re.compile(
    r"(?:^|;)\s*(?:"
    r"(?:import|export)\b[^;]*?\bfrom\s*['\"](?:styled-components|@emotion/(?:styled|react|css))['\"]"
    r"|import\s*['\"](?:styled-components|@emotion/(?:styled|react|css))['\"]"
    r"|(?:import|export)\s*\{[^}]*\bstyled\b[^}]*\}\s*from\s*['\"](?:@mui/material(?:/styles)?|@mui/system|decentraland-ui2)['\"]"
    r"|(?:const|let|var)\b[^;=]*=\s*require\s*\(\s*['\"](?:styled-components|@emotion/(?:styled|react|css))['\"]"
    r")",
    re.M | re.S,
)


def is_ui_source(path):
    return path.endswith(EXTENSIONS) and path.startswith(UI_ROOTS) and not path.startswith(NON_WEB_ROOTS)


def forbidden_imports(text):
    return bool(FORBIDDEN_IMPORT.search(COMMENTS.sub('', text)))


def is_component_stylesheet(path, paths):
    if not path.endswith('.css') or path.endswith('.module.css'):
        return False
    base = path[:-4].casefold()
    normalized = {name.casefold() for name in paths}
    return any(base + extension in normalized for extension in COMPONENT_EXTENSIONS)


def added_lines(diff):
    """Return the added lines of a unified diff, skipping the file headers."""
    lines = []
    in_hunk = False
    for line in diff.splitlines():
        if line.startswith('@@'):
            in_hunk = True
        elif line.startswith('diff --git'):
            in_hunk = False
        elif in_hunk and line.startswith('+'):
            lines.append(line[1:])
    return lines


def git(*args):
    return subprocess.run(['git', '-c', 'core.quotePath=false', *args], cwd=ROOT, check=True,
                          capture_output=True, text=True).stdout


def names(command, *args):
    return {name for name in git(*command, '-z', *args).split('\0') if name}


def verify(revision):
    try:
        git('rev-parse', '--verify', '--quiet', '--end-of-options', f'{revision}^{{commit}}')
        return True
    except subprocess.CalledProcessError:
        return False


def default_base():
    """The merge base with origin/main, so local runs see committed branch changes as CI does."""
    try:
        return git('merge-base', 'HEAD', 'origin/main').strip()
    except subprocess.CalledProcessError:
        return 'HEAD'


def resolve_base(base):
    """Resolve the base revision.

    A push event reports an all-zero SHA for a new ref, and a force push can
    report a commit that is no longer present; both fall back to HEAD~1, or to
    HEAD in a repository with a single commit. Any other unresolvable base is
    an error rather than a silent pass.
    """
    if not ZERO_SHA.match(base):
        if verify(base):
            return base
        if not COMMIT_SHA.match(base):
            raise ValueError(f'unknown base revision {base!r}')
    fallback = 'HEAD~1' if verify('HEAD~1') else 'HEAD'
    print(f'UI standards: base {base!r} is not a reachable commit; comparing with {fallback}', file=sys.stderr)
    return fallback


def check(base='HEAD', staged=False):
    """List violations in changes from base to the working tree, or to the index when staged."""
    base = resolve_base(base)
    if staged:
        diff = ('diff', '--cached')
        tracked = names(('ls-files',))
        untracked = set()
    else:
        diff = ('diff',)
        tracked = names(('ls-files',), '-co', '--exclude-standard')
        untracked = names(('ls-files',), '--others', '--exclude-standard')
    changed = names(diff, '--name-only', '--diff-filter=ACMR', '--end-of-options', base, '--') | untracked
    added = names(diff, '--name-only', '--diff-filter=AR', '--end-of-options', base, '--') | untracked
    issues = []
    for name in sorted(added):
        if is_ui_source(name) and is_component_stylesheet(name, tracked):
            issues.append(f'{name}: new component styles must use .module.css')
    for name in sorted(changed):
        if not is_ui_source(name) or not name.endswith(SCRIPT_EXTENSIONS):
            continue
        if name in untracked:
            text = (ROOT / name).read_text(encoding='utf-8')
        else:
            text = '\n'.join(added_lines(git(*diff, '--no-ext-diff', '--unified=0', '--end-of-options', base, '--', name)))
        if forbidden_imports(text):
            issues.append(f'{name}: new CSS-in-JS imports are not allowed')
    return issues


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base', help='Compare changes with this Git revision (default: merge base with origin/main)')
    parser.add_argument('--staged', action='store_true', help='Check the index instead of the working tree')
    args = parser.parse_args()
    try:
        issues = check(args.base or default_base(), staged=args.staged)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f'UI standards check could not run: {error}', file=sys.stderr)
        return 2
    if issues:
        print('\n'.join(issues), file=sys.stderr)
        return 1
    print('UI standards: checked changed files for CSS-in-JS imports and new component stylesheets')
    return 0


if __name__ == '__main__':
    sys.exit(main())
