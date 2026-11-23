import { Provider } from 'react-redux';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { flatten } from '/@/modules/store/translation/flatten';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import type { Enrollment, PayoutWallet } from '/shared/types/creator-fund';

import { analytics, misc } from '#preload';
import { actions } from '/@/modules/store/creatorFund';
import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';
import * as api from '/@/lib/creatorFundApi';
import { Worlds } from '/@/lib/worlds';

import { TestProviders } from '../../../../tests/utils/TestProviders';
import { createTestStore } from '../../../../tests/utils/testStore';
import {
  CYCLES,
  LATEST_CLOSED,
  OLDEST_CLOSED,
  exampleWorld,
  fail,
  myPlaces,
  ok,
  payout,
  place,
} from './fixtures';
import { PayoutsTab } from './PayoutsTab';

vi.mock('/@/lib/creatorFundApi', () => ({
  getCycles: vi.fn(),
  getEnrollment: vi.fn(),
  getPayouts: vi.fn(),
  getPayoutWallet: vi.fn(),
  getPlaces: vi.fn(),
  putPayoutWallet: vi.fn(),
  putEnrollment: vi.fn(),
  postEmailVerification: vi.fn(),
  putEmailVerification: vi.fn(),
}));
vi.mock('/@/lib/worlds', () => ({ Worlds: vi.fn() }));
Worlds.prototype.fetchWorldScenes = vi.fn();

const ENROLLMENT: Enrollment = {
  status: 'active',
  enrolled_at: '2026-10-30T14:02:11Z',
  terms_version: 1,
  first_name: 'Jane',
  last_name: 'Doe',
  email: 'jane.doe@example.com',
  email_verified_at: null,
};

const VERIFIED: Enrollment = { ...ENROLLMENT, email_verified_at: '2026-10-30T15:00:00Z' };

const EMAIL_NEEDS_SCENE = 'You need to have a published scene to verify your email';
const WALLET_NEEDS_SCENE = 'You need to have a published scene to set up your wallet.';
const VERIFY_FIRST = 'Verify your email to set up your wallet.';

const WALLET: PayoutWallet = {
  address: '0x000000000000000000000000000000000000dEaD',
  updated_at: '2026-10-21T09:00:00Z',
};

const NEW_ADDRESS = '0x0000000000000000000000000000000000000002';

const PAYOUTS = [
  payout(LATEST_CLOSED, 'pending', [
    [place('a', { title: 'Example Place A' }), 9],
    [exampleWorld('w', { title: 'Example Karaoke', base_position: '2,1' }), 13],
  ]),
  payout(OLDEST_CLOSED, 'paid', [[place('b', { title: 'Example Place B' }), 5]]),
];

function mountTab(store: ReturnType<typeof createTestStore>) {
  render(
    <Provider store={store}>
      <TestProviders>
        <PayoutsTab />
      </TestProviders>
    </Provider>,
  );
}

async function renderTab() {
  const store = createTestStore();
  mountTab(store);
  return store;
}

const cycleButton = (table: HTMLElement, name: string) =>
  within(table).getByRole('button', { name });
const walletCard = async () => within(await screen.findByTestId('payouts-wallet-card'));
const contactCard = async () => within(await screen.findByTestId('payouts-contact-card'));

const GET_A_WALLET_URL = 'https://docs.decentraland.org/blockchain-integration/get-a-wallet';
const walletInfo = async () =>
  (await walletCard()).findByRole('button', { name: 'About payout wallets' });
const learnMore = () => screen.findByRole('link', { name: /Learn More/ });
/** The visible line that says why a setup step is locked; its button names it as its description. */
const lockHint = (card: ReturnType<typeof within>, text: string) => card.getByText(text);
const PAYOUT_RULES_URL =
  'https://docs.decentraland.org/creator/creator-fund/program-rules#payout-process';
const openAddDialog = async () => {
  fireEvent.click(await (await walletCard()).findByRole('button', { name: 'Add wallet' }));
  return screen.findByRole('dialog', { name: 'Add payout wallet' });
};
const pause = (ms: number) => act(() => new Promise<void>(resolve => setTimeout(resolve, ms)));

const WALLET_CARD = '[data-testid="payouts-wallet-card"]';
const CARDS = '[data-testid="payouts-wallet-card"], [data-testid="payouts-contact-card"]';
const SPINNER = '[role="progressbar"]';

/** Records whether an element matching `selector` was ever on the page, however briefly. */
function watchFor(selector: string) {
  let seen = !!document.querySelector(selector);
  const record = (records: MutationRecord[]) => {
    for (const { addedNodes, removedNodes } of records) {
      for (const node of [...addedNodes, ...removedNodes]) {
        if (node instanceof Element && (node.matches(selector) || node.querySelector(selector))) {
          seen = true;
        }
      }
    }
  };
  const observer = new MutationObserver(record);
  observer.observe(document.body, { childList: true, subtree: true });
  return {
    seen: () => {
      record(observer.takeRecords());
      return seen;
    },
    stop: () => observer.disconnect(),
  };
}

const tracked = (event: string) =>
  vi
    .mocked(analytics.track)
    .mock.calls.filter(([name]) => name === event)
    .map(([, properties]) => properties);

const PRIVATE_VALUES = [
  ENROLLMENT.email,
  ENROLLMENT.first_name,
  ENROLLMENT.last_name,
  WALLET.address,
  NEW_ADDRESS,
];

function expectNoPrivateValuesTracked() {
  const sent = JSON.stringify(vi.mocked(analytics.track).mock.calls);
  PRIVATE_VALUES.forEach(value => expect(sent).not.toContain(value));
}

/*
 * Each case renders the tab and its modals and queries them by role and accessible name, which
 * takes seconds on a loaded CI runner; vitest's default 5 s cut cases off there while passing.
 */
vi.setConfig({ testTimeout: 15_000 });

