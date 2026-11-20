import { useId, useState } from 'react';
import { Tab, Tabs } from '@ui/atoms/Tabs';

import { analytics } from '#preload';
import { useSelector } from '#store';
import { useFeatureFlags } from '/@/hooks/useFeatureFlags';
import { selectors, type PayoutSetupStep } from '/@/modules/store/creatorFund';
import { FeatureFlag } from '/@/modules/store/featureFlags';
import type { TranslationPath } from '/@/modules/store/translation/types';
import { t } from '/@/modules/store/translation/utils';

import { NotificationDot } from '../NotificationDot';

import { EarnHeader } from './joined/EarnHeader';
import { LeaderboardTab } from './joined/LeaderboardTab';
import { MyPlacesTab } from './joined/MyPlacesTab';
import { PayoutSetupBanner } from './joined/PayoutSetupBanner';
import { PayoutsLocked } from './joined/PayoutsLocked';
import { PayoutsTab, type PayoutSetupRequest } from './joined/PayoutsTab';
import styles from './JoinedPage.module.css';

export type JoinedTab = 'leaderboard' | 'my_places' | 'payouts';

const TABS: { id: JoinedTab; label: TranslationPath }[] = [
  { id: 'leaderboard', label: 'earn.joined.tabs.leaderboard' },
  { id: 'my_places', label: 'earn.joined.tabs.my_places' },
  { id: 'payouts', label: 'earn.joined.tabs.payouts' },
];

type PanelProps = {
  payoutEnabled: boolean;
  setupRequest: PayoutSetupRequest | null;
  onSetupRequestHandled: () => void;
};

const PANELS: Record<JoinedTab, (props: PanelProps) => React.ReactNode> = {
  leaderboard: () => <LeaderboardTab />,
  my_places: () => <MyPlacesTab />,
  payouts: ({ payoutEnabled, setupRequest, onSetupRequestHandled }) =>
    payoutEnabled ? (
      <PayoutsTab
        request={setupRequest}
        onRequestHandled={onSetupRequestHandled}
      />
    ) : (
      <PayoutsLocked />
    ),
};

/** The Creator Fund page for a joined user: the Leaderboard, My Places and Payouts tabs. */
export function JoinedPage({
  tab,
  onTabChange,
}: {
  tab: JoinedTab;
  onTabChange: (tab: JoinedTab) => void;
}) {
  const id = useId();
  const payoutEnabled = useFeatureFlags().isEnabled(FeatureFlag.CREATOR_FUND_PAYOUT);
  const hasClosedCycle = useSelector(state => !!selectors.getLatestClosedCycle(state));
  const payoutSetupPending = useSelector(selectors.isPayoutSetupPending);
  const [setupRequest, setSetupRequest] = useState<PayoutSetupRequest | null>(null);
  const fillsWindow = useSelector(state =>
    tab === 'leaderboard'
      ? state.creatorFund.cycles.status === 'succeeded' && !selectors.getLatestClosedCycle(state)
      : tab === 'payouts' &&
        (!payoutEnabled || state.creatorFund.payouts.data?.payouts.length === 0),
  );
  const viewTab = (next: JoinedTab) => {
    void analytics.track('Creator Fund Tab Viewed', {
      tab: next === 'leaderboard' && !hasClosedCycle ? 'home' : next,
    });
    onTabChange(next);
  };
  const takeSetupStep = (step: PayoutSetupStep) => {
    void analytics.track('Creator Fund Payout Banner Clicked', { step });
    if (tab !== 'payouts') viewTab('payouts');
    if (step === 'add_wallet') setSetupRequest({ step, id: Date.now() });
  };
  return (
    <div
      className={styles.page}
      data-bottom={fillsWindow ? 'flush' : 'padded'}
    >
      <svg
        className={styles.gradientDefs}
        aria-hidden
      >
        <defs>
          <linearGradient
            id="cf-cerise"
            x1="0"
            y1="0"
            x2="0"
            y2="1"
          >
            <stop
              offset="0"
              stopColor="var(--primary)"
            />
            <stop
              offset="1"
              stopColor="var(--lavender)"
            />
          </linearGradient>
        </defs>
      </svg>
      <EarnHeader />
      <PayoutSetupBanner onAction={takeSetupStep} />
      <Tabs
        className={styles.sectionTabs}
        value={tab}
        onChange={value => viewTab(value as JoinedTab)}
        aria-label={t('earn.joined.sections')}
      >
        {TABS.map(({ id: tabId, label }) => {
          const dotted = tabId === 'payouts' && payoutSetupPending && tab !== 'payouts';
          return (
            <Tab
              key={tabId}
              value={tabId}
              id={`${id}-${tabId}`}
              aria-controls={`${id}-panel`}
              aria-label={dotted ? t('earn.joined.tabs.payouts_pending') : undefined}
            >
              <span className={styles.tabLabel}>
                {t(tabId === 'leaderboard' && !hasClosedCycle ? 'earn.joined.tabs.home' : label)}
                {dotted && <NotificationDot />}
              </span>
            </Tab>
          );
        })}
      </Tabs>
      <div
        className={styles.tabPanel}
        role="tabpanel"
        id={`${id}-panel`}
        aria-labelledby={`${id}-${tab}`}
      >
        {PANELS[tab]({
          payoutEnabled,
          setupRequest,
          onSetupRequestHandled: () => setSetupRequest(null),
        })}
      </div>
    </div>
  );
}
