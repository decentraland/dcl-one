import { useId, useRef, useState } from 'react';
import { cx } from '/@/lib/cx';

import Button from '@ui/atoms/Button';

import type { Events } from '/shared/types/analytics';

import { analytics } from '#preload';
import { useDispatch } from '#store';
import { actions } from '/@/modules/store/creatorFund';
import type { TranslationPath } from '/@/modules/store/translation/types';
import { getCurrentLocale, t } from '/@/modules/store/translation/utils';

import { EarnDialog } from '../EarnDialog';
import modal from '../EarnModal.module.css';
import { CloseIcon } from '../icons';

import { CodeInput, EMPTY_CODE } from './CodeInput';
import { ConfirmEmailStep, type ConfirmFocus } from './ConfirmEmailStep';
import { DAILY_LIMIT, SEND_SOON, useResendHeld, useSendCode } from './emailCode';
import parts from './parts.module.css';
import styles from './PayoutModals.module.css';

const EXPIRED: TranslationPath = 'earn.joined.payouts.verify.expired';

const ERRORS: Record<number, TranslationPath> = {
  400: 'earn.joined.payouts.verify.incorrect',
  410: EXPIRED,
};

const RESENDABLE = new Set([410, 429]);

const VERDICTS: Record<number, Events['Creator Fund Email Code Submitted']['result']> = {
  400: 'incorrect',
  410: 'expired',
  429: 'locked',
};

type Step = { name: 'confirm'; focus: ConfirmFocus } | { name: 'code' };

/**
 * The verify wizard's second step: takes the six-digit code sent to `email`.
 * @param props.onDone - called with the snackbar message once the email is verified.
 */
function CodeStep({
  titleId,
  email,
  onBack,
  onDone,
}: {
  titleId: string;
  email: string;
  onBack: () => void;
  onDone: (message: TranslationPath) => void;
}) {
  const dispatch = useDispatch();
  const errorId = useId();
  const sendCode = useSendCode();
  const [code, setCode] = useState<readonly string[]>(EMPTY_CODE);
  const [failure, setFailure] = useState<number | null>(null);
  const [verifying, setVerifying] = useState(false);
  const [sendError, setSendError] = useState<TranslationPath | null>(null);
  const resendHeld = useResendHeld();
  const codeArea = useRef<HTMLDivElement>(null);
  const resendRef = useRef<HTMLButtonElement>(null);
  const complete = code.every(Boolean);
  const error = failure === null ? null : (ERRORS[failure] ?? 'earn.errors.generic');
  const shownError = error ?? sendError;

  const resend = async () => {
    if (resendHeld) return;
    setFailure(null);
    setSendError(null);
    setCode(EMPTY_CODE);
    const failed = await sendCode();
    setSendError(failed === DAILY_LIMIT || failed === SEND_SOON ? null : failed);
  };

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!complete || verifying) return;
    setVerifying(true);
    const result = await dispatch(actions.confirmEmailVerification({ code: code.join('') }));
    setVerifying(false);
    if (actions.confirmEmailVerification.fulfilled.match(result)) {
      void analytics.track('Creator Fund Email Code Submitted', { result: 'verified' });
      onDone('earn.joined.payouts.snackbar.email_verified');
      return;
    }
    const status = result.payload?.status ?? 0;
    if (VERDICTS[status]) {
      void analytics.track('Creator Fund Email Code Submitted', { result: VERDICTS[status] });
    }
    setFailure(status);
    if (RESENDABLE.has(status)) resendRef.current?.focus();
    else codeArea.current?.querySelector('input')?.focus();
  };

  return (
    <form
      className={styles.modalForm}
      noValidate
      data-variant="verify"
      onSubmit={submit}
    >
      <h2
        className={modal.modalTitle}
        id={titleId}
      >
        {t('earn.joined.payouts.verify.title')}
      </h2>
      <p className={styles.body}>
        {getCurrentLocale().formatMessage(
          { id: 'earn.joined.payouts.verify.body_sent' },
          { email, b: (chunks: React.ReactNode[]) => <b key="email">{chunks}</b> },
        )}
        <br />
        {t('earn.joined.payouts.verify.body_instruction')}
      </p>
      <div
        className={styles.codeArea}
        ref={codeArea}
      >
        <CodeInput
          value={code}
          labelledBy={titleId}
          describedBy={shownError ? errorId : undefined}
          invalid={!!error}
          onChange={next => {
            setCode(next);
            setFailure(null);
          }}
        />
        {shownError && (
          <p
            className={styles.codeError}
            id={errorId}
            role="alert"
          >
            <CloseIcon />
            {t(shownError)}
          </p>
        )}
        {(failure === null || RESENDABLE.has(failure)) && (
          <button
            ref={resendRef}
            className={styles.resendButton}
            type="button"
            aria-disabled={resendHeld || undefined}
            onClick={() => void resend()}
          >
            {t('earn.joined.payouts.verify.resend')}
          </button>
        )}
      </div>
      <div className={cx(modal.actions, styles.payoutActions)}>
        <Button
          className={cx(parts.cta, parts.ctaSecondary)}
          variant="secondary"
          size="lg"
          onClick={onBack}
        >
          {t('earn.joined.payouts.verify.back')}
        </Button>
        <Button
          className={parts.cta}
          type="submit"
          size="lg"
          disabled={!complete || verifying || !!error}
          aria-busy={verifying || undefined}
        >
          {t('earn.joined.payouts.verify.submit')}
        </Button>
      </div>
    </form>
  );
}

/**
 * Verifies the enrollment's email in two steps: confirm or edit the email, then enter the code.
 * @param props.onDone - called with the snackbar message once the email is verified.
 */
export function VerifyEmailModal({
  email,
  onClose,
  onDone,
}: {
  email: string;
  onClose: () => void;
  onDone: (message: TranslationPath) => void;
}) {
  const titleId = useId();
  const [step, setStep] = useState<Step>({ name: 'confirm', focus: 'continue' });

  return (
    <EarnDialog
      labelledBy={titleId}
      onClose={onClose}
    >
      {step.name === 'confirm' ? (
        <ConfirmEmailStep
          titleId={titleId}
          email={email}
          focus={step.focus}
          onCancel={onClose}
          onSent={() => setStep({ name: 'code' })}
        />
      ) : (
        <CodeStep
          titleId={titleId}
          email={email}
          onBack={() => setStep({ name: 'confirm', focus: 'edit' })}
          onDone={onDone}
        />
      )}
    </EarnDialog>
  );
}
