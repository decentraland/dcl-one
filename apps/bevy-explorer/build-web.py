#!/usr/bin/env python3
"""Build the Explorer web engine, HUD and bridge scene."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
import tomllib

ENVIRONMENT = ('environment: WASM_PACK_MODE is passed to wasm-pack as --mode. The value no-install uses the '
               'wasm-bindgen on PATH instead of a download, for a sandboxed or offline build. '
               'No workflow of this tree sets it; the person who runs the build does.')
APP = Path(__file__).resolve().parent
ROOT = APP.parents[1]
SOURCE = ROOT / 'rendering/bevy-explorer/source'
PREVIEW = ROOT / 'rendering/avatar-preview'
# The largest gzip -9 size an optimized avatar preview module may have: every Shop and creator page
# that shows an avatar downloads one (the WebGPU module or the WebGL2 one).
PREVIEW_MAX_WASM_GZIP = 10_000_000
spec = importlib.util.spec_from_file_location('play_artifact', ROOT / 'tooling/play-artifact.py')
artifact = importlib.util.module_from_spec(spec)
spec.loader.exec_module(artifact)


def git_bash():
    # `bash` on a Windows PATH is often WSL's launcher (System32). Git for Windows' own bash sits at
    # <root>/bin/bash.exe, which puts Git's tools on PATH; `git --exec-path` is under <root>.
    exec_path = subprocess.run(('git', '--exec-path'), capture_output=True, text=True, check=True).stdout
    for parent in Path(exec_path.strip()).parents:
        if (parent / 'bin/bash.exe').is_file():
            return str(parent / 'bin/bash.exe')
    raise SystemExit('Git Bash not found: install Git for Windows (scripts/gen-ts-bindings.sh needs bash)')


def executable(name):
    # Windows starts corepack (corepack.cmd) only by its full path, not by the bare name.
    if os.name != 'nt':
        return name
    return git_bash() if name == 'bash' else shutil.which(name) or name


def pnpm():
    # A shell with pnpm on its PATH sets PNPM=pnpm (nix develop .#wasm does), as the justfile reads it.
    return tuple(os.environ.get('PNPM', '').split()) or ('corepack', 'pnpm@12.21.0')


def run(*args, cwd=SOURCE, env=None):
    started = time.monotonic()
    subprocess.run((executable(str(args[0])), *args[1:]), cwd=cwd, env=env, check=True)
    print(f'{" ".join(map(str, args))}: {time.monotonic() - started:.1f}s', flush=True)


def build_environment():
    return {**os.environ, 'RUSTUP_TOOLCHAIN': 'nightly-2026-04-15',
            'RAYON_NUM_THREADS': os.environ.get('RAYON_NUM_THREADS', str(min(os.cpu_count() or 1, 16)))}


def cargo_wasm(no_opt, features, env):
    """Compile both modules in one feature graph; returns the directory that holds the two .wasm files."""
    if no_opt:
        env = {**env, 'CARGO_PROFILE_RELEASE_CODEGEN_UNITS': '16'}
    run('cargo', 'build', '--locked', '--release', '--target', 'wasm32-unknown-unknown',
        '--lib', '-p', 'decentra-bevy', '-p', 'dcl_scene_wasm', '--no-default-features',
        '--features=' + ','.join(f'decentra-bevy/{feature}' for feature in features), env=env)
    return Path(env.get('CARGO_TARGET_DIR', SOURCE / 'target')) / 'wasm32-unknown-unknown/release'


def compile_wasm(no_opt, features, env):
    target = cargo_wasm(no_opt, features, env)
    for name, directory in (('webgpu_build', 'pkg'), ('dcl_scene_wasm', 'pkg-scene')):
        output = SOURCE / 'deploy/web/engine' / directory
        run('wasm-bindgen', str(target / f'{name}.wasm'), '--target', 'web',
            '--typescript', '--out-dir', str(output), env=env)
        if not no_opt:
            wasm = output / f'{name}_bg.wasm'
            optimized = wasm.with_suffix('.opt.wasm')
            run('wasm-opt', str(wasm), '-O', '-o', str(optimized), env=env)
            optimized.replace(wasm)


def prepare_engine(no_opt, engine_cache, shared_build, features, env):
    flags = ['--no-opt'] if no_opt else []
    # WASM_PACK_MODE=no-install needs the Cargo.lock's wasm-bindgen-cli installed (see ENVIRONMENT).
    if mode := os.environ.get('WASM_PACK_MODE'):
        flags += ['--mode', mode]
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
        if shared_build:
            compile_wasm(no_opt, features, env)
        else:
            run('wasm-pack', 'build', '--target', 'web', *flags, '--out-dir', './deploy/web/engine/pkg',
                '--no-default-features', '--features=' + ','.join(features), env=env)
            run('wasm-pack', 'build', 'crates/dcl_scene_wasm', '--target', 'web', *flags,
                '--out-dir', '../../deploy/web/engine/pkg-scene', '--no-default-features', env=env)
        pkg = SOURCE / 'deploy/web/engine/pkg'
        (pkg / '.gitignore').unlink(missing_ok=True)
        (pkg / 'manifest.json').write_text(json.dumps({'wasmSize': (pkg / 'webgpu_build_bg.wasm').stat().st_size}))
        (SOURCE / 'deploy/web/engine/pkg-scene/.gitignore').unlink(missing_ok=True)
        run('bash', './scripts/gen-ts-bindings.sh', env=env)
        engine = {'inputs': artifact.engine_inputs(ROOT), 'revision': revision, 'optimized': not no_opt}
        if engine_cache:
            artifact.pack_engine(engine_cache, ROOT, optimized=not no_opt, revision=revision)
    return engine


def compile_preview(no_opt, env):
    """build.sh's two modules with the pinned binding generator and optimizer, in its own target."""
    # Unoptimized (a PR's build) skips only wasm-opt, so it compiles as main does and reuses main's target.
    env = {name: value for name, value in env.items() if name != 'CARGO_TARGET_DIR'}
    manifest = tomllib.loads((PREVIEW / 'Cargo.toml').read_text())
    wasm_opt = manifest['package']['metadata']['wasm-pack']['profile']['release']['wasm-opt']
    module = PREVIEW / 'target/wasm32-unknown-unknown/release/dcl_avatar_preview.wasm'
    for part, features in (('pkg', ()), ('pkg-gl', ('--no-default-features', '--features', 'webgl2'))):
        run('cargo', 'build', '--locked', '--release', '--target', 'wasm32-unknown-unknown', '--lib',
            *features, cwd=PREVIEW, env=env)
        output = ROOT / artifact.PREVIEW_PARTS[part]
        run('wasm-bindgen', str(module), '--target', 'web', '--out-dir', str(output), cwd=PREVIEW, env=env)
        if not no_opt:
            wasm = output / 'dcl_avatar_preview_bg.wasm'
            optimized = wasm.with_suffix('.opt.wasm')
            run('wasm-opt', str(wasm), *wasm_opt, '-o', str(optimized), cwd=PREVIEW, env=env)
            optimized.replace(wasm)
        shutil.copy2(SOURCE / 'deploy/web/engine/content_cache.js', output / 'content_cache.js')


