import { useId, useState } from 'react';

import type { Payout } from '/shared/types/creator-fund';

import { analytics, misc } from '#preload';
import { useSelector } from '#store';
import { formatCycleLabel } from '/@/modules/store/creatorFund/utils';
import { t } from '/@/modules/store/translation/utils';

import { CollapseIcon, ExpandIcon, MapPinIcon, TransactionIcon } from '../icons';

import { formatCount, formatUsdc, placeLocation, placeTitle } from './format';
import { useWorldSceneCounts } from './useWorldSceneCounts';
import leaderboard from './LeaderboardTab.module.css';
import styles from './PayoutsTab.module.css';

const COLUMNS = ['cycle', 'status', 'paid_places', 'amount', 'transaction'] as const;

const ETHERSCAN_TX = 'https://etherscan.io/tx/';

function Usdc({ amount }: { amount: string }) {
  const locale = useSelector(state => state.translation.locale);
  return (
    <span
      className={styles.figure}
      role="cell"
    >
      {t('earn.joined.payouts.table.usdc', { amount: formatUsdc(amount, locale) })}
    </span>
  );
}

function TransactionLink({ hash }: { hash: string | null }) {
  return (
    <span
      className={styles.transactionCell}
      role="cell"
    >
      {hash && (
        <button
          className={styles.transactionButton}
          type="button"
          aria-label={t('earn.joined.payouts.table.view_transaction')}
          onClick={() => {
            void analytics.track('Creator Fund Transaction Viewed', undefined);
            void misc.openExternal(`${ETHERSCAN_TX}${hash}`);
          }}
        >
          <TransactionIcon />
        </button>
      )}
    </span>
  );
}

function PayoutRows({
  payout,
  open,
  onToggle,
  sceneCounts,
}: {
  payout: Payout;
  open: boolean;
  onToggle: () => void;
  sceneCounts: Record<string, number>;
}) {
  const locale = useSelector(state => state.translation.locale);
  const id = useId();
  const placeIds = payout.places.map((_, index) => `${id}-${index}`);
  return (
    <>
      <div
        className={styles.cycleRow}
        role="row"
      >
        <span
          className={styles.cycleCell}
          role="cell"
        >
          <button
            className={styles.disclosure}
            type="button"
            aria-expanded={open}
            aria-controls={open ? placeIds.join(' ') : undefined}
            onClick={onToggle}
          >
            {open ? <CollapseIcon /> : <ExpandIcon />}
            <span className={styles.disclosureLabel}>{formatCycleLabel(payout.cycle, locale)}</span>
          </button>
        </span>
        <span role="cell">
          <span
            className={styles.statusPill}
            data-status={payout.status}
          >
            {t(`earn.joined.payouts.table.${payout.status}`)}
          </span>
        </span>
        <span
          className={styles.figure}
          role="cell"
        >
          {formatCount(payout.paid_places, locale)}
        </span>
        <Usdc amount={payout.amount_usdc} />
        <TransactionLink hash={payout.transaction_hash} />
      </div>
      {open &&
        payout.places.map(({ place, amount_usdc }, index) => (
          <div
            key={place.id}
            className={styles.placeRow}
            id={placeIds[index]}
            role="row"
          >
            <div
              className={styles.placeSpan}
              role="cell"
              aria-colspan={3}
            >
              <div className={leaderboard.placeCell}>
                <img
                  className={leaderboard.thumbnail}
                  src={place.image ?? undefined}
                  alt=""
                  width={89}
                  height={50}
                  loading="lazy"
                />
                <div className={leaderboard.placeText}>
                  <span
                    className={leaderboard.placeName}
                    title={placeTitle(place)}
                  >
                    {placeTitle(place)}
                  </span>
                  <span className={leaderboard.placeWhere}>
                    <MapPinIcon />
                    <span
                      className={leaderboard.placeWhereText}
                      title={placeLocation(place, sceneCounts[place.world_name ?? ''])}
                    >
                      {placeLocation(place, sceneCounts[place.world_name ?? ''])}
                    </span>
                  </span>
                </div>
              </div>
            </div>
            <Usdc amount={amount_usdc} />
            <span role="cell" />
          </div>
        ))}
    </>
  );
}

/** The payouts, one disclosure row per cycle with its paid places; the newest starts open. */
export function PayoutsTable({ payouts }: { payouts: Payout[] }) {
  const [open, setOpen] = useState<ReadonlySet<number>>(() => new Set([payouts[0]?.cycle.id]));
  const sceneCounts = useWorldSceneCounts(
    payouts.flatMap(payout => payout.places.map(entry => entry.place)),
  );
  const toggle = (cycleId: number) =>
    setOpen(current => {
      const next = new Set(current);
      if (!next.delete(cycleId)) next.add(cycleId);
      return next;
    });

  return (
    <div
      className={styles.table}
      role="table"
      aria-label={t('earn.joined.payouts.table.label')}
    >
      <div role="rowgroup">
        <div
          className={styles.headerRow}
          role="row"
        >
          {COLUMNS.map(column => (
            <span
              key={column}
              className={styles.headerCell}
              role="columnheader"
            >
              {t(`earn.joined.payouts.table.${column}`)}
            </span>
          ))}
        </div>
      </div>
      <div role="rowgroup">
        {payouts.map(payout => (
          <PayoutRows
            key={payout.cycle.id}
            payout={payout}
            open={open.has(payout.cycle.id)}
            onToggle={() => toggle(payout.cycle.id)}
            sceneCounts={sceneCounts}
          />
        ))}
      </div>
    </div>
  );
}
