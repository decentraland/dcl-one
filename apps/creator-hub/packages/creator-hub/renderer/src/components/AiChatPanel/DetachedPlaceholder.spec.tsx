import { Provider } from 'react-redux';
import { fireEvent, render, screen, within } from '@testing-library/react';
import { flatten } from '/@/modules/store/translation/flatten';
import { beforeAll, describe, expect, it, vi } from 'vitest';

import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';

import { TestProviders } from '../../../tests/utils/TestProviders';
import { actions as aiActions, fetchProviders } from '/@/modules/store/ai';
import { createTestStore } from '../../../tests/utils/testStore';

import { DetachedPlaceholder } from './DetachedPlaceholder';

function renderPlaceholder(available: boolean, recording = false) {
  const store = createTestStore();
  if (recording) {
    store.dispatch(
      aiActions.setRecorderState({
        status: 'recording',
        startedAt: Date.now(),
        stoppedAt: null,
        micLabel: '',
        tools: null,
        actionCount: 3,
        endedWith: null,
      }),
    );
  }
  store.dispatch({
    type: fetchProviders.fulfilled.type,
    payload: [{ id: 'claude', label: 'Claude', available, managedSignIn: true }],
  });
  const onToggleRecording = vi.fn();
  const onCancelRecording = vi.fn();
  const onSendRecording = vi.fn();
  render(
    <Provider store={store}>
      <TestProviders>
        <DetachedPlaceholder
          onDock={() => {}}
          width={320}
          onToggleRecording={onToggleRecording}
          onCancelRecording={onCancelRecording}
          onSendRecording={onSendRecording}
        />
      </TestProviders>
    </Provider>,
  );
  return { onToggleRecording, onCancelRecording, onSendRecording };
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

  describe('when a recording is running', () => {
    it('should show the recording bar, with its Stop, its Send and its confirmed Discard', () => {
      const { onToggleRecording, onCancelRecording, onSendRecording } = renderPlaceholder(
        true,
        true,
      );
      expect(screen.getByRole('group', { name: 'Recording' })).toBeTruthy();
      fireEvent.click(screen.getByRole('button', { name: 'Stop recording' }));
      expect(onToggleRecording).toHaveBeenCalled();
      fireEvent.click(screen.getByRole('button', { name: 'Stop and send' }));
      expect(onSendRecording).toHaveBeenCalled();
      fireEvent.click(screen.getByRole('button', { name: 'Discard recording' }));
      fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Discard' }));
      expect(onCancelRecording).toHaveBeenCalled();
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
