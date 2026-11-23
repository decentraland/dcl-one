import type { SerializedError } from '@reduxjs/toolkit';
import type { Events } from '/shared/types/analytics';
import type { Project } from '/shared/types/projects';
import type { DeployOptions } from '/shared/types/deploy';
import type { AppSettings } from '/shared/types/settings';
import type { OptimizeResult, OptimizeScanResult } from '/shared/types/optimizer';
import type { WorldSettings } from '/@/lib/worlds';
import type { AddressPermissionPayload, ParcelsPermissionPayload } from '../management/types';
import { type GetState } from '#store';

import { actions as workspaceActions } from '../workspace';
import { actions as editorActions } from '../editor';
import { actions as deploymentActions } from '../deployment';
import { actions as managementActions } from '../management';
import { actions as optimizerActions } from '../optimizer';
import { actions as aiSharingActions } from '../aiSharing';
import { describeError, projectIdForPath } from './describe';

type ActionWithPayload<P, M = void> = {
  type: string;
  payload: P;
  meta: { arg: M };
};

type RejectedAction<M = void> = {
  type: string;
  error: SerializedError;
  meta: { arg: M };
};

type AnalyticsHandler<T, E extends keyof Events = keyof Events> = {
  eventName: E;
  /** Returns null when the action is not worth an event. */
  getPayload: (action: T, getState: GetState) => Events[E] | null;
};

// Written by their own actions (`updateAiSharingSettings`, `setAnalyticsWatchlist`), and they
// hold wallet addresses.
const UNTRACKED_SETTINGS = new Set<string>([
  'aiDataSharing',
  'aiSharingPending',
  'analyticsWatchlist',
]);

/** The names of the settings that differ, one level deep into objects such as `previewOptions`. */
export function changedSettings(previous: AppSettings, next: AppSettings): string[] {
  const changed: string[] = [];
  const keys = new Set([...Object.keys(previous), ...Object.keys(next)]);
  for (const key of keys) {
    if (UNTRACKED_SETTINGS.has(key)) continue;
    const before = (previous as Record<string, unknown>)[key];
    const after = (next as Record<string, unknown>)[key];
    if (JSON.stringify(before) === JSON.stringify(after)) continue;
    const isObject = (value: unknown) =>
      typeof value === 'object' && value !== null && !Array.isArray(value);
    if (isObject(before) && isObject(after)) {
      const a = before as Record<string, unknown>;
      const b = after as Record<string, unknown>;
      for (const sub of new Set([...Object.keys(a), ...Object.keys(b)])) {
        if (JSON.stringify(a[sub]) !== JSON.stringify(b[sub])) changed.push(`${key}.${sub}`);
      }
    } else {
      changed.push(key);
    }
  }
  return changed.sort();
}

