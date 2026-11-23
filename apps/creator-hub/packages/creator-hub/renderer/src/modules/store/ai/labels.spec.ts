import { flatten } from '/@/modules/store/translation/flatten';
import { beforeAll, describe, expect, it } from 'vitest';

import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';

import { recordingAttachmentLabel } from './labels';

describe('recordingAttachmentLabel', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  describe('when a recording goes out with a message', () => {
    it('should name it in the reader language, with its length', () => {
      const label = recordingAttachmentLabel({
        sessions: [
          { dir: '/rec/s1', durationMs: 65_400, actionCount: 4, context: '[Session recording]' },
        ],
        durationMs: 65_400,
        actionCount: 4,
      });

      expect(label).toBe('Session recording 1:05');
    });
  });

  describe('when several recordings go out together', () => {
    it('should count them and give their total length', () => {
      const label = recordingAttachmentLabel({
        sessions: [
          { dir: '/rec/s1', durationMs: 65_400, actionCount: 4, context: '[1]' },
          { dir: '/rec/s2', durationMs: 85_000, actionCount: 8, context: '[2]' },
        ],
        durationMs: 150_400,
        actionCount: 12,
      });

      expect(label).toBe('2 recordings 2:30');
    });
  });
});
