#!/usr/bin/env python3
"""Rewrite the explore parity fixtures in their canonical layout.

A value whose one-line form fits in the width stays on one line. A longer object or list
flows: its members fill lines up to the width, and a member that needs several lines
starts and ends a line. The content does not change.
"""
import json
import sys
from pathlib import Path

WIDTH = 110


def flat(value):
    return json.dumps(value, ensure_ascii=False, separators=(", ", ": "))


def render(value, indent=0):
    text = flat(value)
    if len(text) + indent <= WIDTH or not isinstance(value, (dict, list)) or not value:
        return text
    if isinstance(value, dict):
        pieces = [f"{json.dumps(k, ensure_ascii=False)}: {render(v, indent + 2)}" for k, v in value.items()]
        brackets = "{}"
    else:
        pieces = [render(item, indent + 2) for item in value]
        brackets = "[]"
    pad = " " * (indent + 2)
    lines, line = [], pad
    for index, piece in enumerate(pieces):
        piece += "," if index < len(pieces) - 1 else ""
        multi = "\n" in piece
        if line.strip() and (multi or len(line) + len(piece) > WIDTH):
            lines.append(line.rstrip())
            line = pad
        line += piece + " "
        if multi:
            lines.append(line.rstrip())
            line = pad
    if line.strip():
        lines.append(line.rstrip())
    return brackets[0] + "\n" + "\n".join(lines) + "\n" + " " * indent + brackets[1]


def main(argv):
    root = Path(__file__).resolve().parents[1]
    paths = [Path(p) for p in argv if p != "--check"] or sorted(
        (root / "services/catalyst/tests/fixtures/explore_parity").glob("*.fixture.json"))
    check = "--check" in argv
    changed = 0
    for path in paths:
        before = path.read_text()
        after = render(json.loads(before)) + "\n"
        assert json.loads(after) == json.loads(before)
        if after != before:
            changed += 1
            if check:
                print(f"{path}: not in the canonical layout")
            else:
                path.write_text(after)
    return 1 if check and changed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
