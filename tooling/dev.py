#!/usr/bin/env python3
"""Local Foundation services; invoked by the Nix shell's foundation-dev command."""
import argparse
import base64
import errno
import fcntl
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import secrets
import shlex
import shutil
import signal
import socket
import subprocess
import sys
import textwrap
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
BACKEND = ROOT / 'services/catalyst'
STATE = ROOT / '.cache/dev'
SETTINGS = STATE / 'settings.json'
DARWIN = platform.system() == 'Darwin'


def run(argv, **kwargs):
    return subprocess.run([str(a) for a in argv], check=True, **kwargs)


def save_json(path, value):
    temporary = path.with_suffix('.tmp')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    temporary.replace(path)


def ports_free(base):
    sockets = []
    try:
        # TCP (NATS on base+8 and +9), then the UDP ports: Pulse's WebTransport (the Sites port) and ENet ports,
        # LiveKit's media.
        for kind, port in [*((socket.SOCK_STREAM, port) for port in (base, base + 2, base + 3, base + 4, base + 5,
                                                                      base + 7, base + 8, base + 9)),
                           *((socket.SOCK_DGRAM, port) for port in (base + 2, base + 6, base + 7))]:
            sock = socket.socket(socket.AF_INET, kind)
            sockets.append(sock)
            sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            sock.bind(('127.0.0.1', port))
        return True
    except OSError as error:
        if error.errno == errno.EADDRINUSE:
            return False
        raise
    finally:
        for sock in sockets:
            sock.close()


def livekit_credentials():
    # The stack's own LiveKit server holds no data, so a stack created before it gets a pair on its next start.
    return {'LIVEKIT_API_KEY': 'API' + secrets.token_hex(6), 'LIVEKIT_API_SECRET': secrets.token_hex(32)}


def missing_secrets(config):
    """The generated secrets `config` lacks: the LiveKit pair, and the moderator token the API checks and Pulse
    polls its ban snapshots with (`catalyst secrets init` derives the same pair for a deployment)."""
    added = {}
    if 'LIVEKIT_API_SECRET' not in config:
        added.update(livekit_credentials())
    if 'MODERATOR_TOKEN' not in config:
        added['MODERATOR_TOKEN'] = secrets.token_hex(32)
    return added


def settings(create=False):
    if SETTINGS.exists():
        config = json.loads(SETTINGS.read_text())
        requested = os.environ.get('FOUNDATION_DEV_PORT')
        if create and requested is not None and requested != str(config['port']):
            print(f"Using saved port {config['port']}; FOUNDATION_DEV_PORT={requested} applies only on first startup. "
                  'Stop services before editing .cache/dev/settings.json.', file=sys.stderr)
        if missing_secrets(config):
            # A stack from before a generated secret, on whichever command reads it first; the lock
            # keeps tasks that start together on one value.
            with SETTINGS.with_suffix('.lock').open('w') as lock:
                fcntl.flock(lock, fcntl.LOCK_EX)
                config = json.loads(SETTINGS.read_text())
                if added := missing_secrets(config):
                    config.update(added)
                    save_json(SETTINGS, config)
        return config
    data = STATE / 'postgres'
    if data.exists() and any(data.iterdir()):
        raise RuntimeError('settings.json is missing but PostgreSQL data exists. Restore the original '
                           '.cache/dev/settings.json with its database password and encryption/session keys. '
                           'Data is preserved; no new credentials were generated.')
    if not create:
        raise RuntimeError('Run foundation-dev up first.')
    base = int(os.environ.get('FOUNDATION_DEV_PORT', '5210'))
    if not 1024 <= base <= 65000:
        raise ValueError('FOUNDATION_DEV_PORT must be between 1024 and 65000')
    if 'FOUNDATION_DEV_PORT' not in os.environ:
        while base <= 65000 and not ports_free(base):
            base += 10
    if base > 65000 or not ports_free(base):
        raise RuntimeError('Development ports are occupied; choose FOUNDATION_DEV_PORT.')
    profile = os.environ.get('FOUNDATION_DEV_PROFILE', 'mirror')
    if profile not in ('mirror', 'offline'):
        raise ValueError('FOUNDATION_DEV_PROFILE must be mirror or offline')
    config = {'port': base, 'profile': profile, 'password': secrets.token_hex(24),
              'ENCRYPTION_KEY': secrets.token_hex(32), 'SESSION_SECRET': secrets.token_hex(32),
              **missing_secrets({})}
    save_json(SETTINGS, config)
    return config


def database_url(config, database='catalyst'):
    return f"postgresql://foundation:{config['password']}@127.0.0.1:{config['port'] + 3}/{database}"


def livekit_ports(config):
    # LiveKit's signaling (HTTP and WebSocket), then its WebRTC TCP and UDP ports, in the stack's block.
    return config['port'] + 4, config['port'] + 5, config['port'] + 6


def livekit_yaml(config):
    signaling, tcp, udp = livekit_ports(config)
    key = config['LIVEKIT_API_KEY']
    # Signaling listens on loopback, as the rest of the stack. Media cannot: browsers gather no loopback
    # ICE candidates, so the server offers this machine's own interface addresses (no STUN); a session
    # still needs a token signed with the stack's secret. The webhook drives comms presence in the API.
    return textwrap.dedent(f'''\
        port: {signaling}
        bind_addresses:
          - 127.0.0.1
        rtc:
          tcp_port: {tcp}
          udp_port: {udp}
          use_external_ip: false
        keys:
          {key}: {config['LIVEKIT_API_SECRET']}
        webhook:
          api_key: {key}
          urls:
            - http://127.0.0.1:{config['port']}/comms-gatekeeper/livekit-webhook
        logging:
          level: info
        ''')


def pulse_ports(config):
    # WebTransport (UDP) on the Sites port, the host:port the /play HUD hands the engine as its Pulse
    # server (apps/ui3/src/play/lib/baseDomain.ts); ENet (UDP) and /metrics (TCP) share base+7.
    return config['port'] + 2, config['port'] + 7, config['port'] + 7


def nats_ports(config):
    # The broker's client port, then its HTTP monitor (/healthz is the readiness probe).
    return config['port'] + 8, config['port'] + 9


def nats_url(config):
    return f'nats://127.0.0.1:{nats_ports(config)[0]}'


