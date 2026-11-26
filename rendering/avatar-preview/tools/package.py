#!/usr/bin/env python3
"""Lays out a built preview for publication under content-hashed names.

    python3 tools/package.py <out> [--modules <dir>] [--max-wasm-gzip <bytes>]

<out>/index.html is the page, the one file a browser revalidates; Sites serves it at
/avatar-preview/. Everything it loads goes to <out>/assets/, which Sites serves at
/assets/avatar-preview/ as immutable: each backend's module directory (pkg/, pkg-gl/ from
build.sh, read from --modules, default web/) is named by a hash of its contents, since the
wasm-bindgen glue imports its snippets by relative path, and every other file by a hash of its
bytes. The page and preview.js are rewritten to those names. --max-wasm-gzip fails the package
when a module compresses (gzip -9) to more bytes than that.
"""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import re
import shutil

HERE = Path(__file__).resolve().parent.parent
WEB = HERE / 'web'
BACKENDS = ('pkg', 'pkg-gl')
MODULE = 'dcl_avatar_preview'
# What the page loads from a module directory; wasm-pack's package files and types stay out.
MODULE_FILES = (f'{MODULE}.js', f'{MODULE}_bg.wasm', 'content_cache.js')
STATIC = ('emotes', 'ui')
STATIC_SUFFIXES = ('.glb', '.png', '.otf')
# The page's path is /avatar-preview/, its assets' /assets/avatar-preview/.
ASSETS_FROM_PAGE = '../assets/avatar-preview/'
FILES_MARKER = 'const FILES = null;'
PREVIEW_IMPORT = "import('./preview.js')"


def digest(data):
    return hashlib.sha256(data).hexdigest()[:16]


def hashed(name, data):
    stem, dot, suffix = name.rpartition('.')
    return f'{stem}.{digest(data)}.{suffix}' if dot else f'{name}.{digest(data)}'


def module_files(directory):
    files = [directory / name for name in MODULE_FILES]
    snippets = directory / 'snippets'
    if snippets.is_dir():
        files += sorted(path for path in snippets.rglob('*') if path.is_file())
    for path in files:
        if not path.is_file() or path.is_symlink() or not path.stat().st_size:
            raise SystemExit(f'Missing module file: {path}')
    return files


def wasm_gzip_size(path):
    return len(gzip.compress(path.read_bytes(), compresslevel=9, mtime=0))


def package(out, modules=WEB, web=WEB, max_wasm_gzip=None):
    if out.exists():
        shutil.rmtree(out)
    assets = out / 'assets'
    assets.mkdir(parents=True)
    # What preview.js loads (its modules and emotes) and what the page itself does (its images and font).
    names = {}
    page_names = {}
    sizes = {}
    for backend in BACKENDS:
        source = modules / backend
        files = module_files(source)
        tree = json.dumps([(path.relative_to(source).as_posix(), digest(path.read_bytes())) for path in files])
        target = f'{backend}.{digest(tree.encode())}/'
        for path in files:
            destination = assets / target / path.relative_to(source)
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, destination)
        names[f'{backend}/'] = './' + target
        wasm = source / f'{MODULE}_bg.wasm'
        sizes[backend] = {'wasm': wasm.stat().st_size, 'gzip': wasm_gzip_size(wasm)}
    for folder in STATIC:
        for path in sorted((web / folder).iterdir()):
            if path.suffix not in STATIC_SUFFIXES:
                continue
            data = path.read_bytes()
            name = f'{folder}/{hashed(path.name, data)}'
            (assets / name).parent.mkdir(parents=True, exist_ok=True)
            (assets / name).write_bytes(data)
            (page_names if folder == 'ui' else names)[f'{folder}/{path.name}'] = './' + name
    script = (web / 'preview.js').read_text()
    if script.count(FILES_MARKER) != 1:
        raise SystemExit(f'preview.js must declare `{FILES_MARKER}` once')
    script = script.replace(FILES_MARKER, f'const FILES = {json.dumps(names, sort_keys=True)};')
    preview = hashed('preview.js', script.encode())
    (assets / preview).write_text(script)
    page = (web / 'index.html').read_text()
    if page.count(PREVIEW_IMPORT) != 1:
        raise SystemExit(f'index.html must load the script with `{PREVIEW_IMPORT}` once')
    page = page.replace(PREVIEW_IMPORT, f"import('{ASSETS_FROM_PAGE}{preview}')")

    def asset(match):
        name = page_names.get(match[2])
        if not name:
            raise SystemExit(f'index.html names a file the package does not have: {match[2]}')
        return f"{match[1]}{ASSETS_FROM_PAGE}{name.removeprefix('./')}{match[3]}"

    page = re.sub(r"(url\(')(ui/[^']+)('\))", asset, page)
    (out / 'index.html').write_text(page)
    over = {backend: size['gzip'] for backend, size in sizes.items()
            if max_wasm_gzip is not None and size['gzip'] > max_wasm_gzip}
    for backend, size in sizes.items():
        print(f"{backend}: {size['wasm']} bytes, {size['gzip']} gzip -9")
    if over:
        raise SystemExit(f'Over the size limit of {max_wasm_gzip} gzip bytes per module: '
                         + ', '.join(f'{backend} {size}' for backend, size in over.items()))
    return sizes


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('out', type=Path)
    parser.add_argument('--modules', type=Path, default=WEB, help='the directory holding pkg/ and pkg-gl/')
    parser.add_argument('--max-wasm-gzip', type=int, help='the largest gzip -9 size a module may have, in bytes')
    args = parser.parse_args()
    package(args.out.resolve(), args.modules.resolve(), max_wasm_gzip=args.max_wasm_gzip)


if __name__ == '__main__':
    main()
