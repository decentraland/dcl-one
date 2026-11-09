// Voice chat: private calls and community streams.
//   from: BevyApi.voiceChat (the engine's call state and the social service's started / ended
//         broadcasts) and the social-api's active-streams listing
//   to:   the page: voiceChatState (one snapshot per change), communityVoiceUpdate,
//         activeCommunityVoiceChats, sceneVoiceChat (the stream happening where the player stands)
// Actions go the other way (voiceChatAction → BevyApi.voiceChat.action); their outcome shows on
// the state stream. Mirrors unity-explorer's VoiceChatOrchestrator relays plus its
// CommunityVoiceChatCallStatusService active-chats fetch and SceneVoiceChatTrackerService.
import { getPlayer } from '@dcl/sdk/players'
import { getRealm } from '~system/Runtime'
import { BevyApi } from '../bevy-api'
import type { Ctx } from '../bridge'
import { serviceBase, signed } from '../http'
import { relay } from '../system-helpers'
import type { ActiveCommunityVoiceChat, CommunityVoiceUpdateMessage, VoiceChatState as WireVoiceChatState } from '../../../src/engine/protocol'

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

/** The stream whose places include where the player is (the first wins): in a world (an ENS name,
 *  `*.eth`), a place in that world; in Genesis City, the parcel. A world's parcels are its own, so
 *  they never match a Genesis place (unity-explorer's tracker picks by `IsWorld` the same way). With
 *  the realm not known yet, nothing matches. */
export function streamHere(chats: ActiveCommunityVoiceChat[], parcel: string, realm: string): ActiveCommunityVoiceChat | null {
  const name = realm.toLowerCase()
  if (name === '') return null
  if (name.endsWith('.eth')) return chats.find((c) => c.worlds.some((w) => w.toLowerCase() === name)) ?? null
  return chats.find((c) => c.positions.includes(parcel)) ?? null
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
  // The stream last pushed as `sceneVoiceChat` ('' for none); `null` after a new list, so the next
  // tick pushes again, including `chat: null` when the stream here ended, and fresh counts.
  let publishedHere: string | null = null
  let reads = 0
  let account: string | null = null
  const refreshActive = async (): Promise<void> => {
    if (account == null) return
    const read = ++reads
    const chats = await fetchActiveChats()
    if (read !== reads || chats == null) return
    active = chats
    ctx.send({ kind: 'activeCommunityVoiceChats', chats: active })
    publishedHere = null
  }
  // The list is per account (`isMember`) and signed: none before sign-in, the next account's after.
  const accountChanged = (userId: string | null): void => {
    if (userId === account) return
    account = userId
    reads++ // an answer for the previous account is dropped
    active = []
    publishedHere = null
    ctx.send({ kind: 'activeCommunityVoiceChats', chats: [], loaded: false })
    ctx.send({ kind: 'sceneVoiceChat', chat: null })
    void refreshActive()
  }

  // The social service broadcasts a stream's start and end to everyone but the user who started or
  // ended it, so the list is also re-read when the user's own community stream is entered or left.
  let inStream = ''
  relay('voiceChatState', async () => api.getStateStream(), (state) => {
    // the engine's strings are the wire's unions
    const wire = state as WireVoiceChatState
    ctx.send({ kind: 'voiceChatState', state: wire })
    const now = wire.kind === 'community' && wire.status === 'inCall' ? (wire.communityId ?? '') : ''
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
      status: update.status === 'started' ? 'started' : 'ended',
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
    publishedHere = null
  })

  // Where the player stands, every ~2s: the stream happening in this scene, pushed on change.
  let acc = 2
  let realmName = ''
  ctx.push((dt) => {
    accountChanged(getPlayer()?.userId ?? null)
    acc += dt
    if (acc < 2) return
    acc = 0
    getRealm({})
      .then(({ realmInfo }) => {
        realmName = realmInfo?.realmName ?? ''
      })
      .catch(() => undefined)
    const pos = getPlayer()?.position
    const parcel = pos == null ? '' : `${Math.floor(pos.x / 16)},${Math.floor(pos.z / 16)}`
    const here = streamHere(active, parcel, realmName)
    const key = here?.communityId ?? ''
    if (key === publishedHere) return
    publishedHere = key
    ctx.send({ kind: 'sceneVoiceChat', chat: here })
  })
}
