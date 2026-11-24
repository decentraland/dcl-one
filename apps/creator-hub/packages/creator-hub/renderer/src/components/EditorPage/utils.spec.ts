import { describe, expect, it, vi } from 'vitest';

import type { Project } from '/shared/types/projects';

import { getPublishOptions } from './utils';

vi.mock('/@/modules/store/translation/utils', () => ({
  t: (key: string, values?: Record<string, string>) =>
    `${key}${values ? ` ${Object.values(values).join(',')}` : ''}`,
}));

const actions = { onPublishScene: vi.fn(), onDeployWorld: vi.fn(), onDeployLand: vi.fn() };

describe('getPublishOptions', () => {
  describe('when scene.json lost the world the scene was last published to', () => {
    it('should offer to republish to that world, not to its parcel', () => {
      const project = {
        worldConfiguration: { name: '' },
        scene: { base: '-7,-10', parcels: ['-7,-10'] },
        info: {
          id: 'p',
          skipPublishWarning: false,
          deployments: {
            'world:plaza.dcl.eth:-7,-10': {
              publishedAt: 1,
              deploymentHash: 'bafy',
              parcel: '-7,-10',
              worldName: 'plaza.dcl.eth',
            },
          },
        },
      } as unknown as Project;

      const options = getPublishOptions({ project, isDeploying: false, actions });

      expect(options.map(option => option.id)).toEqual(['deploy-world']);
      expect(options[0].label).toContain('plaza.dcl.eth');
    });
  });
});
