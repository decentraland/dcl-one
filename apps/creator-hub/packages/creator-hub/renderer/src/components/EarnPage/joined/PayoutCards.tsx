import { useId, useRef, useState } from 'react';

import Button from '@ui/atoms/Button';
import Tooltip from '@ui/atoms/Tooltip';

import type { Enrollment, PayoutWallet } from '/shared/types/creator-fund';

import { misc } from '#preload';
import { t } from '/@/modules/store/translation/utils';

import { CopyToClipboard } from '../../CopyToClipboard';

import modal from '../EarnModal.module.css';

import { CopyIcon, InfoIcon, WarningIcon } from '../icons';

import { EARN_TOOLTIP, ExplainedChip } from './parts';
import { shortAddress } from './wallet';
import parts from './parts.module.css';
import styles from './PayoutsTab.module.css';

const GET_A_WALLET_URL = 'https://docs.decentraland.org/blockchain-integration/get-a-wallet';

/**
 * The payout-wallet info button and its interactive tooltip. The tooltip is drawn at the end of the
 * page, so Tab is moved by hand: from the button into "Learn More", and from "Learn More" on from
 * the button. It stays open while focus is in either.
 */
function WalletInfo() {
  const [open, setOpen] = useState(false);
  const [button, setButton] = useState<HTMLButtonElement | null>(null);
  const linkRef = useRef<HTMLAnchorElement>(null);
  const returning = useRef(false);
  const onOpenChange = (next: boolean) => {
    if (returning.current) return;
    if (!next && linkRef.current?.contains(document.activeElement)) {
      returning.current = true;
      button?.focus();
      returning.current = false;
    }
    setOpen(next);
  };
  return (
    <span className={styles.infoGroup}>
      <Tooltip
        {...EARN_TOOLTIP}
        open={open}
        onOpenChange={onOpenChange}
        interactive
        anchor={button}
        content={
          <>
            {t('earn.joined.payouts.wallet.info')}{' '}
            <a
              ref={linkRef}
              className={styles.learnMore}
              href={GET_A_WALLET_URL}
              onClick={event => {
                event.preventDefault();
                void misc.openExternal(GET_A_WALLET_URL);
              }}
              onKeyDown={event => {
                if (event.key !== 'Tab') return;
                if (event.shiftKey) event.preventDefault();
                button?.focus();
              }}
            >
              {t('earn.joined.payouts.wallet.learn_more')}
              <span aria-hidden> →</span>
            </a>
          </>
        }
      >
        <button
          ref={setButton}
          className={parts.infoButton}
          type="button"
          aria-label={t('earn.joined.payouts.wallet.info_label')}
          onKeyDown={event => {
            if (event.key !== 'Tab' || event.shiftKey || !open || !linkRef.current) return;
            event.preventDefault();
            linkRef.current.focus();
          }}
        >
          <InfoIcon />
        </button>
      </Tooltip>
    </span>
  );
}

/**
 * The setup step a card is, as a circled number. Screen readers hear it as the start of the title.
 */
function CardTitle({
  step,
  id,
  titleRef,
  children,
}: {
  step: 1 | 2;
  id: string;
  titleRef?: React.Ref<HTMLHeadingElement>;
  children: React.ReactNode;
}) {
  return (
    <>
      <span
        className={styles.step}
        aria-hidden
      >
        {step}
      </span>
      <h2
        className={styles.cardTitle}
        id={id}
        ref={titleRef}
        tabIndex={titleRef ? -1 : undefined}
      >
        <span className={modal.visuallyHidden}>{t('earn.joined.payouts.step', { step })} </span>
        {children}
      </h2>
    </>
  );
}

/**
 * The payout wallet, step 2: its short address as a chip that copies it, and EDIT; ADD WALLET when
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
          <CardTitle
            step={2}
            id={titleId}
          >
            {t('earn.joined.payouts.wallet.title')}
          </CardTitle>
          <WalletInfo />
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
 * The enrollment's name and read-only email, step 1, with VERIFY EMAIL until the email is verified.
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
            step={1}
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
