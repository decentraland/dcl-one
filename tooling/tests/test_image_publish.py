"""Publication guards are evaluated before contacting the registry."""
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('image_publish', Path(__file__).parents[1] / 'image-publish.py')
publish = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(publish)


class PublicationTest(unittest.TestCase):
    def test_gateway_uses_its_own_artifacts_and_publishes_both_architectures(self):
        sha = 'a' * 40
        env = {'GITHUB_SHA': sha, 'GITHUB_REF': 'refs/heads/main',
               'QUAY_GATEWAY_REPOSITORY': 'quay.io/example/dcl-one-gateway',
               'QUAY_USERNAME': 'example+test', 'QUAY_TOKEN': 'test-only'}

        def command(*args, **kwargs):
            output = ''
            if args[:2] == ('skopeo', 'inspect'):
                arch = 'arm64' if '-arm64-' in args[3] else 'amd64'
                self.assertEqual(args[3],
                    f'docker-archive:/archives/gateway-image-linux-{arch}-{sha}/image.tar.gz')
                output = json.dumps({'architecture': arch, 'os': 'linux', 'config': {'Labels': {
                    'org.opencontainers.image.revision': sha}}})
            elif args[:2] == ('skopeo', 'copy'):
                Path(args[args.index('--digestfile') + 1]).write_text('sha256:' + '1' * 64)
            elif args[:4] == ('docker', 'buildx', 'imagetools', 'inspect'):
                output = json.dumps({'manifests': [
                    {'platform': {'os': 'linux', 'architecture': arch}} for arch in ('amd64', 'arm64')]})
            return subprocess.CompletedProcess(args, 0, stdout=output)

        with patch.dict(publish.os.environ, env, clear=True), \
                patch('sys.argv', ['image-publish.py', '/archives', '--component', 'gateway']), \
                patch.object(publish, 'run', side_effect=command) as commands, \
                patch('sys.stdout', new_callable=io.StringIO):
            publish.main()
        calls = [call.args for call in commands.call_args_list]
        self.assertTrue(all(call[:2] == ('skopeo', 'inspect') for call in calls[:2]))
        copies = [call for call in calls if call[:2] == ('skopeo', 'copy')]
        self.assertEqual([call[-1] for call in copies], [
            f'docker://quay.io/example/dcl-one-gateway:sha-{sha}-{arch}' for arch in ('amd64', 'arm64')])
        indexes = [call for call in calls if call[:4] == ('docker', 'buildx', 'imagetools', 'create')]
        self.assertEqual(len(indexes), 1)
        self.assertIn('quay.io/example/dcl-one-gateway:main', indexes[0])
        self.assertIn(f'quay.io/example/dcl-one-gateway:sha-{sha}', indexes[0])

    def test_apis_and_sites_publish_both_architectures_from_renamed_artifacts(self):
        sha = 'a' * 40
        env = {'GITHUB_SHA': sha, 'GITHUB_REF': 'refs/heads/main',
               'QUAY_APIS_REPOSITORY': 'quay.io/example/dcl-one-apis',
               'QUAY_SITES_REPOSITORY': 'quay.io/example/dcl-one-sites',
               'QUAY_USERNAME': 'example+test', 'QUAY_TOKEN': 'test-only'}

        def command(*args, **kwargs):
            output = ''
            if args[:2] == ('skopeo', 'inspect'):
                arch = 'arm64' if '/apis-sites-images-arm64/' in args[3] else 'amd64'
                output = json.dumps({'architecture': arch, 'os': 'linux', 'config': {'Labels': {
                    'org.opencontainers.image.revision': sha}}})
            elif args[:2] == ('skopeo', 'copy'):
                Path(args[args.index('--digestfile') + 1]).write_text('sha256:' + '1' * 64)
            elif args[:4] == ('docker', 'buildx', 'imagetools', 'inspect'):
                output = json.dumps({'manifests': [
                    {'platform': {'os': 'linux', 'architecture': arch}} for arch in ('amd64', 'arm64')]})
            return subprocess.CompletedProcess(args, 0, stdout=output)

        with patch.dict(publish.os.environ, env, clear=True), \
                patch('sys.argv', ['image-publish.py', '/archives']), \
                patch.object(publish, 'run', side_effect=command) as commands, \
                patch('sys.stdout', new_callable=io.StringIO):
            publish.main()
        calls = [call.args for call in commands.call_args_list]
        expected_archives = [
            f'docker-archive:/archives/apis-sites-images-{arch}/{component}.tar.gz'
            for component in ('apis', 'sites') for arch in ('amd64', 'arm64')]
        self.assertEqual([call[3] for call in calls[:4]], expected_archives)
        self.assertTrue(all(call[:3] == ('skopeo', 'inspect', '--config') for call in calls[:4]))
        copies = [call for call in calls if call[:2] == ('skopeo', 'copy')]
        self.assertEqual([call[-2] for call in copies], expected_archives)
        self.assertEqual([call[-1] for call in copies], [
            f'docker://quay.io/example/dcl-one-{component}:sha-{sha}-{arch}'
            for component in ('apis', 'sites') for arch in ('amd64', 'arm64')])
        indexes = [call for call in calls if call[:4] == ('docker', 'buildx', 'imagetools', 'create')]
        self.assertEqual(len(indexes), 2)
        for component, index in zip(('apis', 'sites'), indexes):
            self.assertIn(f'quay.io/example/dcl-one-{component}:main', index)
            self.assertIn(f'quay.io/example/dcl-one-{component}:sha-{sha}', index)

    def test_bevy_web_validates_both_archives_against_the_source_run_revision(self):
        sha = 'a' * 40
        env = {'GITHUB_SHA': 'b' * 40, 'GITHUB_REF': 'refs/heads/main',
               'QUAY_BEVY_EXPLORER_WEB_REPOSITORY': 'quay.io/example/bevy-web',
               'QUAY_USERNAME': 'example+test', 'QUAY_TOKEN': 'test-only'}

        def inspect(*args, **kwargs):
            self.assertEqual(args[:3], ('skopeo', 'inspect', '--config'))
            arch = 'arm64' if '-arm64-' in args[3] else 'amd64'
            self.assertIn(f'bevy-explorer-web-image-linux-{arch}-{sha}/image.tar.gz', args[3])
            config = {'architecture': arch, 'os': 'linux', 'config': {'Labels': {
                'org.opencontainers.image.revision': sha if arch == 'amd64' else 'c' * 40}}}
            return subprocess.CompletedProcess(args, 0, stdout=json.dumps(config))

        with patch.dict(publish.os.environ, env, clear=True), patch('sys.argv',
                ['image-publish.py', '/archives', '--component', 'bevy-explorer-web', '--revision', sha]), \
                patch.object(publish, 'run', side_effect=inspect) as commands:
            with self.assertRaisesRegex(ValueError, 'revision does not match'):
                publish.main()
        self.assertEqual(commands.call_count, 2)

    def test_headless_publishes_verified_main_artifacts_when_default_branch_is_dev(self):
        sha = 'a' * 40
        env = {'GITHUB_SHA': 'b' * 40, 'GITHUB_REF': 'refs/heads/dev',
               'QUAY_BEVY_EXPLORER_HEADLESS_REPOSITORY': 'quay.io/example/bevy-headless',
               'QUAY_USERNAME': 'example+test', 'QUAY_TOKEN': 'test-only'}

        def command(*args, **kwargs):
            output = ''
            if args[:2] == ('skopeo', 'inspect'):
                output = json.dumps({'architecture': 'amd64', 'os': 'linux', 'config': {'Labels': {
                    'org.opencontainers.image.revision': sha}}})
            elif args[:2] == ('skopeo', 'copy'):
                Path(args[args.index('--digestfile') + 1]).write_text('sha256:' + '1' * 64)
            elif args[:4] == ('docker', 'buildx', 'imagetools', 'inspect'):
                output = json.dumps({'manifests': [{'platform': {'os': 'linux', 'architecture': 'amd64'}}]})
            return subprocess.CompletedProcess(args, 0, stdout=output)

        with patch.dict(publish.os.environ, env, clear=True), patch('sys.argv',
                ['image-publish.py', '/archives', '--component', 'bevy-explorer-headless',
                 '--revision', sha, '--ref', 'refs/heads/main']), \
                patch.object(publish, 'run', side_effect=command) as commands, patch('sys.stdout', new_callable=io.StringIO):
            publish.main()
        copies = [call.args for call in commands.call_args_list if call.args[:2] == ('skopeo', 'copy')]
        self.assertEqual(len(copies), 1)
        self.assertEqual(copies[0][-1], f'docker://quay.io/example/bevy-headless:sha-{sha}-amd64')
        indexes = [call.args for call in commands.call_args_list
                   if call.args[:4] == ('docker', 'buildx', 'imagetools', 'create')]
        self.assertIn(f'quay.io/example/bevy-headless:main', indexes[0])
        self.assertIn(f'quay.io/example/bevy-headless:sha-{sha}', indexes[0])

    def test_repositories_cannot_redirect_credentials_or_supply_a_tag(self):
        self.assertEqual(publish.repository('quay.io/example/foundation-sites'), 'quay.io/example/foundation-sites')
        for value in ('', 'example/foundation', 'other.invalid/org/repo',
                      'quay.io/example/repo:main', 'quay.io/example/repo@sha256:abc'):
            with self.subTest(repository=value), self.assertRaises(ValueError):
                publish.repository(value)

    def test_publication_tags_bind_to_commit_and_branch_or_release(self):
        sha = 'a' * 40
        self.assertEqual(publish.release_tags(sha, 'refs/heads/main'), [f'sha-{sha}', 'main'])
        self.assertEqual(publish.release_tags(sha, 'refs/tags/v1.2.3'), [f'sha-{sha}', 'v1.2.3'])
        for ref in ('refs/pull/1/merge', 'refs/heads/dev', 'refs/heads/feature', 'refs/tags/v1/bad', 'refs/tags/release'):
            with self.subTest(ref=ref), self.assertRaises(ValueError):
                publish.release_tags(sha, ref)
        with self.assertRaises(ValueError):
            publish.release_tags('short', 'refs/heads/main')

    def test_invalid_last_archive_prevents_every_registry_write(self):
        sha = 'a' * 40
        env = {'GITHUB_SHA': sha, 'GITHUB_REF': 'refs/heads/main',
               'QUAY_APIS_REPOSITORY': 'quay.io/example/apis',
               'QUAY_SITES_REPOSITORY': 'quay.io/example/sites',
               'QUAY_USERNAME': 'example+test', 'QUAY_TOKEN': 'test-only'}

        def inspect(*args, **kwargs):
            self.assertEqual(args[:3], ('skopeo', 'inspect', '--config'))
            arch = 'arm64' if 'apis-sites-images-arm64' in args[3] else 'amd64'
            revision = 'b' * 40 if args[3].endswith('apis-sites-images-arm64/sites.tar.gz') else sha
            config = {'architecture': arch, 'os': 'linux',
                      'config': {'Labels': {'org.opencontainers.image.revision': revision}}}
            return subprocess.CompletedProcess(args, 0, stdout=json.dumps(config))

        with patch.dict(publish.os.environ, env, clear=True), \
                patch('sys.argv', ['image-publish.py', '/test-archives']), \
                patch.object(publish, 'run', side_effect=inspect) as commands:
            with self.assertRaisesRegex(ValueError, 'revision does not match'):
                publish.main()
        self.assertEqual(commands.call_count, 4)
