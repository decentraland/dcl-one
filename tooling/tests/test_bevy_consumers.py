"""Artifact consumers must wait on jobs, preserve provenance and reject partial releases."""
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


ci = module('consumers', 'tooling/bevy-ci.py')
packages = module('headless_packages', 'apps/bevy-explorer/headless-packages.py')


def job(name, conclusion='success'):
    return {'name': name, 'status': 'completed', 'conclusion': conclusion}


class ConsumerTests(unittest.TestCase):
    def test_both_image_architectures_are_required(self):
        jobs = [job(name) for name in ci.IMAGES_JOBS]
        self.assertTrue(ci.consumer_ready(jobs, 'images'))
        for bad in [jobs[:1], jobs + jobs[:1], [jobs[0], job(jobs[1]['name'], 'skipped')],
                    [jobs[0], job(jobs[1]['name'], 'failure')]]:
            with self.subTest(jobs=bad), self.assertRaises(ValueError):
                ci.consumer_ready(bad, 'images')

    def test_unselected_consumers_do_not_publish(self):
        self.assertFalse(ci.consumer_ready([], 'images'))
        self.assertFalse(ci.consumer_ready([job(name, 'skipped') for name in ci.IMAGES_JOBS], 'images'))
        self.assertFalse(ci.consumer_ready([job('Headless packages', 'skipped')], 'headless'))

    def test_headless_requires_three_platforms_and_the_tested_linux_producer(self):
        jobs = [job(name) for name in ci.HEADLESS_JOBS]
        with self.assertRaises(ValueError):
            ci.consumer_ready(jobs, 'headless')
        jobs.append(job(ci.PRODUCERS['headless']))
        self.assertTrue(ci.consumer_ready(jobs, 'headless'))
        jobs[-1]['conclusion'] = 'failure'
        with self.assertRaises(ValueError):
            ci.consumer_ready(jobs, 'headless')

    def test_callback_requires_all_expected_nonexpired_artifacts(self):
        run = {'id': 42, 'head_sha': 'a' * 40, 'run_attempt': 1}
        artifacts = [{'name': f'apis-sites-{kind}-{arch}', 'expired': False}
                     for kind in ('images', 'producer') for arch in ('amd64', 'arm64')]
        jobs = [job(name) for name in ci.IMAGES_JOBS]
        with patch.object(ci, 'run_jobs', return_value=jobs):
            for bad in (artifacts[:3], artifacts + artifacts[:1],
                        [*artifacts[:3], {**artifacts[3], 'expired': True}]):
                with self.subTest(artifacts=bad), patch.object(ci, 'api', return_value={
                        'total_count': len(bad), 'artifacts': bad}), self.assertRaises(ValueError):
                    ci.consumer_source(run, 'example/repo', 'images')
            with patch.object(ci, 'api', return_value={'total_count': 4, 'artifacts': artifacts}):
                self.assertTrue(ci.consumer_source(run, 'example/repo', 'images')['build'])


class PlatformPackageTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        self.launcher = self.root / 'package.json'
        self.package = json.loads((ROOT / 'rendering/bevy-explorer/source/deploy/headless/launcher/package.json').read_text())
        self.launcher.write_text(json.dumps(self.package))
        self.names = list(self.package['optionalDependencies'])

    def archive(self, name, version='0.1.0-42.commit-aaaaaaa', filename=None):
        body = json.dumps({'name': name, 'version': version}).encode()
        with tarfile.open(self.root / (filename or name.split('/')[-1] + '.tgz'), 'w:gz') as tar:
            member = tarfile.TarInfo('package/package.json')
            member.size = len(body)
            tar.addfile(member, io.BytesIO(body))

    def test_uses_built_version_instead_of_callback_revision(self):
        for name in self.names:
            self.archive(name)
        self.assertEqual(packages.pin_launcher(self.root, self.launcher, "a" * 40), '0.1.0-42.commit-aaaaaaa')
        result = json.loads(self.launcher.read_text())
        self.assertEqual(result['commit'], 'a' * 40)
        self.assertEqual(set(result['optionalDependencies'].values()), {'0.1.0-42.commit-aaaaaaa'})

    def test_other_revision_does_not_modify_the_launcher(self):
        before = self.launcher.read_text()
        for name in self.names:
            self.archive(name)
        with self.assertRaises(ValueError):
            packages.pin_launcher(self.root, self.launcher, 'b' * 40)
        self.assertEqual(self.launcher.read_text(), before)

    def test_partial_mixed_or_duplicate_archives_never_modify_launcher(self):
        original = self.launcher.read_text()
        for name in self.names[:3]:
            self.archive(name)
        with self.assertRaises(ValueError):
            packages.pin_launcher(self.root, self.launcher, "a" * 40)
        self.archive(self.names[3], '0.1.0-other')
        with self.assertRaises(ValueError):
            packages.pin_launcher(self.root, self.launcher, "a" * 40)
        self.archive(self.names[3])
        self.archive(self.names[0], filename='duplicate.tgz')
        with self.assertRaises(ValueError):
            packages.pin_launcher(self.root, self.launcher, "a" * 40)
        self.assertEqual(self.launcher.read_text(), original)


if __name__ == '__main__':
    unittest.main()
