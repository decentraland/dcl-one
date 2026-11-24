import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Entity } from '@dcl/ecs';

import type * as AnalyticsModule from '../../lib/logic/analytics';
import { Event } from '../../lib/logic/analytics';
import type { SdkContextValue } from '../../lib/sdk/context';
import { ROOT } from '../../lib/sdk/tree';
import { deleteSelectedEntities, duplicateSelectedEntities } from './entity-shortcuts';

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
      removeEntity: vi.fn(),
      removeSelectedEntities: vi.fn(),
      duplicateEntity: vi.fn(() => nextClone++ as Entity),
      dispatch: vi.fn(() => Promise.resolve()),
    },
    components: { Selection: { getOrNull: () => ({ gizmo: 0 }) } },
    renderer: { camera: { setControlEnabled: vi.fn() } },
  } as unknown as SdkContextValue;
}

describe('entity shortcuts', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    mocks.track.mockClear();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe('when the delete shortcut runs with two entities selected', () => {
    let sdk: SdkContextValue;

    beforeEach(() => {
      sdk = createSdk([512 as Entity, 513 as Entity]);
      deleteSelectedEntities(sdk);
    });

    it('should remove every selected entity', () => {
      expect(sdk.operations.removeEntity).toHaveBeenCalledTimes(2);
    });

    it('should track one Delete Entity with the count and the shortcut source', () => {
      expect(mocks.track).toHaveBeenCalledTimes(1);
      expect(mocks.track).toHaveBeenCalledWith(Event.DELETE_ENTITY, {
        entitiesCount: 2,
        source: 'shortcut',
      });
    });
  });

  describe('when the delete shortcut runs with only the scene root selected', () => {
    beforeEach(() => {
      deleteSelectedEntities(createSdk([ROOT]));
    });

    it('should not track a Delete Entity', () => {
      expect(mocks.track).not.toHaveBeenCalled();
    });
  });

  describe('when the duplicate shortcut runs with three entities selected', () => {
    let sdk: SdkContextValue;

    beforeEach(() => {
      sdk = createSdk([512 as Entity, 513 as Entity, 514 as Entity]);
      duplicateSelectedEntities(sdk);
    });

    it('should duplicate every selected entity', () => {
      expect(sdk.operations.duplicateEntity).toHaveBeenCalledTimes(3);
    });

    it('should track one Duplicate Entity with the count and the shortcut source', () => {
      expect(mocks.track).toHaveBeenCalledTimes(1);
      expect(mocks.track).toHaveBeenCalledWith(Event.DUPLICATE_ENTITY, {
        entitiesCount: 3,
        source: 'shortcut',
      });
    });
  });

  describe('when the duplicate shortcut runs with nothing selected', () => {
    beforeEach(() => {
      duplicateSelectedEntities(createSdk([]));
    });

    it('should not track a Duplicate Entity', () => {
      expect(mocks.track).not.toHaveBeenCalled();
    });
  });
});
