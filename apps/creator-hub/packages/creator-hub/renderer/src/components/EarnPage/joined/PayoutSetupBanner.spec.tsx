import { useState } from 'react';
import { Provider } from 'react-redux';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { flatten } from '/@/modules/store/translation/flatten';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import type { Enrollment, PayoutWallet } from '/shared/types/creator-fund';

import { actions } from '/@/modules/store/creatorFund';
import { FeatureFlag, fetchFeatureFlags } from '/@/modules/store/featureFlags';
import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';
import * as api from '/@/lib/creatorFundApi';

import { tracked } from '../../../../tests/utils/analytics';
import { TestProviders } from '../../../../tests/utils/TestProviders';
import { createTestStore } from '../../../../tests/utils/testStore';
import { JoinedPage, type JoinedTab } from '../JoinedPage';
import { CYCLES, fail, myPlaces, ok, place } from './fixtures';

vi.mock('/@/lib/creatorFundApi', () => ({
  getEnrollment: vi.fn(),
  getTerms: vi.fn(),
  getCycles: vi.fn(),
  getPayouts: vi.fn(),
  getPayoutWallet: vi.fn(),
  getPlaces: vi.fn(),
}));
vi.mock('./LeaderboardTab', () => ({ LeaderboardTab: () => <p>leaderboard panel</p> }));
vi.mock('./MyPlacesTab', () => ({ MyPlacesTab: () => <p>my places panel</p> }));

const UNVERIFIED: Enrollment = {
  status: 'active',
  enrolled_at: '2026-10-30T14:02:11Z',
  terms_version: 1,
  first_name: 'Jane',
  last_name: 'Doe',
  email: 'jane.doe@example.com',
  email_verified_at: null,
};

const VERIFIED: Enrollment = { ...UNVERIFIED, email_verified_at: '2026-11-02T10:00:00Z' };

const WALLET: PayoutWallet = {
  address: '0x0000000000000000000000000000000000000002',
  updated_at: '2026-11-02T10:00:00Z',
};

const WALLET_STEP = 'Set up your payout wallet';
const ADD_WALLET_DIALOG = 'Add payout wallet';

const never = () => new Promise<never>(() => {});

function StatefulJoinedPage({ initialTab }: { initialTab: JoinedTab }) {
  const [tab, setTab] = useState(initialTab);
  return (
    <JoinedPage
      tab={tab}
      onTabChange={setTab}
    />
  );
}

async function renderPage(tab: JoinedTab, { payout = true } = {}) {
  const store = createTestStore();
  store.dispatch(
    fetchFeatureFlags.fulfilled(
      { flags: { [FeatureFlag.CREATOR_FUND_PAYOUT]: payout }, variants: {} },
      '',
    ),
  );
  await act(async () => {
    await Promise.all([
      store.dispatch(actions.fetchEnrollment()),
      store.dispatch(actions.fetchCycles()),
      store.dispatch(actions.fetchPayoutWallet()),
      store.dispatch(actions.fetchHasActivePlace()),
    ]);
  });
  render(
    <Provider store={store}>
      <TestProviders>
        <StatefulJoinedPage initialTab={tab} />
      </TestProviders>
    </Provider>,
  );
  return store;
}

const banner = (name: string) => screen.getByRole('region', { name });
const bannerButton = (name: string, label: string) =>
  within(banner(name)).getByRole('button', { name: label });

const pause = (ms: number) => act(() => new Promise<void>(resolve => setTimeout(resolve, ms)));

function press(button: HTMLElement) {
  button.focus();
  fireEvent.click(button);
}

vi.setConfig({ testTimeout: 15_000 });

