import { beforeEach, describe, expect, it } from 'vitest';

import {
  DEFAULT_CONFIG,
  dropCompressAssetsSettings,
  dropGuiEditorSetting,
  mergeConfig,
  promoteAiAssistantSetting,
  promoteScenesPath,
} from '../types/config';
import type { Config } from '../types/config';
import type { AppSettings } from '../types/settings';

type LegacySettings = Partial<AppSettings> & { guiEditor?: boolean };
type StoredSettings = Partial<AppSettings>;

describe('dropGuiEditorSetting', () => {
  describe('when the settings carry the legacy guiEditor key', () => {
    it('should remove the key when it is false', () => {
      const settings: LegacySettings = { guiEditor: false };
      dropGuiEditorSetting(settings);
      expect('guiEditor' in settings).toBe(false);
    });

    it('should remove the key when it is true', () => {
      const settings: LegacySettings = { guiEditor: true };
      dropGuiEditorSetting(settings);
      expect('guiEditor' in settings).toBe(false);
    });

    it('should leave every other setting untouched', () => {
      const settings: LegacySettings = {
        guiEditor: true,
        experimental: true,
        aiAssistant: false,
        exposeMcpServer: true,
      };
      dropGuiEditorSetting(settings);
      expect(settings).toEqual({
        experimental: true,
        aiAssistant: false,
        exposeMcpServer: true,
      });
    });
  });

  describe('when the settings never had the key', () => {
    it('should leave them unchanged', () => {
      const settings: LegacySettings = { experimental: false };
      dropGuiEditorSetting(settings);
      expect('guiEditor' in settings).toBe(false);
      expect(settings).toEqual({ experimental: false });
    });
  });

  describe('when the settings are undefined', () => {
    it('should not throw', () => {
      expect(() => dropGuiEditorSetting(undefined)).not.toThrow();
    });
  });
});

describe('dropCompressAssetsSettings', () => {
  describe('when the settings carry the retired Compress Assets keys', () => {
    it('should remove them and leave every other setting untouched', () => {
      const settings = {
        experimental: true,
        optimizedAssetsByPath: { '/tmp/scene': true },
        previewOptions: { debugger: true, optimizedAssets: true },
      } as unknown as StoredSettings;

      dropCompressAssetsSettings(settings);

      expect(settings).toEqual({ experimental: true, previewOptions: { debugger: true } });
    });
  });

  describe('when the settings have no preview options', () => {
    it('should still remove the per-project map', () => {
      const settings = { optimizedAssetsByPath: {} } as unknown as StoredSettings;

      dropCompressAssetsSettings(settings);

      expect(settings).toEqual({});
    });
  });

  describe('when the settings are undefined', () => {
    it('should not throw', () => {
      expect(() => dropCompressAssetsSettings(undefined)).not.toThrow();
    });
  });
});

describe('promoteAiAssistantSetting', () => {
  describe('when the stored settings predate the promotion', () => {
    it('should turn the assistant on and record that it has run', () => {
      const settings: StoredSettings = { aiAssistant: false };

      promoteAiAssistantSetting(settings);

      expect(settings).toEqual({ aiAssistant: true, aiAssistantPromoted: true });
    });

    it('should record that it has run even when the assistant was already on', () => {
      const settings: StoredSettings = { aiAssistant: true };

      promoteAiAssistantSetting(settings);

      expect(settings).toEqual({ aiAssistant: true, aiAssistantPromoted: true });
    });

    it('should turn the assistant on when the key is absent entirely', () => {
      const settings: StoredSettings = { experimental: false };

      promoteAiAssistantSetting(settings);

      expect(settings.aiAssistant).toBe(true);
      expect(settings.aiAssistantPromoted).toBe(true);
    });

    it('should leave every other setting untouched', () => {
      const settings: StoredSettings = {
        aiAssistant: false,
        experimental: true,
        exposeMcpServer: true,
        useApiKeyFromEnv: true,
      };

      promoteAiAssistantSetting(settings);

      expect(settings).toEqual({
        aiAssistant: true,
        aiAssistantPromoted: true,
        experimental: true,
        exposeMcpServer: true,
        useApiKeyFromEnv: true,
      });
    });
  });

  describe('when the promotion has already run', () => {
    it('should respect an assistant the creator turned back off', () => {
      const settings: StoredSettings = { aiAssistant: false, aiAssistantPromoted: true };

      promoteAiAssistantSetting(settings);

      expect(settings.aiAssistant).toBe(false);
    });

    it('should leave an enabled assistant alone', () => {
      const settings: StoredSettings = { aiAssistant: true, aiAssistantPromoted: true };

      promoteAiAssistantSetting(settings);

      expect(settings).toEqual({ aiAssistant: true, aiAssistantPromoted: true });
    });
  });

  describe('when there are no settings at all', () => {
    it('should not throw', () => {
      expect(() => promoteAiAssistantSetting(undefined)).not.toThrow();
    });
  });
});

describe('DEFAULT_CONFIG', () => {
  it('should keep AI agents outside Creator Hub opt-in', () => {
    expect(DEFAULT_CONFIG.settings.exposeMcpServer).toBe(false);
  });
});

