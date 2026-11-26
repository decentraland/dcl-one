import path from 'node:path';

export const SCENES_DIRECTORY = 'Scenes';
export const SETTINGS_DIRECTORY = 'Settings';
export const CUSTOM_ASSETS_DIRECTORY = 'Custom Items';
export const CONFIG_FILE_NAME = 'config.json';
export const INSTALLED_VERSION_FILE_NAME = 'installed-version.json';

/** The folder in the user's home that holds the default scenes folder. */
export const SCENES_HOME_DIRECTORY = 'Decentraland';

/**
 * Where new scenes go unless the user picks another folder: a folder in their home, which every
 * OS's file manager shows. Not Documents: iCloud and OneDrive often sync it, and syncing a
 * scene's `node_modules` is slow and conflict-prone.
 */
export function getDefaultScenesPath(homePath: string): string {
  return path.join(homePath, SCENES_HOME_DIRECTORY, SCENES_DIRECTORY);
}

/**
 * The default before `getDefaultScenesPath`: the app data folder, which every OS's file manager
 * hides (AppData, ~/Library, ~/.config) and which "remove app data" deletes.
 */
export function getLegacyScenesPath(userDataPath: string): string {
  return path.join(userDataPath, SCENES_DIRECTORY);
}

/**
 * Whether `target` is `base` itself or sits beneath it.
 *
 * Uses `path.relative` rather than a string prefix, because `startsWith` reports
 * `/project-other` as being inside `/project`. Both arguments are resolved first, so `..`
 * segments are collapsed before the comparison and a relative `target` is taken against the
 * process cwd.
 */
export function isInside(base: string, target: string): boolean {
  const relative = path.relative(path.resolve(base), path.resolve(target));
  return !relative.startsWith('..') && !path.isAbsolute(relative);
}
