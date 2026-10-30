#!/usr/bin/env python3
"""Exercise the production images against an isolated, disposable PostgreSQL."""
import argparse
import hashlib
import json
import secrets
import subprocess
import time
import urllib.parse
from pathlib import Path

POSTGRES = 'postgres:18.6-bookworm@sha256:afc7e2d441324c0388fa80c3d24f733b4194a4eb7f47dd8ee2b08eb1a24a647c'
ROOT = Path(__file__).resolve().parents[1]


def docker(*args, check=True):
    result = subprocess.run(['docker', *args], capture_output=True, text=True)
    if check and result.returncode:
        raise RuntimeError(f'Docker {args[0]} failed: {result.stderr or result.stdout}')
    return result


def wait_for(check, description, timeout=90):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if check():
            return
        time.sleep(.5)
    raise RuntimeError(f'Timed out: {description}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--apis', default='foundation-apis:nix')
    parser.add_argument('--sites', default='foundation-sites:nix')
    parser.add_argument('--revision')
    parser.add_argument('--play', type=Path, default=ROOT / '.cache/play/current')
    args = parser.parse_args()
    for image in (args.apis, args.sites):
        config = json.loads(docker('image', 'inspect', image).stdout)[0]['Config']
        assert config['User'] == '1000:1000', 'Production images must run as the unprivileged user'
        if args.revision:
            assert config['Labels']['org.opencontainers.image.revision'] == args.revision

    name = 'foundation-image-smoke-' + secrets.token_hex(5)
    db, api, sites = (name + suffix for suffix in ('-db', '-api', '-sites'))
    volumes = [name + suffix for suffix in ('-pgdata', '-content', '-webdata')]
    password = secrets.token_hex(24)
    database = f'postgresql://foundation:{password}@db:5432/catalyst'
    logs = ROOT / '.cache/image-smoke'
    logs.mkdir(parents=True, exist_ok=True)
    docker('pull', POSTGRES)
    try:
        # No upstream data or network access is available to these services.
        docker('network', 'create', '--internal', name)
        for volume in volumes:
            docker('volume', 'create', volume)
        docker('run', '-d', '--name', db, '--network', name, '--network-alias', 'db',
               '-e', 'POSTGRES_USER=foundation', '-e', f'POSTGRES_PASSWORD={password}',
               '-e', 'POSTGRES_DB=catalyst', '-e', 'POSTGRES_INITDB_ARGS=--locale=en_US.utf8',
               '-v', f'{volumes[0]}:/var/lib/postgresql', POSTGRES)
        def sql(query):
            return docker('exec', db, 'psql', '-U', 'foundation', '-d', 'catalyst',
                          '-AtX', '-v', 'ON_ERROR_STOP=1', '-c', query)
        wait_for(lambda: docker('exec', db, 'pg_isready', '-U', 'foundation', '-d', 'catalyst',
                                 '-h', '127.0.0.1', check=False).returncode == 0, 'PostgreSQL')
        isolated = ['--network', name, '--read-only', '--tmpfs', '/tmp:rw,size=64m',
                    '--cap-drop', 'ALL', '--security-opt', 'no-new-privileges']
        schema = ['run', '--rm', *isolated, '-e', f'DATABASE_URL={database}', args.apis, 'schema']
        docker(*schema, 'apply')
        sql("CREATE TABLE public.image_smoke(value text); INSERT INTO public.image_smoke VALUES ('preserved')")
        assert docker(*schema, 'apply', check=False).returncode != 0, 'A second bootstrap must be refused'
        docker(*schema, 'migrate')
        env = {
            'DATABASE_URL': database, 'NODE_POSTURE': 'journey', 'SERVE_UNSYNCED': 'true',
            'LIVEKIT_ALLOW_DEV_CREDS': 'true', 'LIVEKIT_HOST': '127.0.0.1:7880',
            'ENCRYPTION_KEY': secrets.token_hex(32), 'SCENE_HOST_ENABLED': 'false',
            'PG_POOL_SIZE': '8', 'PUBLIC_URL': 'http://localhost:5212',
        }
        api_env = [item for key, value in env.items() for item in ('-e', f'{key}={value}')]
        docker('run', '-d', '--name', api, '--network-alias', 'api', *isolated,
               '-v', f'{volumes[1]}:/data', *api_env, args.apis)
        docker('run', '-d', '--name', sites, '--network-alias', 'sites', *isolated,
               '-v', f'{volumes[2]}:/data', '-e', 'CATALYST_URL=http://api:5210',
               '-e', f'CATALYST_DATABASE_URL={database}', '-e', f'SESSION_SECRET={secrets.token_hex(32)}',
               args.sites)
        # Internal networks need no host port forwarding. Use the shipped Node
        # runtime as the HTTP client, exercising DNS and container listeners.
        def get(url, body=False):
            result = docker('exec', sites, 'node', '-e', '''
                fetch(process.argv[1], {signal: AbortSignal.timeout(30000)}).then(async r => {
                    if (!r.ok) process.exit(1);
                    const data = Buffer.from(await r.arrayBuffer());
                    console.log(JSON.stringify({bytes: data.length,
                        sha256: require('crypto').createHash('sha256').update(data).digest('hex'),
                        body: process.argv[2] === 'json' ? JSON.parse(data.toString()) : null}));
                }).catch(() => process.exit(1));
            ''', url, 'json' if body else '', check=False)
            return json.loads(result.stdout) if result.returncode == 0 else None
        api_url, sites_url = 'http://api:5210', 'http://sites:5212'
        wait_for(lambda: get(api_url + '/about'), 'API HTTP readiness')
        wait_for(lambda: get(sites_url + '/'), 'Sites HTTP readiness')
        assert get(api_url + '/places/api/places?limit=1'), 'Places must read the database'
        assert get(sites_url + '/docs/index.md'), 'Docs Markdown must ship in the image'
        media = json.loads((ROOT / 'apps/sites/content/docs/_media.json').read_text())
        path, entry = next(iter(media.items()))
        body = get(sites_url + urllib.parse.quote(path, safe='/'))
        assert body and body['bytes'] == entry['bytes'], 'Docs media must contain its bytes, not Git LFS pointers'
        manifest = (args.play / 'sites-play.json').read_bytes()
        delivered = get(sites_url + '/play/sites-play.json')
        assert delivered and delivered['sha256'] == hashlib.sha256(manifest).hexdigest(), 'Wrong Explorer build manifest'
        play = json.loads(manifest)
        # Compare actual HTTP bytes to the verified producer, including both WASM
        # modules and the worker/bridge that must share their protocol.
        for path in ('engine/pkg/webgpu_build_bg.wasm', 'engine/pkg/webgpu_build.js',
                     'engine/pkg-scene/dcl_scene_wasm_bg.wasm', 'bridge-scene/static/BevyExplorerUI/about',
                     'service_worker.js', 'preview_realm.js'):
            delivered = get(sites_url + '/play/' + path)
            assert delivered and delivered['sha256'] == play['files'][path], f'Sites serves different Explorer bytes: {path}'
        print(f"Sites serves verified Bevy producer {play['build_revision']}")
        realm = get(sites_url + '/play/bridge/about', body=True)
        assert realm, 'Sites must expose the bundled bridge realm'
        urn = realm['body']['configurations']['scenesUrn'][0]
        entity, base = urn.removeprefix('urn:decentraland:entity:').split('?=&baseUrl=', 1)
        assert base == sites_url + '/play/bridge-scene/static/', 'Bridge content must resolve under this Sites origin'
        delivered = get(base + entity)
        assert delivered and delivered['sha256'] == play['files']['bridge-scene/static/' + entity], 'Bridge entity is not the compiled bundle'
        docker('restart', db)
        wait_for(lambda: docker('exec', db, 'pg_isready', '-U', 'foundation', '-d', 'catalyst',
                                 '-h', '127.0.0.1', check=False).returncode == 0, 'restarted PostgreSQL')
        wait_for(lambda: get(api_url + '/places/api/places?limit=1'), 'API database reconnection')
        docker('restart', api)
        wait_for(lambda: get(api_url + '/about'), 'restarted API')
        assert sql('SELECT value FROM public.image_smoke').stdout.strip() == 'preserved'
        print('Image smoke passed: non-root, read-only root, schema guards, API/Sites/docs/media, database and API restart.')
    finally:
        for container, label in ((api, 'api'), (sites, 'sites'), (db, 'postgres')):
            result = docker('logs', container, check=False)
            (logs / f'{label}.log').write_text(result.stdout + result.stderr)
            docker('rm', '-f', container, check=False)
        docker('network', 'rm', name, check=False)
        for volume in volumes:
            docker('volume', 'rm', volume, check=False)


if __name__ == '__main__':
    main()
