import type { Entity } from '@dcl/ecs';

import { findPlayerAttachments, parseEngineSceneState } from './engine-scene-state';

const e = (n: number) => n as Entity;
const PLAYER = e(1);
const CAMERA = e(2);
const ANCHORS = new Set([PLAYER, CAMERA]);
const TRANSFORM = {
  position: { x: 0, y: 0, z: 0 },
  rotation: { x: 0, y: 0, z: 0, w: 1 },
  scale: { x: 1, y: 1, z: 1 },
};

describe('parseEngineSceneState', () => {
  describe('when the reply is a crdt snapshot', () => {
    const reply = JSON.stringify({
      '0': {
        Skybox: { stars: { brightness: 1 } },
        SkyboxTime: { fixedTime: 5400 },
        LightSource: { active: false },
        Transform: { ...TRANSFORM, parent: 0 },
      },
      '600': { Transform: { ...TRANSFORM, parent: 2 }, LightSource: { active: true } },
      '601': { AvatarAttach: { anchorPointId: 0 } },
      '602': { AvatarAttach: { avatarId: '0x0000000000000000000000000000000000000001' } },
      '603': { VisibilityComponent: { visible: false } },
      '604': { VisibilityComponent: {} },
    });

    it('should read each entity Transform parent, skipping the root parenting itself', () => {
      const state = parseEngineSceneState(reply)!;
      expect(state.parents.get(e(600))).toBe(CAMERA);
      expect(state.parents.has(e(0))).toBe(false);
    });

    it('should mark only the AvatarAttach entities without an avatarId as local-player attachments', () => {
      const state = parseEngineSceneState(reply)!;
      expect([...state.localAvatarAttached]).toEqual([e(601)]);
    });

    it('should read VisibilityComponent with an unset visible as visible', () => {
      const state = parseEngineSceneState(reply)!;
      expect(state.visibility.get(e(603))).toBe(false);
      expect(state.visibility.get(e(604))).toBe(true);
    });

    it('should keep the root lighting components present, including a disabled LightSource', () => {
      const state = parseEngineSceneState(reply)!;
      expect(Object.fromEntries(state.rootLighting)).toEqual({
        LightSource: { active: false },
        Skybox: { stars: { brightness: 1 } },
        SkyboxTime: { fixedTime: 5400 },
      });
    });
  });

  describe('when the reply is not a snapshot', () => {
    it('should return null', () => {
      expect(parseEngineSceneState('player is not in any scene')).toBeNull();
      expect(parseEngineSceneState('[]')).toBeNull();
      expect(parseEngineSceneState('')).toBeNull();
    });
  });
});

describe('findPlayerAttachments', () => {
  describe('when entities are parented to the camera or the player', () => {
    it('should include them and their descendants, and nothing else', () => {
      const parents = new Map<Entity, Entity | undefined>([
        [e(600), CAMERA],
        [e(601), e(600)],
        [e(602), PLAYER],
        [e(603), e(0)],
        [e(604), undefined],
      ]);
      expect(findPlayerAttachments(parents, new Set(), ANCHORS)).toEqual(
        new Set([e(600), e(601), e(602)]),
      );
    });
  });

  describe('when an entity carries an AvatarAttach on the local player', () => {
    it('should include it and its descendants, with or without a Transform', () => {
      const parents = new Map<Entity, Entity | undefined>([[e(701), e(700)]]);
      expect(findPlayerAttachments(parents, new Set([e(700)]), ANCHORS)).toEqual(
        new Set([e(700), e(701)]),
      );
    });
  });

  describe('when the parent chain is cyclic', () => {
    it('should stop the walk and leave the entities out', () => {
      const parents = new Map<Entity, Entity | undefined>([
        [e(800), e(801)],
        [e(801), e(800)],
      ]);
      expect(findPlayerAttachments(parents, new Set(), ANCHORS).size).toBe(0);
    });
  });
});
