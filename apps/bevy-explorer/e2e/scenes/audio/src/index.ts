import { AudioEvent, AudioSource, AudioStream, Material, MediaState, MeshRenderer, TextShape, Transform, engine } from '@dcl/sdk/ecs'
import { Color4, Vector3 } from '@dcl/sdk/math'
import { setupVisualTest } from './visual-test-setup'

/**
 * audio — scene audio through the engine: AudioSource clips (kira on the standalone, a page
 * graph on the web) and AudioStreams (UUAV on the standalone, the page's media element on the
 * web), and the `AudioEvent`s they report, after unity-explorer's AudioEventsSystem
 * (AudioEventsSystemShould: a clip reports Loading, Playing and Ready, a finished clip's
 * `playing` reads false, a stream reports Playing, a bad url reports Error).
 *
 * Three emitters, each with a label under it showing the last event it received:
 *   - `clip`: a 3 s tone (assets/audio/tone_440_3s.mp3, generate-fixture.sh) that plays once.
 *     Its label also shows the AudioSource's `playing`, which the engine clears at the end.
 *   - `stream`: the 20 s tone streamed from the scene's own content.
 *   - `bad stream`: a url nothing answers.
 */
const STATE_NAMES: Record<number, string> = {
  [MediaState.MS_NONE]: 'none',
  [MediaState.MS_ERROR]: 'error',
  [MediaState.MS_LOADING]: 'loading',
  [MediaState.MS_READY]: 'ready',
  [MediaState.MS_PLAYING]: 'playing',
  [MediaState.MS_BUFFERING]: 'buffering',
  [MediaState.MS_SEEKING]: 'seeking',
  [MediaState.MS_PAUSED]: 'paused',
}

function box(position: Vector3, color: Color4) {
  const entity = engine.addEntity()
  Transform.create(entity, { position, scale: Vector3.create(1.5, 1.5, 1.5) })
  MeshRenderer.setBox(entity)
  Material.setPbrMaterial(entity, { albedoColor: color, roughness: 0.8, metallic: 0 })
  return entity
}

function label(position: Vector3, text: string) {
  const entity = engine.addEntity()
  Transform.create(entity, { position })
  TextShape.create(entity, { text, fontSize: 1.5, textColor: Color4.White() })
  return entity
}

/** The last event the scene received for `entity`, as text; the set is ordered by timestamp. */
function lastEvent(entity: ReturnType<typeof engine.addEntity>): string {
  let last: { state: number } | undefined
  for (const event of AudioEvent.get(entity)) last = event
  return last ? (STATE_NAMES[last.state] ?? String(last.state)) : 'waiting for the first event'
}

export function main() {
  const ground = engine.addEntity()
  Transform.create(ground, { position: Vector3.create(8, 0, 8), scale: Vector3.create(16, 0.05, 16) })
  MeshRenderer.setBox(ground)
  Material.setPbrMaterial(ground, { albedoColor: Color4.create(0.4, 0.4, 0.42, 1), roughness: 0.9, metallic: 0 })

  const clip = box(Vector3.create(4, 1.5, 8), Color4.create(0.9, 0.3, 0.2, 1))
  AudioSource.create(clip, { audioClipUrl: 'assets/audio/tone_440_3s.mp3', playing: true, loop: false, volume: 1 })
  const clipLabel = label(Vector3.create(4, 3, 8), 'clip')

  const stream = box(Vector3.create(8, 1.5, 8), Color4.create(0.2, 0.7, 0.3, 1))
  AudioStream.create(stream, { url: 'assets/audio/tone_440_loop.mp3', playing: true, volume: 1 })
  const streamLabel = label(Vector3.create(8, 3, 8), 'stream')

  const badStream = box(Vector3.create(12, 1.5, 8), Color4.create(0.3, 0.3, 0.9, 1))
  AudioStream.create(badStream, { url: 'https://127.0.0.1:9/missing.mp3', playing: true, volume: 1 })
  const badStreamLabel = label(Vector3.create(12, 3, 8), 'bad stream')

  engine.addSystem(() => {
    const source = AudioSource.getOrNull(clip)
    TextShape.getMutable(clipLabel).text = `clip: ${lastEvent(clip)} · playing=${source?.playing ?? '?'}`
    TextShape.getMutable(streamLabel).text = `stream: ${lastEvent(stream)}`
    TextShape.getMutable(badStreamLabel).text = `bad stream: ${lastEvent(badStream)}`
  })

  setupVisualTest({ lookAtPos: Vector3.create(8, 2, 8), cameraPos: Vector3.create(8, 3, 1.5) })
}
