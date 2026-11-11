// Why the local user cannot reach a DM partner right now: the DM input's copy and the call button's.
import type { DmUserState } from '@explorer-core/engine/protocol'

type Refusal = { message: string; call: string }

const COPY: Record<Exclude<DmUserState, 'connected'>, Refusal> = {
  notConnected: { message: 'You are not connected to chat.', call: 'You are not connected to chat.' },
  disconnected: { message: 'The user you are trying to message is offline.', call: 'This user is offline.' },
  privateMessagesBlocked: {
    message: 'The user you are trying to message only accepts DMs from friends.',
    call: 'This user only accepts calls from friends.'
  },
  blockedByOwnUser: { message: 'To message this user you must first unblock them.', call: 'Unblock this user to call them.' },
  privateMessagesBlockedByOwnUser: {
    message: 'Add this user as a friend to chat, or update your DM settings to connect with everyone.',
    call: 'Your privacy settings only allow calls with friends.'
  },
  otherClient: {
    message: 'User is not connected to chat. They may be using a client without DM support.',
    call: 'This user is not connected to chat. They may be using a client without calls.'
  },
  // a community channel's state; never a DM partner's
  noRouter: { message: 'Community chat is not available right now.', call: 'This user is offline.' }
}

export function dmRefusal(state: Exclude<DmUserState, 'connected'>, purpose: keyof Refusal): string {
  return COPY[state][purpose]
}
