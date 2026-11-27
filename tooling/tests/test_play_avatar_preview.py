"""The avatar preview's publication layout: one revalidated page, every file it loads content-hashed."""
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import tempfile
import unittest
from contextlib import redirect_stdout
import io

REPO = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('preview_package', REPO / 'rendering/avatar-preview/tools/package.py')
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)
HASHED = r'[0-9a-f]{16}'


class PreviewPackageTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.web = self.root / 'web'
        self.out = self.root / 'out'
        self.write('web/index.html', "<style>a { background: url('ui/Stop.png'); } b { src: url('ui/Font.otf') }</style>\n"
                                     "<script type=\"module\">const { boot } = await import('./preview.js');</script>\n")
        self.write('web/preview.js', "const FILES = null;\nexport function boot() {}\n")
        self.write('web/emotes/love.glb', b'glTF love')
        self.write('web/emotes/README.md', 'prose')
        self.write('web/ui/Stop.png', b'png')
        self.write('web/ui/Font.otf', b'otf')
        self.write('web/ui/README.md', 'prose')
        for backend in package.BACKENDS:
            self.write(f'web/{backend}/dcl_avatar_preview.js', f"import './snippets/platform-1/src/web_save.js'; // {backend}")
            self.write(f'web/{backend}/dcl_avatar_preview_bg.wasm', b'\0asm\x01\0\0\0' + backend.encode())
            self.write(f'web/{backend}/content_cache.js', 'export function installContentCachePageFunctions() {}')
            self.write(f'web/{backend}/snippets/platform-1/src/web_save.js', 'export {}')
            for leftover in ('package.json', 'README.md', 'dcl_avatar_preview.d.ts', '.gitignore'):
                self.write(f'web/{backend}/{leftover}', 'not published')

    def write(self, name, content):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content.encode() if isinstance(content, str) else content)
        return path

    def package(self, **options):
        with redirect_stdout(io.StringIO()):
            return package.package(self.out, self.web, self.web, **options)

    def published(self):
        return sorted(path.relative_to(self.out).as_posix() for path in self.out.rglob('*') if path.is_file())

    def test_the_page_loads_only_content_hashed_files_that_exist(self):
        self.package()
        files = self.published()
        self.assertEqual([name for name in files if not name.startswith('assets/')], ['index.html'])
        page = (self.out / 'index.html').read_text()
        script = re.search(rf"import\('\.\./assets/avatar-preview/(preview\.{HASHED}\.js)'\)", page)
        self.assertTrue(script, page)
        for name in re.findall(r"url\('\.\./assets/avatar-preview/([^']+)'\)", page) + [script[1]]:
            with self.subTest(name=name):
                self.assertIn('assets/' + name, files)
        self.assertRegex(page, rf"url\('\.\./assets/avatar-preview/ui/Stop\.{HASHED}\.png'\)")
        self.assertNotIn("url('ui/", page)
        # preview.js names its modules and emotes relative to itself.
        source = (self.out / 'assets' / script[1]).read_text()
        table = json.loads(re.search(r'const FILES = (\{.*\});', source)[1])
        self.assertEqual(set(table), {'pkg/', 'pkg-gl/', 'emotes/love.glb'})
        for name, target in table.items():
            with self.subTest(name=name):
                self.assertTrue((self.out / 'assets' / target).exists())
        self.assertRegex(table['pkg/'], rf'^\./pkg\.{HASHED}/$')

    def test_module_directories_keep_their_relative_imports_and_leave_out_package_files(self):
        self.package()
        files = self.published()
        for backend in package.BACKENDS:
            directory = next(name.split('/')[1] for name in files if re.match(rf'assets/{backend}\.{HASHED}/', name))
            self.assertEqual(sorted(name for name in files if name.startswith(f'assets/{directory}/')), [
                f'assets/{directory}/content_cache.js', f'assets/{directory}/dcl_avatar_preview.js',
                f'assets/{directory}/dcl_avatar_preview_bg.wasm',
                f'assets/{directory}/snippets/platform-1/src/web_save.js'])
        self.assertFalse(any(name.endswith(('.md', '.d.ts', 'package.json', '.gitignore')) for name in files))

    def test_names_follow_content_only(self):
        self.package()
        first = self.published()
        self.package()
        self.assertEqual(self.published(), first)
        # A rebuilt WebGL2 module renames its directory and preview.js, which names it; the WebGPU
        # module, the emotes and the page's images keep their names, so browsers keep them.
        self.write('web/pkg-gl/dcl_avatar_preview_bg.wasm', b'\0asm\x01\0\0\0rebuilt')
        self.package()
        second = self.published()
        changed = sorted(set(first) ^ set(second))
        self.assertTrue(all(re.match(rf'assets/(pkg-gl\.{HASHED}/|preview\.{HASHED}\.js)', name) for name in changed), changed)
        self.assertTrue(any(name.startswith('assets/pkg-gl.') for name in changed))
        # An image changes the page alone.
        self.write('web/ui/Stop.png', b'another png')
        self.package()
        changed = sorted(set(second) ^ set(self.published()))
        self.assertTrue(all(name.startswith('assets/ui/Stop.') for name in changed), changed)

    def test_a_module_over_the_size_limit_fails_the_package(self):
        sizes = self.package(max_wasm_gzip=1000)
        self.assertEqual(set(sizes), set(package.BACKENDS))
        noise = b''.join(hashlib.sha256(bytes([i, j])).digest() for i in range(8) for j in range(256))
        self.write('web/pkg/dcl_avatar_preview_bg.wasm', b'\0asm\x01\0\0\0' + noise)
        with self.assertRaisesRegex(SystemExit, r'Over the size limit of 1000 gzip bytes per module: pkg \d+'):
            self.package(max_wasm_gzip=1000)

    def test_a_missing_module_file_or_rewrite_point_fails_the_package(self):
        (self.web / 'pkg-gl/content_cache.js').unlink()
        with self.assertRaisesRegex(SystemExit, 'Missing module file'):
            self.package()
        self.write('web/pkg-gl/content_cache.js', 'export {}')
        self.write('web/preview.js', 'export function boot() {}\n')
        with self.assertRaisesRegex(SystemExit, 'FILES'):
            self.package()
        self.write('web/preview.js', 'const FILES = null;\n')
        self.write('web/index.html', "<style>a { background: url('ui/Missing.png'); }</style>import('./preview.js')")
        with self.assertRaisesRegex(SystemExit, 'ui/Missing.png'):
            self.package()

    def test_the_tracked_page_and_script_have_their_rewrite_points(self):
        web = REPO / 'rendering/avatar-preview/web'
        self.assertEqual((web / 'preview.js').read_text().count(package.FILES_MARKER), 1)
        self.assertEqual((web / 'index.html').read_text().count(package.PREVIEW_IMPORT), 1)
        for name in re.findall(r"url\('(ui/[^']+)'\)", (web / 'index.html').read_text()):
            with self.subTest(name=name):
                self.assertTrue((web / name).is_file())


if __name__ == '__main__':
    unittest.main()
