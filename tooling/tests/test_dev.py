"""Development configuration, schema selection and PostgreSQL lifecycle checks."""
import importlib.util
import base64
import contextlib
import hashlib
import gzip
import io
import shlex
import json
import os
from pathlib import Path
import plistlib
import shutil
import signal
import socket
import subprocess
import tempfile
import textwrap
import time
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('foundation_dev', Path(__file__).parents[1] / 'dev.py')
dev = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(dev)


class DevTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='foundation-dev-test-')
        self.addCleanup(self.temporary.cleanup)
        self.state = Path(self.temporary.name)
        self.patches = [patch.object(dev, 'STATE', self.state),
                        patch.object(dev, 'SETTINGS', self.state / 'settings.json'),
                        patch.dict(os.environ, {'HOME': str(self.state / 'home')})]
        for p in self.patches:
            p.start()
            self.addCleanup(p.stop)
        with patch.dict(os.environ, {'FOUNDATION_DEV_PROFILE': 'offline'}):
            self.config = dev.settings(create=True)

    def test_trades_snapshot_comes_from_a_local_file(self):
        snapshot = self.state / 'trades.sql.gz'
        snapshot.write_bytes(gzip.compress(b'-- trades'))
        with patch.dict(os.environ, {'FOUNDATION_DEV_TRADES_SNAPSHOT': str(snapshot)}):
            self.assertEqual(dev.trades_snapshot_file(), snapshot)
        with patch.dict(os.environ, {}, clear=False):
            os.environ.pop('FOUNDATION_DEV_TRADES_SNAPSHOT', None)
            self.assertIsNone(dev.trades_snapshot_file(), 'no file, no snapshot')

    def test_the_mirror_profile_serves_the_shop_while_the_first_trades_pass_runs(self):
        env = dev.runtime_env({**self.config, 'profile': 'mirror'})
        self.assertEqual(env['TRADES_SERVE_PARTIAL'], 'true')
        self.assertNotIn('TRADES_SERVE_PARTIAL', dev.runtime_env(self.config))

    def test_the_api_reaches_the_stacks_own_livekit_with_its_generated_pair(self):
        key, secret = self.config['LIVEKIT_API_KEY'], self.config['LIVEKIT_API_SECRET']
        self.assertNotIn(key.lower(), ('devkey', 'devsecret'))
        self.assertGreaterEqual(len(secret), 32, 'livekit-server refuses shorter secrets')
        signaling, tcp, udp = dev.livekit_ports(self.config)
        self.assertEqual(len({self.config['port'], self.config['port'] + 2, self.config['port'] + 3,
                              signaling, tcp, udp}), 6)
        self.assertTrue(all(self.config['port'] < port < self.config['port'] + 10 for port in (signaling, tcp, udp)),
                        'inside the stack\'s port block')
        for config in (self.config, {**self.config, 'profile': 'mirror'}):
            env = dev.runtime_env(config)
            self.assertEqual(env['LIVEKIT_HOST'], f'ws://127.0.0.1:{signaling}')
            self.assertEqual((env['LIVEKIT_API_KEY'], env['LIVEKIT_API_SECRET']), (key, secret))
            self.assertNotIn('LIVEKIT_ALLOW_DEV_CREDS', env)
        yaml = dev.livekit_yaml(self.config)
        for line in (f'port: {signaling}', f'  tcp_port: {tcp}', f'  udp_port: {udp}', '  - 127.0.0.1',
                     f'  {key}: {secret}', f'  api_key: {key}',
                     f"    - http://127.0.0.1:{self.config['port']}/comms-gatekeeper/livekit-webhook"):
            self.assertIn(line + '\n', yaml)
        livekit = dev.project(self.config)['processes']['livekit']
        self.assertEqual(livekit['readiness_probe']['http_get']['port'], signaling)
        self.assertIn('livekit', dev.SERVICES)

    def test_a_slow_postgres_probe_does_not_stop_a_healthy_database(self):
        probe = dev.project(self.config)['processes']['postgres']['readiness_probe']
        self.assertGreaterEqual(probe['timeout_seconds'], 10)

    def test_postgres_only_graph_has_no_web_build_or_api_dependencies(self):
        for profile in ('offline', 'mirror'):
            processes = dev.project({**self.config, 'profile': profile}, postgres_only=True)['processes']
            self.assertEqual(set(processes), {'postgres'})
            self.assertNotIn('depends_on', processes['postgres'])

    def test_postgres_only_readiness_prepares_scratch_collation_without_http(self):
        processes = subprocess.CompletedProcess([], 0, '[{"name":"postgres","is_running":true}]')
        with patch.object(dev, 'pc', return_value=processes), patch.object(dev, 'sql') as sql, \
                patch.object(dev, 'ensure_collation') as collation, patch.object(dev.urllib.request, 'urlopen') as http:
            self.assertEqual(dev.wait_ready(10, postgres_only=True), 0)
        sql.assert_called_once_with(self.config, 'SELECT 1', 'postgres')
        collation.assert_called_once_with(self.config, 'template1')
        http.assert_not_called()

    def test_full_stack_cannot_silently_reuse_a_postgres_only_supervisor(self):
        dev.save_json(self.state / 'process-compose.json', dev.project(self.config, postgres_only=True))
        with patch.object(dev, 'supervisor_running', return_value=True):
            self.assertIsNone(dev.prepare_supervisor(postgres_only=True))
            with self.assertRaisesRegex(RuntimeError, 'foundation-dev down'):
                dev.prepare_supervisor()

    def test_rust_suite_starts_only_postgres_and_still_stops_after_test_failure(self):
        tools = self.state / 'bin'
        tools.mkdir()
        nats = tools / 'nats-server'
        nats.write_text('#!/usr/bin/env bash\necho "nats:$*" >> "$CALLS"\n', newline='\n')
        nats.chmod(0o755)
        command = r'''
          if command -v cygpath >/dev/null; then FIXTURE_ROOT="$(cygpath -u "$FIXTURE_ROOT")"; fi
          export PATH="$FIXTURE_ROOT/bin:$PATH"
          foundation-dev() {
            echo "$*" >> "$CALLS"
            if [ "$1" = env ]; then
              printf 'FOUNDATION_DEV_ROOT=%q; FOUNDATION_DEV_TEST_DATABASE_URL=postgresql://fixture/postgres\n' "$FIXTURE_ROOT"
            fi
          }
          python3() { echo "python:$*" >> "$CALLS"; }
          cargo() { echo "cargo:$*" >> "$CALLS"; }
          make() {
            echo "make:${*: -1}" >> "$CALLS"
            [ "$1" = -C ] && [ "$2" -ef "$FOUNDATION_DEV_ROOT" ] && [ "$3" = catalyst-test ] || exit 78
            [ "$PWD" -ef "$FOUNDATION_DEV_ROOT" ] || exit 79
            [ "$CATALYST_TEST_DATABASE_URL" = postgresql://fixture/postgres ] || exit 80
            [ "$PULSE_NATS_SERVER_BIN" = "$(command -v nats-server)" ] || exit 81
            [ "$ARCHIPELAGO_NATS_SERVER_BIN" = "$PULSE_NATS_SERVER_BIN" ] || exit 82
            return 42
          }
          export -f foundation-dev python3 cargo make
          bash "$1" rust
        '''
        calls = self.state / 'calls'
        bash = str(Path(os.environ.get('ProgramFiles', 'C:/Program Files')) / 'Git/bin/bash.exe') if os.name == 'nt' else 'bash'
        result = subprocess.run([bash, '-c', command, 'test', (dev.ROOT / 'tooling/dev-test.sh').as_posix()],
                                env={**os.environ, 'CALLS': str(calls), 'FIXTURE_ROOT': self.state.as_posix(),
                                     'FOUNDATION_DEV_STARTUP_TIMEOUT': '3600'},
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 42, result.stderr)
        self.assertEqual(calls.read_text().splitlines(),
                         ['up --postgres-only', 'wait --postgres-only --timeout 3600', 'env',
                          'nats:--version',
                          "python:-m unittest discover -s tooling/tests -p test_dev_regressions.py",
                          "python:-m unittest discover -s tooling/tests -p test_enet_provenance.py",
                          'python:library/vendor/rusty_enet/verify.py', 'python:tooling/dev-regressions.py',
                          'cargo:check --locked --manifest-path library/vendor/rusty_enet/Cargo.toml --all-features --all-targets',
                          'cargo:test --locked --manifest-path library/vendor/rusty_enet/Cargo.toml --lib',
                          'make:catalyst-test', 'down'])

    @unittest.skipUnless(shutil.which('process-compose') and shutil.which('initdb') and shutil.which('psql')
                         and os.getuid() != 0, 'requires PostgreSQL and process-compose')
    def test_postgres_only_cli_preserves_rows_and_supplies_scratch_database_collation(self):
        checkout = self.state / 'checkout'
        script = checkout / 'tooling/dev.py'
        script.parent.mkdir(parents=True)
        shutil.copyfile(dev.__file__, script)
        def cli(*args):
            return subprocess.run([os.sys.executable, str(script), *args], check=True, capture_output=True,
                                  text=True, timeout=45, env={**os.environ, 'FOUNDATION_DEV_PROFILE': 'offline'})
        self.addCleanup(lambda: cli('down'))
        cli('up', '--postgres-only')
        cli('wait', '--postgres-only', '--timeout', '30')
        config = json.loads((checkout / '.cache/dev/settings.json').read_text())
        graph = json.loads((checkout / '.cache/dev/process-compose.json').read_text())
        self.assertEqual(set(graph['processes']), {'postgres'})
        dev.sql(config, 'CREATE DATABASE scratch_fixture', 'postgres')
        dev.sql(config, 'CREATE TABLE preserved(value text); INSERT INTO preserved VALUES (\'saved\')', 'scratch_fixture')
        self.assertEqual(dev.sql(config, 'SELECT value COLLATE "en_US.utf8" FROM preserved', 'scratch_fixture'), 'saved')
        cli('down')
        cli('up', '--postgres-only')
        cli('wait', '--postgres-only', '--timeout', '30')
        self.assertEqual(dev.sql(config, 'SELECT value FROM preserved', 'scratch_fixture'), 'saved')

    def test_livekit_gets_its_config_through_the_environment_not_argv(self):
        with patch.dict(os.environ, {'DATABASE_URL': 'postgresql://production.invalid/db'}), \
                patch.object(dev, 'started', return_value='start'), \
                patch.object(dev.os, 'execvpe') as execvpe:
            dev.task('livekit')
        program, argv, env = execvpe.call_args.args
        self.assertEqual((program, argv), ('livekit-server', ['livekit-server']))
        self.assertEqual(env['LIVEKIT_CONFIG'], dev.livekit_yaml(self.config))
        self.assertNotIn('DATABASE_URL', env)
        self.assertNotIn(self.config['LIVEKIT_API_SECRET'], ' '.join(argv))

    def test_pulse_polls_the_apis_bans_with_the_token_the_api_checks(self):
        token = self.config['MODERATOR_TOKEN']
        self.assertEqual(dev.runtime_env(self.config)['MODERATOR_TOKEN'], token)
        env = dev.pulse_env(self.config, self.state / 'cert.pem', self.state / 'key.pem')
        self.assertEqual(env['COMMS_MODERATOR_TOKEN'], token)
        self.assertEqual(env['PULSE_BANS_URL'], f"http://127.0.0.1:{self.config['port']}/comms-gatekeeper/bans")
        older = {k: v for k, v in self.config.items() if k != 'MODERATOR_TOKEN'}
        dev.save_json(dev.SETTINGS, older)
        self.assertEqual({k: dev.settings()[k] for k in older}, older, 'a stack from before the token keeps its secrets')
        self.assertTrue(dev.settings()['MODERATOR_TOKEN'])

    def test_a_stack_created_before_livekit_gets_a_pair_and_keeps_its_secrets(self):
        old = {k: v for k, v in self.config.items() if not k.startswith('LIVEKIT_')}
        dev.save_json(dev.SETTINGS, old)
        # Whichever command reads it first, not only startup: a task an older supervisor restarts
        # builds its environment from it.
        upgraded = dev.settings()
        self.assertEqual({k: upgraded[k] for k in old}, old)
        self.assertGreaterEqual(len(upgraded['LIVEKIT_API_SECRET']), 32)
        self.assertEqual(dev.runtime_env(upgraded)['LIVEKIT_API_SECRET'], upgraded['LIVEKIT_API_SECRET'])
        self.assertEqual(dev.settings(create=True), upgraded, 'the pair is stable once written')

    def test_pulse_serves_webtransport_on_the_sites_host_and_port(self):
        webtransport, enet, metrics = dev.pulse_ports(self.config)
        # The /play HUD hands the engine the page's own host:port as its Pulse server.
        self.assertEqual(webtransport, self.config['port'] + 2)
        self.assertNotIn(enet, (self.config['port'] + offset for offset in range(7)))
        self.assertTrue(self.config['port'] < enet < self.config['port'] + 10, "inside the stack's port block")
        certificate, key = self.state / 'cert.pem', self.state / 'key.pem'
        with patch.dict(os.environ, {'DATABASE_URL': 'postgresql://production.invalid/db',
                                     'PULSE_NATS_URL': 'nats://production.invalid:4222'}):
            env = dev.pulse_env(self.config, certificate, key)
        self.assertEqual(env['PULSE_WT_BIND'], f'127.0.0.1:{webtransport}')
        self.assertEqual((env['PULSE_BIND'], env['PULSE_METRICS_BIND']), (f'127.0.0.1:{enet}', f'127.0.0.1:{metrics}'))
        self.assertEqual(env['PULSE_WT_ENABLED'], 'true')
        self.assertEqual((env['PULSE_WT_CERT_PATH'], env['PULSE_WT_KEY_PATH']), (str(certificate), str(key)))
        self.assertTrue(Path(env['PULSE_REPLAY_STATE_PATH']).is_relative_to(self.state))
        self.assertNotIn('DATABASE_URL', env)
        self.assertEqual(env['PULSE_NATS_URL'], dev.nats_url(self.config), "the stack's own broker, not the shell's")
        pulse = dev.project(self.config)['processes']['pulse']
        self.assertEqual(pulse['readiness_probe']['http_get']['port'], metrics)
        self.assertEqual(pulse['depends_on'], {'api-build': {'condition': 'process_completed_successfully'},
                                               'nats': {'condition': 'process_healthy'}})
        self.assertIn('pulse', dev.SERVICES)
        with patch.object(dev, 'started', return_value='start'), \
                patch.object(dev, 'pulse_certificate', return_value=(certificate, key)), \
                patch.object(dev.os, 'execve') as execve:
            dev.task('pulse')
        binary, argv, env = execve.call_args.args
        self.assertEqual((Path(binary).name, argv[1:]), ('catalyst', ['pulse']))
        self.assertEqual(env, dev.pulse_env(self.config, certificate, key))

    def test_the_stacks_own_nats_carries_pulses_cluster_feed_to_archipelago(self):
        client, monitor = dev.nats_ports(self.config)
        taken = {self.config['port'], self.config['port'] + 3, *dev.livekit_ports(self.config), *dev.pulse_ports(self.config)}
        self.assertFalse({client, monitor} & taken)
        self.assertTrue(all(self.config['port'] < port < self.config['port'] + 10 for port in (client, monitor)),
                        "inside the stack's port block")
        self.assertEqual(dev.nats_url(self.config), f'nats://127.0.0.1:{client}')
        argv = dev.nats_argv(self.config)
        self.assertEqual(argv[0], 'nats-server')
        for flag, value in (('--addr', '127.0.0.1'), ('--port', str(client)), ('--http_port', str(monitor))):
            self.assertEqual(argv[argv.index(flag) + 1], value)
        self.assertNotIn('--jetstream', argv)
        for config in (self.config, {**self.config, 'profile': 'mirror'}):
            with patch.dict(os.environ, {'NATS_URL': 'nats://production.invalid:4222',
                                         'COMMS_FIXED_ADAPTER': 'offline:offline'}):
                env = dev.runtime_env(config)
            self.assertEqual(env['NATS_URL'], dev.nats_url(self.config))
            # Only the primary serve mounts /ws and runs the island subscriber.
            self.assertNotEqual(env.get('FLEET_JOBS_ENABLED', 'true'), 'false')
            # /about advertises the API's own ws-connector, and its health is Pulse's heartbeat over NATS.
            self.assertEqual(env['COMMS_FIXED_ADAPTER'], f"archipelago:ws://127.0.0.1:{self.config['port']}/ws")
            self.assertEqual(env['COMMS_ADAPTER'], 'fixed-adapter:' + env['COMMS_FIXED_ADAPTER'])
        processes = dev.project(self.config)['processes']
        self.assertEqual(processes['nats']['readiness_probe']['http_get'],
                         {'host': '127.0.0.1', 'port': monitor, 'path': '/healthz'})
        self.assertEqual(processes['api']['depends_on']['nats'], {'condition': 'process_healthy'})
        self.assertIn('nats', dev.SERVICES)
        with patch.dict(os.environ, {'DATABASE_URL': 'postgresql://production.invalid/db'}), \
                patch.object(dev, 'started', return_value='start'), \
                patch.object(dev.os, 'execvpe') as execvpe:
            dev.task('nats')
        program, argv, env = execvpe.call_args.args
        self.assertEqual((program, argv), ('nats-server', dev.nats_argv(self.config)))
        self.assertNotIn('DATABASE_URL', env)

    def test_a_busy_nats_port_moves_the_stack(self):
        base = self.config['port'] + 400
        if not dev.ports_free(base):
            self.skipTest('the test port block is busy')
        for port in dev.nats_ports({'port': base}):
            with socket.socket() as sock:
                sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                sock.bind(('127.0.0.1', port))
                sock.listen()
                self.assertFalse(dev.ports_free(base), port)

    @unittest.skipUnless(shutil.which('nats-server'), 'nats-server is supplied by nix develop')
    def test_nats_server_answers_its_readiness_probe(self):
        client, monitor = dev.nats_ports(self.config)
        if not dev.ports_free(self.config['port']):
            self.skipTest('the test port block is busy')
        server = subprocess.Popen(dev.nats_argv(self.config), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

        def stop():
            server.terminate()
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
        self.addCleanup(stop)
        deadline = time.monotonic() + 20
        while True:
            try:
                with dev.urllib.request.urlopen(f'http://127.0.0.1:{monitor}/healthz', timeout=1) as response:
                    self.assertEqual(json.loads(response.read()), {'status': 'ok'})
                break
            except OSError:
                self.assertIsNone(server.poll(), 'nats-server exited')
                self.assertLess(time.monotonic(), deadline, 'nats-server did not answer')
                time.sleep(.2)
        with socket.create_connection(('127.0.0.1', client), timeout=5) as sock:
            self.assertTrue(sock.recv(4096).startswith(b'INFO '), 'a NATS client port')

    def test_a_busy_udp_port_in_the_block_moves_the_stack(self):
        base = self.config['port'] + 400
        if not dev.ports_free(base):
            self.skipTest('the test port block is busy')
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
            sock.bind(('127.0.0.1', base + 2))
            self.assertFalse(dev.ports_free(base), 'Pulse needs UDP on the Sites port')

    @unittest.skipUnless(shutil.which('openssl'), 'openssl is supplied by nix develop')
    def test_pulse_keeps_its_key_and_renews_a_short_lived_certificate(self):
        certificate, key = dev.pulse_certificate()
        self.assertEqual(key.stat().st_mode & 0o777, 0o600)
        spki, pem = dev.pulse_spki(), key.read_bytes()
        renewed, _ = dev.pulse_certificate()
        self.assertEqual((key.read_bytes(), dev.pulse_spki()), (pem, spki), 'the SPKI Chrome is told about stays put')
        text = subprocess.run(['openssl', 'x509', '-in', renewed, '-noout', '-text', '-enddate'],
                              capture_output=True, text=True, check=True).stdout
        self.assertIn('IP Address:127.0.0.1', text)
        self.assertIn('ecPublicKey', text)
        end = subprocess.run(['openssl', 'x509', '-in', renewed, '-noout', '-checkend', str(14 * 86400)],
                             capture_output=True, text=True)
        self.assertNotEqual(end.returncode, 0, 'serverCertificateHashes accepts at most 14 days')
        der = subprocess.run(['openssl', 'x509', '-in', renewed, '-noout', '-pubkey'], capture_output=True, check=True).stdout
        der = subprocess.run(['openssl', 'pkey', '-pubin', '-outform', 'DER'], input=der, capture_output=True,
                             check=True).stdout
        self.assertEqual(spki, base64.b64encode(hashlib.sha256(der).digest()).decode())

    def test_about_advertises_pulses_webtransport_endpoint(self):
        env = dev.pulse_about_env(self.config)
        self.assertNotIn('COMMS_FIXED_ADAPTER', env, 'the realm keeps the archipelago adapter of runtime_env')
        self.assertEqual(env['COMMS_V4_PULSE_WEBTRANSPORT_URL'], f'https://127.0.0.1:{dev.pulse_ports(self.config)[0]}')
        self.assertTrue(env['COMMS_V4_PULSE_AUDIENCE'])

    @unittest.skipUnless(shutil.which('livekit-server'), 'livekit-server is supplied by nix develop')
    def test_livekit_server_starts_on_the_generated_config(self):
        signaling = dev.livekit_ports(self.config)[0]
        if not dev.ports_free(self.config['port']):
            self.skipTest('the test port block is busy')
        server = subprocess.Popen(['livekit-server'], env={'PATH': os.environ['PATH'],
                                                            'LIVEKIT_CONFIG': dev.livekit_yaml(self.config)},
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

        def stop():
            server.terminate()
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
        self.addCleanup(stop)
        deadline = time.monotonic() + 20
        while True:
            try:
                with socket.create_connection(('127.0.0.1', signaling), timeout=1) as sock:
                    sock.sendall(b'GET / HTTP/1.0\r\n\r\n')
                    self.assertIn(b'OK', sock.recv(4096))
                    break
            except OSError:
                self.assertIsNone(server.poll(), 'livekit-server exited')
                self.assertLess(time.monotonic(), deadline, 'livekit-server did not answer')
                time.sleep(.2)

    def test_persistent_settings_do_not_rotate_secrets(self):
        warning = io.StringIO()
        with patch.dict(os.environ, {'FOUNDATION_DEV_PROFILE': 'mirror', 'FOUNDATION_DEV_PORT': '6000'}), \
                contextlib.redirect_stderr(warning):
            self.assertEqual(dev.settings(create=True), self.config)
        self.assertIn(f"Using saved port {self.config['port']}", warning.getvalue())

    def test_missing_settings_cannot_rotate_credentials_for_existing_data(self):
        data = self.state / 'postgres'
        data.mkdir()
        marker = data / 'PG_VERSION'
        marker.write_text('18')
        saved = dev.SETTINGS.read_bytes()
        dev.SETTINGS.unlink()
        with self.assertRaisesRegex(RuntimeError, 'Restore the original'):
            dev.settings(create=True)
        self.assertFalse(dev.SETTINGS.exists())
        self.assertEqual(marker.read_text(), '18')
        dev.SETTINGS.write_bytes(saved)
        self.assertEqual(dev.settings(), self.config)

    def test_runtime_does_not_inherit_operator_configuration(self):
        with patch.dict(os.environ, {'DATABASE_URL': 'postgresql://production.invalid/db',
                                    'NODE_POSTURE': 'mirror', 'SYNC_ENABLED': 'true',
                                    'STRIPE_SECRET_KEY': 'not-a-real-key', 'ENCRYPTION_KEY_FILE': '/unused',
                                    'CONTENT_FORWARD_UPSTREAM_URL': 'https://production.invalid'}):
            env = dev.runtime_env(self.config)
        self.assertEqual(env['DATABASE_URL'], dev.database_url(self.config))
        self.assertEqual(env['CATALYST_DATABASE_URL'], env['DATABASE_URL'])
        self.assertEqual(env['NODE_POSTURE'], 'journey')
        self.assertNotIn('STRIPE_SECRET_KEY', env)
        self.assertNotIn('ENCRYPTION_KEY_FILE', env)
        self.assertEqual(env['CONTENT_FORWARD_UPSTREAM_URL'], '')
        self.assertEqual(env['SYNC_ENABLED'], 'false')

    def test_mirror_fetches_catalogues_and_requested_content_only(self):
        env = dev.runtime_env({**self.config, 'profile': 'mirror'})
        self.assertNotIn('NODE_POSTURE', env)
        self.assertEqual(env['READ_THROUGH_ENABLED'], 'true')
        self.assertTrue(env['SYNC_SOURCE'].startswith('https://'))
        # Worlds read through from their content server; the full worlds pass stays off.
        self.assertTrue(env['WORLDS_CONTENT_MIRROR_SOURCE'].startswith('https://'))
        self.assertNotIn('WORLDS_CONTENT_MIRROR_SOURCE', dev.runtime_env(self.config))
        self.assertEqual(env['WORLDS_READ_THROUGH_TRUST_UPSTREAM_GRANTS'], 'true')
        self.assertNotIn('WORLDS_READ_THROUGH_TRUST_UPSTREAM_GRANTS', dev.runtime_env(self.config))
        for key in ('SYNC_ENABLED', 'WORLDS_CONTENT_MIRROR_ENABLED', 'SCENE_HOST_ENABLED',
                    'MARKETPLACE_TRADE_EVENTS_INDEX_ENABLED', 'CREDITS_USAGE_INDEX_ENABLED'):
            self.assertEqual(env[key], 'false', key)
        # The Shop's catalogue follows the public subgraphs and chain, as `posture show mirror` does.
        for key in ('MARKETPLACE_MIRRORS_ENABLED', 'MARKETPLACE_NFT_INDEX_ENABLED', 'LAND_AUTHZ_INDEX_ENABLED'):
            self.assertEqual(env[key], 'true', key)
        self.assertTrue(env['MATIC_COLLECTIONS_SUBGRAPH_URL'].startswith('https://'))
        self.assertIn('index', dev.project({**self.config, 'profile': 'mirror'})['processes'])
        self.assertNotIn('index', dev.project(self.config)['processes'])
        # A loaded machine slows the probe's Python start; a 1 s timeout would stop the database.
        self.assertGreaterEqual(dev.project(self.config)['processes']['postgres']['readiness_probe']['timeout_seconds'], 10)
        # Every supervised process is a task the CLI accepts.
        for name, process in dev.project({**self.config, 'profile': 'mirror'})['processes'].items():
            task = shlex.split(process['command'].removeprefix('exec '))[-1]
            with patch.object(dev.sys, 'argv', ['foundation-dev', '_task', task]), patch.object(dev, 'task') as run:
                dev.main()
            run.assert_called_once_with(task)
        offline = dev.runtime_env(self.config)
        self.assertEqual(offline['MARKETPLACE_MIRRORS_ENABLED'], 'false')
        self.assertEqual(env['MEDIA_CONVERT_REMOTE_SOURCES'], 'true')
        self.assertNotIn('MEDIA_CONVERT_REMOTE_SOURCES', dev.runtime_env(self.config))
        self.assertEqual(env['PROFILE_IMAGES_ORIGIN_URL'], 'https://profile-images.decentraland.org')
        self.assertNotIn('PROFILE_IMAGES_ORIGIN_URL', offline)
        self.assertEqual(env['EXPLORER_MAP_SOURCE_URL'], 'https://api.decentraland.org')
        self.assertEqual(env['PRICE_POLL_ENABLED'], 'true')
        self.assertNotIn('WORLD_MANIFEST_UPSTREAM_URL', env)
        self.assertNotIn('PULSE_ROOM_AUTHORITY_KEY', env)
        for key in ('STORAGE_ROOT_FOLDER', 'AB_CDN_CACHE_DIR', 'PROFILE_IMAGES_CACHE_DIR', 'BVIMPOSTERS_STORE_ROOT'):
            self.assertTrue(Path(env[key]).is_relative_to(self.state))

    def test_env_names_the_running_stack_for_shell_evaluation(self):
        out = io.StringIO()
        with patch.object(dev.sys, 'argv', ['foundation-dev', 'env']), contextlib.redirect_stdout(out):
            self.assertEqual(dev.main(), 0)
        values = dict(line.split('=', 1) for line in out.getvalue().splitlines())
        self.assertEqual(values['FOUNDATION_DEV_API_URL'], f"http://127.0.0.1:{self.config['port']}")
        self.assertEqual(values['FOUNDATION_DEV_DATABASE_URL'], dev.database_url(self.config))
        result = subprocess.run(['bash', '-c', 'eval "$1"; echo "$FOUNDATION_DEV_SITES_URL"', '-', out.getvalue()],
                                capture_output=True, text=True, check=True)
        self.assertEqual(result.stdout.strip(), f"http://127.0.0.1:{self.config['port'] + 2}")

    def test_schema_stamp_selects_migrate_without_reset(self):
        with patch.object(dev, 'sql', side_effect=['1', 't', 't', 't']) as sql, patch.object(dev, 'run') as run:
            dev.apply_schema(self.config, 'catalyst')
        self.assertEqual(run.call_args.args[0], ['catalyst', 'schema', 'migrate'])
        self.assertEqual(sql.call_count, 4)
        self.assertEqual(sql.call_args_list[2].args[2], 'template1')
        with patch.object(dev, 'sql', side_effect=['', '', 't', 't', 'f']) as sql, patch.object(dev, 'run') as run:
            dev.apply_schema(self.config, 'catalyst')
        self.assertEqual(sql.call_args_list[1].args[1], 'CREATE DATABASE catalyst')
        self.assertEqual(run.call_args.args[0], ['catalyst', 'schema', 'apply'])

    def test_migration_failure_propagates(self):
        with patch.object(dev, 'sql', side_effect=['1', 't', 't', 't']), patch.object(
            dev, 'run', side_effect=subprocess.CalledProcessError(1, ['catalyst', 'schema', 'migrate'])
        ) as run:
            with self.assertRaises(subprocess.CalledProcessError):
                dev.apply_schema(self.config, 'catalyst')
        self.assertEqual(run.call_count, 1)

    def test_concurrent_bootstrap_then_migrate_is_serialized(self):
        # Two real manager processes share a minimal database stand-in. A
        # duplicate CREATE DATABASE fails, and migration requires the stamp.
        script = self.state / 'schema-race.py'
        script.write_text(textwrap.dedent('''
            import importlib.util, pathlib, sys, time
            spec = importlib.util.spec_from_file_location('dev', sys.argv[1])
            d = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(d)
            d.STATE = pathlib.Path(sys.argv[2])
            d.SETTINGS = d.STATE / 'settings.json'
            database, stamp = d.STATE / 'database', d.STATE / 'stamp'
            def sql(config, query, db='catalyst'):
                if 'pg_database' in query:
                    exists = database.exists()
                    time.sleep(.2)
                    return '1' if exists else ''
                if query == 'CREATE DATABASE catalyst':
                    database.touch(exist_ok=False)
                    return ''
                if 'pg_collation' in query:
                    return 't'
                if 'meta.schema' in query:
                    return 't' if stamp.exists() else 'f'
                raise AssertionError(query)
            def run(argv, **kwargs):
                with (d.STATE / 'actions').open('a') as log:
                    log.write(argv[2] + '\\n')
                time.sleep(.2)
                stamp.touch()
            d.sql, d.run = sql, run
            d.apply_schema(d.settings(), 'catalyst')
        '''))
        children = [subprocess.Popen([os.sys.executable, str(script), str(dev.__file__), str(self.state)],
                                     stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) for _ in range(2)]
        for child in children:
            self.addCleanup(lambda child=child: child.poll() is None and child.kill())
        for child in children:
            stdout, stderr = child.communicate(timeout=10)
            self.assertEqual(child.returncode, 0, stdout + stderr)
        self.assertEqual((self.state / 'actions').read_text().splitlines(), ['apply', 'migrate'])

    def test_port_conflict_detected_without_connecting_to_existing_database(self):
        with socket.socket() as listener:
            listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            listener.bind(('127.0.0.1', 0))
            listener.listen()
            self.assertFalse(dev.ports_free(listener.getsockname()[1]))

    def test_closed_http_connections_do_not_block_restart(self):
        port = self.config['port']
        with socket.socket() as listener:
            listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            listener.bind(('127.0.0.1', port))
            listener.listen()
            with socket.create_connection(('127.0.0.1', port)) as client:
                connection, _ = listener.accept()
                connection.close()
                self.assertEqual(client.recv(1), b'')
        self.assertTrue(dev.ports_free(port))

    def test_incomplete_postgres_initialization_is_preserved(self):
        data = self.state / 'postgres'
        data.mkdir()
        marker = data / 'important-data'
        marker.write_text('keep')
        with self.assertRaisesRegex(RuntimeError, 'Incomplete'):
            dev.postgres(self.config)
        self.assertEqual(marker.read_text(), 'keep')

    def test_noninteractive_shell_does_not_start_services(self):
        hook = dev.ROOT / 'infra/dev/shell-hook.sh'
        command = 'foundation-dev() { echo unexpected-start; }; source "$1"'
        result = subprocess.run(['bash', '-c', command, 'test', str(hook)], cwd=dev.ROOT,
                                text=True, capture_output=True, check=True)
        self.assertNotIn('unexpected-start', result.stdout)

    def test_interactive_shell_starts_services_unless_disabled(self):
        hook = dev.ROOT / 'infra/dev/shell-hook.sh'
        command = 'foundation-dev() { echo "dev-command:$*"; }; source "$1"'
        for enabled in ('1', '0'):
            result = subprocess.run(['bash', '--noprofile', '--norc', '-ic', command, 'test', str(hook)],
                                    cwd=dev.ROOT, text=True, capture_output=True, check=True,
                                    env={**os.environ, 'FOUNDATION_DEV_AUTOSTART': enabled})
            self.assertEqual('dev-command:up' in result.stdout, enabled == '1')

    def test_wait_reports_failed_build_without_waiting_for_timeout(self):
        failed = subprocess.CompletedProcess([], 0, json.dumps([
            {'name': 'api-build', 'exit_code': 101, 'is_running': False}
        ]))
        with patch.object(dev, 'pc', return_value=failed), patch.object(dev.time, 'sleep') as sleep:
            with self.assertRaisesRegex(RuntimeError, 'api-build failed'):
                dev.wait_ready(10)
        sleep.assert_not_called()

    def test_first_pass_pending_lists_configured_mirrors_without_a_complete_pass(self):
        status = {'data': {
            'configured_mirrors': {'places': 'https://p', 'worlds': 'https://p', 'events': None},
            'mirrors': [
                {'mirror': 'places', 'last_complete_pass_at': None, 'last_error': None},
                {'mirror': 'worlds', 'last_complete_pass_at': '2026-10-20T00:00:00Z', 'last_error': None},
                {'mirror': 'events', 'last_complete_pass_at': None, 'last_error': 'unconfigured'},
            ],
        }}
        self.assertEqual(dev.first_pass_pending(status), (['places'], {}))
        status['data']['mirrors'][0]['last_error'] = 'HTTP 502'
        self.assertEqual(dev.first_pass_pending(status), (['places'], {'places': 'HTTP 502'}))
        status['data']['configured_mirrors'] = {'places': None, 'worlds': None, 'events': None}
        self.assertEqual(dev.first_pass_pending(status), ([], {}))

    def test_down_reports_unavailable_supervisor_with_occupied_port(self):
        with socket.socket() as listener:
            listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            listener.bind(('127.0.0.1', self.config['port']))
            listener.listen()
            with patch.object(dev, 'pc', return_value=subprocess.CompletedProcess([], 1)):
                with self.assertRaisesRegex(RuntimeError, 'no processes were stopped'):
                    dev.down()

    def test_down_preserves_stale_postgres_pid_file_without_error(self):
        child = subprocess.Popen([os.sys.executable, '-c', 'pass'])
        child.wait(timeout=10)
        pidfile = self.state / 'postgres/postmaster.pid'
        pidfile.parent.mkdir()
        contents = f'{child.pid}\nremaining PostgreSQL metadata\n'
        pidfile.write_text(contents)
        with patch.object(dev, 'pc', return_value=subprocess.CompletedProcess([], 1)):
            self.assertEqual(dev.down(), 0)
        self.assertEqual(pidfile.read_text(), contents)

    def test_down_reports_live_postgres_pid_without_stopping_it(self):
        pidfile = self.state / 'postgres/postmaster.pid'
        pidfile.parent.mkdir()
        pidfile.write_text(f'{os.getpid()}\n')
        for denied in (False, True):
            with self.subTest(permission_denied=denied), \
                    patch.object(dev, 'pc', return_value=subprocess.CompletedProcess([], 1)), \
                    patch.object(dev.os, 'kill', wraps=os.kill,
                                 side_effect=PermissionError() if denied else None) as kill:
                with self.assertRaisesRegex(RuntimeError, f'PID {os.getpid()} recorded by PostgreSQL is alive'):
                    dev.down()
                kill.assert_called_once_with(os.getpid(), 0)
        self.assertTrue(pidfile.exists())

    def test_down_rejects_invalid_postgres_pid_without_signalling(self):
        pidfile = self.state / 'postgres/postmaster.pid'
        pidfile.parent.mkdir()
        for value in ('', 'invalid', '0', '-1'):
            with self.subTest(pid=value), \
                    patch.object(dev, 'pc', return_value=subprocess.CompletedProcess([], 1)), \
                    patch.object(dev.os, 'kill') as kill:
                pidfile.write_text(value)
                with self.assertRaisesRegex(RuntimeError, 'Invalid PostgreSQL PID file'):
                    dev.down()
                kill.assert_not_called()
                self.assertEqual(pidfile.read_text(), value)

    def test_smoke_stops_services_when_up_fails(self):
        log = self.state / 'smoke-commands'
        command = 'foundation-dev() { echo "$1" >> "$TEST_LOG"; [ "$1" != up ]; }; export -f foundation-dev; bash "$1"'
        result = subprocess.run(['bash', '-c', command, 'test', str(dev.ROOT / 'tooling/dev-smoke.sh')],
                                env={**os.environ, 'TEST_LOG': str(log)}, capture_output=True, text=True)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertEqual(log.read_text().splitlines(), ['up', 'down'])

    @unittest.skipUnless(shutil.which('process-compose'), 'process-compose is supplied by nix develop')
    def test_persistent_supervisor_socket_works_in_long_checkout_path(self):
        state = self.state / ('checkout-' + 'x' * 120) / '.cache/dev'
        state.mkdir(parents=True)
        project = state / 'test.json'
        project.write_text(json.dumps({'version': '0.5', 'processes': {
            'hold': {'command': 'sleep 60'}
        }}))
        with patch.object(dev, 'STATE', state), patch.object(dev, 'ROOT', self.state):
            self.assertEqual(dev.socket_path(), state / 'pc.sock')
            digest = dev.hashlib.sha256(str(dev.ROOT).encode()).hexdigest()[:16]
            legacy = Path('/tmp') / f'foundation-dev-{os.getuid()}-{digest}' / 'pc.sock'
            legacy.parent.mkdir(mode=0o700)
            self.addCleanup(legacy.parent.rmdir)
            self.addCleanup(lambda: legacy.unlink(missing_ok=True))
            # An old, stale control socket must not put a newly started stack
            # back in /tmp, or keep down waiting after its own socket closes.
            legacy.touch()
            self.assertEqual(dev.socket_path(), legacy)
            try:
                dev.pc('up', '--config', str(project), '--disable-dotenv', '--detached', '--tui=false',
                       check=True, capture_output=True, text=True)
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline:
                    result = dev.pc('process', 'list', '--output', 'json', capture_output=True, text=True)
                    if result.returncode == 0:
                        break
                    time.sleep(.1)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertTrue((state / 'pc.sock').is_socket())
                self.assertEqual(json.loads(result.stdout)[0]['name'], 'hold')
            finally:
                dev.down()
            self.assertFalse((state / 'pc.sock').exists())

    def install_service(self, system):
        with patch.object(dev, 'DARWIN', system == 'Darwin'), \
                patch.object(dev.shutil, 'which', return_value='/nix/bin/nix'), \
                patch.object(dev, 'systemctl') as systemctl, patch.object(dev, 'down') as down, \
                patch.object(dev, 'service_start') as start, contextlib.redirect_stdout(io.StringIO()):
            dev.service_install()
            path = dev.service_file()
        down.assert_called_once()
        start.assert_called_once()
        return path, systemctl

    def test_systemd_unit_supervises_in_the_foreground_and_restarts_only_after_failure(self):
        path, systemctl = self.install_service('Linux')
        unit = path.read_text()
        self.assertEqual(path.parent, self.state / 'home/.config/systemd/user')
        self.assertIn(f'ExecStart=/nix/bin/nix develop {dev.ROOT} --command foundation-dev _supervise', unit)
        self.assertIn('Restart=on-failure', unit)
        self.assertIn('WantedBy=default.target', unit)
        systemctl.assert_any_call('enable', dev.service_name())

    def test_launch_agent_supervises_in_the_foreground_and_restarts_only_after_failure(self):
        path, systemctl = self.install_service('Darwin')
        agent = plistlib.loads(path.read_bytes())
        self.assertEqual(path.parent, self.state / 'home/Library/LaunchAgents')
        self.assertEqual(agent['ProgramArguments'],
                         ['/nix/bin/nix', 'develop', str(dev.ROOT), '--command', 'foundation-dev', '_supervise'])
        self.assertEqual(agent['KeepAlive'], {'SuccessfulExit': False})
        self.assertTrue(agent['RunAtLoad'])
        systemctl.assert_not_called()

    def test_up_and_down_use_the_installed_service(self):
        self.install_service('Linux')
        with patch.object(dev, 'DARWIN', False), \
                patch.object(dev, 'service_start') as start, patch.object(dev, 'service_stop') as stop, \
                patch.object(dev, 'pc', return_value=subprocess.CompletedProcess([], 1)) as pc, \
                contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(dev.up(), 0)
            start.assert_called_once()
            pc.assert_not_called()
            self.assertEqual(dev.down(), 0)
            stop.assert_called_once()

    def test_foreground_supervisor_stops_only_verified_orphans(self):
        orphan = subprocess.Popen(['sleep', '60'], start_new_session=True)
        self.addCleanup(orphan.kill)
        unrelated = subprocess.Popen(['sleep', '60'], start_new_session=True)
        self.addCleanup(unrelated.kill)
        tasks = self.state / 'tasks'
        tasks.mkdir()
        dev.save_json(tasks / 'api.json', {'pid': orphan.pid, 'started': dev.started(orphan.pid)})
        dev.save_json(tasks / 'sites.json', {'pid': unrelated.pid, 'started': 'Thu Jan  1 00:00:00 1970'})
        with contextlib.redirect_stderr(io.StringIO()):
            dev.reap_orphans()
        self.assertEqual(orphan.wait(timeout=10), -signal.SIGTERM)
        self.assertIsNone(unrelated.poll())
        self.assertEqual(list(tasks.iterdir()), [])

    def test_foreground_supervisor_refuses_a_running_detached_stack(self):
        with patch.object(dev, 'supervisor_running', return_value=True), \
                patch.object(dev.os, 'execvpe') as execvpe:
            with self.assertRaisesRegex(RuntimeError, 'detached development stack'):
                dev.supervise()
        execvpe.assert_not_called()

    @unittest.skipUnless(shutil.which('process-compose'), 'process-compose is supplied by nix develop')
    def test_supervisor_accepts_generated_configuration(self):
        path = self.state / 'process-compose.json'
        path.write_text(json.dumps(dev.project(self.config)))
        subprocess.run(['process-compose', '--log-file', str(self.state / 'pc.log'),
                        'up', '--config', str(path), '--disable-dotenv', '--dry-run'],
                       check=True, capture_output=True)

    @unittest.skipUnless(shutil.which('initdb') and shutil.which('psql') and os.getuid() != 0,
                         'requires PostgreSQL and a non-root user (provided by nix develop)')
    def test_postgres_restart_preserves_rows_and_rejects_wrong_password(self):
        script = Path(dev.__file__)
        launch = ('import importlib.util, pathlib; '
                  f's=importlib.util.spec_from_file_location("dev", {str(script)!r}); '
                  'd=importlib.util.module_from_spec(s); s.loader.exec_module(d); '
                  f'd.STATE=pathlib.Path({str(self.state)!r}); '
                  f'd.postgres({self.config!r})')
        log = (self.state / 'postgres.log').open('w')
        def stop(process):
            if process.poll() is None:
                process.send_signal(signal.SIGINT)
                process.wait(timeout=15)
        def start():
            process = subprocess.Popen([os.sys.executable, '-c', launch], stdout=log, stderr=log)
            self.addCleanup(lambda: stop(process))
            deadline = time.monotonic() + 30
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    self.fail((self.state / 'postgres.log').read_text())
                try:
                    dev.sql(self.config, 'SELECT 1', 'postgres')
                    return process
                except subprocess.CalledProcessError:
                    time.sleep(.1)
            self.fail('PostgreSQL startup timed out')
        try:
            process = start()
            dev.ensure_collation(self.config, 'postgres')
            order = dev.sql(self.config, '''SELECT string_agg(name, ',' ORDER BY name COLLATE "en_US.utf8")
                FROM unnest(ARRAY['move-04.glb','game.js','OpenFork_0.glb','move_02.glb','scene.json']) AS name''', 'postgres')
            self.assertEqual(order, 'game.js,move_02.glb,move-04.glb,OpenFork_0.glb,scene.json')
            # The API pins search_path=content,meta; the collation must resolve without public.
            self.assertEqual(dev.sql(self.config, '''SET search_path = information_schema;
                SELECT 'ok' COLLATE "en_US.utf8"''', 'postgres'), 'ok')
            dev.sql(self.config, "CREATE TABLE preserved(value text); INSERT INTO preserved VALUES ('saved')", 'postgres')
            with self.assertRaises(subprocess.CalledProcessError):
                dev.sql({**self.config, 'password': 'wrong'}, 'SELECT 1', 'postgres')
            stop(process)
            process = start()
            dev.ensure_collation(self.config, 'postgres')
            self.assertEqual(dev.sql(self.config, 'SELECT value FROM preserved', 'postgres'), 'saved')
            stop(process)
        finally:
            log.close()


if __name__ == '__main__':
    unittest.main()
