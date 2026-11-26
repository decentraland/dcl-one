import { createSlice, type PayloadAction } from '@reduxjs/toolkit';

import type { AiSharingQueuedItem } from '/shared/types/ai-sharing';

import type * as api from '/@/lib/aiSharingApi';

import { decide, deleteEverything } from './consent';
import {
  eraseConversation,
  eraseSharedTurn,
  loadDigests,
  loadSharedTurns,
  rateTurn,
  refreshQueue,
} from './lists';
import { flush } from './upload';

export * from './selectors';

/** Backoff between failed uploads: 1, 2, 4… minutes, capped at an hour before the jitter below. */
const MIN_BACKOFF_MS = 60_000;
const MAX_BACKOFF_MS = 60 * 60_000;
// Each wait is stretched by a random 0–20%, so clients that failed together (an outage, the
// day's quota) don't all come back in the same second. Never shortened: Retry-After holds. So the
// longest wait is 72 minutes; clamping after the jitter would bring every capped client back at
// the same instant again.
export const BACKOFF_JITTER = 0.2;

export type DeleteStatus = 'idle' | 'confirming' | 'deleting' | 'deleted' | 'failed';

export interface SharedTurnsState {
  status: 'idle' | 'loading' | 'loaded' | 'failed';
  // Bumped by every reset load and delete; a page from an older one is stale.
  generation: number;
  turns: api.AiSharingStoredTurn[];
  next: string | null;
  erasing: string[];
}

export interface DigestsState {
  status: 'idle' | 'loading' | 'loaded' | 'failed';
  // Bumped by every reset load and erasure; a page from an older one is stale.
  generation: number;
  digests: api.AiSharingDigest[];
  next: string | null;
  // Pages on screen, so a reload after an erasure brings back as many.
  pages: number;
}

export interface WalletBackoff {
  failures: number;
  nextUploadAt: number;
}

export interface AiSharingState {
  /** The consent card's "Not now", for this app run only. */
  notNow: boolean;
  queued: AiSharingQueuedItem[];
  deleteStatus: DeleteStatus;
  deletedCount: number | null;
  /** The wallet whose consent this run has settled with the service, so uploads may start. */
  syncedWallet: string | null;
  /** Upload backoff per wallet (lowercase): another wallet's failures don't hold this one back. */
  backoff: Record<string, WalletBackoff>;
  /** How many views show the upload queue; it is read again after an upload only while one does. */
  queueShown: number;
  /**
   * The service's consent version differs from the text this app shows (newer, or not yet
   * rolled out). No grant from this app can be accepted, so it stops offering sharing.
   */
  versionMismatch: boolean;
  /** What the signed-in wallet has shared, as the service holds it (Settings). */
  shared: SharedTurnsState;
  /** The summaries made of the signed-in wallet's conversations (Settings). */
  digests: DigestsState;
}

const initialState: AiSharingState = {
  notNow: false,
  queued: [],
  deleteStatus: 'idle',
  deletedCount: null,
  syncedWallet: null,
  backoff: {},
  queueShown: 0,
  versionMismatch: false,
  shared: { status: 'idle', generation: 0, turns: [], next: null, erasing: [] },
  digests: { status: 'idle', generation: 0, digests: [], next: null, pages: 0 },
};

/** A rejection from a load that a newer reset or erasure already replaced. */
const isStale = (payload: unknown, generation: number) =>
  typeof payload === 'object' &&
  payload !== null &&
  'generation' in payload &&
  payload.generation !== generation;

