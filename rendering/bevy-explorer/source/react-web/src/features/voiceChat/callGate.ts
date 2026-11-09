// Whether the local user may call someone right now, and the reason shown when not — unity-explorer's
// CallButtonPresenter tooltips: the user is offline, accepts calls from friends only, the local user's
// own privacy blocks it, a call is already running, a community stream is running, the remote is busy.
import { showToast } from '../../design'
import type { DmUserState, VoiceChatActionWire, VoiceChatState } from '../../engine/protocol'
import type { EngineSession, VoiceChatSession } from '../session/useEngineSession'

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
    case 'noRouter': // a community channel's state; never a DM partner's
      return { allowed: false, reason: 'This user is offline.' }
    case 'privateMessagesBlocked':
      return { allowed: false, reason: 'This user only accepts calls from friends.' }
    case 'privateMessagesBlockedByOwnUser':
      return { allowed: false, reason: 'Your privacy settings only allow calls with friends.' }
    case 'blockedByOwnUser':
      return { allowed: false, reason: 'Unblock this user to call them.' }
  }
}

/** Whether the user may join a community stream. Another call does not block it: the engine hangs
 *  up first. */
export function streamGate(voice: VoiceChatState | null, isGuest: boolean): CallGate {
  if (isGuest) return { allowed: false, reason: 'Voice chat is not available for guest accounts.' }
  if (voice == null) return { allowed: false, reason: 'Voice chat is not available.' }
  return { allowed: true }
}

/** A start or join is in flight: another click would hang it up and start over. */
export function streamPending(voice: VoiceChatState | null): boolean {
  return voice?.status === 'startingCall' && voice.kind === 'community'
}

export function streamAct(voiceChat: VoiceChatSession, action: Extract<VoiceChatActionWire, { op: 'startCommunityCall' | 'joinCommunityCall' }>): void {
  const gate = streamGate(voiceChat.state, voiceChat.guest)
  if (!gate.allowed) showToast(gate.reason, { tone: 'error' })
  else if (!streamPending(voiceChat.state)) voiceChat.act(action)
}

/** Whether a community's stream is live: the live list once it has been read for this account; until
 *  then the community's own snapshot (`voiceChatStatus.isActive`, read with the community list) too. */
export function streamLive(voiceChat: Pick<VoiceChatSession, 'active' | 'activeLoaded'> | undefined, communityId: string, snapshot?: boolean): boolean {
  const listed = voiceChat?.active.some((c) => c.communityId === communityId) === true
  if (voiceChat?.activeLoaded === true) return listed
  return listed || snapshot === true
}

/** The call is being set up, rings, or runs. */
export function voiceActive(voice: VoiceChatState | null): boolean {
  return voice != null && voice.status !== 'disconnected' && voice.status !== 'busy' && voice.status !== 'genericError'
}

/** In the given community's stream (connected or joining). */
export function inCommunityStream(voice: VoiceChatState | null, communityId: string): boolean {
  return voiceActive(voice) && voice?.kind === 'community' && voice.communityId === communityId
}

/** The local user's own call to this address is being set up or rings. */
export function callingAlready(voice: VoiceChatState | null, address: string): boolean {
  return (
    voice?.kind === 'private' &&
    (voice.status === 'startingCall' || voice.status === 'startedCall') &&
    voice.peer?.toLowerCase() === address.toLowerCase()
  )
}

/** What the HUD knows of a user to call: their DM tab's state, or that the local user blocked them.
 *  A user never messaged has no state yet; the social service still refuses what it must. */
function knownPartner(session: EngineSession, address: string): { state: DmUserState | null; online: boolean } | undefined {
  const a = address.toLowerCase()
  const known = session.chat.conversations.find((c) => c.address === a)
  if (session.friends.blocked.some((b) => b.toLowerCase() === a)) return { state: 'blockedByOwnUser', online: known?.online ?? false }
  return known
}

/** Ring a user from a profile card, passport or the friends list: opens the DM conversation first
 *  (unity-explorer's `StartPrivateCallWithUserId`), then rings, or says why not. */
export function startCallWith(session: EngineSession, address: string): void {
  // a repeated click while this very call is being set up or rings: nothing to refuse
  if (callingAlready(session.voiceChat.state, address)) {
    session.chat.openConversation(address)
    return
  }
  const gate = callGate(session.voiceChat.state, knownPartner(session, address), session.profile.data?.isGuest === true)
  if (!gate.allowed) {
    showToast(gate.reason, { tone: 'error' })
    return
  }
  session.chat.openConversation(address)
  session.voiceChat.act({ op: 'startPrivateCall', address })
}