def nats_argv(config):
    client, monitor = nats_ports(config)
    # Core NATS only: Pulse and archipelago use no JetStream, so the broker keeps nothing on disk.
    return ['nats-server', '--name', 'foundation-dev', '--addr', '127.0.0.1', '--port', str(client),
            '--http_port', str(monitor)]


def openssl(*args, **kwargs):
    return run(['openssl', *args], capture_output=True, **kwargs).stdout


def pulse_spki():
    """The base64 SHA-256 of the stack's Pulse key, the value Chrome's --ignore-certificate-errors-spki-list takes."""
    der = openssl('pkey', '-in', STATE / 'pulse/key.pem', '-pubout', '-outform', 'DER')
    return base64.b64encode(hashlib.sha256(der).digest()).decode()


def pulse_certificate():
    """Pulse's WebTransport certificate: a key kept per stack, so its SPKI hash stays stable, and a fresh
    self-signed certificate on every start, valid for 13 days."""
    directory = STATE / 'pulse'
    directory.mkdir(parents=True, exist_ok=True, mode=0o700)
    key = directory / 'key.pem'
    if not key.exists():
        temporary = directory / 'key.tmp'
        temporary.unlink(missing_ok=True)
        os.close(os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600))
        openssl('genpkey', '-algorithm', 'EC', '-pkeyopt', 'ec_paramgen_curve:P-256', '-out', temporary)
        temporary.replace(key)
    certificate = directory / 'cert.pem'
    openssl('req', '-x509', '-new', '-key', key, '-out', certificate, '-days', '13', '-sha256',
            '-subj', '/CN=localhost', '-addext', 'subjectAltName=DNS:localhost,IP:127.0.0.1')
    return certificate, key


def pulse_about_env(config):
    """The /about comms block that tells the /play HUD Pulse's WebTransport endpoint. The realm's adapter
    stays the archipelago one `runtime_env` sets."""
    url = f'https://127.0.0.1:{pulse_ports(config)[0]}'
    return {'COMMS_V4_PULSE_WEBTRANSPORT_URL': url, 'COMMS_V4_PULSE_AUDIENCE': f'http://127.0.0.1:{config["port"] + 2}'}


def pulse_env(config, certificate, key):
    webtransport, enet, metrics = pulse_ports(config)
    env = {name: os.environ[name] for name in ('PATH', 'HOME', 'TMPDIR') if name in os.environ}
    return {**env, 'PULSE_BIND': f'127.0.0.1:{enet}', 'PULSE_METRICS_BIND': f'127.0.0.1:{metrics}',
            'PULSE_WT_ENABLED': 'true', 'PULSE_WT_BIND': f'127.0.0.1:{webtransport}',
            'PULSE_WT_CERT_PATH': str(certificate), 'PULSE_WT_KEY_PATH': str(key),
            'PULSE_REPLAY_STATE_PATH': str(STATE / 'pulse/replay.tsv'), 'NO_COLOR': '1',
            # The cluster feed to the API's archipelago, and its engine.discovery heartbeat.
            'PULSE_NATS_URL': nats_url(config),
            # Pulse polls the API's moderator ban snapshot with the token the API checks.
            'COMMS_MODERATOR_TOKEN': config['MODERATOR_TOKEN'],
            'PULSE_BANS_URL': f'http://127.0.0.1:{config["port"]}/comms-gatekeeper/bans'}


