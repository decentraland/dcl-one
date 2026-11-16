import { Provider } from 'react-redux';
import { fireEvent, render, screen } from '@testing-library/react';
import { flatten } from 'flat';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { dark, ThemeProvider } from 'decentraland-ui2/dist/theme';

import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';
import { fetchProviders } from '/@/modules/store/ai';
import { createTestStore } from '../../../tests/utils/testStore';

import { DetachedPlaceholder } from './DetachedPlaceholder';

function renderPlaceholder(available: boolean) {
  const store = createTestStore();
  store.dispatch({
    type: fetchProviders.fulfilled.type,
    payload: [{ id: 'claude', label: 'Claude', available, managedSignIn: true }],
  });
  const onToggleRecording = vi.fn();
  render(
    <Provider store={store}>
      <ThemeProvider theme={dark}>
        <DetachedPlaceholder
          onDock={() => {}}
          width={320}
          onToggleRecording={onToggleRecording}
        />
      </ThemeProvider>
    </Provider>,
  );
  return { onToggleRecording };
}

describe('DetachedPlaceholder', () => {
  beforeAll(() => setCurrentLocale('en', flatten(en)));

  describe('when the chat is popped out and an assistant is available', () => {
    it('should offer the Record button here too', () => {
      const { onToggleRecording } = renderPlaceholder(true);
      fireEvent.click(screen.getByRole('button', { name: 'Record' }));
      expect(onToggleRecording).toHaveBeenCalled();
    });
  });

  describe('when no assistant is available', () => {
    it('should not start a recording, as the chat itself would not', () => {
      renderPlaceholder(false);
      expect((screen.getByRole('button', { name: 'Record' }) as HTMLButtonElement).disabled).toBe(
        true,
      );
    });
  });
});
