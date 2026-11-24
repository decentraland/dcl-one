import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import en from '/@/modules/store/translation/locales/en.json';
import es from '/@/modules/store/translation/locales/es.json';
import zh from '/@/modules/store/translation/locales/zh.json';
import { RENDERER } from '/shared/types/settings';
import { editor } from '#preload';
import { RealmServerStopped } from '.';

function lookup(locale: object, id: string): unknown {
  return id.split('.').reduce<any>((node, key) => node?.[key], locale);
}

vi.mock('/@/modules/store/translation/utils', () => ({
  t: (id: string) => lookup(en, id) ?? id,
}));

const state = vi.hoisted(() => ({ projectPath: '/scenes/demo', renderer: 'bevy' }));

vi.mock('/@/hooks/useEditor', () => ({
  useEditor: () => ({ project: { path: state.projectPath } }),
}));
vi.mock('/@/hooks/useSettings', () => ({
  useSettings: () => ({ settings: { renderer: state.renderer } }),
}));

describe('RealmServerStopped', () => {
  let onClose: ReturnType<typeof vi.fn<() => void>>;

  beforeEach(() => {
    state.projectPath = '/scenes/demo';
    state.renderer = RENDERER.BEVY;
    onClose = vi.fn<() => void>();
    vi.mocked(editor.ensureBevyRealm).mockResolvedValue({
      url: 'http://localhost:8005',
      wsUrl: 'ws://localhost:8005/data-layer',
    });
    render(
      <RealmServerStopped
        path="/scenes/demo"
        onClose={onClose}
      />,
    );
  });

  afterEach(() => {
    cleanup();
  });

  it.each([
    ['en', en],
    ['es', es],
    ['zh', zh],
  ])('should have every label in %s', (_name, locale) => {
    for (const key of ['title', 'actions.dismiss', 'actions.retry']) {
      expect(lookup(locale, `snackbar.realm_server_stopped.${key}`)).toEqual(expect.any(String));
    }
  });

  describe('when Retry is clicked', () => {
    beforeEach(() => {
      fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    });

    it('should ask main for a serving realm of the scene', () => {
      expect(editor.ensureBevyRealm).toHaveBeenCalledWith('/scenes/demo');
    });

    it('should close the notice', () => {
      expect(onClose).toHaveBeenCalled();
    });
  });

  describe('when Retry is clicked after the editor left the scene', () => {
    beforeEach(() => {
      cleanup();
      state.projectPath = '/scenes/other';
      render(
        <RealmServerStopped
          path="/scenes/demo"
          onClose={onClose}
        />,
      );
      fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    });

    it('should close the notice without starting a realm nobody owns', () => {
      expect(onClose).toHaveBeenCalled();
      expect(editor.ensureBevyRealm).not.toHaveBeenCalled();
    });
  });

  describe('when Dismiss is clicked', () => {
    beforeEach(() => {
      fireEvent.click(screen.getByRole('button', { name: 'Dismiss' }));
    });

    it('should close the notice without restarting the realm', () => {
      expect(onClose).toHaveBeenCalled();
      expect(editor.ensureBevyRealm).not.toHaveBeenCalled();
    });
  });
});
