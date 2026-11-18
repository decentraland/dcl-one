import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Entity } from '@dcl/ecs';
import type { SkyboxController as SkyboxControllerClass } from '../packs/smart_items/assets/skybox/skybox-controller';
import { fakeSdkEcs } from './script-harness';

// An SDK older than 7.32 exports no Skybox, Sun or EnvironmentFog. The keys are listed as undefined
// because vitest throws on reading a key a mocked module doesn't define.
vi.mock('@dcl/sdk/ecs', async () => ({
  ...(await fakeSdkEcs()),
  Skybox: undefined,
  Sun: undefined,
  EnvironmentFog: undefined,
}));

describe('SkyboxController on an SDK older than 7.32', () => {
  let skybox: SkyboxControllerClass;
  let logError: ReturnType<typeof vi.spyOn>;

  beforeEach(async () => {
    logError = vi.spyOn(console, 'error').mockImplementation(() => {});
    const { SkyboxController } = await import(
      '../packs/smart_items/assets/skybox/skybox-controller'
    );
    skybox = new (SkyboxController as any)('src', 512 as Entity) as SkyboxControllerClass;
  });

  afterEach(() => {
    logError.mockRestore();
  });

  describe('when it starts', () => {
    it('should not throw, so the scripts after it still run', () => {
      expect(() => skybox.start()).not.toThrow();
    });

    it('should log the SDK version it needs', () => {
      skybox.start();
      expect(logError).toHaveBeenCalledWith(expect.stringContaining('@dcl/sdk 7.32.0'));
    });
  });

  describe('when its actions run', () => {
    it('should not throw', () => {
      expect(() => {
        skybox.enable();
        skybox.disable();
        skybox.toggle();
        skybox.hideClouds();
        skybox.showClouds();
        skybox.clearFog();
        skybox.restoreFog();
      }).not.toThrow();
    });
  });
});
