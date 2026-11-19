import type { ReactNode } from 'react';
import { Provider } from 'react-redux';
import { renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { Project } from '/shared/types/projects';
import type { RPCInfo } from '/@/modules/rpc';

import { createTestStore, type TestStore } from '../../tests/utils/testStore';
import { actions as workspaceActions } from '../modules/store/workspace';
import { useEditor } from './useEditor';

vi.mock('#store', async () => {
  const { useDispatch, useSelector } = await import('react-redux');
  return { useDispatch, useSelector };
});
vi.mock('/@/hooks/useAuth', () => ({ useAuth: () => ({}) }));
vi.mock('/@/hooks/useInspector', () => ({ useInspector: () => ({}) }));

const PROJECT: Project = {
  id: 'test-project',
  path: '/scenes/my-scene',
  title: 'Old name',
  thumbnail: '',
  layout: { rows: 1, cols: 1 },
  scene: { base: '0,0', parcels: ['0,0'] },
  createdAt: 0,
  updatedAt: 0,
  publishedAt: 0,
  size: 0,
  dependencyAvailableUpdates: {},
  info: { id: 'test-project', skipPublishWarning: false },
};

const DEPLOYMENT = {
  publishedAt: 1,
  deploymentHash: 'bafy-test',
  parcel: '0,0',
  worldName: 'my-world.dcl.eth',
};

const sceneJson = (scene: object) => Buffer.from(JSON.stringify(scene));

describe('when the editor writes scene.json after the project changed', () => {
  let store: TestStore;

  beforeEach(async () => {
    store = createTestStore();
    store.dispatch(
      workspaceActions.getWorkspace.fulfilled(
        { ...store.getState().workspace, projects: [PROJECT] },
        'request-id',
      ),
    );
    store.dispatch(workspaceActions.runProject.fulfilled(PROJECT, 'request-id', PROJECT));
    const wrapper = ({ children }: { children: ReactNode }) => (
      <Provider store={store as never}>{children}</Provider>
    );
    // The inspector RPC keeps the handler from when the editor loaded.
    const { updateScene } = renderHook(() => useEditor(), { wrapper }).result.current;

    // A publish to a World records the world after the editor loaded.
    store.dispatch(
      workspaceActions.updateProject({
        ...PROJECT,
        worldConfiguration: { name: 'my-world.dcl.eth' },
      }),
    );

    // The publish's record reaches only the workspace entry.
    store.dispatch(
      workspaceActions.recordDeployment.fulfilled(
        { 'world:my-world.dcl.eth:0,0': DEPLOYMENT },
        'id',
        {
          path: PROJECT.path,
          info: {} as never,
        },
      ),
    );

    await updateScene({} as RPCInfo, {
      path: 'scene.json',
      content: sceneJson({
        display: { title: 'New name' },
        scene: { base: '0,0', parcels: ['0,0'] },
      }),
    });
  });

  it('should take the new name', () => {
    expect(store.getState().editor.project?.title).toBe('New name');
  });

  it('should keep the deployment the publish recorded', () => {
    expect(store.getState().editor.project?.info.deployments).toEqual({
      'world:my-world.dcl.eth:0,0': DEPLOYMENT,
    });
    expect(store.getState().workspace.projects[0].info.deployments).toEqual({
      'world:my-world.dcl.eth:0,0': DEPLOYMENT,
    });
  });

  it('should keep the world the publish recorded', () => {
    expect(store.getState().editor.project?.worldConfiguration).toEqual({
      name: 'my-world.dcl.eth',
    });
  });
});
