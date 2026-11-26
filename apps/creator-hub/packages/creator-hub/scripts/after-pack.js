import fs from 'node:fs';
import path from 'node:path';

/**
 * node-pty's `spawn-helper` (macOS/Linux) is exec'd by every pty spawn, but its 1.1.0 prebuild
 * ships it as 0644 and electron-builder copies that mode verbatim — so the packaged app can't
 * sign anyone in ("posix_spawnp failed"). Runtime code re-adds the exec bit too, but that needs
 * a writable install; do it here so the shipped bits are already correct. Runs after files are
 * assembled and before code signing, so the mode change is sealed by the signature.
 */
// os.release() gives the Darwin version on macOS, so macOS 13.0 is 22.0.0. Add a line here for a new
// mac.minimumSystemVersion in electron-builder.cjs.
const DARWIN_OF_MACOS = { '13.0': '22.0.0' };

/**
 * An installed app takes an update only when electron-updater accepts the `minimumSystemVersion` of
 * latest-mac.yml. It compares that value with os.release(), a Darwin version, and electron-builder does
 * not write the value. Without it, an app on macOS 12 installs a release that macOS then refuses to open.
 */
export function setMacUpdateMinimum(packager) {
  const options = packager.platformSpecificBuildOptions;
  const macos = options.minimumSystemVersion;
  if (macos == null) return;
  const darwin = DARWIN_OF_MACOS[macos];
  if (darwin == null)
    throw new Error(`after-pack.js: no Darwin version for mac.minimumSystemVersion ${macos}`);
  options.releaseInfo = { ...options.releaseInfo, minimumSystemVersion: darwin };
}

export default async function afterPack(context) {
  if (context.electronPlatformName === 'darwin') setMacUpdateMinimum(context.packager);
  if (context.electronPlatformName === 'win32') return;

  const found = [];
  const walk = dir => {
    let entries;
    try {
      entries = fs.readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.name === 'spawn-helper') found.push(full);
    }
  };
  walk(context.appOutDir);

  for (const helper of found) {
    try {
      const { mode } = fs.statSync(helper);
      if ((mode & 0o111) === 0) {
        fs.chmodSync(helper, mode | 0o111);
        console.log(`  Made node-pty spawn-helper executable: ${helper}`);
      }
    } catch (error) {
      console.warn(`  Could not chmod ${helper}: ${error.message}`);
    }
  }
}