describe('PayoutSetupBanner', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  beforeEach(() => {
    vi.mocked(api.getEnrollment).mockResolvedValue(ok(VERIFIED));
    vi.mocked(api.getTerms).mockReturnValue(never());
    vi.mocked(api.getCycles).mockResolvedValue(ok(CYCLES));
    vi.mocked(api.getPayouts).mockResolvedValue(ok({ payouts: [] }));
    vi.mocked(api.getPayoutWallet).mockResolvedValue(fail(404));
    vi.mocked(api.getPlaces).mockResolvedValue(ok(myPlaces([place('a')])));
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  describe('when the email is verified and no payout wallet is set', () => {
    it.each(['leaderboard', 'my_places', 'payouts'] as const)(
      'should ask for the payout wallet on %s, above the tabs',
      async tab => {
        await renderPage(tab);

        const region = banner(WALLET_STEP);
        expect(within(region).getByRole('heading', { level: 2 }).textContent).toBe(WALLET_STEP);
        expect(
          within(region).getByText('Add a wallet to receive your Creator Fund payouts in USDC.'),
        ).toBeTruthy();
        expect(within(region).getByRole('button', { name: 'Add wallet' })).toBeTruthy();
        expect(
          region.compareDocumentPosition(screen.getByRole('tablist')) &
            Node.DOCUMENT_POSITION_FOLLOWING,
        ).toBeTruthy();
      },
    );
  });

  describe('when the email is not verified', () => {
    it('should still ask for the payout wallet, never for the email', async () => {
      vi.mocked(api.getEnrollment).mockResolvedValue(ok(UNVERIFIED));
      await renderPage('leaderboard');

      const region = banner(WALLET_STEP);
      expect(
        within(region).getByText('Add a wallet to receive your Creator Fund payouts in USDC.'),
      ).toBeTruthy();
      expect(within(region).getByRole('button', { name: 'Add wallet' })).toBeTruthy();
      expect(within(region).queryByText(/email/i)).toBeNull();
    });
  });

  describe('when no step can be taken', () => {
    it.each<[string, () => void, { payout?: boolean }]>([
      [
        'no place is in the program',
        () =>
          vi
            .mocked(api.getPlaces)
            .mockResolvedValue(ok(myPlaces([place('a', { status: 'not_eligible' })]))),
        {},
      ],
      [
        'the setup is complete',
        () => vi.mocked(api.getPayoutWallet).mockResolvedValue(ok(WALLET)),
        {},
      ],
      ['the Payouts flag is off', () => {}, { payout: false }],
    ])('should show no banner when %s', async (_name, arrange, options) => {
      arrange();
      await renderPage('leaderboard', options);

      expect(document.querySelector('[data-payout-setup-banner]')).toBeNull();
    });
  });

  describe('when the setup requests reload', () => {
    it('should keep the banner and the Payouts dot while the wallet and the places reload', async () => {
      const store = await renderPage('leaderboard');
      const region = banner(WALLET_STEP);
      vi.mocked(api.getPayoutWallet).mockReturnValue(never());
      vi.mocked(api.getPlaces).mockReturnValue(never());

      act(() => {
        void store.dispatch(actions.fetchPayoutWallet());
        void store.dispatch(actions.fetchHasActivePlace());
      });

      expect(banner(WALLET_STEP)).toBe(region);
      expect(screen.getByRole('tab', { name: 'PAYOUTS, setup not finished' })).toBeTruthy();
    });

    it('should keep the same banner while arriving on Payouts refetches the wallet and the places', async () => {
      await renderPage('my_places');
      const region = banner(WALLET_STEP);
      vi.mocked(api.getPayoutWallet).mockReturnValue(never());
      vi.mocked(api.getPlaces).mockReturnValue(never());

      fireEvent.click(screen.getByRole('tab', { name: 'PAYOUTS, setup not finished' }));

      expect(screen.getByRole('tab', { name: 'Payouts', selected: true })).toBeTruthy();
      expect(region.isConnected).toBe(true);
      expect(banner(WALLET_STEP)).toBe(region);
    });
  });

  describe('when its button is pressed', () => {
    it('should switch to Payouts and open Add payout wallet once the tab loads', async () => {
      await renderPage('my_places');

      press(bannerButton(WALLET_STEP, 'Add wallet'));

      expect(screen.getByRole('tab', { name: 'Payouts', selected: true })).toBeTruthy();
      expect(await screen.findByRole('dialog', { name: ADD_WALLET_DIALOG })).toBeTruthy();
      expect(tracked('Creator Fund Payout Banner Clicked')).toEqual([{ step: 'add_wallet' }]);
      expect(tracked('Creator Fund Tab Viewed')).toEqual([{ tab: 'payouts' }]);
    });

    it('should only switch to Payouts while the email is not verified', async () => {
      vi.mocked(api.getEnrollment).mockResolvedValue(ok(UNVERIFIED));
      await renderPage('leaderboard');
      const button = bannerButton(WALLET_STEP, 'Add wallet');

      press(button);

      expect(screen.getByRole('tab', { name: 'Payouts', selected: true })).toBeTruthy();
      await screen.findByTestId('payouts-contact-card');
      await pause(150);
      expect(screen.queryByRole('dialog')).toBeNull();
      expect(document.activeElement).toBe(button);
      expect(tracked('Creator Fund Payout Banner Clicked')).toEqual([{ step: 'verify_email' }]);
      expect(tracked('Creator Fund Email Verification Started')).toEqual([]);
    });

    it('should open the dialog at once when Payouts is already the tab', async () => {
      await renderPage('payouts');
      await screen.findByTestId('payouts-wallet-card');

      press(bannerButton(WALLET_STEP, 'Add wallet'));

      expect(await screen.findByRole('dialog', { name: ADD_WALLET_DIALOG })).toBeTruthy();
      expect(tracked('Creator Fund Tab Viewed')).toEqual([]);
    });

    it('should open no dialog when the reloaded answer no longer names the step', async () => {
      await renderPage('my_places');
      vi.mocked(api.getPayoutWallet).mockResolvedValue(ok(WALLET));

      press(bannerButton(WALLET_STEP, 'Add wallet'));

      await waitFor(() => expect(document.querySelector('[data-payout-setup-banner]')).toBeNull());
      await screen.findByTestId('payouts-wallet-card');
      expect(screen.queryByRole('dialog')).toBeNull();
    });

    it('should open no dialog when the tab fails to load', async () => {
      await renderPage('my_places');
      vi.mocked(api.getPayouts).mockResolvedValue(fail(500));

      press(bannerButton(WALLET_STEP, 'Add wallet'));

      expect(await screen.findByRole('button', { name: 'Retry' })).toBeTruthy();
      expect(screen.queryByRole('dialog')).toBeNull();
    });

    it('should open no dialog after Retry once the failed request was dropped', async () => {
      await renderPage('my_places');
      vi.mocked(api.getPayouts).mockResolvedValue(fail(500));
      press(bannerButton(WALLET_STEP, 'Add wallet'));
      const retry = await screen.findByRole('button', { name: 'Retry' });
      vi.mocked(api.getPayouts).mockResolvedValue(ok({ payouts: [] }));

      fireEvent.click(retry);

      await screen.findByTestId('payouts-wallet-card');
      expect(screen.queryByRole('dialog')).toBeNull();
    });

    it('should give focus back to the banner button on Cancel', async () => {
      await renderPage('my_places');
      press(bannerButton(WALLET_STEP, 'Add wallet'));
      const dialog = await screen.findByRole('dialog', { name: ADD_WALLET_DIALOG });

      fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));

      await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
      expect(document.activeElement).toBe(bannerButton(WALLET_STEP, 'Add wallet'));
    });
  });
});
