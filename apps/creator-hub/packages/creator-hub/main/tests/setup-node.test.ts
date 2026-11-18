import fs from 'fs';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({ warn: vi.fn() }));

vi.mock('os', async importOriginal => ({
  ...(await importOriginal<Record<string, unknown>>()),
  platform: () => 'linux',
}));
vi.mock('electron', () => ({ app: { getAppPath: () => '/opt/hub/resources/app.asar' } }));
vi.mock('../src/modules/cmd-shim', () => ({ writeCmdShim: vi.fn(() => Promise.resolve()) }));
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
    vi.spyOn(fs, 'lstatSync').mockImplementation(() => {
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

  it('keeps a link that already starts this app, and still puts its folder on PATH', async () => {
    vi.mocked(fs.lstatSync).mockReturnValue({ isSymbolicLink: () => true } as fs.Stats);
    vi.spyOn(fs, 'readlinkSync').mockReturnValue(process.execPath);
    const { setupNodeBinary } = await loadModule();

    setupNodeBinary();

    expect(symlink).not.toHaveBeenCalled();
    expect(process.env.PATH?.startsWith('/opt/hub/resources/app.asar.unpacked:')).toBe(true);
  });

  it('replaces a link to another app', async () => {
    vi.mocked(fs.lstatSync).mockReturnValue({ isSymbolicLink: () => true } as fs.Stats);
    vi.spyOn(fs, 'readlinkSync').mockReturnValue('/old/creator-hub');
    const rm = vi.spyOn(fs, 'rmSync').mockImplementation(() => undefined);
    const { setupNodeBinary } = await loadModule();

    setupNodeBinary();

    expect(rm).toHaveBeenCalledWith(LINK);
    expect(symlink).toHaveBeenCalledWith(process.execPath, LINK);
  });

  it('does not fail when a link to another app cannot be removed from a read-only folder', async () => {
    vi.mocked(fs.lstatSync).mockReturnValue({ isSymbolicLink: () => true } as fs.Stats);
    vi.spyOn(fs, 'readlinkSync').mockReturnValue('/old/creator-hub');
    vi.spyOn(fs, 'rmSync').mockImplementation(() => {
      throw fsError('EACCES');
    });
    const { setupNodeBinary, getInjectedPathEntries } = await loadModule();

    expect(() => setupNodeBinary()).not.toThrow();

    expect(symlink).not.toHaveBeenCalled();
    expect(getInjectedPathEntries()).toEqual([]);
  });

  it('fails on any other link error', async () => {
    symlink.mockImplementation(() => {
      throw fsError('ENOSPC');
    });
    const { setupNodeBinary } = await loadModule();

    expect(() => setupNodeBinary()).toThrow('ENOSPC');
  });
});
