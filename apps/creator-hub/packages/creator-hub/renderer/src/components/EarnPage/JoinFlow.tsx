import { useCallback, useEffect, useId, useRef, useState } from 'react';
import cx from 'classnames';
import { Button, IconButton, Modal as MuiModal } from 'decentraland-ui2';

import welcomeImage from '/assets/images/earn/welcome.webp';

import { analytics, misc } from '#preload';

import type { WalletReason } from '/shared/types/creator-fund';

import { useDispatch } from '#store';
import { actions } from '/@/modules/store/creatorFund';
import type { TranslationPath } from '/@/modules/store/translation/types';
import { getCurrentLocale, t } from '/@/modules/store/translation/utils';

import { Modal } from '../Modals';

import { ContactInfoForm, type ContactInfoFormProps, type ContactValues } from './ContactInfoForm';
import { CheckingSpinner } from './Spinner';
import { TermsModal } from './TermsModal';
import { CloseIcon } from './icons';
import modal from './EarnModal.module.css';
import styles from './JoinFlow.module.css';

export type JoinOutcome = 'cancelled' | 'joined' | 'conflict';

const SUPPORT_EMAIL = 'creatorfund@decentraland.org';

type Step =
  | { name: 'checking' }
  | { name: 'check_failed' }
  | { name: 'ineligible'; reasons: string[] }
  | { name: 'terms' }
  | { name: 'contact'; termsVersion: number }
  | { name: 'welcome' };

type Submission = Pick<ContactInfoFormProps, 'errors' | 'status'>;

const REASONS: Record<WalletReason, TranslationPath> = {
  internal: 'earn.not_eligible.reasons.internal',
  suspended: 'earn.not_eligible.reasons.suspended',
  excluded: 'earn.not_eligible.reasons.excluded',
};

const reasonLines = (reasons: string[]) => [
  ...new Set(
    (reasons.length ? reasons : ['unknown']).map(reason =>
      t(REASONS[reason as WalletReason] ?? 'earn.not_eligible.reasons.unknown'),
    ),
  ),
];

function StatusShell({
  labelledBy,
  describedBy,
  role = 'dialog',
  busy,
  onDismiss,
  children,
}: {
  labelledBy: string;
  describedBy?: string;
  role?: 'dialog' | 'alertdialog';
  busy?: boolean;
  onDismiss: () => void;
  children: React.ReactNode;
}) {
  return (
    <div
      className={modal.statusDialog}
      role={role}
      aria-modal="true"
      aria-labelledby={labelledBy}
      aria-describedby={describedBy}
      aria-busy={busy || undefined}
      onKeyDown={event => event.key === 'Escape' && onDismiss()}
    >
      <div
        className={modal.glowLayer}
        aria-hidden
      >
        <div className={modal.statusGlow} />
      </div>
      {children}
    </div>
  );
}

function Close({ onClick }: { onClick: () => void }) {
  return (
    <IconButton
      className={modal.closeButton}
      aria-label={t('earn.close')}
      onClick={onClick}
    >
      <CloseIcon />
    </IconButton>
  );
}

/**
 * Why the wallet can't join, one line per reason, with GOT IT.
 * @param props.onClose - called from GOT IT and Escape.
 */
export function IneligibleModal({ reasons, onClose }: { reasons: string[]; onClose: () => void }) {
  const titleId = useId();
  const reasonId = useId();
  return (
    <Modal
      className={modal.statusModal}
      open
    >
      <StatusShell
        role="alertdialog"
        labelledBy={titleId}
        describedBy={reasonId}
        onDismiss={onClose}
      >
        <Close onClick={onClose} />
        <div className={cx(modal.statusBody, styles.resultBody)}>
          <h2
            className={modal.modalTitle}
            id={titleId}
          >
            {t('earn.not_eligible.title')}
          </h2>
          <p
            className={modal.modalText}
            id={reasonId}
          >
            {reasonLines(reasons).map(line => (
              <span
                className={styles.reasonLine}
                key={line}
              >
                {line}
              </span>
            ))}
          </p>
          <Button
            className={modal.modalCta}
            variant="contained"
            color="primary"
            size="large"
            autoFocus
            onClick={onClose}
          >
            {t('earn.not_eligible.got_it')}
          </Button>
        </div>
      </StatusShell>
    </Modal>
  );
}

