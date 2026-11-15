import fs from 'fs';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({ warn: vi.fn() }));

vi.mock('os', async importOriginal => ({
  ...(await importOriginal<Record<string, unknown>>()),
  platform: () => 'linux',
}));
vi.mock('electron', () => ({ app: { getAppPath: () => '/opt/hub/resources/app.asar' } }));
vi.mock('cmd-shim', () => ({ default: vi.fn() }));
vi.mock('electron-log/main', () => ({
  default: { info: vi.fn(), warn: mocks.warn, error: vi.fn() },
}));
vi.mock('../src/modules/path', () => ({
  getBinPath: () => '/opt/hub/resources/app.asar.unpacked/node_modules/npm/bin/npm-cli.js',
}));

const LINK = '/opt/hub/resources/app.asar.unpacked/node';

function fsError(code: string): NodeJS.ErrnoException {
  return Object.assign(new Error(code), { code });
}

async function loadModule() {
  vi.resetModules();
  return import('../src/modules/setup-node');
}

describe('setupNodeBinary in a packaged Linux app', () => {
  let symlink: ReturnType<typeof vi.spyOn>;
  const path = process.env.PATH;

  beforeEach(() => {
    vi.stubEnv('DEV', false);
    vi.stubEnv('TEST', '');
    process.env.PATH = '/usr/bin';
    vi.spyOn(fs, 'statSync').mockImplementation(() => {
      throw fsError('ENOENT');
    });
    symlink = vi.spyOn(fs, 'symlinkSync').mockImplementation(() => undefined);
  });

  afterEach(() => {
    process.env.PATH = path;
    vi.unstubAllEnvs();
    vi.restoreAllMocks();
    mocks.warn.mockClear();
  });

  it('links node in the app folder and puts the folder first on PATH', async () => {
    const { setupNodeBinary, getInjectedPathEntries } = await loadModule();

    setupNodeBinary();

    expect(symlink).toHaveBeenCalledWith(process.execPath, LINK);
    expect(getInjectedPathEntries()).toEqual([
      '/opt/hub/resources/app.asar.unpacked',
      '/opt/hub/resources/app.asar.unpacked/node_modules/npm/bin',
    ]);
    expect(process.env.PATH?.startsWith('/opt/hub/resources/app.asar.unpacked:')).toBe(true);
  });

  it.each(['EROFS', 'EACCES', 'EPERM'])(
    'does not fail when the app folder gives %s, and adds no shim folder to PATH',
    async code => {
      symlink.mockImplementation(() => {
        throw fsError(code);
      });
      const { setupNodeBinary, getInjectedPathEntries } = await loadModule();

      expect(() => setupNodeBinary()).not.toThrow();

      expect(getInjectedPathEntries()).toEqual([]);
      expect(process.env.PATH).toBe('/usr/bin:/usr/local/bin');
      expect(mocks.warn).toHaveBeenCalledWith(expect.stringContaining(LINK));
    },
  );

  it('fails on any other link error', async () => {
    symlink.mockImplementation(() => {
      throw fsError('ENOSPC');
    });
    const { setupNodeBinary } = await loadModule();

    expect(() => setupNodeBinary()).toThrow('ENOSPC');
  });
});