def prepare_preview(no_opt, preview_cache, shared_build, env):
    """The avatar preview's modules, compiled from this tree's engine crates or restored for the same inputs."""
    if preview_cache and preview_cache.exists():
        preview = artifact.restore_preview(preview_cache, ROOT, allow_unoptimized=no_opt)
        print(f"Reusing the compiled avatar preview from {preview['revision']}", flush=True)
        return preview
    for part in artifact.PREVIEW_PARTS.values():
        if (ROOT / part).exists():
            shutil.rmtree(ROOT / part)
    if shared_build:
        compile_preview(no_opt, env)
    else:
        run('bash', './build.sh', *(['--fast'] if no_opt else []), cwd=PREVIEW, env=env)
    revision = os.environ.get('GITHUB_SHA', 'local')
    if preview_cache:
        return artifact.pack_preview(preview_cache, ROOT, optimized=not no_opt, revision=revision)
    return {'inputs': artifact.preview_inputs(ROOT), 'revision': revision, 'optimized': not no_opt}


def web_features(performance_profiler):
    return ['livekit', 'social', *(['performance-profiler'] if performance_profiler else [])]


def web(no_opt=False, engine_cache=None, shared_build=False, performance_profiler=False, preview_cache=None):
    # the release engine leaves out `performance-profiler`: its spans and allocation counter cost
    # every frame even while the profiler records nothing
    features = web_features(performance_profiler)
    env = build_environment()
    revision = os.environ.get('GITHUB_SHA', 'local')
    with ThreadPoolExecutor(max_workers=2) as pool:
        installations = [
            pool.submit(run, *pnpm(), '--dir', 'react-web',
                        'install', '--frozen-lockfile', '--ignore-scripts', env=env),
            pool.submit(run, *pnpm(), '--dir', 'react-web/bridge-scene',
                        'install', '--frozen-lockfile', '--ignore-scripts', env=env),
        ]
        engine = prepare_engine(no_opt, engine_cache, shared_build, features, env)
        preview = prepare_preview(no_opt, preview_cache, shared_build, env)
        for installation in installations:
            installation.result()
    run('node', './react-web/scripts/shared-inputs.mjs', '--check', env=env)
    metadata = SOURCE / 'target/sandbox-worker.meta.json'
    metadata.parent.mkdir(exist_ok=True)
    run('node', './react-web/node_modules/esbuild/bin/esbuild', './deploy/web/engine/sandbox_worker.js',
        '--bundle', '--format=esm', '--log-level=error', f'--metafile={metadata}',
        '--outfile=./deploy/web/engine/pkg/sandbox_worker.bundle.js', env=env)
    entries = [o for o in json.loads(metadata.read_text())['outputs'].values() if o.get('entryPoint')]
    if len(entries) != 1 or entries[0]['exports']:
        raise SystemExit('Sandbox worker must have one entry point and export nothing')
    # Relative URLs allow the same build at / or a gateway subpath. No upstream CDN stamping.
    web_env = {**env, 'PUBLIC_URL': '', 'SCENE_BASE_URL': '../'}
    run(*pnpm(), '--dir', 'react-web', 'run', 'build', env=web_env)
    run(*pnpm(), '--dir', 'react-web/bridge-scene',
        '--filter', '@dcl/hashing', 'build', env=web_env)
    run('node', str(ROOT / 'tooling/check-local-hashing.mjs'),
        str(SOURCE / 'react-web/bridge-scene'), env=web_env)
    # Node 25+ ships a localStorage global (empty without --localstorage-file) that
    # vitest keeps over jsdom's; CI runs Node 24, so the tests see it switched off.
    node_options = ' '.join(filter(None, [env.get('NODE_OPTIONS'), '--no-experimental-webstorage']))
    run(*pnpm(), '--dir', 'react-web', 'run', 'test', env={**web_env, 'NODE_OPTIONS': node_options})
    run(*pnpm(), '--dir', 'react-web/bridge-scene', 'run', 'bundle', env=web_env)
    output = APP / 'dist/web'
    if output.exists():
        shutil.rmtree(output)
    shutil.copytree(SOURCE / 'deploy/web', output,
                    ignore=shutil.ignore_patterns('node_modules', '.env', '.gitignore', 'package*.json', 'scripts', '*.md'))
    for name in ('index.html', 'app.html', 'engine/pkg/webgpu_build_bg.wasm',
                 'engine/pkg-scene/dcl_scene_wasm_bg.wasm', 'service_worker.js'):
        if not (output / name).is_file():
            raise SystemExit(f'Missing web build output: {name}')
    # Sites serves the page at /avatar-preview/ and its content-hashed files at /assets/avatar-preview/.
    run(sys.executable, str(PREVIEW / 'tools/package.py'), str(output / 'avatar-preview'),
        *(['--max-wasm-gzip', str(PREVIEW_MAX_WASM_GZIP)] if preview['optimized'] else []), cwd=ROOT)
    shutil.copy2(SOURCE / 'LICENSE', output / 'LICENSE')
    (output / 'build-info.json').write_text(json.dumps({'component': 'bevy-explorer-web',
        'revision': revision, 'engine': engine, 'preview': preview}) + '\n')
    run(sys.executable, str(ROOT / 'tooling/play-artifact.py'), 'pack', str(output),
        *([] if engine['optimized'] and preview['optimized'] else ['--unoptimized']), cwd=ROOT)
    print(output)


