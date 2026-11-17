import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { flatten } from '/@/modules/store/translation/flatten';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale, t } from '/@/modules/store/translation/utils';

import { expectFocusTrap, pressBackdrop, pressEscape } from '../../../../tests/utils/modalContract';
import { TestProviders } from '../../../../tests/utils/TestProviders';

import { MobileQRCode } from './component';

function renderModal() {
  const onClose = vi.fn();
  render(
    <TestProviders>
      <MobileQRCode
        open
        url="http://localhost:8000"
        qr="data:image/png;base64,AA=="
        onClose={onClose}
      />
    </TestProviders>,
  );
  return { onClose };
}

describe('MobileQRCode', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  afterEach(cleanup);

  it('should be a dialog named by its title, with the code and the address', () => {
    renderModal();

    expect(screen.getByRole('dialog', { name: t('modal.mobile_qr.title') })).toBeTruthy();
    expect(screen.getByRole('img', { name: 'QR Code' })).toBeTruthy();
    expect(screen.getByText('http://localhost:8000')).toBeTruthy();
  });

  it('should keep focus in the dialog', () => {
    renderModal();

    expectFocusTrap();
  });

  it('should close from the close button, from Escape and from a press on the backdrop', () => {
    const { onClose } = renderModal();

    fireEvent.click(screen.getByRole('button', { name: 'close' }));
    expect(onClose).toHaveBeenCalledTimes(1);
    pressEscape();
    expect(onClose).toHaveBeenCalledTimes(2);
    pressBackdrop();
    expect(onClose).toHaveBeenCalledTimes(3);
  });
});
