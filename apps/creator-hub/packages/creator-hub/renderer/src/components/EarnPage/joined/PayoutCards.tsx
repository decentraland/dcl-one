import { useId, useRef, useState } from 'react';

import Button from '@ui/atoms/Button';
import Tooltip from '@ui/atoms/Tooltip';

import type { Enrollment, PayoutWallet } from '/shared/types/creator-fund';

import { misc } from '#preload';
import { t } from '/@/modules/store/translation/utils';

import { CopyToClipboard } from '../../CopyToClipboard';

import { CopyIcon, InfoIcon, WarningIcon } from '../icons';

import { EARN_TOOLTIP } from './parts';
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
        <WalletInfo />
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