describe('mergeConfig', () => {
  let defaults: Config;

  beforeEach(() => {
    defaults = structuredClone(DEFAULT_CONFIG);
    defaults.settings.scenesPath = '/home/creator/scenes';
    defaults.workspace.paths = ['/default/scene'];
  });

  describe('when the stored config has only some of the keys', () => {
    let merged: Config;

    beforeEach(() => {
      merged = mergeConfig(
        {
          workspace: { paths: ['/scenes/a', '/scenes/b'] },
          settings: { previewOptions: { debugger: false } },
        } as unknown as Partial<Config>,
        defaults,
      );
    });

    it('should keep the stored values', () => {
      expect(merged.settings.previewOptions.debugger).toBe(false);
    });

    it('should take the other values from the defaults, at each depth', () => {
      expect(merged.version).toBe(defaults.version);
      expect(merged.settings.scenesPath).toBe('/home/creator/scenes');
      expect(merged.settings.previewOptions.skipAuthScreen).toBe(true);
    });

    it('should replace a default array with the stored array, not join them', () => {
      expect(merged.workspace.paths).toEqual(['/scenes/a', '/scenes/b']);
    });

    it('should not share an object with the defaults', () => {
      merged.settings.previewOptions.showWarnings = false;

      expect(defaults.settings.previewOptions.showWarnings).toBe(true);
    });
  });

  describe('when the stored config has keys that the defaults do not have', () => {
    it('should keep them after the keys of the defaults', () => {
      const merged = mergeConfig({ userId: 'user', version: 1 }, defaults);

      expect(merged.userId).toBe('user');
      expect(Object.keys(merged)).toEqual([...Object.keys(defaults), 'userId']);
    });
  });

  describe('when a stored value is of another type than the default', () => {
    it('should keep the stored value', () => {
      const merged = mergeConfig(
        { editors: null, workspace: [] } as unknown as Partial<Config>,
        defaults,
      );

      expect(merged.editors).toBeNull();
      expect(merged.workspace).toEqual([]);
    });
  });

  describe('when the stored config has a __proto__ key', () => {
    it('should not change the prototype of the result', () => {
      const stored = JSON.parse(
        '{"__proto__":{"polluted":true},"settings":{"__proto__":{"polluted":true}}}',
      );

      const merged = mergeConfig(stored, defaults) as Config & { polluted?: boolean };

      expect(merged.polluted).toBeUndefined();
      expect((merged.settings as AppSettings & { polluted?: boolean }).polluted).toBeUndefined();
      expect(Object.getPrototypeOf(merged)).toBe(Object.prototype);
    });
  });
});

describe('promoteScenesPath', () => {
  const legacyPath = '/user/data/Scenes';
  const defaultPath = '/home/me/Decentraland/Scenes';

  describe('when the install still uses the hidden app data folder and it holds no scenes', () => {
    it.each([legacyPath, `${legacyPath}/`, ''])(
      'should move "%s" to the visible default and record that it has run',
      scenesPath => {
        const settings: StoredSettings = { scenesPath: scenesPath || defaultPath };

        promoteScenesPath(settings, {
          storedPath: scenesPath,
          legacyPath,
          defaultPath,
          legacyHasScenes: false,
        });

        expect(settings).toEqual({ scenesPath: defaultPath, scenesPathPromoted: true });
      },
    );
  });

  describe('when the hidden app data folder holds scenes', () => {
    it('should keep the folder and record that it has run', () => {
      const settings: StoredSettings = { scenesPath: legacyPath };

      promoteScenesPath(settings, {
        storedPath: legacyPath,
        legacyPath,
        defaultPath,
        legacyHasScenes: true,
      });

      expect(settings).toEqual({ scenesPath: legacyPath, scenesPathPromoted: true });
    });

    it('should keep it when the stored config names no folder', () => {
      // The merge has already filled in the new default.
      const settings: StoredSettings = { scenesPath: defaultPath };

      promoteScenesPath(settings, {
        storedPath: undefined,
        legacyPath,
        defaultPath,
        legacyHasScenes: true,
      });

      expect(settings).toEqual({ scenesPath: legacyPath, scenesPathPromoted: true });
    });
  });

  describe('when the user chose another folder', () => {
    it('should keep it and record that it has run', () => {
      const settings: StoredSettings = { scenesPath: '/projects/dcl' };

      promoteScenesPath(settings, {
        storedPath: '/projects/dcl',
        legacyPath,
        defaultPath,
        legacyHasScenes: false,
      });

      expect(settings).toEqual({ scenesPath: '/projects/dcl', scenesPathPromoted: true });
    });
  });

  describe('when it has already run', () => {
    it('should leave a later choice of the old folder alone', () => {
      const settings: StoredSettings = { scenesPath: legacyPath, scenesPathPromoted: true };

      promoteScenesPath(settings, {
        storedPath: legacyPath,
        legacyPath,
        defaultPath,
        legacyHasScenes: false,
      });

      expect(settings).toEqual({ scenesPath: legacyPath, scenesPathPromoted: true });
    });
  });
});
