import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { flatten } from '/@/modules/store/translation/flatten';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';

import { TestProviders } from '../../../tests/utils/TestProviders';

import { RecordingConsentModal } from './RecordingConsentModal';

function renderModal(downloadMb: number) {
  const onAccept = vi.fn();
  const onCancel = vi.fn();
  render(
    <TestProviders>
      <RecordingConsentModal
        open
        downloadMb={downloadMb}
        onAccept={onAccept}
        onCancel={onCancel}
      />
    </TestProviders>,
  );
  return { dialog: screen.getByRole('dialog'), onAccept, onCancel };
}

describe('RecordingConsentModal', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  afterEach(cleanup);

  describe('when it asks the creator for the first time', () => {
    it('should say what is recorded, where it stays and what is shared', () => {
      const { dialog } = renderModal(0);

      const text = dialog.textContent ?? '';
      expect(text).toContain('What gets recorded?');
      expect(text).toContain('Your data stays on your computer');
      expect(text).toContain('not the audio');
      expect(within(dialog).getByText('Record', { selector: 'b' })).toBeTruthy();
    });

    it('should name the speech model download while it is still to come', () => {
      const { dialog } = renderModal(270);
      expect(dialog.textContent).toContain('(about 270 MB)');
    });

    it('should leave the download out once the model is installed', () => {
      const { dialog } = renderModal(0);
      expect(dialog.textContent).not.toContain('MB');
      expect(dialog.textContent).toContain("system's default microphone");
    });

    it('should start on Got it and back out on Cancel', () => {
      const { dialog, onAccept, onCancel } = renderModal(0);

      fireEvent.click(within(dialog).getByRole('button', { name: 'Got it' }));
      fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));

      expect(onAccept).toHaveBeenCalledTimes(1);
      expect(onCancel).toHaveBeenCalledTimes(1);
    });
  });
});
