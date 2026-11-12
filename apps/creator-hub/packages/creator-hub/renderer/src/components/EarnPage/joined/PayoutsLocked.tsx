import cx from 'classnames';

import emptyWallet from '/assets/images/empty-wallet.svg';

import { t } from '/@/modules/store/translation/utils';

import payouts from './PayoutsTab.module.css';
import styles from './PayoutsLocked.module.css';

/** The Payouts tab in place of `PayoutsTab` while the `creatorhub-creator-fund-payout` flag is off. */
export function PayoutsLocked() {
  return (
    <div className={cx(payouts.panel, payouts.emptyPanel, styles.lockedPanel)}>
      <img
        className={styles.illustration}
        src={emptyWallet}
        alt=""
        width={80}
        height={80}
      />
      <h2 className={styles.title}>{t('earn.joined.payouts.locked')}</h2>
    </div>
  );
}
