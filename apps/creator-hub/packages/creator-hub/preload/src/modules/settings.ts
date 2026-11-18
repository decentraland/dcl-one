import path from 'path';
import { ipcRenderer, type IpcRendererEvent } from 'electron';
import {
  DEPENDENCY_UPDATE_STRATEGY,
  DEFAULT_DEPENDENCY_UPDATE_STRATEGY,
} from '/shared/types/settings';
import type { AppSettings } from '/shared/types/settings';
import { watchlistFor, withWatchlist } from '/shared/types/analytics-watchlist';
import { SCENES_DIRECTORY } from '/shared/paths';

import { invoke } from '../services/ipc';
import { getConfig, setConfig } from '../services/config';

export async function getDefaultScenesPath() {
  const userDataPath = await invoke('electron.getUserDataPath');
  return path.join(userDataPath, SCENES_DIRECTORY);
}

export async function getScenesPath() {
  const config = await getConfig();
  return config.settings?.scenesPath || (await getDefaultScenesPath());
}

export async function isCustomScenesPath(currentPath: string): Promise<boolean> {
  const defaultPath = await getDefaultScenesPath();
  return currentPath !== defaultPath;
}

export function isValidUpdateStrategy(value?: string): value is DEPENDENCY_UPDATE_STRATEGY {
  return Object.values(DEPENDENCY_UPDATE_STRATEGY).includes(value as DEPENDENCY_UPDATE_STRATEGY);
}

export async function getUpdateDependenciesStrategy() {
  const { dependencyUpdateStrategy } = (await getConfig()).settings;
  if (isValidUpdateStrategy(dependencyUpdateStrategy)) return dependencyUpdateStrategy;
  return DEFAULT_DEPENDENCY_UPDATE_STRATEGY;
}

/**
 * Writes the app settings, except the AI sharing keys and the Analytics watchlist: those are
 * written only by `updateAiSharingSettings` and `setAnalyticsWatchlist`, so a screen holding an
 * older copy of the settings can't bring back a decision or pins the creator has since changed.
 */
export async function updateAppSettings(settings: AppSettings) {
  await setConfig(config => {
    const { aiDataSharing, aiSharingPending, analyticsWatchlist } = config.settings ?? {};
    config.settings = { ...settings, aiDataSharing, aiSharingPending, analyticsWatchlist };
  });
}

/** The places `address` pinned to the Analytics watchlist; none when nothing valid is stored. */
export async function getAnalyticsWatchlist(address: string): Promise<string[]> {
  const config = await getConfig();
  return watchlistFor(config.settings?.analyticsWatchlist, address);
}

/** Replaces `address`'s Analytics watchlist, leaving every other wallet's and setting as stored. */
export async function setAnalyticsWatchlist(address: string, placeIds: string[]): Promise<void> {
  await setConfig(config => {
    config.settings = {
      ...config.settings,
      analyticsWatchlist: withWatchlist(config.settings?.analyticsWatchlist, address, placeIds),
    };
  });
}

export type AiSharingSettings = Pick<AppSettings, 'aiDataSharing' | 'aiSharingPending'>;

/** Writes the AI sharing keys of the settings, and nothing else. */
export async function updateAiSharingSettings(sharing: AiSharingSettings) {
  await setConfig(config => {
    config.settings = { ...config.settings, ...sharing };
  });
}

export async function selectSceneFolder(): Promise<string | undefined> {
  const [projectPath] = await invoke('electron.showOpenDialog', {
    title: 'Select Scenes Folder',
    properties: ['openDirectory', 'createDirectory', 'promptToCreate'],
  });

  return projectPath;
}

export async function selectEditorPath(): Promise<string[]> {
  return invoke('electron.showOpenDialog', {
    title: 'Select Code Editor',
    properties: ['openFile'],
  });
}

export async function checkForUpdates(config?: { autoDownload?: boolean }) {
  return await invoke('updater.checkForUpdates', config);
}

export async function getDownloadedVersion() {
  return await invoke('updater.getDownloadedVersion');
}

export async function quitAndInstall(version: string) {
  return await invoke('updater.quitAndInstall', version);
}

export async function downloadUpdate() {
  return await invoke('updater.downloadUpdate');
}

export async function setupUpdaterEvents() {
  return await invoke('updater.setupUpdaterEvents');
}

export async function getInstalledVersion() {
  return await invoke('updater.getInstalledVersion');
}

export async function deleteVersionFile() {
  return await invoke('updater.deleteVersionFile');
}

export function downloadingStatus(
  cb: (
    event: IpcRendererEvent,
    progress: {
      percent: number;
      finished: boolean;
      version: string | null;
      isDownloading: boolean;
      error?: string;
    },
  ) => void,
) {
  ipcRenderer.on('updater.downloadProgress', cb);
  return {
    cleanup: () => {
      ipcRenderer.off('updater.downloadProgress', cb);
    },
  };
}
export function getCurrentVersion() {
  throw new Error('Function not implemented.');
}

export async function getEditors() {
  return invoke('code.getEditors');
}

export async function addEditor(editorPath: string) {
  return await invoke('code.addEditor', editorPath);
}

export async function setDefaultEditor(editorPath: string) {
  return await invoke('code.setDefaultEditor', editorPath);
}

export async function removeEditor(editorPath: string) {
  return await invoke('code.removeEditor', editorPath);
}

export async function getReleaseNotes(version: string) {
  return await invoke('updater.getReleaseNotes', version);
}
