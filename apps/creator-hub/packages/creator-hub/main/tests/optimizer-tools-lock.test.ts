import { createHash } from 'node:crypto';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  LOCK_FILE,
  MANIFEST_FILE,
  checkLock,
  fetchRegistryPackages,
  readMinimumReleaseAge,
  registryIntegrityProblems,
  registryTarball,
  releaseAgeProblems,
  writeCheckedLock,
} from '../../scripts/lock-optimizer-tools.js';

const REGISTRY = 'https://registry.npmjs.org/';
const MINUTE = 60_000;
const NOW = Date.parse('2026-06-10T00:00:00.000Z');

type Entry = {
  name?: string;
  version?: string;
  resolved?: string;
  integrity?: string;
  hasInstallScript?: boolean;
  dependencies?: Record<string, string>;
};
type Lock = { lockfileVersion: number; packages: Record<string, Entry> };
type Packument = {
  time?: Record<string, string>;
  versions?: Record<string, { dist?: { integrity?: string } }>;
};

function sha512(content: string) {
  return `sha512-${createHash('sha512').update(content).digest('base64')}`;
}

function makeManifest() {
  return { dependencies: { tool: '1.2.0' } };
}

function makeLock(): Lock {
  return {
    lockfileVersion: 3,
    packages: {
      '': { dependencies: { tool: '1.2.0' } },
      'node_modules/tool': {
        version: '1.2.0',
        resolved: `${REGISTRY}tool/-/tool-1.2.0.tgz`,
        integrity: sha512('tool'),
      },
      'node_modules/@scope/dep': {
        version: '3.0.1',
        resolved: `${REGISTRY}@scope/dep/-/dep-3.0.1.tgz`,
        integrity: sha512('dep'),
      },
    },
  };
}

function makePackuments(toolPublished: string, depPublished: string): Record<string, Packument> {
  return {
    tool: {
      time: { '1.2.0': toolPublished },
      versions: { '1.2.0': { dist: { integrity: sha512('tool') } } },
    },
    '@scope/dep': {
      time: { '3.0.1': depPublished },
      versions: { '3.0.1': { dist: { integrity: sha512('dep') } } },
    },
  };
}

function registry(packuments: Record<string, Packument>) {
  return vi.fn(async (url: string) => {
    const name = decodeURIComponent(url.slice(REGISTRY.length));
    return {
      ok: name in packuments,
      status: name in packuments ? 200 : 404,
      json: async () => packuments[name],
    };
  });
}

