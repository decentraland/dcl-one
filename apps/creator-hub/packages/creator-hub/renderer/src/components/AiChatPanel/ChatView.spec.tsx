import { StrictMode } from 'react';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { flatten } from '/@/modules/store/translation/flatten';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';
import type { AiMessage } from '/@/modules/store/ai/types';

import { TestProviders } from '../../../tests/utils/TestProviders';
import { ChatView, type ChatViewProps } from './ChatView';

vi.mock('#preload', () => ({
  ai: {
    getPathForFile: vi.fn(),
    getMcpServerInfo: vi.fn(async () => ({ url: '', token: '' })),
  },
}));

const userMessage: AiMessage = {
  id: 'u-1',
  role: 'user',
  parts: [{ kind: 'text', text: 'add a door' }],
  done: true,
};

function chat(props: Partial<ChatViewProps>) {
  const noop = () => {};
  return (
    <TestProviders>
      <ChatView
        providers={[{ id: 'claude', label: 'Claude', available: true, managedSignIn: true }]}
        provider="claude"
        messages={[]}
        busy={false}
        detecting={false}
        selection={[]}
        assetMentions={[]}
        recording={null}
        onRemoveRecording={noop}
        billingDismissed
        sessions={[]}
        currentSessionId="s1"
        onSend={noop}
        onStop={noop}
        onNewChat={noop}
        onProviderChange={noop}
        onRevertTurn={noop}
        onAnswerPrompt={noop}
        onRecheck={noop}
        onDismissBilling={noop}
        onSwitchSession={noop}
        onDeleteSession={noop}
        onClearSelection={noop}
        onRemoveAssetMention={noop}
        onClose={noop}
        {...props}
      />
    </TestProviders>
  );
}

function renderChat(props: Partial<ChatViewProps>) {
  const result = render(chat(props));
  return { ...result, update: (next: Partial<ChatViewProps>) => result.rerender(chat(next)) };
}

describe('ChatView working indicator', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  afterEach(cleanup);

  describe('when a message was sent and the assistant has not started replying yet', () => {
    it('should show that the assistant is working', () => {
      renderChat({ busy: true, messages: [userMessage] });
      expect(screen.getByRole('status').textContent).toContain(en.editor.ai.thinking);
    });
  });

  describe('when the assistant has replied partway and is still working', () => {
    it('should keep showing that the assistant is working', () => {
      renderChat({
        busy: true,
        messages: [
          userMessage,
          {
            id: 't1',
            role: 'assistant',
            parts: [
              { kind: 'text', text: 'Placing the door now.' },
              { kind: 'tool', tool: 'place_smart_item', detail: '' },
            ],
            done: false,
          },
        ],
      });
      expect(screen.getByRole('status').textContent).toContain(en.editor.ai.thinking);
    });
  });

  describe('when the assistant is waiting for the user to answer its question', () => {
    it('should not claim the assistant is working', () => {
      renderChat({
        busy: true,
        messages: [
          userMessage,
          {
            id: 't1',
            role: 'assistant',
            parts: [
              {
                kind: 'prompt',
                prompt: {
                  id: 'q1',
                  question: 'Which door?',
                  options: [{ label: 'Wooden' }, { label: 'Metal' }],
                  multiSelect: false,
                  allowOther: false,
                },
              },
            ],
            done: false,
          },
        ],
      });
      expect(screen.queryByRole('status')).toBeNull();
    });
  });

  describe('when the turn has finished', () => {
    it('should not show the working indicator', () => {
      renderChat({
        busy: false,
        messages: [
          userMessage,
          { id: 't1', role: 'assistant', parts: [{ kind: 'text', text: 'Done.' }], done: true },
        ],
      });
      expect(screen.queryByRole('status')).toBeNull();
    });
  });
});

