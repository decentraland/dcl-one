import contextlib
import importlib.util
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import urllib.error

ROOT = Path(__file__).resolve().parents[2]


def load(relative):
    spec = importlib.util.spec_from_file_location('readiness_' + Path(relative).stem.replace('-', '_'), ROOT / relative)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


dev = load('tooling/dev.py')
image = load('tooling/image-smoke.py')
gateway = load('infra/gateway/smoke.py')


class FinalReadinessSampleTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.state = Path(temporary.name)
        self.clock = 0
        self.sleeps = []
        state = patch.object(dev, 'STATE', self.state)
        state.start()
        self.addCleanup(state.stop)
        monotonic = patch.object(dev.time, 'monotonic', side_effect=lambda: self.clock)
        monotonic.start()
        self.addCleanup(monotonic.stop)
        sleeping = patch.object(dev.time, 'sleep', side_effect=self.sleep)
        sleeping.start()
        self.addCleanup(sleeping.stop)

    def sleep(self, delay):
        self.sleeps.append(delay)
        self.clock += delay

    def test_supervisor_that_starts_during_the_final_sleep_is_ready(self):
        checks = []
        def running():
            checks.append(self.clock)
            return self.clock >= 10
        with patch.object(dev, 'service_file', return_value=self.state / 'missing'), \
                patch.object(dev, 'prepare_supervisor', return_value=([], {})), \
                patch.object(dev, 'pc'), patch.object(dev, 'status', return_value=0), \
                patch.object(dev, 'supervisor_running', side_effect=running):
            self.assertEqual(dev.up(), 0)
        self.assertEqual(self.clock, 10)
        self.assertEqual(checks[-1], 10)

    def test_http_services_that_answer_during_the_final_sleep_are_ready(self):
        checks = []
        def response(*args, **kwargs):
            checks.append(self.clock)
            if self.clock < .3:
                raise urllib.error.URLError('starting')
            return contextlib.nullcontext(SimpleNamespace(status=200))
        with patch.object(dev, 'settings', return_value={'port': 5210}), \
                patch.object(dev, 'pc', return_value=SimpleNamespace(returncode=0, stdout='[]')), \
                patch.object(dev.urllib.request, 'urlopen', side_effect=response), \
                patch.object(dev, 'wait_first_mirror_pass', return_value=0):
            self.assertEqual(dev.wait_ready(.3), 0)
        self.assertEqual(self.clock, .3)
        self.assertEqual(checks[-2:], [.3, .3])

    def test_postgres_ready_during_the_final_sleep_installs_collation(self):
        def sql(*args):
            if self.clock < .3:
                raise subprocess.CalledProcessError(1, 'psql')
        with patch.object(dev, 'settings', return_value={'port': 5210}), \
                patch.object(dev, 'pc', return_value=SimpleNamespace(returncode=0, stdout='[]')), \
                patch.object(dev, 'sql', side_effect=sql), patch.object(dev, 'ensure_collation') as collation:
            self.assertEqual(dev.wait_ready(.3, postgres_only=True), 0)
        self.assertEqual(self.clock, .3)
        collation.assert_called_once_with({'port': 5210}, 'template1')

    def test_shutdown_checks_both_resources_after_its_final_sleep(self):
        for resource in ('socket', 'ports'):
            with self.subTest(resource=resource):
                self.clock = 0
                settings = self.state / 'settings.json'
                settings.touch()
                socket = SimpleNamespace(exists=lambda: resource == 'socket' and self.clock < 120)
                with patch.object(dev, 'SETTINGS', settings), \
                        patch.object(dev, 'service_file', return_value=self.state / 'missing'), \
                        patch.object(dev, 'settings', return_value={'port': 5210}), \
                        patch.object(dev, 'socket_path', return_value=socket), \
                        patch.object(dev, 'supervisor_running', return_value=True), patch.object(dev, 'pc'), \
                        patch.object(dev, 'ports_free', side_effect=lambda port: resource != 'ports' or self.clock >= 120):
                    self.assertEqual(dev.down(), 0)
                self.assertEqual(self.clock, 120)

    def test_gateway_responding_at_the_deadline_is_ready(self):
        checks = []
        def response(*args):
            checks.append(self.clock)
            return (200 if self.clock >= 20 else 503), b''
        with patch.object(gateway, 'request', side_effect=response):
            gateway.wait_ready('fixture')
        self.assertEqual(self.clock, 20)
        self.assertEqual(checks[-1], 20)

    def test_image_probe_samples_the_deadline_before_success_or_timeout(self):
        for ready in (True, False):
            with self.subTest(ready=ready):
                self.clock = 0
                checks = []
                def check():
                    checks.append(self.clock)
                    return ready and self.clock >= .3
                if ready:
                    image.wait_until(check, 'fixture', timeout=.3)
                else:
                    with self.assertRaisesRegex(RuntimeError, 'Timed out: fixture'):
                        image.wait_until(check, 'fixture', timeout=.3)
                self.assertEqual(self.clock, .3)
                self.assertEqual(checks[-1], .3)

    def test_hard_image_probe_errors_are_not_retried(self):
        failure = ValueError('invalid configuration')
        def check():
            raise failure
        with self.assertRaises(ValueError) as raised:
            image.wait_until(check, 'fixture')
        self.assertIs(raised.exception, failure)
        self.assertEqual(self.sleeps, [])
