import { useEffect, useId, useRef, useState } from 'react';
import { cx } from '/@/lib/cx';

import Button from '@ui/atoms/Button';
import TextField from '@ui/atoms/TextField';

import { useDispatch, useSelector } from '#store';
import { actions } from '/@/modules/store/creatorFund';
import type { TranslationPath } from '/@/modules/store/translation/types';
import { t } from '/@/modules/store/translation/utils';
import { EMAIL_MAX_LENGTH, isEmail } from '/@/modules/utils';

import modal from '../EarnModal.module.css';
import { EditIcon } from '../icons';

import {
  DAILY_LIMIT,
  NO_ACTIVE_PLACE,
  SEND_FAILED,
  SEND_SOON,
  sameEmail,
  useLastCode,
  useResendHeld,
  useSendCode,
} from './emailCode';
import parts from './parts.module.css';
import styles from './PayoutModals.module.css';

type FieldMode = 'read-only' | 'editing';

export type ConfirmFocus = 'continue' | 'edit';

const BLOCKING: readonly TranslationPath[] = [NO_ACTIVE_PLACE, DAILY_LIMIT];

const INVALID_EMAIL: TranslationPath = 'earn.contact.invalid_email';

/** Saves an edited email over the enrollment, keeping its names, under the current terms. */
function useSaveEmail() {
  const dispatch = useDispatch();
  const enrollment = useSelector(state => state.creatorFund.enrollment.data);
  const termsVersion = useSelector(state => state.creatorFund.terms.data?.version);
  return async (email: string) => {
    const terms_version = termsVersion ?? enrollment?.terms_version;
    if (!enrollment || terms_version == null) return false;
    const { first_name, last_name } = enrollment;
    const result = await dispatch(
      actions.saveEnrollment({ first_name, last_name, email, terms_version }),
    );
    return actions.saveEnrollment.fulfilled.match(result);
  };
}

/**
 * The verify wizard's first step: confirms the email, or edits and saves it, then sends a code.
 * A new address waits on this step while the last code's cooldown runs.
 * @param props.focus - what takes focus on mount.
 * @param props.onSent - called once a code is on its way to the email.
 */
export function ConfirmEmailStep({
  titleId,
  email,
  focus,
  onCancel,
  onSent,
}: {
  titleId: string;
  email: string;
  focus: ConfirmFocus;
  onCancel: () => void;
  onSent: () => void;
}) {
  const inputId = useId();
  const errorId = useId();
  const sendCode = useSendCode();
  const saveEmail = useSaveEmail();
  const lastCode = useLastCode();
  const resendHeld = useResendHeld();
  const [draft, setDraft] = useState(email);
  const [mode, setMode] = useState<FieldMode>('read-only');
  const [blurred, setBlurred] = useState(false);
  const [error, setError] = useState<TranslationPath | null>(null);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const editRef = useRef<HTMLButtonElement>(null);
  const continueRef = useRef<HTMLButtonElement>(null);

  useEffect(() => (focus === 'edit' ? editRef : continueRef).current?.focus(), []);

  const trimmed = draft.trim();
  const valid = isEmail(trimmed);
  const waiting = error === SEND_SOON && resendHeld && sameEmail(trimmed, email);
  const notice = error === SEND_SOON && !waiting ? null : error;
  const line = blurred && trimmed && !valid ? INVALID_EMAIL : notice;
  const blocked = !valid || busy || waiting || (notice !== null && BLOCKING.includes(notice));

  const edit = () => {
    setMode('editing');
    inputRef.current?.focus();
    inputRef.current?.select();
  };

  const fail = (reason: TranslationPath) => {
    setBusy(false);
    setError(reason);
    inputRef.current?.focus();
  };

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (blocked) return;
    const changed = !sameEmail(trimmed, email);
    const newAddress = lastCode.email
      ? !sameEmail(lastCode.email, trimmed)
      : changed || error === SEND_SOON;
    setError(null);
    setBusy(true);
    if (changed) {
      if (!(await saveEmail(trimmed))) return fail(SEND_FAILED);
      setDraft(trimmed);
      setMode('read-only');
    }
    if (lastCode.cooling()) {
      if (newAddress) return fail(SEND_SOON);
      return onSent();
    }
    const failed = await sendCode();
    if (failed && (failed !== SEND_SOON || newAddress)) return fail(failed);
    onSent();
  };

  return (
    <form
      className={styles.modalForm}
      noValidate
      data-variant="confirm"
      onSubmit={submit}
    >
      <h2
        className={modal.modalTitle}
        id={titleId}
      >
        {t('earn.joined.payouts.verify.confirm_title')}
      </h2>
      <p className={styles.body}>{t('earn.joined.payouts.verify.confirm_body')}</p>
      <div className={cx(styles.field, styles.emailField)}>
        <label
          className={styles.fieldLabel}
          htmlFor={inputId}
        >
          {t('earn.joined.payouts.contact.email')}
        </label>
        <div className={styles.emailControl}>
          <TextField
            className={cx(styles.fieldInput, styles.emailInput)}
            id={inputId}
            name="email"
            inputRef={inputRef}
            type="email"
            value={draft}
            readOnly={mode === 'read-only'}
            autoComplete="email"
            spellCheck={false}
            maxLength={EMAIL_MAX_LENGTH}
            error={line === INVALID_EMAIL}
            aria-describedby={line && line !== SEND_SOON ? errorId : undefined}
            onChange={event => setDraft(event.target.value)}
            onBlur={() => setBlurred(true)}
            endAdornment={
              mode === 'read-only' && (
                <button
                  ref={editRef}
                  className={styles.editButton}
                  type="button"
                  aria-label={t('earn.joined.payouts.verify.edit_email')}
                  onClick={edit}
                >
                  <EditIcon />
                </button>
              )
            }
          />
          <div className={styles.helperRow}>
            {line && (
              <p
                className={line === SEND_SOON ? styles.waitingLine : styles.failureLine}
                id={errorId}
                role={line === SEND_SOON ? 'status' : 'alert'}
              >
                {t(line)}
              </p>
            )}
          </div>
        </div>
      </div>
      <div className={cx(modal.actions, styles.payoutActions)}>
        <Button
          className={cx(parts.cta, parts.ctaSecondary)}
          variant="secondary"
          size="lg"
          onClick={onCancel}
        >
          {t('earn.joined.payouts.cancel')}
        </Button>
        <Button
          ref={continueRef}
          className={parts.cta}
          type="submit"
          size="lg"
          disabled={blocked}
          aria-busy={busy || undefined}
          aria-describedby={line === SEND_SOON ? errorId : undefined}
        >
          {t('earn.joined.payouts.verify.continue')}
        </Button>
      </div>
    </form>
  );
}
