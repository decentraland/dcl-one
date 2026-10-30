#!/usr/bin/env python3
"""Check mechanically detectable violations in changed web UI files.

Best effort: a pass means no CSS-in-JS import or new same-name component
stylesheet was detected in the changed files, not that they follow every rule
in docs/ui-standards.md.
"""
import argparse
import re
import subprocess
import sys
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parent.parent
SCRIPT_EXTENSIONS = ('.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs')
EXTENSIONS = SCRIPT_EXTENSIONS + ('.css',)
COMPONENT_EXTENSIONS = ('.tsx', '.jsx')
UI_ROOTS = ('apps/', 'library/ui/', 'rendering/bevy-explorer/source/react-web/')
# Under apps/ but not web UI: the scene SDK toolchain's scripts, scene templates and fixtures.
NON_WEB_ROOTS = ('apps/dcl-scene-cli/scripts/', 'apps/dcl-scene-cli/src/templates/', 'apps/dcl-scene-cli/testdata/')
# Application entry stylesheets (App.css, index.css, main.css) are global by design.
ENTRY_STYLESHEETS = ('app', 'index', 'main')
ZERO_SHA = re.compile(r'^0+$')
COMMIT_SHA = re.compile(r'^[0-9a-f]{40}$')
CSS_IN_JS_MODULE = re.compile(r'^(?:styled-components|@emotion/(?:styled|react|css)|@mui/(?:material/styles|system)/styled)(?:/.*)?$')
MUI_MODULE = re.compile(r'^(?:@mui/material(?:/styles)?|@mui/system|decentraland-ui2)$')
TOKEN = re.compile(
    r"(?P<comment>//[^\n]*|/\*.*?(?:\*/|\Z))"
    r"|(?P<string>'(?:\\.|[^'\\\n])*'|\"(?:\\.|[^\"\\\n])*\"|`(?:\\.|[^`\\])*`)"
    r"|(?P<word>[A-Za-z_$][\w$]*)"
    r"|(?P<punct>\S)",
    re.S,
)


def is_ui_source(path):
    return path.endswith(EXTENSIONS) and path.startswith(UI_ROOTS) and not path.startswith(NON_WEB_ROOTS)


def tokens(text):
    """Split source into words, punctuation and string literals, dropping comments."""
    result = []
    for match in TOKEN.finditer(text):
        kind = match.lastgroup
        if kind == 'comment':
            continue
        value = match.group()
        result.append((kind, value[1:-1] if kind == 'string' else value))
    return result


def forbidden_imports(text):
    items = tokens(text)
    for index, (kind, value) in enumerate(items):
        if kind != 'word' or value not in ('import', 'export', 'require'):
            continue
        if index and items[index - 1] == ('punct', '.'):
            continue
        following = items[index + 1:]
        if following and following[0] == ('punct', '('):
            if len(following) > 1 and following[1][0] == 'string' and CSS_IN_JS_MODULE.match(following[1][1]):
                return True
            continue
        if value == 'require':
            continue
        # A module specifier is the first string, right after the keyword or after `from`.
        names = set()
        previous = (kind, value)
        for next_kind, next_value in following:
            if next_kind == 'string':
                is_specifier = previous == ('word', 'from') or (previous == ('word', 'import') and not names)
                if is_specifier and (CSS_IN_JS_MODULE.match(next_value)
                                     or ('styled' in names and MUI_MODULE.match(next_value))):
                    return True
                break
            if next_kind == 'punct' and next_value == ';':
                break
            if next_kind == 'word':
                names.add(next_value)
            previous = (next_kind, next_value)
    return False


def mentions_css_in_js(text):
    """Whether added text could belong to a CSS-in-JS import, so the whole file needs a scan."""
    return any((kind == 'word' and value == 'styled') or (kind == 'string' and CSS_IN_JS_MODULE.match(value))
               for kind, value in tokens(text))


def is_component_stylesheet(path, paths):
    if not path.endswith('.css') or path.endswith('.module.css'):
        return False
    if PurePosixPath(path).stem.casefold() in ENTRY_STYLESHEETS:
        return False
    base = path[:-4].casefold()
    normalized = {name.casefold() for name in paths}
    return any(base + extension in normalized for extension in COMPONENT_EXTENSIONS)


def is_plain_stylesheet(path):
    return path.endswith('.css') and not path.endswith('.module.css')


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


def new_files(diff, base):
    """Paths added since base, plus renames whose old path was not already a plain stylesheet."""
    fields = git(*diff, '--name-status', '-z', '-M', '--diff-filter=AR', '--end-of-options', base, '--').split('\0')
    result = set()
    index = 0
    while index < len(fields) and fields[index]:
        status = fields[index]
        if status.startswith('R'):
            old, new = fields[index + 1], fields[index + 2]
            if not is_plain_stylesheet(old):
                result.add(new)
            index += 3
        else:
            result.add(fields[index + 1])
            index += 2
    return result


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
        print('UI standards: origin/main is not available; checking uncommitted changes only (pass --base)',
              file=sys.stderr)
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

    def current(name):
        return git('show', f':{name}') if staged else (ROOT / name).read_text(encoding='utf-8')

    changed = names(diff, '--name-only', '--diff-filter=ACMR', '--end-of-options', base, '--') | untracked
    added = new_files(diff, base) | untracked
    issues = []
    for name in sorted(added):
        if is_ui_source(name) and is_component_stylesheet(name, tracked):
            issues.append(f'{name}: new component styles must use .module.css')
    for name in sorted(changed):
        if not is_ui_source(name) or not name.endswith(SCRIPT_EXTENSIONS):
            continue
        if name in untracked:
            text = current(name)
        else:
            text = '\n'.join(added_lines(git(*diff, '--no-ext-diff', '--unified=0', '--end-of-options', base, '--', name)))
            if mentions_css_in_js(text):
                text = current(name)
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
