import { hasSharingConsent } from '/shared/types/ai-sharing';

import { aiSharing } from '#preload';
import * as api from '/@/lib/aiSharingApi';
import { createAsyncThunk } from '/@/modules/store/thunk';
import type { AppDispatch, AppState } from '/@/modules/store';

import { currentWallet, selectSharingConsent } from './selectors';
import { updatePending, withoutRatings } from './settingsWrites';
import { sliceActions as actions } from './slice';
import { flush } from './upload';

/** What main has queued for the signed-in wallet (other wallets' records stay hidden). */
export const refreshQueue = createAsyncThunk('aiSharing/refreshQueue', async () => {
  const wallet = currentWallet();
  return wallet === null ? [] : aiSharing.list(wallet);
});

/**
 * Reads the queue again only while Settings shows it: listing reads every queued file, which
 * the upload loop must not pay for on each try when nobody is looking.
 */
export async function refreshShownQueue(
  getState: () => AppState,
  dispatch: AppDispatch,
): Promise<void> {
  if (getState().aiSharing.queueShown > 0) await dispatch(refreshQueue());
}

/** The next page of what the signed-in wallet has shared, or the first with `reset`. */
export const loadSharedTurns = createAsyncThunk(
  'aiSharing/loadSharedTurns',
  async ({ reset }: { reset: boolean }, { getState, rejectWithValue }) => {
    const wallet = currentWallet();
    if (wallet === null) return rejectWithValue('Signed out');
    // Read before the request: a delete or a fresh load that lands meanwhile bumps it, and this
    // page is then dropped instead of bringing back turns that are gone.
    const generation = getState().aiSharing.shared.generation;
    const cursor = reset ? null : getState().aiSharing.shared.next;
    const response = await api.listTurns(wallet, cursor);
    if (!response.ok) return rejectWithValue(response.error);
    return { ...response.data, reset, generation };
  },
);

/**
 * Erases one shared turn; sharing stays on. The service drops the summary of its conversation
 * with it, so summaries already on screen are loaded again.
 */
export const eraseSharedTurn = createAsyncThunk(
  'aiSharing/eraseSharedTurn',
  async (recordId: string, { dispatch, getState, rejectWithValue }) => {
    const wallet = currentWallet();
    if (wallet === null) return rejectWithValue('Signed out');
    const response = await api.eraseTurn(wallet, recordId);
    if (!response.ok) return rejectWithValue(response.error);
    if (getState().aiSharing.digests.status !== 'idle') void dispatch(loadDigests({ reset: true }));
    return recordId;
  },
);

/** The next page of the summaries made of the signed-in wallet's conversations. */
export const loadDigests = createAsyncThunk(
  'aiSharing/loadDigests',
  async ({ reset }: { reset: boolean }, { getState, rejectWithValue }) => {
    const wallet = currentWallet();
    if (wallet === null) return rejectWithValue('Signed out');
    const generation = getState().aiSharing.digests.generation;
    const cursor = reset ? null : getState().aiSharing.digests.next;
    const response = await api.listDigests(wallet, cursor);
    if (!response.ok) return rejectWithValue(response.error);
    return { ...response.data, reset, generation };
  },
);

/**
 * The creator marked a conversation as not shared: what it already shared goes too. Main has
 * dropped its queued turns; this asks the service to erase the ones it holds, and keeps asking
 * (saved, so a quit doesn't lose it) until the service has. `recordIds` are the conversation's
 * shared turns, whose waiting ratings are dropped.
 */
export const eraseConversation = createAsyncThunk(
  'aiSharing/eraseConversation',
  async (
    { conversationId, recordIds }: { conversationId: string; recordIds: string[] },
    { getState, dispatch },
  ) => {
    const wallet = currentWallet();
    if (wallet === null || !api.isConfigured()) return;
    if (selectSharingConsent(getState(), wallet) === undefined) return;
    const decidedAt = new Date().toISOString();
    await updatePending(getState, dispatch, wallet, p => ({
      ...withoutRatings(p, recordIds),
      erasures: [
        ...(p.erasures ?? []).filter(e => e.conversationId !== conversationId),
        { conversationId, decidedAt },
      ],
    }));
    dispatch(actions.clearSharedTurns());
    dispatch(actions.clearDigests());
    await dispatch(flush());
  },
);

/** A thumbs up or down (or neither) on one shared reply. Sent with the next upload. */
export const rateTurn = createAsyncThunk(
  'aiSharing/rateTurn',
  async (
    { recordId, rating }: { recordId: string; rating: api.AiSharingRating },
    { getState, dispatch },
  ) => {
    const wallet = currentWallet();
    if (wallet === null || !api.isConfigured()) return;
    if (!hasSharingConsent(selectSharingConsent(getState(), wallet))) return;
    await updatePending(getState, dispatch, wallet, p => ({
      ...p,
      ratings: { ...p.ratings, [recordId]: rating },
    }));
    await dispatch(flush());
  },
);
