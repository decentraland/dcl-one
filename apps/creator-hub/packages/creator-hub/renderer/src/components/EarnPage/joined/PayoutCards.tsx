import { useId } from 'react';

import Button from '@ui/atoms/Button';

import type { Enrollment, PayoutWallet } from '/shared/types/creator-fund';

import { t } from '/@/modules/store/translation/utils';

import { CopyToClipboard } from '../../CopyToClipboard';

import { CopyIcon, WarningIcon } from '../icons';

import { InfoTip } from './InfoTip';
import { ExplainedChip } from './parts';
import { shortAddress } from './wallet';
import styles from './PayoutsTab.module.css';

const GET_A_WALLET_URL = 'https://docs.decentraland.org/blockchain-integration/get-a-wallet';

/** A Payouts card's title; `titleRef` makes it a focus target after a modal closes. */
function CardTitle({
  id,
  titleRef,
  children,
}: {
  id: string;
  titleRef?: React.Ref<HTMLHeadingElement>;
  children: React.ReactNode;
}) {
  return (
    <h2
      className={styles.cardTitle}
      id={id}
      ref={titleRef}
      tabIndex={titleRef ? -1 : undefined}
    >
      {children}
    </h2>
  );
}

/**
 * The payout wallet, the second setup step: its short address as a chip that copies it, and EDIT; ADD WALLET when
 * none is set. Locked until the email is verified, since every change is emailed to it.
 * @param props.wallet - `undefined` while it loads.
 */
export function WalletCard({
  wallet,
  locked,
  editRef,
  onAdd,
  onChange,
}: {
  wallet: PayoutWallet | null | undefined;
  locked: boolean;
  editRef?: React.Ref<HTMLButtonElement>;
  onAdd: () => void;
  onChange: () => void;
}) {
  const titleId = useId();
  return (
    <section
      className={styles.card}
      aria-labelledby={titleId}
      data-locked={locked || undefined}
      data-testid="payouts-wallet-card"
    >
      <div className={styles.cardTop}>
        <div className={styles.cardHeader}>
          <CardTitle id={titleId}>{t('earn.joined.payouts.wallet.title')}</CardTitle>
          <InfoTip
            label="earn.joined.payouts.wallet.info_label"
            text="earn.joined.payouts.wallet.info"
            href={GET_A_WALLET_URL}
          />
          {locked && (
            <span className={styles.headerChip}>
              <ExplainedChip
                status="unavailable"
                title={t('earn.joined.payouts.wallet.unavailable')}
              />
            </span>
          )}
        </div>
        {wallet && (
          <Button
            ref={editRef}
            className={styles.cornerButton}
            variant="outline"
            disabled={locked}
            onClick={onChange}
          >
            {t('earn.joined.payouts.wallet.edit')}
          </Button>
        )}
      </div>
      {wallet && (
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
      )}
      {wallet === null && (
        <Button
          className={styles.addWalletButton}
          variant="primary"
          disabled={locked}
          onClick={onAdd}
        >
          {t('earn.joined.payouts.wallet.add')}
        </Button>
      )}
    </section>
  );
}

/**
 * The enrollment's name and read-only email, the first setup step, with VERIFY EMAIL until the email is verified.
 * Locked while none of the creator's places is in the program: the service sends no code then.
 */
export function ContactCard({
  enrollment,
  locked,
  titleRef,
  onVerify,
}: {
  enrollment: Enrollment | null;
  locked: boolean;
  titleRef?: React.Ref<HTMLHeadingElement>;
  onVerify: () => void;
}) {
  const titleId = useId();
  const unverified = !!enrollment && !enrollment.email_verified_at;
  return (
    <section
      className={styles.card}
      aria-labelledby={titleId}
      data-locked={locked || undefined}
      data-testid="payouts-contact-card"
    >
      <div className={styles.cardTop}>
        <div className={styles.cardHeader}>
          <CardTitle
            id={titleId}
            titleRef={titleRef}
          >
            {t('earn.joined.payouts.contact.title')}
          </CardTitle>
          {locked && (
            <span className={styles.headerChip}>
              <ExplainedChip
                status="unavailable"
                title={t('earn.joined.payouts.contact.unavailable')}
              />
            </span>
          )}
        </div>
        {unverified && (
          <Button
            className={styles.verifyEmailButton}
            variant="primary"
            disabled={locked}
            onClick={onVerify}
          >
            {t('earn.joined.payouts.contact.verify')}
          </Button>
        )}
      </div>
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