describe('PayoutsTab', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  beforeEach(() => {
    vi.mocked(api.getCycles).mockResolvedValue(ok(CYCLES));
    vi.mocked(api.getEnrollment).mockResolvedValue(ok(VERIFIED));
    vi.mocked(api.getPlaces).mockResolvedValue(ok(myPlaces([place('a')])));
    vi.mocked(api.getPayouts).mockResolvedValue(ok({ payouts: PAYOUTS }));
    vi.mocked(api.getPayoutWallet).mockResolvedValue(ok(WALLET));
    vi.mocked(Worlds.prototype.fetchWorldScenes).mockResolvedValue({ scenes: [], total: 3 } as any);
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  describe('when the tab first loads', () => {
    it('should show only the spinner while the payouts and the payout wallet are pending', async () => {
      vi.mocked(api.getPayouts).mockReturnValue(new Promise(() => {}));
      vi.mocked(api.getPayoutWallet).mockReturnValue(new Promise(() => {}));
      await renderTab();

      expect(await screen.findByRole('progressbar', { name: 'Loading' })).toBeTruthy();
      expect(document.querySelector(WALLET_CARD)).toBeNull();
      expect(screen.queryByTestId('payouts-contact-card')).toBeNull();
    });

    it('should keep the spinner and no cards while the payout wallet is still pending', async () => {
      vi.mocked(api.getPayoutWallet).mockReturnValue(new Promise(() => {}));
      const store = await renderTab();

      await waitFor(() => expect(store.getState().creatorFund.payouts.status).toBe('succeeded'));

      expect(screen.getByRole('progressbar', { name: 'Loading' })).toBeTruthy();
      expect(document.querySelector(WALLET_CARD)).toBeNull();
      expect(screen.queryByRole('table')).toBeNull();
    });

    it('should go from the spinner straight to the error block, never showing the cards, when the payouts fail', async () => {
      vi.mocked(api.getPayouts).mockResolvedValue(fail(404));
      const cards = watchFor(WALLET_CARD);

      try {
        await renderTab();

        expect((await screen.findByRole('alert')).textContent).toBe(
          'Something went wrong while loading your payouts.',
        );
        expect(cards.seen()).toBe(false);
      } finally {
        cards.stop();
      }
    });

    it('should show the cards and the table once both have loaded', async () => {
      await renderTab();

      expect(await screen.findByRole('table', { name: 'Payouts' })).toBeTruthy();
      expect(document.querySelector(WALLET_CARD)).toBeTruthy();
      expect(screen.getByTestId('payouts-contact-card')).toBeTruthy();
      expect(screen.queryByRole('progressbar')).toBeNull();
    });

    it('should go back to the spinner on Retry', async () => {
      vi.mocked(api.getPayouts).mockResolvedValueOnce(fail(500));
      await renderTab();
      fireEvent.click(await screen.findByRole('button', { name: 'Retry' }));

      expect(screen.getByRole('progressbar', { name: 'Loading' })).toBeTruthy();
      expect(document.querySelector(WALLET_CARD)).toBeNull();
      expect(await screen.findByRole('table', { name: 'Payouts' })).toBeTruthy();
    });
  });

  describe('when the tab mounts again after its payouts failed', () => {
    it('should go from the spinner to the error block without ever showing the cards', async () => {
      vi.mocked(api.getPayouts).mockResolvedValue(fail(404));
      vi.mocked(api.getPayoutWallet).mockResolvedValue(fail(404));
      const store = await renderTab();
      await screen.findByRole('alert');
      cleanup();
      const cards = watchFor(CARDS);

      try {
        mountTab(store);

        expect(screen.getByRole('progressbar', { name: 'Loading' })).toBeTruthy();
        expect((await screen.findByRole('alert')).textContent).toBe(
          'Something went wrong while loading your payouts.',
        );
        expect(cards.seen()).toBe(false);
      } finally {
        cards.stop();
      }
    });
  });

  describe('when the tab mounts again after everything loaded', () => {
    it('should show the cards at once and refetch without the spinner', async () => {
      const store = await renderTab();
      await screen.findByRole('table', { name: 'Payouts' });
      cleanup();
      vi.mocked(api.getPayouts).mockClear();
      const spinner = watchFor(SPINNER);

      try {
        mountTab(store);

        expect(document.querySelector(WALLET_CARD)).toBeTruthy();
        expect(screen.getByRole('table', { name: 'Payouts' })).toBeTruthy();
        await waitFor(() => expect(store.getState().creatorFund.payouts.status).toBe('succeeded'));
        expect(api.getPayouts).toHaveBeenCalledTimes(1);
        expect(document.querySelector(WALLET_CARD)).toBeTruthy();
        expect(spinner.seen()).toBe(false);
      } finally {
        spinner.stop();
      }
    });
  });

  describe('when the payout wallet is saved after the tab has loaded', () => {
    it('should keep the cards on screen, never falling back to the spinner', async () => {
      vi.mocked(api.putPayoutWallet).mockResolvedValue(
        ok({ address: NEW_ADDRESS, updated_at: '2026-11-04T10:00:00Z' }),
      );
      await renderTab();
      await screen.findByRole('table', { name: 'Payouts' });
      fireEvent.click(await (await walletCard()).findByRole('button', { name: 'Edit' }));
      const dialog = await screen.findByRole('dialog', { name: 'Change payout wallet' });
      fireEvent.change(within(dialog).getByLabelText('Wallet address'), {
        target: { value: NEW_ADDRESS },
      });
      const spinner = watchFor(SPINNER);

      try {
        fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

        expect((await screen.findByRole('status')).textContent).toBe('Payout wallet updated');
        expect(document.querySelector(WALLET_CARD)).toBeTruthy();
        expect(spinner.seen()).toBe(false);
      } finally {
        spinner.stop();
      }
    });
  });

  describe('when there are payouts', () => {
    it('should list them in a table with the newest cycle open and the others closed', async () => {
      await renderTab();

      const table = await screen.findByRole('table', { name: 'Payouts' });
      expect(
        within(table)
          .getAllByRole('columnheader')
          .map(header => header.textContent),
      ).toEqual(['Cycle', 'Status', 'Paid Places', 'Amount', 'Transaction']);
      expect(cycleButton(table, 'Oct 16 - Oct 31, 2026').getAttribute('aria-expanded')).toBe(
        'true',
      );
      expect(cycleButton(table, 'Oct 1 - Oct 15, 2026').getAttribute('aria-expanded')).toBe(
        'false',
      );
      expect(within(table).getByText('22 USDC')).toBeTruthy();
      expect(within(table).getByText('9 USDC')).toBeTruthy();
      expect(within(table).getByText('Example Place A')).toBeTruthy();
      expect(within(table).queryByText('Example Place B')).toBeNull();
    });

    it('should show a payout with cents to the cent, never rounded to whole USDC', async () => {
      vi.mocked(api.getPayouts).mockResolvedValue(
        ok({
          payouts: [
            payout(LATEST_CLOSED, 'paid', [
              [place('a', { title: 'Example Place A' }), 1842.5],
              [place('c', { title: 'Example Place C' }), 0.49],
            ]),
          ],
        }),
      );
      await renderTab();

      const table = await screen.findByRole('table', { name: 'Payouts' });
      expect(within(table).getByText('1,842.99 USDC')).toBeTruthy();
      expect(within(table).getByText('1,842.50 USDC')).toBeTruthy();
      expect(within(table).getByText('0.49 USDC')).toBeTruthy();
    });

    it('should open a second row while the first stays open', async () => {
      await renderTab();
      const table = await screen.findByRole('table', { name: 'Payouts' });

      fireEvent.click(cycleButton(table, 'Oct 1 - Oct 15, 2026'));

      expect(cycleButton(table, 'Oct 1 - Oct 15, 2026').getAttribute('aria-expanded')).toBe('true');
      expect(cycleButton(table, 'Oct 16 - Oct 31, 2026').getAttribute('aria-expanded')).toBe(
        'true',
      );
      expect(screen.getByText('Example Place B')).toBeTruthy();
    });

    it('should name a scene in a World of several scenes by its parcel', async () => {
      await renderTab();

      expect(await screen.findByText('example-world.dcl.eth - 2,1')).toBeTruthy();
    });

    it('should show each status and open a paid transaction on Etherscan, with no button on a pending one', async () => {
      await renderTab();
      const table = await screen.findByRole('table', { name: 'Payouts' });
      const rowOf = (status: string) =>
        within(within(table).getByText(status).closest('[role="row"]') as HTMLElement);
      const etherscan = { name: 'View transaction on Etherscan' };

      expect(within(table).getAllByRole('button', etherscan)).toHaveLength(1);
      expect(rowOf('Pending').queryByRole('button', etherscan)).toBeNull();
      expect(rowOf('Pending').getAllByRole('cell')).toHaveLength(5);

      fireEvent.click(rowOf('Paid').getByRole('button', etherscan));

      expect(misc.openExternal).toHaveBeenCalledTimes(1);
      expect(misc.openExternal).toHaveBeenCalledWith(
        `https://etherscan.io/tx/0x${'1'.padStart(64, '0')}`,
      );
    });
  });

  describe('when there are no payouts yet', () => {
    it('should say the first one comes after the running cycle', async () => {
      vi.mocked(api.getPayouts).mockResolvedValue(ok({ payouts: [] }));

      await renderTab();

      expect(
        await screen.findByRole('heading', { level: 2, name: 'Your rewards journey starts here!' }),
      ).toBeTruthy();
      expect(
        screen.getByText('Your first payout is coming after the Nov 1 - Nov 15 cycle ends.'),
      ).toBeTruthy();
      expect(screen.queryByRole('table')).toBeNull();
    });
  });

  describe('when the payouts fail to load', () => {
    it('should offer a retry', async () => {
      vi.mocked(api.getPayouts).mockResolvedValueOnce(fail(500));

      await renderTab();
      fireEvent.click(await screen.findByRole('button', { name: 'Retry' }));

      expect(await screen.findByRole('table', { name: 'Payouts' })).toBeTruthy();
    });
  });

  describe('when the payout wallet is set', () => {
    it('should show it short, with the whole address as its title, a copy button and EDIT', async () => {
      await renderTab();

      const address = await screen.findByText('0x00000...00dEaD');
      const wallet = await walletCard();
      expect(address.getAttribute('title')).toBe(WALLET.address);
      expect(wallet.getByRole('button', { name: 'Copy wallet address' })).toBeTruthy();
      expect(wallet.getByRole('button', { name: 'Edit' })).toBeTruthy();
      expect(wallet.queryByRole('button', { name: 'Add wallet' })).toBeNull();
    });

    it('should copy the whole address from a real button, not one nested in another', async () => {
      await renderTab();
      const copy = await (await walletCard()).findByRole('button', { name: 'Copy wallet address' });

      fireEvent.click(copy);

      expect(copy.tagName).toBe('BUTTON');
      expect(copy.parentElement?.closest('[role="button"], button')).toBeNull();
      expect(misc.copyToClipboard).toHaveBeenCalledWith(WALLET.address);
    });

    it('should copy the whole address when the address text in the chip is clicked', async () => {
      await renderTab();
      const copy = await (await walletCard()).findByRole('button', { name: 'Copy wallet address' });

      fireEvent.click(within(copy).getByText('0x00000...00dEaD'));

      expect(misc.copyToClipboard).toHaveBeenCalledWith(WALLET.address);
      expect(copy.querySelector('button, [role="button"]')).toBeNull();
    });

    it('should keep focus on EDIT after the change', async () => {
      vi.mocked(api.putPayoutWallet).mockResolvedValue(
        ok({ address: NEW_ADDRESS, updated_at: '2026-11-04T10:00:00Z' }),
      );
      await renderTab();
      const edit = await (await walletCard()).findByRole('button', { name: 'Edit' });
      fireEvent.click(edit);
      const dialog = await screen.findByRole('dialog', { name: 'Change payout wallet' });
      fireEvent.change(within(dialog).getByLabelText('Wallet address'), {
        target: { value: NEW_ADDRESS },
      });
      fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

      await waitFor(() => expect(document.activeElement).toBe(edit));
    });

    it('should change it and say so', async () => {
      vi.mocked(api.putPayoutWallet).mockResolvedValue(
        ok({ address: NEW_ADDRESS, updated_at: '2026-11-04T10:00:00Z' }),
      );
      await renderTab();
      fireEvent.click(await (await walletCard()).findByRole('button', { name: 'Edit' }));
      const dialog = await screen.findByRole('dialog', { name: 'Change payout wallet' });

      fireEvent.change(within(dialog).getByLabelText('Wallet address'), {
        target: { value: NEW_ADDRESS },
      });
      fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

      expect((await screen.findByRole('status')).textContent).toBe('Payout wallet updated');
      expect(screen.queryByRole('dialog')).toBeNull();
      expect(screen.getByText('0x00000...000002')).toBeTruthy();
      expect(api.getPayoutWallet).toHaveBeenCalledTimes(1);
    });
  });

  describe('when the payout wallet info button gets focus', () => {
    it('should show the tooltip and name it as the description of the button', async () => {
      await renderTab();
      const info = await walletInfo();

      act(() => info.focus());

      const tooltip = await screen.findByRole('tooltip');
      expect(tooltip.textContent).toMatch(/^A digital wallet is where you store/);
      expect(info.getAttribute('aria-describedby')).toBe(tooltip.id);
    });

    it('should move focus to Learn More on Tab and keep the tooltip open', async () => {
      await renderTab();
      const info = await walletInfo();
      act(() => info.focus());
      const link = await learnMore();

      const moved = !fireEvent.keyDown(info, { key: 'Tab' });

      expect(moved).toBe(true);
      expect(document.activeElement).toBe(link);
      expect(screen.getByRole('tooltip')).toBeTruthy();
    });

    it('should go back to the button on Shift+Tab from Learn More, with the tooltip open', async () => {
      await renderTab();
      const info = await walletInfo();
      act(() => info.focus());
      const link = await learnMore();
      fireEvent.keyDown(info, { key: 'Tab' });

      const held = !fireEvent.keyDown(link, { key: 'Tab', shiftKey: true });

      expect(held).toBe(true);
      expect(document.activeElement).toBe(info);
      expect(screen.getByRole('tooltip')).toBeTruthy();
      expect(info.getAttribute('aria-describedby')).toBe(screen.getByRole('tooltip').id);
    });

    it('should close the tooltip when Tab leaves Learn More for the next control', async () => {
      await renderTab();
      const info = await walletInfo();
      const edit = (await walletCard()).getByRole('button', { name: 'Edit' });
      act(() => info.focus());
      const link = await learnMore();
      fireEvent.keyDown(info, { key: 'Tab' });

      // Tab goes on from the button: the handler puts focus there and leaves the key to the page.
      const left = fireEvent.keyDown(link, { key: 'Tab' });
      expect(left).toBe(true);
      expect(document.activeElement).toBe(info);
      act(() => edit.focus());

      await waitFor(() => expect(screen.queryByRole('tooltip')).toBeNull());
      expect(info.hasAttribute('aria-describedby')).toBe(false);
    });

    it('should close the tooltip on Escape in Learn More and put focus on the button', async () => {
      await renderTab();
      const info = await walletInfo();
      act(() => info.focus());
      const link = await learnMore();
      fireEvent.keyDown(info, { key: 'Tab' });

      fireEvent.keyDown(link, { key: 'Escape' });

      await waitFor(() => expect(screen.queryByRole('tooltip')).toBeNull());
      expect(document.activeElement).toBe(info);
    });

    it('should open the tooltip on the next hover after Escape closed it from Learn More', async () => {
      await renderTab();
      const info = await walletInfo();
      act(() => info.focus());
      const link = await learnMore();
      fireEvent.keyDown(info, { key: 'Tab' });
      fireEvent.keyDown(link, { key: 'Escape' });
      await waitFor(() => expect(screen.queryByRole('tooltip')).toBeNull());

      fireEvent.mouseEnter(info);

      expect(await screen.findByRole('tooltip')).toBeTruthy();
    });

    it('should open the wallet guide in the browser from Learn More', async () => {
      await renderTab();
      const info = await walletInfo();
      act(() => info.focus());

      const followed = fireEvent.click(await learnMore());

      expect(followed).toBe(false);
      expect(misc.openExternal).toHaveBeenCalledWith(GET_A_WALLET_URL);
    });
  });

  describe('when the pointer crosses the payout wallet info button', () => {
    it('should never open the tooltip when the pointer leaves before the open delay', async () => {
      await renderTab();
      const info = await walletInfo();
      const tooltip = watchFor('[role="tooltip"]');

      fireEvent.mouseEnter(info);
      fireEvent.mouseLeave(info);
      await pause(400);

      expect(tooltip.seen()).toBe(false);
      tooltip.stop();
      expect(info.hasAttribute('aria-describedby')).toBe(false);
    });

    it('should open the tooltip when the pointer stays', async () => {
      await renderTab();
      const info = await walletInfo();

      fireEvent.mouseEnter(info);

      const tooltip = await screen.findByRole('tooltip');
      expect(within(tooltip).getByRole('link', { name: /Learn More/ })).toBeTruthy();
    });
  });

  describe('when the info button beside the payout wallet dialog title gets focus', () => {
    beforeEach(() => {
      vi.mocked(api.getPayoutWallet).mockResolvedValue(fail(404));
    });

    it('should explain the payout wallet in the tooltip and name it as the description of the button', async () => {
      await renderTab();
      const dialog = await openAddDialog();
      const info = within(dialog).getByRole('button', { name: 'About payout wallets' });

      act(() => info.focus());

      const tooltip = await screen.findByRole('tooltip');
      expect(tooltip.textContent).toMatch(
        /^Your payout wallet is where the Creator Fund sends your rewards, in USDC on Ethereum \(ERC-20\)/,
      );
      expect(info.getAttribute('aria-describedby')).toBe(tooltip.id);
      expect(screen.getByRole('dialog', { name: 'Add payout wallet' })).toBe(dialog);
    });

    it('should move focus to Learn More on Tab and keep the tooltip and the dialog open', async () => {
      await renderTab();
      const dialog = await openAddDialog();
      const info = within(dialog).getByRole('button', { name: 'About payout wallets' });
      act(() => info.focus());
      const link = await learnMore();

      fireEvent.keyDown(info, { key: 'Tab' });

      expect(document.activeElement).toBe(link);
      expect(screen.getByRole('tooltip')).toBeTruthy();
      expect(screen.getByRole('dialog', { name: 'Add payout wallet' })).toBe(dialog);
    });

    it('should close only the tooltip on Escape in Learn More, keeping the dialog open', async () => {
      await renderTab();
      const dialog = await openAddDialog();
      const info = within(dialog).getByRole('button', { name: 'About payout wallets' });
      act(() => info.focus());
      const link = await learnMore();
      fireEvent.keyDown(info, { key: 'Tab' });

      fireEvent.keyDown(link, { key: 'Escape' });

      await waitFor(() => expect(screen.queryByRole('tooltip')).toBeNull());
      expect(document.activeElement).toBe(info);
      expect(screen.getByRole('dialog', { name: 'Add payout wallet' })).toBe(dialog);
    });

    it('should open the payout rules in the browser from Learn More, keeping the dialog open', async () => {
      await renderTab();
      const dialog = await openAddDialog();
      act(() => within(dialog).getByRole('button', { name: 'About payout wallets' }).focus());

      const followed = fireEvent.click(await learnMore());

      expect(followed).toBe(false);
      expect(misc.openExternal).toHaveBeenCalledTimes(1);
      expect(misc.openExternal).toHaveBeenCalledWith(PAYOUT_RULES_URL);
      expect(screen.getByRole('dialog', { name: 'Add payout wallet' })).toBe(dialog);
    });

    it('should offer the same button when the wallet is changed', async () => {
      vi.mocked(api.getPayoutWallet).mockResolvedValue(ok(WALLET));
      await renderTab();
      fireEvent.click(await (await walletCard()).findByRole('button', { name: 'Edit' }));
      const dialog = await screen.findByRole('dialog', { name: 'Change payout wallet' });

      expect(within(dialog).getByRole('button', { name: 'About payout wallets' })).toBeTruthy();
    });
  });

  describe('when there is no payout wallet', () => {
    beforeEach(() => {
      vi.mocked(api.getPayoutWallet).mockResolvedValue(fail(404));
    });

    it('should offer only ADD WALLET', async () => {
      await renderTab();

      const wallet = await walletCard();
      expect(await wallet.findByRole('button', { name: 'Add wallet' })).toBeTruthy();
      expect(wallet.queryByRole('button', { name: 'Edit' })).toBeNull();
      expect(wallet.queryByRole('button', { name: 'Copy wallet address' })).toBeNull();
    });

    it('should give ADD WALLET and an unverified VERIFY EMAIL the same primary button', async () => {
      vi.mocked(api.getEnrollment).mockResolvedValue(ok(ENROLLMENT));
      await renderTab();

      const add = await (await walletCard()).findByRole('button', { name: 'Add wallet' });
      const verify = await (await contactCard()).findByRole('button', { name: 'Verify email' });
      for (const button of [add, verify]) {
        expect(button.classList).toContain('btn--primary');
      }
    });

    it('should hold the submit until the address is valid and flag a bad one on blur', async () => {
      await renderTab();
      fireEvent.click(await (await walletCard()).findByRole('button', { name: 'Add wallet' }));
      const dialog = await screen.findByRole('dialog', { name: 'Add payout wallet' });
      const field = within(dialog).getByLabelText('Wallet address');
      const submit = within(dialog).getByRole('button', { name: 'Add wallet' });

      fireEvent.change(field, { target: { value: '0x123' } });
      fireEvent.blur(field);

      expect(submit.hasAttribute('disabled')).toBe(true);
      expect(field.getAttribute('aria-invalid')).toBe('true');
      expect(
        within(dialog).getByText(
          'Invalid wallet address. It should start with 0x and be 42 characters long.',
        ),
      ).toBeTruthy();
      expect(within(dialog).getByRole('note').textContent).toMatch(/ERC-20/);

      fireEvent.change(field, { target: { value: NEW_ADDRESS } });

      expect(submit.hasAttribute('disabled')).toBe(false);
    });

    it('should add one and say so', async () => {
      vi.mocked(api.putPayoutWallet).mockResolvedValue(
        ok({ address: NEW_ADDRESS, updated_at: '2026-11-04T10:00:00Z' }, 201),
      );
      await renderTab();
      fireEvent.click(await (await walletCard()).findByRole('button', { name: 'Add wallet' }));
      const dialog = await screen.findByRole('dialog', { name: 'Add payout wallet' });

      fireEvent.change(within(dialog).getByLabelText('Wallet address'), {
        target: { value: NEW_ADDRESS },
      });
      fireEvent.click(within(dialog).getByRole('button', { name: 'Add wallet' }));

      const status = await screen.findByRole('status');
      await waitFor(() => expect(status.textContent).toBe('Payout wallet added'));
      expect(api.putPayoutWallet).toHaveBeenCalledWith({ address: NEW_ADDRESS });
      const edit = await (await walletCard()).findByRole('button', { name: 'Edit' });
      await waitFor(() => expect(document.activeElement).toBe(edit));
    });

    it('should show the refusal of an address on the field', async () => {
      vi.mocked(api.putPayoutWallet).mockResolvedValue({
        ...fail(400),
        body: { error: 'invalid_address' },
      });
      await renderTab();
      fireEvent.click(await (await walletCard()).findByRole('button', { name: 'Add wallet' }));
      const dialog = await screen.findByRole('dialog', { name: 'Add payout wallet' });

      fireEvent.change(within(dialog).getByLabelText('Wallet address'), {
        target: { value: NEW_ADDRESS },
      });
      fireEvent.click(within(dialog).getByRole('button', { name: 'Add wallet' }));

      const field = within(dialog).getByLabelText('Wallet address');
      await waitFor(() => expect(field.getAttribute('aria-invalid')).toBe('true'));
      const line = document.getElementById(field.getAttribute('aria-describedby')!)!;
      expect(line.getAttribute('aria-live')).toBe('polite');
      expect(line.textContent).toBe(
        'Invalid wallet address. It should start with 0x and be 42 characters long.',
      );
      expect(document.activeElement).toBe(field);
    });

    it('should close on Escape', async () => {
      await renderTab();
      fireEvent.click(await (await walletCard()).findByRole('button', { name: 'Add wallet' }));
      const dialog = await screen.findByRole('dialog', { name: 'Add payout wallet' });

      fireEvent.keyDown(dialog, { key: 'Escape' });

      await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    });
  });

  describe('when the email is not verified', () => {
    const CONFIRM_TITLE = '1. Confirm your email';
    const CODE_TITLE = '2. Enter the verification code';
    const NEW_EMAIL = 'jane.doe@example.org';
    const GENERIC = 'Something went wrong. Please try again.';
    const DAILY_LIMIT = "You've requested too many codes today. Try again tomorrow.";
    const SEND_SOON = 'You can send a code to this address in a moment.';

    const tooSoon = (inMs: number) => ({
      ...fail(429),
      body: { error: 'too_soon', resend_available_at: new Date(Date.now() + inMs).toISOString() },
    });

    beforeEach(() => {
      vi.mocked(api.getEnrollment).mockResolvedValue(ok(ENROLLMENT));
      vi.mocked(api.postEmailVerification).mockResolvedValue(
        ok(
          {
            sent_at: new Date().toISOString(),
            expires_at: new Date(Date.now() + 600e3).toISOString(),
            resend_available_at: new Date(Date.now() + 60e3).toISOString(),
          },
          202,
        ),
      );
      vi.mocked(api.putEnrollment).mockImplementation(async request =>
        ok({ ...ENROLLMENT, ...request }),
      );
    });

    async function openVerify() {
      const store = await renderTab();
      fireEvent.click(await (await contactCard()).findByRole('button', { name: 'Verify email' }));
      return { store, dialog: await screen.findByRole('dialog', { name: CONFIRM_TITLE }) };
    }

    const continueButton = (dialog: HTMLElement) => within(dialog).getByText('Continue');
    const emailInput = (dialog: HTMLElement) =>
      within(dialog).getByLabelText('Email') as HTMLInputElement;
    const pencil = (dialog: HTMLElement) => within(dialog).getByLabelText('Edit email');
    const focusOn = (element: HTMLElement) =>
      waitFor(() => expect(document.activeElement).toBe(element));

    const toCodeStep = async (dialog: HTMLElement) => {
      fireEvent.click(continueButton(dialog));
      await screen.findByRole('dialog', { name: CODE_TITLE });
    };

    async function openCodeStep() {
      const { dialog } = await openVerify();
      await toCodeStep(dialog);
      return dialog;
    }

    const editEmail = (dialog: HTMLElement, value: string) => {
      fireEvent.click(pencil(dialog));
      fireEvent.change(emailInput(dialog), { target: { value } });
    };

    const typeCode = (dialog: HTMLElement, code: string) =>
      fireEvent.paste(within(dialog).getByLabelText('Digit 1 of 6'), {
        clipboardData: { getData: () => code },
      });

    it('should track the start of email verification once, with no properties', async () => {
      await openVerify();

      expect(tracked('Creator Fund Email Verification Started')).toEqual([undefined]);
      expectNoPrivateValuesTracked();
    });

    describe('and the dialog opens on its first step', () => {
      it('should show the email read-only with focus on CONTINUE, and send nothing', async () => {
        const { dialog } = await openVerify();

        expect(emailInput(dialog).value).toBe('jane.doe@example.com');
        expect(emailInput(dialog).readOnly).toBe(true);
        expect(document.activeElement).toBe(continueButton(dialog));
        expect(api.postEmailVerification).not.toHaveBeenCalled();
      });

      it('should send a code on CONTINUE and open the code step on the first box', async () => {
        const dialog = await openCodeStep();

        expect(api.postEmailVerification).toHaveBeenCalledTimes(1);
        expect(api.putEnrollment).not.toHaveBeenCalled();
        expect(within(dialog).getByText('jane.doe@example.com').tagName).toBe('B');
        await focusOn(within(dialog).getByLabelText('Digit 1 of 6'));
        const resend = within(dialog).getByText('Resend code');
        await waitFor(() => expect(resend.getAttribute('aria-disabled')).toBe('true'));
        expect(within(dialog).getByText('Verify').hasAttribute('disabled')).toBe(true);
        expect(within(dialog).getByText('Back')).toBeTruthy();
        expect(within(dialog).queryByText('Cancel')).toBeNull();
      });

      it('should make the field editable from the pencil, with focus in it and its text selected', async () => {
        const select = vi.spyOn(HTMLInputElement.prototype, 'select');
        const { dialog } = await openVerify();

        fireEvent.click(pencil(dialog));

        const input = emailInput(dialog);
        expect(input.readOnly).toBe(false);
        expect(document.activeElement).toBe(input);
        expect(select.mock.contexts).toEqual([input]);
        expect(within(dialog).queryByLabelText('Edit email')).toBeNull();
        select.mockRestore();
      });

      it('should save a changed email with the names and the current terms, then send the code to it', async () => {
        const { store, dialog } = await openVerify();
        store.dispatch(actions.fetchTerms.pending('terms', undefined));
        store.dispatch(
          actions.fetchTerms.fulfilled(
            { version: 2, title: 'Terms', published_at: '2026-11-01T00:00:00Z', content: '' },
            'terms',
            undefined,
          ),
        );

        editEmail(dialog, `  ${NEW_EMAIL} `);
        await toCodeStep(dialog);

        expect(api.putEnrollment).toHaveBeenCalledWith({
          first_name: 'Jane',
          last_name: 'Doe',
          email: NEW_EMAIL,
          terms_version: 2,
        });
        expect(vi.mocked(api.putEnrollment).mock.invocationCallOrder[0]).toBeLessThan(
          vi.mocked(api.postEmailVerification).mock.invocationCallOrder[0],
        );
        expect(within(dialog).getByText(NEW_EMAIL).tagName).toBe('B');
      });

      it('should count an email equal to the current one once trimmed as unchanged', async () => {
        const { dialog } = await openVerify();

        editEmail(dialog, ' jane.doe@example.com ');
        await toCodeStep(dialog);

        expect(api.putEnrollment).not.toHaveBeenCalled();
        expect(api.postEmailVerification).toHaveBeenCalledTimes(1);
      });

      it('should count an email that differs from the current one only in case as unchanged', async () => {
        const { dialog } = await openVerify();

        editEmail(dialog, ' JANE.DOE@Example.com ');
        await toCodeStep(dialog);

        expect(api.putEnrollment).not.toHaveBeenCalled();
        expect(api.postEmailVerification).toHaveBeenCalledTimes(1);
      });

      it('should open the code step on the cooling code when its address comes back in another case', async () => {
        const dialog = await openCodeStep();
        fireEvent.click(within(dialog).getByText('Back'));
        await screen.findByRole('dialog', { name: CONFIRM_TITLE });

        editEmail(dialog, 'Jane.Doe@EXAMPLE.com');
        await toCodeStep(dialog);

        expect(api.putEnrollment).not.toHaveBeenCalled();
        expect(api.postEmailVerification).toHaveBeenCalledTimes(1);
      });

      it('should hold CONTINUE on an empty or invalid email and flag an invalid one under the field', async () => {
        const { dialog } = await openVerify();

        editEmail(dialog, '');
        expect(continueButton(dialog).hasAttribute('disabled')).toBe(true);
        expect(within(dialog).queryByRole('alert')).toBeNull();

        fireEvent.change(emailInput(dialog), { target: { value: 'jane.doe@' } });
        fireEvent.blur(emailInput(dialog));

        const alert = await within(dialog).findByRole('alert');
        expect(alert.textContent).toBe('Invalid email');
        expect(emailInput(dialog).getAttribute('aria-invalid')).toBe('true');
        expect(emailInput(dialog).getAttribute('aria-describedby')).toContain(alert.id);
        expect(continueButton(dialog).hasAttribute('disabled')).toBe(true);
      });

      it('should stay on the step with the error line when the save fails', async () => {
        vi.mocked(api.putEnrollment).mockResolvedValue(fail(500));
        const { dialog } = await openVerify();

        editEmail(dialog, NEW_EMAIL);
        fireEvent.click(continueButton(dialog));

        expect((await within(dialog).findByRole('alert')).textContent).toBe(GENERIC);
        expect(api.postEmailVerification).not.toHaveBeenCalled();
        expect(continueButton(dialog).hasAttribute('disabled')).toBe(false);
        await focusOn(emailInput(dialog));
      });

      it('should stay on the step with the error line when the service says the email is already verified', async () => {
        vi.mocked(api.putEnrollment).mockResolvedValue({
          ...fail(409),
          body: { error: 'email_verified' },
        } as never);
        const { dialog } = await openVerify();

        editEmail(dialog, NEW_EMAIL);
        fireEvent.click(continueButton(dialog));

        expect((await within(dialog).findByRole('alert')).textContent).toBe(GENERIC);
        expect(api.postEmailVerification).not.toHaveBeenCalled();
        expect(continueButton(dialog).hasAttribute('disabled')).toBe(false);
      });

      it('should wait on a changed email while a 150 s cooldown runs', async () => {
        vi.mocked(api.postEmailVerification).mockResolvedValueOnce(tooSoon(150e3) as never);
        const { dialog } = await openVerify();

        editEmail(dialog, NEW_EMAIL);
        fireEvent.click(continueButton(dialog));

        await expectWaiting(dialog);
      });

      it('should keep the saved email read-only when the send fails after it, and retry only the send', async () => {
        vi.mocked(api.postEmailVerification).mockResolvedValueOnce(fail(500));
        const { dialog } = await openVerify();

        editEmail(dialog, NEW_EMAIL);
        fireEvent.click(continueButton(dialog));

        expect((await within(dialog).findByRole('alert')).textContent).toBe(GENERIC);
        expect(emailInput(dialog).value).toBe(NEW_EMAIL);
        expect(emailInput(dialog).readOnly).toBe(true);
        await waitFor(() => expect(continueButton(dialog).hasAttribute('disabled')).toBe(false));
        await focusOn(emailInput(dialog));

        await toCodeStep(dialog);
        expect(api.putEnrollment).toHaveBeenCalledTimes(1);
        expect(api.postEmailVerification).toHaveBeenCalledTimes(2);
      });

      it('should say a failed send under the field and let CONTINUE try again', async () => {
        vi.mocked(api.postEmailVerification).mockResolvedValueOnce(fail(500));
        const { dialog } = await openVerify();

        fireEvent.click(continueButton(dialog));

        expect((await within(dialog).findByRole('alert')).textContent).toBe(GENERIC);
        await waitFor(() => expect(continueButton(dialog).hasAttribute('disabled')).toBe(false));
        await focusOn(emailInput(dialog));
        await toCodeStep(dialog);
        expect(api.postEmailVerification).toHaveBeenCalledTimes(2);
      });

      it.each([
        ['30 s', 30e3],
        ['90 s', 90e3],
        ['150 s', 150e3],
        ['3 min', 180e3],
      ])(
        'should open the code step without a new code while the last one cools down for %s',
        async (_away, ms) => {
          vi.mocked(api.postEmailVerification).mockResolvedValue(tooSoon(ms) as never);
          const { dialog } = await openVerify();

          await toCodeStep(dialog);

          expect(within(dialog).queryByRole('alert')).toBeNull();
          expect(within(dialog).getByText('Resend code').getAttribute('aria-disabled')).toBe(
            'true',
          );
        },
      );

      it.each([
        ['6 h', 6 * 3600e3],
        ['12 h', 12 * 3600e3],
      ])(
        'should say the daily limit and hold CONTINUE when the next code is %s away',
        async (_away, ms) => {
          vi.mocked(api.postEmailVerification).mockResolvedValue(tooSoon(ms) as never);
          const { dialog } = await openVerify();

          fireEvent.click(continueButton(dialog));

          expect((await within(dialog).findByRole('alert')).textContent).toBe(DAILY_LIMIT);
          expect(continueButton(dialog).hasAttribute('disabled')).toBe(true);
          await focusOn(emailInput(dialog));
          expect(screen.getByRole('dialog', { name: CONFIRM_TITLE })).toBe(dialog);
        },
      );

      const expectWaiting = async (dialog: HTMLElement) => {
        const status = await within(dialog).findByRole('status');
        expect(status.textContent).toBe(SEND_SOON);
        expect(within(dialog).queryByRole('alert')).toBeNull();
        expect(emailInput(dialog).value).toBe(NEW_EMAIL);
        expect(emailInput(dialog).readOnly).toBe(true);
        expect(pencil(dialog)).toBeTruthy();
        expect(continueButton(dialog).hasAttribute('disabled')).toBe(true);
        expect(continueButton(dialog).getAttribute('aria-describedby')).toBe(status.id);
        expect(screen.queryByRole('dialog', { name: CODE_TITLE })).toBeNull();
        await focusOn(emailInput(dialog));
      };

      const waitOut = async (dialog: HTMLElement) => {
        await waitFor(() => expect(continueButton(dialog).hasAttribute('disabled')).toBe(false), {
          timeout: 3000,
        });
        expect(within(dialog).queryByRole('status')).toBeNull();
      };

      it('should keep a changed email on this step until the cooldown the service answers ends, then send on CONTINUE', async () => {
        vi.mocked(api.postEmailVerification).mockResolvedValueOnce(tooSoon(600) as never);
        const { dialog } = await openVerify();

        editEmail(dialog, NEW_EMAIL);
        fireEvent.click(continueButton(dialog));

        await expectWaiting(dialog);
        await waitOut(dialog);
        expect(api.postEmailVerification).toHaveBeenCalledTimes(1);
        await toCodeStep(dialog);
        expect(api.postEmailVerification).toHaveBeenCalledTimes(2);
        expect(api.putEnrollment).toHaveBeenCalledTimes(1);
        expect(within(dialog).getByText(NEW_EMAIL).tagName).toBe('B');
      });

      it('should keep a changed email on this step while the code just sent cools down, sending nothing until CONTINUE', async () => {
        vi.mocked(api.postEmailVerification).mockResolvedValueOnce(
          ok(
            {
              sent_at: new Date().toISOString(),
              expires_at: new Date(Date.now() + 600e3).toISOString(),
              resend_available_at: new Date(Date.now() + 600).toISOString(),
            },
            202,
          ),
        );
        const dialog = await openCodeStep();
        fireEvent.click(within(dialog).getByText('Back'));
        await screen.findByRole('dialog', { name: CONFIRM_TITLE });

        editEmail(dialog, NEW_EMAIL);
        fireEvent.click(continueButton(dialog));

        await expectWaiting(dialog);
        expect(api.postEmailVerification).toHaveBeenCalledTimes(1);
        await waitOut(dialog);
        await toCodeStep(dialog);
        expect(api.postEmailVerification).toHaveBeenCalledTimes(2);
        expect(within(dialog).getByText(NEW_EMAIL).tagName).toBe('B');
      });

      const waitOnNewEmail = async () => {
        const dialog = await openCodeStep();
        fireEvent.click(within(dialog).getByText('Back'));
        await screen.findByRole('dialog', { name: CONFIRM_TITLE });
        editEmail(dialog, NEW_EMAIL);
        fireEvent.click(continueButton(dialog));
        await expectWaiting(dialog);
        return dialog;
      };

      it('should take the address the last code went to back while waiting, and open the code step on that code', async () => {
        const dialog = await waitOnNewEmail();

        editEmail(dialog, 'jane.doe@example.com');

        expect(within(dialog).queryByRole('status')).toBeNull();
        expect(continueButton(dialog).hasAttribute('disabled')).toBe(false);
        await toCodeStep(dialog);
        expect(vi.mocked(api.putEnrollment).mock.calls.map(([request]) => request.email)).toEqual([
          NEW_EMAIL,
          'jane.doe@example.com',
        ]);
        expect(api.postEmailVerification).toHaveBeenCalledTimes(1);
        expect(within(dialog).getByText('jane.doe@example.com').tagName).toBe('B');
      });

      it('should keep waiting when the saved address is edited only in case', async () => {
        const dialog = await waitOnNewEmail();

        editEmail(dialog, 'JANE.DOE@example.ORG');

        expect(within(dialog).getByRole('status').textContent).toBe(SEND_SOON);
        expect(continueButton(dialog).hasAttribute('disabled')).toBe(true);
        expect(api.putEnrollment).toHaveBeenCalledTimes(1);
      });

      it('should save a third address at once while waiting, then wait on it', async () => {
        const dialog = await waitOnNewEmail();

        editEmail(dialog, 'jane.doe@example.net');

        expect(within(dialog).queryByRole('status')).toBeNull();
        expect(continueButton(dialog).hasAttribute('disabled')).toBe(false);
        fireEvent.click(continueButton(dialog));
        await waitFor(() => expect(api.putEnrollment).toHaveBeenCalledTimes(2));
        expect(vi.mocked(api.putEnrollment).mock.calls[1][0].email).toBe('jane.doe@example.net');
        expect((await within(dialog).findByRole('status')).textContent).toBe(SEND_SOON);
        expect(emailInput(dialog).value).toBe('jane.doe@example.net');
        expect(api.postEmailVerification).toHaveBeenCalledTimes(1);
      });

      it('should keep waiting for the changed email when the dialog is closed and opened again', async () => {
        const dialog = await openCodeStep();
        fireEvent.click(within(dialog).getByText('Back'));
        await screen.findByRole('dialog', { name: CONFIRM_TITLE });
        editEmail(dialog, NEW_EMAIL);
        fireEvent.click(continueButton(dialog));
        await expectWaiting(dialog);

        fireEvent.keyDown(dialog, { key: 'Escape' });
        await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
        fireEvent.click(await (await contactCard()).findByRole('button', { name: 'Verify email' }));
        const again = await screen.findByRole('dialog', { name: CONFIRM_TITLE });
        fireEvent.click(continueButton(again));

        await expectWaiting(again);
        expect(api.postEmailVerification).toHaveBeenCalledTimes(1);
      });

      it('should say a refused send with no time for the next code as a failure, and let CONTINUE try again', async () => {
        vi.mocked(api.postEmailVerification).mockResolvedValue({
          ...fail(429),
          body: { error: 'too_soon' },
        });
        const { dialog } = await openVerify();

        fireEvent.click(continueButton(dialog));

        expect((await within(dialog).findByRole('alert')).textContent).toBe(GENERIC);
        await waitFor(() => expect(continueButton(dialog).hasAttribute('disabled')).toBe(false));
        expect(screen.getByRole('dialog', { name: CONFIRM_TITLE })).toBe(dialog);
      });

      it('should say an active place is needed when the service refuses the code for it, and lock the step', async () => {
        vi.mocked(api.postEmailVerification).mockResolvedValue({
          ...fail(403),
          body: { error: 'no_active_place' },
        });
        vi.mocked(api.getPlaces)
          .mockResolvedValueOnce(ok(myPlaces([place('a')])))
          .mockResolvedValue(ok(myPlaces([])));
        const { dialog } = await openVerify();

        fireEvent.click(continueButton(dialog));

        expect((await within(dialog).findByRole('alert')).textContent).toBe(EMAIL_NEEDS_SCENE);
        expect(continueButton(dialog).hasAttribute('disabled')).toBe(true);
        await focusOn(emailInput(dialog));
        const contact = await contactCard();
        await waitFor(() => expect(lockHint(contact, EMAIL_NEEDS_SCENE)).toBeTruthy());
      });
    });

    describe('and BACK is pressed on the code step', () => {
      it('should return to the first step read-only, with focus on the pencil and the code cleared', async () => {
        const dialog = await openCodeStep();
        typeCode(dialog, '12');

        fireEvent.click(within(dialog).getByText('Back'));

        expect(await screen.findByRole('dialog', { name: CONFIRM_TITLE })).toBe(dialog);
        expect(emailInput(dialog).readOnly).toBe(true);
        await waitFor(() => expect(document.activeElement).toBe(pencil(dialog)));
        await toCodeStep(dialog);
        expect(api.postEmailVerification).toHaveBeenCalledTimes(1);
        expect((within(dialog).getByLabelText('Digit 1 of 6') as HTMLInputElement).value).toBe('');
      });
    });

    it.each([
      ['verified', ok({ ...ENROLLMENT, email_verified_at: '2026-11-04T10:02:00Z' })],
      ['incorrect', { ...fail(400), body: { error: 'invalid_code' } }],
      ['expired', { ...fail(410), body: { error: 'code_expired' } }],
      ['locked', { ...fail(429), body: { error: 'too_many_attempts' } }],
    ] as const)('should track a %s code once', async (result, response) => {
      vi.mocked(api.putEmailVerification).mockResolvedValue(response as never);
      const dialog = await openCodeStep();

      typeCode(dialog, '123456');
      fireEvent.click(within(dialog).getByRole('button', { name: 'Verify' }));

      await waitFor(() =>
        expect(tracked('Creator Fund Email Code Submitted')).toEqual([{ result }]),
      );
      expectNoPrivateValuesTracked();
    });

    it('should track no code verdict when the check itself fails', async () => {
      vi.mocked(api.putEmailVerification).mockResolvedValue(fail(500));
      const dialog = await openCodeStep();

      typeCode(dialog, '123456');
      fireEvent.click(within(dialog).getByRole('button', { name: 'Verify' }));

      await within(dialog).findByRole('alert');
      expect(tracked('Creator Fund Email Code Submitted')).toEqual([]);
    });

    it('should show the contact values with the Unverified badge', async () => {
      await renderTab();

      expect(await screen.findByText('Jane Doe')).toBeTruthy();
      expect(screen.getByText('jane.doe@example.com')).toBeTruthy();
      expect(screen.getByText('Unverified')).toBeTruthy();
    });

    it('should verify the code, say so and drop the badge and VERIFY EMAIL', async () => {
      vi.mocked(api.putEmailVerification).mockResolvedValue(
        ok({ ...ENROLLMENT, email_verified_at: '2026-11-04T10:02:00Z' }),
      );
      const dialog = await openCodeStep();

      typeCode(dialog, '000000');
      fireEvent.click(within(dialog).getByRole('button', { name: 'Verify' }));

      const status = await screen.findByRole('status');
      await waitFor(() => expect(status.textContent).toBe('Email verified'));
      expect(api.putEmailVerification).toHaveBeenCalledWith({ code: '000000' });
      expect(screen.queryByText('Unverified')).toBeNull();
      expect((await contactCard()).queryByRole('button', { name: 'Verify email' })).toBeNull();
      const heading = screen.getByRole('heading', { level: 2, name: 'Contact Information' });
      await waitFor(() => expect(document.activeElement).toBe(heading));
      const wallet = await walletCard();
      expect(wallet.queryByText(VERIFY_FIRST)).toBeNull();
      expect(wallet.getByRole('button', { name: 'Edit' }).hasAttribute('disabled')).toBe(false);
    });

    it('should lock the payout wallet, saying beside ADD WALLET to verify the email first', async () => {
      vi.mocked(api.getPayoutWallet).mockResolvedValue(fail(404));
      await renderTab();
      const wallet = await walletCard();
      const add = await wallet.findByRole('button', { name: 'Add wallet' });

      expect(add.hasAttribute('disabled')).toBe(true);
      expect(screen.getByTestId('payouts-wallet-card').hasAttribute('data-locked')).toBe(true);
      expect(add.getAttribute('aria-describedby')).toBe(lockHint(wallet, VERIFY_FIRST).id);
      expect(screen.queryByText('Unavailable')).toBeNull();
      expect((await contactCard()).queryByText(EMAIL_NEEDS_SCENE)).toBeNull();
    });

    it('should keep a wallet set before the email was verified, with EDIT disabled', async () => {
      await renderTab();
      const wallet = await walletCard();

      expect(await wallet.findByText('0x00000...00dEaD')).toBeTruthy();
      const edit = wallet.getByRole('button', { name: 'Edit' });
      expect(edit.hasAttribute('disabled')).toBe(true);
      expect(edit.getAttribute('aria-describedby')).toBe(lockHint(wallet, VERIFY_FIRST).id);
    });

    it.each([
      ['an incorrect code', { ...fail(400), body: { error: 'invalid_code' } }, 'first box'],
      ['an expired code', { ...fail(410), body: { error: 'code_expired' } }, 'resend'],
      ['too many tries', { ...fail(429), body: { error: 'too_many_attempts' } }, 'resend'],
    ] as const)('should move focus to what %s is about', async (_case, response, target) => {
      vi.mocked(api.putEmailVerification).mockResolvedValue(response as never);
      const dialog = await openCodeStep();

      typeCode(dialog, '123456');
      fireEvent.click(within(dialog).getByRole('button', { name: 'Verify' }));

      await within(dialog).findByRole('alert');
      await waitFor(() =>
        expect(document.activeElement).toBe(
          target === 'resend'
            ? within(dialog).getByRole('button', { name: 'Resend code' })
            : within(dialog).getByLabelText('Digit 1 of 6'),
        ),
      );
    });

    it('should keep Resend code under an expired code and let it send a new one', async () => {
      vi.mocked(api.putEmailVerification).mockResolvedValue({
        ...fail(410),
        body: { error: 'code_expired' },
      });
      vi.mocked(api.postEmailVerification)
        .mockResolvedValueOnce(
          ok(
            {
              sent_at: '2026-11-04T09:00:00Z',
              expires_at: '2026-11-04T09:10:00Z',
              resend_available_at: '2026-11-04T09:01:00Z',
            },
            202,
          ),
        )
        .mockResolvedValue(
          ok(
            {
              sent_at: new Date().toISOString(),
              expires_at: new Date(Date.now() + 600e3).toISOString(),
              resend_available_at: new Date(Date.now() + 60e3).toISOString(),
            },
            202,
          ),
        );
      const dialog = await openCodeStep();
      typeCode(dialog, '123456');
      fireEvent.click(within(dialog).getByRole('button', { name: 'Verify' }));
      await within(dialog).findByRole('alert');

      const resend = within(dialog).getByRole('button', { name: 'Resend code' });
      expect(resend.getAttribute('aria-disabled')).toBeNull();
      fireEvent.click(resend);

      await waitFor(() => expect(api.postEmailVerification).toHaveBeenCalledTimes(2));
      expect(within(dialog).queryByRole('alert')).toBeNull();
    });

    it('should say a failed resend on the error line and keep Resend code to try again', async () => {
      vi.mocked(api.postEmailVerification)
        .mockResolvedValueOnce(
          ok(
            {
              sent_at: '2026-11-04T09:00:00Z',
              expires_at: '2026-11-04T09:10:00Z',
              resend_available_at: '2026-11-04T09:01:00Z',
            },
            202,
          ),
        )
        .mockResolvedValueOnce(fail(500))
        .mockResolvedValue(
          ok(
            {
              sent_at: new Date().toISOString(),
              expires_at: new Date(Date.now() + 600e3).toISOString(),
              resend_available_at: new Date(Date.now() + 60e3).toISOString(),
            },
            202,
          ),
        );
      const dialog = await openCodeStep();
      fireEvent.click(within(dialog).getByRole('button', { name: 'Resend code' }));

      expect((await within(dialog).findByRole('alert')).textContent).toBe(GENERIC);
      const resend = within(dialog).getByRole('button', { name: 'Resend code' });
      expect(resend.getAttribute('aria-disabled')).toBeNull();
      fireEvent.click(resend);

      await waitFor(() => expect(api.postEmailVerification).toHaveBeenCalledTimes(3));
      await waitFor(() => expect(within(dialog).queryByRole('alert')).toBeNull());
    });

    it.each([
      [400, 'invalid_code', 'Incorrect code. Check the email and try again.', false],
      [429, 'too_many_attempts', GENERIC, true],
    ])('should explain a %s %s', async (status, error, message, resendShown) => {
      vi.mocked(api.putEmailVerification).mockResolvedValue({ ...fail(status), body: { error } });
      const dialog = await openCodeStep();

      typeCode(dialog, '123456');
      fireEvent.click(within(dialog).getByRole('button', { name: 'Verify' }));

      expect((await within(dialog).findByRole('alert')).textContent).toBe(message);
      expect(within(dialog).getByLabelText('Digit 1 of 6').getAttribute('aria-invalid')).toBe(
        'true',
      );
      expect(!!within(dialog).queryByRole('button', { name: 'Resend code' })).toBe(resendShown);
    });
  });

  describe.each([
    ['payout wallet', () => vi.mocked(api.getPayoutWallet)],
    ['enrollment', () => vi.mocked(api.getEnrollment)],
  ])('when the %s fails to load', (_name, mocked) => {
    it('should show the tab error instead of empty cards, and retry', async () => {
      mocked().mockResolvedValueOnce(fail(500));

      await renderTab();

      expect((await screen.findByRole('alert')).textContent).toBe(
        'Something went wrong while loading your payouts.',
      );
      expect(screen.queryByRole('heading', { name: 'Payout Wallet' })).toBeNull();
      fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
      expect(await screen.findByRole('heading', { name: 'Payout Wallet' })).toBeTruthy();
    });
  });

  describe('when the setup steps show', () => {
    it('should put the email first and the payout wallet second', async () => {
      await renderTab();
      await walletCard();

      expect(
        [...document.querySelectorAll(CARDS)].map(card => card.getAttribute('data-testid')),
      ).toEqual(['payouts-contact-card', 'payouts-wallet-card']);
    });

    it('should ask for the places with no cycle, the view the service checks', async () => {
      await renderTab();
      await walletCard();

      expect(api.getPlaces).toHaveBeenCalledWith();
    });
  });

  describe('when none of the places is in the program', () => {
    beforeEach(() => {
      vi.mocked(api.getEnrollment).mockResolvedValue(ok(ENROLLMENT));
      vi.mocked(api.getPlaces).mockResolvedValue(
        ok(myPlaces([place('a', { status: 'not_eligible', reasons: ['content_policy'] })])),
      );
    });

    it('should lock the email step, say an active place is needed, and send no code', async () => {
      await renderTab();
      const contact = await contactCard();
      const verify = await contact.findByRole('button', { name: 'Verify email' });

      expect(verify.hasAttribute('disabled')).toBe(true);
      fireEvent.click(verify);
      expect(screen.queryByRole('dialog')).toBeNull();
      expect(api.postEmailVerification).not.toHaveBeenCalled();
      expect(verify.getAttribute('aria-describedby')).toBe(lockHint(contact, EMAIL_NEEDS_SCENE).id);
    });

    it('should put the line in the title row after the title and VERIFY EMAIL, so it can fall under them', async () => {
      await renderTab();
      const contact = await contactCard();
      const verify = await contact.findByRole('button', { name: 'Verify email' });
      const title = contact.getByRole('heading', { name: 'Contact Information' });

      expect(verify.previousElementSibling).toBe(title.parentElement);
      expect(verify.nextElementSibling).toBe(lockHint(contact, EMAIL_NEEDS_SCENE));
    });

    it('should keep the payout wallet locked behind the email', async () => {
      await renderTab();

      expect(lockHint(await walletCard(), VERIFY_FIRST)).toBeTruthy();
    });

    describe('and the email is verified', () => {
      beforeEach(() => {
        vi.mocked(api.getEnrollment).mockResolvedValue(ok(VERIFIED));
      });

      it('should lock EDIT, keep the address, and say an active place is needed', async () => {
        await renderTab();
        const wallet = await walletCard();
        const edit = await wallet.findByRole('button', { name: 'Edit' });

        expect(edit.hasAttribute('disabled')).toBe(true);
        expect(wallet.getByTitle(WALLET.address)).toBeTruthy();
        expect(edit.getAttribute('aria-describedby')).toBe(lockHint(wallet, WALLET_NEEDS_SCENE).id);
        expect(screen.queryByText(VERIFY_FIRST)).toBeNull();
      });

      it('should lock ADD WALLET when no wallet is set', async () => {
        vi.mocked(api.getPayoutWallet).mockResolvedValue(fail(404));
        await renderTab();
        const wallet = await walletCard();
        const add = await wallet.findByRole('button', { name: 'Add wallet' });

        expect(add.hasAttribute('disabled')).toBe(true);
        fireEvent.click(add);
        expect(screen.queryByRole('dialog')).toBeNull();
        expect(add.getAttribute('aria-describedby')).toBe(lockHint(wallet, WALLET_NEEDS_SCENE).id);
      });
    });
  });

  describe('when the places fail to load', () => {
    it('should leave the payout wallet of a verified email to the service', async () => {
      vi.mocked(api.getPlaces).mockResolvedValue(fail(500));
      await renderTab();
      const wallet = await walletCard();

      expect((await wallet.findByRole('button', { name: 'Edit' })).hasAttribute('disabled')).toBe(
        false,
      );
      expect(wallet.queryByText(WALLET_NEEDS_SCENE)).toBeNull();
    });

    it('should show the steps and leave VERIFY EMAIL to the service', async () => {
      vi.mocked(api.getEnrollment).mockResolvedValue(ok(ENROLLMENT));
      vi.mocked(api.getPlaces).mockResolvedValue(fail(500));
      await renderTab();
      const contact = await contactCard();

      expect(
        (await contact.findByRole('button', { name: 'Verify email' })).hasAttribute('disabled'),
      ).toBe(false);
      expect(contact.queryByText(EMAIL_NEEDS_SCENE)).toBeNull();
      expect(screen.queryByRole('alert')).toBeNull();
    });
  });

  describe('when a cycle row is closed', () => {
    it('should point its disclosure at no rows, and span the place cell over three columns', async () => {
      await renderTab();
      const table = await screen.findByRole('table', { name: 'Payouts' });

      expect(cycleButton(table, 'Oct 1 - Oct 15, 2026').hasAttribute('aria-controls')).toBe(false);
      const controls = cycleButton(table, 'Oct 16 - Oct 31, 2026').getAttribute('aria-controls')!;
      expect(controls.split(' ').every(id => document.getElementById(id))).toBe(true);
      expect(
        within(document.getElementById(controls.split(' ')[0])!)
          .getAllByRole('cell')[0]
          .getAttribute('aria-colspan'),
      ).toBe('3');
    });
  });

  describe('when the email is verified', () => {
    it('should hide the badge and VERIFY EMAIL', async () => {
      vi.mocked(api.getEnrollment).mockResolvedValue(
        ok({ ...ENROLLMENT, email_verified_at: '2026-10-30T15:00:00Z' }),
      );

      await renderTab();

      expect(await screen.findByText('Jane Doe')).toBeTruthy();
      expect(screen.queryByText('Unverified')).toBeNull();
      expect((await contactCard()).queryByRole('button', { name: 'Verify email' })).toBeNull();
    });
  });

  describe('when a payout wallet is saved', () => {
    async function saveWallet(button: 'Add wallet' | 'Edit', dialogName: string, submit: string) {
      await renderTab();
      fireEvent.click(await (await walletCard()).findByRole('button', { name: button }));
      const dialog = await screen.findByRole('dialog', { name: dialogName });
      fireEvent.change(within(dialog).getByLabelText('Wallet address'), {
        target: { value: NEW_ADDRESS },
      });
      fireEvent.click(within(dialog).getByRole('button', { name: submit }));
    }

    it('should track a first wallet as an add that worked', async () => {
      vi.mocked(api.getPayoutWallet).mockResolvedValue(fail(404));
      vi.mocked(api.putPayoutWallet).mockResolvedValue(
        ok({ address: NEW_ADDRESS, updated_at: '2026-11-04T10:00:00Z' }, 201),
      );

      await saveWallet('Add wallet', 'Add payout wallet', 'Add wallet');

      await waitFor(() =>
        expect(tracked('Creator Fund Payout Wallet Saved')).toEqual([{ action: 'add', ok: true }]),
      );
      expectNoPrivateValuesTracked();
    });

    it('should track a changed wallet as an edit that worked', async () => {
      vi.mocked(api.putPayoutWallet).mockResolvedValue(
        ok({ address: NEW_ADDRESS, updated_at: '2026-11-04T10:00:00Z' }),
      );

      await saveWallet('Edit', 'Change payout wallet', 'Save');

      await waitFor(() =>
        expect(tracked('Creator Fund Payout Wallet Saved')).toEqual([{ action: 'edit', ok: true }]),
      );
      expectNoPrivateValuesTracked();
    });

    it('should say the wallet was kept when the service could not email the change', async () => {
      vi.mocked(api.putPayoutWallet).mockResolvedValue({
        ...fail(502),
        body: { error: 'email_unavailable' },
      });

      await saveWallet('Edit', 'Change payout wallet', 'Save');

      const dialog = screen.getByRole('dialog', { name: 'Change payout wallet' });
      expect((await within(dialog).findByRole('alert')).textContent).toBe(
        "We couldn't email you about this change, so your wallet wasn't saved. Please try again.",
      );
    });

    it('should say an active place is needed when the service finds none, and lock the step', async () => {
      vi.mocked(api.putPayoutWallet).mockResolvedValue({
        ...fail(403),
        body: { error: 'no_active_place' },
      });
      vi.mocked(api.getPlaces)
        .mockResolvedValueOnce(ok(myPlaces([place('a')])))
        .mockResolvedValue(ok(myPlaces([])));

      await saveWallet('Edit', 'Change payout wallet', 'Save');

      const dialog = screen.getByRole('dialog', { name: 'Change payout wallet' });
      expect((await within(dialog).findByRole('alert')).textContent).toBe(WALLET_NEEDS_SCENE);
      const wallet = await walletCard();
      await waitFor(() => expect(lockHint(wallet, WALLET_NEEDS_SCENE)).toBeTruthy());
    });

    it('should track a refused wallet as a save that failed', async () => {
      vi.mocked(api.putPayoutWallet).mockResolvedValue({
        ...fail(400),
        body: { error: 'invalid_address' },
      });

      await saveWallet('Edit', 'Change payout wallet', 'Save');

      await waitFor(() =>
        expect(tracked('Creator Fund Payout Wallet Saved')).toEqual([
          { action: 'edit', ok: false },
        ]),
      );
      expectNoPrivateValuesTracked();
    });
  });

  describe('when a second notice comes while the first still shows', () => {
    const SECOND_ADDRESS = '0x0000000000000000000000000000000000000003';

    async function save(open: string, dialogName: string, submit: string, address: string) {
      fireEvent.click(await (await walletCard()).findByRole('button', { name: open }));
      const dialog = await screen.findByRole('dialog', { name: dialogName });
      fireEvent.change(within(dialog).getByLabelText('Wallet address'), {
        target: { value: address },
      });
      fireEvent.click(within(dialog).getByRole('button', { name: submit }));
      await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    }

    beforeEach(() => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      vi.mocked(api.putPayoutWallet).mockImplementation(async ({ address }) =>
        ok({ address, updated_at: '2026-11-04T10:00:00Z' }),
      );
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it.each([
      [
        'a wallet is added, then changed',
        fail(404),
        ['Add wallet', 'Add payout wallet', 'Add wallet'],
      ],
      ['the wallet is changed twice', ok(WALLET), ['Edit', 'Change payout wallet', 'Save']],
    ] as const)(
      'should show the second for its full time when %s',
      async (_case, firstWallet, [open, dialogName, submit]) => {
        vi.mocked(api.getPayoutWallet).mockResolvedValue(firstWallet);
        await renderTab();
        const status = await screen.findByRole('status');

        await save(open, dialogName, submit, NEW_ADDRESS);
        act(() => vi.advanceTimersByTime(3_000));
        await save('Edit', 'Change payout wallet', 'Save', SECOND_ADDRESS);
        act(() => vi.advanceTimersByTime(4_000));

        expect(status.textContent).toBe('Payout wallet updated');
        act(() => vi.advanceTimersByTime(1_500));
        expect(status.textContent).toBe('');
      },
    );
  });

  describe('when a paid transaction is opened', () => {
    it('should track it once, with no properties', async () => {
      await renderTab();
      const table = await screen.findByRole('table', { name: 'Payouts' });

      fireEvent.click(within(table).getByRole('button', { name: 'View transaction on Etherscan' }));

      expect(tracked('Creator Fund Transaction Viewed')).toEqual([undefined]);
      expectNoPrivateValuesTracked();
    });
  });
});
