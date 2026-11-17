#!/usr/bin/env python3
"""Run the shared metadata corpus through the pure Nix validator without building packages."""
import argparse
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
from hud_manifest_fixture import WEB, MANIFEST, SHARED, seal, corpus_files, index_corpus_payload, corpus_manifest

REPOSITORY = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('checked_hud', REPOSITORY / 'tooling/hud_shared_inputs.py')
reader = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(reader)


def evaluate(root, cases_file, cases, python_only):
    if not python_only:
        subprocess.run(['nix-instantiate', '--eval', '--strict', '--json',
                        str(REPOSITORY / 'infra/packages/hud-shared-inputs-corpus.nix'),
                        '--arg', 'root', str(root), '--arg', 'casesFile', str(cases_file)], check=True)
    for case in cases:
        # Full-source equality is Nix's pre-filter boundary; a Gitless Python source is filtered.
        if not (root / '.git').exists() and not case.get('filteredSource', False):
            continue
        (root / MANIFEST).write_text(json.dumps(case['manifest']))
        try:
            reader.read_manifest(root)
            accepted = True
        except reader.ManifestError:
            accepted = False
        if accepted != case['valid']:
            raise AssertionError('Python/Nix corpus expectation differs: ' + case['name'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--python-only', action='store_true', help='Check fixture expectations locally without claiming Nix evaluation')
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='hud-nix-corpus-') as temporary:
        workspace = Path(temporary)
        root = workspace / 'source'
        root.mkdir()
        def write(name, content):
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content)
        subprocess.run(['git', 'init', '-q', str(root)], check=True)
        write(WEB + 'src/main.ts', 'export {}')
        write(SHARED[0], 'export const value = 1')
        write('library/shop/node_modules/fixture.ts', 'export const dependency = 1')
        for name in ('A.ts', 'a-lower.ts', '_punct.ts', 'é.ts', '\ue000.ts', '\U00010000.ts'):
            write('library/shop/' + name, 'export {}')
        lfs = corpus_files(root)
        seal(root)
        subprocess.run(['git', '-C', str(root), 'add', '-f', '.'], check=True)
        index_corpus_payload(root, lfs)
        baseline = json.loads((root / MANIFEST).read_text())
        definitions = json.loads((REPOSITORY / 'tooling/tests/hud_manifest_invalid_cases.json').read_text())
        cases = [{'name': 'canonical-v2-mixed-UTF8-names', 'manifest': baseline, 'valid': True}]
        cases += [{'name': case['name'], 'manifest': corpus_manifest(baseline, case, root, lfs), 'valid': case['valid'],
                   'nixValid': case.get('nixValid', case['valid'])}
                  for case in definitions]
        cases_file = workspace / 'cases.json'
        cases_file.write_text(json.dumps(cases))
        evaluate(root, cases_file, cases, args.python_only)
        (root / MANIFEST).write_text(json.dumps(baseline))
        shutil.rmtree(root / '.git')
        required = {row['path'] for group in ('controls', 'files') for row in baseline[group]} | {MANIFEST}
        for path in sorted(root.rglob('*'), key=lambda item: len(item.parts), reverse=True):
            if path.is_file() and path.relative_to(root).as_posix() not in required:
                path.unlink()
            elif path.is_dir() and not any(path.iterdir()):
                path.rmdir()
        subset_cases = [
            {'name': 'checked-filtered-subset', 'manifest': baseline, 'valid': True, 'filteredSource': True},
            {'name': 'full-source-cannot-omit-witnessed-names', 'manifest': baseline, 'valid': False}]
        cases_file.write_text(json.dumps(subset_cases))
        evaluate(root, cases_file, subset_cases, args.python_only)
        write('library/shop/.consumer-foundations-checked/output.ts', 'canonical private output')
        cases = [{'name': 'known-output-directory-is-excluded', 'manifest': baseline, 'valid': True, 'filteredSource': True}]
        cases_file.write_text(json.dumps(cases))
        evaluate(root, cases_file, cases, args.python_only)
        write('library/shop/.consumer-foundations-shadow.ts', 'export {}')
        cases = [{'name': 'regular-output-prefix-is-not-excluded', 'manifest': baseline, 'valid': False, 'filteredSource': True}]
        cases_file.write_text(json.dumps(cases))
        evaluate(root, cases_file, cases, args.python_only)
        print('HUD metadata/namespace corpus passed' + (' (Python fixture validation only; Nix not executed)' if args.python_only else ' (pure Nix and Python)'))


if __name__ == '__main__':
    main()