describe('ChatView asset mentions', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  afterEach(() => {
    cleanup();
  });

  describe('when two assets are mentioned', () => {
    it('should show each asset by file name', () => {
      renderChat({
        assetMentions: [
          { path: 'assets/asset-packs/dance/Dance_emote.glb' },
          { path: 'assets/models/Throw.glb' },
        ],
      });
      expect(screen.getByText('Asset: Dance_emote.glb')).toBeTruthy();
      expect(screen.getByText('Asset: Throw.glb')).toBeTruthy();
    });

    it('should remove only the asset whose remove button is clicked', () => {
      const onRemoveAssetMention = vi.fn();
      renderChat({
        assetMentions: [
          { path: 'assets/asset-packs/dance/Dance_emote.glb' },
          { path: 'assets/models/Throw.glb' },
        ],
        onRemoveAssetMention,
      });
      fireEvent.click(screen.getAllByRole('button', { name: 'Remove' })[1]);
      expect(onRemoveAssetMention).toHaveBeenCalledWith({ path: 'assets/models/Throw.glb' });
    });
  });

  describe('when a built-in animation is mentioned', () => {
    it('should show it by its emote name', () => {
      renderChat({ assetMentions: [{ builtInEmote: 'wave' }] });
      expect(screen.getByText('Built-in animation: wave')).toBeTruthy();
    });
  });
});

describe('ChatView chat history menu', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  afterEach(() => {
    cleanup();
  });

  describe('when a saved chat is deleted with the keyboard', () => {
    it('should delete that chat from its menu item and keep the focus in the menu', () => {
      const onDeleteSession = vi.fn();
      const now = Date.now();
      renderChat({
        sessions: [
          { id: 's1', title: 'Door', updatedAt: now },
          { id: 's2', title: 'Window', updatedAt: now },
        ],
        onDeleteSession,
      });
      fireEvent.click(screen.getByRole('button', { name: 'New chat' }));
      const door = screen.getByRole('menuitem', { name: /Door/ });
      const window = screen.getByRole('menuitem', { name: /Window/ });

      expect(door.getAttribute('aria-keyshortcuts')).toBe('Delete');
      door.focus();
      fireEvent.keyDown(door, { key: 'Delete' });

      expect(onDeleteSession).toHaveBeenCalledExactlyOnceWith('s1');
      expect(document.activeElement).toBe(window);
    });
  });
});

