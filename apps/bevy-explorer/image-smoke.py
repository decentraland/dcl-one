#!/usr/bin/env python3
"""Exercise the exact local Bevy image without host ports or external networking."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time
import uuid


def docker(*args, check=True):
    return subprocess.run(['docker', *args], check=check, text=True, capture_output=True, timeout=120)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('product', choices=('web', 'headless'))
    parser.add_argument('--revision', required=True)
    parser.add_argument('--arch', required=True, choices=('amd64', 'arm64'))
    args = parser.parse_args()
    image = f'bevy-explorer-{args.product}:ci'
    info = json.loads(docker('image', 'inspect', image).stdout)[0]
    assert info['Architecture'] == args.arch and info['Os'] == 'linux'
    assert info['Config']['Labels']['org.opencontainers.image.revision'] == args.revision
    assert info['Config']['User'] in ('101:101', '1000:1000')
    name = 'bevy-image-smoke-' + uuid.uuid4().hex[:12]
    logs = Path(__file__).parent / 'dist' / f'image-smoke-{args.product}.log'
    logs.parent.mkdir(parents=True, exist_ok=True)
    common = ['--name', name, '--network', 'none', '--read-only', '--cap-drop', 'ALL',
              '--security-opt', 'no-new-privileges', '--tmpfs', '/tmp:rw,nosuid,size=512m']
    try:
        if args.product == 'headless':
            result = docker('run', *common, '--tmpfs', '/data:rw,nosuid,uid=1000,gid=1000', image,
                            '--realm', 'http://127.0.0.1:1', '--preview', '--server-mode', '--timeout', '20', check=False)
            output = result.stdout + result.stderr
            logs.write_text(output)
            if result.returncode:
                raise AssertionError(f'Headless exited with {result.returncode}:\n{output[-4000:]}')
            assert '[headless]' in output and 'panicked at' not in output, output[-4000:]
        else:
            docker('run', '-d', *common, image)
            for _ in range(30):
                ready = docker('exec', name, 'wget', '-q', '-O', '-', 'http://127.0.0.1:8080/healthz', check=False)
                if ready.returncode == 0:
                    break
                time.sleep(1)
            else:
                raise AssertionError('Web image never became ready')
            headers = {}
            for path in ('/', '/app.html', '/engine/headless.html', '/service_worker.js'):
                response = docker('exec', name, 'wget', '-S', '-O', '/dev/null', f'http://127.0.0.1:8080{path}')
                headers[path] = response.stderr.lower()
            assert 'cross-origin-opener-policy:' not in headers['/']
            assert 'document-isolation-policy: isolate-and-credentialless' in headers['/app.html']
            assert 'document-isolation-policy: isolate-and-credentialless' in headers['/engine/headless.html']
            assert 'cross-origin-embedder-policy: credentialless' in headers['/service_worker.js']
            for path in ('engine/pkg/webgpu_build_bg.wasm', 'engine/pkg-scene/dcl_scene_wasm_bg.wasm'):
                docker('exec', name, 'wget', '-q', '-O', '/tmp/smoke.wasm', f'http://127.0.0.1:8080/{path}')
                actual = docker('exec', name, 'sha256sum', '/tmp/smoke.wasm').stdout.split()[0]
                expected = hashlib.sha256((Path(__file__).parent / 'dist/web' / path).read_bytes()).hexdigest()
                assert actual == expected, f'WASM bytes differ: {path}'
            logs.write_text(docker('logs', name).stdout + '\n' + json.dumps(headers, indent=2))
        print(f'Image smoke passed: bevy-explorer-{args.product}, linux/{args.arch}, non-root, read-only, network isolated')
    finally:
        if not logs.exists():
            result = docker('logs', name, check=False)
            logs.write_text(result.stdout + result.stderr)
        docker('rm', '-f', name, check=False)


if __name__ == '__main__':
    main()
