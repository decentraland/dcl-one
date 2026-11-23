import { describe, expect, it } from 'vitest';

import { RECORDED_EVENT_KINDS as INSPECTOR_KINDS } from '../../../inspector/src/lib/recorder/types';
import { RECORDED_EVENT_KINDS } from '../types/recording';

describe('RECORDED_EVENT_KINDS', () => {
  describe('when the host and the inspector each declare the event kinds', () => {
    it('should list the same kinds, so main never drops an event the inspector sends', () => {
      expect([...RECORDED_EVENT_KINDS].sort()).toEqual([...INSPECTOR_KINDS].sort());
    });
  });
});
