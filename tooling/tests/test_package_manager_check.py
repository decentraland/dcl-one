"""Internal JavaScript projects install with pnpm."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location('package_manager_check', Path(__file__).parents[1] / 'package-manager-check.py')
check = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(check)


def write(root, rel, text=''):
    path = root / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding='utf-8')


class PackageManagerTest(unittest.TestCase):
    def test_pnpm_projects_and_creator_scenes_pass(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(root, 'apps/x/pnpm-lock.yaml')
            write(root, 'apps/x/package.json', '{"packageManager": "pnpm@%s+sha512.%s"}' % (check.PNPM_VERSION, check.PNPM_SHA512))
            write(root, 'apps/w/pnpm-lock.yaml')
            write(root, 'apps/w/package.json', '{"packageManager": "pnpm@%s"}' % check.PNPM_VERSION)
            write(root, 'apps/x/pnpm-workspace.yaml', 'ignoreScripts: true\nsaveExact: true\n')
            write(root, 'infra/dev/service-test/fixture/apps/x/pnpm-lock.yaml')
            write(root, 'apps/x/.npmrc', '\n'.join((
                '; registry and auth only',
                'registry=https://registry.npmjs.org/',
                '@dcl:registry=https://registry.npmjs.org/',
                '//registry.npmjs.org/:_authToken=${NPM_TOKEN}',
                'strict-ssl = true')))
            write(root, 'apps/x/node_modules/y/package-lock.json')
            write(root, 'library/js-sdk-toolchain/test/snapshots/package-lock.json')
            write(root, 'apps/v/package.json', '{"scripts": {"build": "pnpm run lib && tsc", "lib": "tsc -p lib"}}')
            write(root, 'library/dcl-scene-build/testdata/t/package.json',
                  '{"scripts": {"upgrade-sdk": "npm install --save-dev @dcl/sdk@latest"}}')
            write(root, 'rendering/bevy-explorer/source/react-web/package.json',
                  '{"scripts": {"bundle:native": "vite build && npm --prefix bridge-scene run export-native"}}')
            write(root, '.github/workflows/x.yml', '\n'.join((
                '      # npm run build is what this used to call',
                "      paths: ['**/yarn.lock', '**/package-lock.json']",
                '      - run: corepack enable && pnpm install --frozen-lockfile && pnpm run build',
                '      - run: corepack pnpm@12.21.0 install --frozen-lockfile',
                '      - run: pnpm pack && pnpm publish "$tgz" --access public --no-git-checks && pnpm whoami',
                '      - run: corepack pnpm@12.21.0 view "$pkg" dist-tags --json && pnpm dist-tag rm "$pkg" "$TAG"',
                '      - run: echo "Verifying npm authentication" && npm_preview_field pr-1 version',
                "      - run: link=$(artifact_link 'headless-' 'npm package')")))
            self.assertEqual(check.offenders(root), [])

    def test_npm_lockfiles_unread_settings_and_npm_in_workflows_fail(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(root, 'apps/x/package-lock.json')
            write(root, 'apps/y/yarn.lock')
            write(root, 'apps/p/pnpm-lock.yaml')
            write(root, 'apps/p/package.json', '{"packageManager": "pnpm@10.19.0"}')
            write(root, 'apps/q/pnpm-lock.yaml')
            write(root, 'apps/q/package.json', '{}')
            write(root, 'apps/r/pnpm-lock.yaml')
            write(root, 'apps/r/package.json', '{"packageManager": "pnpm@%s+sha512.abc"}' % check.PNPM_VERSION)
            write(root, 'apps/s/package.json', '{"scripts": {"a": "tsc", "build": "npm run a", "e": "yarn x", "lint": "npx eslint ."}}')
            write(root, 'rendering/bevy-explorer/source/react-web/package.json',
                  '{"scripts": {"bundle:native": "npm --prefix b run x", "dev": "npm run x"}}')
            write(root, 'apps/z/.npmrc', '\n'.join((
                'ignore-scripts=true', 'save-exact = true', 'allow-builds=esbuild', 'minimum-release-age=0', 'fund=false')))
            commands = (
                'npm ci', 'cd app && npm run build', 'npx tsc', 'npm --prefix app ci', 'npm -w pkg run build',
                'npm --silent install', 'npx --yes npm@11.21.1 ci', 'npx --yes pnpm@12.21.0 install', 'yarn install',
                'cd app && yarnpkg add thing', 'npm pack && npm publish "$tgz"', 'npm --registry https://r.example view x',
                'npm whoami', 'npx --yes npm@11.21.1 dist-tag rm "$pkg" "$TAG"', 'corepack pnpm@11.0.0 install')
            write(root, '.github/workflows/x.yml', ''.join(f'      - run: {command}\n' for command in commands))
            write(root, '.github/actions/a/run.sh', 'npm install --global thing\n')
            self.assertEqual(check.offenders(root), [
                '.github/actions/a/run.sh:1: runs npm or yarn; use pnpm',
                *(f'.github/workflows/x.yml:{line}: runs npm or yarn; use pnpm'
                  for line in range(1, len(commands))),
                f'.github/workflows/x.yml:{len(commands)}: fetches pnpm 11.0.0; the projects pin {check.PNPM_VERSION}',
                f'apps/p/package.json: pins `pnpm@10.19.0`; set packageManager to pnpm@{check.PNPM_VERSION}',
                f'apps/q/package.json: pins `nothing`; set packageManager to pnpm@{check.PNPM_VERSION}',
                f'apps/r/package.json: pins pnpm {check.PNPM_VERSION} with another integrity; use sha512.{check.PNPM_SHA512}',
                *(f'apps/s/package.json: script `{script}` runs npm or yarn; use pnpm' for script in ('build', 'e', 'lint')),
                'apps/x/package-lock.json: npm or yarn lockfile; convert it with `pnpm import` and delete it',
                'apps/y/yarn.lock: npm or yarn lockfile; convert it with `pnpm import` and delete it',
                *(f'apps/z/.npmrc: pnpm does not read `{key}` here; move it to pnpm-workspace.yaml or delete it'
                  for key in ('ignore-scripts', 'save-exact', 'allow-builds', 'minimum-release-age', 'fund')),
                'rendering/bevy-explorer/source/react-web/package.json: script `dev` runs npm or yarn; use pnpm'])

    def test_the_repository_passes(self):
        self.assertEqual(check.offenders(check.ROOT), [])


if __name__ == '__main__':
    unittest.main()