def runtime_env(config):
    # Do not inherit a shell's production database, upstreams or credentials.
    env = {key: os.environ[key] for key in (
        'PATH', 'HOME', 'USER', 'TMPDIR', 'LANG', 'LC_ALL', 'LOCALE_ARCHIVE',
        'SSL_CERT_FILE', 'NIX_SSL_CERT_FILE'
    ) if key in os.environ}
    base = config['port']
    env.update({
        'DATABASE_URL': database_url(config), 'CATALYST_DATABASE_URL': database_url(config),
        'HTTP_SERVER_HOST': '127.0.0.1',
        'HTTP_SERVER_PORT': str(base), 'PUBLIC_URL': f'http://127.0.0.1:{base + 2}',
        'PG_POOL_SIZE': '12', 'SERVE_UNSYNCED': 'true',
        # The stack's own livekit-server (the `livekit` task) and its generated pair.
        'LIVEKIT_HOST': f'ws://127.0.0.1:{livekit_ports(config)[0]}',
        'LIVEKIT_API_KEY': config['LIVEKIT_API_KEY'], 'LIVEKIT_API_SECRET': config['LIVEKIT_API_SECRET'],
        'MODERATOR_TOKEN': config['MODERATOR_TOKEN'],
        # The stack's own nats-server (the `nats` task). The API is the stack's one primary serve (FLEET_JOBS_ENABLED
        # stays on), so it serves /ws and its island subscriber turns Pulse's cluster feed into island credentials;
        # Pulse's heartbeat makes /about's comms block healthy; the realm advertises the connector, as deployed.
        'NATS_URL': nats_url(config),
        'COMMS_FIXED_ADAPTER': f'archipelago:ws://127.0.0.1:{base}/ws',
        'COMMS_ADAPTER': f'fixed-adapter:archipelago:ws://127.0.0.1:{base}/ws',
        'REALM_NAME': 'Foundation local development',
        'SYNC_ENABLED': 'false', 'SNAPSHOT_GENERATION_INTERVAL_HOURS': '0',
        'CONTENT_HISTORY_PRUNE_ENABLED': 'false', 'WORLDS_CONTENT_MIRROR_ENABLED': 'false',
        'MARKETPLACE_MIRRORS_ENABLED': 'false', 'MARKETPLACE_NFT_INDEX_ENABLED': 'false',
        'MARKETPLACE_TRADE_EVENTS_INDEX_ENABLED': 'false', 'MARKETPLACE_MANA_TRANSFERS_INDEX_ENABLED': 'false',
        'LAND_AUTHZ_INDEX_ENABLED': 'false', 'CREDITS_USAGE_INDEX_ENABLED': 'false',
        'PRICE_POLL_ENABLED': 'false',
        'COMMUNITIES_MIRROR_UPSTREAM': 'false', 'SCENE_HOST_ENABLED': 'false',
        'PRESENCE_COLLECTOR_ENABLED': 'false', 'JOBS_FIRST_PASS_NOW': 'true',
        'CONTENT_FORWARD_UPSTREAM_URL': '', 'WORLDS_FORWARD_UPSTREAM_URL': '',
        'STORAGE_ROOT_FOLDER': str(STATE / 'content'),
        'AB_CDN_CACHE_DIR': str(STATE / 'ab-cdn'), 'AB_CDN_CACHE_MAX_BYTES': '536870912',
        'PROFILE_IMAGES_CACHE_DIR': str(STATE / 'profile-images'),
        'PROFILE_IMAGES_CACHE_MAX_BYTES': '134217728',
        'BVIMPOSTERS_STORE_ROOT': str(STATE / 'imposters'),
        'BVIMPOSTERS_STORE_MAX_BYTES': '134217728',
        'SESSION_SECRET': config['SESSION_SECRET'], 'ENCRYPTION_KEY': config['ENCRYPTION_KEY'],
        'HOST': '127.0.0.1', 'PORT': str(base + 2),
        'CATALYST_URL': f'http://127.0.0.1:{base}',
        'DCL_CATALYST_PROXY_TARGET': f'http://127.0.0.1:{base}',
    })
    for domain in ('CONTENT', 'CREATE', 'DATA', 'SOCIAL', 'WORLDS', 'OPS'):
        env[f'{domain}_WRITES_ENABLED'] = 'true'
    # Explicit development settings, without a production posture/restore gate.
    # Authentication and mirrored-row ownership checks still apply.
    if config['profile'] == 'offline':
        env['NODE_POSTURE'] = 'journey'
    else:
        env.update({
            'READ_THROUGH_ENABLED': 'true',
            'SYNC_SOURCE': 'https://peer.decentraland.org/content,https://peer-eu1.decentraland.org/content',
            # This unsynced copy of the peers still takes the user's own signed profile deploys
            # (/play saves the avatar); every other type answers the bootstrapping 503.
            'SERVE_UNSYNCED_DEPLOY_TYPES': 'profile',
            'PLACES_MIRROR_UPSTREAM': 'true', 'PLACES_UPSTREAM_URL': 'https://places.decentraland.org',
            'WORLDS_MIRROR_UPSTREAM': 'true', 'WORLDS_UPSTREAM_URL': 'https://places.decentraland.org',
            # No worlds content pass (WORLDS_CONTENT_MIRROR_ENABLED=false): a world is read
            # through from this source, verified, the first time it is asked for.
            'WORLDS_CONTENT_MIRROR_SOURCE': 'https://worlds-content-server.decentraland.org',
            # A scene signed by a deployer the owner granted upstream: on the upstream's word.
            'WORLDS_READ_THROUGH_TRUST_UPSTREAM_GRANTS': 'true',
            'EVENTS_MIRROR_UPSTREAM': 'true', 'EVENTS_UPSTREAM_URL': 'https://events.decentraland.org',
            'EVENTS_POSTER_UPSTREAM_URL': 'https://events-assets-099ac00.decentraland.org',
            'EXPLORER_MIRRORS_ENABLED': 'true',
            'EXPLORER_LISTS_SOURCE_URL': 'https://dcl-lists.decentraland.org',
            'EXPLORER_DENYLIST_SOURCE_URL': 'https://config.decentraland.org/denylist.json',
            'EXPLORER_FEATURE_FLAGS_SOURCE_URL': 'https://feature-flags.decentraland.org/explorer.json',
            'EXPLORER_REGISTRY_SOURCE_URL': 'https://asset-bundle-registry.decentraland.org',
            'ABGEN_UPSTREAM_AB_CDN': 'https://ab-cdn.decentraland.org',
            # Mirrored place and event images come from the upstream peers' content URLs.
            'MEDIA_CONVERT_REMOTE_SOURCES': 'true',
            # Avatar faces of profiles this node holds no snapshot files of: no renderer
            # runs here, so they come from the upstream service into the blob store.
            'PROFILE_IMAGES_ORIGIN_URL': 'https://profile-images.decentraland.org',
            # The Shop's catalogue: the marketplace mirrors and the NFT and LAND
            # indexes `catalyst posture show mirror` names, run by the index process.
            'MARKETPLACE_MIRRORS_ENABLED': 'true',
            'TRADES_SYNC_UPSTREAM_URL': 'https://marketplace-api.decentraland.org/v1/trades',
            # The Shop's feeds wait for one complete trades pass; take it in one go rather than
            # 5,000 trades per 15-minute tick (about an hour instead of four on a fresh stack).
            'TRADES_SYNC_PAGES_PER_RUN': '0',
            # Development only: the Shop reads the order book while the first pass is still running.
            'TRADES_SERVE_PARTIAL': 'true',
            'ETH_COLLECTIONS_SUBGRAPH_URL': 'https://subgraph.decentraland.org/collections-ethereum-mainnet',
            'MATIC_COLLECTIONS_SUBGRAPH_URL': 'https://subgraph.decentraland.org/collections-matic-mainnet',
            'MARKETPLACE_SUBGRAPH_URL': 'https://subgraph.decentraland.org/marketplace',
            'MARKETPLACE_NFT_INDEX_ENABLED': 'true', 'LAND_AUTHZ_INDEX_ENABLED': 'true',
            'ETH_RPC_URL': 'https://rpc.decentraland.org/mainnet',
            'BUILDER_CONTENT_BUCKET_URL': 'https://builder-items.decentraland.org',
            # Scene-template models (Qm... CIDs) live in the asset-packs bucket, which this
            # route redirects to; the builder pulls from it when the item bucket misses.
            'BUILDER_ASSET_PACKS_BUCKET_URL': 'https://builder-api.decentraland.org/v1/storage',
            # Credit prices need a MANA spot; the map (Places, the HUD's map, estate images) its grid.
            'PRICE_POLL_ENABLED': 'true', 'COINGECKO_URL': 'https://api.coingecko.com/api/v3',
            'PRICE_FALLBACK_SOURCES': 'coinbase=https://api.coinbase.com,binance=https://api.binance.com,'
                                      'kraken=https://api.kraken.com',
            'EXPLORER_MAP_SOURCE_URL': 'https://api.decentraland.org',
            'RPC_ENDPOINT_POLYGON': 'https://rpc.decentraland.org/polygon',
        })
    return env


def socket_path():
    current = STATE / 'pc.sock'
    # Allow an already running supervisor to be stopped after updating tooling.
    digest = hashlib.sha256(str(ROOT).encode()).hexdigest()[:16]
    legacy = Path('/tmp') / f'foundation-dev-{os.getuid()}-{digest}' / 'pc.sock'
    if not current.exists() and legacy.exists():
        if legacy.parent.is_symlink() or legacy.parent.stat().st_uid != os.getuid():
            raise RuntimeError('Development socket directory is not owned by this user')
        return legacy
    return current


