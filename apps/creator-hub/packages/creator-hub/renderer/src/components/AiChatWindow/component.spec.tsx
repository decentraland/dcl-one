import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { flatten } from 'flat';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { dark, ThemeProvider } from 'decentraland-ui2/dist/theme';

import type { AiMirrorState } from '/shared/types/ai';

import { ai as aiPreload } from '#preload';
import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';

vi.mock('#store', () => ({ useDispatch: () => vi.fn() }));

const { AiChatWindow } = await import('./component');

const MIRROR: AiMirrorState = {
  providers: [{ id: 'claude', label: 'Claude', available: true, managedSignIn: true } as never],
  provider: 'claude',
  model: 'default',
  messages: [],
  busy: false,
  detecting: false,
  selection: [],
  assetMentions: [],
  recording: null,
  recorder: { status: 'idle', startedAt: null, micLabel: '', tools: null, actionCount: 0 },
  billingDismissed: true,
  sessions: [],
  currentSessionId: 's1',
};

describe('the popped-out chat window', () => {
  let push: (mirror: AiMirrorState) => void;

  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  beforeEach(() => {
    vi.mocked(aiPreload.getMcpServerInfo).mockResolvedValue({ url: '', token: '' });
    vi.mocked(aiPreload.onAiMirrorState).mockImplementation((cb: typeof push) => {
      push = cb;
      return { cleanup: vi.fn() };
    });
    render(
      <ThemeProvider theme={dark}>
        <AiChatWindow />
      </ThemeProvider>,
    );
  });

  afterEach(cleanup);

  describe('when a recording made here is transcribed', () => {
    beforeEach(() => {
      act(() =>
        push({ ...MIRROR, draftPrompt: { text: 'make the door red', nonce: 1, mode: 'append' } }),
      );
    });

    it('should put the transcript in its own composer', () => {
      expect((screen.getByRole('textbox') as HTMLTextAreaElement).value).toBe('make the door red');
    });

    it('should tell the editor window the draft was used', () => {
      expect(aiPreload.sendAiRemoteCommand).toHaveBeenCalledWith({ type: 'consumeDraftPrompt' });
    });
  });

  describe('when a recording made here is still transcribing', () => {
    it('should ask the editor window to send it without the transcript', () => {
      act(() =>
        push({
          ...MIRROR,
          recorder: { ...MIRROR.recorder!, status: 'stopping', startedAt: Date.now() - 1000 },
        }),
      );

      fireEvent.click(screen.getByRole('button', { name: 'Send without transcript' }));

      expect(aiPreload.sendAiRemoteCommand).toHaveBeenCalledWith({ type: 'skipTranscription' });
    });
  });
});
