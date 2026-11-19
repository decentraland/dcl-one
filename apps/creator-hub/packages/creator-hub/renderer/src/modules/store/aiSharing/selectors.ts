import type { AiSharingView } from '/shared/types/ai';
import {
  AI_SHARING_CONSENT_VERSION,
  consentFor,
  hasSharingConsent,
  type AiSharingConsent,
  type AiSharingPending,
} from '/shared/types/ai-sharing';

import { AuthServerProvider } from '/@/lib/auth';
import * as api from '/@/lib/aiSharingApi';
import { FeatureFlag } from '/@/modules/store/featureFlags/types';
import type { AppState } from '/@/modules/store';

/** The signed-in wallet, lowercase, or null when signed out. */
export const currentWallet = (): string | null =>
  AuthServerProvider.getAccount()?.toLowerCase() ?? null;

/** Sharing can be offered and turns uploaded: the flag is on, the service exists and agrees. */
export const isSharingFeatureOn = (state: AppState): boolean =>
  state.featureFlags.flags[FeatureFlag.AI_PROMPT_SHARING] === true &&
  api.isConfigured() &&
  !state.aiSharing.versionMismatch;

/** The decision of `wallet` (the signed-in one by default). Consent belongs to the account. */
export const selectSharingConsent = (
  state: AppState,
  wallet: string | null = currentWallet(),
): AiSharingConsent | undefined => consentFor(state.workspace.settings?.aiDataSharing, wallet);

/**
 * Whether Settings shows the sharing controls: wherever sharing is offered, and wherever this
 * wallet has a decision stored, so withdrawing and erasing stay possible after the flag is
 * turned off or the consent text moves on.
 */
export const canManageSharing = (state: AppState, wallet: string | null): boolean =>
  isSharingFeatureOn(state) ||
  (api.isConfigured() && selectSharingConsent(state, wallet) !== undefined);

/** Whether a turn sent now should be shared: feature on, consent current, signed in, not excluded. */
export function shouldShareTurn(state: AppState): boolean {
  if (!isSharingFeatureOn(state)) return false;
  if (!hasSharingConsent(selectSharingConsent(state))) return false;
  const session = state.ai.sessions.find(s => s.id === state.ai.currentSessionId);
  return session?.shareExcluded !== true;
}

/**
 * What the chat shows. The card waits for a first finished reply (asking before anything has
 * happened gets reflexive "no"s), and comes back after a consent version bump only for those
 * who had said yes.
 */
export function selectSharingView(state: AppState, wallet: string | null): AiSharingView {
  const session = state.ai.sessions.find(s => s.id === state.ai.currentSessionId);
  const conversationExcluded = session?.shareExcluded === true;
  if (!isSharingFeatureOn(state)) return { status: 'hidden', conversationExcluded };
  const consent = selectSharingConsent(state, wallet);
  if (hasSharingConsent(consent)) {
    const conversationHasShared = state.ai.messages.some(m => m.recordId !== undefined);
    return { status: 'on', conversationExcluded, conversationHasShared };
  }
  const notAskedYet =
    consent === undefined ||
    (consent.consent === 'granted' && consent.version < AI_SHARING_CONSENT_VERSION);
  // A finished reply: not a failed turn, and not one the user stopped.
  const hasReply = state.ai.messages.some(
    m => m.role === 'assistant' && m.done && !m.error && !m.cancelled,
  );
  if (!notAskedYet || state.aiSharing.notNow || !hasReply) {
    return { status: 'hidden', conversationExcluded };
  }
  return { status: wallet !== null ? 'ask' : 'signin', conversationExcluded };
}

/** How long `wallet`'s uploads wait after failures; none when its last try worked. */
export const selectBackoff = (state: AppState, wallet: string) =>
  state.aiSharing.backoff[wallet] ?? { failures: 0, nextUploadAt: 0 };

/** What is waiting to reach the service for `wallet`. */
export const selectPending = (state: AppState, wallet: string | null): AiSharingPending =>
  (wallet !== null ? state.workspace.settings?.aiSharingPending?.[wallet] : undefined) ?? {};
