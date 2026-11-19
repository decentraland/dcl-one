import { consentFor, type AiSharingConsent, type AiSharingPending } from '/shared/types/ai-sharing';

import { actions as workspaceActions } from '/@/modules/store/workspace';
import type { AppDispatch, AppState } from '/@/modules/store';

type SharingSettings = Pick<
  AppState['workspace']['settings'],
  'aiDataSharing' | 'aiSharingPending'
>;

// The sharing keys land in the store only once written, so two writes in flight would each drop
// the other's change. Every write here waits for the one before it, reads the keys as they are
// then, and fails loudly: main reads the saved decision when a turn ends, so nothing may go on
// as if it was saved when it wasn't. `change` returns null to write nothing.
let settingsWrite: Promise<unknown> = Promise.resolve();

export function writeSettings(
  getState: () => AppState,
  dispatch: AppDispatch,
  change: (settings: SharingSettings) => SharingSettings | null,
): Promise<void> {
  const write = settingsWrite.then(async () => {
    const { aiDataSharing, aiSharingPending } = getState().workspace.settings;
    const next = change({ aiDataSharing, aiSharingPending });
    if (next !== null) await dispatch(workspaceActions.updateAiSharingSettings(next)).unwrap();
  });
  settingsWrite = write.catch(() => {});
  return write;
}

export function saveConsent(
  getState: () => AppState,
  dispatch: AppDispatch,
  wallet: string,
  consent: AiSharingConsent,
): Promise<void> {
  return writeSettings(getState, dispatch, settings => ({
    ...settings,
    aiDataSharing: { ...settings.aiDataSharing, [wallet]: consent },
  }));
}

export const sameDecision = (a: AiSharingConsent | undefined, b: AiSharingConsent) =>
  a !== undefined &&
  a.consent === b.consent &&
  a.version === b.version &&
  a.decidedAt === b.decidedAt;

/**
 * Saves `next` only while `expected` is still the wallet's saved decision. A reply that arrives
 * after the creator decided again must not bring the older decision back.
 */
export function replaceConsent(
  getState: () => AppState,
  dispatch: AppDispatch,
  wallet: string,
  expected: AiSharingConsent,
  next: AiSharingConsent,
): Promise<void> {
  return writeSettings(getState, dispatch, settings =>
    sameDecision(consentFor(settings.aiDataSharing, wallet), expected)
      ? { ...settings, aiDataSharing: { ...settings.aiDataSharing, [wallet]: next } }
      : null,
  );
}

export function updatePending(
  getState: () => AppState,
  dispatch: AppDispatch,
  wallet: string,
  change: (pending: AiSharingPending) => AiSharingPending,
): Promise<void> {
  return writeSettings(getState, dispatch, settings => {
    const { [wallet]: current, ...others } = settings.aiSharingPending ?? {};
    const next = change(current ?? {});
    const empty =
      (next.erasures ?? []).length === 0 && Object.keys(next.ratings ?? {}).length === 0;
    return { ...settings, aiSharingPending: empty ? others : { ...others, [wallet]: next } };
  });
}

export const withoutRatings = (
  pending: AiSharingPending,
  recordIds?: string[],
): AiSharingPending => {
  if (recordIds === undefined) return { ...pending, ratings: {} };
  const ratings = { ...pending.ratings };
  for (const id of recordIds) delete ratings[id];
  return { ...pending, ratings };
};
