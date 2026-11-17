import { execFile } from 'child_process';
import fs from 'fs';
import os from 'os';
import path from 'path';
import log from 'electron-log/main';

/** The desktop entry that the AppImage writes for itself. The deb installs `creator-hub.desktop`. */
export const APPIMAGE_DESKTOP_ENTRY = 'creator-hub-appimage.desktop';

/**
 * A desktop entry that starts the AppImage at `appImagePath` with a `scheme://` link.
 * The desktop entry specification, "The Exec key", gives the two levels of escapes: the reserved
 * characters of a quoted argument, then the backslashes of a string value.
 */
export function appImageDesktopEntry(appImagePath: string, scheme: string): string {
  const exec = appImagePath
    .replace(/(["`$\\])/g, '\\$1')
    .replace(/\\/g, '\\\\')
    .replace(/%/g, '%%');
  return [
    '[Desktop Entry]',
    'Type=Application',
    'Name=Decentraland Creator Hub',
    `Exec="${exec}" %U`,
    'Terminal=false',
    // The entry is there to open links, not to add the app to the menu.
    'NoDisplay=true',
    `MimeType=x-scheme-handler/${scheme};`,
    '',
  ].join('\n');
}

/**
 * The path of the AppImage that this process runs from, if it runs from one. A process that an
 * AppImage starts gets the `APPIMAGE` variable of that AppImage, so the variable alone can name
 * another app: the app must also run from the folder where the AppImage is mounted.
 */
export function runningAppImage(env: NodeJS.ProcessEnv, execPath: string): string | undefined {
  const { APPIMAGE, APPDIR } = env;
  if (!APPIMAGE || !APPDIR) return undefined;
  return execPath.startsWith(APPDIR.replace(/\/+$/, '') + '/') ? APPIMAGE : undefined;
}

/**
 * Makes the running AppImage the handler of `scheme://` links. An AppImage is one file with no
 * install step, so no desktop entry tells the desktop which links it opens, and without one the
 * sign-in link from the browser does not come back to the app. The user can move the file, so
 * each start writes the entry again. Another installed handler (the deb) stays the default.
 */
export function registerAppImageProtocolClient(appImagePath: string, scheme: string): void {
  // A line break ends the Exec line, and the remaining text becomes other keys of the entry.
  if (/[\r\n]/.test(appImagePath)) {
    log.error('[Deeplink] The AppImage path has a line break, no link handler');
    return;
  }
  const applications = path.join(dataHome(), 'applications');
  try {
    fs.mkdirSync(applications, { recursive: true });
    fs.writeFileSync(
      path.join(applications, APPIMAGE_DESKTOP_ENTRY),
      appImageDesktopEntry(appImagePath, scheme),
    );
  } catch (error) {
    log.error('[Deeplink] Could not write the desktop entry of the AppImage:', error);
    return;
  }
  const mimeType = `x-scheme-handler/${scheme}`;
  execFile('xdg-mime', ['query', 'default', mimeType], (queryError, stdout) => {
    const current = queryError ? '' : String(stdout).trim();
    if (current && current !== APPIMAGE_DESKTOP_ENTRY && desktopEntryExists(current)) {
      log.info(`[Deeplink] ${current} already opens ${scheme} links, keeping it`);
      return;
    }
    execFile('xdg-mime', ['default', APPIMAGE_DESKTOP_ENTRY, mimeType], error => {
      if (error) log.error('[Deeplink] Could not make the AppImage the link handler:', error);
    });
  });
}

function dataHome(): string {
  return process.env.XDG_DATA_HOME || path.join(os.homedir(), '.local', 'share');
}

/**
 * True when a desktop entry with this file name is installed. The desktop keeps a default after
 * its entry is gone (an uninstalled deb), and links then open nothing.
 */
function desktopEntryExists(entry: string): boolean {
  const dataDirs = (process.env.XDG_DATA_DIRS || '/usr/local/share:/usr/share').split(':');
  return [dataHome(), ...dataDirs]
    .filter(Boolean)
    .some(dir => fs.existsSync(path.join(dir, 'applications', entry)));
}
