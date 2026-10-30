#!/usr/bin/env python3
"""The suite's result as the `inworld` section of the Bevy Explorer CI status comment.

    python3 tooling/bevy-e2e-report.py <junit.xml> <run url> [<label>]

Reads the JUnit report `node --test` writes (a `testcase` with a `failure` failed; one with a
`<skipped type="todo">` is a recorded gap; one with `<skipped type="skipped">` was skipped) and
prints Markdown: a badge, the counts and the failing scenarios. A missing report is a failure to
say so.
"""
import sys
import xml.etree.ElementTree as ElementTree
from collections import Counter
from pathlib import Path

MAX_FAILURES = 25


def outcome(case):
    if case.find('failure') is not None or case.find('error') is not None:
        return 'failed'
    skipped = case.find('skipped')
    if skipped is not None:
        return 'todo' if skipped.get('type') == 'todo' else 'skipped'
    return 'passed'


def summarize(xml_text):
    root = ElementTree.fromstring(xml_text)
    counts = Counter()
    failures = []
    for case in root.iter('testcase'):
        result = outcome(case)
        counts[result] += 1
        if result == 'failed':
            problem = case.find('failure')
            if problem is None:
                problem = case.find('error')
            message = (problem.get('message') or '').strip().splitlines()
            failures.append((case.get('classname') or '', case.get('name') or '', message[0] if message else ''))
    return counts, failures


def section(xml_text, run_url, label='InWorld'):
    counts, failures = summarize(xml_text)
    total = sum(counts.values())
    if total == 0:
        status, colour = 'No%20scenarios', 'lightgrey'
    elif counts['failed']:
        status, colour = f"{counts['failed']}%20failed", 'ff0000'
    else:
        status, colour = f"{counts['passed']}%20passed", '3fb950'
    lines = [f'[![{label}](https://img.shields.io/badge/{label}-{status}-{colour}?logo=github&logoColor=white&style=for-the-badge)]({run_url})', '']
    lines.append(f"{counts['passed']} passed, {counts['failed']} failed, {counts['todo']} recorded gaps (todo), "
                 f"{counts['skipped']} skipped, of {total} scenarios ([run]({run_url})).")
    if failures:
        lines.append('')
        lines.append('Failed:')
        for classname, name, message in failures[:MAX_FAILURES]:
            where = f'{classname}: ' if classname else ''
            detail = f' — {message}' if message else ''
            lines.append(f'- {where}{name}{detail}')
        if len(failures) > MAX_FAILURES:
            lines.append(f'- and {len(failures) - MAX_FAILURES} more')
    return '\n'.join(lines) + '\n'


def missing(run_url, label='InWorld'):
    return (f'[![{label}](https://img.shields.io/badge/{label}-No%20report-ff0000?logo=github&logoColor=white&style=for-the-badge)]({run_url})\n\n'
            f'The suite wrote no report: it did not start or was cut off ([run]({run_url})).\n')


def main(argv):
    if len(argv) < 3:
        print(__doc__, file=sys.stderr)
        return 2
    path, run_url = Path(argv[1]), argv[2]
    label = argv[3] if len(argv) > 3 else 'InWorld'
    if not path.is_file():
        sys.stdout.write(missing(run_url, label))
        return 0
    sys.stdout.write(section(path.read_text(encoding='utf-8'), run_url, label))
    return 0


if __name__ == '__main__':
    raise SystemExit(main(sys.argv))
