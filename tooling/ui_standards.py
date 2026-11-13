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
# Application entry stylesheets (App.css, index.css, main.css directly in an app's src/) are global by design.
ENTRY_STYLESHEETS = ('app', 'index', 'main')
ENTRY_DIRECTORY = re.compile(r'^(?:apps/[^/]+/src|rendering/bevy-explorer/source/react-web/src)/[^/]+$')
# Files that already broke these rules when a whole project was imported (apps/creator-hub,
# docs/adr/2026-11-09-creator-hub-in-dcl-one.md). They pass only as the files that change adds;
# any later change to one is checked like every other file. The list is read from the base revision,
# so nothing can be added to it after the change that created it (imported_paths).
IMPORTED = 'tooling/ui-standards-imported.txt'
ZERO_SHA = re.compile(r'^0+$')
COMMIT_SHA = re.compile(r'^[0-9a-f]{40}$')
CSS_IN_JS_MODULE = re.compile(r'^(?:styled-components|@emotion/(?:styled|react|css)|@mui/(?:material/styles|system)/styled|@mui/styled-engine(?:-sc)?)(?:/.*)?$')
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


STATEMENT_WORDS = {'const', 'let', 'var', 'function', 'class', 'return', 'if', 'for', 'while', 'switch',
                   'interface', 'enum', 'default', 'export', 'import', 'await', 'async', 'new'}
EMOTION_PRAGMA = re.compile(r'@jsxImportSource\s+@emotion/react\b')


def import_clause(items, start):
    """Read an import/export clause from items[start:] up to its specifier.

    Returns (names, specifier) or None when the statement is not a module import or re-export.
    Braces may span lines; outside them, a statement keyword ends the clause, so code without
    semicolons does not run into the next statement.
    """
    names = []
    depth = 0
    previous = None
    for kind, value in items[start:]:
        if kind == 'string':
            if previous == ('word', 'from') or (previous is None and not names):
                return names, value
            return None
        if kind == 'punct' and value == ';':
            return None
        if kind == 'punct' and value == '{':
            depth += 1
        elif kind == 'punct' and value == '}':
            depth -= 1
        elif kind == 'word':
            if depth == 0 and value in STATEMENT_WORDS and value != 'default':
                return None
            names.append(value)
        elif kind == 'punct' and value == '*':
            names.append(value)
        previous = (kind, value)
    return None


def destructured_before(items, index):
    """Names destructured on the left of `= [await] require(...)` / `= await import(...)`."""
    cursor = index - 1
    if cursor >= 0 and items[cursor] == ('word', 'await'):
        cursor -= 1
    if cursor < 0 or items[cursor] != ('punct', '='):
        return set()
    cursor -= 1
    if cursor < 0 or items[cursor] != ('punct', '}'):
        return set()
    names = set()
    while cursor >= 0 and items[cursor] != ('punct', '{'):
        if items[cursor][0] == 'word':
            names.add(items[cursor][1])
        cursor -= 1
    return names


def forbidden_imports(text):
    """Return the module specifiers of the CSS-in-JS imports in source text."""
    found = ['@emotion/react (jsxImportSource)'] if EMOTION_PRAGMA.search(text) else []
    items = tokens(text)
    namespaces = set()
    for index, (kind, value) in enumerate(items):
        if kind != 'word' or value not in ('import', 'export', 'require'):
            continue
        if index and items[index - 1] == ('punct', '.'):
            continue
        following = items[index + 1:index + 3]
        if following and following[0] == ('punct', '('):
            if len(following) > 1 and following[1][0] == 'string':
                specifier = following[1][1]
                if CSS_IN_JS_MODULE.match(specifier) or (
                        MUI_MODULE.match(specifier) and 'styled' in destructured_before(items, index)):
                    found.append(specifier)
            continue
        if value == 'require':
            continue
        clause = import_clause(items, index + 1)
        if clause is None:
            continue
        names, specifier = clause
        if CSS_IN_JS_MODULE.match(specifier) or ('styled' in names and MUI_MODULE.match(specifier)):
            found.append(specifier)
        elif MUI_MODULE.match(specifier) and names[:2] == ['*', 'as'] and len(names) > 2:
            namespaces.add(names[2])
    for namespace in sorted(namespaces):
        uses_styled = any(items[i][0] == 'word' and items[i][1] == namespace and items[i + 1] == ('punct', '.')
                          and items[i + 2] == ('word', 'styled') for i in range(len(items) - 2))
        if uses_styled:
            found.append(f'{namespace}.styled')
    return found


