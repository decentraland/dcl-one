import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { Entity } from '@dcl/ecs';

import type * as AnalyticsModule from '../../../lib/logic/analytics';
import { Event } from '../../../lib/logic/analytics';
import TriggerInspector from './TriggerInspector';

const mocks = vi.hoisted(() => ({
  track: vi.fn(),
  triggers: [] as unknown[],
  gltfSrc: 'assets/scene/door.glb' as string | undefined,
}));

vi.mock('../../../lib/logic/analytics', async importActual => ({
  ...(await importActual<typeof AnalyticsModule>()),
  analytics: { track: mocks.track },
}));

vi.mock('../../../hooks/sdk/useSdk', () => {
  const component = { has: () => false, getOrNull: () => null };
  return {
    useSdk: () => ({
      components: {
        Actions: component,
        Triggers: component,
        Name: component,
        States: component,
        Counter: component,
        GltfContainer: {
          ...component,
          getOrNull: () => (mocks.gltfSrc ? { src: mocks.gltfSrc } : null),
        },
      },
    }),
  };
});

vi.mock('../../../hooks/sdk/useComponentValue', () => ({
  useComponentValue: () => [{ value: mocks.triggers }, vi.fn(), () => true],
  getComponentValue: () => ({ value: [] }),
}));
vi.mock('../../../hooks/sdk/useHasComponent', () => ({ useHasComponent: () => true }));
vi.mock('../../../hooks/sdk/useEntitiesWith', () => ({ useEntitiesWith: () => [] }));

vi.mock('../../Container', () => ({
  Container: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}));
vi.mock('../../ui/InfoTooltip', () => ({ InfoTooltip: () => null }));
vi.mock('./TriggerEvent', () => ({ TriggerEvent: () => null }));
vi.mock('./TriggerCondition', () => ({ TriggerConditionContainer: () => null }));
vi.mock('./TriggerAction', () => ({ TriggerActionContainer: () => null }));

const entity = 512 as Entity;

describe('TriggerInspector', () => {
  beforeEach(() => {
    mocks.track.mockClear();
    mocks.triggers = [{ type: 'on_click', conditions: [], actions: [] }];
    mocks.gltfSrc = 'assets/scene/door.glb';
  });

  afterEach(cleanup);

  describe('when Add New Trigger Event is clicked', () => {
    beforeEach(() => {
      render(<TriggerInspector entity={entity} />);
      fireEvent.click(screen.getByRole('button', { name: /Add New Trigger Event/ }));
    });

    it('should track one Add Smart Item Trigger with the new count and the item path', () => {
      expect(mocks.track).toHaveBeenCalledTimes(1);
      expect(mocks.track).toHaveBeenCalledWith(Event.ADD_SMART_ITEM_TRIGGER, {
        triggersCount: 2,
        itemPath: 'assets/scene/door.glb',
      });
    });
  });

  describe('when the entity has no model', () => {
    beforeEach(() => {
      mocks.gltfSrc = undefined;
      render(<TriggerInspector entity={entity} />);
      fireEvent.click(screen.getByRole('button', { name: /Add New Trigger Event/ }));
    });

    it('should track the add without an item path', () => {
      expect(mocks.track).toHaveBeenCalledWith(Event.ADD_SMART_ITEM_TRIGGER, {
        triggersCount: 2,
        itemPath: undefined,
      });
    });
  });
});