describe('the optimizer tools lock', () => {
  describe('when the committed lock is checked against the pinned package.json', () => {
    it('should have no problem', async () => {
      const manifest = JSON.parse(await fs.readFile(MANIFEST_FILE, 'utf8'));
      const lock = JSON.parse(await fs.readFile(LOCK_FILE, 'utf8'));

      expect(Object.keys(manifest.dependencies).length).toBeGreaterThan(0);
      expect(checkLock(manifest, lock)).toEqual([]);
    });
  });

  describe('checkLock', () => {
    let manifest: ReturnType<typeof makeManifest>;
    let lock: Lock;

    beforeEach(() => {
      manifest = makeManifest();
      lock = makeLock();
    });

    describe('when the lock agrees with the pins', () => {
      it('should report nothing', () => {
        expect(checkLock(manifest, lock)).toEqual([]);
      });
    });

    describe('when a pin moved and the lock was not made again', () => {
      it('should name the package and both versions', () => {
        manifest.dependencies.tool = '1.3.0';

        expect(checkLock(manifest, lock)).toEqual([
          'tool: pinned 1.3.0, the lock declares 1.2.0 and has 1.2.0',
        ]);
      });
    });

    describe('when the lock installs another version than the one it declares', () => {
      it('should report the package', () => {
        lock.packages['node_modules/tool'].version = '1.2.1';
        lock.packages['node_modules/tool'].resolved = `${REGISTRY}tool/-/tool-1.2.1.tgz`;

        expect(checkLock(manifest, lock)).toEqual([
          'tool: pinned 1.2.0, the lock declares 1.2.0 and has 1.2.1',
        ]);
      });
    });

    describe('when the lock has a top-level package that is not pinned', () => {
      it('should report the package', () => {
        lock.packages[''].dependencies = { tool: '1.2.0', extra: '2.0.0' };

        expect(checkLock(manifest, lock)).toEqual([
          'extra: pinned undefined, the lock declares 2.0.0 and has undefined',
        ]);
      });
    });

    describe('when an entry has no integrity', () => {
      it('should report the entry', () => {
        delete lock.packages['node_modules/@scope/dep'].integrity;

        expect(checkLock(manifest, lock)).toEqual(['node_modules/@scope/dep: no integrity']);
      });
    });

    describe('when an entry has an integrity that is not one sha512 hash', () => {
      it.each([
        'sha1-2jmj7l5rSw0yVb/vlWAYkK/YBwk=',
        'sha512-dep',
        `${sha512('dep')} ${sha512('other')}`,
      ])('should report the entry for %s', integrity => {
        lock.packages['node_modules/@scope/dep'].integrity = integrity;

        expect(checkLock(manifest, lock)).toEqual([
          `node_modules/@scope/dep: the integrity is not one sha512 hash (${integrity})`,
        ]);
      });
    });

    describe('when an entry does not resolve to the registry tarball of its name and version', () => {
      it.each([
        ['outside the npm registry', 'https://registry.npmjs.org.example.com/dep/-/dep-3.0.1.tgz'],
        ['over http', 'http://registry.npmjs.org/@scope/dep/-/dep-3.0.1.tgz'],
        ['to a git repository', 'git+ssh://git@github.com/scope/dep.git#0123456'],
        ['to nothing', undefined],
        ['to the tarball of another package', `${REGISTRY}other/-/other-3.0.1.tgz`],
        ['to the tarball of the package without its scope', `${REGISTRY}dep/-/dep-3.0.1.tgz`],
        ['to the tarball of another version', `${REGISTRY}@scope/dep/-/dep-3.0.2.tgz`],
        ['to its tarball with a query', `${REGISTRY}@scope/dep/-/dep-3.0.1.tgz?v=2`],
      ])('should report the entry when it resolves %s', (_case, resolved) => {
        lock.packages['node_modules/@scope/dep'].resolved = resolved;

        expect(checkLock(manifest, lock)).toEqual([
          `node_modules/@scope/dep: resolves to ${resolved}, ` +
            `not to ${REGISTRY}@scope/dep/-/dep-3.0.1.tgz`,
        ]);
      });
    });

    describe('when a pinned package resolves to the tarball of another package', () => {
      it('should report the entry, with the integrity of that tarball too', () => {
        lock.packages['node_modules/tool'].resolved = `${REGISTRY}other/-/other-1.2.0.tgz`;
        lock.packages['node_modules/tool'].integrity = sha512('other');

        expect(checkLock(manifest, lock)).toEqual([
          `node_modules/tool: resolves to ${REGISTRY}other/-/other-1.2.0.tgz, ` +
            `not to ${REGISTRY}tool/-/tool-1.2.0.tgz`,
        ]);
      });
    });

    describe('when the version of an entry holds a path to another tarball', () => {
      it('should report the entry', () => {
        const version = '3.0.1/../../../other/-/other-3.0.1';
        lock.packages['node_modules/@scope/dep'].version = version;
        lock.packages['node_modules/@scope/dep'].resolved =
          `${REGISTRY}@scope/dep/-/dep-${version}.tgz`;

        expect(checkLock(manifest, lock)).toEqual([
          `node_modules/@scope/dep: @scope/dep@${version} is not a name and a version`,
        ]);
      });
    });

    describe('when a scoped package is nested under another package', () => {
      it('should report nothing', () => {
        lock.packages['node_modules/tool/node_modules/@scope/dep'] = {
          version: '2.0.0-beta.1',
          resolved: `${REGISTRY}@scope/dep/-/dep-2.0.0-beta.1.tgz`,
          integrity: sha512('dep 2'),
        };

        expect(checkLock(manifest, lock)).toEqual([]);
      });
    });

    describe('when the key of an entry is not a node_modules path of package names', () => {
      it.each(['packages/dep', 'node_modules/../dep', 'node_modules/@scope/dep/lib'])(
        'should report the entry for %s',
        key => {
          lock.packages[key] = lock.packages['node_modules/@scope/dep'];

          expect(checkLock(manifest, lock)).toContain(
            `${key}: the key is not a node_modules path of package names`,
          );
        },
      );
    });

    describe('when an entry has a name that is not the name of its key', () => {
      beforeEach(() => {
        lock.packages['node_modules/dep-cjs'] = {
          name: '@scope/dep',
          version: '3.0.1',
          resolved: `${REGISTRY}@scope/dep/-/dep-3.0.1.tgz`,
          integrity: sha512('dep'),
        };
      });

      it('should report the entry', () => {
        expect(checkLock(manifest, lock)).toEqual([
          'node_modules/dep-cjs: is named @scope/dep, not dep-cjs',
        ]);
      });

      it('should report the entry when the lock declares the name as an alias of the key', () => {
        lock.packages['node_modules/tool'].dependencies = { 'dep-cjs': 'npm:@scope/dep@^3.0.0' };

        expect(checkLock(manifest, lock)).toEqual([
          'node_modules/dep-cjs: is named @scope/dep, not dep-cjs',
        ]);
      });

      it('should report a pinned package that takes the name of another package', () => {
        lock.packages['node_modules/tool'].name = 'other';
        lock.packages['node_modules/tool'].resolved = `${REGISTRY}other/-/other-1.2.0.tgz`;
        lock.packages['node_modules/@scope/dep'].dependencies = { tool: 'npm:other@1.2.0' };

        expect(checkLock(manifest, lock)).toContain('node_modules/tool: is named other, not tool');
      });
    });

    describe('when an entry has an install script', () => {
      beforeEach(() => {
        lock.packages['node_modules/@scope/dep'].hasInstallScript = true;
      });

      it('should report the entry when the package is not expected to have one', () => {
        expect(checkLock(manifest, lock)).toEqual([
          'node_modules/@scope/dep: has an install script, which is not expected',
        ]);
      });

      it('should accept the entry when the package is expected to have one', () => {
        expect(checkLock(manifest, lock, ['@scope/dep'])).toEqual([]);
      });
    });
  });

  describe('readMinimumReleaseAge', () => {
    describe('when the workspace file sets the age', () => {
      it('should return the minutes', () => {
        const yaml = 'packages:\n  - packages/*\nminimumReleaseAge: 4320 # three days\n';

        expect(readMinimumReleaseAge(yaml)).toBe(4320);
      });
    });

    describe('when the workspace file sets only the exclude list', () => {
      it('should return undefined', () => {
        const yaml = 'packages:\n  - packages/*\nminimumReleaseAgeExclude:\n  - tool\n';

        expect(readMinimumReleaseAge(yaml)).toBeUndefined();
      });
    });

    describe('when it reads the workspace file of this repository', () => {
      it('should return a positive number of minutes', async () => {
        const workspaceFile = new URL('../../../../pnpm-workspace.yaml', import.meta.url);

        expect(readMinimumReleaseAge(await fs.readFile(workspaceFile, 'utf8'))).toBeGreaterThan(0);
      });
    });
  });

  describe('registryTarball', () => {
    it('should give the tarball of a package without a scope', () => {
      expect(registryTarball('tool', '1.2.0')).toBe(`${REGISTRY}tool/-/tool-1.2.0.tgz`);
    });

    it('should leave the scope out of the file name of a scoped package', () => {
      expect(registryTarball('@scope/dep', '3.0.1')).toBe(`${REGISTRY}@scope/dep/-/dep-3.0.1.tgz`);
    });

    it.each([
      ['../tool', '1.2.0'],
      ['tool/lib', '1.2.0'],
      ['tool', '1.2'],
      ['tool', '1.2.0/x'],
      ['tool', undefined],
      [undefined, '1.2.0'],
    ])('should give nothing for %s@%s', (name, version) => {
      expect(registryTarball(name, version)).toBeUndefined();
    });
  });

  describe('fetchRegistryPackages', () => {
    describe('when the registry has every package', () => {
      it('should ask once for each name, with the name alone', async () => {
        const packuments = makePackuments('2026-01-01T00:00:00.000Z', '2026-02-01T00:00:00.000Z');
        const fetchImpl = registry(packuments);

        const packages = await fetchRegistryPackages(['tool', '@scope/dep', 'tool'], fetchImpl);

        expect(fetchImpl.mock.calls.map(call => call[0]).sort()).toEqual([
          `${REGISTRY}@scope%2Fdep`,
          `${REGISTRY}tool`,
        ]);
        expect(fetchImpl.mock.calls.every(call => call.length === 1)).toBe(true);
        expect(packages.get('@scope/dep')).toEqual(packuments['@scope/dep']);
      });
    });

    describe('when the registry does not have a package', () => {
      it('should fail', async () => {
        const fetchImpl = registry({ tool: {} });

        await expect(fetchRegistryPackages(['tool', '@scope/dep'], fetchImpl)).rejects.toThrow(
          '@scope/dep: the registry answered 404',
        );
      });
    });
  });

  describe('registryIntegrityProblems', () => {
    let packages: Map<string, Required<Packument>>;

    beforeEach(async () => {
      packages = await fetchRegistryPackages(
        ['tool', '@scope/dep'],
        registry(makePackuments('2026-01-01T00:00:00.000Z', '2026-02-01T00:00:00.000Z')),
      );
    });

    describe('when every integrity is the one the registry has for the release', () => {
      it('should report nothing', () => {
        expect(registryIntegrityProblems(makeLock(), packages)).toEqual([]);
      });
    });

    describe('when the registry has another integrity for a release', () => {
      it('should report the entry with both values', () => {
        packages.get('tool')!.versions['1.2.0'] = { dist: { integrity: sha512('other') } };

        expect(registryIntegrityProblems(makeLock(), packages)).toEqual([
          `tool@1.2.0: the lock has integrity ${sha512('tool')}, the registry has ${sha512('other')}`,
        ]);
      });
    });

    describe('when the registry has the integrity only for another version', () => {
      it('should report the entry', () => {
        packages.get('@scope/dep')!.versions = { '3.0.2': { dist: { integrity: sha512('dep') } } };

        expect(registryIntegrityProblems(makeLock(), packages)).toEqual([
          `@scope/dep@3.0.1: the lock has integrity ${sha512('dep')}, the registry has undefined`,
        ]);
      });
    });
  });

  describe('releaseAgeProblems', () => {
    let packages: Map<string, Required<Packument>>;

    beforeEach(async () => {
      packages = await fetchRegistryPackages(
        ['tool', '@scope/dep'],
        registry(
          makePackuments(
            new Date(NOW - 1441 * MINUTE).toISOString(),
            new Date(NOW - 1440 * MINUTE).toISOString(),
          ),
        ),
      );
    });

    describe('when every release is at least the minimum age old', () => {
      it('should report nothing', () => {
        expect(releaseAgeProblems(makeLock(), packages, 1440, NOW)).toEqual([]);
      });
    });

    describe('when a release is younger than the minimum age', () => {
      it('should report the entry with its publish time', () => {
        packages.get('@scope/dep')!.time = {
          '3.0.1': new Date(NOW - 1439 * MINUTE).toISOString(),
        };

        expect(releaseAgeProblems(makeLock(), packages, 1440, NOW)).toEqual([
          '@scope/dep@3.0.1: published 2026-06-09T00:01:00.000Z, less than 1440 minutes ago',
        ]);
      });
    });

    describe('when the registry has no time for the exact version', () => {
      it('should report the entry', () => {
        packages.get('tool')!.time = { '1.1.0': new Date(NOW - 9999 * MINUTE).toISOString() };

        expect(releaseAgeProblems(makeLock(), packages, 1440, NOW)).toEqual([
          'tool@1.2.0: the registry gives no publish time',
        ]);
      });
    });
  });

  describe('writeCheckedLock', () => {
    const COMMITTED = '{"committed":true}\n';
    let dir: string;
    let manifestFile: string;
    let lockFile: string;
    let lockText: string;
    let old: Record<string, Packument>;

    beforeEach(async () => {
      dir = await fs.mkdtemp(path.join(os.tmpdir(), 'optimizer-tools-lock-'));
      manifestFile = path.join(dir, 'package.json');
      lockFile = path.join(dir, 'package-lock.json');
      await fs.writeFile(manifestFile, JSON.stringify(makeManifest()));
      await fs.writeFile(lockFile, COMMITTED);
      lockText = `${JSON.stringify(makeLock(), null, 2)}\n`;
      old = makePackuments(
        new Date(NOW - 5000 * MINUTE).toISOString(),
        new Date(NOW - 5000 * MINUTE).toISOString(),
      );
    });

    afterEach(async () => {
      await fs.rm(dir, { recursive: true, force: true });
    });

    describe('when the new lock is clean and every release is old enough', () => {
      it('should write the new lock as it is', async () => {
        const fetchImpl = registry(old);

        await writeCheckedLock(lockText, {
          manifestFile,
          lockFile,
          minimumReleaseAge: 1440,
          fetchImpl,
          now: NOW,
        });

        expect(await fs.readFile(lockFile, 'utf8')).toBe(lockText);
      });
    });

    describe('when a release in the new lock is too recent', () => {
      it('should fail and keep the committed lock', async () => {
        old.tool.time = { '1.2.0': new Date(NOW - 10 * MINUTE).toISOString() };
        const fetchImpl = registry(old);

        await expect(
          writeCheckedLock(lockText, {
            manifestFile,
            lockFile,
            minimumReleaseAge: 1440,
            fetchImpl,
            now: NOW,
          }),
        ).rejects.toThrow(/not written:\ntool@1\.2\.0: published .* less than 1440 minutes ago/);
        expect(await fs.readFile(lockFile, 'utf8')).toBe(COMMITTED);
      });
    });

    describe('when the registry has another integrity for a release in the new lock', () => {
      it('should fail and keep the committed lock', async () => {
        old.tool.versions = { '1.2.0': { dist: { integrity: sha512('other') } } };
        const fetchImpl = registry(old);

        await expect(
          writeCheckedLock(lockText, {
            manifestFile,
            lockFile,
            minimumReleaseAge: 1440,
            fetchImpl,
            now: NOW,
          }),
        ).rejects.toThrow(
          /not written:\ntool@1\.2\.0: the lock has integrity .* the registry has /,
        );
        expect(await fs.readFile(lockFile, 'utf8')).toBe(COMMITTED);
      });
    });

    describe('when the new lock fails the check of the committed files', () => {
      it('should fail without asking the registry, and keep the committed lock', async () => {
        const fetchImpl = registry(old);
        const broken = makeLock();
        delete broken.packages['node_modules/tool'].integrity;

        await expect(
          writeCheckedLock(JSON.stringify(broken), {
            manifestFile,
            lockFile,
            minimumReleaseAge: 1440,
            fetchImpl,
            now: NOW,
          }),
        ).rejects.toThrow('node_modules/tool: no integrity');
        expect(fetchImpl).not.toHaveBeenCalled();
        expect(await fs.readFile(lockFile, 'utf8')).toBe(COMMITTED);
      });
    });
  });
});