def mentions_css_in_js(text):
    """Whether added text could belong to a CSS-in-JS import, so the whole file needs a scan."""
    if EMOTION_PRAGMA.search(text):
        return True
    return any((kind == 'word' and value == 'styled')
               or (kind == 'string' and (CSS_IN_JS_MODULE.match(value) or MUI_MODULE.match(value)))
               for kind, value in tokens(text))


def is_component_stylesheet(path, paths):
    if not path.endswith('.css') or path.endswith('.module.css'):
        return False
    if PurePosixPath(path).stem.casefold() in ENTRY_STYLESHEETS and ENTRY_DIRECTORY.match(path):
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
    fields = git(*diff, '--no-color', '--name-status', '-z', '-M', '--diff-filter=AR', '--end-of-options', base, '--').split('\0')
    result = set()
    index = 0
    while index < len(fields) and fields[index]:
        status = fields[index]
        if status.startswith('R'):
            old, new = fields[index + 1], fields[index + 2]
            moved = is_plain_stylesheet(old) and PurePosixPath(old).name == PurePosixPath(new).name
            if not moved:
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
    an error rather than a silent pass, and so is a shallow clone without the
    history to compare against.
    """
    if not ZERO_SHA.match(base):
        if verify(base):
            return base
        if not COMMIT_SHA.match(base):
            raise ValueError(f'unknown base revision {base!r}')
    if verify('HEAD~1'):
        fallback = 'HEAD~1'
    elif git('rev-parse', '--is-shallow-repository').strip() == 'true':
        raise ValueError(f'base {base!r} is not available in this shallow clone; fetch more history')
    else:
        fallback = 'HEAD'
    print(f'UI standards: base {base!r} is not a reachable commit; comparing with {fallback}', file=sys.stderr)
    return fallback


def parse_imported(text):
    if text is None:
        return None
    return frozenset(line.strip() for line in text.splitlines() if line.strip() and not line.startswith('#'))


def imported_paths(base_text, current_text):
    """The import list's exempt paths, and the issues a change to the list raises.

    The base revision's list is the one that counts, so a change can't exempt its own files by
    listing them. Only the change that creates the list (none at base) exempts what it lists, and
    the list can't be deleted afterwards, since deleting it would let the next change create it.
    """
    base_paths = parse_imported(base_text)
    current_paths = parse_imported(current_text)
    if base_paths is None:
        return current_paths or frozenset(), []
    if current_paths is None:
        return base_paths, [f'{IMPORTED}: the import list can\'t be deleted; empty it instead']
    if current_paths - base_paths:
        return base_paths, [f'{IMPORTED}: paths can\'t be added to the import list']
    return base_paths, []


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
        if staged:
            return git('show', f':{name}')
        return (ROOT / name).read_text(encoding='utf-8', errors='replace')

    def previous(name):
        try:
            return git('show', f'{base}:{name}')
        except subprocess.CalledProcessError:
            return ''

    def existing(read, name):
        try:
            return read(name)
        except (OSError, subprocess.CalledProcessError):
            return None

    def at_base(name):
        return git('show', f'{base}:{name}')

    changed = names(diff, '--name-only', '--diff-filter=ACMR', '--end-of-options', base, '--') | untracked
    added = new_files(diff, base) | untracked
    exempt, issues = imported_paths(existing(at_base, IMPORTED), existing(current, IMPORTED))
    imported = exempt & added
    for name in sorted(added - imported):
        if is_ui_source(name) and is_component_stylesheet(name, tracked):
            issues.append(f'{name}: new component styles must use .module.css')
    for name in sorted(changed - imported):
        if not is_ui_source(name) or not name.endswith(SCRIPT_EXTENSIONS):
            continue
        if name in untracked:
            new_imports = forbidden_imports(current(name))
        else:
            text = '\n'.join(added_lines(git(*diff, '--no-ext-diff', '--no-color', '--unified=0',
                                              '--end-of-options', base, '--', name)))
            if not mentions_css_in_js(text):
                continue
            new_imports = forbidden_imports(current(name))
            for existing in forbidden_imports(previous(name)):
                if existing in new_imports:
                    new_imports.remove(existing)
        if new_imports:
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
