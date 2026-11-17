import { cleanup, render, screen } from '@testing-library/react';
import { flatten } from 'flat';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { dark, ThemeProvider } from 'decentraland-ui2/dist/theme';

import { MAX_RECORDING_MS } from '/shared/types/recording';
import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';

import { RecordingConsentModal } from './RecordingConsentModal';

describe('RecordingConsentModal', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  afterEach(cleanup);

  describe('when it asks the creator for the first time', () => {
    it('should say how long a recording can run and what is kept', () => {
      render(
        <ThemeProvider theme={dark}>
          <RecordingConsentModal
            open
            downloadMb={0}
            onAccept={vi.fn()}
            onCancel={vi.fn()}
          />
        </ThemeProvider>,
      );

      const text = screen.getByRole('dialog').textContent ?? '';
      expect(text).toContain(`after ${MAX_RECORDING_MS / 60_000} minutes`);
      expect(text).not.toContain('{minutes}');
      expect(text).toContain('30 days');
    });
  });
});
