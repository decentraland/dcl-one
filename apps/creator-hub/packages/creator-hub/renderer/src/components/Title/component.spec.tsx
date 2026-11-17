import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { flatten } from '/@/modules/store/translation/flatten';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';

import { TestProviders } from '../../../tests/utils/TestProviders';
import { Title } from '.';

describe('Title', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  afterEach(cleanup);

  describe('when it has a back action', () => {
    it('should be the page heading with a Back button', () => {
      const onBack = vi.fn();
      render(
        <TestProviders>
          <Title
            value="Manage - Land Name"
            onBack={onBack}
          />
        </TestProviders>,
      );

      expect(screen.getByRole('heading', { level: 1, name: 'Manage - Land Name' })).toBeTruthy();
      fireEvent.click(screen.getByRole('button', { name: 'Back' }));
      expect(onBack).toHaveBeenCalled();
    });
  });
});
