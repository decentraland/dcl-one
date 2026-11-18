import { fireEvent, render, screen } from '@testing-library/react';
import { flatten } from 'flat';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { dark, ThemeProvider } from 'decentraland-ui2/dist/theme';

import type { AiSharingView } from '/shared/types/ai';

import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';

import { SharingConsentCard, SharingStatusChip, TurnRating } from './SharingConsent';

const copy = en.editor.ai.sharing;

function renderBoth(sharing: AiSharingView) {
  const handlers = {
    onShareDecision: vi.fn(),
    onShareNotNow: vi.fn(),
    onSetConversationShared: vi.fn(),
  };
  render(
    <ThemeProvider theme={dark}>
      <SharingStatusChip
        sharing={sharing}
        {...handlers}
      />
      <SharingConsentCard
        sharing={sharing}
        {...handlers}
      />
    </ThemeProvider>,
  );
  return handlers;
}

beforeAll(() => {
  setCurrentLocale('en', flatten(en));
});

describe('when the creator has not been asked yet', () => {
  const view: AiSharingView = { status: 'ask', conversationExcluded: false };

  it('should offer both answers as equal buttons', () => {
    renderBoth(view);

    expect(screen.getByRole('button', { name: copy.accept })).toBeTruthy();
    expect(screen.getByRole('button', { name: copy.decline })).toBeTruthy();
  });

  it('should record a yes', () => {
    const { onShareDecision } = renderBoth(view);
    fireEvent.click(screen.getByRole('button', { name: copy.accept }));

    expect(onShareDecision).toHaveBeenCalledWith(true);
  });

  it('should record a no', () => {
    const { onShareDecision } = renderBoth(view);
    fireEvent.click(screen.getByRole('button', { name: copy.decline }));

    expect(onShareDecision).toHaveBeenCalledWith(false);
  });

  it('should treat closing the card as "not now", not as an answer', () => {
    const { onShareDecision, onShareNotNow } = renderBoth(view);
    fireEvent.click(screen.getByRole('button', { name: copy.not_now }));

    expect(onShareNotNow).toHaveBeenCalled();
    expect(onShareDecision).not.toHaveBeenCalled();
  });

  it('should list what is shared, what is not, how it is analysed and what may be published on request', () => {
    renderBoth(view);
    expect(screen.queryByText(copy.details_not_shared)).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: copy.details_toggle }));

    expect(screen.getByText(copy.details_shared)).toBeTruthy();
    expect(screen.getByText(copy.details_not_shared)).toBeTruthy();
    expect(screen.getByText(copy.details_analysis)).toBeTruthy();
    expect(screen.getByText(copy.details_publish)).toBeTruthy();
  });

  it('should not show the sharing chip', () => {
    renderBoth(view);

    expect(screen.queryByRole('button', { name: `${copy.chip}: ${copy.chip_menu}` })).toBeNull();
  });
});

describe('when the creator is signed out', () => {
  it('should ask them to sign in and offer no way to say yes', () => {
    renderBoth({ status: 'signin', conversationExcluded: false });

    expect(screen.getByText(copy.signin_body)).toBeTruthy();
    expect(screen.queryByRole('button', { name: copy.accept })).toBeNull();
  });
});

describe('when sharing is on', () => {
  it('should show the chip and no card', () => {
    renderBoth({ status: 'on', conversationExcluded: false });

    expect(screen.getByRole('button', { name: `${copy.chip}: ${copy.chip_menu}` })).toBeTruthy();
    expect(screen.queryByRole('button', { name: copy.accept })).toBeNull();
  });

  it('should let the creator stop sharing this conversation', () => {
    const { onSetConversationShared } = renderBoth({ status: 'on', conversationExcluded: false });
    fireEvent.click(screen.getByRole('button', { name: `${copy.chip}: ${copy.chip_menu}` }));
    fireEvent.click(screen.getByRole('menuitem', { name: copy.menu_exclude }));

    expect(onSetConversationShared).toHaveBeenCalledWith(false);
  });

  it('should let the creator share an excluded conversation again', () => {
    const { onSetConversationShared } = renderBoth({ status: 'on', conversationExcluded: true });
    fireEvent.click(
      screen.getByRole('button', { name: `${copy.chip_excluded}: ${copy.chip_menu}` }),
    );
    fireEvent.click(screen.getByRole('menuitem', { name: copy.menu_include }));

    expect(onSetConversationShared).toHaveBeenCalledWith(true);
  });

  it('should let the creator stop sharing altogether', () => {
    const { onShareDecision } = renderBoth({ status: 'on', conversationExcluded: false });
    fireEvent.click(screen.getByRole('button', { name: `${copy.chip}: ${copy.chip_menu}` }));
    fireEvent.click(screen.getByRole('menuitem', { name: copy.menu_stop }));

    expect(onShareDecision).toHaveBeenCalledWith(false);
  });
});

describe('when the feature is hidden', () => {
  it('should render nothing', () => {
    renderBoth({ status: 'hidden', conversationExcluded: false });

    expect(screen.queryByRole('button')).toBeNull();
  });
});

describe('when rating a shared reply', () => {
  const on: AiSharingView = { status: 'on', conversationExcluded: false };

  function renderRating(props: Partial<Parameters<typeof TurnRating>[0]> = {}) {
    const onRate = vi.fn();
    render(
      <ThemeProvider theme={dark}>
        <TurnRating
          sharing={on}
          recordId="r1"
          onRate={onRate}
          {...props}
        />
      </ThemeProvider>,
    );
    return onRate;
  }

  it('should offer thumbs up and down', () => {
    const onRate = renderRating();
    fireEvent.click(screen.getByRole('button', { name: copy.rate_up }));

    expect(onRate).toHaveBeenCalledWith('up');
    expect(screen.getByRole('button', { name: copy.rate_down }).getAttribute('aria-pressed')).toBe(
      'false',
    );
  });

  it('should clear the rating when the chosen thumb is pressed again', () => {
    const onRate = renderRating({ rating: 'down' });
    const down = screen.getByRole('button', { name: copy.rate_down });

    expect(down.getAttribute('aria-pressed')).toBe('true');
    fireEvent.click(down);
    expect(onRate).toHaveBeenCalledWith(null);
  });

  it('should show nothing for a reply that was not shared', () => {
    renderRating({ recordId: undefined });

    expect(screen.queryByRole('button')).toBeNull();
  });

  it('should show nothing in a conversation marked as not shared', () => {
    renderRating({ sharing: { status: 'on', conversationExcluded: true } });

    expect(screen.queryByRole('button')).toBeNull();
  });
});
