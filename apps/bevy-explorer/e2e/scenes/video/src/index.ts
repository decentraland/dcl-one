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
 * Two planes at z = 12 facing the spawn, each playing (looped) one file of `assets/` into its own
 * basic material: `recover` (`assets/recover.webm`, x = 5) and `broken` (`assets/broken.webm`,
 * x = 11). Each logs `video <name> state=<VideoState name>` whenever its reported state changes,
 * and `video ready` once both exist.
 */

function screen(name: string, src: string, x: number): Entity {
  const entity = engine.addEntity()
  Transform.create(entity, { position: Vector3.create(x, 2, 12), scale: Vector3.create(4, 4, 1) })
  MeshRenderer.setPlane(entity)
  VideoPlayer.create(entity, { src, playing: true, loop: true })
  Material.setBasicMaterial(entity, { texture: Material.Texture.Video({ videoPlayerEntity: entity }) })
  videoEventsSystem.registerVideoEventsEntity(entity, (event) => {
    console.log(`video ${name} state=${VideoState[event.state]}`)
  })
  return entity
}

export function main(): void {
  screen('recover', 'assets/recover.webm', 5)
  screen('broken', 'assets/broken.webm', 11)
  console.log('video ready')
}
