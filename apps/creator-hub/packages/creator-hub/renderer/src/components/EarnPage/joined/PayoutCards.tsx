import { useId } from 'react';

import Button from '@ui/atoms/Button';

import type { Enrollment, PayoutWallet } from '/shared/types/creator-fund';

import type { TranslationPath } from '/@/modules/store/translation/types';
import { t } from '/@/modules/store/translation/utils';

import { CopyToClipboard } from '../../CopyToClipboard';

import { CopyIcon, WarningIcon } from '../icons';

import { InfoTip } from './InfoTip';
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

/** Why the payout wallet step is locked. */
export type WalletLock = 'unverified_email' | 'no_active_place';

const WALLET_LOCK_HINT: Record<WalletLock, TranslationPath> = {
  unverified_email: 'earn.joined.payouts.wallet.unavailable',
  no_active_place: 'earn.joined.payouts.wallet.no_active_place',
};

/**
 * The payout wallet, the second setup step: its short address as a chip that copies it, and EDIT; ADD WALLET when
 * none is set. While `lock` is set the buttons are disabled and a line beside them says why.
 * @param props.wallet - `undefined` while it loads.
 * @param props.lock - why the step is locked, or `null` when it is open.
 */
export function WalletCard({
  wallet,
  lock,
  editRef,
  onAdd,
  onChange,
}: {
  wallet: PayoutWallet | null | undefined;
  lock: WalletLock | null;
  editRef?: React.Ref<HTMLButtonElement>;
  onAdd: () => void;
  onChange: () => void;
}) {
  const titleId = useId();
  const hintId = useId();
  const locked = lock !== null;
  const describedBy = locked ? hintId : undefined;
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
        </div>
        {wallet && (
          <Button
            ref={editRef}
            className={styles.cornerButton}
            variant="outline"
            disabled={locked}
            aria-describedby={describedBy}
            onClick={onChange}
          >
            {t('earn.joined.payouts.wallet.edit')}
          </Button>
        )}
      </div>
      {wallet !== undefined && (
        <div className={styles.setupRow}>
          {wallet ? (
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
          ) : (
            <Button
              className={styles.setupButton}
              variant="primary"
              disabled={locked}
              aria-describedby={describedBy}
              onClick={onAdd}
            >
              {t('earn.joined.payouts.wallet.add')}
            </Button>
          )}
          {lock && (
            <p
              id={hintId}
              className={styles.setupHint}
            >
              {t(WALLET_LOCK_HINT[lock])}
            </p>
          )}
        </div>
      )}
    </section>
  );
}

/**
 * The enrollment's name and read-only email, the first setup step, with VERIFY EMAIL until the email is verified.
 * Locked while none of the creator's places is in the program, since the service sends no code then: VERIFY EMAIL is
 * disabled and a line says why: beside it when the line fits there on one line, otherwise on its own row under it.
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
  const hintId = useId();
  const unverified = !!enrollment && !enrollment.email_verified_at;
  return (
    <section
      className={styles.card}
      aria-labelledby={titleId}
      data-locked={locked || undefined}
      data-testid="payouts-contact-card"
    >
      <div className={styles.contactTop}>
        <div className={styles.cardHeader}>
          <CardTitle
            id={titleId}
            titleRef={titleRef}
          >
            {t('earn.joined.payouts.contact.title')}
          </CardTitle>
        </div>
        {unverified && (
          <Button
            className={`${styles.setupButton} ${styles.verifyEmailButton}`}
            variant="primary"
            disabled={locked}
            aria-describedby={locked ? hintId : undefined}
            onClick={onVerify}
          >
            {t('earn.joined.payouts.contact.verify')}
          </Button>
        )}
        {locked && (
          <p
            id={hintId}
            className={styles.setupHint}
          >
            {t('earn.joined.payouts.contact.unavailable')}
          </p>
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
