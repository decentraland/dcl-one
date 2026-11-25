import emptyArt from '/assets/images/earn/payouts-empty.webp';

import { useSelector } from '#store';
import { selectors } from '/@/modules/store/creatorFund';
import { formatCycleDays } from '/@/modules/store/creatorFund/utils';
import { t } from '/@/modules/store/translation/utils';

import styles from './PayoutsTab.module.css';

/** Before the first payout: when it comes, over the character art. */
export function PayoutsEmpty() {
  const locale = useSelector(state => state.translation.locale);
  const cycle = useSelector(selectors.getFirstPayoutCycle);
  return (
    <section className={styles.emptyArea}>
      <img
        className={styles.emptyArt}
        src={emptyArt}
        alt=""
        width={1361}
        height={822}
      />
      <div className={styles.emptyCopy}>
        <div className={styles.emptyHead}>
          <h2 className={styles.emptyTitle}>{t('earn.joined.payouts.empty.title')}</h2>
          {cycle && (
            <p className={styles.emptyLine}>
              {t('earn.joined.payouts.empty.first_payout', {
                cycle: formatCycleDays(cycle, locale),
              })}
            </p>
          )}
        </div>
        <p className={styles.emptyBody}>{t('earn.joined.payouts.empty.body')}</p>
      </div>
    </section>
  );
}