def pc_argv(*args):
    path = STATE / 'pc.sock' if args and args[0] == 'up' else socket_path()
    # A relative Unix socket address avoids Darwin's path length limit while
    # keeping the socket in persistent checkout state, outside temporary dirs.
    address = path.name if path.parent == STATE else str(path)
    logfile = 'supervisor.log' if args and args[0] == 'up' else 'supervisor-client.log'
    return ['process-compose', '--use-uds', '--unix-socket', address, '--log-file', str(STATE / logfile), *args]


def pc(*args, **kwargs):
    STATE.mkdir(parents=True, exist_ok=True, mode=0o700)
    return subprocess.run(pc_argv(*args), cwd=STATE, **kwargs)


def supervisor_running():
    return pc('process', 'list', stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0


def project(config, postgres_only=False):
    def command(task):
        return 'exec ' + shlex.join([sys.executable, str(ROOT / 'tooling/dev.py'), '_task', task])

    def process(task, dependencies=None):
        result = {'command': command(task), 'working_dir': str(ROOT),
                  'log_location': str(STATE / f'{task}.log'),
                  'log_configuration': {'disable_json': True, 'no_color': True,
                                        'no_metadata': True, 'fields_order': ['message'],
                                        'flush_each_line': True,
                                        'rotation': {'max_size_mb': 10, 'max_backups': 3}},
                  'shutdown': {'timeout_seconds': 30}}
        if dependencies:
            result['depends_on'] = {name: {'condition': condition} for name, condition in dependencies.items()}
        return result

    postgres = process('postgres')
    postgres['shutdown']['signal'] = 2  # PostgreSQL fast shutdown; no orphaned sessions.
    # The probe starts Python and psql; on a loaded machine that takes longer than the 1 s default, and
    # 60 timed-out probes in a row make process-compose stop a healthy PostgreSQL.
    postgres['readiness_probe'] = {'exec': {'command': command('database-ready')},
                                   'period_seconds': 1, 'timeout_seconds': 15, 'failure_threshold': 60}
    if postgres_only:
        return {'version': '0.5', 'processes': {'postgres': postgres}}
    api = process('api', {'postgres': 'process_healthy', 'api-build': 'process_completed_successfully',
                          'web-install': 'process_completed_successfully', 'nats': 'process_healthy'})
    api['readiness_probe'] = {'http_get': {'host': '127.0.0.1', 'port': config['port'], 'path': '/about'},
                             'period_seconds': 2, 'failure_threshold': 1800}
    pulse = process('pulse', {'api-build': 'process_completed_successfully', 'nats': 'process_healthy'})
    pulse['readiness_probe'] = {'http_get': {'host': '127.0.0.1', 'port': pulse_ports(config)[2], 'path': '/metrics'},
                                'period_seconds': 1, 'failure_threshold': 60}
    livekit = process('livekit')
    livekit['readiness_probe'] = {'http_get': {'host': '127.0.0.1', 'port': livekit_ports(config)[0], 'path': '/'},
                                  'period_seconds': 1, 'failure_threshold': 60}
    nats = process('nats')
    nats['readiness_probe'] = {'http_get': {'host': '127.0.0.1', 'port': nats_ports(config)[1], 'path': '/healthz'},
                               'period_seconds': 1, 'failure_threshold': 60}
    sites = process('sites', {'web-install': 'process_completed_successfully'})
    sites['readiness_probe'] = {'http_get': {'host': '127.0.0.1', 'port': config['port'] + 2, 'path': '/'},
                               'period_seconds': 2, 'failure_threshold': 1800}
    processes = {
        'postgres': postgres,
        'api-build': process('api-build', {'postgres': 'process_healthy'}), 'api': api,
        'web-install': process('web-install'), 'sites': sites, 'livekit': livekit, 'pulse': pulse,
        'nats': nats,
    }
    if config['profile'] == 'mirror':
        # The marketplace mirrors and chain indexes run in `catalyst index`, beside `serve`.
        processes['index'] = process('index', {'postgres': 'process_healthy',
                                               'api-build': 'process_completed_successfully'})
    return {'version': '0.5', 'processes': processes}


def sql(config, query, database='catalyst'):
    env = runtime_env(config)
    env.update(PGHOST='127.0.0.1', PGPORT=str(config['port'] + 3), PGUSER='foundation',
               PGPASSWORD=config['password'], PGDATABASE=database)
    return run(['psql', '-X', '-v', 'ON_ERROR_STOP=1', '-Atq'], input=query, text=True,
               capture_output=True, env=env).stdout.strip()


def postgres(config):
    env = runtime_env(config)
    data = STATE / 'postgres'
    if not (data / 'PG_VERSION').exists():
        if data.exists() and any(data.iterdir()):
            raise RuntimeError('Incomplete PostgreSQL initialization; data preserved. Inspect .cache/dev/postgres.')
        password_file = STATE / 'pg-password'
        password_file.write_text(config['password'])
        try:
            run(['initdb', '-D', data, '-U', 'foundation', '--auth-local=trust',
                 '--auth-host=scram-sha-256', '--pwfile', password_file, '--encoding=UTF8', '--locale=en_US.UTF-8'], env=env)
        finally:
            password_file.unlink(missing_ok=True)
    version = run(['postgres', '--version'], capture_output=True, text=True, env=env).stdout.split()[2].split('.')[0]
    if (data / 'PG_VERSION').read_text().strip() != version:
        raise RuntimeError('PostgreSQL major version changed. Data preserved; migrate it before restarting.')
    os.execvpe('postgres', ['postgres', '-D', str(data), '-h', '127.0.0.1',
                           '-p', str(config['port'] + 3), '-k', '',
                           '-c', 'max_connections=40', '-c', 'shared_buffers=64MB'], env)


def ensure_collation(config, database='catalyst'):
    # Linux imports this name from glibc at initdb. Darwin calls its native
    # UTF-8 locale en_US.UTF-8; expose the name used by the existing queries in
    # pg_catalog, the one schema every search_path resolves (the API pins
    # search_path=content,meta, so a public collation is invisible to it).
    # Never replace a collation in an existing database.
    exists = sql(config, "SELECT EXISTS (SELECT 1 FROM pg_collation WHERE collname='en_US.utf8' "
                         "AND collnamespace='pg_catalog'::regnamespace)", database)
    if exists != 't':
        sql(config, 'CREATE COLLATION pg_catalog."en_US.utf8" (provider=libc, locale=\'en_US.UTF-8\')', database)


def apply_schema(config, binary):
    # Source saves can change schema, so every API start checks it. Serialize
    # the entire bootstrap/migrate sequence against an explicit api-build retry.
    with (STATE / 'schema.lock').open('w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if sql(config, "SELECT 1 FROM pg_database WHERE datname='catalyst'", 'postgres') != '1':
            sql(config, 'CREATE DATABASE catalyst', 'postgres')
        ensure_collation(config)
        # Test scratch databases are created from template1 and need it too.
        ensure_collation(config, 'template1')
        stamped = sql(config, "SELECT to_regclass('meta.schema') IS NOT NULL") == 't'
        run([binary, 'schema', 'migrate' if stamped else 'apply'], env=runtime_env(config))


# The trades mirror's tables, in foreign-key order. A snapshot of them lets a fresh stack skip most
# of the first trades pass (77,580 detail fetches): the mirror then only fetches what is newer.
TRADES_TABLES = ('shop.trades', 'shop.trade_assets', 'shop.trade_assets_erc20',
                 'shop.trade_assets_erc721', 'shop.trade_assets_item')


def psql_env(config):
    env = runtime_env(config)
    env.update(PGHOST='127.0.0.1', PGPORT=str(config['port'] + 3), PGUSER='foundation',
               PGPASSWORD=config['password'], PGDATABASE='catalyst')
    return env


def export_trades_snapshot(config, out):
    # Mirrored rows only; a trade signed into this stack stays local.
    tables = [arg for table in TRADES_TABLES for arg in ('--table', table)]
    dump = subprocess.run(['pg_dump', '--data-only', '--no-owner', '--no-privileges', *tables],
                          env=psql_env(config), check=True, capture_output=True).stdout
    with gzip.open(out, 'wb') as file:
        file.write(dump)
    print(f'Wrote {out} ({Path(out).stat().st_size} bytes); its sha256 is {hashlib.sha256(Path(out).read_bytes()).hexdigest()}')


def trades_snapshot_file():
    # FOUNDATION_DEV_TRADES_SNAPSHOT names a local file (`foundation-dev trades-snapshot` writes one).
    local = os.environ.get('FOUNDATION_DEV_TRADES_SNAPSHOT')
    return Path(local) if local else None


def seed_trades(config):
    if config['profile'] != 'mirror' or sql(config, 'SELECT count(*) FROM shop.trades') != '0':
        return
    snapshot = trades_snapshot_file()
    if snapshot is None:
        return
    with gzip.open(snapshot, 'rb') as file:
        subprocess.run(['psql', '-X', '-v', 'ON_ERROR_STOP=1', '-q', '--single-transaction'],
                       input=file.read(), env=psql_env(config), check=True, stdout=subprocess.DEVNULL)
    print(f"Seeded {sql(config, 'SELECT count(*) FROM shop.trades')} trades from {snapshot.name}.")


# Creator Hub's asset catalogue, from the @dcl/asset-packs package Sites installs; the files it
# names are pulled from BUILDER_CONTENT_BUCKET_URL (then BUILDER_ASSET_PACKS_BUCKET_URL) on first read.
ASSET_PACKS_CATALOG = ROOT / 'apps/sites/node_modules/@dcl/asset-packs/catalog.json'


def import_asset_catalog(config, binary):
    if not ASSET_PACKS_CATALOG.exists():
        return
    if sql(config, 'SELECT count(*) FROM builder.asset_catalog') != '0':
        return
    run([binary, 'builder', 'catalog', ASSET_PACKS_CATALOG], env=runtime_env(config))


# Long-running tasks and the signal that stops each one cleanly.
SERVICES = {'postgres': signal.SIGINT, 'api': signal.SIGTERM, 'sites': signal.SIGTERM, 'index': signal.SIGTERM,
            'livekit': signal.SIGTERM, 'pulse': signal.SIGTERM, 'nats': signal.SIGTERM}


def started(pid):
    # A process's start time, or '' once it has exited (zombies included).
    state, _, start = subprocess.run(['ps', '-o', 'stat=,lstart=', '-p', str(pid)],
                                     capture_output=True, text=True).stdout.strip().partition(' ')
    return '' if state.startswith('Z') else start.strip()


def reap_orphans():
    # process-compose starts tasks in their own process groups, so a supervisor
    # killed outright leaves them running; launchd has no cgroup to clean up.
    # Only a process with the recorded PID and start time is signalled.
    for name, stop in SERVICES.items():
        path = STATE / 'tasks' / f'{name}.json'
        if not path.exists():
            continue
        record = json.loads(path.read_text())
        pid = record['pid']
        if record['started'] and started(pid) == record['started']:
            print(f'Stopping {name} (PID {pid}) left by a supervisor that exited abnormally.', file=sys.stderr)
            group = name != 'postgres' and os.getpgid(pid) == pid
            for signum in (stop, signal.SIGKILL):
                (os.killpg if group else os.kill)(pid, signum)
                deadline = time.monotonic() + 30
                while (alive := started(pid) == record['started']) and time.monotonic() < deadline:
                    time.sleep(.2)
                if not alive:
                    break
        path.unlink()


def watch(*paths):
    return [arg for path in paths for arg in ('--watch', path)]


def task(name):
    config = settings()
    if name in SERVICES:
        # The PID survives the exec into the service itself.
        (STATE / 'tasks').mkdir(exist_ok=True, mode=0o700)
        save_json(STATE / 'tasks' / f'{name}.json', {'pid': os.getpid(), 'started': started(os.getpid())})
    if name == 'postgres':
        postgres(config)
    elif name == 'database-ready':
        sql(config, 'SELECT 1', 'postgres')
    elif name == 'web-install':
        for app in ('ui3', 'sites'):
            run(['pnpm', '--dir', ROOT / 'apps' / app, 'install', '--frozen-lockfile'])
        # Our built Explorer, verified against the source/toolchain/packaging inputs.
        run([sys.executable, ROOT / 'tooling/fetch-play.py'])
    elif name == 'sites':
        os.chdir(ROOT / 'apps/sites')
        os.execvpe('pnpm', ['pnpm', 'dev'], runtime_env(config))
    elif name == 'api':
        os.chdir(BACKEND)
        # Watch source inputs only. Failed edits remain visible in api.log and
        # the next save retries; dependencies and Cargo artifacts stay cached.
        os.execvp('watchexec', ['watchexec', '--restart', '--debounce', '500ms',
                  *watch('src', *sorted(p.parent.name for p in BACKEND.glob('*/Cargo.toml')),
                         '../../schemas', '../../library', 'Cargo.toml', 'Cargo.lock'),
                  '--exts', 'rs,toml,sql,lock', '--', sys.executable, str(ROOT / 'tooling/dev.py'), '_task', 'api-run'])
    elif name == 'livekit':
        # The config travels in the environment, so the secret never shows in ps.
        env = {key: os.environ[key] for key in ('PATH', 'HOME', 'TMPDIR') if key in os.environ}
        os.execvpe('livekit-server', ['livekit-server'], {**env, 'LIVEKIT_CONFIG': livekit_yaml(config)})
    elif name == 'nats':
        env = {key: os.environ[key] for key in ('PATH', 'HOME', 'TMPDIR') if key in os.environ}
        os.execvpe('nats-server', nats_argv(config), env)
    elif name == 'pulse':
        certificate, key = pulse_certificate()
        binary = BACKEND / 'target/debug/catalyst'
        os.execve(binary, [str(binary), 'pulse'], pulse_env(config, certificate, key))
    elif name == 'index':
        binary = BACKEND / 'target/debug/catalyst'
        os.execve(binary, [str(binary), 'index'], runtime_env(config))
    elif name in ('api-build', 'api-run'):
        target = BACKEND / 'target'
        run(['cargo', 'build', '--locked', '--bin', 'catalyst', '--target-dir', target], cwd=BACKEND)
        binary = target / 'debug/catalyst'
        apply_schema(config, binary)
        if name == 'api-build':
            # Before the index process starts mirroring, so the snapshot's rows are not fetched again.
            seed_trades(config)
            return
        import_asset_catalog(config, binary)
        os.execve(binary, [str(binary), 'serve'], {**runtime_env(config), **pulse_about_env(config)})


def status():
    config = settings()
    print(f"Sites: http://127.0.0.1:{config['port'] + 2}   API: http://127.0.0.1:{config['port']}   "
          f"LiveKit: ws://127.0.0.1:{livekit_ports(config)[0]}   Pulse: https://127.0.0.1:{pulse_ports(config)[0]} (UDP)   "
          f"NATS: {nats_url(config)}")
    if (STATE / 'pulse/key.pem').exists():
        # The pinned web Explorer passes no serverCertificateHashes; Chrome honours an SPKI exception for WebTransport
        # only with --webtransport-developer-mode, and the exception list only beside --user-data-dir.
        print(f'Pulse (avatars) needs Chrome run with --user-data-dir=<dir> --webtransport-developer-mode --ignore-certificate-errors-spki-list={pulse_spki()}')
    print(f"Data: {STATE}   Profile: {config['profile']}", flush=True)
    return pc('process', 'list', '--output', 'wide').returncode


def prepare_supervisor(postgres_only=False):
    """Write the process graph; returns the supervisor's `up` arguments and environment, or None if it runs."""
    config = settings(create=True)
    if supervisor_running():
        running = json.loads((STATE / 'process-compose.json').read_text())['processes']
        if not postgres_only and set(running) == {'postgres'}:
            raise RuntimeError('PostgreSQL-only stack is running. Run foundation-dev down before starting the full stack.')
        return None
    if not ports_free(config['port']):
        raise RuntimeError('Configured ports are occupied. Stop the conflicting service or edit .cache/dev/settings.json.')
    temporary = STATE / 'tmp'
    temporary.mkdir(exist_ok=True, mode=0o700)
    save_json(STATE / 'process-compose.json', project(config, postgres_only))
    return (['up', '--config', str(STATE / 'process-compose.json'), '--disable-dotenv', '--ordered-shutdown', '--tui=false'],
            {**os.environ, 'TMPDIR': str(temporary), 'TMP': str(temporary), 'TEMP': str(temporary)})


def supervise():
    # The service manager's main process: the supervisor in the foreground, so
    # SIGTERM from systemd or launchd stops the stack in dependency order.
    STATE.mkdir(parents=True, exist_ok=True, mode=0o700)
    with (STATE / 'manager.lock').open('w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if supervisor_running():
            raise RuntimeError('A detached development stack is running; stop it with foundation-dev down.')
        reap_orphans()
        args, env = prepare_supervisor()
    os.chdir(STATE)
    # Task output is already in each task's log; keep it out of service.log and the journal.
    os.dup2(os.open(os.devnull, os.O_WRONLY), 1)
    os.execvpe('process-compose', pc_argv(*args), env)


def service_name():
    return 'one.dcl.foundation-dev.' + hashlib.sha256(str(ROOT).encode()).hexdigest()[:8]


def service_file():
    if DARWIN:
        return Path.home() / 'Library/LaunchAgents' / f'{service_name()}.plist'
    return Path.home() / '.config/systemd/user' / f'{service_name()}.service'


def systemctl(*args, **kwargs):
    if not Path('/run/systemd/system').is_dir():
        raise RuntimeError('systemd is not running. On WSL add "[boot]" and "systemd=true" to /etc/wsl.conf, '
                           'then run wsl.exe --shutdown from Windows.')
    return run(['systemctl', '--user', *args], **kwargs)


def launchctl(*args, **kwargs):
    return subprocess.run(['launchctl', *args], **kwargs)


def service_start():
    if not DARWIN:
        systemctl('start', service_name())
    elif launchctl('print', f'gui/{os.getuid()}/{service_name()}', capture_output=True).returncode:
        launchctl('bootstrap', f'gui/{os.getuid()}', str(service_file()), check=True)
    else:
        launchctl('kickstart', f'gui/{os.getuid()}/{service_name()}', check=True)


def service_stop():
    if DARWIN:
        # bootout waits for ExitTimeOut; the next login loads the agent again.
        launchctl('bootout', f'gui/{os.getuid()}/{service_name()}', capture_output=True)
    else:
        systemctl('stop', service_name())


def service_install():
    # The service environment has no FOUNDATION_DEV_PORT or _PROFILE; apply them now.
    STATE.mkdir(parents=True, exist_ok=True, mode=0o700)
    settings(create=True)
    path = service_file()
    path.parent.mkdir(parents=True, exist_ok=True)
    nix = shutil.which('nix')
    if nix is None:
        raise RuntimeError('nix is not on PATH.')
    # Re-entering the flake on every start picks up toolchain changes.
    command = [nix, 'develop', str(ROOT), '--command', 'foundation-dev', '_supervise']
    if DARWIN:
        path.write_bytes(plistlib.dumps({
            'Label': service_name(), 'ProgramArguments': command, 'WorkingDirectory': str(ROOT),
            'EnvironmentVariables': {'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'FOUNDATION_DEV_AUTOSTART': '0'},
            # Restart after a crash, not after foundation-dev down.
            'RunAtLoad': True, 'KeepAlive': {'SuccessfulExit': False}, 'ThrottleInterval': 10,
            'ExitTimeOut': 150, 'ProcessType': 'Interactive',
            'StandardOutPath': str(STATE / 'service.log'), 'StandardErrorPath': str(STATE / 'service.log'),
        }))
    else:
        path.write_text(textwrap.dedent(f'''\
            [Unit]
            Description=Foundation development stack ({ROOT})

            [Service]
            WorkingDirectory={ROOT}
            Environment=FOUNDATION_DEV_AUTOSTART=0
            ExecStart={shlex.join(command)}
            Restart=on-failure
            RestartSec=10
            TimeoutStopSec=150

            [Install]
            WantedBy=default.target
            '''))
        systemctl('daemon-reload')
        systemctl('enable', service_name())
    down()
    service_start()
    print(f'Installed {path}; the stack now starts at login and restarts after a crash.')
    if not DARWIN:
        print(f'To start it at boot without a login session: loginctl enable-linger {os.environ.get("USER", "")}'.rstrip())
    print('Startup output: foundation-dev logs service. Remove with foundation-dev service uninstall.')


def service_uninstall():
    path = service_file()
    if not path.exists():
        print('No service installed for this checkout.')
        return
    if DARWIN:
        service_stop()
    else:
        systemctl('disable', '--now', service_name())
    path.unlink()
    if not DARWIN:
        systemctl('daemon-reload')
    print(f'Removed {path}. PostgreSQL data and settings are preserved; foundation-dev up starts a detached stack.')


def desktop():
    # Rust edits rebuild and relaunch the shell; the /play page it loads is the
    # Sites dev server, so HUD edits arrive through Vite HMR without a relaunch.
    if not os.environ.get('FOUNDATION_DEV_DESKTOP_SHELL'):
        os.execvp('nix', ['nix', 'develop', f'{ROOT}#desktop', '--command', 'foundation-dev', 'desktop'])
    config = settings()
    os.chdir(ROOT / 'apps/desktop')
    env = {**os.environ, 'DCL_PLAY_URL': f"http://127.0.0.1:{config['port'] + 2}/play"}
    os.execvpe('watchexec', ['watchexec', '--restart', '--debounce', '500ms',
                             *watch('src', 'capabilities', 'build.rs', 'Cargo.toml', 'tauri.conf.json'),
                             '--exts', 'rs,toml,json', '--', 'cargo', 'run'], env)


def up(postgres_only=False):
    STATE.mkdir(parents=True, exist_ok=True, mode=0o700)
    if service_file().exists():
        if postgres_only:
            raise RuntimeError('PostgreSQL-only runs require a checkout without an installed development service.')
        service_start()
        print('The installed service runs the stack; use foundation-dev wait or foundation-dev status.')
        return 0
    with (STATE / 'manager.lock').open('w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        prepared = prepare_supervisor(postgres_only)
        if prepared is None:
            print('The development stack is already running.')
            return status()
        args, env = prepared
        pc(*args, '--detached', check=True, env=env)
        deadline = time.monotonic() + 10
        while not supervisor_running():
            if time.monotonic() >= deadline:
                raise RuntimeError('Supervisor startup timed out. Inspect .cache/dev/supervisor.log.')
            time.sleep(.1)
    print('Starting PostgreSQL only.' if postgres_only else
          'Starting PostgreSQL, the API and Sites. The first run downloads dependencies and compiles Rust.')
    print('Use foundation-dev logs api-build, foundation-dev status or foundation-dev wait.')
    print('Services remain running after this shell exits. Stop them with foundation-dev down.')
    return status()


def wait_ready(timeout, postgres_only=False):
    config = settings()
    urls = [f"http://127.0.0.1:{config['port']}/about", f"http://127.0.0.1:{config['port'] + 2}/"]
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        processes = pc('process', 'list', '--output', 'json', capture_output=True, text=True)
        if processes.returncode:
            raise RuntimeError('The development supervisor is not running. Run foundation-dev up.')
        failed = {p['name'] for p in json.loads(processes.stdout)
                  if p.get('exit_code', 0) != 0 and not p.get('is_running')}
        if failed:
            graph = project(config, postgres_only)['processes']
            roots = sorted(name for name in failed
                           if not failed.intersection(graph.get(name, {}).get('depends_on', {})))
            name = (roots or sorted(failed))[0]
            raise RuntimeError(f'{name} failed. Inspect foundation-dev logs {name}.')
        try:
            if postgres_only:
                sql(config, 'SELECT 1', 'postgres')
                # Scratch databases clone template1. Match the collation that
                # full-stack startup installs, without compiling the API.
                with (STATE / 'schema.lock').open('w') as lock:
                    fcntl.flock(lock, fcntl.LOCK_EX)
                    ensure_collation(config, 'template1')
                print('PostgreSQL is ready for scratch test databases.')
                return 0
            for url in urls:
                with urllib.request.urlopen(url, timeout=3) as response:
                    if response.status != 200:
                        raise RuntimeError('Service not ready')
            print('API and Sites are ready.')
            return wait_first_mirror_pass(config, deadline)
        except (urllib.error.URLError, TimeoutError, OSError, RuntimeError, subprocess.CalledProcessError):
            time.sleep(1)
    raise RuntimeError('Startup timed out. Check foundation-dev status and foundation-dev logs api/sites.')


def first_pass_pending(status):
    """The configured mirrors with no complete pass yet, and the errors of those that failed."""
    data = status['data']
    done = {m['mirror'] for m in data['mirrors'] if m.get('last_complete_pass_at')}
    pending = [name for name, source in data['configured_mirrors'].items() if source and name not in done]
    failed = {m['mirror']: m['last_error'] for m in data['mirrors'] if m['mirror'] in pending and m.get('last_error')}
    return pending, failed


def wait_first_mirror_pass(config, deadline):
    # Catalogue routes answer 503 until their mirror's first pass. An upstream is never a
    # prerequisite for readiness: an error, an unreadable status or the deadline only warns.
    url = f"http://127.0.0.1:{config['port']}/api/status"
    announced = None
    while True:
        try:
            with urllib.request.urlopen(url, timeout=3) as response:
                pending, failed = first_pass_pending(json.load(response))
        except (urllib.error.URLError, TimeoutError, OSError, ValueError, KeyError):
            print(f'Could not read {url}; catalogue routes answer 503 until their first mirror pass.')
            return 0
        if not pending:
            if announced:
                print('Mirrors completed their first pass.')
            return 0
        if failed:
            for name, error in failed.items():
                print(f'The {name} mirror failed before its first pass ({error}); its routes answer 503. See {url}.')
            return 0
        if time.monotonic() >= deadline:
            print(f"The first {', '.join(pending)} mirror pass is still running; see {url}.")
            return 0
        if pending != announced:
            print(f"Waiting for the first {', '.join(pending)} mirror pass; its routes answer 503 until then.")
            announced = pending
        time.sleep(2)


def live_postgres_pid():
    try:
        with (STATE / 'postgres/postmaster.pid').open() as pidfile:
            pid = int(pidfile.readline())
        if pid <= 0:
            raise ValueError('Expected a positive PID')
    except FileNotFoundError:
        return None
    except ValueError as error:
        raise RuntimeError('Invalid PostgreSQL PID file. Inspect .cache/dev/postgres/postmaster.pid; '
                           'no processes were stopped.') from error
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        # PostgreSQL checks stale PID files itself on the next startup.
        return None
    except PermissionError:
        pass  # The process exists even though this user cannot signal it.
    return pid


def down():
    if service_file().exists():
        service_stop()
    if not STATE.exists():
        return 0
    with (STATE / 'manager.lock').open('w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        config = settings() if SETTINGS.exists() else None
        control_socket = socket_path()
        if not supervisor_running():
            pid = live_postgres_pid()
            if pid is not None:
                raise RuntimeError(f'Supervisor is unavailable while PID {pid} recorded by PostgreSQL is alive. '
                                   'Inspect .cache/dev/supervisor.log and verify the process identity; '
                                   'no processes were stopped.')
            if config and not ports_free(config['port']):
                raise RuntimeError('Supervisor is unavailable while development ports are occupied. '
                                   'Inspect .cache/dev/supervisor.log; no processes were stopped.')
            return 0
        pc('down', check=True)
        deadline = time.monotonic() + 120
        while time.monotonic() < deadline:
            if not control_socket.exists() and (config is None or ports_free(config['port'])):
                return 0
            time.sleep(.2)
    raise RuntimeError('Shutdown timed out. Inspect .cache/dev/supervisor.log before restarting.')


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    for command in ('down', 'status', 'ui', 'psql', 'env', 'desktop', '_supervise'):
        commands.add_parser(command)
    commands.add_parser('up').add_argument('--postgres-only', action='store_true',
                                          help='Start only PostgreSQL for backend tests')
    commands.add_parser('logs').add_argument('service', choices=('api', 'api-build', 'sites', 'postgres', 'web-install', 'index', 'livekit', 'pulse', 'nats', 'service'))
    commands.add_parser('restart').add_argument('service', choices=('api', 'api-build', 'sites', 'postgres', 'web-install', 'index', 'livekit', 'pulse', 'nats'))
    commands.add_parser('service', help='Run the stack as a per-user systemd unit or LaunchAgent').add_argument(
        'action', choices=('install', 'uninstall'))
    wait_parser = commands.add_parser('wait')
    wait_parser.add_argument('--timeout', type=int, default=1800)
    wait_parser.add_argument('--postgres-only', action='store_true')
    commands.add_parser('profile').add_argument('profile', choices=('mirror', 'offline'))
    commands.add_parser('trades-snapshot', help="Write the mirrored trades to a gzipped SQL file").add_argument('out')
    commands.add_parser('_task', help='Internal supervised process').add_argument('task', choices=(
        'postgres', 'database-ready', 'api-build', 'api', 'api-run', 'web-install', 'sites', 'index', 'livekit', 'pulse', 'nats'))
    args = parser.parse_args()
    if args.command == '_task':
        task(args.task)
    elif args.command == 'up':
        return up(args.postgres_only)
    elif args.command == 'status':
        return status()
    elif args.command == 'wait':
        return wait_ready(args.timeout, args.postgres_only)
    elif args.command == 'down':
        return down()
    elif args.command == '_supervise':
        supervise()
    elif args.command == 'service':
        service_install() if args.action == 'install' else service_uninstall()
    elif args.command == 'desktop':
        desktop()
    elif args.command == 'ui':
        return pc('attach').returncode
    elif args.command == 'logs':
        if args.service == 'service' and not DARWIN:
            os.execvp('journalctl', ['journalctl', '--user', '-f', '-n', '80', '-u', service_name()])
        os.execvp('tail', ['tail', '-n', '80', '-F', str(STATE / f'{args.service}.log')])
    elif args.command == 'restart':
        return pc('process', 'restart', args.service).returncode
    elif args.command == 'profile':
        if supervisor_running():
            raise RuntimeError('Run foundation-dev down before changing profile.')
        config = settings()
        config['profile'] = args.profile
        save_json(SETTINGS, config)
        print(f'Profile set to {args.profile}; run foundation-dev up. Existing data is preserved.')
    elif args.command == 'env':
        # eval "$(foundation-dev env)" names the running stack for scripts and test suites.
        config = settings()
        for key, value in (('FOUNDATION_DEV_API_URL', f"http://127.0.0.1:{config['port']}"),
                           ('FOUNDATION_DEV_SITES_URL', f"http://127.0.0.1:{config['port'] + 2}"),
                           ('FOUNDATION_DEV_DATABASE_URL', database_url(config)),
                           ('FOUNDATION_DEV_TEST_DATABASE_URL', database_url(config, 'postgres'))):
            print(f'{key}={shlex.quote(value)}')
    elif args.command == 'psql':
        os.execvpe('psql', ['psql', '-X'], psql_env(settings()))
    elif args.command == 'trades-snapshot':
        export_trades_snapshot(settings(), args.out)
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (RuntimeError, ValueError, subprocess.CalledProcessError) as error:
        print(f'foundation-dev: {error}', file=sys.stderr)
        sys.exit(1)
