import { hasSharingConsent, type AiTurnRecord } from '/shared/types/ai-sharing';
import type { MetricsResponse } from '/shared/types/metrics';

import { aiSharing } from '#preload';
import * as api from '/@/lib/aiSharingApi';
import { createAsyncThunk } from '/@/modules/store/thunk';
import type { AppDispatch, AppState } from '/@/modules/store';

import { checkVersionMismatch, syncConsent } from './consent';
import { refreshShownQueue } from './lists';
import {
  currentWallet,
  isSharingFeatureOn,
  selectBackoff,
  selectPending,
  selectSharingConsent,
} from './selectors';
import { updatePending, withoutRatings } from './settingsWrites';
import { sliceActions as actions } from './slice';

// Wallets with a flush running: the timer and a consent change can both start one, and two at
// once would upload a batch twice. Per wallet, so a switch doesn't drop the new wallet's first.
const flushing = new Set<string>();

/**
 * Settles consent with the service and uploads what main queued for the signed-in wallet, a
 * batch at a time. Records for another wallet stay queued until it signs in again (main drops
 * them after 7 days).
 */
export const flush = createAsyncThunk(
  'aiSharing/flush',
  async (_: void, { getState, dispatch }) => {
    const wallet = currentWallet();
    if (wallet === null || flushing.has(wallet)) return;
    flushing.add(wallet);
    try {
      await flushQueue(getState, dispatch, wallet);
    } finally {
      flushing.delete(wallet);
    }
  },
);

/** Records per upload, kept under the service's body cap whatever the records weigh. */
function nextBatch(records: AiTurnRecord[], maxRecords: number): AiTurnRecord[] {
  const batch: AiTurnRecord[] = [];
  let bytes = UPLOAD_ENVELOPE_BYTES;
  for (const record of records.slice(0, maxRecords)) {
    const size = new TextEncoder().encode(JSON.stringify(record)).length + 1;
    if (batch.length > 0 && bytes + size > api.MAX_UPLOAD_BYTES) break;
    batch.push(record);
    bytes += size;
  }
  return batch;
}

const UPLOAD_ENVELOPE_BYTES = '{"records":[]}'.length;

// Rejections that say nothing about the record itself: keep it and try again later.
const RETRYABLE_REJECTIONS = new Set(['address_mismatch']);

/** Whether uploading for `wallet` may continue: still signed in as it, still sharing. */
function mayUpload(state: AppState, wallet: string): boolean {
  return (
    currentWallet() === wallet &&
    isSharingFeatureOn(state) &&
    hasSharingConsent(selectSharingConsent(state, wallet))
  );
}

function backOff(dispatch: AppDispatch, wallet: string, response?: MetricsResponse<unknown>): void {
  const retryAfterSeconds =
    response !== undefined && !response.ok ? response.retryAfterSeconds : undefined;
  dispatch(actions.uploadFailed({ wallet, retryAfterSeconds }));
}

