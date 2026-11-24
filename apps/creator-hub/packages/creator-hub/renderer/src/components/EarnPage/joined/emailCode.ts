import { useCallback, useEffect, useState } from 'react';

import { useDispatch, useSelector } from '#store';
import { actions } from '/@/modules/store/creatorFund';
import type { TranslationPath } from '/@/modules/store/translation/types';

export const SEND_FAILED: TranslationPath = 'earn.errors.generic';

/** The service sends no code until one of the creator's places is in the program. */
export const NO_ACTIVE_PLACE: TranslationPath = 'earn.joined.payouts.contact.unavailable';

export const DAILY_LIMIT: TranslationPath = 'earn.joined.payouts.verify.daily_limit';

/** The last code's cooldown still runs, so the service sends no new one yet. */
export const SEND_SOON: TranslationPath = 'earn.joined.payouts.verify.send_soon';

const DAILY_LIMIT_CUTOFF_MS = 10 * 60_000;

/** Whether two addresses are one to the service, which ignores case. */
export const sameEmail = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

/**
 * Sends a verification code to the enrollment's email.
 * @returns `null` once a code is sent, `SEND_SOON` while the last one cools down, else the line saying why not.
 */
export function useSendCode() {
  const dispatch = useDispatch();
  return useCallback(async (): Promise<TranslationPath | null> => {
    const result = await dispatch(actions.sendEmailVerification());
    if (!actions.sendEmailVerification.rejected.match(result)) return null;
    const failure = result.payload;
    if (failure?.status === 429) {
      const reopensAt = Date.parse(failure.body?.resend_available_at ?? '');
      if (Number.isNaN(reopensAt)) return SEND_FAILED;
      return reopensAt - Date.now() > DAILY_LIMIT_CUTOFF_MS ? DAILY_LIMIT : SEND_SOON;
    }
    if (failure?.body?.error === 'no_active_place') {
      void dispatch(actions.fetchHasActivePlace());
      return NO_ACTIVE_PLACE;
    }
    return SEND_FAILED;
  }, [dispatch]);
}

/** The address the last code was sent to, and whether its cooldown still runs. */
export function useLastCode() {
  const verification = useSelector(state => state.creatorFund.emailVerification);
  return {
    email: verification.data?.email,
    cooling: () =>
      verification.status === 'succeeded' &&
      !!verification.data &&
      Date.parse(verification.data.resend_available_at) > Date.now(),
  };
}

/** When a resend is allowed again, by the last send's answer; `null` while nothing holds it. */
function useResendAt() {
  const verification = useSelector(state => state.creatorFund.emailVerification);
  if (verification.status === 'loading') return Infinity;
  const held =
    verification.failure?.body?.resend_available_at ?? verification.data?.resend_available_at;
  return held ? Date.parse(held) : null;
}

/** Whether a resend is held by the last send's answer; wakes when the hold ends, with no countdown. */
export function useResendHeld() {
  const resendAt = useResendAt();
  const [wakeCount, wake] = useState(0);
  const held = resendAt !== null && resendAt > Date.now();
  // A timer can fire before the clock reaches the deadline; each wake rearms it if needed.
  useEffect(() => {
    if (!held || resendAt === null || !Number.isFinite(resendAt)) return;
    // Longer delays overflow the browser's signed 32-bit timer and wake immediately.
    const delay = Math.min(resendAt - Date.now(), 2_147_483_647);
    const timer = setTimeout(() => wake(count => count + 1), delay);
    return () => clearTimeout(timer);
  }, [held, resendAt, wakeCount]);
  return held;
}
