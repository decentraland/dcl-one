import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { flatten } from '/@/modules/store/translation/flatten';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';

import { TestProviders } from '../../../../tests/utils/TestProviders';

import { TitleBarModal } from '.';

function renderModal() {
  const onClose = vi.fn();
  render(
    <TestProviders>
      <TitleBarModal
        open
        title="Create New Password"
        onClose={onClose}
      >
        <p>modal content</p>
      </TitleBarModal>
    </TestProviders>,
  );
  return onClose;
}

describe('TitleBarModal', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  afterEach(cleanup);

  it('should be a modal dialog named by its title bar, holding its content', () => {
    renderModal();

    const dialog = screen.getByRole('dialog', { name: 'Create New Password' });
    expect(dialog.getAttribute('aria-modal')).toBe('true');
    expect(dialog.textContent).toContain('modal content');
  });

  it('should close from its close button and from Escape', () => {
    const onClose = renderModal();

    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });

    expect(onClose).toHaveBeenCalledTimes(2);
  });
});