def main():
    parser = argparse.ArgumentParser(description=__doc__, epilog=ENVIRONMENT)
    parser.add_argument('--no-opt', action='store_true', help='Skip wasm-opt for development/PR builds')
    parser.add_argument('--engine-cache', type=Path,
                        help='CI-only compiled WASM cache, restored with an exact input key')
    parser.add_argument('--preview-cache', type=Path,
                        help='CI-only compiled avatar preview cache, restored with an exact input key')
    parser.add_argument('--shared-build', action='store_true',
                        help='Compile both WASM modules in one Cargo feature graph; requires wasm-bindgen and wasm-opt')
    parser.add_argument('--performance-profiler', action='store_true',
                        help="Compile in the Profiler section's spans and allocation counter (off in release builds)")
    parser.add_argument('--target-only', action='store_true',
                        help='Run only the Cargo steps of a shared build (the two modules and the bindings '
                             'test build) and stop; CI fills a target cache with them')
    args = parser.parse_args()
    if args.performance_profiler and args.engine_cache:
        parser.error('--performance-profiler builds a different engine than the cache holds')
    if args.target_only:
        if args.engine_cache or args.shared_build:
            parser.error('--target-only packs no engine: it takes neither --engine-cache nor --shared-build')
        env = build_environment()
        cargo_wasm(args.no_opt, web_features(args.performance_profiler), env)
        # The bindings script compiles a test build in the same target; a build that restores
        # the target without it compiles that again.
        run('bash', './scripts/gen-ts-bindings.sh', env=env)
        return
    web(args.no_opt, args.engine_cache.resolve() if args.engine_cache else None, args.shared_build,
        args.performance_profiler, args.preview_cache.resolve() if args.preview_cache else None)


if __name__ == '__main__':
    main()
