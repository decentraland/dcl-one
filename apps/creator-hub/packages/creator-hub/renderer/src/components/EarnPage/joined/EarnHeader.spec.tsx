import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { flatten } from '/@/modules/store/translation/flatten';
import { afterEach, beforeAll, describe, expect, it } from 'vitest';

import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';

import { TestProviders } from '../../../../tests/utils/TestProviders';
import { EarnMenu } from './EarnHeader';

describe('EarnMenu', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  afterEach(() => {
    cleanup();
  });

  describe('when the More options button is clicked while the menu is open', () => {
    it('should close the menu', async () => {
      render(
        <TestProviders>
          <EarnMenu />
        </TestProviders>,
      );
      const button = screen.getByRole('button', { name: 'More options' });
      fireEvent.click(button);
      expect(await screen.findByRole('menu')).toBeTruthy();
      expect(button.getAttribute('aria-expanded')).toBe('true');

      fireEvent.click(button);

      expect(screen.queryByRole('menu')).toBeNull();
      expect(button.getAttribute('aria-expanded')).toBe('false');
    });
  });
});
