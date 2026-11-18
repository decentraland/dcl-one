#!/usr/bin/env python3
"""Fail on an allow or expect that suppresses dead code in the Explorer's own Rust sources.

Dead code is deleted, used or gated with #[cfg]. Only the ShaderType derive's never-called check
fns take `expect(dead_code, reason = SHADER_TYPE_REASON)`, which fails once that code is gone.
"""
import argparse
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
SOURCE = 'rendering/bevy-explorer/source'
SCANNED = ('build.rs', 'src', 'crates', 'local-harness')  # vendor/ holds upstream code
SKIP_DIRS = {'target'}
SUPPRESSING = {'dead_code', 'unused', 'warnings'}
SHADER_TYPE_REASON = 'the ShaderType derive emits a never-called check fn per field'

ATTRIBUTE_START = re.compile(r'#!?\[')
STRING = re.compile(r'"(?:\\.|[^"\\])*"')
LINT_LIST = re.compile(r'\b(allow|expect)\s*\(([^()]*)\)')
REASON = re.compile(r'^reason\s*=\s*S(\d+)$')


def attributes(text):
    """Yield (line, text) for each `#[...]` and `#![...]`, across lines and with strings kept."""
    for start in ATTRIBUTE_START.finditer(text):
        depth, i, in_string = 0, start.end() - 1, False
        while i < len(text):
            c = text[i]
            if in_string:
                if c == '\\':
                    i += 1
                elif c == '"':
                    in_string = False
            elif c == '"':
                in_string = True
            elif c == '[':
                depth += 1
            elif c == ']':
                depth -= 1
                if depth == 0:
                    break
            i += 1
        yield text.count('\n', 0, start.start()) + 1, text[start.start():i + 1]


def suppresses_dead_code(attribute):
    """True when an allow or expect in `attribute` covers dead code, apart from the ShaderType expect."""
    strings = []

    def hold(match):
        strings.append(match.group(0)[1:-1])
        return f'S{len(strings) - 1}'

    for kind, body in LINT_LIST.findall(STRING.sub(hold, attribute)):
        entries = [entry.strip() for entry in body.split(',') if entry.strip()]
        lints = [entry for entry in entries if '=' not in entry]
        if not SUPPRESSING & set(lints):
            continue
        reasons = [REASON.match(entry) for entry in entries if '=' in entry]
        reason = strings[int(reasons[0].group(1))] if reasons and reasons[0] else None
        if kind == 'expect' and lints == ['dead_code'] and reason == SHADER_TYPE_REASON:
            continue
        return True
    return False


def offenders(root):
    found = []
    source = root / SOURCE
    for entry in SCANNED:
        path = source / entry
        files = [path] if path.is_file() else sorted(path.rglob('*.rs'))
        for file in files:
            if SKIP_DIRS & set(file.relative_to(source).parts[:-1]):
                continue
            text = file.read_text(encoding='utf-8', errors='replace')
            for line, attribute in attributes(text):
                if suppresses_dead_code(attribute):
                    found.append((file.relative_to(root).as_posix(), line, ' '.join(attribute.split())))
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--root', type=Path, default=ROOT)
    args = parser.parse_args()
    found = offenders(args.root)
    for rel, line, attribute in found:
        print(f'{rel}:{line}: {attribute}: delete, use or #[cfg]-gate the dead code instead', file=sys.stderr)
    return 1 if found else 0


if __name__ == '__main__':
    sys.exit(main())
