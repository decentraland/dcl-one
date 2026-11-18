import {
  Entity,
  Material,
  MeshRenderer,
  Transform,
  VideoPlayer,
  VideoState,
  engine,
  videoEventsSystem,
} from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

/**
 * video: the screens of `specs/media-retry.test.ts`, which serves this scene from a loopback realm
 * that fails the media files' first requests.
 *
 * Three planes facing the spawn, each showing (looped) one file of `assets/` in its own basic
 * material: `recover` (`assets/recover.webm`, at (5, 2, 8)), nearest the spawn, and `broken`
 * (`assets/broken.webm`, at (11, 2, 15)), nearest (11, 0, 12), play from the start; `paused`
 * (`assets/paused.webm`, at (2, 2, 15)), nearest (2, 0, 12), starts paused and is set playing
 * 1 s after it first reports `VsError`, logging `video paused play`. Each logs
 * `video <name> state=<VideoState name>` whenever its reported state changes, and `video ready`
 * once all exist.
 */

function screen(name: string, src: string, x: number, z: number, playing: boolean, onState?: (state: VideoState) => void): Entity {
  const entity = engine.addEntity()
  Transform.create(entity, { position: Vector3.create(x, 2, z), scale: Vector3.create(4, 4, 1) })
  MeshRenderer.setPlane(entity)
  VideoPlayer.create(entity, { src, playing, loop: true })
  Material.setBasicMaterial(entity, { texture: Material.Texture.Video({ videoPlayerEntity: entity }) })
  videoEventsSystem.registerVideoEventsEntity(entity, (event) => {
    console.log(`video ${name} state=${VideoState[event.state]}`)
    onState?.(event.state)
  })
  return entity
}

export function main(): void {
  screen('recover', 'assets/recover.webm', 5, 8, true)
  screen('broken', 'assets/broken.webm', 11, 15, true)
  let playPausedInSeconds: number | undefined
  const paused = screen('paused', 'assets/paused.webm', 2, 15, false, (state) => {
    if (state === VideoState.VS_ERROR && playPausedInSeconds === undefined) playPausedInSeconds = 1
  })
  engine.addSystem((dt) => {
    if (playPausedInSeconds === undefined || playPausedInSeconds <= 0) return
    playPausedInSeconds -= dt
    if (playPausedInSeconds > 0) return
    VideoPlayer.getMutable(paused).playing = true
    console.log('video paused play')
  })
  console.log('video ready')
}