async function flushQueue(
  getState: () => AppState,
  dispatch: AppDispatch,
  wallet: string,
): Promise<void> {
  if (!api.isConfigured()) return;
  const consent = selectSharingConsent(getState(), wallet);
  if (consent === undefined) return;
  if (Date.now() < selectBackoff(getState(), wallet).nextUploadAt) return;
  // Like withdrawals, erasures go out whatever the consent and the flag say.
  const erasureFailure = await deliverErasures(getState, dispatch, wallet);
  if (erasureFailure !== null) {
    backOff(dispatch, wallet, erasureFailure);
    return;
  }
  // A "yes" goes out only while sharing is offered (the flag on, the consent text matching): a
  // grant made offline must not reach the service after the feature was turned off. A "no"
  // always goes out.
  if (consent.consent === 'granted' && !isSharingFeatureOn(getState())) return;

  if (getState().aiSharing.syncedWallet !== wallet) {
    if (!(await syncConsent(getState, dispatch, wallet))) {
      if (!getState().aiSharing.versionMismatch) backOff(dispatch, wallet);
      await refreshShownQueue(getState, dispatch);
      return;
    }
  }

  // Lowered when the service still finds a batch too large (a proxy in front of it with a
  // smaller cap, say), so the next try sends fewer records.
  let maxRecords: number = api.MAX_RECORDS_PER_UPLOAD;
  // Ratings go after the uploads, so the records they rate are on the service.
  let uploaded = false;
  // Checked before every batch: consent, the account or the flag can change mid-upload.
  while (mayUpload(getState(), wallet)) {
    const queued = await aiSharing.peek(wallet, api.MAX_RECORDS_PER_UPLOAD);
    if (queued.length === 0) {
      uploaded = true;
      break;
    }
    const batch = nextBatch(queued, maxRecords);
    const response = await api.uploadTurns(wallet, batch);

    if (response.ok) {
      // An empty 2xx body comes back as null: treat it as "all accepted".
      const rejected = Array.isArray(response.data?.rejected) ? response.data.rejected : [];
      const kept = new Set(
        rejected.filter(r => RETRYABLE_REJECTIONS.has(r.reason)).map(r => r.recordId),
      );
      if (rejected.length > 0) {
        // Ids and reason codes only, never content.
        console.warn('[AI sharing] the service rejected records', rejected);
      }
      await aiSharing.ack(batch.map(r => r.recordId).filter(id => !kept.has(id)));
      if (kept.size > 0) {
        // Kept to retry later, with the usual backoff, not every minute.
        backOff(dispatch, wallet);
        break;
      }
      dispatch(actions.uploadSucceeded({ wallet }));
      continue;
    }
    if (response.status === 0 && currentWallet() !== wallet) break;
    if (response.status === 403) {
      // No current consent for this wallet on the service: the consent text moved, or a
      // decision was made elsewhere. Stop, and let the next sync settle which decision stands.
      if (!checkVersionMismatch(dispatch, response)) dispatch(actions.setSyncedWallet(null));
      backOff(dispatch, wallet, response);
      break;
    }
    if (response.status === 413) {
      if (batch.length > 1) {
        maxRecords = Math.ceil(batch.length / 2);
        continue;
      }
      // Can't happen within the 128 KB record cap; if it does, the record can never upload.
      console.warn('[AI sharing] dropped a record larger than the service accepts', [
        batch[0].recordId,
      ]);
      await aiSharing.ack([batch[0].recordId]);
      continue;
    }
    // 400 (a malformed request: a client or signing bug, not the records), 401 (clock skew,
    // expired identity), 429, 503 (writes off), 5xx and network errors keep the records: they
    // are retried later and, at worst, expire from the queue after 7 days.
    backOff(dispatch, wallet, response);
    break;
  }
  if (uploaded) await deliverRatings(getState, dispatch, wallet);
  await refreshShownQueue(getState, dispatch);
}

/** Sends the wallet's waiting conversation erasures, oldest first; the failed reply, if any. */
async function deliverErasures(
  getState: () => AppState,
  dispatch: AppDispatch,
  wallet: string,
): Promise<MetricsResponse<unknown> | null> {
  for (const erasure of selectPending(getState(), wallet).erasures ?? []) {
    if (currentWallet() !== wallet) return null;
    const response = await api.eraseConversation(wallet, erasure.conversationId, erasure.decidedAt);
    if (!response.ok) return response;
    await updatePending(getState, dispatch, wallet, p => ({
      ...p,
      erasures: (p.erasures ?? []).filter(e => !sameErasure(e, erasure)),
    }));
  }
  return null;
}

const sameErasure = (
  a: { conversationId: string; decidedAt: string },
  b: { conversationId: string; decidedAt: string },
) => a.conversationId === b.conversationId && a.decidedAt === b.decidedAt;

/**
 * Sends the wallet's waiting ratings. A rating the service has no record for (404: rejected or
 * erased) or finds malformed (400) can never land and is dropped; any other failure keeps the
 * rest for the next try.
 */
async function deliverRatings(
  getState: () => AppState,
  dispatch: AppDispatch,
  wallet: string,
): Promise<void> {
  const ratings = Object.entries(selectPending(getState(), wallet).ratings ?? {});
  for (const [recordId, rating] of ratings) {
    if (!mayUpload(getState(), wallet)) return;
    const response = await api.putRating(wallet, recordId, rating);
    if (!response.ok && response.status !== 404 && response.status !== 400) {
      if (response.status === 403 && !checkVersionMismatch(dispatch, response)) {
        dispatch(actions.setSyncedWallet(null));
      }
      backOff(dispatch, wallet, response);
      return;
    }
    // Only if unchanged: a newer click meanwhile is still to be sent.
    await updatePending(getState, dispatch, wallet, p =>
      p.ratings?.[recordId] === rating ? withoutRatings(p, [recordId]) : p,
    );
  }
}
