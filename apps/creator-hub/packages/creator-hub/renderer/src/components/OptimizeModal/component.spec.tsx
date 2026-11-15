import { Provider } from 'react-redux';
import { fireEvent, render, screen } from '@testing-library/react';
import { flatten } from '/@/modules/store/translation/flatten';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import type { OptimizeScanResult, OptimizeToolsInfo } from '/shared/types/optimizer';
import type { Project } from '/shared/types/projects';
import { optimizer as optimizerPreload } from '#preload';
import { actions } from '/@/modules/store/optimizer';
import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale, t } from '/@/modules/store/translation/utils';

import { expectFocusTrap, pressBackdrop, pressEscape } from '../../../tests/utils/modalContract';
import { TestProviders } from '../../../tests/utils/TestProviders';
import { createTestStore } from '../../../tests/utils/testStore';

import { OptimizeModal } from './component';

const project = { path: '/scenes/one' } as Project;

const scan: OptimizeScanResult = {
  glbCount: 2,
  totalBytes: 2048,
  glbBytes: 1024,
  textureBytes: 1024,
  embeddedTextureCount: 1,
  externalTextureCount: 1,
  hasBackup: false,
  lastOptimizedAt: null,
};

function toolsInfo(status: OptimizeToolsInfo['status']): OptimizeToolsInfo {
  return { status, tools: [], downloadSizeMb: 40 };
}

function renderModal({ acknowledged = false } = {}) {
  const store = createTestStore();
  store.dispatch(actions.open());
  if (acknowledged) store.dispatch(actions.acknowledge());
  render(
    <Provider store={store as never}>
      <TestProviders>
        <OptimizeModal project={project} />
      </TestProviders>
    </Provider>,
  );
  return { store };
}

describe('OptimizeModal', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  beforeEach(() => {
    vi.mocked(optimizerPreload.tools).mockResolvedValue(toolsInfo('ready'));
    vi.mocked(optimizerPreload.scan).mockResolvedValue(scan);
    vi.mocked(optimizerPreload.subscribeProgress).mockReturnValue({ cleanup: vi.fn() });
  });

  it('should be a dialog named by its title', () => {
    renderModal();

    expect(screen.getByRole('dialog', { name: t('optimize.title') })).toBeTruthy();
  });

  it('should keep focus in the dialog', () => {
    renderModal();

    expectFocusTrap();
  });

  it('should close on Escape', () => {
    const { store } = renderModal();

    pressEscape();

    expect(store.getState().optimizer.isOpen).toBe(false);
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('should stay open on a press on the backdrop', () => {
    const { store } = renderModal();

    pressBackdrop();

    expect(store.getState().optimizer.isOpen).toBe(true);
    expect(screen.getByRole('dialog')).toBeTruthy();
  });

  it('should name each option switch by its label and change it on a press', async () => {
    renderModal({ acknowledged: true });

    const mesh = await screen.findByRole('switch', { name: t('optimize.options.mesh.enabled') });
    const before = mesh.getAttribute('aria-checked');
    fireEvent.click(mesh);

    expect(mesh.getAttribute('aria-checked')).not.toBe(before);
    expect(screen.getAllByRole('switch')).toHaveLength(4);
  });

  it('should name the format list by its label', async () => {
    renderModal({ acknowledged: true });

    expect(
      await screen.findByRole('combobox', { name: t('optimize.options.textures.format') }),
    ).toBeTruthy();
  });

  it('should name the progress bar while the tools download', async () => {
    vi.mocked(optimizerPreload.tools).mockResolvedValue(toolsInfo('missing'));
    vi.mocked(optimizerPreload.installTools).mockReturnValue(new Promise(() => {}));
    renderModal({ acknowledged: true });

    expect(
      await screen.findByRole('progressbar', { name: t('optimize.tools.installing') }),
    ).toBeTruthy();
    expect(optimizerPreload.installTools).toHaveBeenCalledWith(project.path);
  });
});
