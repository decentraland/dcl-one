import { flatten } from 'flat';
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
        durationMs: 65_400,
        actionCount: 4,
        dir: '/rec/s1',
        context: '[Session recording]',
      });

      expect(label).toBe('Session recording 1:05');
    });
  });
});
