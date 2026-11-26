"""Development configuration, schema selection and PostgreSQL lifecycle checks."""
import importlib.util
import base64
import contextlib
import hashlib
import hmac
import gzip
import io
import shlex
import json
import os
from pathlib import Path
import plistlib
import shutil
import signal
import select
import socket
import subprocess
import tempfile
import textwrap
import time
import unittest
import urllib.request
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('foundation_dev', Path(__file__).parents[1] / 'dev.py')
dev = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(dev)


class DevTest(unittest.TestCase):
    def setUp(self):
        # Under /tmp so the stack's own temporary folder stays inside it (see supervisor_temporary).
        self.temporary = tempfile.TemporaryDirectory(prefix='foundation-dev-test-', dir='/tmp')
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

    def test_a_dev_stack_starts_with_the_marketplace_override_on_and_keeps_a_developers_own(self):
        queries = []
        with patch.object(dev, 'sql', lambda config, query, database='catalyst': queries.append(query) or ''):
            dev.seed_dev_flags(self.config)
        self.assertEqual(len(queries), 1)
        self.assertIn("('dapps', 'dapps-shop-marketplace-features', true)", queries[0])
        self.assertIn('ON CONFLICT (app, name) DO NOTHING', queries[0])

    def test_the_stack_keeps_its_temporary_folder_in_a_short_checkout(self):
        temporary = dev.supervisor_temporary()
        self.assertEqual(temporary, self.state / 'tmp')
        self.assertEqual(temporary.stat().st_mode & 0o777, 0o700)

    def deep_state(self):
        deep = self.state / ('d' * 80)
        deep.mkdir()
        return patch.object(dev, 'STATE', deep)

    def test_a_deep_checkout_gets_a_temporary_folder_the_ingress_socket_fits_in(self):
        with self.deep_state():
            temporary = dev.supervisor_temporary()
        self.addCleanup(temporary.rmdir)
        self.assertTrue(str(temporary).startswith('/tmp/foundation-dev-'))
        self.assertLessEqual(len(f'{temporary}/RT_tbf92YMdmLWi/service_ipc.sock'), dev.SOCKET_PATH_MAX)
        self.assertEqual(temporary.stat().st_mode & 0o777, 0o700)

    def test_a_short_temporary_folder_others_can_use_is_refused(self):
        with self.deep_state():
            temporary = dev.supervisor_temporary()
            self.addCleanup(temporary.rmdir)
            temporary.chmod(0o777)
            with self.assertRaisesRegex(RuntimeError, 'only you can use'):
                dev.supervisor_temporary()

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

    def test_stream_keys_reach_the_stacks_own_rtmp_ingress(self):
        valkey, rtmp, relay, health = dev.ingress_ports(self.config)
        block = range(self.config['port'], self.config['port'] + 10)
        self.assertFalse({valkey, rtmp, relay, health} & set(block), "outside the stack's full block")
        self.assertEqual(len({valkey, rtmp, relay, health}), 4)
        # LiveKit and the ingress meet in the stack's Valkey; the server hands out the ingress's RTMP URL.
        yaml = dev.livekit_yaml(self.config)
        password = self.config['VALKEY_PASSWORD']
        self.assertGreaterEqual(len(password), 32)
        for line in ('redis:', f'  address: 127.0.0.1:{valkey}', f'  password: {password}', 'ingress:',
                     f'  rtmp_base_url: rtmp://127.0.0.1:{rtmp}/x'):
            self.assertIn(line + '\n', yaml)
        ingress = dev.ingress_yaml(self.config)
        signaling = dev.livekit_ports(self.config)[0]
        for line in (f"api_key: {self.config['LIVEKIT_API_KEY']}", f"api_secret: {self.config['LIVEKIT_API_SECRET']}",
                     f'ws_url: ws://127.0.0.1:{signaling}', f'  address: 127.0.0.1:{valkey}', f'  password: {password}',
                     f'rtmp_port: {rtmp}', 'whip_port: -1', f'http_relay_port: {relay}', f'health_port: {health}',
                     # A busy development machine still takes a stream.
                     '  rtmp_cpu_cost: 0.01', '  min_idle_ratio: 0.001'):
            self.assertIn(line + '\n', ingress)
        processes = dev.project(self.config)['processes']
        self.assertEqual(processes['livekit']['depends_on'], {'valkey': {'condition': 'process_healthy'}})
        self.assertEqual(processes['ingress']['depends_on'], {'livekit': {'condition': 'process_healthy'}})
        self.assertEqual(processes['ingress']['readiness_probe']['http_get'],
                         {'host': '127.0.0.1', 'port': health, 'path': '/'})
        self.assertIn('valkey-ready', processes['valkey']['readiness_probe']['exec']['command'])
        self.assertTrue({'valkey', 'ingress'} <= set(dev.SERVICES))
        for line in ('bind 127.0.0.1', f'port {valkey}', f'requirepass {password}', 'save ""', 'appendonly no'):
            self.assertIn(line + '\n', dev.valkey_conf(self.config))
        with patch.object(dev, 'started', return_value='start'), patch.object(dev.os, 'execvpe') as execvpe:
            dev.task('valkey')
        program, argv, env = execvpe.call_args.args
        conf = self.state / 'valkey.conf'
        self.assertEqual((program, argv), ('valkey-server', ['valkey-server', str(conf)]))
        self.assertNotIn(password, ' '.join(argv), 'the password stays out of ps')
        self.assertEqual(conf.read_text(), dev.valkey_conf(self.config))
        self.assertEqual(conf.stat().st_mode & 0o777, 0o600)
        with patch.object(dev, 'started', return_value='start'), patch.object(dev.os, 'execvpe') as execvpe:
            dev.task('ingress')
        program, argv, env = execvpe.call_args.args
        self.assertEqual((program, argv), ('ingress', ['ingress']))
        self.assertEqual(env['INGRESS_CONFIG_BODY'], ingress, 'the secret travels in the environment, not argv')
        self.assertIn('PATH', env, 'the ingress starts its per-stream handlers from PATH')
        self.assertEqual(env['GST_PLUGIN_FEATURE_RANK'], 'avdec_h264:MAX', 'a hardware decoder ends the stream')

    def test_behind_a_public_origin_streamers_get_the_origins_host(self):
        for origin, host in (('https://dev.example:8443', 'dev.example'), ('https://[2001:db8::1]', '[2001:db8::1]')):
            config = {**self.config, 'public_origin': origin}
            rtmp = dev.ingress_ports(config)[1]
            self.assertEqual(dev.rtmp_base_url(config), f'rtmp://{host}:{rtmp}/x')
            self.assertIn(f'  rtmp_base_url: {dev.rtmp_base_url(config)}\n', dev.livekit_yaml(config))
            # status prints what the server hands out.
            with patch.object(dev, 'pc', return_value=subprocess.CompletedProcess([], 0)), \
                    patch.object(dev, 'tls_files', return_value=None), contextlib.redirect_stdout(io.StringIO()) as out, \
                    patch.object(dev, 'settings', return_value=config):
                dev.status()
            self.assertIn(f'RTMP ingress: {dev.rtmp_base_url(config)}', out.getvalue())

    def test_a_stack_running_since_before_the_ingress_asks_for_a_full_restart(self):
        old_graph = {name: {} for name in dev.project(self.config)['processes'] if name not in ('valkey', 'ingress')}
        dev.save_json(self.state / 'process-compose.json', {'processes': old_graph})
        self.assertEqual(dev.stale_graph(self.config), {'valkey', 'ingress'})
        with patch.object(dev, 'supervisor_running', return_value=True):
            # LiveKit's config now names Valkey, which the old graph never started.
            with self.assertRaisesRegex(RuntimeError, 'predates ingress, valkey.*down, then foundation-dev up'):
                dev.prepare_supervisor()
            with patch.object(dev.sys, 'argv', ['foundation-dev', 'restart', 'livekit']), \
                    patch.object(dev, 'pc') as pc, self.assertRaisesRegex(RuntimeError, 'predates'):
                dev.main()
            pc.assert_not_called()
        dev.save_json(self.state / 'process-compose.json', {'processes': dev.project(self.config)['processes']})
        self.assertEqual(dev.stale_graph(self.config), set())
        dev.save_json(self.state / 'process-compose.json', {'processes': {'postgres': {}}})
        self.assertEqual(dev.stale_graph(self.config), set(), 'a PostgreSQL-only run has its own check')

    def test_a_stack_created_before_the_ingress_gets_a_free_block_and_keeps_it(self):
        old = {k: v for k, v in self.config.items() if k != 'ingress_port'}
        dev.save_json(dev.SETTINGS, old)
        start = old['port'] + 10000
        with socket.socket() as busy:
            # A held port in the first candidate block moves the ingress to the next one.
            busy.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            try:
                busy.bind(('127.0.0.1', start + 2))
            except OSError:
                self.skipTest('the test port block is busy')
            busy.listen()
            self.assertFalse(dev.ingress_ports_free(start))
            upgraded = dev.settings()
        self.assertEqual({k: upgraded[k] for k in old}, old)
        self.assertGreater(upgraded['ingress_port'], start)
        self.assertEqual(upgraded['ingress_port'] % 10, start % 10)
        self.assertEqual(dev.settings(create=True), upgraded, 'the block is stable once written')

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
          # Resource handling has its own tests; isolate this lifecycle fixture
          # from the machine's descriptor limits without changing them.
          ulimit() { case "$*" in -Sn) echo 65536 ;; -Hn) echo unlimited ;; *) return 77 ;; esac; }
          sysctl() { [ "$*" = '-n kern.maxfilesperproc' ] || return 77; echo 65536; }
          make() {
            echo "make:${*: -1}" >> "$CALLS"
            [ "$1" = -C ] && [ "$2" -ef "$FOUNDATION_DEV_ROOT" ] && [ "$3" = catalyst-test ] || exit 78
            [ "$PWD" -ef "$FOUNDATION_DEV_ROOT" ] || exit 79
            [ "$CATALYST_TEST_DATABASE_URL" = postgresql://fixture/postgres ] || exit 80
            [ "$PULSE_NATS_SERVER_BIN" = "$(command -v nats-server)" ] || exit 81
            [ "$ARCHIPELAGO_NATS_SERVER_BIN" = "$PULSE_NATS_SERVER_BIN" ] || exit 82
            return 42
          }
          export -f foundation-dev python3 cargo make ulimit sysctl
          bash "$1" rust
        '''
        calls = self.state / 'calls'
        bash = str(Path(os.environ.get('ProgramFiles', 'C:/Program Files')) / 'Git/bin/bash.exe') if os.name == 'nt' else 'bash'
        result = subprocess.run([bash, '-c', command, 'test', (dev.ROOT / 'tooling/dev-test.sh').as_posix()],
                                env={**os.environ, 'CALLS': str(calls), 'FIXTURE_ROOT': self.state.as_posix(),
                                     'FOUNDATION_DEV_STARTUP_TIMEOUT': '3600'},
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 42, result.stderr)
        listed = calls.read_text().splitlines()
        # The provisioned tests run beside the broad run, so either may log first.
        listed[-3:-1] = sorted(listed[-3:-1], key=lambda call: call.startswith('make:'))
        self.assertEqual(listed,
                         ['up --postgres-only', 'wait --postgres-only --timeout 3600', 'env',
                          'nats:--version',
                          "python:-m unittest discover -s tooling/tests -p test_catalyst_test_resources.py",
                          "python:-m unittest discover -s tooling/tests -p test_dev_regressions.py",
                          "python:-m unittest discover -s tooling/tests -p test_archipelago_fixtures.py",
                          'python:tooling/archipelago-fixtures.py check',
                          "python:-m unittest discover -s tooling/tests -p test_enet_provenance.py",
                          'python:library/vendor/rusty_enet/verify.py', 'python:tooling/dev-regressions.py',
                          'cargo:check --locked --manifest-path library/vendor/rusty_enet/Cargo.toml --all-features --all-targets',
                          'cargo:test --locked --manifest-path library/vendor/rusty_enet/Cargo.toml --lib',
                          'python:tooling/dev-regressions.py --archipelago-external',
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

    def postgres_argv(self, disposable_env, stderr=None):
        """The PostgreSQL argv of a start with initdb and the version check stubbed; output goes to stderr if given."""
        def fake_run(argv, **kwargs):
            if argv[0] == 'initdb':
                Path(argv[2]).mkdir(parents=True)
                (Path(argv[2]) / 'PG_VERSION').write_text('18\n')
            return subprocess.CompletedProcess(argv, 0, 'postgres (PostgreSQL) 18.6\n')
        env = {k: v for k, v in os.environ.items() if k != 'FOUNDATION_DEV_DISPOSABLE_POSTGRES'}
        if disposable_env is not None:
            env['FOUNDATION_DEV_DISPOSABLE_POSTGRES'] = disposable_env
        with patch.dict(os.environ, env, clear=True), patch.object(dev, 'run', side_effect=fake_run), \
                patch.object(dev.os, 'execvpe') as execvpe, \
                contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(stderr or io.StringIO()):
            dev.postgres(self.config)
        program, argv, _ = execvpe.call_args.args
        self.assertEqual(program, 'postgres')
        return argv

    def test_postgres_keeps_full_durability_by_default(self):
        argv = self.postgres_argv(None)
        for setting in ('fsync=off', 'synchronous_commit=off', 'full_page_writes=off'):
            self.assertNotIn(setting, argv)
        self.assertEqual(list((self.state / 'postgres').iterdir()), [self.state / 'postgres/PG_VERSION'])

    def test_disposable_test_cluster_runs_without_durability(self):
        argv = self.postgres_argv('1')
        settings = [argv[i + 1] for i, arg in enumerate(argv) if arg == '-c']
        for setting in ('fsync=off', 'synchronous_commit=off', 'full_page_writes=off',
                        'max_locks_per_transaction=256'):
            self.assertIn(setting, settings)
        self.assertIn('fsync=off', self.postgres_argv('1'), 'a restart keeps the settings')

    def test_existing_cluster_keeps_durability_when_the_disposable_flag_is_set(self):
        self.postgres_argv(None)
        stderr = io.StringIO()
        argv = self.postgres_argv('1', stderr)
        self.assertNotIn('fsync=off', argv, 'a cluster created without the flag may hold real data')
        self.assertIn('FOUNDATION_DEV_DISPOSABLE_POSTGRES ignored', stderr.getvalue())

    def test_disposable_flag_needs_an_exact_value(self):
        for value in ('', '0', 'true', 'yes'):
            with self.subTest(value=value):
                shutil.rmtree(self.state / 'postgres', ignore_errors=True)
                self.assertNotIn('fsync=off', self.postgres_argv(value))

    @unittest.skipUnless(shutil.which('initdb') and shutil.which('postgres') and shutil.which('psql')
                         and os.getuid() != 0, 'requires PostgreSQL')
    def test_disposable_settings_start_a_real_cluster(self):
        data = self.state / 'pg'
        subprocess.run(['initdb', '-D', data, '-U', 'foundation', '--auth=trust'], check=True,
                       capture_output=True)
        log = self.state / 'pg.log'
        # The free port is released before PostgreSQL binds it, so another process can take it in between;
        # one retry on a new port covers that race.
        for attempt in range(2):
            with socket.socket() as sock:
                sock.bind(('127.0.0.1', 0))
                port = sock.getsockname()[1]
            argv = ['postgres', '-D', str(data), '-h', '127.0.0.1', '-p', str(port), '-k', '']
            for setting in dev.DISPOSABLE_POSTGRES_SETTINGS:
                argv += ['-c', setting]
            with log.open('w') as stderr:
                server = subprocess.Popen(argv, stdout=subprocess.DEVNULL, stderr=stderr)
            self.addCleanup(server.wait, 30)
            self.addCleanup(server.send_signal, signal.SIGINT)
            query = ['psql', '-X', '-Atq', '-h', '127.0.0.1', '-p', str(port), '-U', 'foundation', '-d', 'postgres']
            deadline = time.monotonic() + 30
            while server.poll() is None and subprocess.run([*query, '-c', 'SELECT 1'], capture_output=True).returncode != 0:
                self.assertLess(time.monotonic(), deadline, 'PostgreSQL did not start:\n' + log.read_text())
                time.sleep(0.2)
            if server.poll() is None:
                break
            if attempt or 'already in use' not in log.read_text():
                self.fail('PostgreSQL exited at startup:\n' + log.read_text())
        shown = subprocess.run([*query, '-c', 'SELECT name, setting FROM pg_settings WHERE name IN '
                                "('fsync','synchronous_commit','full_page_writes','wal_level') ORDER BY name"],
                               check=True, capture_output=True, text=True).stdout.split()
        self.assertEqual(shown, ['fsync|off', 'full_page_writes|off', 'synchronous_commit|off', 'wal_level|minimal'])
        subprocess.run([*query, '-c', 'CREATE DATABASE scratch'], check=True, capture_output=True)
        subprocess.run([*query, '-c', 'DROP DATABASE scratch WITH (FORCE)'], check=True, capture_output=True)

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
        self.assertNotIn('AUTHORITATIVE_SERVER_ADDRESS', env)
        host = '0x1111111111111111111111111111111111111111'
        hosted = dev.pulse_env({**self.config, 'AUTHORITATIVE_SERVER_ADDRESS': host},
                               self.state / 'cert.pem', self.state / 'key.pem')
        self.assertEqual(hosted['AUTHORITATIVE_SERVER_ADDRESS'], host)
        older = {k: v for k, v in self.config.items() if k != 'MODERATOR_TOKEN'}
        dev.save_json(dev.SETTINGS, older)
        self.assertEqual({k: dev.settings()[k] for k in older}, older, 'a stack from before the token keeps its secrets')
        self.assertTrue(dev.settings()['MODERATOR_TOKEN'])

    def test_scenes_reach_their_third_party_services_through_the_nodes_proxy(self):
        self.assertEqual(dev.runtime_env(self.config)['SCENE_PROXY_HOSTS'], ','.join(dev.SCENE_PROXY_HOSTS))
        own = {**self.config, 'scene_proxy_hosts': ['rooms.example.test', 'files.example.test']}
        self.assertEqual(dev.runtime_env(own)['SCENE_PROXY_HOSTS'], 'rooms.example.test,files.example.test')
        self.assertEqual(dev.runtime_env({**self.config, 'scene_proxy_hosts': []})['SCENE_PROXY_HOSTS'], '')
        with self.assertRaises(SystemExit):
            dev.runtime_env({**self.config, 'scene_proxy_hosts': 'rooms.example.test'})

    def test_a_stale_cast_bot_path_warns_and_stops_no_command(self):
        bot = self.state / 'cast-bot'
        with patch('sys.stderr', new_callable=io.StringIO) as warning:
            env = dev.runtime_env({**self.config, 'cast_bot_bin': str(bot), 'personal_worlds': 2})
        self.assertNotIn('CAST_BOT_BIN', env)
        self.assertEqual(env['WORLDS_PERSONAL_WORLDS_MAX'], '2')
        self.assertIn(f'cast_bot_bin {bot} is not executable', warning.getvalue())
        bot.write_text('#!/bin/sh\n')
        bot.chmod(0o755)
        self.assertEqual(dev.runtime_env({**self.config, 'cast_bot_bin': str(bot)})['CAST_BOT_BIN'], str(bot))

    def test_the_message_router_runs_only_when_its_binary_is_set(self):
        router = self.state / 'catalyst-message-router'
        stale = {**self.config, 'message_router_bin': str(router)}
        for config in (self.config, stale):
            with patch('sys.stderr', new_callable=io.StringIO):
                env, processes = dev.runtime_env(config), dev.project(config)['processes']
            self.assertNotIn('MESSAGE_ROUTER_KEY', env)
            self.assertNotIn('MESSAGE_ROUTER_IDENTITY_PREFIX', env)
            self.assertNotIn('message-router', processes)
        with patch('sys.stderr', new_callable=io.StringIO) as warning:
            dev.runtime_env(stale)
        self.assertIn(f'message_router_bin {router} is not executable', warning.getvalue())
        router.write_text('#!/bin/sh\n')
        router.chmod(0o755)
        env = dev.runtime_env(stale)
        self.assertEqual(env['MESSAGE_ROUTER_KEY'], self.config['MESSAGE_ROUTER_KEY'])
        self.assertEqual(env['MESSAGE_ROUTER_IDENTITY_PREFIX'], 'message-router-prd')
        process = dev.project(stale)['processes']['message-router']
        self.assertEqual(process['depends_on'], {'livekit': {'condition': 'process_healthy'},
                                                 'api': {'condition': 'process_healthy'}})
        self.assertEqual(process['availability']['restart'], 'on_failure')
        self.assertEqual(process['readiness_probe']['http_get']['port'], dev.message_router_metrics_port(stale))
        self.assertEqual(process['readiness_probe']['http_get']['path'], '/health/ready')
        self.assertIn('message-router', dev.SERVICES)
        # Turning the router on under a running stack asks for a full restart.
        dev.save_json(self.state / 'process-compose.json', {'processes': dev.project(self.config)['processes']})
        self.assertEqual(dev.stale_graph(stale), {'message-router'})

    def test_the_router_port_is_checked_only_when_the_router_runs(self):
        base = self.config['ingress_port']
        router = self.state / 'catalyst-message-router'
        router.write_text('#!/bin/sh\n')
        router.chmod(0o755)
        checked = []
        with patch.object(dev, 'bindable', side_effect=lambda ports: checked.append(ports) or True):
            self.assertTrue(dev.ingress_ports_free(base, router=False))
            self.assertTrue(dev.ingress_ports_free(base))
        self.assertNotIn(base + 4, [port for _, port, _ in checked[0]])
        self.assertIn(base + 4, [port for _, port, _ in checked[1]])
        self.assertEqual(dev.message_router_metrics_port(self.config), base + 4)
        for config, router_checked in ((self.config, False), ({**self.config, 'message_router_bin': str(router)}, True)):
            with patch.object(dev, 'settings', return_value=config), patch.object(dev, 'supervisor_running', return_value=False), \
                    patch.object(dev, 'ports_free', return_value=True), patch.object(dev, 'save_json'), \
                    patch.object(dev, 'ingress_ports_free', return_value=True) as free:
                dev.prepare_supervisor()
            free.assert_called_once_with(base, router=router_checked)

    def test_the_message_router_signs_with_its_own_key_and_reaches_loopback(self):
        key = self.config['MESSAGE_ROUTER_KEY']
        self.assertEqual(len(base64.urlsafe_b64decode(key + '=')), 32)
        self.assertNotIn('=', key)
        self.assertNotEqual(key, self.config['LIVEKIT_API_SECRET'])
        base = self.config['port']
        for config in (self.config, {**self.config, 'public_origin': 'https://dev.example.test'}):
            env = dev.message_router_env(config)
            self.assertEqual(env['MESSAGE_ROUTER_KEY'], key)
            self.assertEqual(env['MESSAGE_ROUTER_TOKEN_URL'],
                             f'http://127.0.0.1:{base}/internal/message-router/token/v1')
            self.assertEqual(env['MESSAGE_ROUTER_LIVEKIT_URL'], f'ws://127.0.0.1:{dev.livekit_ports(config)[0]}')
            self.assertEqual(env['DATABASE_URL'], dev.database_url(config))
            self.assertEqual(env['MESSAGE_ROUTER_REPLICA'], '0')
            self.assertNotIn('LIVEKIT_API_SECRET', env)
            self.assertNotIn(self.config['LIVEKIT_API_SECRET'], env.values())
        metrics = dev.message_router_metrics_port(self.config)
        self.assertEqual(env['MESSAGE_ROUTER_METRICS_BIND'], f'127.0.0.1:{metrics}')
        self.assertNotIn(metrics, (*dev.ingress_ports(self.config), *range(base, base + 10)))
        older = {k: v for k, v in self.config.items() if k != 'MESSAGE_ROUTER_KEY'}
        dev.save_json(dev.SETTINGS, older)
        self.assertEqual({k: dev.settings()[k] for k in older}, older, 'a stack from before the key keeps its secrets')
        self.assertTrue(dev.settings()['MESSAGE_ROUTER_KEY'])

    def test_the_message_router_identity_is_the_one_the_explorer_trusts(self):
        # The engine gets a base domain from /play only on a bare dotted host; otherwise it keeps decentraland.org.
        for origin, prefix in ((None, 'message-router-prd'),
                               ('https://dev.example.test:5443', 'message-router-prd'),
                               ('https://dev.example.test', 'message-router-dev'),
                               ('https://dev.example.test:443', 'message-router-dev'),
                               ('http://localhost', 'message-router-prd'),
                               ('https://play.decentraland.zone', 'message-router-dev'),
                               ('https://play.decentraland.org', 'message-router-prd'),
                               # the page's host in punycode is a bare dotted host
                               ('https://dév.example.test', 'message-router-dev')):
            config = {**self.config, 'public_origin': dev.public_origin(origin)}
            self.assertEqual(dev.message_router_identity_prefix(config), prefix, origin)

    def test_profile_images_render_here_once_the_avatar_renderer_is_built(self):
        with patch.dict(os.environ, {}, clear=False), patch.object(dev, 'AVATAR_RENDER_BIN', self.state / 'missing'):
            os.environ.pop('FOUNDATION_DEV_AVATAR_RENDER_BIN', None)
            env = dev.runtime_env({**self.config, 'profile': 'mirror'})
        self.assertNotIn('PROFILE_IMAGES_RENDER_ENABLED', env)
        self.assertEqual(env['PROFILE_IMAGES_ORIGIN_URL'], 'https://profile-images.decentraland.org')
        renderer = self.state / 'avatar_render'
        renderer.write_text('#!/bin/sh\n')
        renderer.chmod(0o755)
        with patch.dict(os.environ, {'FOUNDATION_DEV_AVATAR_RENDER_BIN': str(renderer)}):
            env = dev.runtime_env({**self.config, 'profile': 'mirror'})
        self.assertEqual(env['PROFILE_IMAGES_RENDER_ENABLED'], 'true')
        self.assertEqual(env['PROFILE_IMAGES_RENDERER'], 'bevy')
        self.assertEqual(env['PROFILE_IMAGES_AVATAR_RENDER_BIN'], str(renderer))
        self.assertEqual(env['PROFILE_IMAGES_ORIGIN_URL'], 'https://profile-images.decentraland.org',
                         'the upstream stays the last resort')

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
        self.assertEqual(env['DATABASE_URL'], dev.database_url(self.config))
        self.assertEqual(env['PULSE_V4_AUDIENCE'], dev.pulse_about_env(self.config)['COMMS_V4_PULSE_AUDIENCE'])
        self.assertEqual(dev.runtime_env(self.config)['ARCHIPELAGO_CONTROL_V4_AUDIENCE'], env['PULSE_V4_AUDIENCE'])
        self.assertEqual(dev.runtime_env(self.config)['DATABASE_URL'], env['DATABASE_URL'])
        self.assertEqual(env['PULSE_NATS_URL'], dev.nats_url(self.config), "the stack's own broker, not the shell's")
        pulse = dev.project(self.config)['processes']['pulse']
        self.assertEqual(pulse['readiness_probe']['http_get']['port'], metrics)
        self.assertEqual(pulse['depends_on'], {'api': {'condition': 'process_healthy'},
                                               'nats': {'condition': 'process_healthy'}})
        self.assertIn('pulse', dev.SERVICES)
        with patch.object(dev, 'started', return_value='start'), \
                patch.object(dev, 'pulse_certificate', return_value=(certificate, key)), \
                patch.object(dev.os, 'execve') as execve:
            dev.task('pulse')
        binary, argv, env = execve.call_args.args
        self.assertEqual((Path(binary).name, argv[1:]), ('catalyst', ['pulse']))
        self.assertEqual(env, dev.pulse_env(self.config, certificate, key))

    def test_api_can_accept_traffic_before_pulse_is_ready_for_local_players(self):
        processes = dev.project(self.config)['processes']
        attempts = 0
        observed = []
        api_url = f"http://127.0.0.1:{self.config['port']}"

        def response(url, timeout):
            observed.append(url)
            if url == api_url + '/health':
                details = {'comms': {'status': 'pass' if attempts >= 1 else 'fail', 'required': False},
                           'postgres': {'status': 'fail' if attempts == 1 else 'pass', 'required': False}}
                body = io.BytesIO(json.dumps({'status': 'pass', 'details': details}).encode())
                body.status = 200
                return body
            return contextlib.nullcontext(type('Response', (), {'status': 200})())

        def next_attempt(_delay):
            nonlocal attempts
            mirrors.assert_not_called()
            attempts += 1

        listing = subprocess.CompletedProcess([], 0, stdout='[]')
        with patch.object(dev.urllib.request, 'urlopen', side_effect=response), \
                patch.object(dev, 'pc', return_value=listing), \
                patch.object(dev, 'backoff_delays', return_value=iter([0, 0])), \
                patch.object(dev.time, 'sleep', side_effect=next_attempt), \
                patch.object(dev, 'wait_first_mirror_pass', return_value=0) as mirrors:
            probe = processes['api']['readiness_probe']['http_get']
            # The real /status route answers 200 after schema/state/listener setup,
            # while /health includes the still-absent Pulse heartbeat.
            with dev.urllib.request.urlopen(api_url + probe['path'], timeout=3) as ready:
                self.assertEqual(ready.status, 200)
            self.assertEqual(processes['pulse']['depends_on']['api'], {'condition': 'process_healthy'})
            self.assertEqual(dev.wait_ready(10), 0)
        self.assertEqual(attempts, 2)
        self.assertEqual(observed.count(api_url + '/health'), 3)
        mirrors.assert_called_once()

    def test_local_mirror_can_start_while_its_map_indexes_catch_up(self):
        config = {**self.config, 'profile': 'mirror'}
        listing = subprocess.CompletedProcess([], 0, stdout='[]')
        with patch.object(dev, 'settings', return_value=config), \
                patch.object(dev, 'pc', return_value=listing), \
                patch.object(dev, 'wait_first_mirror_pass', return_value=0), \
                patch.object(dev.urllib.request, 'urlopen') as request:
            request.return_value.__enter__.return_value.status = 200
            self.assertEqual(dev.wait_ready(10), 0)
            self.assertEqual(request.call_args_list[0].args[0], f"http://127.0.0.1:{config['port']}/about")

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

    def test_two_stacks_on_one_machine_do_not_share_the_metrics_exporter(self):
        env = dev.runtime_env(self.config)
        self.assertEqual(env['INDEX_METRICS_BIND'], f"127.0.0.1:{self.config['port'] + 6}")
        self.assertEqual(env['SERVE_METRICS_BIND'], f"127.0.0.1:{self.config['port'] + 1}", "not the binary's default 5006")
        with socket.socket() as busy:
            busy.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            busy.bind(('127.0.0.1', self.config['port'] + 1))
            busy.listen()
            self.assertFalse(dev.ports_free(self.config['port']), 'a held metrics port is a held block')

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
        for delay in dev.backoff_delays(deadline):
            try:
                with dev.urllib.request.urlopen(f'http://127.0.0.1:{monitor}/healthz', timeout=1) as response:
                    self.assertEqual(json.loads(response.read()), {'status': 'ok'})
                break
            except OSError:
                self.assertIsNone(server.poll(), 'nats-server exited')
                self.assertLess(time.monotonic(), deadline, 'nats-server did not answer')
                time.sleep(min(delay, max(0, deadline - time.monotonic())))
        else:
            self.fail('nats-server did not answer')
        with socket.create_connection(('127.0.0.1', client), timeout=5) as sock:
            self.assertTrue(sock.recv(4096).startswith(b'INFO '), 'a NATS client port')

    def test_a_busy_udp_port_in_the_block_moves_the_stack(self):
        base = self.config['port'] + 400
        if not dev.ports_free(base):
            self.skipTest('the test port block is busy')
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
            sock.bind(('127.0.0.1', base + 2))
            self.assertFalse(dev.ports_free(base), 'Pulse needs UDP on the Sites port')

    def test_a_public_origin_serves_enet_where_native_clients_dial(self):
        # A native client dials ENet on UDP 7777 of pulse-server.<base domain>, never a port of the block.
        base, files = self.config['port'], (Path('c'), Path('k'))
        config = {**self.config, 'public_origin': 'https://dev.example.test'}
        env = dev.pulse_env(config, *files)
        self.assertEqual((env['PULSE_BIND'], env['PULSE_METRICS_BIND']), ('0.0.0.0:7777', f'127.0.0.1:{base + 7}'))
        second = {**config, 'pulse_enet_port': 7778}
        self.assertEqual(dev.pulse_env(second, *files)['PULSE_BIND'], '0.0.0.0:7778', 'a second stack on the machine')
        self.assertEqual(dev.pulse_env({**self.config, 'pulse_enet_port': 7778}, *files)['PULSE_BIND'],
                         f'127.0.0.1:{base + 7}', 'a loopback stack stays in its block')
        # Startup checks both of Pulse's public UDP ports.
        with patch.object(dev, 'settings', return_value=second), patch.object(dev, 'supervisor_running', return_value=False), \
                patch.object(dev, 'ingress_ports_free', return_value=True), patch.object(dev, 'save_json'), \
                patch.object(dev, 'ports_free', return_value=True) as free:
            dev.prepare_supervisor()
        free.assert_called_once_with(base, (dev.WEB_PULSE_PORT, 7778))
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as busy:
            busy.bind(('0.0.0.0', 0))
            self.assertFalse(dev.ports_free(base, (busy.getsockname()[1],)), 'a taken public port stops startup')

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

    def test_a_public_origin_is_what_the_realm_advertises(self):
        config = {**self.config, 'public_origin': 'https://dev.example.test:5443'}
        env = dev.runtime_env(config)
        self.assertEqual(env['PUBLIC_URL'], 'https://dev.example.test:5443')
        self.assertEqual(env['COMMS_FIXED_ADAPTER'], 'archipelago:wss://dev.example.test:5443/ws')
        self.assertEqual(env['COMMS_ADAPTER'], 'fixed-adapter:' + env['COMMS_FIXED_ADAPTER'])
        self.assertEqual(env['LIVEKIT_WS_URL'], 'wss://dev.example.test:5443', 'players reach LiveKit through the front')
        self.assertEqual(env['LIVEKIT_HOST'], f'ws://127.0.0.1:{dev.livekit_ports(config)[0]}', 'the API still dials loopback')
        self.assertEqual(env['DEV_ALLOWED_HOSTS'], '.dev.example.test', 'the host and its subdomains')
        self.assertEqual(dev.runtime_env({**self.config, 'public_origin': 'http://10.0.0.5:5443'})['DEV_ALLOWED_HOSTS'],
                         '10.0.0.5')
        self.assertEqual(env['HTTP_TRUSTED_PROXY_CIDRS'], '127.0.0.1/32,::1/128', 'the front is the loopback peer')
        self.assertEqual(env['CAST_PRESENTER_PUBLIC_URL'], 'https://cast-presenter-service.dev.example.test:5443')
        self.assertEqual((env['HTTP_SERVER_HOST'], env['HOST']), ('127.0.0.1', '127.0.0.1'), 'the front is the only listener')
        # The web Explorer dials Pulse at the page's host and port over UDP, which the front cannot carry.
        self.assertEqual(dev.pulse_ports(config)[0], 5443)
        about = dev.pulse_about_env(config)
        self.assertEqual(about['COMMS_V4_PULSE_WEBTRANSPORT_URL'], 'https://dev.example.test:5443')
        self.assertEqual(about['COMMS_V4_PULSE_AUDIENCE'], 'dev.example.test:5443')
        self.assertEqual(dev.pulse_env(config, Path('c'), Path('k'))['PULSE_WT_BIND'], '0.0.0.0:5443')
        portless = {**self.config, 'public_origin': 'http://dev.example.test'}
        self.assertEqual(dev.pulse_ports(portless)[0], dev.WEB_PULSE_PORT, "the engine's default when the page has no port")
        self.assertEqual(dev.runtime_env(portless)['COMMS_FIXED_ADAPTER'], 'archipelago:ws://dev.example.test/ws')
        self.assertNotIn('CAST_PRESENTER_PUBLIC_URL', dev.runtime_env(portless), 'the API refuses a plain http URL')
        loopback = dev.runtime_env(self.config)
        self.assertEqual(loopback['PUBLIC_URL'], f"http://127.0.0.1:{self.config['port'] + 2}")
        self.assertNotIn('LIVEKIT_WS_URL', loopback)
        self.assertNotIn('DEV_ALLOWED_HOSTS', loopback)
        self.assertNotIn('HTTP_TRUSTED_PROXY_CIDRS', loopback)
        self.assertEqual(dev.pulse_env(self.config, Path('c'), Path('k'))['PULSE_WT_BIND'],
                         f"127.0.0.1:{self.config['port'] + 2}")

    def test_the_public_origin_is_an_origin_taken_on_first_startup(self):
        for bad in ('dev.example.test', 'https://dev.example.test/play', 'https://dev.example.test?x=1',
                    'ftp://dev.example.test', 'https://user@dev.example.test'):
            with self.assertRaisesRegex(ValueError, 'origin'):
                dev.public_origin(bad)
        self.assertEqual(dev.public_origin('https://dev.example.test/'), 'https://dev.example.test')
        self.assertIsNone(dev.public_origin('local'))
        self.assertIsNone(dev.public_origin(None))
        self.assertIsNone(self.config.get('public_origin'), 'the default stack stays on loopback')
        state = self.state / 'public'
        state.mkdir()
        with patch.object(dev, 'STATE', state), patch.object(dev, 'SETTINGS', state / 'settings.json'), \
                patch.dict(os.environ, {'FOUNDATION_DEV_PUBLIC_ORIGIN': 'https://dev.example.test:5443',
                                        'FOUNDATION_DEV_PORT': '6300'}):
            self.assertEqual(dev.settings(create=True)['public_origin'], 'https://dev.example.test:5443')
            self.assertEqual(json.loads((state / 'settings.json').read_text())['public_origin'],
                             'https://dev.example.test:5443', 'the installed service has no environment')
        with self.assertRaisesRegex(ValueError, 'go together'):
            dev.origin_settings({'FOUNDATION_DEV_PUBLIC_ORIGIN': 'https://dev.example.test',
                                 'FOUNDATION_DEV_TLS_CERT': '/unused'})
        with self.assertRaisesRegex(ValueError, 'FOUNDATION_DEV_PUBLIC_ORIGIN'):
            dev.origin_settings({'FOUNDATION_DEV_TLS_CERT': '/unused', 'FOUNDATION_DEV_TLS_KEY': '/unused'})

    def test_a_developers_certificate_replaces_the_generated_pulse_pair(self):
        # The settings keep resolved paths, and the macOS temporary directory is behind a link.
        certificate, key = (self.state / 'dev.crt').resolve(), (self.state / 'dev.key').resolve()
        config = {**self.config, 'public_origin': 'https://dev.example.test'}
        with self.assertRaisesRegex(RuntimeError, 'missing'):
            dev.origin_settings({}, 'https://dev.example.test', str(certificate), str(key))
        certificate.write_text('cert'), key.write_text('key')
        config.update(dev.origin_settings({}, 'https://dev.example.test', str(certificate), str(key)))
        dev.save_json(dev.SETTINGS, config)
        self.assertEqual(dev.pulse_certificate(config), (certificate, key))
        self.assertFalse((self.state / 'pulse').exists(), 'nothing generated beside the given pair')
        with patch.object(dev, 'started', return_value='start'), patch.object(dev.os, 'execve') as execve:
            dev.task('pulse')
        self.assertEqual(execve.call_args.args[2]['PULSE_WT_CERT_PATH'], str(certificate))
        self.assertEqual(execve.call_args.args[2]['PULSE_WT_BIND'], f'0.0.0.0:{dev.WEB_PULSE_PORT}')
        # Without one, the generated certificate names the public host so a pinned browser can match it.
        generated, _ = dev.pulse_certificate({**self.config, 'public_origin': 'https://dev.example.test'})
        text = subprocess.run(['openssl', 'x509', '-in', generated, '-noout', '-ext', 'subjectAltName'],
                              capture_output=True, text=True, check=True).stdout
        self.assertIn('DNS:dev.example.test', text)
        self.assertIn('IP Address:127.0.0.1', text)

    def test_the_origin_command_reconfigures_a_stopped_stack(self):
        out = io.StringIO()
        with patch.object(dev, 'supervisor_running', return_value=True), \
                patch.object(dev.sys, 'argv', ['foundation-dev', 'origin', 'https://dev.example.test']):
            with self.assertRaisesRegex(RuntimeError, 'foundation-dev down'):
                dev.main()
        with patch.object(dev, 'supervisor_running', return_value=False), contextlib.redirect_stdout(out), \
                patch.object(dev.sys, 'argv', ['foundation-dev', 'origin', 'https://dev.example.test:5443']):
            dev.main()
        self.assertEqual(dev.settings()['public_origin'], 'https://dev.example.test:5443')
        self.assertIn('https://dev.example.test:5443', out.getvalue())
        with patch.object(dev.sys, 'argv', ['foundation-dev', 'env']), contextlib.redirect_stdout(out):
            dev.main()
        self.assertIn("FOUNDATION_DEV_PUBLIC_URL=https://dev.example.test:5443", out.getvalue())
        with patch.object(dev, 'supervisor_running', return_value=False), contextlib.redirect_stdout(out), \
                patch.object(dev.sys, 'argv', ['foundation-dev', 'origin', 'local']):
            dev.main()
        self.assertIsNone(dev.settings()['public_origin'])
        self.assertEqual(dev.runtime_env(dev.settings())['PUBLIC_URL'], f"http://127.0.0.1:{self.config['port'] + 2}")

    def start_server(self, argv, env):
        server = subprocess.Popen(argv, env={'PATH': os.environ['PATH'], **env},
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

        def stop():
            server.terminate()
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
        self.addCleanup(stop)
        return server

    def wait_http_ok(self, server, port, name, timeout=20):
        deadline = time.monotonic() + timeout
        for delay in dev.backoff_delays(deadline):
            try:
                with socket.create_connection(('127.0.0.1', port), timeout=1) as sock:
                    sock.sendall(b'GET / HTTP/1.0\r\n\r\n')
                    self.assertIn(b'200', sock.recv(4096).split(b'\r\n')[0])
                    return
            except OSError:
                self.assertIsNone(server.poll(), f'{name} exited')
                self.assertLess(time.monotonic(), deadline, f'{name} did not answer')
                time.sleep(min(delay, max(0, deadline - time.monotonic())))
        self.fail(f'{name} did not answer')

    @unittest.skipUnless(shutil.which('livekit-server') and shutil.which('valkey-server') and shutil.which('ingress'),
                         'livekit-server, valkey and livekit-ingress are supplied by nix develop')
    def test_livekit_and_its_ingress_start_on_the_generated_config(self):
        if not dev.ports_free(self.config['port']) or not dev.ingress_ports_free(self.config['ingress_port']):
            self.skipTest('the test port block is busy')
        conf = self.state / 'valkey.conf'
        conf.write_text(dev.valkey_conf(self.config))
        valkey = self.start_server(['valkey-server', str(conf)], {})
        deadline = time.monotonic() + 20
        while not dev.valkey_ready(self.config):
            self.assertIsNone(valkey.poll(), 'valkey-server exited')
            self.assertLess(time.monotonic(), deadline, 'valkey-server did not answer')
            time.sleep(0.1)
        livekit = self.start_server(['livekit-server'], {'LIVEKIT_CONFIG': dev.livekit_yaml(self.config)})
        self.wait_http_ok(livekit, dev.livekit_ports(self.config)[0], 'livekit-server')
        ingress = self.start_server(['ingress'], {'INGRESS_CONFIG_BODY': dev.ingress_yaml(self.config)})
        # The first start on a fresh store scans GStreamer's plugins first.
        self.wait_http_ok(ingress, dev.ingress_ports(self.config)[3], 'ingress', timeout=300)
        # The request /scene-stream-access makes (IngressClient::create_ingress): the server answers
        # with the stack's RTMP listener, which a streaming app then pushes to.
        info = self.livekit_twirp('Ingress', 'CreateIngress', {'ingressAdmin': True}, {
            'inputType': 'RTMP_INPUT', 'name': 'room-ingress', 'roomName': 'room', 'participantIdentity': 'streamer'})
        # The server answers in proto field names (`stream_key`, `ingress_id`), which the API's
        # parse_ingress reads (twirp_answers_in_proto_field_names_are_read in shared/src/livekit.rs).
        self.assertEqual(info['url'], f'rtmp://127.0.0.1:{dev.ingress_ports(self.config)[1]}/x')
        self.assertTrue(info['stream_key'] and info['ingress_id'])
        self.assertNotIn('streamKey', info)

    def livekit_twirp(self, service, method, grant, body):
        def b64(raw):
            return base64.urlsafe_b64encode(raw).rstrip(b'=').decode()
        now = int(time.time())
        claims = {'iss': self.config['LIVEKIT_API_KEY'], 'nbf': now - 5, 'exp': now + 60, 'video': grant}
        unsigned = b64(json.dumps({'alg': 'HS256', 'typ': 'JWT'}).encode()) + '.' + b64(json.dumps(claims).encode())
        signature = hmac.new(self.config['LIVEKIT_API_SECRET'].encode(), unsigned.encode(), hashlib.sha256).digest()
        request = urllib.request.Request(
            f'http://127.0.0.1:{dev.livekit_ports(self.config)[0]}/twirp/livekit.{service}/{method}',
            data=json.dumps(body).encode(), method='POST',
            headers={'Authorization': f'Bearer {unsigned}.{b64(signature)}', 'Content-Type': 'application/json'})
        with urllib.request.urlopen(request, timeout=10) as response:
            return json.load(response)

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

    def test_runtime_takes_only_the_public_sign_in_ids_from_the_shell(self):
        with patch.dict(os.environ, {'THIRDWEB_CLIENT_ID': 'example-client-id', 'MAGIC_API_KEY': 'pk_example123',
                                    'WALLETCONNECT_PROJECT_ID': 'example-project', 'THIRDWEB_SECRET_KEY': 'not-a-real-key'}):
            env = dev.runtime_env(self.config)
        self.assertEqual(env['THIRDWEB_CLIENT_ID'], 'example-client-id')
        self.assertEqual(env['MAGIC_API_KEY'], 'pk_example123')
        self.assertEqual(env['WALLETCONNECT_PROJECT_ID'], 'example-project')
        self.assertNotIn('THIRDWEB_SECRET_KEY', env)
        with patch.dict(os.environ, {}, clear=False):
            for key in dev.PUBLIC_SIGN_IN_IDS:
                os.environ.pop(key, None)
            self.assertFalse(set(dev.PUBLIC_SIGN_IN_IDS) & set(dev.runtime_env(self.config)))

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
                    'CREDITS_USAGE_INDEX_ENABLED'):
            self.assertEqual(env[key], 'false', key)
        # The Shop's catalogue follows the public subgraphs and chain, as `posture show mirror` does; its
        # listings wait for the trade-events index, which walks mainnet over the tagged endpoints.
        for key in ('MARKETPLACE_MIRRORS_ENABLED', 'MARKETPLACE_NFT_INDEX_ENABLED', 'LAND_AUTHZ_INDEX_ENABLED',
                    'SHOP_RENTALS_INDEX_ENABLED', 'MARKETPLACE_COMMITTEE_INDEX_ENABLED',
                    'MARKETPLACE_TRADE_EVENTS_INDEX_ENABLED'):
            self.assertEqual(env[key], 'true', key)
        self.assertEqual(sorted(e.split('=', 1)[0] for e in env['TRADE_RPC_URLS'].split(',')), ['1', '137'])
        self.assertTrue(all(e.endswith('?project=dcl-one-dev') for e in env['TRADE_RPC_URLS'].split(',')))
        # The mirror posture's pause between index passes, and the tag naming dev stacks in the RPC's logs.
        self.assertEqual(env['MARKETPLACE_INDEX_INTERVAL_SECS'], '30')
        for key in ('ETH_RPC_URL', 'RPC_ENDPOINT_POLYGON'):
            self.assertTrue(env[key].endswith('?project=dcl-one-dev'), key)
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
        # The map grid is built from the node's own indexes, not copied from api.decentraland.org.
        self.assertNotIn('EXPLORER_MAP_SOURCE_URL', env)
        self.assertEqual(env['PRICE_POLL_ENABLED'], 'true')
        self.assertNotIn('WORLD_MANIFEST_UPSTREAM_URL', env)
        self.assertNotIn('PULSE_ROOM_AUTHORITY_KEY', env)
        for key in ('STORAGE_ROOT_FOLDER', 'AB_CDN_CACHE_DIR', 'PROFILE_IMAGES_CACHE_DIR', 'BVIMPOSTERS_STORE_ROOT'):
            self.assertTrue(Path(env[key]).is_relative_to(self.state))

    def test_the_testnet_profile_trades_and_relays_on_amoy(self):
        env = dev.runtime_env({**self.config, 'profile': 'testnet'})
        mirror = dev.runtime_env({**self.config, 'profile': 'mirror'})
        self.assertEqual(env['VITE_SHOP_CHAIN_ID'], '80002')
        self.assertEqual(env['COLLECTIONS_CHAIN_ID'], '80002')
        self.assertEqual(env['CONTRACT_ADDRESSES_CHAIN_KEY'], 'amoy')
        # Without a relayer key, the published list (see the relayer key test for the other case).
        self.assertEqual(env['CONTRACT_ADDRESSES_URL'], 'https://contracts.decentraland.org/addresses.json')
        # On-chain cancels and executions close local trades: the trade-events index walks the testnets.
        self.assertEqual(env['MARKETPLACE_TRADE_EVENTS_INDEX_ENABLED'], 'true')
        self.assertEqual((env['MARKETPLACE_INDEX_CHAIN_ID_MATIC'], env['MARKETPLACE_INDEX_CHAIN_ID_ETHEREUM']), ('80002', '11155111'))
        # A trade signed into the stack closes within seconds: the testnet keeps the binary's pause, not the mirror's.
        self.assertEqual((env['MARKETPLACE_INDEX_INTERVAL_SECS'], mirror['MARKETPLACE_INDEX_INTERVAL_SECS']), ('5', '30'))
        self.assertEqual(mirror['MARKETPLACE_TRADE_EVENTS_INDEX_ENABLED'], 'true')
        self.assertNotIn('MARKETPLACE_INDEX_CHAIN_ID_MATIC', mirror)
        # The NFT index knows mainnet's contracts only, so it does not walk the testnets.
        self.assertEqual(env['MARKETPLACE_NFT_INDEX_ENABLED'], 'false')
        self.assertEqual(mirror['MARKETPLACE_NFT_INDEX_ENABLED'], 'true')
        self.assertEqual((env['SHOP_RENTALS_INDEX_ENABLED'], mirror['SHOP_RENTALS_INDEX_ENABLED']), ('false', 'true'))
        self.assertEqual((env['MARKETPLACE_COMMITTEE_INDEX_ENABLED'], mirror['MARKETPLACE_COMMITTEE_INDEX_ENABLED']),
                         ('false', 'true'))
        self.assertIn('decentraland.zone', env['TRADES_SYNC_UPSTREAM_URL'])
        self.assertIn('decentraland.zone', env['TRANSACTIONS_UPSTREAM_URL'])
        self.assertTrue(env['RPC_ENDPOINT_POLYGON'].endswith('/amoy'))
        for key in ('VITE_SHOP_CHAIN_ID', 'COLLECTIONS_CHAIN_ID', 'CONTRACT_ADDRESSES_CHAIN_KEY', 'CONTRACT_ADDRESSES_URL'):
            self.assertNotIn(key, mirror, key)
        # MANA settles on Amoy's venues, which the node accepts only with a testnet ETH_NETWORK, the Matic index on
        # Amoy and Amoy's own endpoint; USD prices convert at the feed and tolerance of Amoy's marketplaces.
        self.assertEqual(env['ETH_NETWORK'], 'sepolia')
        self.assertEqual(env['POLYGON_CHAIN_ID'], env['COLLECTIONS_CHAIN_ID'])
        self.assertNotIn('POLYGON_CHAIN_ID', mirror)
        self.assertIn('80002=https://', env['TRADE_RPC_URLS'])
        self.assertEqual(env['MANA_USD_AGGREGATOR_ADDRESS'], '0xdcf00f5f60b62b07e668a84c0cedaf6f453d416e')
        self.assertEqual(env['USD_PEGGED_ORACLE_MAX_AGE_SECS'], '27')
        for key in ('MANA_USD_AGGREGATOR_ADDRESS', 'USD_PEGGED_ORACLE_MAX_AGE_SECS'):
            self.assertNotIn(key, mirror, key)
        # Otherwise the mirror profile's catalogue passes, against the testnet subgraphs.
        self.assertEqual(env['MARKETPLACE_MIRRORS_ENABLED'], 'true')
        self.assertIn('index', dev.project({**self.config, 'profile': 'testnet'})['processes'])

    def test_the_name_simulation_is_off_unless_asked_and_only_on_the_testnet_profile(self):
        with patch.dict(os.environ, {}, clear=False):
            os.environ.pop('FOUNDATION_DEV_NAME_SIMULATION', None)
            self.assertNotIn('VITE_SHOP_NAME_SIMULATION', dev.runtime_env({**self.config, 'profile': 'testnet'}))
        for asked in ('0', 'true', ''):
            with patch.dict(os.environ, {'FOUNDATION_DEV_NAME_SIMULATION': asked}):
                self.assertNotIn('VITE_SHOP_NAME_SIMULATION', dev.runtime_env({**self.config, 'profile': 'testnet'}), asked)
        with patch.dict(os.environ, {'FOUNDATION_DEV_NAME_SIMULATION': '1', 'VITE_SHOP_NAME_SIMULATION': 'true'}):
            env = dev.runtime_env({**self.config, 'profile': 'testnet'})
            self.assertEqual((env['VITE_SHOP_NAME_SIMULATION'], env['VITE_SHOP_CHAIN_ID']), ('true', '80002'))
            # Never on the mirror profile, which settles on Polygon, even when the shell's environment has it.
            self.assertNotIn('VITE_SHOP_NAME_SIMULATION', dev.runtime_env({**self.config, 'profile': 'mirror'}))

    def test_the_testnet_profile_settles_credits_on_amoy(self):
        env = dev.runtime_env({**self.config, 'profile': 'testnet'})
        mirror = dev.runtime_env({**self.config, 'profile': 'mirror'})
        # dcl-one's own manager: the settlement gate refuses the shared upstream one (0x8052…2fb3).
        self.assertEqual(env['CREDITS_MANAGER_CONTRACT'], '0xb65fd692d15c30c3adcacdd5ccf80e1facecfc7c')
        self.assertNotEqual(env['CREDITS_MANAGER_CONTRACT'], '0x8052a560e6e6ac86eeb7e711a4497f639b322fb3')
        # The Sites build trusts the same manager.
        self.assertEqual(env['VITE_SHOP_CREDITS_MANAGER'], env['CREDITS_MANAGER_CONTRACT'])
        self.assertEqual((env['CREDITS_USAGE_INDEX_ENABLED'], env['CREDITS_MOCK_CARD']), ('true', 'true'))
        # Its usage index replays from the manager's deployment block, not the shared manager's.
        self.assertEqual(env['CREDITS_USAGE_START_BLOCK'], '52152027')
        for key in ('CREDITS_MANAGER_CONTRACT', 'VITE_SHOP_CREDITS_MANAGER', 'CREDITS_MOCK_CARD', 'CREDITS_SIGNER_KEY_FILE', 'CREDITS_USAGE_START_BLOCK'):
            self.assertNotIn(key, mirror, key)
        # Without a signer key in the stack's state, nothing signs credits on chain.
        self.assertEqual(env['CREDITS_ONCHAIN_AUTHORIZE_ENABLED'], 'false')
        self.assertNotIn('CREDITS_SIGNER_KEY_FILE', env)
        key = self.state / 'credits-signer.key'
        key.write_text('  \n')  # empty: the server would read no key, so authorization stays off
        self.assertEqual(dev.runtime_env({**self.config, 'profile': 'testnet'})['CREDITS_ONCHAIN_AUTHORIZE_ENABLED'], 'false')
        key.write_text('not a real key')
        key.chmod(0o644)
        warning = io.StringIO()
        with contextlib.redirect_stderr(warning):
            dev.runtime_env({**self.config, 'profile': 'testnet'})
        self.assertIn('chmod 600', warning.getvalue())
        key.chmod(0o600)
        env = dev.runtime_env({**self.config, 'profile': 'testnet'})
        self.assertEqual(env['CREDITS_ONCHAIN_AUTHORIZE_ENABLED'], 'true')
        self.assertEqual(env['CREDITS_SIGNER_KEY_FILE'], str(key))
        # The file's path is passed, never its contents.
        self.assertNotIn('not a real key', '\n'.join(env.values()))

    def test_the_testnet_relay_signs_its_own_transactions_only_with_a_relayer_key(self):
        relayer = ('META_TX_BROADCAST_ENABLED', 'RELAYER_PRIVATE_KEY_FILE', 'RELAYER_MAX_FEE_PER_GAS_WEI',
                   'MAX_RELAYED_TRANSACTIONS_PER_DAY', 'NAMES_SIGNER_ENABLED')
        published = 'https://contracts.decentraland.org/addresses.json'
        # Without a key the relay forwards to the .zone transactions API over the published list, which lacks
        # dcl-one's manager, so the relay refuses that manager itself (marked not sent) instead of forwarding it.
        env = dev.runtime_env({**self.config, 'profile': 'testnet'})
        for name in relayer:
            self.assertNotIn(name, env, name)
        self.assertIn('decentraland.zone', env['TRANSACTIONS_UPSTREAM_URL'])
        self.assertEqual(env['CONTRACT_ADDRESSES_URL'], published)
        key = self.state / 'relayer.key'
        key.write_text('  \n')  # empty: the server would read no key, so the signer stays off
        env = dev.runtime_env({**self.config, 'profile': 'testnet'})
        for name in relayer:
            self.assertNotIn(name, env, name)
        self.assertEqual(env['CONTRACT_ADDRESSES_URL'], published)
        key.write_text('not a real key')
        key.chmod(0o644)
        warning = io.StringIO()
        with contextlib.redirect_stderr(warning):
            dev.runtime_env({**self.config, 'profile': 'testnet'})
        self.assertIn('chmod 600', warning.getvalue())
        key.chmod(0o600)
        warning = io.StringIO()
        with contextlib.redirect_stderr(warning):
            env = dev.runtime_env({**self.config, 'profile': 'testnet'})
        self.assertEqual(warning.getvalue(), '', 'a private key on a loopback stack warns of nothing')
        self.assertEqual(env['META_TX_BROADCAST_ENABLED'], 'true')
        self.assertEqual(env['RELAYER_PRIVATE_KEY_FILE'], str(key))
        # Capped: 200 gwei at most per gas, and 100 free calls (approvals, cancels) a day across every address.
        self.assertEqual(env['RELAYER_MAX_FEE_PER_GAS_WEI'], '200000000000')
        self.assertEqual(env['MAX_RELAYED_TRANSACTIONS_PER_DAY'], '100')
        # The bundled `amoy` entry, which carries dcl-one's manager, so the relay admits it.
        self.assertNotIn('CONTRACT_ADDRESSES_URL', env)
        # The key signs on Amoy alone: never on the NAMEs chain, whose ETH_RPC_URL is Sepolia's here.
        self.assertEqual(env['NAMES_SIGNER_ENABLED'], 'false')
        self.assertTrue(env['ETH_RPC_URL'].endswith('/sepolia'))
        # The file's path is passed, never its contents.
        self.assertNotIn('not a real key', '\n'.join(env.values()))
        # The mirror profile, which relays on Polygon mainnet, never reads it.
        mirror = dev.runtime_env({**self.config, 'profile': 'mirror'})
        for name in relayer:
            self.assertNotIn(name, mirror, name)

    def test_a_testnet_stack_behind_a_public_origin_never_starts_with_a_relayer_key(self):
        exposed = {**self.config, 'profile': 'testnet', 'public_origin': 'https://dev.example.test'}
        published = 'https://contracts.decentraland.org/addresses.json'
        # Without a key, or with an empty one, a public testnet stack starts: the .zone transactions API pays.
        key = self.state / 'relayer.key'
        self.assertEqual(dev.runtime_env(exposed)['CONTRACT_ADDRESSES_URL'], published)
        key.write_text('  \n')
        self.assertEqual(dev.runtime_env(exposed)['CONTRACT_ADDRESSES_URL'], published)
        # With one, anyone who reaches the origin could spend its POL: the stack's environment is refused,
        # naming both ways out, and `up` refuses before it starts any process.
        key.write_text('not a real key')
        key.chmod(0o600)
        with self.assertRaises(SystemExit) as refused:
            dev.runtime_env(exposed)
        why = str(refused.exception)
        for part in ('https://dev.example.test', 'foundation-dev origin local', 'remove the key', '.zone'):
            self.assertIn(part, why)
        self.assertNotIn('not a real key', why)
        with patch.object(dev, 'settings', return_value=exposed), \
                patch.object(dev, 'supervisor_running', return_value=False), \
                patch.object(dev, 'save_json') as written:
            with self.assertRaises(SystemExit):
                dev.prepare_supervisor()
        written.assert_not_called()
        # With an installed service, `up` refuses before it starts the service, whose supervisor would only
        # restart in a loop on the same refusal.
        (self.state / 'settings.json').write_text(json.dumps(exposed))
        with patch.object(dev, 'service_file', return_value=key), \
                patch.object(dev, 'service_start') as started, \
                patch.object(dev, 'supervisor_running', return_value=False):
            with self.assertRaises(SystemExit):
                dev.up()
        started.assert_not_called()
        # `service install` refuses before it touches the installed unit or stops the running stack.
        with patch.object(dev, 'DARWIN', False):
            unit = dev.service_file()
        unit.parent.mkdir(parents=True, exist_ok=True)
        unit.write_text('the unit installed before')
        with patch.object(dev, 'DARWIN', False), \
                patch.object(dev.shutil, 'which', return_value='/nix/bin/nix'), \
                patch.object(dev, 'systemctl') as systemctl, patch.object(dev, 'down') as down, \
                patch.object(dev, 'service_start') as started:
            with self.assertRaises(SystemExit):
                dev.service_install()
        self.assertEqual(unit.read_text(), 'the unit installed before')
        for untouched in (systemctl, down, started):
            untouched.assert_not_called()
        # The key file is read only for a testnet stack behind a public origin: an unreadable one never stops
        # another profile, nor a testnet stack on loopback.
        with patch.object(dev, 'relayer_key', side_effect=AssertionError('read the relayer key')):
            for config in ({**exposed, 'profile': 'mirror'}, {**exposed, 'profile': 'offline'},
                           {**exposed, 'public_origin': None}):
                dev.refuse_an_exposed_relayer(config)
            with patch.object(dev, 'settings', return_value={**exposed, 'profile': 'mirror'}), \
                    patch.object(dev, 'supervisor_running', return_value=False), \
                    patch.object(dev, 'ports_free', return_value=True), \
                    patch.object(dev, 'ingress_ports_free', return_value=True), \
                    patch.object(dev, 'save_json'):
                self.assertIsNotNone(dev.prepare_supervisor())
        # What only reads the environment, or runs PostgreSQL alone, signs nothing and still works.
        self.assertEqual(dev.psql_env(exposed)['PGDATABASE'], 'catalyst')
        with patch.object(dev, 'run', return_value=subprocess.CompletedProcess([], 0, stdout='1\n')):
            self.assertEqual(dev.sql(exposed, 'SELECT 1'), '1')
        printed = io.StringIO()
        with patch.object(dev.sys, 'argv', ['foundation-dev', 'env']), contextlib.redirect_stdout(printed):
            dev.main()
        self.assertIn('FOUNDATION_DEV_PUBLIC_URL=', printed.getvalue())
        with patch.object(dev, 'settings', return_value=exposed), \
                patch.object(dev, 'supervisor_running', return_value=False), \
                patch.object(dev, 'ports_free', return_value=True), \
                patch.object(dev, 'ingress_ports_free', return_value=True), \
                patch.object(dev, 'save_json'):
            self.assertIsNotNone(dev.prepare_supervisor(postgres_only=True))
        # The same key on loopback starts, capped; another profile never reads it.
        self.assertEqual(dev.runtime_env({**exposed, 'public_origin': None})['MAX_RELAYED_TRANSACTIONS_PER_DAY'], '100')
        self.assertNotIn('RELAYER_PRIVATE_KEY_FILE', dev.runtime_env({**exposed, 'profile': 'mirror'}))

    def test_testnet_data_never_shares_a_database_with_another_profile(self):
        (self.state / 'postgres').mkdir()
        (self.state / 'postgres' / 'PG_VERSION').write_text('18')
        def run(*argv):
            with patch.object(dev.sys, 'argv', ['foundation-dev', *argv]), \
                    patch.object(dev, 'supervisor_running', return_value=False):
                return dev.main()
        with self.assertRaisesRegex(RuntimeError, 'offline profile'):
            run('profile', 'testnet')
        self.assertEqual(dev.settings()['profile'], 'offline')
        run('profile', 'mirror')  # neither is testnet: allowed over existing data
        self.assertEqual(dev.settings()['profile'], 'mirror')
        dev.save_json(dev.SETTINGS, {**dev.settings(), 'profile': 'testnet'})
        with self.assertRaisesRegex(RuntimeError, 'testnet profile'):
            run('profile', 'mirror')
        with self.assertRaisesRegex(RuntimeError, 'not a snapshot'), patch.object(dev, 'export_trades_snapshot') as export:
            run('trades-snapshot', str(self.state / 'out.sql.gz'))
        export.assert_not_called()

    def test_a_different_profile_in_the_environment_keeps_the_saved_one_and_says_so(self):
        warning = io.StringIO()
        with patch.dict(os.environ, {'FOUNDATION_DEV_PROFILE': 'testnet'}), contextlib.redirect_stderr(warning):
            self.assertEqual(dev.settings(create=True)['profile'], 'offline')
        self.assertIn('Using saved profile offline', warning.getvalue())

    def test_env_names_the_running_stack_for_shell_evaluation(self):
        out = io.StringIO()
        with patch.object(dev.sys, 'argv', ['foundation-dev', 'env']), contextlib.redirect_stdout(out):
            self.assertEqual(dev.main(), 0)
        values = dict(line.split('=', 1) for line in out.getvalue().splitlines())
        self.assertEqual(values['FOUNDATION_DEV_API_URL'], f"http://127.0.0.1:{self.config['port']}")
        self.assertEqual(values['FOUNDATION_DEV_DATABASE_URL'], dev.database_url(self.config))
        self.assertEqual(values['FOUNDATION_DEV_STACK_PROFILE'], 'offline')
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
            import importlib.util, pathlib, sys
            spec = importlib.util.spec_from_file_location('dev', sys.argv[1])
            d = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(d)
            d.STATE = pathlib.Path(sys.argv[2])
            d.SETTINGS = d.STATE / 'settings.json'
            database, stamp = d.STATE / 'database', d.STATE / 'stamp'
            def sql(config, query, db='catalyst'):
                if 'pg_database' in query:
                    exists = database.exists()
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
                stamp.touch()
            real_flock = d.fcntl.flock
            def flock(lock, operation):
                try:
                    real_flock(lock, operation | d.fcntl.LOCK_NB)
                except BlockingIOError:
                    print('blocked', flush=True)
                    real_flock(lock, operation)
                print('locked', flush=True)
                assert sys.stdin.readline().strip() == 'continue'
            d.fcntl.flock = flock
            d.sql, d.run = sql, run
            d.apply_schema(d.settings(), 'catalyst')
        '''))
        children = []
        def start():
            child = subprocess.Popen([os.sys.executable, str(script), str(dev.__file__), str(self.state)],
                                     stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            children.append(child)
            self.addCleanup(lambda: child.poll() is None and child.kill())
            return child
        def expect(child, state):
            self.assertTrue(select.select([child.stdout], [], [], 10)[0], f'child did not reach {state}')
            self.assertEqual(child.stdout.readline().strip(), state)
        def release(child):
            child.stdin.write('continue\n')
            child.stdin.flush()
        first = start()
        expect(first, 'locked')
        second = start()
        expect(second, 'blocked')
        release(first)
        expect(second, 'locked')
        release(second)
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

    def test_pulse_comes_back_with_a_fresh_generation_after_losing_its_lease(self):
        processes = dev.project(self.config)['processes']
        policy = processes['pulse']['availability']
        self.assertEqual(policy['restart'], 'on_failure')
        # Unlimited: the supervisor never resets the count, and a lease can be lost many times over a stack's life.
        self.assertEqual(policy['max_restarts'], 0)
        self.assertGreaterEqual(policy['backoff_seconds'], 1)
        # The other tasks keep failing the wait on their first non-zero exit.
        self.assertEqual([name for name, process in processes.items() if 'availability' in process], ['pulse'])

    def test_wait_tolerates_pulse_between_restarts_and_fails_when_they_run_out(self):
        api_url = f"http://127.0.0.1:{self.config['port']}"
        restarting = json.dumps([{'name': 'pulse', 'status': 'Restarting', 'exit_code': 1, 'is_running': False}])
        running = json.dumps([{'name': 'pulse', 'status': 'Running', 'exit_code': 1, 'is_running': True}])
        health_checks = []

        def response(url, timeout):
            # The last heartbeat can still be fresh while Pulse waits for its restart.
            if url == api_url + '/health':
                health_checks.append(url)
                body = io.BytesIO(b'{"status":"pass","details":{"comms":{"status":"pass"},"postgres":{"status":"pass"}}}')
                body.status = 200
                return body
            return contextlib.nullcontext(type('Response', (), {'status': 200})())

        listings = iter([subprocess.CompletedProcess([], 0, restarting), subprocess.CompletedProcess([], 0, running)])
        with patch.object(dev, 'pc', side_effect=lambda *a, **k: next(listings)), \
                patch.object(dev.urllib.request, 'urlopen', side_effect=response), \
                patch.object(dev, 'backoff_delays', return_value=iter([0])), \
                patch.object(dev.time, 'sleep'), \
                patch.object(dev, 'wait_first_mirror_pass', return_value=0), \
                contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(dev.wait_ready(10), 0)
        # Ready only on the listing after the restart.
        self.assertEqual(len(health_checks), 1)
        self.assertIsNone(next(listings, None))
        exhausted = subprocess.CompletedProcess([], 0, json.dumps([
            {'name': 'pulse', 'status': 'Completed', 'exit_code': 1, 'is_running': False, 'restarts': 10}]))
        with patch.object(dev, 'pc', return_value=exhausted), patch.object(dev.time, 'sleep') as sleep, \
                contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(RuntimeError, 'pulse failed'):
                dev.wait_ready(10)
        sleep.assert_not_called()

    def test_a_wait_that_times_out_on_a_restarting_process_names_it(self):
        restarting = subprocess.CompletedProcess([], 0, json.dumps(
            [{'name': 'pulse', 'status': 'Restarting', 'exit_code': 1, 'is_running': False}]))
        with patch.object(dev, 'pc', return_value=restarting), \
                patch.object(dev, 'backoff_delays', return_value=iter([0, 0])), \
                patch.object(dev.time, 'sleep'), contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(RuntimeError, 'while pulse kept restarting. Inspect foundation-dev logs pulse'):
                dev.wait_ready(10)

    def test_wait_narrates_progress_with_the_building_task_output(self):
        listing = subprocess.CompletedProcess([], 0, json.dumps([
            {'name': 'api-build', 'status': 'Running', 'is_running': True, 'exit_code': 0},
            {'name': 'api', 'status': 'Pending', 'is_running': False, 'exit_code': 0},
        ]))
        log = self.state / 'api-build.log'
        log.write_text('previous run\n')
        def polling(*args, **kwargs):
            with log.open('a') as output:
                output.write('   Compiling catalyst-worlds v0.1.0\n')
            return listing
        with patch.object(dev, 'pc', side_effect=polling), patch.object(dev, 'PROGRESS_SECONDS', 0), \
                patch.object(dev.urllib.request, 'urlopen', side_effect=dev.urllib.error.URLError('starting')), \
                patch.object(dev.time, 'sleep', side_effect=InterruptedError), \
                contextlib.redirect_stdout(io.StringIO()) as out:
            with self.assertRaises(InterruptedError):
                dev.wait_ready(10)
        self.assertIn('Waiting up to 10s for the API and Sites.', out.getvalue())
        self.assertIn('running: api-build; waiting to start: api; api-build: Compiling catalyst-worlds v0.1.0',
                      out.getvalue())
        self.assertNotIn('previous run', out.getvalue())

    def test_last_line_uses_new_output_and_strips_terminal_sequences(self):
        log = self.state / 'api-build.log'
        log.write_text('a previous run with a long stale line\n')
        position = dev.log_position(log)
        self.assertEqual(dev.last_line(log, after=position), '')
        with log.open('a') as output:
            output.write('\x1b[31mCompiling\x1b[0m\r\x1b]0;build title\x07Finished\x1b[K\n')
        self.assertEqual(dev.last_line(log, after=position), 'Finished')
        log.write_text('truncated\n')
        self.assertEqual(dev.last_line(log, after=position), 'truncated')
        log.unlink()
        log.write_text('rotated\n')
        self.assertEqual(dev.last_line(log, after=position), 'rotated')

    def test_wait_does_not_report_the_previous_builds_last_line(self):
        (self.state / 'api-build.log').write_text('old build failed\n')
        listing = subprocess.CompletedProcess([], 0, stdout=json.dumps([
            {'name': 'api-build', 'is_running': True}]))
        with patch.object(dev, 'pc', return_value=listing), patch.object(dev, 'PROGRESS_SECONDS', 0), \
                patch.object(dev.urllib.request, 'urlopen', side_effect=dev.urllib.error.URLError('starting')), \
                patch.object(dev.time, 'sleep', side_effect=InterruptedError), \
                contextlib.redirect_stdout(io.StringIO()) as out:
            with self.assertRaises(InterruptedError):
                dev.wait_ready(10)
        self.assertIn('running: api-build', out.getvalue())
        self.assertNotIn('old build failed', out.getvalue())

    def test_process_compose_commands_carry_a_configuration_directory(self):
        with patch.object(dev.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0)) as run:
            dev.pc('process', 'list')
        env = run.call_args.kwargs['env']
        self.assertTrue(env['XDG_CONFIG_DIRS'].endswith(f":{self.state / 'xdg'}"))
        self.assertTrue((self.state / 'xdg/process-compose').is_dir())

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
                for delay in dev.backoff_delays(deadline):
                    result = dev.pc('process', 'list', '--output', 'json', capture_output=True, text=True)
                    if result.returncode == 0:
                        break
                    time.sleep(min(delay, max(0, deadline - time.monotonic())))
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertTrue((state / 'pc.sock').is_socket())
                self.assertEqual(json.loads(result.stdout)[0]['name'], 'hold')
            finally:
                dev.down()
            self.assertFalse((state / 'pc.sock').exists())

    @unittest.skipUnless(shutil.which('process-compose'), 'process-compose is supplied by nix develop')
    def test_the_supervisor_lists_pulse_between_restarts_as_wait_expects(self):
        policy = {**dev.project(self.config)['processes']['pulse']['availability'], 'max_restarts': 1}
        project = self.state / 'test.json'
        project.write_text(json.dumps({'version': '0.5', 'processes': {
            'hold': {'command': 'sleep 60'},
            'pulse': {'command': 'exit 1', 'availability': policy}}}))
        seen = []

        def pulse():
            result = dev.pc('process', 'list', '--output', 'json', capture_output=True, text=True)
            if result.returncode:
                return None
            state = next((p for p in json.loads(result.stdout) if p['name'] == 'pulse'), None)
            if state:
                seen.append((state['status'], state['is_running'], state['exit_code'], state['restarts']))
            return state

        def until(predicate, seconds):
            deadline = time.monotonic() + seconds
            for delay in dev.backoff_delays(deadline, first=.05, maximum=.2):
                if (state := pulse()) and predicate(state):
                    return state
                time.sleep(delay)
            self.fail(f'pulse listings: {seen}')

        try:
            dev.pc('up', '--config', str(project), '--disable-dotenv', '--detached', '--tui=false',
                   check=True, capture_output=True, text=True)
            backoff = until(lambda s: s['status'] == 'Restarting', 15)
            self.assertEqual((backoff['is_running'], backoff['exit_code']), (False, 1))
            final = until(lambda s: s['restarts'] == 1 and s['status'] != 'Restarting' and not s['is_running'], 15)
            self.assertEqual(final['exit_code'], 1)
            with patch.object(dev.time, 'sleep'), contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaisesRegex(RuntimeError, 'pulse failed'):
                    dev.wait_ready(10)
        finally:
            dev.down()

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
            for delay in dev.backoff_delays(deadline):
                if process.poll() is not None:
                    self.fail((self.state / 'postgres.log').read_text())
                try:
                    dev.sql(self.config, 'SELECT 1', 'postgres')
                    return process
                except subprocess.CalledProcessError:
                    time.sleep(min(delay, max(0, deadline - time.monotonic())))
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
