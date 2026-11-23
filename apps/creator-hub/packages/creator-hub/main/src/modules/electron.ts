import path from 'path';
import fs from 'fs/promises';
import {
  app,
  BrowserWindow,
  clipboard,
  dialog,
  type OpenDialogOptions,
  type SaveDialogOptions,
  shell,
} from 'electron';
import log from 'electron-log/main';
import { getDefaultScenesPath } from '/shared/paths';
import type { Env } from '/shared/types/env';
import type { DeploymentLogEntry, RendererErrorLogEntry } from '/shared/types/ipc';

// Store environment override from CLI arguments
let envOverride: Env | null = null;

export function setEnvOverride(value: Env | null) {
  envOverride = value;
}

export function getEnvOverride(): Env | null {
  return envOverride;
}

export function getHome() {
  return app.getPath('home');
}

/** Where new scenes go unless the user picks another folder. */
export function getDefaultScenesPathForUser() {
  return getDefaultScenesPath(getHome());
}

export function getUserDataPath() {
  return app.getPath('userData');
}

export function getAppDataPath() {
  return app.getPath('appData');
}

export function getAppHomeLegacy() {
  return path.join(getHome(), '.decentraland');
}

export async function getWorkspaceConfigPath(_path: string) {
  const editorHomePath = path.join(_path, '.editor');
  try {
    await fs.stat(_path);
  } catch (error) {
    await fs.mkdir(editorHomePath, { recursive: true });
  }
  return editorHomePath;
}

export async function showOpenDialog(opts: Partial<OpenDialogOptions>): Promise<string[]> {
  const window = BrowserWindow.getFocusedWindow();
  if (!window) return [];

  const { filePaths } = await dialog.showOpenDialog(window, opts);
  return filePaths;
}

export async function showSaveDialog(opts: Partial<SaveDialogOptions>): Promise<string | null> {
  const window = BrowserWindow.getFocusedWindow();
  if (!window) return null;

  const { canceled, filePath } = await dialog.showSaveDialog(window, opts);
  return canceled || !filePath ? null : filePath;
}

export async function openExternal(url: string) {
  shell.openExternal(url);
}

/**
 * Opens a `decentraland://` link in the installed desktop client. Resolves false when no app
 * handles the scheme: macOS and Linux then do nothing at all, so the caller can fall back to
 * the web.
 */
export async function openDeepLink(url: string): Promise<boolean> {
  if (!url.startsWith('decentraland://')) throw new Error('Not a decentraland:// link');
  if (!app.getApplicationNameForProtocol(url)) return false;
  await shell.openExternal(url);
  return true;
}

export async function getAppVersion() {
  return app.getVersion();
}

export async function copyToClipboard(text: string) {
  await clipboard.writeText(text);
}

// A dialog opened in one window by a press in another (the detached chat's Record button) is
// otherwise behind the window the user is looking at.
export async function focusWindow(sender: Electron.WebContents) {
  const window = BrowserWindow.fromWebContents(sender);
  if (!window || window.isDestroyed()) return;
  if (window.isMinimized()) window.restore();
  window.focus();
}

export async function logDeployment(entry: DeploymentLogEntry): Promise<void> {
  if (entry.outcome === 'published') {
    log.info(
      `[Deploy] Published "${entry.title}" to ${entry.target} at ${entry.baseParcel}: ${entry.rootCID}`,
    );
  } else {
    log.warn(`[Deploy] Publish failed: ${entry.error}`);
  }
}

/** Enough for a stack and a component stack; a renderer can't flood the log through it. */
const RENDERER_ERROR_MAX_LENGTH = 8000;

const cap = (text: string) =>
  text.length > RENDERER_ERROR_MAX_LENGTH
    ? `${text.slice(0, RENDERER_ERROR_MAX_LENGTH)}… [${text.length - RENDERER_ERROR_MAX_LENGTH} more characters]`
    : text;

export async function logRendererError(entry: RendererErrorLogEntry): Promise<void> {
  const boundary = cap(String(entry.boundary));
  const error = cap(String(entry.error));
  const componentStack = entry.componentStack && cap(String(entry.componentStack));
  log.error(`[ErrorBoundary:${boundary}] ${error}${componentStack ? `\n${componentStack}` : ''}`);
}