function CheckFailedModal({ onRetry, onClose }: { onRetry: () => void; onClose: () => void }) {
  const titleId = useId();
  const bodyId = useId();
  return (
    <Modal
      className={modal.statusModal}
      open
    >
      <StatusShell
        role="alertdialog"
        labelledBy={titleId}
        describedBy={bodyId}
        onDismiss={onClose}
      >
        <Close onClick={onClose} />
        <div className={cx(modal.statusBody, styles.resultBody)}>
          <h2
            className={modal.modalTitle}
            id={titleId}
          >
            {t('earn.check_failed.title')}
          </h2>
          <p
            className={modal.modalText}
            id={bodyId}
          >
            {getCurrentLocale().formatMessage(
              { id: 'earn.check_failed.body' },
              {
                email: (
                  <a
                    className={styles.supportEmail}
                    key="email"
                    href={`mailto:${SUPPORT_EMAIL}`}
                    onClick={event => {
                      event.preventDefault();
                      void analytics.track('Creator Fund Contact Support', {
                        source: 'check_failed',
                      });
                      void misc.openExternal(`mailto:${SUPPORT_EMAIL}`);
                    }}
                  >
                    {SUPPORT_EMAIL}
                  </a>
                ),
              },
            )}
          </p>
          <div className={cx(modal.actions, styles.ctaPair)}>
            <Button
              className={modal.modalCta}
              variant="contained"
              color="secondary"
              size="large"
              onClick={onClose}
            >
              {t('earn.close')}
            </Button>
            <Button
              className={modal.modalCta}
              variant="contained"
              color="primary"
              size="large"
              autoFocus
              onClick={onRetry}
            >
              {t('earn.errors.retry')}
            </Button>
          </div>
        </div>
      </StatusShell>
    </Modal>
  );
}

/**
 * The join flow: eligibility check, terms, contact info, welcome.
 * @param props.onExit - called once, with how the flow ended.
 */
