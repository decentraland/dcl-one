import { type AppSettings } from './settings';
import {
  DEFAULT_DEPENDENCY_UPDATE_STRATEGY,
  DEFAULT_PREVIEW_CLIENT,
  DEFAULT_RENDERER,
} from './settings';

export const CURRENT_CONFIG_VERSION = 2;

export type EditorConfig = {
  name: string;
  path: string;
  isDefault?: boolean;
  hidden?: boolean;
};

export type Config = {
  version: number;
  workspace: {
    paths: string[];
  };
  settings: AppSettings;
  userId?: string;
  editors?: EditorConfig[];
  installedAt?: string;
  lastVersion?: string;
};

export const DEFAULT_CONFIG: Config = {
  version: CURRENT_CONFIG_VERSION,
  workspace: {
    paths: [],
  },
  settings: {
    scenesPath: '', // Will be set with userDataPath + SCENES_DIRECTORY by main/preload
    dependencyUpdateStrategy: DEFAULT_DEPENDENCY_UPDATE_STRATEGY,
    previewOptions: {
      debugger: true,
      skipAuthScreen: true,
      enableLandscapeTerrains: true,
      openNewInstance: false,
      multiInstance: false,
      mcp: false,
      showWarnings: true,
      loadRawGltfs: false,
      client: DEFAULT_PREVIEW_CLIENT,
    },
    experimental: false,
    renderer: DEFAULT_RENDERER,
    aiAssistant: true,
    exposeMcpServer: false,
    useApiKeyFromEnv: false,
    useDevAssetCatalog: false,
  },
  editors: [],
};

function isPlainObject(value: unknown): value is Record<string, unknown> {
  if (typeof value !== 'object' || value === null) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

/**
 * Gives a new object with the stored values on top of the defaults. Objects merge key by key.
 * Any other stored value replaces the default as it is: an array is not joined with the
 * default array, and an empty string or `null` stays.
 */
function mergeOver(defaults: unknown, stored: unknown): unknown {
  if (!isPlainObject(stored)) return stored;
  const base = isPlainObject(defaults) ? defaults : {};
  const merged: Record<string, unknown> = {};
  for (const key of Object.keys(base)) merged[key] = mergeOver(undefined, base[key]);
  for (const key of Object.keys(stored)) {
    // A key such as `__proto__` comes from the prototype of `base`: writing it would change
    // the prototype of the result.
    if (key in base && !Object.hasOwn(base, key)) continue;
    const value = stored[key];
    // A stored scenes path that is not text gives way to the default path.
    const useDefault = key === 'scenesPath' && typeof value === 'object' && value !== null;
    merged[key] = (useDefault && base[key]) || mergeOver(base[key], value);
  }
  return merged;
}

export function mergeConfig(target: Partial<Config>, source: Config): Config {
  return mergeOver(source, target) as Config;
}

export function dropGuiEditorSetting(settings: Partial<AppSettings> | undefined): void {
  if (settings) {
    delete (settings as Record<string, unknown>).guiEditor;
  }
}

/** Removes the retired "Compress Assets" keys that stored configs still carry, in place. */
export function dropCompressAssetsSettings(settings: Partial<AppSettings> | undefined): void {
  if (!settings) return;
  delete (settings as Record<string, unknown>).optimizedAssetsByPath;
  if (settings.previewOptions) {
    delete (settings.previewOptions as Record<string, unknown>).optimizedAssets;
  }
}

/** Enables the AI assistant once, in place, on merged settings not yet promoted. */
export function promoteAiAssistantSetting(settings: Partial<AppSettings> | undefined): void {
  if (settings && !settings.aiAssistantPromoted) {
    settings.aiAssistant = true;
    settings.aiAssistantPromoted = true;
  }
}

/** Enables the debug console once, in place, on merged settings not yet promoted — the
 * toolbar's "Open Debug Console" checkbox was removed, so this is now the only way an
 * existing install gets it turned on. */
export function promoteDebugConsoleSetting(settings: Partial<AppSettings> | undefined): void {
  if (settings && settings.previewOptions && !settings.debugConsolePromoted) {
    settings.previewOptions.debugger = true;
    settings.debugConsolePromoted = true;
  }
}
