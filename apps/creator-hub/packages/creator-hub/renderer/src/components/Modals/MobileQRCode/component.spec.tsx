import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { flatten } from '/@/modules/store/translation/flatten';
import { act } from 'react';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { analytics, editor } from '#preload';
import type { MobileDebugSessionInfo } from '/shared/types/ipc';

import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale, t } from '/@/modules/store/translation/utils';

import { expectFocusTrap, pressBackdrop, pressEscape } from '../../../../tests/utils/modalContract';
import { TestProviders } from '../../../../tests/utils/TestProviders';
import { tracked } from '../../../../tests/utils/analytics';

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

describe('MobileQRCode analytics', () => {
  const session = (id: number, disconnected = false) =>
    ({
      id,
      sessionId: null,
      deviceName: null,
      connectedAt: '2026-01-01T00:00:00.000Z',
      disconnectedAt: disconnected ? '2026-01-01T00:01:00.000Z' : null,
      messageCount: 0,
    }) as unknown as MobileDebugSessionInfo;

  let publish: (sessions: MobileDebugSessionInfo[]) => void = () => {};

  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  afterEach(cleanup);

  describe('when devices connect one after another', () => {
    it('should report each device once, with how many are connected then', () => {
      vi.mocked(analytics.track).mockClear();
      vi.mocked(editor.onMobileDebugSessions).mockImplementation(listener => {
        publish = listener;
        return () => {};
      });
      renderModal();

      act(() => publish([session(1)]));
      act(() => publish([session(1)]));
      act(() => publish([session(1), session(2)]));
      act(() => publish([session(1, true), session(2), session(3)]));

      expect(tracked('Mobile Preview Connected')).toEqual([
        { devices: 1 },
        { devices: 2 },
        { devices: 2 },
      ]);
    });
  });
});
