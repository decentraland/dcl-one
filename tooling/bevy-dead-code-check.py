#!/usr/bin/env python3
"""Fail on an allow, expect or warn that weakens dead code in the Explorer's own Rust sources.

Dead code is deleted, used or gated with #[cfg]. Only a module that holds nothing but `use` items
and ShaderType-derived structs takes `#![expect(dead_code, reason = SHADER_TYPE_REASON)]` for the
derive's never-called check fns; the expect fails once that code is gone.
"""
import argparse
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
SOURCE = 'rendering/bevy-explorer/source'
SCANNED = ('build.rs', 'src', 'crates')  # vendor/ holds upstream code; local-harness/ is test tooling
SKIP_DIRS = {'target'}
SUPPRESSING = {'dead_code', 'unused', 'warnings'}
SHADER_TYPE_REASON = 'the ShaderType derive emits a never-called check fn per field'

ATTRIBUTE_START = re.compile(r'#\s*(!?)\s*\[')
STRING = re.compile(r'"(?:\\.|[^"\\])*"')
CHAR = re.compile(r"'(?:\\(?:u\{[0-9a-fA-F]+\}|x[0-9a-fA-F]{2}|.)|[^\\'\n])'")
RAW_STRING = re.compile(r'r(#*)"')
LINT_LIST = re.compile(r'\b(allow|expect|warn)\s*\(([^()]*)\)')
REASON = re.compile(r'^reason\s*=\s*S(\d+)$')
VISIBILITY = r'(?:pub\s*(?:\([^)]*\))?\s*)?'
USE_ITEM = re.compile(VISIBILITY + r'use\b[^;]*;')
STRUCT_ITEM = re.compile(VISIBILITY + r'struct\s+\w+\s*\{')
SHADER_TYPE_DERIVE = re.compile(r'^#\s*\[\s*derive\s*\([^)]*\bShaderType\b[^)]*\)\s*\]$')


def without_comments(text, blank_literals=False):
    """`text` with every comment, nested block comments included, blanked to spaces; offsets and
    newlines are kept. Strings and chars are kept, or with `blank_literals` blanked inside their
    quotes."""
    out, i, n = [], 0, len(text)
    while i < n:
        raw = RAW_STRING.match(text, i) if text[i] == 'r' and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == '_')) else None
        if raw:
            end = text.find('"' + raw.group(1), raw.end())
            end = n if end < 0 else end + 1 + len(raw.group(1))
        elif text[i] == '"':
            match = STRING.match(text, i)
            end = match.end() if match else n
        elif text[i] == "'":
            match = CHAR.match(text, i)
            end = match.end() if match else i + 1  # a lifetime
        elif text.startswith('//', i):
            end = text.find('\n', i)
            end = n if end < 0 else end
            out.append(' ' * (end - i))
            i = end
            continue
        elif text.startswith('/*', i):
            depth, end = 0, i
            while end < n:
                if text.startswith('/*', end):
                    depth, end = depth + 1, end + 2
                elif text.startswith('*/', end):
                    depth, end = depth - 1, end + 2
                    if depth == 0:
                        break
                else:
                    end += 1
            out.append(re.sub(r'[^\n]', ' ', text[i:end]))
            i = end
            continue
        else:
            end = i + 1
        literal = text[i:end]
        if blank_literals and end - i > 1:
            literal = literal[0] + re.sub(r'[^\n]', ' ', literal[1:-1]) + literal[-1]
        out.append(literal)
        i = end
    return ''.join(out)


def closing(code, i, opening, close):
    """The index of the `close` that matches the `opening` at `i` in literal-blanked `code`."""
    depth = 0
    while i < len(code):
        c = code[i]
        if c == opening:
            depth += 1
        elif c == close:
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return len(code)


def attributes(text, code):
    """Yield (start, end, inner, attribute) for each `#[...]` and `#![...]` in `code`, the
    literal-blanked form of comment-free `text`; `attribute` keeps its strings."""
    for start in ATTRIBUTE_START.finditer(code):
        end = closing(code, start.end() - 1, '[', ']') + 1
        yield start.start(), end, bool(start.group(1)), text[start.start():end]


def lint_lists(attribute):
    """Yield (level, lints, reason) for each allow, expect or warn list in `attribute`."""
    strings = []

    def hold(match):
        strings.append(match.group(0)[1:-1])
        return f'S{len(strings) - 1}'

    for level, body in LINT_LIST.findall(STRING.sub(hold, attribute)):
        entries = [entry.strip() for entry in body.split(',') if entry.strip()]
        reasons = [REASON.match(entry) for entry in entries if '=' in entry]
        reason = strings[int(reasons[0].group(1))] if reasons and reasons[0] else None
        yield level, [entry for entry in entries if '=' not in entry], reason


def only_shader_types(body):
    """True when `body`, a literal-blanked module body, holds only `use` items and structs that
    derive ShaderType, with at least one such struct."""
    i, derives, structs = 0, False, 0
    while True:
        while i < len(body) and body[i].isspace():
            i += 1
        if i == len(body):
            return structs > 0 and not derives
        if body[i] == '#':
            end = closing(body, i, '[', ']') + 1
            derives = derives or bool(SHADER_TYPE_DERIVE.match(body[i:end]))
            i = end
        elif match := USE_ITEM.match(body, i):
            i = match.end()
        elif (match := STRUCT_ITEM.match(body, i)) and derives:
            i = closing(body, match.end() - 1, '{', '}') + 1
            derives, structs = False, structs + 1
        else:
            return False


def suppresses_dead_code(code, start, end, inner, attribute):
    """True when the attribute at `code[start:end]` weakens dead code, apart from the ShaderType
    expect at the head of a module that holds only ShaderType structs."""
    for level, lints, reason in lint_lists(attribute):
        if not SUPPRESSING & set(lints):
            continue
        if level == 'expect' and inner and lints == ['dead_code'] and reason == SHADER_TYPE_REASON:
            opening = code.rfind('{', 0, start)
            if opening >= 0 and not code[opening + 1:start].strip():
                if only_shader_types(code[end:closing(code, opening, '{', '}')]):
                    continue
        return True
    return False


def offenders(root):
    source = root / SOURCE
    if not source.is_dir():
        raise FileNotFoundError(f'{source}: no Explorer sources to check')
    found = []
    for entry in SCANNED:
        path = source / entry
        files = [path] if path.is_file() else sorted(path.rglob('*.rs'))
        for file in files:
            if SKIP_DIRS & set(file.relative_to(source).parts[:-1]):
                continue
            source_text = file.read_text(encoding='utf-8', errors='replace')
            text, code = without_comments(source_text), without_comments(source_text, blank_literals=True)
            for start, end, inner, attribute in attributes(text, code):
                if suppresses_dead_code(code, start, end, inner, attribute):
                    line = code.count('\n', 0, start) + 1
                    found.append((file.relative_to(root).as_posix(), line, ' '.join(attribute.split())))
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--root', type=Path, default=ROOT)
    args = parser.parse_args()
    try:
        found = offenders(args.root)
    except FileNotFoundError as error:
        print(error, file=sys.stderr)
        return 2
    for rel, line, attribute in found:
        print(f'{rel}:{line}: {attribute}: delete, use or #[cfg]-gate the dead code instead', file=sys.stderr)
    return 1 if found else 0


if __name__ == '__main__':
    sys.exit(main())
