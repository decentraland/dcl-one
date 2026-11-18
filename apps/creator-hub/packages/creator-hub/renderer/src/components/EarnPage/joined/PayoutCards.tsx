import { useId } from 'react';

import Button from '@ui/atoms/Button';

import type { Enrollment, PayoutWallet } from '/shared/types/creator-fund';

import { t } from '/@/modules/store/translation/utils';

import { CopyToClipboard } from '../../CopyToClipboard';

import { CopyIcon, WarningIcon } from '../icons';

import { InfoTip } from './InfoTip';
import { shortAddress } from './wallet';
import styles from './PayoutsTab.module.css';

const GET_A_WALLET_URL = 'https://docs.decentraland.org/blockchain-integration/get-a-wallet';

/**
 * The payout wallet: its short address as a chip that copies it, and EDIT; ADD WALLET when none is set.
 * @param props.wallet - `undefined` while it loads.
 */
export function WalletCard({
  wallet,
  editRef,
  onAdd,
  onChange,
}: {
  wallet: PayoutWallet | null | undefined;
  editRef?: React.Ref<HTMLButtonElement>;
  onAdd: () => void;
  onChange: () => void;
}) {
  const titleId = useId();
  return (
    <section
      className={styles.card}
      aria-labelledby={titleId}
      data-testid="payouts-wallet-card"
    >
      <div className={styles.cardHeader}>
        <h2
          className={styles.cardTitle}
          id={titleId}
        >
          {t('earn.joined.payouts.wallet.title')}
        </h2>
        <InfoTip
          label="earn.joined.payouts.wallet.info_label"
          text="earn.joined.payouts.wallet.info"
          href={GET_A_WALLET_URL}
        />
      </div>
      {wallet && (
        <>
          <CopyToClipboard
            className={styles.addressChip}
            text={wallet.address}
            label={t('earn.joined.payouts.wallet.copy')}
            showPopup
          >
            <span
              className={styles.address}
              title={wallet.address}
            >
              {shortAddress(wallet.address)}
            </span>
            <CopyIcon />
          </CopyToClipboard>
          <Button
            ref={editRef}
            className={styles.cornerButton}
            variant="outline"
            onClick={onChange}
          >
            {t('earn.joined.payouts.wallet.edit')}
          </Button>
        </>
      )}
      {wallet === null && (
        <Button
          className={styles.addWalletButton}
          variant="primary"
          onClick={onAdd}
        >
          {t('earn.joined.payouts.wallet.add')}
        </Button>
      )}
    </section>
  );
}

/** The enrollment's name and read-only email, with VERIFY EMAIL until the email is verified. */
export function ContactCard({
  enrollment,
  titleRef,
  onVerify,
}: {
  enrollment: Enrollment | null;
  titleRef?: React.Ref<HTMLHeadingElement>;
  onVerify: () => void;
}) {
  const titleId = useId();
  const unverified = !!enrollment && !enrollment.email_verified_at;
  return (
    <section
      className={styles.card}
      aria-labelledby={titleId}
      data-testid="payouts-contact-card"
    >
      <div className={styles.cardHeader}>
        <h2
          className={styles.cardTitle}
          id={titleId}
          ref={titleRef}
          tabIndex={-1}
        >
          {t('earn.joined.payouts.contact.title')}
        </h2>
      </div>
      {unverified && (
        <Button
          className={styles.cornerButton}
          variant="outline"
          onClick={onVerify}
        >
          {t('earn.joined.payouts.contact.verify')}
        </Button>
      )}
      {enrollment && (
        <div className={styles.contactValues}>
          <div className={styles.contactValue}>
            <span className={styles.contactLabel}>
              {t('earn.joined.payouts.contact.full_name')}
            </span>
            <span
              className={styles.contactText}
            >{`${enrollment.first_name} ${enrollment.last_name}`}</span>
          </div>
          <div className={styles.contactValue}>
            <span className={styles.contactLabel}>
              {t('earn.joined.payouts.contact.email')}
              {unverified && (
                <span className={styles.unverifiedBadge}>
                  <WarningIcon />
                  {t('earn.joined.payouts.contact.unverified')}
                </span>
              )}
            </span>
            <span className={styles.contactText}>{enrollment.email}</span>
          </div>
        </div>
      )}
    </section>
  );
}
