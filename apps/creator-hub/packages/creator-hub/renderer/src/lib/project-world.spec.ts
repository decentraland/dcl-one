import { describe, expect, it } from 'vitest';

import type { Project, ProjectDeployment } from '/shared/types/projects';

import { getProjectWorldName } from './project-world';

const record = (overrides: Partial<ProjectDeployment>): ProjectDeployment => ({
  publishedAt: 1,
  deploymentHash: 'bafy',
  parcel: '-7,-10',
  ...overrides,
});

const project = (
  worldName: string | undefined,
  deployments: Record<string, ProjectDeployment> = {},
) =>
  ({
    worldConfiguration: worldName === undefined ? undefined : { name: worldName },
    info: { id: 'p', skipPublishWarning: false, deployments },
  }) as unknown as Project;

describe('getProjectWorldName', () => {
  describe('when scene.json names the world', () => {
    it('should use it', () => {
      expect(getProjectWorldName(project('plaza.dcl.eth'))).toBe('plaza.dcl.eth');
    });
  });

  describe('when scene.json lost the world the scene was last published to', () => {
    it('should use the world of the latest publish', () => {
      const deployments = {
        'land:-7,-10': record({ publishedAt: 1 }),
        'world:plaza.dcl.eth:-7,-10': record({ publishedAt: 2, worldName: 'plaza.dcl.eth' }),
      };
      expect(getProjectWorldName(project('', deployments))).toBe('plaza.dcl.eth');
    });
  });

  describe('when the scene was last published to LAND', () => {
    it('should have no world', () => {
      const deployments = {
        'world:plaza.dcl.eth:0,0': record({ publishedAt: 1, worldName: 'plaza.dcl.eth' }),
        'land:-7,-10': record({ publishedAt: 2 }),
      };
      expect(getProjectWorldName(project(undefined, deployments))).toBeUndefined();
    });
  });
});
