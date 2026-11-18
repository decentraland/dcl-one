import { configureStore } from '@reduxjs/toolkit';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { misc } from '#preload';
import type { AuthIdentity } from '/@/lib/identity';
import type { Info } from '/@/lib/deploy';

import { deploymentLogMiddleware } from './deploymentLog';
import { deploy, type Deployment } from './slice';

const createStore = () =>
  configureStore({
    reducer: { noop: (state: null = null) => state },
    middleware: getDefaultMiddleware =>
      getDefaultMiddleware({ serializableCheck: false }).concat(deploymentLogMiddleware),
  });

const arg = { identity: {} as AuthIdentity, deployment: { path: '/scenes/a' } as Deployment };

describe('deploymentLogMiddleware', () => {
  beforeEach(() => {
    vi.mocked(misc.logDeployment).mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  describe('when a content server accepts a world publish', () => {
    it('should log the scene, its target and the deployed entity', () => {
      const info = {
        title: 'Punkland',
        rootCID: 'bafyroot',
        isWorld: true,
        baseParcel: '0,0',
      } as Info;
      createStore().dispatch(deploy.fulfilled({ info } as never, 'request', arg));

      expect(misc.logDeployment).toHaveBeenCalledWith({
        outcome: 'published',
        title: 'Punkland',
        rootCID: 'bafyroot',
        target: 'world',
        baseParcel: '0,0',
      });
    });
  });

  describe('when a publish fails', () => {
    it('should log the error', () => {
      createStore().dispatch(
        deploy.rejected(new Error('Request timed out'), 'request', arg, undefined),
      );

      expect(misc.logDeployment).toHaveBeenCalledWith({
        outcome: 'failed',
        error: 'Error: Request timed out',
      });
    });
  });
});
