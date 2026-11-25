import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { flatten } from '/@/modules/store/translation/flatten';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale, t } from '/@/modules/store/translation/utils';

import { expectFocusTrap, pressBackdrop, pressEscape } from '../../../../tests/utils/modalContract';
import { TestProviders } from '../../../../tests/utils/TestProviders';
import { tracked } from '../../../../tests/utils/analytics';

import { InstallClient } from './component';

const mocks = vi.hoisted(() => ({
  dispatch: vi.fn(),
  openExternalURL: vi.fn((url: string) => ({ type: 'editor/openExternalURL', url })),
}));

vi.mock('#store', () => ({ useDispatch: () => mocks.dispatch }));

vi.mock('/@/modules/store/editor', () => ({
  actions: { openExternalURL: mocks.openExternalURL },
}));

function setPlatform(platform: string) {
  Object.defineProperty(navigator, 'userAgentData', { configurable: true, value: { platform } });
}

function renderModal() {
  const onClose = vi.fn();
  render(
    <TestProviders>
      <InstallClient
        open
        onClose={onClose}
      />
    </TestProviders>,
  );
  return { onClose };
}

describe('InstallClient', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  beforeEach(() => {
    setPlatform('macOS');
  });

  afterEach(() => {
    cleanup();
    Reflect.deleteProperty(navigator, 'userAgentData');
    vi.restoreAllMocks();
    vi.clearAllMocks();
  });

  it('should be a dialog named by its heading', () => {
    renderModal();

    expect(screen.getByRole('dialog', { name: t('modal.install_client.title') })).toBeTruthy();
  });

  it('should keep focus in the dialog', () => {
    renderModal();

    expectFocusTrap();
  });

  it('should stay open on Escape and on a press on the backdrop', () => {
    const { onClose } = renderModal();

    pressEscape();
    pressBackdrop();

    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole('dialog')).toBeTruthy();
  });

  it('should open the download page and close from the download button', () => {
    const { onClose } = renderModal();

    fireEvent.click(screen.getByRole('button', { name: t('modal.install_client.download_macos') }));

    expect(mocks.openExternalURL).toHaveBeenCalledWith('https://decentraland.org/download/');
    expect(mocks.dispatch).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('should offer the Windows download on another system', () => {
    setPlatform('Windows');
    renderModal();

    expect(
      screen.getByRole('button', { name: t('modal.install_client.download_win') }),
    ).toBeTruthy();
  });

  it('should read the user agent when the platform is not exposed', () => {
    Reflect.deleteProperty(navigator, 'userAgentData');
    vi.spyOn(navigator, 'userAgent', 'get').mockReturnValue(
      'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36',
    );
    renderModal();

    expect(
      screen.getByRole('button', { name: t('modal.install_client.download_macos') }),
    ).toBeTruthy();
  });

  it('should close from Cancel', () => {
    const { onClose } = renderModal();

    fireEvent.click(screen.getByRole('button', { name: t('modal.install_client.cancel') }));

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  describe('analytics', () => {
    it('should track the prompt as shown when it opens', () => {
      renderModal();

      expect(tracked('Install Client Prompt')).toEqual([{ action: 'shown' }]);
    });

    it('should track the download', () => {
      renderModal();

      fireEvent.click(
        screen.getByRole('button', { name: t('modal.install_client.download_macos') }),
      );

      expect(tracked('Install Client Prompt')).toContainEqual({ action: 'download' });
    });

    it('should track the dismissal from Cancel', () => {
      renderModal();

      fireEvent.click(screen.getByRole('button', { name: t('modal.install_client.cancel') }));

      expect(tracked('Install Client Prompt')).toContainEqual({ action: 'dismiss' });
    });
  });
});
