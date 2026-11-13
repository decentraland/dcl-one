// Voice chat: private calls and community streams.
//   from: BevyApi.voiceChat (the engine's call state and the social service's started / ended
//         broadcasts) and the social-api's active-streams listing
//   to:   the page: voiceChatState (one snapshot per change), communityVoiceUpdate,
//         activeCommunityVoiceChats (the page matches them to the parcel `sceneInfo` names)
// Actions go the other way (voiceChatAction → BevyApi.voiceChat.action); their outcome shows on
// the state stream.
import { getPlayer } from '@dcl/sdk/players'
import { BevyApi } from '../bevy-api'
import type { Ctx } from '../bridge'
import { serviceBase, signed } from '../http'
import { relay } from '../system-helpers'
import type { ActiveCommunityVoiceChat, CommunityVoiceUpdateMessage } from '@explorer-core/engine/protocol'

type ActiveChatRaw = {
  communityId?: string
  communityName?: string
  communityImage?: string
  isMember?: boolean
  positions?: string[]
  worlds?: string[]
  participantCount?: number
  moderatorCount?: number
}

/** The answer is `{ data: { activeChats, total } }`; `signed` has already taken off `data`. A failed
 *  fetch is `null`, not an empty list: it says nothing about which streams are live. */
export async function fetchActiveChats(): Promise<ActiveCommunityVoiceChat[] | null> {
  let res: { activeChats?: ActiveChatRaw[] }
  try {
    res = (await signed<{ activeChats?: ActiveChatRaw[] }>(`${await serviceBase('socialApi')}/v1/community-voice-chats/active`)) ?? {}
  } catch (e: unknown) {
    console.error('[voiceChat] active streams failed', e)
    return null
  }
  return (res.activeChats ?? [])
    .filter((c) => c.communityId != null && c.communityId !== '')
    .map((c) => ({
      communityId: c.communityId ?? '',
      communityName: c.communityName ?? '',
      communityImage: c.communityImage,
      isMember: c.isMember === true,
      positions: c.positions ?? [],
      worlds: c.worlds ?? [],
      participantCount: c.participantCount ?? 0,
      moderatorCount: c.moderatorCount ?? 0
    }))
}

/** The live list after a stream's start or end broadcast. The broadcast has no counts: a stream
 *  just started has its creator, a moderator, in it. */
export function patchActive(
  chats: ActiveCommunityVoiceChat[],
  update: Pick<CommunityVoiceUpdateMessage, 'communityId' | 'status' | 'positions' | 'worlds' | 'isMember' | 'communityName' | 'communityImage'>
): ActiveCommunityVoiceChat[] {
  const rest = chats.filter((c) => c.communityId !== update.communityId)
  if (update.status === 'ended') return rest
  if (rest.length < chats.length) return chats
  return [
    ...chats,
    {
      communityId: update.communityId,
      communityName: update.communityName,
      communityImage: update.communityImage ?? undefined,
      isMember: update.isMember,
      positions: update.positions,
      worlds: update.worlds,
      participantCount: 1,
      moderatorCount: 1
    }
  ]
}

export function registerVoiceChat(ctx: Ctx): void {
  const api = BevyApi.voiceChat
  if (api == null) {
    console.log('[voiceChat] runtime has no voice chat')
    return
  }

  ctx.on('voiceChatAction', (msg) => {
    api.action(msg.action)
  })

  // The live streams: read on sign-in, on hello and when the user's own stream is entered or left;
  // other streams' starts and ends patch the list from their broadcast, which reaches every online
  // user (a read each would be one request per user per event). Reads can overlap: only the latest
  // answer counts, and a failed read keeps the last list. The read is signed, so none goes out
  // while signed out.
  let active: ActiveCommunityVoiceChat[] = []
  let reads = 0
  let account: string | null = null
  const refreshActive = async (): Promise<void> => {
    if (account == null) return
    const read = ++reads
    const chats = await fetchActiveChats()
    if (read !== reads || chats == null) return
    active = chats
    ctx.send({ kind: 'activeCommunityVoiceChats', chats: active })
  }
  // The list is per account (`isMember`) and signed: none before sign-in, the next account's after.
  const accountChanged = (userId: string | null): void => {
    if (userId === account) return
    account = userId
    reads++ // an answer for the previous account is dropped
    active = []
    ctx.send({ kind: 'activeCommunityVoiceChats', chats: [], loaded: false })
    void refreshActive()
  }

  // The social service leaves the starter out of its stream's start broadcast (the end reaches
  // everyone), and neither broadcast carries counts, so the list is also re-read when the user's own
  // community stream is entered or left: it lists the starter's stream and refreshes its counts.
  let inStream = ''
  relay('voiceChatState', async () => api.getStateStream(), (state) => {
    ctx.send({ kind: 'voiceChatState', state })
    const now = state.kind === 'community' && state.status === 'inCall' ? (state.communityId ?? '') : ''
    if (now !== inStream) {
      inStream = now
      void refreshActive()
    }
  })
  ctx.on('hello', () => {
    void refreshActive()
  })

  relay('communityVoiceUpdate', async () => api.getCommunityUpdateStream(), (update) => {
    const msg: CommunityVoiceUpdateMessage = {
      kind: 'communityVoiceUpdate',
      communityId: update.communityId,
      status: update.status,
      // i64s arrive as bigint
      createdAt: Number(update.createdAt),
      endedAt: update.endedAt == null ? null : Number(update.endedAt),
      positions: update.positions,
      worlds: update.worlds,
      isMember: update.isMember,
      communityName: update.communityName,
      communityImage: update.communityImage
    }
    ctx.send(msg)
    if (account == null) return
    reads++ // a read in flight predates this change
    active = patchActive(active, msg)
    ctx.send({ kind: 'activeCommunityVoiceChats', chats: active })
  })

  ctx.push(() => {
    accountChanged(getPlayer()?.userId ?? null)
  })
}
