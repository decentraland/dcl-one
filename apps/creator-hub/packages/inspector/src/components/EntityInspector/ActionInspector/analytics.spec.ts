import { beforeEach, describe, expect, it, vi } from 'vitest';

import type * as AnalyticsModule from '../../../lib/logic/analytics';
import { Event } from '../../../lib/logic/analytics';
import { trackActionTypeSelected } from './analytics';

const mocks = vi.hoisted(() => ({ track: vi.fn() }));

vi.mock('../../../lib/logic/analytics', async importActual => ({
  ...(await importActual<typeof AnalyticsModule>()),
  analytics: { track: mocks.track },
}));

describe('trackActionTypeSelected', () => {
  beforeEach(() => {
    mocks.track.mockClear();
  });

  describe('when the first type is picked for a new action row', () => {
    beforeEach(() => {
      trackActionTypeSelected('', 'play_sound', 'assets/scene/door.glb');
    });

    it('should track an Add Smart Item Action with the type and item path', () => {
      expect(mocks.track).toHaveBeenCalledWith(Event.ADD_SMART_ITEM_ACTION, {
        actionType: 'play_sound',
        itemPath: 'assets/scene/door.glb',
      });
    });
  });

  describe('when an existing action is retyped', () => {
    beforeEach(() => {
      trackActionTypeSelected('play_sound', 'stop_sound', 'assets/scene/door.glb');
    });

    it('should not track an Add Smart Item Action', () => {
      expect(mocks.track).not.toHaveBeenCalled();
    });
  });
});
