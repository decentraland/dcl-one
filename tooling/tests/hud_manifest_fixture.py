"""Synthetic resolved graphs for artifact tests; no source import resolution."""
import hashlib
import json
from pathlib import Path

WEB = 'rendering/bevy-explorer/source/react-web/'
MANIFEST = WEB + 'shared-inputs.json'
SHARED = ('library/shop/model/money.ts', 'library/catalyst-api/credits/BalanceOut.ts',
          'library/ui/foundations/index.ts', 'library/ui/foundations/colors/colors.css')
CONTROLS = tuple(WEB + name for name in ('package.json', 'package-lock.json', 'tsconfig.json',
                                       'vite.config.ts', 'vitest.config.ts', 'vite.shared.ts',
                                       'scripts/shared-inputs.mjs')) + ('library/ui/package.json', 'library/ui/foundations/typography/font-source.json')


def seal(root, shared=SHARED):
    root = Path(root)
    for name in CONTROLS:
        path = root / name
        if not path.exists():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('fixture resolver control')
    roots = sorted(p.relative_to(root).as_posix() for p in (root / WEB).rglob('*') if p.is_file()
                   and not p.relative_to(root).as_posix().startswith(WEB + 'src/engine/generated/')
                   and 'node_modules' not in p.relative_to(root).parts
                   and (p.relative_to(root).as_posix().startswith(WEB + 'src/')
                        or p.relative_to(root).as_posix() in (WEB + 'app.html', WEB + 'index.html')))
    names = sorted(set(roots) | {p for p in shared if (root / p).is_file()})
    rows = [{'path': p, 'sha256': hashlib.sha256((root / p).read_bytes()).hexdigest()} for p in names]
    target = root / MANIFEST
    target.parent.mkdir(parents=True, exist_ok=True)
    controls = [{'path': p, 'sha256': hashlib.sha256((root / p).read_bytes()).hexdigest()} for p in sorted(CONTROLS)]
    target.write_text(json.dumps({'schema': 1, 'controls': controls, 'roots': roots, 'files': rows, 'edges': []}, sort_keys=True) + '\n')
