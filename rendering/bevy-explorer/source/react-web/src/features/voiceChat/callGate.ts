// Whether the local user may call someone right now, and the reason shown when not — unity-explorer's
// CallButtonPresenter tooltips: the user is offline, accepts calls from friends only, the local user's
// own privacy blocks it, a call is already running, a community stream is running, the remote is busy.
import { showToast } from '../../design'
import type { DmUserState, VoiceChatState } from '../../engine/protocol'
import type { EngineSession } from '../session/useEngineSession'

export type CallGate = { allowed: true; reason?: undefined } | { allowed: false; reason: string }

export function callGate(
  voice: VoiceChatState | null,
  partner: { state: DmUserState | null; online: boolean } | undefined,
  isGuest: boolean
): CallGate {
  if (isGuest) return { allowed: false, reason: 'Voice chat is not available for guest accounts.' }
  // the engine sends a snapshot as soon as the HUD subscribes: none means this build has no voice chat
  if (voice == null) return { allowed: false, reason: 'Voice chat is not available.' }
  if (voice != null && voice.kind === 'community' && voice.status !== 'disconnected' && voice.status !== 'busy' && voice.status !== 'genericError') {
    return { allowed: false, reason: 'Leave the community stream to start a call.' }
  }
  if (voice != null && voice.kind === 'private' && voice.status !== 'disconnected' && voice.status !== 'busy' && voice.status !== 'genericError') {
    return { allowed: false, reason: 'You are already in a call.' }
  }
  if (partner == null || partner.state == null) return { allowed: true }
  switch (partner.state) {
    case 'connected':
      return partner.online ? { allowed: true } : { allowed: false, reason: 'This user is offline.' }
    case 'disconnected':
    case 'notConnected':
    case 'otherClient':
      return { allowed: false, reason: 'This user is offline.' }
    case 'privateMessagesBlocked':
      return { allowed: false, reason: 'This user only accepts calls from friends.' }
    case 'privateMessagesBlockedByOwnUser':
      return { allowed: false, reason: 'Your privacy settings only allow calls with friends.' }
    case 'blockedByOwnUser':
      return { allowed: false, reason: 'Unblock this user to call them.' }
  }
}

/** The call is being set up, rings, or runs. */
export function voiceActive(voice: VoiceChatState | null): boolean {
  return voice != null && voice.status !== 'disconnected' && voice.status !== 'busy' && voice.status !== 'genericError'
}

/** In the given community's stream (connected or joining). */
export function inCommunityStream(voice: VoiceChatState | null, communityId: string): boolean {
  return voiceActive(voice) && voice?.kind === 'community' && voice.communityId === communityId
}

/** Ring a user from a profile card, passport or the friends list: opens the DM conversation first
 *  (unity-explorer's `StartPrivateCallWithUserId`), then rings, or says why not. */
export function startCallWith(session: EngineSession, address: string): void {
  const gate = callGate(session.voiceChat.state, undefined, session.profile.data?.isGuest === true)
  if (!gate.allowed) {
    showToast(gate.reason, { tone: 'error' })
    return
  }
  session.chat.openConversation(address)
  session.voiceChat.act({ op: 'startPrivateCall', address })
}
