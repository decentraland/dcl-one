import { AI_SHARING_CONSENT_VERSION, type AiSharingConsent } from '/shared/types/ai-sharing';

import { aiSharing } from '#preload';
import * as api from '/@/lib/aiSharingApi';
import { createAsyncThunk } from '/@/modules/store/thunk';
import type { AppDispatch, AppState } from '/@/modules/store';

import { refreshShownQueue } from './lists';
import { currentWallet, selectSharingConsent } from './selectors';
import {
  replaceConsent,
  sameDecision,
  saveConsent,
  updatePending,
  withoutRatings,
} from './settingsWrites';
import { sliceActions as actions } from './slice';

const toServerDecision = (consent: AiSharingConsent): api.AiSharingDecision => ({
  granted: consent.consent === 'granted',
  version: consent.version,
  decidedAt: consent.decidedAt,
});

/**
 * The service names a consent version other than this app's in a 403 or 409: newer than the text
 * this app can show, or one it hasn't rolled out yet. Either way no grant from here can be
 * accepted, so stop offering sharing. Returns whether that is the case.
 */
export function checkVersionMismatch(dispatch: AppDispatch, response: { body?: unknown }): boolean {
  const version = (response.body as { currentVersion?: unknown } | undefined)?.currentVersion;
  if (typeof version !== 'number' || version === AI_SHARING_CONSENT_VERSION) return false;
  dispatch(actions.setVersionMismatch());
  return true;
}

/**
 * Takes the service's latest decision as this wallet's, in place of `local`, emptying its queue
 * on a "no". Nothing changes if the creator decided again here meanwhile.
 */
async function adoptRemote(
  getState: () => AppState,
  dispatch: AppDispatch,
  wallet: string,
  local: AiSharingConsent,
  remote: api.AiSharingConsentState,
): Promise<void> {
  if (remote.decidedAt === null || remote.version === null) return;
  await replaceConsent(getState, dispatch, wallet, local, {
    consent: remote.granted ? 'granted' : 'denied',
    version: remote.version,
    decidedAt: remote.decidedAt,
    delivered: true,
  });
  const saved = selectSharingConsent(getState(), wallet);
  if (saved?.consent === 'denied' && saved.decidedAt === remote.decidedAt) {
    await aiSharing.purge(wallet);
  }
}

// Decisions reach the service one at a time, in the order they were made: it keeps the one it
// receives last, which must be the creator's last click.
let delivering: Promise<unknown> = Promise.resolve();

/**
 * Sends this wallet's decision. The service orders decisions by when it receives them, so the
 * creator's latest click wins; it refuses (409) a grant older than a withdrawal it already holds,
 * which is an offline machine delivering a stale "yes" after the creator said no elsewhere. Then
 * the withdrawal stands here too. Returns whether the service now holds a decision for us.
 */
function deliver(
  getState: () => AppState,
  dispatch: AppDispatch,
  wallet: string,
  consent: AiSharingConsent,
): Promise<boolean> {
  const run = delivering.then(() => deliverNow(getState, dispatch, wallet, consent));
  delivering = run.catch(() => {});
  return run;
}

async function deliverNow(
  getState: () => AppState,
  dispatch: AppDispatch,
  wallet: string,
  consent: AiSharingConsent,
): Promise<boolean> {
  const response = await api.putConsent(wallet, toServerDecision(consent));
  if (response.ok) {
    await replaceConsent(getState, dispatch, wallet, consent, { ...consent, delivered: true });
    // A newer decision made meanwhile is still to be sent: the service doesn't hold it yet.
    return sameDecision(selectSharingConsent(getState(), wallet), consent);
  }
  if (response.status === 409) {
    if (checkVersionMismatch(dispatch, response)) return false;
    // A "no" is never replaced by the service's "yes": withdrawals always apply there, so a 409
    // on one is a transient refusal, and it is retried rather than undone.
    if (consent.consent === 'denied') return false;
    const remote = await api.getConsent(wallet);
    if (!remote.ok) return false;
    await adoptRemote(getState, dispatch, wallet, consent, remote.data);
    return true;
  }
  return false;
}

