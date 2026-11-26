import type { SaveDialogOptions } from 'electron';

import type { DeploymentLogEntry, RendererErrorLogEntry } from '/shared/types/ipc';
import { isUrl } from '/shared/utils';

import { invoke } from '../services/ipc';

export async function fetchAuthIdentity(url: string, id: string, verifier: string) {
  return invoke('electron.fetchAuthIdentity', url, id, verifier);
}

export async function openExternal(url: string) {
  if (!isUrl(url)) throw new Error('Invalid URL provided');
  await invoke('electron.openExternal', url);
}

/** Opens a `decentraland://` link in the desktop client; false when it isn't installed. */
export async function openDeepLink(url: string) {
  return invoke('electron.openDeepLink', url);
}

export async function showSaveDialog(opts: Partial<SaveDialogOptions>) {
  return invoke('electron.showSaveDialog', opts);
}

export async function copyToClipboard(text: string) {
  await invoke('electron.copyToClipboard', text);
}

// Bring this window to the front.
export async function focusWindow() {
  await invoke('electron.focusWindow');
}

export async function logDeployment(entry: DeploymentLogEntry) {
  await invoke('electron.logDeployment', entry);
}

export async function logRendererError(entry: RendererErrorLogEntry) {
  await invoke('electron.logRendererError', entry);
}
