import { Material, MeshRenderer, TextShape, Transform, VideoPlayer, VideoState, engine, videoEventsSystem } from '@dcl/sdk/ecs'
import { Color4, Vector3 } from '@dcl/sdk/math'
import { setupVisualTest } from './visual-test-setup'

/**
 * video — scene video through the engine's player: UUAV on the standalone, the browser's video
 * element on the web. After unity-explorer's UUAV PlayMode suite (PlaybackShould: the state
 * reaches Playing, the clock advances, a presented frame is the fixture's colour).
 *
 * The screen loops `assets/video/red_tone.mp4` (generate-fixture.sh): 6 s of solid red at
 * 320x240 with a 440 Hz tone, so every frame is the same red and the source has an audio stream.
 * The label under the screen shows the last video event the scene received.
 */
/** `VideoState` is a const enum: no reverse lookup at runtime. */
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

export function main() {
  const ground = engine.addEntity()
  Transform.create(ground, {
    position: Vector3.create(8, 0, 8),
    scale: Vector3.create(16, 0.05, 16),
  })
  MeshRenderer.setBox(ground)
  Material.setPbrMaterial(ground, {
    albedoColor: Color4.create(0.4, 0.4, 0.42, 1),
    roughness: 0.9,
    metallic: 0,
  })

  // The screen: a box so every face carries the texture, whichever side the camera sees.
  const screen = engine.addEntity()
  Transform.create(screen, {
    position: Vector3.create(8, 2, 8),
    scale: Vector3.create(4, 3, 0.1),
  })
  MeshRenderer.setBox(screen)
  VideoPlayer.create(screen, {
    src: 'assets/video/red_tone.mp4',
    playing: true,
    loop: true,
  })
  Material.setBasicMaterial(screen, {
    texture: Material.Texture.Video({ videoPlayerEntity: screen }),
  })

  const label = engine.addEntity()
  Transform.create(label, { position: Vector3.create(8, 0.3, 7.9) })
  TextShape.create(label, {
    text: 'video: waiting for the first event',
    fontSize: 2,
    textColor: Color4.White(),
  })

  videoEventsSystem.registerVideoEventsEntity(screen, (event) => {
    TextShape.getMutable(label).text = `video: ${STATE_NAMES[event.state] ?? event.state} at ${event.currentOffset.toFixed(1)} s`
  })

  setupVisualTest({ lookAt: screen, cameraPos: Vector3.create(8, 2, 2.5) })
}
