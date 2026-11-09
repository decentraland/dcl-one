#!/usr/bin/env python3
"""Install the pinned Linux WASM binding generator and optimizer."""
import argparse
import hashlib
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import tomllib

APP = Path(__file__).resolve().parent
ROOT = APP.parents[1]
spec = importlib.util.spec_from_file_location('install_protoc', APP / 'install-protoc.py')
protoc = importlib.util.module_from_spec(spec)
spec.loader.exec_module(protoc)
TOOLS = {
    'wasm-bindgen': (
        '0.2.114',
        'https://github.com/wasm-bindgen/wasm-bindgen/releases/download/0.2.114/wasm-bindgen-0.2.114-x86_64-unknown-linux-musl.tar.gz',
        'ce21be002bdbc22ede2a5cf250cb801d2106956fc3c529c75d9452729db3490c',
        'wasm-bindgen-0.2.114-x86_64-unknown-linux-musl/wasm-bindgen',
    ),
    'wasm-opt': (
        '129',
        'https://github.com/WebAssembly/binaryen/releases/download/version_129/binaryen-version_129-x86_64-linux.tar.gz',
        '50b9fa62b9abea752da92ec57e0c555fee578760cd237c40107957715d2976ba',
        'binaryen-version_129/bin/wasm-opt',
    ),
}


def install(name, destination):
    version, url, digest, member = TOOLS[name]
    data = protoc.download(url)
    if hashlib.sha256(data).hexdigest() != digest:
        raise ValueError(f'{name} archive SHA-256 does not match the pin')
    with tarfile.open(fileobj=io.BytesIO(data), mode='r:gz') as archive:
        contents = archive.extractfile(member)
        if contents is None:
            raise ValueError(f'Missing {name} executable')
        binary = destination / name
        binary.write_bytes(contents.read())
    binary.chmod(0o755)
    actual = subprocess.check_output([str(binary), '--version'], text=True).strip()
    if actual != (f'wasm-bindgen {version}' if name == 'wasm-bindgen' else f'wasm-opt version {version}'):
        raise ValueError(f'Unexpected {name} version: {actual}')
    print(actual, flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--no-opt', action='store_true')
    args = parser.parse_args()
    lock = tomllib.loads((ROOT / 'rendering/bevy-explorer/source/Cargo.lock').read_text())
    versions = {p['version'] for p in lock['package'] if p['name'] == 'wasm-bindgen'}
    if versions != {TOOLS['wasm-bindgen'][0]}:
        raise ValueError('Update the wasm-bindgen tool pin to match Cargo.lock')
    destination = Path(tempfile.mkdtemp(prefix='bevy-web-tools-', dir=os.environ['RUNNER_TEMP']))
    install('wasm-bindgen', destination)
    if not args.no_opt:
        install('wasm-opt', destination)
    with open(os.environ['GITHUB_PATH'], 'a', encoding='utf-8') as path:
        path.write(f'{destination}\n')


if __name__ == '__main__':
    main()
