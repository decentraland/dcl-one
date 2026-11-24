import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { AppSettings } from '/shared/types/settings';
import type { EditorConfig } from '/shared/types/config';

import { TestProviders } from '../../../../../../tests/utils/TestProviders';
import EditorTab from './component';

vi.mock('/@/modules/store/translation/utils', async importOriginal => ({
  ...((await importOriginal()) as object),
  t: (id: string, values?: Record<string, string>) => (values?.name ? `${id} ${values.name}` : id),
}));

const VS_CODE: EditorConfig = { name: 'VS Code', path: '/apps/code', isDefault: true };
const ZED: EditorConfig = { name: 'Zed', path: '/apps/zed' };

const SETTINGS = { previewOptions: {} } as AppSettings;

describe('EditorTab', () => {
  let onSetDefaultEditor: ReturnType<typeof vi.fn<(path: string) => void>>;
  let onAddEditor: ReturnType<typeof vi.fn<(path: string) => void>>;
  let onRemoveEditor: ReturnType<typeof vi.fn<(path: string) => void>>;
  let onSelectEditorPath: ReturnType<typeof vi.fn<() => Promise<string | null>>>;

  function renderTab(editors: EditorConfig[]) {
    render(
      <TestProviders>
        <EditorTab
          settings={SETTINGS}
          updateSettings={vi.fn()}
          editors={editors}
          loading={false}
          onSetDefaultEditor={onSetDefaultEditor}
          onAddEditor={onAddEditor}
          onRemoveEditor={onRemoveEditor}
          onSelectEditorPath={onSelectEditorPath}
        />
      </TestProviders>,
    );
  }

  beforeEach(() => {
    onSetDefaultEditor = vi.fn<(path: string) => void>();
    onAddEditor = vi.fn<(path: string) => void>();
    onRemoveEditor = vi.fn<(path: string) => void>();
    onSelectEditorPath = vi.fn<() => Promise<string | null>>().mockResolvedValue('/apps/new');
  });

  afterEach(cleanup);

  describe('when no editor is set', () => {
    it('should show the translated placeholder', () => {
      renderTab([]);

      expect(screen.getByRole('combobox').textContent).toBe(
        'modal.app_settings.fields.code_editor.placeholder',
      );
    });
  });

  describe('when the keyboard picks the last option', () => {
    it('should add an editor from the device, not set a default', async () => {
      renderTab([VS_CODE, ZED]);
      const combobox = screen.getByRole('combobox');

      fireEvent.keyDown(combobox, { key: 'End' });
      expect(
        within(screen.getByRole('listbox'))
          .getAllByRole('option')
          .map(option => option.textContent),
      ).toEqual(['VS Code', 'Zed', 'modal.app_settings.fields.code_editor.choose_device']);
      fireEvent.keyDown(combobox, { key: 'Enter' });

      await waitFor(() => expect(onAddEditor).toHaveBeenCalledWith('/apps/new'));
      expect(onSelectEditorPath).toHaveBeenCalledTimes(1);
      expect(onSetDefaultEditor).not.toHaveBeenCalled();
    });
  });

  describe('when the keyboard picks an editor', () => {
    it('should make it the default editor', () => {
      renderTab([VS_CODE, ZED]);
      const combobox = screen.getByRole('combobox');

      fireEvent.keyDown(combobox, { key: 'Home' });
      fireEvent.keyDown(combobox, { key: 'ArrowDown' });
      fireEvent.keyDown(combobox, { key: 'Enter' });

      expect(onSetDefaultEditor).toHaveBeenCalledExactlyOnceWith('/apps/zed');
    });
  });

  describe('when an editor is removed', () => {
    it('should offer a remove button for each editor outside the list box', () => {
      renderTab([VS_CODE, ZED]);
      const list = screen.getByRole('list', { name: 'modal.app_settings.fields.code_editor.list' });

      const remove = within(list).getByRole('button', {
        name: 'modal.app_settings.fields.code_editor.remove Zed',
      });
      remove.focus();
      expect(document.activeElement).toBe(remove);
      fireEvent.click(remove);

      expect(onRemoveEditor).toHaveBeenCalledExactlyOnceWith('/apps/zed');
      expect(within(list).getAllByRole('button')).toHaveLength(2);
    });
  });
});
