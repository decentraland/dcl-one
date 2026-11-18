import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { flatten } from '/@/modules/store/translation/flatten';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi, type Mock } from 'vitest';

import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';

import { TestProviders } from '../../../tests/utils/TestProviders';

const session = vi.hoisted(() => ({
  pauseHubCapture: vi.fn<() => () => void>(() => () => {}),
  onRecordedActionCount: vi.fn(() => () => {}),
}));
vi.mock('/@/modules/recording/session', () => session);
vi.mock('/@/modules/recording/level', () => ({ onMicLevel: () => () => {} }));

const { RecordingCard } = await import('./RecordingCard');

describe('RecordingCard', () => {
  let onDiscard: Mock<() => void>;
  let resume: Mock<() => void>;

  function renderCard() {
    render(
      <TestProviders>
        <RecordingCard
          startedAt={Date.now() - 42_000}
          micLabel="Built-in mic"
          actionCount={3}
          onStop={vi.fn()}
          onDiscard={onDiscard}
        />
      </TestProviders>,
    );
  }

  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  beforeEach(() => {
    onDiscard = vi.fn<() => void>();
    resume = vi.fn<() => void>();
    session.pauseHubCapture.mockReset().mockReturnValue(resume);
  });

  afterEach(cleanup);

  describe('when the creator is asked to confirm a discard', () => {
    it('should keep the question out of what the recording captured', () => {
      renderCard();
      expect(session.pauseHubCapture).not.toHaveBeenCalled();

      fireEvent.click(screen.getByRole('button', { name: 'Discard' }));

      expect(session.pauseHubCapture).toHaveBeenCalledTimes(1);
      expect(resume).not.toHaveBeenCalled();
    });

    it('should record again once they back out of it', () => {
      renderCard();
      fireEvent.click(screen.getByRole('button', { name: 'Discard' }));

      fireEvent.click(screen.getByRole('button', { name: 'Keep recording' }));

      expect(resume).toHaveBeenCalled();
    });
  });
});
