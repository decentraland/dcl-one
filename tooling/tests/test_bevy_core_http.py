"""Keep trusted Explorer browser requests observable at their point of origin."""
from pathlib import Path
import re
import subprocess
import unittest


ROOT = Path(__file__).resolve().parents[2]
SOURCE = 'rendering/bevy-explorer/source/'
HUDS = (SOURCE + 'react-web/src/', 'apps/ui3/src/play/')
WEB = SOURCE + 'deploy/web/'
# The sandbox uses SDK Fetch. The service worker, preview realm and local-network
# shim forward existing requests. Boot injects the observed transport into perf_stream.
# The guest HUD observes Thirdweb at its backend boundary. SDK scenes use the same
# Thirdweb functions with their own Fetch ownership.
RAW_FETCH = {WEB + 'engine/core_http.js', WEB + 'engine/sandbox_worker.js', WEB + 'engine/scene_fetch.js',
             WEB + 'service_worker.js', WEB + 'preview_realm.js', WEB + 'engine/perf_stream.js',
             *(hud + name for hud in HUDS for name in ('lib/localNetworkFetch.ts', 'features/auth/thirdweb.ts'))}
ORIGIN = re.compile(r'\b(?:window|self|globalThis)\.fetch\b|(?<![.\w$])fetch\s*\(|'
                    r'(?:=|\?\?)\s*fetch\s*[;,)]|\bnew\s+XMLHttpRequest\s*\(|\bnavigator\.sendBeacon\s*\(')


class BrowserHttpOrigins(unittest.TestCase):
    def test_trusted_features_observe_browser_http(self):
        files = subprocess.check_output(['git', 'ls-files', '-z', '--', WEB, *HUDS], cwd=ROOT)
        findings = []
        for path in files.decode().split('\0'):
            if (not path or Path(path).suffix not in {'.js', '.ts', '.tsx', '.html'} or path in RAW_FETCH
                    or set(Path(path).parts) & {'test', 'tests', 'scripts'} or '.test.' in path):
                continue
            for n, line in enumerate((ROOT / path).read_text(encoding='utf-8').splitlines(), 1):
                if not line.lstrip().startswith(('//', '*', '/*', '<!--')) and ORIGIN.search(line):
                    findings.append(f'{path}:{n}: {line.strip()}')
        self.assertFalse(findings, 'Use coreFetch or hudFetch for trusted HTTP:\n' + '\n'.join(findings))
