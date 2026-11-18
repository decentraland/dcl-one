import { createHash } from 'node:crypto';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { npmRun } from '../../scripts/lock-optimizer-tools.js';
import {
  MANIFEST_FILE,
  PLATFORMS,
  checkLocks,
  lockFile,
  platformManifest,
  writeCheckedPlatformLock,
} from '../../scripts/lock-recording-tools.js';

vi.mock('electron', () => ({
  app: { getPath: () => '/tmp', getVersion: () => '1.0.0' },
}));
vi.mock('electron-log/main', () => ({ default: { info: vi.fn(), warn: vi.fn(), error: vi.fn() } }));
vi.mock('../src/modules/bin', () => ({ run: vi.fn() }));

const REGISTRY = 'https://registry.npmjs.org/';
const MINUTE = 60_000;
const NOW = Date.parse('2026-06-10T00:00:00.000Z');
const WHISPER = '@fugood/whisper.node';

function sha512(content: string) {
  return `sha512-${createHash('sha512').update(content).digest('base64')}`;
}

function readManifest() {
  return fs.readFile(MANIFEST_FILE, 'utf8').then(text => JSON.parse(text));
}

type Entry = { version?: string; resolved?: string; integrity?: string };

function makeLock(binding: string) {
  return {
    lockfileVersion: 3,
    packages: {
      '': { dependencies: { [binding]: '1.1.3', [WHISPER]: '1.1.3' } },
      [`node_modules/${WHISPER}`]: {
        version: '1.1.3',
        resolved: `${REGISTRY}${WHISPER}/-/whisper.node-1.1.3.tgz`,
        integrity: sha512('whisper'),
        hasInstallScript: true,
      },
      [`node_modules/${binding}`]: {
        version: '1.1.3',
        resolved: `${REGISTRY}${binding}/-/${binding.slice(binding.indexOf('/') + 1)}-1.1.3.tgz`,
        integrity: sha512(binding),
      },
    } as Record<string, Entry>,
  };
}

function registry(published: string, integrities: Record<string, string>) {
  return vi.fn(async (url: string) => {
    const name = decodeURIComponent(url.slice(REGISTRY.length));
    return {
      ok: name in integrities,
      status: name in integrities ? 200 : 404,
      json: async () => ({
        time: { '1.1.3': published },
        versions: { '1.1.3': { dist: { integrity: integrities[name] } } },
      }),
    };
  });
}

describe('the speech-to-text locks', () => {
  describe('when every committed lock is checked against the pinned package.json', () => {
    it('should have no problem', async () => {
      const base = await readManifest();

      expect(Object.keys(base.dependencies)).toContain(WHISPER);
      expect(checkLocks(base)).toEqual([]);
    });

    it('should cover one lock per platform the app ships to', async () => {
      await Promise.all(
        PLATFORMS.map(([platform, arch]: [string, string]) =>
          expect(fs.access(lockFile(platform, arch))).resolves.toBeUndefined(),
        ),
      );
    });
  });

  describe('when a committed lock does not answer for its pins', () => {
    it('should name the platform and the problem', async () => {
      const base = await readManifest();
      const broken = makeLock('@fugood/node-whisper-darwin-arm64');
      delete broken.packages[`node_modules/${WHISPER}`].integrity;

      expect(checkLocks(base, () => broken)).toContain(
        `darwin-arm64: node_modules/${WHISPER}: no integrity`,
      );
    });
  });

  describe('when the manifest of a platform is built', () => {
    it('should be the one the app installs there', async () => {
      const base = await readManifest();
      const tools = await import('../src/modules/recording/tools');

      expect(
        `${JSON.stringify(platformManifest(base, process.platform, process.arch), null, 2)}\n`,
      ).toBe(tools.platformManifest());
    });

    it('should make this platform binding a direct dependency at the pinned version', () => {
      const base = { dependencies: { [WHISPER]: '1.1.3' } };

      expect(platformManifest(base, 'win32', 'x64').dependencies).toEqual({
        [WHISPER]: '1.1.3',
        '@fugood/node-whisper-win32-x64': '1.1.3',
      });
    });
  });

  describe('when a new lock is written', () => {
    const binding = '@fugood/node-whisper-linux-x64';
    const COMMITTED = '{"committed":true}\n';
    let dir: string;
    let lockPath: string;
    let lockText: string;
    let manifest: { dependencies: Record<string, string> };
    let integrities: Record<string, string>;

    beforeEach(async () => {
      dir = await fs.mkdtemp(path.join(os.tmpdir(), 'recording-tools-lock-'));
      lockPath = path.join(dir, 'package-lock.json');
      await fs.writeFile(lockPath, COMMITTED);
      manifest = platformManifest({ dependencies: { [WHISPER]: '1.1.3' } }, 'linux', 'x64');
      lockText = `${JSON.stringify(makeLock(binding), null, 2)}\n`;
      integrities = { [WHISPER]: sha512('whisper'), [binding]: sha512(binding) };
    });

    afterEach(async () => {
      await fs.rm(dir, { recursive: true, force: true });
    });

    function write(options: { published?: string; fetchImpl?: typeof fetch } = {}) {
      return writeCheckedPlatformLock(lockText, {
        manifest,
        platform: 'linux',
        arch: 'x64',
        lockFile: lockPath,
        minimumReleaseAge: 1440,
        fetchImpl:
          options.fetchImpl ??
          registry(options.published ?? new Date(NOW - 5000 * MINUTE).toISOString(), integrities),
        now: NOW,
      });
    }

    it('should accept the wrapper install script the app never runs', async () => {
      await write();

      expect(await fs.readFile(lockPath, 'utf8')).toBe(lockText);
    });

    it('should refuse a release that is younger than the workspace allows', async () => {
      await expect(write({ published: new Date(NOW - 10 * MINUTE).toISOString() })).rejects.toThrow(
        /not written:\n.*less than 1440 minutes ago/s,
      );
      expect(await fs.readFile(lockPath, 'utf8')).toBe(COMMITTED);
    });

    it('should refuse an integrity the registry does not publish', async () => {
      const fetchImpl = registry(new Date(NOW - 5000 * MINUTE).toISOString(), {
        ...integrities,
        [binding]: sha512('other'),
      });

      await expect(write({ fetchImpl: fetchImpl as unknown as typeof fetch })).rejects.toThrow(
        /not written:\n.*the registry has /s,
      );
      expect(await fs.readFile(lockPath, 'utf8')).toBe(COMMITTED);
    });
  });

  describe('when npm is started to make a lock', () => {
    const asPlatform = (platform: string) =>
      Object.defineProperty(process, 'platform', { value: platform, configurable: true });
    const real = process.platform;

    afterEach(() => asPlatform(real));

    it('should start the binary directly everywhere but Windows', () => {
      asPlatform('darwin');

      expect(npmRun(['install', '--no-fund'])).toEqual({
        command: 'npm',
        args: ['install', '--no-fund'],
        options: {},
      });
    });

    it('should start the Windows shim through a shell, since execFile cannot', () => {
      asPlatform('win32');

      expect(npmRun(['install', '--no-fund'])).toEqual({
        command: 'npm.cmd',
        args: ['install', '--no-fund'],
        options: { shell: true },
      });
    });

    it('should quote an argument the shell would otherwise read as its own', () => {
      asPlatform('win32');

      expect(npmRun(['--prefix=C:\\Program Files\\dcl', 'a&b', 'plain']).args).toEqual([
        '"--prefix=C:\\Program Files\\dcl"',
        '"a&b"',
        'plain',
      ]);
    });
  });
});
