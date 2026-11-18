import path from 'path';
import fs from 'fs';
import { platform } from 'os';
import { app } from 'electron';
import log from 'electron-log/main';
import { writeCmdShim } from './cmd-shim';
import { getBinPath, getNodeCmdPath } from './path';

// Where releases up to 0.56 put the `node` link: inside the app bundle.
const LEGACY_NODE_LINK = path.join(app.getAppPath(), '../app.asar.unpacked', 'node');

/**
 * Removes the `node` link an earlier release left inside the bundle, so the installed app passes
 * its signature check again. Only a link to an executable is removed; failing to is harmless.
 */
function removeLegacyNodeLink() {
  if (platform() === 'win32') return;
  try {
    if (fs.lstatSync(LEGACY_NODE_LINK).isSymbolicLink()) fs.rmSync(LEGACY_NODE_LINK);
  } catch {
    /* not there, or the bundle is read-only */
  }
}

function joinEnvPaths(...paths: string[]) {
  return paths.filter(Boolean).join(path.delimiter);
}

/**
 * The PATH entries this module prepends to the app's own environment. Both hold shims that
 * only work inside Creator Hub: `node` is a link to the Electron executable, and the npm
 * scripts next to it resolve `node` from PATH and so land on that same link.
 *
 * Anything the Hub launches for the user — an external editor, and every terminal opened
 * inside it — must drop these, or `node`/`npm` in that terminal re-launch Creator Hub
 * instead of running (the single-instance lock then just refocuses the open window).
 */
let injectedPathEntries: string[] = [];

export function getInjectedPathEntries(): string[] {
  return injectedPathEntries;
}

/**
 * True when the shim folder can't be written (a locked-down home folder, say).
 */
function isReadOnlyInstall(error: unknown): boolean {
  const code = (error as NodeJS.ErrnoException).code;
  return code === 'EROFS' || code === 'EACCES' || code === 'EPERM';
}

/**
 * Makes `nodeCmdPath` start `nodeBinPath`, replacing any other file there. False when the app
 * folder is read-only: child processes then run on the bundled Node (node-runtime.ts), which
 * needs no link. `lstat`, because `stat` follows the link and never reports one.
 */
function linkNode(nodeCmdPath: string, nodeBinPath: string): boolean {
  let existing: fs.Stats | undefined;
  try {
    existing = fs.lstatSync(nodeCmdPath);
  } catch {
    existing = undefined;
  }
  if (existing?.isSymbolicLink() && fs.readlinkSync(nodeCmdPath) === nodeBinPath) {
    log.info('[Install] Node binaries already installed');
    return true;
  }

  log.info(`[Install] Installing node bin linking from ${nodeCmdPath} to ${nodeBinPath}`);
  try {
    fs.mkdirSync(path.dirname(nodeCmdPath), { recursive: true });
    if (existing) fs.rmSync(nodeCmdPath);
    if (platform() === 'win32') {
      // writeCmdShim adds the .cmd extension itself
      writeCmdShim(nodeBinPath, nodeCmdPath.replace(/\.cmd$/, '')).catch(error =>
        log.error('[Install] Could not write the node launchers:', error),
      );
    } else {
      fs.symlinkSync(nodeBinPath, nodeCmdPath);
    }
    return true;
  } catch (error) {
    if (!isReadOnlyInstall(error)) throw error;
    log.warn(`[Install] The node shim folder is read-only, no node link: ${nodeCmdPath}`);
    return false;
  }
}

export function setupNodeBinary() {
  // Only run in production mode
  if (import.meta.env.DEV || import.meta.env.TEST) {
    return;
  }

  removeLegacyNodeLink();
  const nodeCmdPath = getNodeCmdPath();
  const nodeBinPath = process.execPath;
  const npmBinPath = getBinPath('npm', 'npm');

  if (linkNode(nodeCmdPath, nodeBinPath)) {
    // Update PATH environment variable
    injectedPathEntries = [path.dirname(nodeCmdPath), path.dirname(npmBinPath)];
    process.env.PATH = joinEnvPaths(...injectedPathEntries, process.env.PATH || '');
  }

  if (platform() !== 'win32') {
    // on unix systems we need to install the path to the local bin folder for the Open in VSCode feature to work
    process.env.PATH = joinEnvPaths(process.env.PATH || '', '/usr/local/bin');
  }

  log.info('[Install] node command:', nodeCmdPath);
  log.info('[Install] node bin:', nodeBinPath);
  log.info('[Install] npm bin:', npmBinPath);
  log.info('[Install] $PATH', process.env.PATH);
}
