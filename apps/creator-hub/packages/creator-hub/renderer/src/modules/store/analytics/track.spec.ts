import { describe, expect, it } from 'vitest';

import type { AppSettings } from '/shared/types/settings';
import { DeploymentError } from '/@/lib/deploy';
import { getInitialDeploymentStatus } from '../deployment/utils';

import { actions as deploymentActions } from '../deployment';
import { actions as editorActions } from '../editor';
import { actions as workspaceActions } from '../workspace';
import { actions as aiSharingActions } from '../aiSharing';
import { analyticsConfig, changedSettings } from './track';

const publishScene = analyticsConfig[editorActions.publishScene.fulfilled.type];
const getState = (() => ({ editor: { project: { id: 'project-1' } } })) as never;
const payloadFor = (arg: { target?: string; targetContent?: string }) =>
  publishScene.getPayload({ type: '', payload: undefined, meta: { arg } } as never, getState);

describe('Publish Scene analytics', () => {
  describe('when the scene is published to a World', () => {
    it('should report the target as worlds', () => {
      expect(payloadFor({ targetContent: 'https://worlds.example.org' })).toEqual({
        project_id: 'project-1',
        target: 'worlds',
        targetContent: 'https://worlds.example.org',
      });
    });
  });

  describe('when the scene is published to LAND', () => {
    it('should report the catalyst it was sent to', () => {
      expect(payloadFor({ target: 'https://peer.example.org' })).toMatchObject({
        target: 'https://peer.example.org',
        targetContent: 'unknown',
      });
    });
  });
});

const payload = (type: string, action: object, state: object) =>
  analyticsConfig[type].getPayload({ type, ...action } as never, (() => state) as never);

describe('Publish Scene Failed analytics', () => {
  it('should report the destination and the error without its file paths', () => {
    expect(
      payload(
        editorActions.publishScene.rejected.type,
        {
          meta: { arg: { targetContent: 'https://worlds.example.org' } },
          error: { message: 'ENOENT: no such file /Users/jane/scenes/park/scene.json' },
        },
        { editor: { project: { id: 'project-1' } } },
      ),
    ).toEqual({
      project_id: 'project-1',
      target: 'worlds',
      targetContent: 'https://worlds.example.org',
      error: 'ENOENT: no such file <path>',
    });
  });
});

describe('Execute Scene Deployment Failed analytics', () => {
  const state = (executeRequestId = 'run-1') => ({
    workspace: { projects: [{ id: 'project-1', path: '/scenes/park' }] },
    editor: {},
    deployment: {
      deployments: { '/scenes/park': { info: { isWorld: true }, executeRequestId } },
    },
  });
  const rejected = (action: object, executeRequestId?: string) =>
    payload(
      deploymentActions.executeDeployment.rejected.type,
      { meta: { arg: '/scenes/park', requestId: 'run-1' }, ...action },
      state(executeRequestId),
    );
  const rejectedWithValue = (error: DeploymentError) =>
    rejected({
      payload: error,
      error: { message: 'Rejected' },
      meta: {
        arg: '/scenes/park',
        requestId: 'run-1',
        requestStatus: 'rejected',
        rejectedWithValue: true,
      },
    });

  describe('when the run threw', () => {
    it('should report the error name and message', () => {
      expect(
        rejected({ error: { name: 'DEPLOY_REQUEST_TIMEOUT', message: 'Deployment timed out' } }),
      ).toEqual({
        project_id: 'project-1',
        is_world: true,
        error_name: 'DEPLOY_REQUEST_TIMEOUT',
        error: 'Deployment timed out',
      });
    });
  });

  describe('when the run failed through rejectWithValue', () => {
    it('should report the deployment error instead of Rejected', () => {
      expect(
        rejectedWithValue(new DeploymentError('DEPLOYMENT_FAILED', getInitialDeploymentStatus())),
      ).toEqual({
        project_id: 'project-1',
        is_world: true,
        error_name: 'DEPLOYMENT_FAILED',
        error: 'Deployment error: DEPLOYMENT_FAILED',
      });
    });
  });

  describe('when the creator cancelled the run', () => {
    it('should send nothing', () => {
      expect(
        rejectedWithValue(
          new DeploymentError('DEPLOYMENT_CANCELLED', getInitialDeploymentStatus()),
        ),
      ).toBeNull();
      expect(rejected({ error: { name: 'DEPLOYMENT_CANCELLED' } })).toBeNull();
    });
  });

  describe('when a newer publish replaced the run', () => {
    it('should send nothing', () => {
      expect(rejected({ error: { name: 'DEPLOYMENT_FAILED' } }, 'run-2')).toBeNull();
    });
  });
});

describe('Import Project analytics', () => {
  describe('when the creator closes the folder picker', () => {
    it('should send no event', () => {
      expect(
        payload(workspaceActions.importProject.fulfilled.type, { payload: undefined }, {}),
      ).toBeNull();
    });
  });

  describe('when a scene is imported', () => {
    it('should report the imported project', () => {
      expect(
        payload(
          workspaceActions.importProject.fulfilled.type,
          { payload: { id: 'project-2' } },
          {},
        ),
      ).toEqual({ project_id: 'project-2' });
    });
  });
});

describe('Delete Project analytics', () => {
  it('should report the project and whether its files were deleted', () => {
    expect(
      payload(
        workspaceActions.deleteProject.fulfilled.type,
        { meta: { arg: { path: '/scenes/park', shouldDeleteFiles: true } } },
        { workspace: { projects: [{ id: 'project-1', path: '/scenes/park' }] }, editor: {} },
      ),
    ).toEqual({ project_id: 'project-1', delete_files: true });
  });
});

describe('App Settings Changed analytics', () => {
  const previous = {
    scenesPath: '/Users/jane/scenes',
    previewOptions: { debugger: false, openNewInstance: false },
    experimental: false,
    aiDataSharing: {},
  } as unknown as AppSettings;

  it('should name the changed settings, nested ones by their path, without values', () => {
    expect(
      changedSettings(previous, {
        ...previous,
        scenesPath: '/Users/jane/other',
        previewOptions: { ...previous.previewOptions, debugger: true },
      }),
    ).toEqual(['previewOptions.debugger', 'scenesPath']);
  });

  it('should leave out the AI sharing settings, which hold wallet addresses', () => {
    expect(
      changedSettings(previous, {
        ...previous,
        aiDataSharing: { '0xabc': { consent: 'granted' } },
      } as unknown as AppSettings),
    ).toEqual([]);
  });

  describe('when nothing changed', () => {
    it('should send no event', () => {
      expect(
        payload(
          workspaceActions.updateSettings.fulfilled.type,
          { meta: { arg: previous } },
          { workspace: { settings: previous } },
        ),
      ).toBeNull();
    });
  });
});

describe('AI Sharing Decided analytics', () => {
  const decided = (saved: boolean) =>
    payload(aiSharingActions.decide.fulfilled.type, { payload: saved, meta: { arg: true } }, {});

  describe('when the decision was saved', () => {
    it('should report it', () => {
      expect(decided(true)).toEqual({ granted: true });
    });
  });

  describe('when the creator was signed out and nothing was saved', () => {
    it('should send nothing', () => {
      expect(decided(false)).toBeNull();
    });
  });
});
