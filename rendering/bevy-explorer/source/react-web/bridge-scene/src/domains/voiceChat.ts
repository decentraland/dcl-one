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
import { signed } from '../http'
import { relay } from '../system-helpers'
import { socialApiBase } from './communities'
import type { ActiveCommunityVoiceChat, VoiceChatState as WireVoiceChatState } from '../../../src/engine/protocol'

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

/** Unity `ActiveCommunityVoiceChatsResponse`: `{ data: { activeChats, total } }`. */
async function fetchActiveChats(): Promise<ActiveCommunityVoiceChat[]> {
  const res = await signed<{ data?: { activeChats?: ActiveChatRaw[] } }>(`${await socialApiBase()}/v1/community-voice-chats/active`).catch(
    (e: unknown) => {
      console.error('[voiceChat] active streams failed', e)
      return undefined
    }
  )
  return (res?.data?.activeChats ?? [])
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

/** The stream whose places include the parcel or the world the player is in (the first wins). */
export function streamHere(chats: ActiveCommunityVoiceChat[], parcel: string, realm: string): ActiveCommunityVoiceChat | null {
  const world = realm.toLowerCase()
  return (
    chats.find((c) => c.positions.includes(parcel) || (world !== '' && c.worlds.some((w) => w.toLowerCase() === world))) ?? null
  )
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

  relay('voiceChatState', async () => api.getStateStream(), (state) => {
    // the engine's strings are the wire's unions
    ctx.send({ kind: 'voiceChatState', state: state as WireVoiceChatState })
  })

  // The live streams: read on start, and again whenever one starts or ends.
  let active: ActiveCommunityVoiceChat[] = []
  let publishedHere = ''
  const refreshActive = async (): Promise<void> => {
    active = await fetchActiveChats()
    ctx.send({ kind: 'activeCommunityVoiceChats', chats: active })
    publishedHere = ''
  }
  ctx.on('hello', () => {
    void refreshActive()
  })

  relay('communityVoiceUpdate', async () => api.getCommunityUpdateStream(), (update) => {
    ctx.send({
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
    })
    void refreshActive()
  })

  // Where the player stands, every ~2s: the stream happening in this scene, pushed on change.
  let acc = 2
  let realmName = ''
  ctx.push((dt) => {
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
