import os from 'node:os';
import { parse } from 'semver';
import type * as ElectronUpdater from 'electron-updater';
import { afterEach, describe, expect, it, vi } from 'vitest';

// @ts-expect-error The hook is plain JavaScript without types.
import { setMacUpdateMinimum } from '../../scripts/after-pack.js';

afterEach(() => vi.restoreAllMocks());

describe('setMacUpdateMinimum', () => {
  it('writes the Darwin version of mac.minimumSystemVersion into the release info', () => {
    const packager = {
      platformSpecificBuildOptions: {
        minimumSystemVersion: '13.0',
        releaseInfo: { releaseName: 'x' },
      },
    };
    setMacUpdateMinimum(packager);
    expect(packager.platformSpecificBuildOptions.releaseInfo).toEqual({
      releaseName: 'x',
      minimumSystemVersion: '22.0.0',
    });
  });

  it('stops the build for a minimum it has no Darwin version for', () => {
    expect(() =>
      setMacUpdateMinimum({ platformSpecificBuildOptions: { minimumSystemVersion: '99.0' } }),
    ).toThrow(/99\.0/);
  });

  it('leaves the release info alone without a minimum', () => {
    const packager = { platformSpecificBuildOptions: {} };
    setMacUpdateMinimum(packager);
    expect(packager.platformSpecificBuildOptions).toEqual({});
  });

  it('makes electron-updater refuse the update on macOS 12 and accept it on macOS 13', async () => {
    const { AppUpdater } = await vi.importActual<typeof ElectronUpdater>('electron-updater');
    const packager = {
      platformSpecificBuildOptions: { minimumSystemVersion: '13.0' } as Record<string, any>,
    };
    setMacUpdateMinimum(packager);
    const updateInfo = {
      version: '2.0.0',
      files: [],
      path: '',
      sha512: '',
      releaseDate: '',
      ...packager.platformSpecificBuildOptions.releaseInfo,
    };
    const self: any = {
      currentVersion: parse('1.0.0'),
      allowDowngrade: false,
      isUserWithinRollout: () => true,
      _logger: { info: () => {}, warn: () => {} },
    };
    self.isUpdateSupported = (info: unknown) =>
      (AppUpdater.prototype as any).checkIfUpdateSupported.call(self, info);
    const available = (AppUpdater.prototype as any).isUpdateAvailable.bind(self);
    vi.spyOn(os, 'release').mockReturnValue('21.6.0');
    await expect(available(updateInfo)).resolves.toBe(false);
    vi.spyOn(os, 'release').mockReturnValue('22.6.0');
    await expect(available(updateInfo)).resolves.toBe(true);
  });
});