const slice = createSlice({
  name: 'aiSharing',
  initialState,
  reducers: {
    notNow: state => {
      state.notNow = true;
    },
    setVersionMismatch: state => {
      state.versionMismatch = true;
    },
    setSyncedWallet: (state, { payload }: PayloadAction<string | null>) => {
      state.syncedWallet = payload;
    },
    showQueue: state => {
      state.queueShown += 1;
    },
    hideQueue: state => {
      state.queueShown = Math.max(0, state.queueShown - 1);
    },
    uploadSucceeded: (state, { payload }: PayloadAction<{ wallet: string }>) => {
      delete state.backoff[payload.wallet];
    },
    uploadFailed: {
      reducer: (
        state,
        { payload }: PayloadAction<{ wallet: string; retryAfterSeconds?: number; jitter: number }>,
      ) => {
        const failures = (state.backoff[payload.wallet]?.failures ?? 0) + 1;
        const backoff = Math.min(MIN_BACKOFF_MS * 2 ** (failures - 1), MAX_BACKOFF_MS);
        // Capped like the backoff: a far-off Retry-After must not stall uploads, erasures and
        // withdrawals for the rest of the session.
        const asked = Math.min((payload.retryAfterSeconds ?? 0) * 1000, MAX_BACKOFF_MS);
        const wait = Math.max(backoff, asked);
        state.backoff[payload.wallet] = {
          failures,
          nextUploadAt: Date.now() + Math.round(wait * (1 + BACKOFF_JITTER * payload.jitter)),
        };
      },
      // The random draw happens here, so the reducer stays a pure function of its action.
      prepare: (payload: { wallet: string; retryAfterSeconds?: number }) => ({
        payload: { ...payload, jitter: Math.random() },
      }),
    },
    askDeleteConfirmation: state => {
      state.deleteStatus = 'confirming';
    },
    resetDelete: state => {
      state.deleteStatus = 'idle';
      state.deletedCount = null;
    },
    clearSharedTurns: state => {
      state.shared = { ...initialState.shared, generation: state.shared.generation + 1 };
    },
    clearDigests: state => {
      state.digests = { ...initialState.digests, generation: state.digests.generation + 1 };
    },
  },
  extraReducers: builder => {
    builder
      .addCase(refreshQueue.fulfilled, (state, { payload }) => {
        state.queued = payload;
      })
      .addCase(deleteEverything.pending, state => {
        state.deleteStatus = 'deleting';
        state.deletedCount = null;
        state.shared.generation += 1;
        state.digests.generation += 1;
      })
      .addCase(deleteEverything.fulfilled, (state, { payload }) => {
        state.deleteStatus = 'deleted';
        state.deletedCount = payload;
      })
      .addCase(deleteEverything.rejected, state => {
        state.deleteStatus = 'failed';
      })
      .addCase(loadSharedTurns.pending, (state, { meta }) => {
        if (meta.arg.reset) state.shared.generation += 1;
        state.shared.status = 'loading';
      })
      .addCase(loadSharedTurns.fulfilled, (state, { payload }) => {
        if (payload.generation !== state.shared.generation) return;
        state.shared.turns = payload.reset
          ? payload.records
          : [...state.shared.turns, ...payload.records];
        state.shared.next = payload.next;
        state.shared.status = 'loaded';
      })
      .addCase(loadSharedTurns.rejected, state => {
        state.shared.status = 'failed';
      })
      .addCase(loadDigests.pending, (state, { meta }) => {
        if (meta.arg.reset) state.digests.generation += 1;
        state.digests.status = 'loading';
      })
      .addCase(loadDigests.fulfilled, (state, { payload }) => {
        if (payload.generation !== state.digests.generation) return;
        state.digests.digests = payload.reset
          ? payload.digests
          : [...state.digests.digests, ...payload.digests];
        state.digests.next = payload.next;
        state.digests.pages = payload.reset ? payload.pages : state.digests.pages + payload.pages;
        state.digests.status = 'loaded';
      })
      .addCase(loadDigests.rejected, (state, { payload }) => {
        if (isStale(payload, state.digests.generation)) return;
        state.digests.status = 'failed';
      })
      .addCase(eraseSharedTurn.pending, (state, { meta }) => {
        state.shared.erasing.push(meta.arg);
      })
      .addCase(eraseSharedTurn.fulfilled, (state, { payload }) => {
        state.shared.turns = state.shared.turns.filter(t => t.recordId !== payload);
        state.shared.erasing = state.shared.erasing.filter(id => id !== payload);
      })
      .addCase(eraseSharedTurn.rejected, (state, { meta }) => {
        state.shared.erasing = state.shared.erasing.filter(id => id !== meta.arg);
      });
  },
});

/** The plain reducers' action creators; the thunks in this folder dispatch them. */
export const sliceActions = slice.actions;

export const actions = {
  ...slice.actions,
  refreshQueue,
  decide,
  deleteEverything,
  loadSharedTurns,
  eraseSharedTurn,
  loadDigests,
  eraseConversation,
  rateTurn,
  flush,
};
export const reducer = slice.reducer;
