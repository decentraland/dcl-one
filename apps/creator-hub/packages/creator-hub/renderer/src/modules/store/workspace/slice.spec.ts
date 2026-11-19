import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { Project } from '/shared/types/projects';

import { workspace } from '#preload';

import { createTestStore } from '../../../../tests/utils/testStore';
import { actions as workspaceActions } from '.';

const TEST_PATH = '/test/scene';
const STOCK_THUMBNAIL = 'stock-template-picture';
const CAPTURED_THUMBNAIL = 'captured-scene-picture';
const PROJECT: Project = {
  id: 'test-project',
  path: TEST_PATH,
  title: 'Test scene',
  thumbnail: STOCK_THUMBNAIL,
  layout: { rows: 1, cols: 1 },
  scene: { base: '0,0', parcels: ['0,0'] },
  createdAt: 0,
  updatedAt: 0,
  publishedAt: 0,
  size: 0,
  dependencyAvailableUpdates: {},
  info: { id: 'test-project', skipPublishWarning: false },
};

describe('when the editor saves a captured scene thumbnail', () => {
  let store: ReturnType<typeof createTestStore>;
  let result: string;

  beforeEach(async () => {
    store = createTestStore();
    store.dispatch(
      workspaceActions.getWorkspace.fulfilled(
        { ...store.getState().workspace, projects: [PROJECT] },
        'request-id',
      ),
    );
    store.dispatch(workspaceActions.runProject.fulfilled(PROJECT, 'request-id', PROJECT));
    // A template scene keeps naming its stock picture in scene.json after the capture.
    vi.mocked(workspace.getProjectThumbnailAsBase64).mockResolvedValue(STOCK_THUMBNAIL);
    vi.mocked(workspace.saveThumbnail).mockResolvedValue(undefined);

    result = await store
      .dispatch(
        workspaceActions.saveAndGetThumbnail({ path: TEST_PATH, thumbnail: CAPTURED_THUMBNAIL }),
      )
      .unwrap();
  });

  it('should write the captured picture to the project', () => {
    expect(vi.mocked(workspace.saveThumbnail).mock.calls[0][0]).toEqual({
      path: TEST_PATH,
      thumbnail: CAPTURED_THUMBNAIL,
    });
  });

  it('should resolve with the captured picture', () => {
    expect(result).toBe(CAPTURED_THUMBNAIL);
  });

  it('should show the captured picture on the open project', () => {
    expect(store.getState().editor.project?.thumbnail).toBe(CAPTURED_THUMBNAIL);
  });

  it('should show the captured picture on the project in the scene list', () => {
    expect(store.getState().workspace.projects[0].thumbnail).toBe(CAPTURED_THUMBNAIL);
  });
});
