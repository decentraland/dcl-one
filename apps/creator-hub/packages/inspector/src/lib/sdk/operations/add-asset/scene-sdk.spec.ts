import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { getSceneClient } from '../../../rpc/scene';
import type { SceneClient } from '../../../rpc/scene/client';
import { canUpdateSceneSdk, getSdkRequirement, updateSceneSdk } from './scene-sdk';

vi.mock('../../../rpc/scene', () => ({ getSceneClient: vi.fn() }));

describe('scene SDK', () => {
  let client: { getSdkVersion: ReturnType<typeof vi.fn>; updateSdk: ReturnType<typeof vi.fn> };
  let logWarning: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    client = { getSdkVersion: vi.fn(), updateSdk: vi.fn() };
    vi.mocked(getSceneClient).mockReturnValue(client as unknown as SceneClient);
    logWarning = vi.spyOn(console, 'warn').mockImplementation(() => {});
  });

  afterEach(() => {
    logWarning.mockRestore();
    vi.mocked(getSceneClient).mockReset();
  });

  describe('when reading what an asset needs', () => {
    it('should not ask the host about an asset that declares no minimum', async () => {
      expect(await getSdkRequirement(undefined)).toEqual({});
      expect(client.getSdkVersion).not.toHaveBeenCalled();
    });

    it('should pair the minimum with the version the host reports', async () => {
      client.getSdkVersion.mockResolvedValue({ version: '7.31.0' });
      expect(await getSdkRequirement('7.32.0')).toEqual({
        required: '7.32.0',
        installed: '7.31.0',
      });
    });

    it('should leave the installed version unknown when the host cannot answer', async () => {
      client.getSdkVersion.mockRejectedValue(new Error('Method "get_sdk_version" not implemented'));
      expect(await getSdkRequirement('7.32.0')).toEqual({ required: '7.32.0' });
    });

    it('should leave the installed version unknown without a host', async () => {
      vi.mocked(getSceneClient).mockReturnValue(undefined);
      expect(await getSdkRequirement('7.32.0')).toEqual({ required: '7.32.0' });
      expect(canUpdateSceneSdk()).toBe(false);
    });
  });

  describe('when updating the SDK of the scene', () => {
    it('should succeed when the scene then has the SDK the asset needs', async () => {
      client.updateSdk.mockResolvedValue({ ok: true });
      client.getSdkVersion.mockResolvedValue({ version: '7.33.0' });
      expect(await updateSceneSdk('7.32.0')).toBe(true);
    });

    it('should fail when the update left the scene on an older SDK', async () => {
      client.updateSdk.mockResolvedValue({ ok: true });
      client.getSdkVersion.mockResolvedValue({ version: '7.31.0' });
      expect(await updateSceneSdk('7.32.0')).toBe(false);
    });

    it('should fail when the host could not update', async () => {
      client.updateSdk.mockResolvedValue({ ok: false });
      expect(await updateSceneSdk('7.32.0')).toBe(false);
      expect(client.getSdkVersion).not.toHaveBeenCalled();
    });

    it('should fail when the request to the host fails', async () => {
      client.updateSdk.mockRejectedValue(new Error('timeout'));
      expect(await updateSceneSdk('7.32.0')).toBe(false);
    });
  });
});
