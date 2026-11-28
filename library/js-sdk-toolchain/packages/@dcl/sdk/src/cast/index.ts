import { engine, Entity, Transform, VideoPlayer, videoEventsSystem } from '@dcl/ecs'
import {
  consumeMessages,
  getActiveVideoStreams,
  publishData,
  subscribeToTopic,
  unsubscribeFromTopic
} from '~system/CommsApi'
import { getRealm } from '~system/Runtime'
import { signedFetch } from '~system/SignedFetch'
import { getUserData } from '~system/UserIdentity'
import { Role } from './machine'
import { Cast, Io, open } from './runtime'

export * from './machine'
export { Cast, Io } from './runtime'

const topic = 'presentation'

const system: Io = {
  now: Date.now,
  frame: (fn) => void engine.addSystem(fn),
  async realm() {
    const [{ realmInfo }, { data }] = await Promise.all([getRealm({}), getUserData({})])
    return { baseUrl: realmInfo?.baseUrl ?? '', preview: !!realmInfo?.isPreview, address: data?.userId ?? '' }
  },
  tracks: async () => (await getActiveVideoStreams({})).streams,
  async packets() {
    // Unity answers a bare array, Bevy the declared { messages }; each has its sender and its data.
    const answer: any = await consumeMessages({ topic })
    return Array.isArray(answer) ? answer : answer.messages
  },
  async fetch(method, url, body) {
    const headers: Record<string, string> = body ? { 'Content-Type': 'application/json' } : {}
    const { status, body: text } = await signedFetch({ url, init: { method, body, headers } })
    return { status, body: text }
  },
  topic: (on) => void (on ? subscribeToTopic : unsubscribeFromTopic)({ topic }),
  publish: (data) => void publishData({ topic, data }),
  // Only the source is written: a paused player leaves Bevy's list of active tracks.
  screen(entity, src) {
    const player = VideoPlayer.getMutableOrNull(entity as Entity)
    if (player) player.src = src
    else VideoPlayer.create(entity as Entity, { src })
  },
  video(entity) {
    const event = videoEventsSystem.getVideoState(entity as Entity)
    return event && { stamp: event.timestamp, state: event.state }
  },
  box(entity, to) {
    const { scale } = Transform.getOrCreateMutable(entity as Entity)
    if (to) [scale.x, scale.y] = to
    return [scale.x, scale.y]
  }
}

/** Follows the scene's stream and Cast as `role`; the server still decides what the wallet may do. */
export const openCast = <R extends Role>(role: R): Cast<R> => open(role, system)
