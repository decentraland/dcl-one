import { Material, MeshRenderer, TextShape, Transform, VideoPlayer, VideoState, engine, videoEventsSystem } from '@dcl/sdk/ecs'
import { Color4, Vector3 } from '@dcl/sdk/math'
import { getActiveVideoStreams } from '~system/CommsApi'
import { setupVisualTest } from './visual-test-setup'

/**
 * livekit — LiveKit video through the engine's VideoPlayers, after unity-explorer's
 * LivekitPlayer tests (LivekitPlayerShould: a pinned participant's track plays, the current
 * stream follows the manager, a departed participant falls back): three screens with a label
 * under each showing its last VideoEvent state.
 *
 *   - `current`: `livekit-video://current-stream`, whatever the stream manager picks.
 *   - `pinned`: the first stream `getActiveVideoStreams` lists for an identity starting with
 *     `pinned-`, re-set whenever that address changes; `livekit-video://nobody/TR_none` while
 *     there is none (a participant that is not in the room, so the player falls back).
 *   - `invalid`: `livekit-video://broken`, a malformed address.
 *
 * The list label shows what `getActiveVideoStreams` returned, polled every 2 s.
 */
const POLL_S = 2
const STATE_NAMES: Record<number, string> = {
  [VideoState.VS_NONE]: 'none',
  [VideoState.VS_ERROR]: 'error',
  [VideoState.VS_LOADING]: 'loading',
  [VideoState.VS_READY]: 'ready',
  [VideoState.VS_PLAYING]: 'playing',
  [VideoState.VS_BUFFERING]: 'buffering',
  [VideoState.VS_SEEKING]: 'seeking',
  [VideoState.VS_PAUSED]: 'paused',
}
const NOBODY = 'livekit-video://nobody/TR_none'

function screen(name: string, x: number, src: string) {
  const entity = engine.addEntity()
  Transform.create(entity, { position: Vector3.create(x, 2, 10), scale: Vector3.create(4, 3, 0.1) })
  MeshRenderer.setBox(entity)
  VideoPlayer.create(entity, { src, playing: true })
  Material.setBasicMaterial(entity, { texture: Material.Texture.Video({ videoPlayerEntity: entity }) })
  const label = engine.addEntity()
  Transform.create(label, { position: Vector3.create(x, 0.3, 9.9) })
  TextShape.create(label, { text: `${name}: waiting for the first event`, fontSize: 1.5, textColor: Color4.White() })
  videoEventsSystem.registerVideoEventsEntity(entity, (event) => {
    TextShape.getMutable(label).text = `${name}: ${STATE_NAMES[event.state] ?? event.state}`
  })
  return entity
}

export function main() {
  const ground = engine.addEntity()
  Transform.create(ground, { position: Vector3.create(8, 0, 8), scale: Vector3.create(16, 0.05, 16) })
  MeshRenderer.setBox(ground)
  Material.setPbrMaterial(ground, { albedoColor: Color4.create(0.4, 0.4, 0.42, 1), roughness: 0.9, metallic: 0 })

  screen('current', 3, 'livekit-video://current-stream')
  const pinned = screen('pinned', 8, NOBODY)
  screen('invalid', 13, 'livekit-video://broken')

  const list = engine.addEntity()
  Transform.create(list, { position: Vector3.create(8, 4.2, 10) })
  TextShape.create(list, { text: 'streams: not asked yet', fontSize: 1, textColor: Color4.Yellow() })

  let sinceAsked = POLL_S
  let asking = false
  engine.addSystem((dt) => {
    sinceAsked += dt
    if (asking || sinceAsked < POLL_S) return
    sinceAsked = 0
    asking = true
    getActiveVideoStreams({})
      .then((response) => {
        const streams = response.streams ?? []
        TextShape.getMutable(list).text = streams.length === 0 ? 'streams: none' : `streams: ${streams.map((s) => `${s.identity} ${s.trackSid}`).join(' | ')}`
        const wanted = streams.find((s) => s.identity.startsWith('pinned-') && s.trackSid !== 'livekit-video://current-stream')?.trackSid ?? NOBODY
        const player = VideoPlayer.getMutable(pinned)
        if (player.src !== wanted) player.src = wanted
      })
      .catch((error) => {
        TextShape.getMutable(list).text = `streams: ${error}`
      })
      .finally(() => {
        asking = false
      })
  })

  setupVisualTest({ lookAtPos: Vector3.create(8, 2, 10), cameraPos: Vector3.create(8, 3, 2) })
}
