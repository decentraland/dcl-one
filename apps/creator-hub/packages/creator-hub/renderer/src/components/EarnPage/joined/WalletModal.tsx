import { useEffect, useId, useRef, useState } from 'react';
import { cx } from '/@/lib/cx';

import Button from '@ui/atoms/Button';
import TextField from '@ui/atoms/TextField';

import { analytics } from '#preload';
import { useDispatch } from '#store';
import { actions } from '/@/modules/store/creatorFund';
import type { TranslationPath } from '/@/modules/store/translation/types';
import { t } from '/@/modules/store/translation/utils';
import { CREATOR_FUND_RULES_URL } from '/@/modules/utils';

import { EarnDialog } from '../EarnDialog';
import modal from '../EarnModal.module.css';
import contact from '../ContactInfoForm.module.css';
import { WarningIcon } from '../icons';

import { InfoTip } from './InfoTip';
import { isWalletAddress } from './wallet';
import parts from './parts.module.css';
import styles from './PayoutModals.module.css';

export type WalletModalVariant = 'add' | 'change';

/** The Program Rules section on payouts: currency, timing, the payout wallet and how to set it. */
const PAYOUT_RULES_URL = `${CREATOR_FUND_RULES_URL}#payout-process`;

const COPY: Record<
  WalletModalVariant,
  { title: TranslationPath; body: TranslationPath; submit: TranslationPath; done: TranslationPath }
> = {
  add: {
    title: 'earn.joined.payouts.add.title',
    body: 'earn.joined.payouts.add.body',
    submit: 'earn.joined.payouts.add.submit',
    done: 'earn.joined.payouts.snackbar.wallet_added',
  },
  change: {
    title: 'earn.joined.payouts.change.title',
    body: 'earn.joined.payouts.change.body',
    submit: 'earn.joined.payouts.change.submit',
    done: 'earn.joined.payouts.snackbar.wallet_updated',
  },
};

type Submission = 'idle' | 'saving' | 'refused' | 'failed' | 'unnotified' | 'no_active_place';

const FAILURE_LINE: Partial<Record<Submission, TranslationPath>> = {
  failed: 'earn.errors.generic',
  unnotified: 'earn.joined.payouts.wallet.notify_failed',
  no_active_place: 'earn.joined.payouts.wallet.no_active_place',
};

/** The wallet address field, focused once it mounts inside the dialog; its error line is live. */
function WalletField({
  value,
  invalid,
  inputRef,
  onChange,
  onBlur,
}: {
  value: string;
  invalid: boolean;
  inputRef: React.RefObject<HTMLInputElement>;
  onChange: (value: string) => void;
  onBlur: () => void;
}) {
  const id = useId();
  const errorId = `${id}-error`;

  useEffect(() => inputRef.current?.focus(), []);

  return (
    <div className={styles.field}>
      <label
        className={styles.fieldLabel}
        htmlFor={id}
      >
        {t('earn.joined.payouts.wallet_field.label')}
      </label>
      <TextField
        className={styles.fieldInput}
        id={id}
        inputRef={inputRef}
        value={value}
        placeholder={t('earn.joined.payouts.wallet_field.placeholder')}
        autoComplete="off"
        spellCheck={false}
        error={invalid}
        aria-describedby={invalid ? errorId : undefined}
        data-filled={value ? 'true' : undefined}
        onChange={event => onChange(event.target.value.trim())}
        onBlur={onBlur}
      />
      <span
        className={cx(contact.fieldError, styles.fieldError)}
        id={errorId}
        aria-live="polite"
      >
        {invalid && (
          <>
            <WarningIcon />
            {t('earn.joined.payouts.wallet_field.invalid')}
          </>
        )}
      </span>
    </div>
  );
}

/**
 * Adds or changes the payout wallet.
 * @param props.onDone - called with the snackbar message once the wallet is saved.
 */
export function WalletModal({
  variant,
  onClose,
  onDone,
}: {
  variant: WalletModalVariant;
  onClose: () => void;
  onDone: (message: TranslationPath) => void;
}) {
  const dispatch = useDispatch();
  const titleId = useId();
  const copy = COPY[variant];
  const [address, setAddress] = useState('');
  const [blurred, setBlurred] = useState(false);
  const [submission, setSubmission] = useState<Submission>('idle');
  const fieldRef = useRef<HTMLInputElement>(null);
  const valid = isWalletAddress(address);
  const invalid = (blurred && !!address && !valid) || submission === 'refused';

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!valid || submission === 'saving') return;
    setSubmission('saving');
    const result = await dispatch(actions.savePayoutWallet({ address }));
    void analytics.track('Creator Fund Payout Wallet Saved', {
      action: variant === 'add' ? 'add' : 'edit',
      ok: actions.savePayoutWallet.fulfilled.match(result),
    });
    if (actions.savePayoutWallet.fulfilled.match(result)) {
      onDone(copy.done);
      return;
    }
    if (result.payload?.status === 400) {
      setSubmission('refused');
      fieldRef.current?.focus();
      return;
    }
    const error = result.payload?.body?.error;
    if (error === 'no_active_place') {
      setSubmission('no_active_place');
      void dispatch(actions.fetchHasActivePlace());
      return;
    }
    setSubmission(error === 'email_unavailable' ? 'unnotified' : 'failed');
  };

  const failureLine = FAILURE_LINE[submission];

  return (
    <EarnDialog
      labelledBy={titleId}
      onClose={onClose}
    >
      <form
        className={styles.modalForm}
        noValidate
        onSubmit={submit}
      >
        <div className={styles.modalIntro}>
          <div className={styles.modalTitleRow}>
            <h2
              className={modal.modalTitle}
              id={titleId}
            >
              {t(copy.title)}
            </h2>
            <InfoTip
              label="earn.joined.payouts.wallet.info_label"
              text="earn.joined.payouts.wallet.payout_info"
              href={PAYOUT_RULES_URL}
            />
          </div>
          <p className={styles.body}>{t(copy.body)}</p>
        </div>
        <WalletField
          value={address}
          invalid={invalid}
          inputRef={fieldRef}
          onChange={value => {
            setAddress(value);
            if (submission === 'refused') setSubmission('idle');
          }}
          onBlur={() => setBlurred(true)}
        />
        <div
          className={styles.callout}
          role="note"
        >
          <WarningIcon />
          {t('earn.joined.payouts.erc20')}
        </div>
        {failureLine && (
          <p
            className={styles.failureLine}
            role="alert"
          >
            {t(failureLine)}
          </p>
        )}
        <div className={cx(modal.actions, styles.payoutActions)}>
          <Button
            className={cx(parts.cta, parts.ctaSecondary)}
            variant="secondary"
            size="lg"
            onClick={onClose}
          >
            {t('earn.joined.payouts.cancel')}
          </Button>
          <Button
            className={parts.cta}
            type="submit"
            size="lg"
            disabled={!valid || submission === 'saving'}
            aria-busy={submission === 'saving' || undefined}
          >
            {t(copy.submit)}
          </Button>
        </div>
      </form>
    </EarnDialog>
  );
}