export function JoinFlow({ onExit }: { onExit: (outcome: JoinOutcome) => void }) {
  const dispatch = useDispatch();
  const titleId = useId();
  const [step, setStep] = useState<Step>({ name: 'checking' });
  const [checkAttempt, setCheckAttempt] = useState(0);
  const [submission, setSubmission] = useState<Submission>({});
  const [draft, setDraft] = useState<ContactValues>();
  const onExitRef = useRef(onExit);
  onExitRef.current = onExit;
  const checkRef = useRef<{ abort: () => void } | null>(null);
  /*
   * Set once the flow has called `onExit`. Close and Cancel stay live while the enrollment is
   * saved, and an answer that lands after them must not call `onExit` a second time.
   */
  const exitedRef = useRef(false);
  const exit = useCallback((outcome: JoinOutcome) => {
    if (exitedRef.current) return;
    exitedRef.current = true;
    onExitRef.current(outcome);
  }, []);

  useEffect(() => {
    const check = dispatch(actions.fetchEligibility());
    checkRef.current = check;
    check
      .unwrap()
      .then(result => {
        if (result.eligible) {
          void analytics.track('Creator Fund Eligibility Checked', { result: 'eligible' });
          setStep({ name: 'terms' });
          return;
        }
        void analytics.track('Creator Fund Eligibility Checked', {
          result: 'ineligible',
          reasons: result.reasons,
        });
        setStep({ name: 'ineligible', reasons: result.reasons });
      })
      .catch(error => {
        if (error?.name === 'AbortError') return;
        void analytics.track('Creator Fund Eligibility Checked', { result: 'error' });
        setStep({ name: 'check_failed' });
      });
    return () => check.abort();
  }, [dispatch, checkAttempt]);

  const retryCheck = useCallback(() => {
    setStep({ name: 'checking' });
    setCheckAttempt(value => value + 1);
  }, []);

  const cancel = useCallback(() => {
    checkRef.current?.abort();
    exit('cancelled');
  }, [exit]);

  const finish = useCallback(() => exit('joined'), [exit]);

  const submit = useCallback(
    async (values: ContactValues) => {
      if (step.name !== 'contact') return;
      setDraft(values);
      setSubmission({ status: 'submitting' });
      const result = await dispatch(
        actions.saveEnrollment({ terms_version: step.termsVersion, ...values }),
      );
      const joined = actions.saveEnrollment.fulfilled.match(result);
      void analytics.track(
        'Creator Fund Contact Submitted',
        joined ? { ok: true } : { ok: false, status: result.payload?.status },
      );
      if (joined) void analytics.track('Creator Fund Joined', { terms_version: step.termsVersion });
      if (exitedRef.current) return;
      if (joined) {
        setStep({ name: 'welcome' });
        return;
      }
      const failure = result.payload;
      if (failure?.status === 403) {
        setStep({ name: 'ineligible', reasons: failure.body?.reasons ?? [] });
        return;
      }
      if (failure?.status === 409) {
        exit('conflict');
        return;
      }
      if (failure?.status === 400) {
        const latest = await dispatch(actions.fetchTerms());
        if (exitedRef.current) return;
        if (
          actions.fetchTerms.fulfilled.match(latest) &&
          latest.payload.version !== step.termsVersion
        ) {
          setSubmission({});
          setStep({ name: 'terms' });
          return;
        }
      }
      setSubmission({ errors: [t('earn.errors.generic')] });
    },
    [dispatch, step, exit],
  );

  switch (step.name) {
    case 'checking':
      return (
        <Modal
          className={modal.statusModal}
          open
        >
          <StatusShell
            labelledBy={titleId}
            busy
            onDismiss={cancel}
          >
            <Close onClick={cancel} />
            <div className={cx(modal.statusBody, styles.checkingBody)}>
              <CheckingSpinner label={t('earn.checking.progress_label')} />
              <h2
                className={modal.modalTitle}
                id={titleId}
              >
                {t('earn.checking.title')}
              </h2>
              <p className={modal.modalText}>
                {t('earn.checking.body_1')}
                <br />
                {t('earn.checking.body_2')}
              </p>
            </div>
          </StatusShell>
        </Modal>
      );
    case 'check_failed':
      return (
        <CheckFailedModal
          onRetry={retryCheck}
          onClose={cancel}
        />
      );
    case 'ineligible':
      return (
        <IneligibleModal
          reasons={step.reasons}
          onClose={cancel}
        />
      );
    case 'terms':
      return (
        <TermsModal
          variant="accept"
          onAccept={termsVersion => setStep({ name: 'contact', termsVersion })}
          onClose={cancel}
        />
      );
    case 'contact':
      return (
        <Modal
          className={modal.statusModal}
          open
        >
          <StatusShell
            labelledBy={titleId}
            onDismiss={cancel}
          >
            <Close onClick={cancel} />
            <ContactInfoForm
              titleId={titleId}
              initialValues={draft}
              onSubmit={submit}
              onCancel={cancel}
              {...submission}
            />
          </StatusShell>
        </Modal>
      );
    case 'welcome':
      return (
        <MuiModal
          className={modal.welcomeRoot}
          open
        >
          <div className={modal.welcomeFrame}>
            <img
              className={styles.welcomeImage}
              src={welcomeImage}
              alt=""
              aria-hidden
            />
            <div className={modal.welcomePaper}>
              <StatusShell
                labelledBy={titleId}
                onDismiss={finish}
              >
                <Close onClick={finish} />
                <div className={cx(modal.statusBody, styles.resultBody)}>
                  <h2
                    className={modal.modalTitle}
                    id={titleId}
                  >
                    {t('earn.welcome.title')}
                  </h2>
                  <p className={modal.modalText}>{t('earn.welcome.body')}</p>
                  <Button
                    className={modal.modalCta}
                    variant="contained"
                    color="primary"
                    size="large"
                    autoFocus
                    onClick={finish}
                  >
                    {t('earn.welcome.lets_go')}
                  </Button>
                </div>
              </StatusShell>
            </div>
          </div>
        </MuiModal>
      );
  }
}