/** The answer on the card or the Settings switch, for the signed-in wallet. */
export const decide = createAsyncThunk(
  'aiSharing/decide',
  async (granted: boolean, { getState, dispatch }) => {
    const wallet = currentWallet();
    if (wallet === null) return;
    const consent: AiSharingConsent = {
      consent: granted ? 'granted' : 'denied',
      version: AI_SHARING_CONSENT_VERSION,
      decidedAt: new Date().toISOString(),
      delivered: false,
    };
    // Saved before anything else: main reads it when a running turn ends.
    await saveConsent(getState, dispatch, wallet, consent);
    if (!granted) {
      await aiSharing.purge(wallet);
      // The service takes ratings only while sharing is on.
      await updatePending(getState, dispatch, wallet, p => withoutRatings(p));
    }
    // If this fails, the decision stays undelivered and the uploader's sync sends it.
    const delivered = await deliver(getState, dispatch, wallet, consent);
    dispatch(actions.setSyncedWallet(delivered ? wallet : null));
    await refreshShownQueue(getState, dispatch);
  },
);

/**
 * "Delete everything I've shared": sharing stops first (main reads the saved "no" when a running
 * turn ends), then the local queue goes, then the service erases the records and records the
 * withdrawal in one request, dated by this machine's clock like every other decision.
 */
export const deleteEverything = createAsyncThunk(
  'aiSharing/deleteEverything',
  async (_: void, { getState, dispatch, rejectWithValue }) => {
    const wallet = currentWallet();
    if (wallet === null) return rejectWithValue('Signed out');
    const withdrawal: AiSharingConsent = {
      consent: 'denied',
      version: AI_SHARING_CONSENT_VERSION,
      decidedAt: new Date().toISOString(),
      delivered: false,
    };
    await saveConsent(getState, dispatch, wallet, withdrawal);
    await aiSharing.purge(wallet);
    // Erasing everything covers every conversation erasure and rating still waiting.
    await updatePending(getState, dispatch, wallet, () => ({}));
    await refreshShownQueue(getState, dispatch);
    const response = await api.eraseAll(wallet, withdrawal.decidedAt);
    if (!response.ok) {
      // The "no" stays saved and undelivered, so the uploader keeps retrying it.
      dispatch(actions.setSyncedWallet(null));
      return rejectWithValue(response.error);
    }
    await replaceConsent(getState, dispatch, wallet, withdrawal, {
      ...withdrawal,
      delivered: true,
    });
    dispatch(actions.setSyncedWallet(wallet));
    dispatch(actions.clearSharedTurns());
    dispatch(actions.clearDigests());
    return response.data.deleted;
  },
);

/**
 * Makes this machine and the service agree on the wallet's decision, once per run and wallet. A
 * decision made here that never reached the service is sent, keeping its original time; so is
 * one the service has lost (a restore). Otherwise the service's latest decision is the creator's
 * latest anywhere (another machine's withdrawal, say) and is adopted. Returns whether they agree.
 */
export async function syncConsent(
  getState: () => AppState,
  dispatch: AppDispatch,
  wallet: string,
): Promise<boolean> {
  const local = selectSharingConsent(getState(), wallet);
  if (local === undefined) return false;
  const remote = await api.getConsent(wallet);
  if (!remote.ok) return false;

  const sameAsRemote =
    remote.data.decidedAt === local.decidedAt &&
    remote.data.granted === (local.consent === 'granted') &&
    remote.data.version === local.version;
  let agreed = true;
  if (sameAsRemote) {
    if (local.delivered !== true) {
      await replaceConsent(getState, dispatch, wallet, local, { ...local, delivered: true });
    }
  } else if (local.delivered !== true || remote.data.decidedAt === null) {
    agreed = await deliver(getState, dispatch, wallet, local);
  } else {
    await adoptRemote(getState, dispatch, wallet, local, remote.data);
  }
  // A decision made while this ran is not settled yet: the next sync sends it.
  if (agreed && selectSharingConsent(getState(), wallet)?.delivered !== true) agreed = false;
  if (agreed) dispatch(actions.setSyncedWallet(wallet));
  return agreed;
}