describe('ChatView session recorder', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  afterEach(cleanup);

  describe('when a session recording is pending', () => {
    const recording = {
      sessions: [
        {
          dir: '/tmp/recordings/s1',
          durationMs: 42_000,
          actionCount: 18,
          context: '[Session recording] …',
        },
      ],
      durationMs: 42_000,
      actionCount: 18,
    };

    const twoRecordings = {
      sessions: [
        recording.sessions[0],
        {
          dir: '/tmp/recordings/s2',
          durationMs: 708_000,
          actionCount: 22,
          context: '[Session recording 2] …',
        },
      ],
      durationMs: 750_000,
      actionCount: 40,
    };

    it('should show it with its duration and action count', () => {
      renderChat({ recording });

      expect(screen.getByText('Session recording · 0:42 · 18 actions')).toBeTruthy();
    });

    it('should show one chip for every recording waiting to go out', () => {
      renderChat({ recording: twoRecordings });

      expect(screen.getByText('2 recordings · 12:30 · 40 actions')).toBeTruthy();
      expect(screen.getAllByTestId('ai-recording-chip')).toHaveLength(1);
    });

    it('should say when one of several carried no speech', () => {
      renderChat({ recording: { ...twoRecordings, transcriptionError: 'no speech found' } });

      expect(screen.getByText('2 recordings · 12:30 · 40 actions (no speech)')).toBeTruthy();
    });

    it('should say when it carried no speech', () => {
      renderChat({ recording: { ...recording, transcriptionError: 'no speech found' } });

      expect(screen.getByText('Session recording · 0:42 · 18 actions (no speech)')).toBeTruthy();
    });

    it('should discard it from the remove button', () => {
      const onRemoveRecording = vi.fn();
      renderChat({ recording, onRemoveRecording });

      fireEvent.click(screen.getByRole('button', { name: 'Discard recording' }));

      expect(onRemoveRecording).toHaveBeenCalled();
    });

    it('should let the user send it without typing anything', () => {
      const onSend = vi.fn();
      renderChat({ recording, onSend });

      fireEvent.click(screen.getByRole('button', { name: 'Send' }));

      expect(onSend).toHaveBeenCalledWith('', undefined);
    });
  });

  describe('when the panel offers the Record button', () => {
    const idle = {
      status: 'idle' as const,
      startedAt: null,
      stoppedAt: null,
      micLabel: '',
      tools: null,
      actionCount: 0,
    };

    it('should show it in the composer and pass presses on', () => {
      const onToggleRecording = vi.fn();
      renderChat({ recorder: idle, onToggleRecording });

      fireEvent.click(screen.getByRole('button', { name: 'Record' }));

      expect(onToggleRecording).toHaveBeenCalled();
      expect(screen.queryByTestId('recording-card')).toBeNull();
    });

    it('should not start a recording while no assistant is available', () => {
      renderChat({
        providers: [{ id: 'claude', label: 'Claude', available: false, managedSignIn: true }],
        recorder: idle,
        onToggleRecording: vi.fn(),
      });

      expect((screen.getByRole('button', { name: 'Record' }) as HTMLButtonElement).disabled).toBe(
        true,
      );
    });

    describe('and a recording is running', () => {
      const recording = () => ({
        ...idle,
        status: 'recording' as const,
        startedAt: Date.now() - 42_000,
        actionCount: 7,
      });

      it('should show a card with the time, the limit and the actions captured', () => {
        renderChat({ recorder: recording(), onToggleRecording: vi.fn() });

        const card = screen.getByRole('group', { name: 'Recording' });
        expect(within(card).getByRole('timer').textContent).toBe('0:42');
        expect(within(card).getByText('/ 10:00')).toBeTruthy();
        expect(within(card).getByText('7 actions captured')).toBeTruthy();
        expect(screen.queryByRole('button', { name: 'Record' })).toBeNull();
      });

      it('should put the keyboard on Stop when the recording has just started', () => {
        const { update } = renderChat({ recorder: idle, onToggleRecording: vi.fn() });

        update({ recorder: recording(), onToggleRecording: vi.fn() });

        expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Stop recording' }));
      });

      it('should leave the keyboard alone when the panel opens mid-recording', () => {
        renderChat({ recorder: recording(), onToggleRecording: vi.fn() });

        expect(document.activeElement).not.toBe(
          screen.getByRole('button', { name: 'Stop recording' }),
        );
      });

      it('should put the keyboard back in the composer when the card goes away', () => {
        const { update } = renderChat({ recorder: recording(), onToggleRecording: vi.fn() });
        screen.getByRole('button', { name: 'Stop recording' }).focus();

        update({
          recorder: { ...idle, status: 'stopping' as const, startedAt: Date.now() - 1000 },
          onToggleRecording: vi.fn(),
        });

        expect(document.activeElement).toBe(screen.getByRole('textbox'));
      });

      it('should stop from the card', () => {
        const onToggleRecording = vi.fn();
        renderChat({ recorder: recording(), onToggleRecording });

        fireEvent.click(screen.getByRole('button', { name: 'Stop recording' }));

        expect(onToggleRecording).toHaveBeenCalled();
      });

      it('should discard only after the user confirms', () => {
        const onCancelRecording = vi.fn();
        renderChat({ recorder: recording(), onToggleRecording: vi.fn(), onCancelRecording });

        fireEvent.click(screen.getByRole('button', { name: 'Discard' }));
        const dialog = screen.getByRole('dialog', { name: 'Discard this recording?' });
        expect(onCancelRecording).not.toHaveBeenCalled();

        fireEvent.click(within(dialog).getByRole('button', { name: 'Discard' }));
        expect(onCancelRecording).toHaveBeenCalledTimes(1);
      });

      it('should keep recording when the user backs out of the confirmation', () => {
        const onCancelRecording = vi.fn();
        renderChat({ recorder: recording(), onToggleRecording: vi.fn(), onCancelRecording });

        fireEvent.click(screen.getByRole('button', { name: 'Discard' }));
        fireEvent.click(screen.getByRole('button', { name: 'Keep recording' }));

        expect(onCancelRecording).not.toHaveBeenCalled();
        expect(screen.getByTestId('recording-card')).toBeTruthy();
      });
    });

    describe('and the recording is being transcribed', () => {
      const stopping = () => ({
        ...idle,
        status: 'stopping' as const,
        startedAt: Date.now() - 42_000,
        stoppedAt: Date.now(),
      });

      it('should show the progress above a composer that stays usable', () => {
        const onStop = vi.fn();
        renderChat({ recorder: stopping(), busy: true, onToggleRecording: vi.fn(), onStop });

        const card = screen.getByRole('group', { name: 'Transcribing your recording' });
        expect(within(card).getByText('0:42')).toBeTruthy();
        expect(screen.getByRole('textbox')).toBeTruthy();

        fireEvent.click(screen.getByRole('button', { name: 'Stop' }));
        expect(onStop).toHaveBeenCalled();
      });

      it('should say it is transcribing only in the card', () => {
        renderChat({ recorder: stopping(), onToggleRecording: vi.fn() });

        expect(screen.queryAllByText(/^Transcribing/)).toHaveLength(1);
      });

      it('should put the card where the recording card was, above the composer', () => {
        renderChat({ recorder: stopping(), onToggleRecording: vi.fn() });

        const card = screen.getByTestId('recording-transcribing');
        const composer = screen.getByRole('textbox');
        expect(composer.contains(card)).toBe(false);
        expect(
          card.compareDocumentPosition(composer) & Node.DOCUMENT_POSITION_FOLLOWING,
        ).toBeTruthy();
      });

      it('should offer sending the recording without waiting for the transcript', () => {
        const onSkipTranscription = vi.fn();
        renderChat({ recorder: stopping(), onToggleRecording: vi.fn(), onSkipTranscription });

        fireEvent.click(screen.getByRole('button', { name: 'Send without transcript' }));

        expect(onSkipTranscription).toHaveBeenCalled();
      });
    });
  });

  describe('when the Record button is not wired', () => {
    it('should not show it', () => {
      renderChat({});

      expect(screen.queryByTestId('editor-record-button')).toBeNull();
    });
  });
});

