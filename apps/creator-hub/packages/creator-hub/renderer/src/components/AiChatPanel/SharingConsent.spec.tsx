import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { fireEvent, render, screen } from '@testing-library/react';
import { beforeAll, describe, expect, it, vi } from 'vitest';

import type { AiSharingView } from '/shared/types/ai';

import { en } from '/@/modules/store/translation/locales';
import { flatten } from '/@/modules/store/translation/flatten';
import { setCurrentLocale } from '/@/modules/store/translation/utils';

import { SharingConsentCard, SharingStatusChip, TurnRating } from './SharingConsent';
import styles from './SharingConsent.module.css';

const copy = en.editor.ai.sharing;

function renderBoth(sharing: AiSharingView) {
  const handlers = {
    onShareDecision: vi.fn(),
    onShareNotNow: vi.fn(),
    onSetConversationShared: vi.fn(),
  };
  render(
    <>
      <SharingStatusChip
        sharing={sharing}
        {...handlers}
      />
      <SharingConsentCard
        sharing={sharing}
        {...handlers}
      />
    </>,
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

  const openMenu = (excluded = false) =>
    fireEvent.click(
      screen.getByRole('button', {
        name: `${excluded ? copy.chip_excluded : copy.chip}: ${copy.chip_menu}`,
      }),
    );

  it('should show a dot and "Sharing", and both switches on', () => {
    renderBoth({ status: 'on', conversationExcluded: false });
    openMenu();

    expect(
      screen.getByRole('switch', { name: copy.menu_share_chat }).getAttribute('aria-checked'),
    ).toBe('true');
    expect(
      screen.getByRole('switch', { name: copy.menu_share_all }).getAttribute('aria-checked'),
    ).toBe('true');
  });

  // The bare Popover clears its own surface with a rule that outranks a class passed to it, so
  // a menu styled through the Popover's className rendered with no background over the chat.
  it('should draw the menu surface inside the bare popover, not on it', () => {
    renderBoth({ status: 'on', conversationExcluded: false });
    openMenu();

    const group = screen.getByRole('group', { name: copy.chip_menu });
    expect(group.classList.contains(styles.menu)).toBe(true);
    expect(group.closest('[data-bare]')?.classList.contains(styles.menu)).toBe(false);
  });

  it('should ask before stopping a chat that already shared, and erase only once confirmed', () => {
    const { onSetConversationShared } = renderBoth({
      status: 'on',
      conversationExcluded: false,
      conversationHasShared: true,
    });
    openMenu();
    fireEvent.click(screen.getByRole('switch', { name: copy.menu_share_chat }));

    expect(onSetConversationShared).not.toHaveBeenCalled();
    expect(screen.getByRole('alertdialog').textContent).toContain(copy.menu_chat_confirm);

    fireEvent.click(screen.getByRole('button', { name: copy.menu_chat_confirm_yes }));

    expect(onSetConversationShared).toHaveBeenCalledExactlyOnceWith(false);
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });

  // jsdom does no layout, so this reads the rule itself: the two uppercase buttons are wider
  // than the 240 px menu, and on one row "Keep sharing" was drawn outside the menu.
  it('should let the confirmation buttons wrap inside the menu', () => {
    const sharingCss = readFileSync(join(__dirname, 'SharingConsent.module.css'), 'utf8');
    const rule = (selector: string) =>
      sharingCss.match(new RegExp(`${selector.replace(/[.*>]/g, '\\$&')}\\s*\\{([^}]*)\\}`))?.[1];

    expect(rule('.menuConfirmActions')).toMatch(/flex-wrap:\s*wrap/);
    expect(rule('.menuConfirmActions > *')).toMatch(/flex:\s*1 1 auto/);
  });

  it('should treat turning the switch back on during the confirmation as a cancel', () => {
    const { onSetConversationShared } = renderBoth({
      status: 'on',
      conversationExcluded: false,
      conversationHasShared: true,
    });
    openMenu();
    const thisChat = screen.getByRole('switch', { name: copy.menu_share_chat });
    fireEvent.click(thisChat);
    expect(thisChat.getAttribute('aria-checked')).toBe('false');

    fireEvent.click(thisChat);

    expect(onSetConversationShared).not.toHaveBeenCalled();
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(thisChat.getAttribute('aria-checked')).toBe('true');
  });

  it('should re-include a chat excluded elsewhere while the confirmation was open', () => {
    const onSetConversationShared = vi.fn();
    const view = (conversationExcluded: boolean): AiSharingView => ({
      status: 'on',
      conversationExcluded,
      conversationHasShared: true,
    });
    const { rerender } = render(
      <SharingStatusChip
        sharing={view(false)}
        onSetConversationShared={onSetConversationShared}
      />,
    );
    openMenu();
    const thisChat = screen.getByRole('switch', { name: copy.menu_share_chat });
    fireEvent.click(thisChat);
    rerender(
      <SharingStatusChip
        sharing={view(true)}
        onSetConversationShared={onSetConversationShared}
      />,
    );

    fireEvent.click(thisChat);

    expect(onSetConversationShared).toHaveBeenCalledExactlyOnceWith(true);
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });

  it('should keep sharing a chat when the creator cancels', () => {
    const { onSetConversationShared } = renderBoth({
      status: 'on',
      conversationExcluded: false,
      conversationHasShared: true,
    });
    openMenu();
    const thisChat = screen.getByRole('switch', { name: copy.menu_share_chat });
    fireEvent.click(thisChat);
    fireEvent.click(screen.getByRole('button', { name: copy.menu_chat_confirm_no }));

    expect(onSetConversationShared).not.toHaveBeenCalled();
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(thisChat.getAttribute('aria-checked')).toBe('true');
  });

  it('should stop sharing a chat that shared nothing without asking', () => {
    const { onSetConversationShared } = renderBoth({
      status: 'on',
      conversationExcluded: false,
      conversationHasShared: false,
    });
    openMenu();
    fireEvent.click(screen.getByRole('switch', { name: copy.menu_share_chat }));

    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(onSetConversationShared).toHaveBeenCalledExactlyOnceWith(false);
  });

  it('should say that what an unshared chat had shared was deleted', () => {
    renderBoth({ status: 'on', conversationExcluded: true, conversationHasShared: true });
    openMenu(true);

    expect(screen.getByText(copy.menu_chat_erased)).toBeTruthy();
  });

  it('should not mention a deletion when the chat had shared nothing', () => {
    renderBoth({ status: 'on', conversationExcluded: true, conversationHasShared: false });
    openMenu(true);

    expect(screen.queryByText(copy.menu_chat_erased)).toBeNull();
  });

  it('should let the creator share an excluded conversation again', () => {
    const { onSetConversationShared } = renderBoth({ status: 'on', conversationExcluded: true });
    openMenu(true);
    const thisChat = screen.getByRole('switch', { name: copy.menu_share_chat });

    expect(thisChat.getAttribute('aria-checked')).toBe('false');
    fireEvent.click(thisChat);
    expect(onSetConversationShared).toHaveBeenCalledWith(true);
  });

  it('should let the creator stop sharing altogether', () => {
    const { onShareDecision } = renderBoth({ status: 'on', conversationExcluded: false });
    openMenu();
    fireEvent.click(screen.getByRole('switch', { name: copy.menu_share_all }));

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
      <>
        <TurnRating
          sharing={on}
          recordId="r1"
          onRate={onRate}
          {...props}
        />
      </>,
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
