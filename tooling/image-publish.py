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
             'bevy-explorer-web': ('amd64', 'arm64'), 'bevy-explorer-headless': ('amd64',),
             'sites-polygon-credits': ('amd64', 'arm64')}
# A component built for one deployment (image-sites-polygon-credits): published into its base
# component's repository under its own tag suffix, never under the base's tags.
VARIANTS = {'sites-polygon-credits': ('sites', 'polygon-credits')}


def archive_path(root, component, arch, sha):
    if component in ('apis', 'sites', *VARIANTS):
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
        tags = [f'sha-{sha}', 'main']
    elif ref.startswith('refs/tags/v') and re.fullmatch(r'v[\w.-]{0,127}', ref[10:], re.ASCII):
        tags = [f'sha-{sha}', ref[10:]]
    else:
        raise ValueError('Publication is restricted to main and v* release tags')
    # A variant shares its base's repository under a suffix: a release named like a variant tag
    # would publish over another release's variant, and a suffixed tag must still fit Docker's 128.
    for _, suffix in VARIANTS.values():
        if any(tag.endswith(f'-{suffix}') for tag in tags):
            raise ValueError(f'A release tag cannot end in a variant suffix (-{suffix})')
        if any(len(f'{tag}-{suffix}') > 128 for tag in tags):
            raise ValueError(f'A release tag is too long for its -{suffix} variant (128 characters)')
    return tags


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archives', type=Path)
    parser.add_argument('--component', choices=PLATFORMS, help='Publish one independently named component')
    parser.add_argument('--revision', help='Tested source commit for workflow_run artifact publication')
    parser.add_argument('--ref', help='Source ref validated by the calling workflow (defaults to GITHUB_REF)')
    args = parser.parse_args()
    sha = args.revision or os.environ.get('GITHUB_SHA', '')
    tags = release_tags(sha, args.ref or os.environ.get('GITHUB_REF', ''))
    components = (args.component,) if args.component else ('apis', 'sites', *VARIANTS)
    configured = [os.environ.get(f'QUAY_{name.upper().replace("-", "_")}_REPOSITORY')
                  for name in PLATFORMS if name not in VARIANTS]
    configured = [value for value in configured if value]
    if len(set(configured)) != len(configured):
        raise ValueError('Components require different repositories')
    base = {service: VARIANTS[service][0] if service in VARIANTS else service for service in components}
    repositories = {service: repository(os.environ.get(f'QUAY_{base[service].upper().replace("-", "_")}_REPOSITORY', ''))
                    for service in components}
    own = [repo for service, repo in repositories.items() if service not in VARIANTS]
    if len(set(own)) != len(own):
        raise ValueError('Components require different repositories')
    suffix = {service: f'-{VARIANTS[service][1]}' if service in VARIANTS else '' for service in components}
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
        # Every per-architecture image first, then the moving tags: a copy that fails leaves no
        # component's `main` (or release) moved ahead of the others.
        sources = {}
        for service, repo in repositories.items():
            sources[service] = []
            for arch in PLATFORMS[service]:
                archive = archive_path(args.archives, service, arch, sha)
                digestfile = Path(temporary) / f'{service}-{arch}.digest'
                run('skopeo', 'copy', '--retry-times', '3', '--authfile', str(authfile),
                    '--format', 'oci', '--digestfile', str(digestfile), f'docker-archive:{archive}',
                    f'docker://{repo}:sha-{sha}{suffix[service]}-{arch}')
                digest = digestfile.read_text().strip()
                if not re.fullmatch(r'sha256:[0-9a-f]{64}', digest):
                    raise ValueError('Registry copy did not return a valid digest')
                sources[service].append(f'{repo}@{digest}')
        for service, repo in repositories.items():
            service_tags = [f'{tag}{suffix[service]}' for tag in tags]
            tag_args = [item for tag in service_tags for item in ('--tag', f'{repo}:{tag}')]
            run('docker', 'buildx', 'imagetools', 'create', *tag_args, *sources[service], env=env)
            manifest = json.loads(run('docker', 'buildx', 'imagetools', 'inspect', '--raw',
                                     f'{repo}:{service_tags[0]}', env=env, capture_output=True).stdout)
            platforms = {(m['platform']['os'], m['platform']['architecture']) for m in manifest['manifests']}
            if platforms != {('linux', arch) for arch in PLATFORMS[service]}:
                raise ValueError(f'Published index has unexpected platforms: {platforms}')
            platform_names = ', '.join(f'linux/{arch}' for arch in PLATFORMS[service])
            print(f'Published {repo}:{service_tags[0]} ({platform_names})')
            summary = os.environ.get('GITHUB_STEP_SUMMARY')
            if summary:
                with open(summary, 'a') as output:
                    output.write(f'- `{repo}:{service_tags[0]}` — {platform_names}\n')


if __name__ == '__main__':
    main()
