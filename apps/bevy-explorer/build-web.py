#!/usr/bin/env python3
"""Build the Explorer web engine, HUD and bridge scene."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

APP = Path(__file__).resolve().parent
ROOT = APP.parents[1]
SOURCE = ROOT / 'rendering/bevy-explorer/source'
spec = importlib.util.spec_from_file_location('play_artifact', ROOT / 'tooling/play-artifact.py')
artifact = importlib.util.module_from_spec(spec)
spec.loader.exec_module(artifact)


def run(*args, cwd=SOURCE, env=None):
    subprocess.run(args, cwd=cwd, env=env, check=True)


def web(no_opt=False, engine_cache=None):
    env = {**os.environ, 'RUSTUP_TOOLCHAIN': 'nightly-2026-04-15'}
    flags = ['--no-opt'] if no_opt else []
    revision = os.environ.get('GITHUB_SHA', 'local')
    if engine_cache and engine_cache.exists():
        engine = artifact.restore_engine(engine_cache, ROOT, allow_unoptimized=no_opt)
        print(f"Reusing compiled WASM from {engine['revision']}", flush=True)
    else:
        # Only generated outputs: stale workers/bindings must not enter a new cache.
        for part in artifact.ENGINE_PARTS.values():
            target = ROOT / part
            if target.exists():
                shutil.rmtree(target)
        run('wasm-pack', 'build', '--target', 'web', *flags, '--out-dir', './deploy/web/engine/pkg',
            '--no-default-features', '--features=livekit,social', env=env)
        pkg = SOURCE / 'deploy/web/engine/pkg'
        (pkg / '.gitignore').unlink(missing_ok=True)
        (pkg / 'manifest.json').write_text(json.dumps({'wasmSize': (pkg / 'webgpu_build_bg.wasm').stat().st_size}))
        run('wasm-pack', 'build', 'crates/dcl_scene_wasm', '--target', 'web', *flags,
            '--out-dir', '../../deploy/web/engine/pkg-scene', '--no-default-features', env=env)
        (SOURCE / 'deploy/web/engine/pkg-scene/.gitignore').unlink(missing_ok=True)
        run('bash', './scripts/gen-ts-bindings.sh', env=env)
        engine = {'inputs': artifact.engine_inputs(ROOT), 'revision': revision, 'optimized': not no_opt}
        if engine_cache:
            artifact.pack_engine(engine_cache, ROOT, optimized=not no_opt, revision=revision)
    metadata = SOURCE / 'target/sandbox-worker.meta.json'
    metadata.parent.mkdir(exist_ok=True)
    run('npx', '--yes', 'esbuild@0.28.1', './deploy/web/engine/sandbox_worker.js',
        '--bundle', '--format=esm', '--log-level=error', f'--metafile={metadata}',
        '--outfile=./deploy/web/engine/pkg/sandbox_worker.bundle.js', env=env)
    entries = [o for o in json.loads(metadata.read_text())['outputs'].values() if o.get('entryPoint')]
    if len(entries) != 1 or entries[0]['exports']:
        raise SystemExit('Sandbox worker must have one entry point and export nothing')
    run('npm', 'ci', '--prefix', 'react-web', env=env)
    run('npm', 'ci', '--prefix', 'react-web/bridge-scene', env=env)
    # Relative URLs allow the same build at / or a gateway subpath. No upstream CDN stamping.
    web_env = {**env, 'PUBLIC_URL': '', 'SCENE_BASE_URL': '/bridge-scene/static/'}
    run('npm', 'run', 'build', '--prefix', 'react-web', env=web_env)
    run('npm', 'run', 'test', '--prefix', 'react-web', env=web_env)
    run('npm', 'run', 'bundle', '--prefix', 'react-web/bridge-scene', env=web_env)
    output = APP / 'dist/web'
    if output.exists():
        shutil.rmtree(output)
    shutil.copytree(SOURCE / 'deploy/web', output,
                    ignore=shutil.ignore_patterns('node_modules', '.env', '.gitignore', 'package*.json', 'scripts', '*.md'))
    for name in ('index.html', 'app.html', 'engine/pkg/webgpu_build_bg.wasm',
                 'engine/pkg-scene/dcl_scene_wasm_bg.wasm', 'service_worker.js'):
        if not (output / name).is_file():
            raise SystemExit(f'Missing web build output: {name}')
    shutil.copy2(SOURCE / 'LICENSE', output / 'LICENSE')
    (output / 'build-info.json').write_text(json.dumps({'component': 'bevy-explorer-web',
        'revision': revision, 'engine': engine}) + '\n')
    run(sys.executable, str(ROOT / 'tooling/play-artifact.py'), 'pack', str(output),
        *([] if engine['optimized'] else ['--unoptimized']), cwd=ROOT)
    print(output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--no-opt', action='store_true', help='Skip wasm-opt for development/PR builds')
    parser.add_argument('--engine-cache', type=Path,
                        help='CI-only compiled WASM cache, restored with an exact input key')
    args = parser.parse_args()
    web(args.no_opt, args.engine_cache.resolve() if args.engine_cache else None)


if __name__ == '__main__':
    main()
