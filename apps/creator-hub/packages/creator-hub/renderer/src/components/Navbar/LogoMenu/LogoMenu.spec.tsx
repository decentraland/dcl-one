import { act, fireEvent, render, screen } from '@testing-library/react';
import { flatten } from '/@/modules/store/translation/flatten';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { feedback, misc } from '#preload';
import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';

import { TestProviders } from '../../../../tests/utils/TestProviders';

import { LogoMenu } from '.';

describe('LogoMenu', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  describe('when the menu is open', () => {
    it('should offer "Report an issue"', () => {
      render(
        <TestProviders>
          <LogoMenu
            onClickAbout={vi.fn()}
            onClickSettings={vi.fn()}
            onClickCheckForUpdates={vi.fn()}
          />
        </TestProviders>,
      );
      fireEvent.click(screen.getByTestId('logo-menu-button'));
      expect(screen.getByTestId('logo-menu-report-bug').textContent).toBe('Report an issue');
    });

    it('should stop the page scroll until the menu closes', () => {
      render(
        <TestProviders>
          <LogoMenu
            onClickAbout={vi.fn()}
            onClickSettings={vi.fn()}
            onClickCheckForUpdates={vi.fn()}
          />
        </TestProviders>,
      );
      expect(document.body.style.overflow).toBe('');

      fireEvent.click(screen.getByTestId('logo-menu-button'));
      expect(document.body.style.overflow).toBe('hidden');

      fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
      expect(screen.queryByRole('menu')).toBeNull();
      expect(document.body.style.overflow).toBe('');
    });
  });

  describe('when the user picks Report an issue', () => {
    beforeEach(() => {
      render(
        <TestProviders>
          <LogoMenu
            onClickAbout={vi.fn()}
            onClickSettings={vi.fn()}
            onClickCheckForUpdates={vi.fn()}
          />
        </TestProviders>,
      );
      fireEvent.click(screen.getByTestId('logo-menu-button'));
      fireEvent.click(screen.getByTestId('logo-menu-report-bug'));
    });

    it('should open the report dialog instead of the Typeform', () => {
      expect(screen.getByRole('dialog', { name: 'Report an issue' })).toBeTruthy();
      expect(misc.openExternal).not.toHaveBeenCalled();
    });

    it('should send the report tagged as coming from the logo menu', async () => {
      vi.mocked(feedback.send).mockResolvedValue('sent');
      fireEvent.change(screen.getByRole('textbox', { name: 'What went wrong?' }), {
        target: { value: 'The app froze' },
      });

      await act(async () => fireEvent.click(screen.getByRole('button', { name: 'Send' })));

      expect(feedback.send).toHaveBeenCalledWith({
        message: 'The app froze',
        includeLogs: true,
        associatedEventId: undefined,
        source: 'logo-menu',
      });
    });
  });
});
