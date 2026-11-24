import { useEffect, useRef, useState } from 'react';
import { cx } from '/@/lib/cx';

import { Toast } from '@ui/atoms/Toast';

import { analytics } from '#preload';
import { useDispatch, useSelector } from '#store';
import { actions, selectors } from '/@/modules/store/creatorFund';
import type { TranslationPath } from '/@/modules/store/translation/types';
import { t } from '/@/modules/store/translation/utils';

import modalStyles from '../EarnModal.module.css';
import { SnackbarCheckIcon } from '../icons';

import { ListState } from './parts';
import { ContactCard, WalletCard, type WalletLock } from './PayoutCards';
import { PayoutsEmpty } from './PayoutsEmpty';
import { PayoutsTable } from './PayoutsTable';
import { VerifyEmailModal } from './VerifyEmailModal';
import { WalletModal } from './WalletModal';
import styles from './PayoutsTab.module.css';

const SNACKBAR_MS = 5_000;

type OpenModal = 'add_wallet' | 'change_wallet' | 'verify_email';

type FocusTarget = 'edit' | 'contact';

/** Opens Add payout wallet once the tab has loaded; `id` tells two requests apart. */
export type PayoutSetupRequest = { step: 'add_wallet'; id: number };

const FOCUS_AFTER: Record<OpenModal, FocusTarget> = {
  add_wallet: 'edit',
  change_wallet: 'edit',
  verify_email: 'contact',
};

/**
 * The Creator Fund payouts: two setup steps, then the payout per cycle. Step 1 verifies the email;
 * step 2, the payout wallet, needs it verified, since every change to it is emailed there. Both
 * need one of the creator's places in the program; a failed read of the places locks neither.
 */
export function PayoutsTab({
  request = null,
  onRequestHandled,
}: {
  request?: PayoutSetupRequest | null;
  onRequestHandled?: () => void;
}) {
  const dispatch = useDispatch();
  const cycles = useSelector(state => state.creatorFund.cycles);
  const enrollment = useSelector(state => state.creatorFund.enrollment);
  const payouts = useSelector(state => state.creatorFund.payouts);
  const wallet = useSelector(state => state.creatorFund.payoutWallet);
  const hasActivePlace = useSelector(state => state.creatorFund.hasActivePlace);
  const noActivePlace = useSelector(selectors.isPayoutSetupLocked);
  const setupStep = useSelector(selectors.getPayoutSetupStep);
  const [loaded, setLoaded] = useState(() =>
    [payouts, wallet, enrollment, hasActivePlace].every(
      resource => resource.status === 'succeeded',
    ),
  );
  const [modal, setModal] = useState<OpenModal | null>(null);
  const [notice, setNotice] = useState<{ message: TranslationPath } | null>(null);
  const [focusTarget, setFocusTarget] = useState<FocusTarget | null>(null);
  const editRef = useRef<HTMLButtonElement>(null);
  const contactTitleRef = useRef<HTMLHeadingElement>(null);
  const mounted = useRef(false);

  const loadedWhenSettled = (requests: Promise<unknown>[]) =>
    void Promise.all(requests).then(() => mounted.current && setLoaded(true));

  useEffect(() => {
    mounted.current = true;
    if (cycles.status === 'idle') void dispatch(actions.fetchCycles());
    const requests: Promise<unknown>[] = [
      dispatch(actions.fetchPayouts()),
      dispatch(actions.fetchPayoutWallet()),
      dispatch(actions.fetchHasActivePlace()),
    ];
    if (enrollment.status === 'idle') requests.push(dispatch(actions.fetchEnrollment()));
    loadedWhenSettled(requests);
    return () => {
      mounted.current = false;
    };
  }, []);

  useEffect(() => {
    if (modal || !focusTarget) return;
    (focusTarget === 'edit' ? editRef : contactTitleRef).current?.focus();
    setFocusTarget(null);
  }, [modal, focusTarget]);

  const failed = [payouts, wallet, enrollment].some(resource => resource.status === 'failed');

  useEffect(() => {
    if (!request || !loaded) return;
    onRequestHandled?.();
    if (failed || setupStep !== request.step) return;
    setModal(request.step);
  }, [loaded, request?.id]);

  const close = () => setModal(null);
  const done = (message: TranslationPath) => {
    if (modal) setFocusTarget(FOCUS_AFTER[modal]);
    setModal(null);
    setNotice({ message });
  };

  const retry = () => {
    setLoaded(false);
    const requests: Promise<unknown>[] = [];
    if (payouts.status === 'failed') requests.push(dispatch(actions.fetchPayouts()));
    if (wallet.status === 'failed') requests.push(dispatch(actions.fetchPayoutWallet()));
    if (enrollment.status === 'failed') requests.push(dispatch(actions.fetchEnrollment()));
    if (hasActivePlace.status === 'failed') requests.push(dispatch(actions.fetchHasActivePlace()));
    loadedWhenSettled(requests);
  };

  const empty = loaded && !failed && payouts.data?.payouts.length === 0;
  const emailVerified = !!enrollment.data?.email_verified_at;
  let walletLock: WalletLock | null = null;
  if (!emailVerified) walletLock = 'unverified_email';
  else if (noActivePlace) walletLock = 'no_active_place';

  let body: React.ReactNode;
  if (!loaded) {
    body = <ListState kind="loading" />;
  } else if (failed) {
    body = (
      <ListState
        kind="error"
        message={t('earn.joined.payouts.error')}
        onRetry={retry}
      />
    );
  } else {
    let list: React.ReactNode = <ListState kind="loading" />;
    if (empty) list = <PayoutsEmpty />;
    else if (payouts.data) list = <PayoutsTable payouts={payouts.data.payouts} />;
    body = (
      <>
        <div className={styles.cardsRow}>
          <ContactCard
            enrollment={enrollment.data}
            locked={!emailVerified && noActivePlace}
            titleRef={contactTitleRef}
            onVerify={() => {
              void analytics.track('Creator Fund Email Verification Started', undefined);
              setModal('verify_email');
            }}
          />
          <WalletCard
            wallet={wallet.status === 'succeeded' ? wallet.data : undefined}
            lock={walletLock}
            editRef={editRef}
            onAdd={() => setModal('add_wallet')}
            onChange={() => setModal('change_wallet')}
          />
        </div>
        {list}
      </>
    );
  }

  return (
    <div className={cx(styles.panel, empty && styles.emptyPanel)}>
      {body}
      {(modal === 'add_wallet' || modal === 'change_wallet') && (
        <WalletModal
          variant={modal === 'add_wallet' ? 'add' : 'change'}
          onClose={close}
          onDone={done}
        />
      )}
      {modal === 'verify_email' && enrollment.data && (
        <VerifyEmailModal
          email={enrollment.data.email}
          onClose={close}
          onDone={done}
        />
      )}
      <span
        className={modalStyles.visuallyHidden}
        role="status"
      >
        {notice && t(notice.message)}
      </span>
      <Toast
        open={!!notice}
        restartKey={notice}
        duration={SNACKBAR_MS}
        onClose={() => setNotice(null)}
        placement="bottom-center"
        role="none"
        className={styles.toast}
      >
        <div
          className={styles.notice}
          aria-hidden
        >
          <SnackbarCheckIcon />
          {notice && t(notice.message)}
        </div>
      </Toast>
    </div>
  );
}
