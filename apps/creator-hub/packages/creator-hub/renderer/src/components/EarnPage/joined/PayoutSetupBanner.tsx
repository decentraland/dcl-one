import { useId } from 'react';
import Button from '@ui/atoms/Button';

import { useSelector } from '#store';
import { selectors, type PayoutSetupStep } from '/@/modules/store/creatorFund';
import { t } from '/@/modules/store/translation/utils';

import { PayoutWalletIcon } from '../icons';

import parts from './parts.module.css';
import styles from './PayoutSetupBanner.module.css';

/** Asks for the payout wallet while a Payouts setup step is pending; its button hands that step to `onAction`. */
export function PayoutSetupBanner({ onAction }: { onAction: (step: PayoutSetupStep) => void }) {
  const titleId = useId();
  const step = useSelector(selectors.getPayoutSetupStep);
  if (!step) return null;
  return (
    <section
      className={styles.banner}
      aria-labelledby={titleId}
      data-payout-setup-banner
    >
      <div
        className={styles.glow}
        aria-hidden
      />
      <span
        className={styles.badge}
        aria-hidden
      >
        <PayoutWalletIcon />
      </span>
      <div className={styles.copy}>
        <h2
          id={titleId}
          className={styles.title}
        >
          {t('earn.joined.setup_banner.wallet.title')}
        </h2>
        <p className={styles.description}>{t('earn.joined.setup_banner.wallet.description')}</p>
      </div>
      <Button
        variant="primary"
        className={parts.setupButton}
        onClick={() => onAction(step)}
      >
        {t('earn.joined.payouts.wallet.add')}
      </Button>
    </section>
  );
}
