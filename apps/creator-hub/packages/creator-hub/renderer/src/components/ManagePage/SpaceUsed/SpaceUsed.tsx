import { useId, useState } from 'react';

import { useSelector } from '#store';
import { toMB } from '/@/modules/file';
import { getCurrentLocale, t } from '/@/modules/store/translation/utils';

import { WorldsYourStorageModal } from '../../Modals/WorldsYourStorageModal';
import { InfoTip } from '../InfoTip';

import styles from './SpaceUsed.module.css';

/** The WORLDS tab's storage bar: space used by the account's Worlds out of its allowance. */
export function SpaceUsed() {
  const { storageStats, accountHoldings } = useSelector(state => state.management);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const labelId = useId();

  if (!storageStats) return null;

  const max = toMB(Number(storageStats.maxAllowedSpace) || 0);
  const used = toMB(Number(storageStats.usedSpace) || 0);
  const percent = max > 0 ? Math.min(100, (used * 100) / max) : 0;
  const usage = (b: (chunks: string[]) => unknown) =>
    t('manage.worlds_storage.usage', {
      used: getCurrentLocale().formatNumber(used, {
        minimumFractionDigits: 2,
        maximumFractionDigits: 2,
      }),
      max: getCurrentLocale().formatNumber(max, { maximumFractionDigits: 0 }),
      b,
    });

  return (
    <div className={styles.root}>
      <div className={styles.labelRow}>
        <span
          id={labelId}
          className={styles.label}
        >
          {t('manage.worlds_storage.used')}
        </span>
        <InfoTip
          label={t('manage.worlds_storage.info_label')}
          size="small"
        >
          {t('manage.worlds_storage.description')}{' '}
          <button
            type="button"
            className={styles.detailsButton}
            onClick={() => setDetailsOpen(true)}
          >
            {t('manage.worlds_storage.link')}
          </button>
        </InfoTip>
      </div>
      <div className={styles.valueRow}>
        <span className={styles.value}>
          {usage(chunks => (
            <b>{chunks}</b>
          ))}
        </span>
        <div
          className={styles.track}
          role="progressbar"
          aria-labelledby={labelId}
          aria-valuemin={0}
          aria-valuemax={max}
          aria-valuenow={used}
          aria-valuetext={[usage(chunks => chunks.join(''))].flat().join('')}
        >
          <span
            className={styles.fill}
            style={{ width: `${percent}%` }}
          />
        </div>
      </div>
      <WorldsYourStorageModal
        open={detailsOpen}
        onClose={() => setDetailsOpen(false)}
        stats={storageStats}
        accountHoldings={accountHoldings}
      />
    </div>
  );
}
