import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { flatten } from '/@/modules/store/translation/flatten';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { dark, ThemeProvider } from 'decentraland-ui2/dist/theme';

import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale, t } from '/@/modules/store/translation/utils';

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
    <ThemeProvider theme={dark}>
      <InstallClient
        open
        onClose={onClose}
      />
    </ThemeProvider>,
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
});
