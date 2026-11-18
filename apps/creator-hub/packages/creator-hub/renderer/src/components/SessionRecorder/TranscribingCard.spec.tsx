import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { flatten } from '/@/modules/store/translation/flatten';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi, type Mock } from 'vitest';

import type { RecordingToolsStatus } from '/shared/types/recording';
import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';

import { TestProviders } from '../../../tests/utils/TestProviders';

import { TranscribingCard } from './TranscribingCard';

describe('TranscribingCard', () => {
  let onSkip: Mock<() => void>;
  let tools: RecordingToolsStatus | null;

  function renderCard(stoppedAt?: number) {
    const startedAt = Date.now() - 42_000;
    render(
      <TestProviders>
        <TranscribingCard
          startedAt={startedAt}
          stoppedAt={stoppedAt ?? Date.now()}
          actionCount={3}
          tools={tools}
          onSkip={onSkip}
        />
      </TestProviders>,
    );
  }

  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  beforeEach(() => {
    onSkip = vi.fn<() => void>();
    tools = { state: 'ready' };
  });

  afterEach(cleanup);

  describe('when the speech tools are ready', () => {
    beforeEach(() => renderCard());

    it('should say it is transcribing, for how long it recorded and what was captured', () => {
      const card = screen.getByRole('group', { name: 'Transcribing your recording' });
      expect(card.textContent).toContain('Turning your speech into text on this computer');
      expect(card.textContent).toContain('0:42');
      expect(card.textContent).toContain('3 actions captured');
    });

    it('should show progress without a percentage', () => {
      expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBeNull();
    });
  });

  describe('when the speech model is still downloading', () => {
    beforeEach(() => {
      tools = { state: 'installing', message: 'Downloading', percent: 45 };
      renderCard();
    });

    it('should say it is getting ready and why', () => {
      const card = screen.getByRole('group', { name: 'Getting ready to transcribe' });
      expect(card.textContent).toContain('Downloading the speech model (one time)');
    });

    it('should show how far the download is', () => {
      expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('45');
    });
  });

  describe('when the creator sends without waiting', () => {
    beforeEach(() => {
      renderCard();
      fireEvent.click(screen.getByRole('button', { name: 'Send without transcript' }));
    });

    it('should give up on the transcript', () => {
      expect(onSkip).toHaveBeenCalled();
    });
  });

  describe('when the panel is opened while the transcription runs', () => {
    it('should show the length the recording had, not the time since it stopped', () => {
      const startedAt = Date.now() - 600_000;
      render(
        <TestProviders>
          <TranscribingCard
            startedAt={startedAt}
            stoppedAt={startedAt + 42_000}
            actionCount={3}
            tools={{ state: 'ready' }}
            onSkip={onSkip}
          />
        </TestProviders>,
      );

      const card = screen.getByRole('group', { name: 'Transcribing your recording' });
      expect(card.textContent).toContain('0:42');
    });
  });
});
