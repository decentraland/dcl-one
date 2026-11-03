#!/usr/bin/env python3
"""Inventory source metric names/labels against literal Rust instrumentation.

This static check is deliberately not evidence of runtime semantics or scrape
value equality. Dynamic macro names and provider-specific default collectors
require separate review. Run --write after relevant instrumentation changes.
"""
import argparse
import json
import re
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'services/catalyst/tests/fixtures/conformance/social'
OUTPUT = FIXTURES / 'metrics-inventory.json'
MACRO = re.compile(r'metrics::(counter|gauge|histogram)!\s*\(\s*"([^"]+)"')


def inventory():
    source = json.loads((FIXTURES / 'auxiliary-cases.json').read_text())
    candidate = {}
    for folder in ['services/catalyst', 'library']:
        for path in sorted((ROOT / folder).rglob('*.rs')):
            relative = path.relative_to(ROOT).as_posix()
            if '/tests/' in relative or relative.endswith('/tests.rs'):
                continue
            # A cfg(test) method may precede production methods. Scan the
            # whole production file rather than truncating everything after it.
            # Inline test-module hits are expressly possible and need review.
            text = path.read_text()
            for match in MACRO.finditer(text):
                # Nested calls inside label expressions must not truncate later labels.
                cursor = match.end()
                depth, quoted, escaped = 1, False, False
                while cursor < len(text) and depth:
                    char = text[cursor]
                    if quoted:
                        if escaped:
                            escaped = False
                        elif char == '\\':
                            escaped = True
                        elif char == '"':
                            quoted = False
                    elif char == '"':
                        quoted = True
                    elif char == '(':
                        depth += 1
                    elif char == ')':
                        depth -= 1
                    cursor += 1
                body = text[match.end():cursor - 1]
                candidate.setdefault(match[2], []).append({
                    'path': relative, 'line': text[:match.start()].count('\n') + 1,
                    'kind': match[1], 'scope': 'production_file_inline_test_possible',
                    'labels': sorted(set(re.findall(r'"([a-zA-Z_][a-zA-Z0-9_]*)"\s*=>', body)))
                })
    rows = []
    for name, declaration in source['metric_declarations'].items():
        locations = candidate.get(name, [])
        expected = sorted(declaration.get('labelNames', []))
        kind = declaration['type']
        status = 'missing_literal_instrumentation' if not locations else (
            'literal_type_or_labels_differ' if any(row['labels'] != expected or row['kind'] != kind for row in locations)
            else 'literal_shape_present_runtime_unverified')
        rows.append({'name': name, 'type': kind, 'source_labels': expected,
                     'source_buckets': declaration.get('buckets'),
                     'candidate': locations, 'status': status})
    return {
        'baseline': source['baseline'],
        'scope': 'Static literal name/type/label inventory across entire production Rust files, excluding dedicated test paths. Inline test-module hits are possible and require source review. Presence does not establish production execution, runtime event meaning, series initialization, histogram buckets, dynamic-name sites or sample values. Missing literals remain candidates until dynamic sites are checked.',
        'known_differences': [
            'WKC_METRICS_RESET_AT_NIGHT=true is accepted but the social view does not reset; the global recorder is preserved for other domains.',
            'Node/V8/process default collectors are not fabricated by the Rust exporter. Their runtime-specific families are outside this declared-family inventory.',
            'The filtered shared recorder can include shared-family labels emitted by other domains; source-compatible per-family instrumentation and labels require review.',
            'The Social view renders all 44 source HELP/TYPE declarations and seeds only the 16 captured unlabelled counter/gauge zeros; the other 28 declarations have no invented initial samples. Registration and literal presence do not prove event or value equality.',
            'Portable ordinary/guarded acceptance is observed after the Tokio sink accepts a frame, before flush. Native uWS backpressure/drop/drain/buffer observations remain unavailable; the success label does not establish native buffering equivalence.',
            'Source logger event coverage and LOG_LEVEL filtering remain incomplete. Source authentication-race completion, stale subscriber reconciliation, and stream producer errors represented as ordinary Generator termination remain explicit event gaps.'
        ],
        'metrics': rows
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    expected = inventory()
    if args.write:
        OUTPUT.write_text(json.dumps(expected, indent=2) + '\n')
    elif not OUTPUT.exists() or json.loads(OUTPUT.read_text()) != expected:
        raise SystemExit('metrics inventory is stale; run tooling/social-service-metrics.py --write')
    print('source metric declaration inventory:', len(expected['metrics']), dict(Counter(row['status'] for row in expected['metrics'])))
    print('Static shapes only; runtime interpretation and reset parity are not inferred.')


if __name__ == '__main__':
    main()
