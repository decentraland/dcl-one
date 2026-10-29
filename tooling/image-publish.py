#!/usr/bin/env python3
"""Publish already tested image archives; credentials are runtime environment only."""
import argparse
import base64
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

PLATFORMS = {'apis': ('amd64', 'arm64'), 'sites': ('amd64', 'arm64'), 'gateway': ('amd64', 'arm64'),
             'bevy-explorer-web': ('amd64', 'arm64'), 'bevy-explorer-headless': ('amd64',)}


def archive_path(root, component, arch, sha):
    if component in ('apis', 'sites'):
        return root / f'apis-sites-images-{arch}' / f'{component}.tar.gz'
    return root / f'{component}-image-linux-{arch}-{sha}' / 'image.tar.gz'


def run(*args, **kwargs):
    return subprocess.run(args, check=True, text=True, **kwargs)


def repository(value):
    if not re.fullmatch(r'quay\.io/[a-z0-9][a-z0-9_-]*/[a-z0-9][a-z0-9._-]*', value):
        raise ValueError('Set each QUAY_*_REPOSITORY to quay.io/organization/repository (without a tag)')
    return value


def release_tags(sha, ref):
    if not re.fullmatch(r'[0-9a-f]{40}', sha):
        raise ValueError('GITHUB_SHA must be a full commit SHA')
    if ref == 'refs/heads/main':
        return [f'sha-{sha}', 'main']
    if ref.startswith('refs/tags/v') and re.fullmatch(r'v[\w.-]{0,127}', ref[10:], re.ASCII):
        return [f'sha-{sha}', ref[10:]]
    raise ValueError('Publication is restricted to main and v* release tags')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archives', type=Path)
    parser.add_argument('--component', choices=PLATFORMS, help='Publish one independently named component')
    parser.add_argument('--revision', help='Tested source commit for workflow_run artifact publication')
    parser.add_argument('--ref', help='Source ref validated by the calling workflow (defaults to GITHUB_REF)')
    args = parser.parse_args()
    sha = args.revision or os.environ.get('GITHUB_SHA', '')
    tags = release_tags(sha, args.ref or os.environ.get('GITHUB_REF', ''))
    components = (args.component,) if args.component else ('apis', 'sites')
    configured = [os.environ.get(f'QUAY_{name.upper().replace("-", "_")}_REPOSITORY') for name in PLATFORMS]
    configured = [value for value in configured if value]
    if len(set(configured)) != len(configured):
        raise ValueError('Components require different repositories')
    repositories = {service: repository(os.environ.get(f'QUAY_{service.upper().replace("-", "_")}_REPOSITORY', ''))
                    for service in components}
    if len(set(repositories.values())) != len(repositories):
        raise ValueError('Components require different repositories')
    user, token = (os.environ.get(key, '') for key in ('QUAY_USERNAME', 'QUAY_TOKEN'))
    if not user or not token:
        raise ValueError('Configure the QUAY_USERNAME and QUAY_TOKEN GitHub secrets')

    # Validate every archive before the first registry mutation.
    for service in repositories:
        for arch in PLATFORMS[service]:
            archive = archive_path(args.archives, service, arch, sha)
            config = json.loads(run('skopeo', 'inspect', '--config', f'docker-archive:{archive}',
                                    capture_output=True).stdout)
            if (config.get('architecture') != arch or config.get('os') != 'linux' or
                    config.get('config', {}).get('Labels', {}).get('org.opencontainers.image.revision') != sha):
                raise ValueError(f'Archive architecture or revision does not match the release: {archive}')

    with tempfile.TemporaryDirectory(prefix='foundation-quay-') as temporary:
        authfile = Path(temporary) / 'config.json'
        auth = base64.b64encode(f'{user}:{token}'.encode()).decode()
        # setup-buildx-action installs the plugin in the original Docker config.
        plugin_dir = Path(os.environ.get('DOCKER_CONFIG') or Path.home() / '.docker') / 'cli-plugins'
        authfile.write_text(json.dumps({'auths': {'quay.io': {'auth': auth}},
                                       'cliPluginsExtraDirs': [str(plugin_dir)]}))
        authfile.chmod(0o600)
        env = {**os.environ, 'DOCKER_CONFIG': temporary}
        run('docker', 'buildx', 'version', env=env)
        for service, repo in repositories.items():
            sources = []
            for arch in PLATFORMS[service]:
                archive = archive_path(args.archives, service, arch, sha)
                digestfile = Path(temporary) / f'{service}-{arch}.digest'
                run('skopeo', 'copy', '--retry-times', '3', '--authfile', str(authfile),
                    '--format', 'oci', '--digestfile', str(digestfile), f'docker-archive:{archive}',
                    f'docker://{repo}:sha-{sha}-{arch}')
                digest = digestfile.read_text().strip()
                if not re.fullmatch(r'sha256:[0-9a-f]{64}', digest):
                    raise ValueError('Registry copy did not return a valid digest')
                sources.append(f'{repo}@{digest}')
            tag_args = [item for tag in tags for item in ('--tag', f'{repo}:{tag}')]
            run('docker', 'buildx', 'imagetools', 'create', *tag_args, *sources, env=env)
            manifest = json.loads(run('docker', 'buildx', 'imagetools', 'inspect', '--raw',
                                     f'{repo}:{tags[0]}', env=env, capture_output=True).stdout)
            platforms = {(m['platform']['os'], m['platform']['architecture']) for m in manifest['manifests']}
            if platforms != {('linux', arch) for arch in PLATFORMS[service]}:
                raise ValueError(f'Published index has unexpected platforms: {platforms}')
            platform_names = ', '.join(f'linux/{arch}' for arch in PLATFORMS[service])
            print(f'Published {repo}:{tags[0]} ({platform_names})')
            summary = os.environ.get('GITHUB_STEP_SUMMARY')
            if summary:
                with open(summary, 'a') as output:
                    output.write(f'- `{repo}:{tags[0]}` — {platform_names}\n')


if __name__ == '__main__':
    main()