describe('ChatView draft prompt', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  afterEach(cleanup);

  const composer = () => screen.getByRole('textbox') as HTMLTextAreaElement;
  const nextFrame = () => new Promise(resolve => requestAnimationFrame(resolve));

  describe('when a transcript arrives and the user had already typed', () => {
    it('should add it after what they wrote', () => {
      const { update } = renderChat({});
      fireEvent.change(composer(), { target: { value: 'make it blue' } });

      update({ draftPrompt: { text: 'and taller', nonce: 1, mode: 'append' } });

      expect(composer().value).toBe('make it blue and taller');
    });
  });

  describe('when a transcript arrives and the composer is empty', () => {
    it('should become the whole draft', () => {
      const { update } = renderChat({});

      update({ draftPrompt: { text: 'add a door', nonce: 1, mode: 'append' } });

      expect(composer().value).toBe('add a door');
    });

    it('should be consumed so it only lands once', () => {
      const onDraftConsumed = vi.fn();
      const { update } = renderChat({ onDraftConsumed });

      update({ draftPrompt: { text: 'add a door', nonce: 1, mode: 'append' }, onDraftConsumed });

      expect(onDraftConsumed).toHaveBeenCalledTimes(1);
    });
  });

  describe('when the user is working outside the chat', () => {
    it('should leave the focus where it is', async () => {
      const outside = document.createElement('button');
      document.body.appendChild(outside);
      const { update } = renderChat({});
      outside.focus();

      update({ draftPrompt: { text: 'add a door', nonce: 1, mode: 'append' } });
      await nextFrame();

      expect(document.activeElement).toBe(outside);
      outside.remove();
    });
  });

  describe('when the user is already in the chat', () => {
    it('should put them back in the composer', async () => {
      const { update } = renderChat({});
      composer().focus();

      update({ draftPrompt: { text: 'add a door', nonce: 1, mode: 'append' } });
      await nextFrame();

      expect(document.activeElement).toBe(composer());
    });
  });

  describe('when another part of the editor seeds a prompt', () => {
    it('should replace the draft and take the focus', async () => {
      const outside = document.createElement('button');
      document.body.appendChild(outside);
      const { update } = renderChat({});
      fireEvent.change(composer(), { target: { value: 'typed' } });
      outside.focus();

      update({ draftPrompt: { text: 'describe a reaction', nonce: 1, mode: 'replace' } });
      await nextFrame();

      expect(composer().value).toBe('describe a reaction');
      expect(document.activeElement).toBe(composer());
      outside.remove();
    });
  });

  describe('when React double-invokes the effect', () => {
    it('should apply the transcript once', () => {
      render(
        <StrictMode>
          {chat({ draftPrompt: { text: 'add a door', nonce: 1, mode: 'append' } })}
        </StrictMode>,
      );

      expect((screen.getByRole('textbox') as HTMLTextAreaElement).value).toBe('add a door');
    });
  });
});
