import { beforeEach, describe, expect, it, vi, type Mock } from 'vitest';
import type { Entity } from '@dcl/ecs';

import type * as AnalyticsModule from '../../lib/logic/analytics';
import { Event } from '../../lib/logic/analytics';
import type { SdkContextValue } from '../../lib/sdk/context';
import { duplicateInTree, removeFromTree } from './entity-actions';

const mocks = vi.hoisted(() => ({ track: vi.fn() }));

vi.mock('../../lib/logic/analytics', async importActual => ({
  ...(await importActual<typeof AnalyticsModule>()),
  analytics: { track: mocks.track },
}));

function createSdk(selected: Entity[]) {
  let nextClone = 900;
  return {
    operations: {
      getSelectedEntities: vi.fn(() => selected),
      removeSelectedEntities: vi.fn(),
      duplicateEntity: vi.fn(() => nextClone++ as Entity),
      dispatch: vi.fn(() => Promise.resolve()),
    },
    components: { Selection: { getOrNull: () => ({ gizmo: 0 }) } },
  } as unknown as SdkContextValue;
}

describe('tree context menu entity actions', () => {
  let callback: Mock;

  beforeEach(() => {
    mocks.track.mockClear();
    callback = vi.fn();
  });

  describe('when Delete is chosen for a single entity', () => {
    beforeEach(() => {
      removeFromTree(createSdk([512 as Entity]), 512 as Entity, callback);
    });

    it('should remove the entity', () => {
      expect(callback).toHaveBeenCalledWith(512);
    });

    it('should track a Delete Entity from the context menu', () => {
      expect(mocks.track).toHaveBeenCalledWith(Event.DELETE_ENTITY, {
        entitiesCount: 1,
        source: 'context_menu',
      });
    });
  });

  describe('when Delete is chosen with three entities selected', () => {
    beforeEach(() => {
      const selected = [512, 513, 514] as Entity[];
      removeFromTree(createSdk(selected), 512 as Entity, callback);
    });

    it('should remove every selected entity', () => {
      expect(callback).toHaveBeenCalledTimes(3);
    });

    it('should track one Delete Entity with the selection count', () => {
      expect(mocks.track).toHaveBeenCalledTimes(1);
      expect(mocks.track).toHaveBeenCalledWith(Event.DELETE_ENTITY, {
        entitiesCount: 3,
        source: 'context_menu',
      });
    });
  });

  describe('when Delete is chosen in a file tree', () => {
    beforeEach(() => {
      removeFromTree(createSdk([]), 'assets/scene/model.glb', callback);
    });

    it('should not track a Delete Entity', () => {
      expect(mocks.track).not.toHaveBeenCalled();
    });
  });

  describe('when Duplicate is chosen for a single entity', () => {
    beforeEach(() => {
      duplicateInTree(createSdk([512 as Entity]), 512 as Entity, callback);
    });

    it('should duplicate the entity', () => {
      expect(callback).toHaveBeenCalledWith(512, 0);
    });

    it('should track a Duplicate Entity from the context menu', () => {
      expect(mocks.track).toHaveBeenCalledWith(Event.DUPLICATE_ENTITY, {
        entitiesCount: 1,
        source: 'context_menu',
      });
    });
  });

  describe('when Duplicate is chosen with two entities selected', () => {
    let sdk: SdkContextValue;

    beforeEach(() => {
      sdk = createSdk([512, 513] as Entity[]);
      duplicateInTree(sdk, 512 as Entity, callback);
    });

    it('should duplicate every selected entity', () => {
      expect(sdk.operations.duplicateEntity).toHaveBeenCalledTimes(2);
    });

    it('should track one Duplicate Entity with the selection count', () => {
      expect(mocks.track).toHaveBeenCalledTimes(1);
      expect(mocks.track).toHaveBeenCalledWith(Event.DUPLICATE_ENTITY, {
        entitiesCount: 2,
        source: 'context_menu',
      });
    });
  });

  describe('when Duplicate is chosen in a UI Designer tree', () => {
    beforeEach(() => {
      duplicateInTree(createSdk([]), { id: 'node-1' }, callback);
    });

    it('should not track a Duplicate Entity', () => {
      expect(mocks.track).not.toHaveBeenCalled();
    });
  });
});