export const analyticsConfig: Record<string, AnalyticsHandler<any>> = {
  [workspaceActions.createProject.fulfilled.type]: {
    eventName: 'Create Project',
    getPayload: (action: ActionWithPayload<Project>): Events['Create Project'] => ({
      project_id: action.payload.id,
      project_name: action.payload.title,
      template: action.payload.description || '',
      rows: action.payload.layout.rows,
      cols: action.payload.layout.cols,
    }),
  },
  [workspaceActions.runProject.fulfilled.type]: {
    eventName: 'Open Project',
    getPayload: (action: ActionWithPayload<Project>): Events['Open Project'] => ({
      project_id: action.payload.id,
      project_name: action.payload.title,
    }),
  },
  [workspaceActions.updateProject.type]: {
    eventName: 'Save Project Success',
    getPayload: (action: ActionWithPayload<Project>): Events['Save Project Success'] => ({
      project_id: action.payload.id,
      project_name: action.payload.title,
    }),
  },
  [editorActions.runScene.pending.type]: {
    eventName: 'Preview Scene',
    getPayload: (_action: ActionWithPayload<void>, getState: GetState): Events['Preview Scene'] => {
      const projectId = getState().editor.project?.id;
      if (!projectId) throw new Error('No project ID found when trying to preview scene');
      return {
        project_id: projectId,
      };
    },
  },
  [editorActions.publishScene.fulfilled.type]: {
    eventName: 'Publish Scene',
    getPayload: (
      action: ActionWithPayload<void, DeployOptions>,
      getState: GetState,
    ): Events['Publish Scene'] => {
      const projectId = getState().editor.project?.id;
      if (!projectId) throw new Error('No project ID found when trying to publish scene');
      return {
        project_id: projectId,
        // A world publish names no target, only the Worlds content server it deploys to.
        target: action.meta.arg.target || (action.meta.arg.targetContent ? 'worlds' : 'unknown'),
        targetContent: action.meta.arg.targetContent || 'unknown',
      };
    },
  },
  [deploymentActions.executeDeployment.pending.type]: {
    eventName: 'Execute Scene Deployment',
    getPayload: (
      action: ActionWithPayload<void, string>,
      getState: GetState,
    ): Events['Execute Scene Deployment'] => {
      const state = getState();
      const path = action.meta.arg;
      const deployment = state.deployment.deployments[path];
      const project = state.editor.project;
      const worldScenes =
        project?.worldConfiguration?.name === state.management.worldSettings.worldName
          ? state.management.worldSettings.scenes
          : [];

      if (!project?.id || !deployment) {
        throw new Error('Missing project or deployment info when trying to execute deployment');
      }

      return {
        project_id: project.id,
        scene_size: deployment.files.reduce((sum, f) => sum + f.size, 0),
        file_count: deployment.files.length,
        base_parcel: deployment.info.baseParcel,
        parcels: deployment.info.parcels,
        is_world: deployment.info.isWorld,
        world_name: deployment.info.isWorld ? project?.worldConfiguration?.name : undefined,
        existing_world_scenes: worldScenes.map(scene => ({
          entity_id: scene.entityId,
          parcels: scene.parcels,
          size: Number(scene.size),
        })),
      };
    },
  },
  [managementActions.addAddressPermission.fulfilled.type]: {
    eventName: 'Add World Permissions',
    getPayload: (
      action: ActionWithPayload<void, AddressPermissionPayload>,
    ): Events['Add World Permissions'] => ({
      world_name: action.meta.arg.worldName,
      wallet_address: action.meta.arg.walletAddress,
      permission_name: action.meta.arg.permissionName,
    }),
  },
  [managementActions.removeAddressPermission.fulfilled.type]: {
    eventName: 'Remove World Permissions',
    getPayload: (
      action: ActionWithPayload<void, AddressPermissionPayload>,
    ): Events['Remove World Permissions'] => ({
      world_name: action.meta.arg.worldName,
      wallet_address: action.meta.arg.walletAddress,
      permission_name: action.meta.arg.permissionName,
    }),
  },
  [managementActions.addParcelsPermission.fulfilled.type]: {
    eventName: 'Add Collaborator Parcel Permissions',
    getPayload: (
      action: ActionWithPayload<void, ParcelsPermissionPayload>,
    ): Events['Add Collaborator Parcel Permissions'] => ({
      world_name: action.meta.arg.worldName,
      wallet_address: action.meta.arg.walletAddress,
      permission_name: action.meta.arg.permissionName,
      parcels_count: action.meta.arg.parcels.length,
    }),
  },
  [managementActions.removeParcelsPermission.fulfilled.type]: {
    eventName: 'Remove Collaborator Parcel Permissions',
    getPayload: (
      action: ActionWithPayload<void, ParcelsPermissionPayload>,
    ): Events['Remove Collaborator Parcel Permissions'] => ({
      world_name: action.meta.arg.worldName,
      wallet_address: action.meta.arg.walletAddress,
      permission_name: action.meta.arg.permissionName,
      parcels_count: action.meta.arg.parcels.length,
    }),
  },
  [managementActions.updateWorldSettings.fulfilled.type]: {
    eventName: 'Update World Settings',
    getPayload: (
      action: ActionWithPayload<void, { worldName: string; worldSettings: Partial<WorldSettings> }>,
      getState: GetState,
    ): Events['Update World Settings'] => {
      const { worldName, worldSettings: changedSettings } = action.meta.arg;
      const state = getState();
      const previousSettings =
        state.management.worldSettings.worldName === worldName
          ? state.management.worldSettings.settings
          : {};

      return {
        world_name: worldName,
        world_settings: { ...previousSettings, ...changedSettings, thumbnail: undefined }, // Thumbnail is omitted as it can be a large base64 string
        changed_fields: {
          title: changedSettings.title !== undefined,
          description: changedSettings.description !== undefined,
          thumbnail: changedSettings.thumbnail !== undefined,
          content_rating: changedSettings.contentRating !== undefined,
          categories: changedSettings.categories !== undefined,
        },
        scenes_list:
          state.management.worldSettings.worldName === worldName
            ? state.management.worldSettings.scenes.map(scene => ({
                entityId: scene.entityId,
                deployer: scene.deployer,
                parcels_count: scene.parcels?.length || 0,
                size: scene.size,
                created_at: scene.createdAt,
              }))
            : [],
      };
    },
  },
  [managementActions.unpublishWorldScene.fulfilled.type]: {
    eventName: 'Unpublish World Scene',
    getPayload: (
      action: ActionWithPayload<void, { worldName: string; sceneCoord: string }>,
    ): Events['Unpublish World Scene'] => ({
      world_name: action.meta.arg.worldName,
      scene_coord: action.meta.arg.sceneCoord,
    }),
  },
  [managementActions.unpublishEntireWorld.fulfilled.type]: {
    eventName: 'Unpublish Entire World',
    getPayload: (
      action: ActionWithPayload<void, { worldName: string }>,
    ): Events['Unpublish Entire World'] => ({
      world_name: action.meta.arg.worldName,
    }),
  },
  [editorActions.publishScene.rejected.type]: {
    eventName: 'Publish Scene Failed',
    getPayload: (
      action: RejectedAction<DeployOptions>,
      getState: GetState,
    ): Events['Publish Scene Failed'] => ({
      project_id: getState().editor.project?.id ?? null,
      target: action.meta.arg.target || (action.meta.arg.targetContent ? 'worlds' : 'unknown'),
      targetContent: action.meta.arg.targetContent || 'unknown',
      error: describeError(action.error),
    }),
  },
  [deploymentActions.executeDeployment.rejected.type]: {
    eventName: 'Execute Scene Deployment Failed',
    getPayload: (
      action: RejectedAction<string>,
      getState: GetState,
    ): Events['Execute Scene Deployment Failed'] => {
      const state = getState();
      const deployment = state.deployment.deployments[action.meta.arg];
      return {
        project_id: projectIdForPath(state, action.meta.arg),
        is_world: deployment?.info.isWorld ?? null,
        error: describeError(action.error),
      };
    },
  },
  [editorActions.runScene.rejected.type]: {
    eventName: 'Preview Scene Failed',
    getPayload: (
      action: RejectedAction<{ path: string; mobile?: boolean }>,
      getState: GetState,
    ): Events['Preview Scene Failed'] => ({
      project_id: projectIdForPath(getState(), action.meta.arg.path),
      mobile: !!action.meta.arg.mobile,
      error: describeError(action.error),
    }),
  },
  [editorActions.getMobileQR.fulfilled.type]: {
    eventName: 'Preview On Mobile',
    getPayload: (
      action: ActionWithPayload<unknown, { path: string }>,
      getState: GetState,
    ): Events['Preview On Mobile'] => ({
      project_id: projectIdForPath(getState(), action.meta.arg.path),
    }),
  },
  [editorActions.openTutorial.fulfilled.type]: {
    eventName: 'Open Tutorial',
    getPayload: (
      action: ActionWithPayload<void, { id: string; title?: string }>,
    ): Events['Open Tutorial'] => ({
      video_id: action.meta.arg.id,
      title: action.meta.arg.title ?? '',
    }),
  },
  [workspaceActions.deleteProject.fulfilled.type]: {
    eventName: 'Delete Project',
    getPayload: (
      action: ActionWithPayload<void, { path: string; shouldDeleteFiles?: boolean }>,
      getState: GetState,
    ): Events['Delete Project'] => ({
      project_id: projectIdForPath(getState(), action.meta.arg.path),
      delete_files: !!action.meta.arg.shouldDeleteFiles,
    }),
  },
  [workspaceActions.duplicateProject.fulfilled.type]: {
    eventName: 'Duplicate Project',
    getPayload: (
      action: ActionWithPayload<Project, string>,
      getState: GetState,
    ): Events['Duplicate Project'] => ({
      project_id: action.payload.id,
      source_project_id: projectIdForPath(getState(), action.meta.arg),
    }),
  },
  [workspaceActions.renameProject.fulfilled.type]: {
    eventName: 'Rename Project',
    getPayload: (action: ActionWithPayload<Project>): Events['Rename Project'] => ({
      project_id: action.payload.id,
    }),
  },
  [workspaceActions.importProject.fulfilled.type]: {
    eventName: 'Import Project',
    // No project: the creator closed the folder picker.
    getPayload: (
      action: ActionWithPayload<Project | undefined>,
    ): Events['Import Project'] | null =>
      action.payload ? { project_id: action.payload.id } : null,
  },
  [workspaceActions.updateSettings.fulfilled.type]: {
    eventName: 'App Settings Changed',
    getPayload: (
      action: ActionWithPayload<void, AppSettings>,
      getState: GetState,
    ): Events['App Settings Changed'] | null => {
      const changed = changedSettings(getState().workspace.settings, action.meta.arg);
      return changed.length > 0 ? { changed } : null;
    },
  },
  [optimizerActions.scanProject.fulfilled.type]: {
    eventName: 'Optimize Scan',
    getPayload: (
      action: ActionWithPayload<{ path: string; scan: OptimizeScanResult }, string>,
      getState: GetState,
    ): Events['Optimize Scan'] => ({
      project_id: projectIdForPath(getState(), action.meta.arg),
      glb_count: action.payload.scan.glbCount,
      total_bytes: action.payload.scan.totalBytes,
      has_backup: action.payload.scan.hasBackup,
    }),
  },
  [optimizerActions.runOptimize.fulfilled.type]: {
    eventName: 'Optimize Scene',
    getPayload: (
      action: ActionWithPayload<{ result: OptimizeResult }, { path: string }>,
      getState: GetState,
    ): Events['Optimize Scene'] => {
      const { result } = action.payload;
      return {
        project_id: projectIdForPath(getState(), action.meta.arg.path),
        glbs_processed: result.glbsProcessed,
        glbs_changed: result.glbsChanged,
        glbs_failed: result.files.filter($ => $.status === 'failed').length,
        bytes_before: result.bytesBefore,
        bytes_after: result.bytesAfter,
      };
    },
  },
  [optimizerActions.runOptimize.rejected.type]: {
    eventName: 'Optimize Scene Failed',
    getPayload: (
      action: RejectedAction<{ path: string }>,
      getState: GetState,
    ): Events['Optimize Scene Failed'] => ({
      project_id: projectIdForPath(getState(), action.meta.arg.path),
      error: describeError(action.error),
    }),
  },
  [optimizerActions.revertProject.fulfilled.type]: {
    eventName: 'Optimize Revert',
    getPayload: (
      action: ActionWithPayload<unknown, string>,
      getState: GetState,
    ): Events['Optimize Revert'] => ({
      project_id: projectIdForPath(getState(), action.meta.arg),
    }),
  },
  [optimizerActions.installTools.fulfilled.type]: {
    eventName: 'Optimize Tools Installed',
    getPayload: (
      action: ActionWithPayload<unknown, string>,
      getState: GetState,
    ): Events['Optimize Tools Installed'] => ({
      project_id: projectIdForPath(getState(), action.meta.arg),
    }),
  },
  [optimizerActions.installTools.rejected.type]: {
    eventName: 'Optimize Tools Install Failed',
    getPayload: (
      action: RejectedAction<string>,
      getState: GetState,
    ): Events['Optimize Tools Install Failed'] => ({
      project_id: projectIdForPath(getState(), action.meta.arg),
      error: describeError(action.error),
    }),
  },
  [aiSharingActions.decide.fulfilled.type]: {
    eventName: 'AI Sharing Decided',
    getPayload: (action: ActionWithPayload<void, boolean>): Events['AI Sharing Decided'] => ({
      granted: action.meta.arg,
    }),
  },
  [aiSharingActions.deleteEverything.fulfilled.type]: {
    eventName: 'AI Sharing Data Deleted',
    getPayload: (): Events['AI Sharing Data Deleted'] => undefined,
  },
};
