import {
  AudioAnalysis,
  AudioSource,
  AudioStream,
  InputAction,
  Material,
  MeshCollider,
  MeshRenderer,
  PBAudioAnalysisMode,
  TextShape,
  Transform,
  VideoPlayer,
  engine,
  pointerEventsSystem,
} from '@dcl/sdk/ecs'
import { Color4, Vector3 } from '@dcl/sdk/math'
import { setupVisualTest } from './visual-test-setup'

/**
 * audio — scene audio and its analysis through the engine, after unity-explorer's AudioAnalysis
 * playground (Plugins/NativeAudioAnalysis/Playground): four sources, each with an `AudioAnalysis`
 * driving an equaliser of eight band cubes and an amplitude cube, and a label with the numbers.
 *
 *   - `clip` (log mode) and `clip raw` (raw mode): the looping 20 s tone as an AudioSource.
 *   - `stream`: the same tone streamed from the scene's content as an AudioStream.
 *   - `video`: the video scene's fixture as a VideoPlayer (its audio is the same tone).
 *
 * Two boxes take clicks: `pause / resume` toggles the log clip's `playing`, `detach` removes
 * its `AudioAnalysis`. The scene also pauses the log clip by itself at PAUSE_AT_S, so a run
 * without input shows the values holding.
 */
const PAUSE_AT_S = 12
const BANDS = 8

type Entity = ReturnType<typeof engine.addEntity>

function box(position: Vector3, scale: Vector3, color: Color4): Entity {
  const entity = engine.addEntity()
  Transform.create(entity, { position, scale })
  MeshRenderer.setBox(entity)
  Material.setPbrMaterial(entity, { albedoColor: color, roughness: 0.8, metallic: 0 })
  return entity
}

function label(position: Vector3, text: string, fontSize = 1): Entity {
  const entity = engine.addEntity()
  Transform.create(entity, { position })
  TextShape.create(entity, { text, fontSize, textColor: Color4.White() })
  return entity
}

/** An equaliser under `name`: eight band cubes, an amplitude cube and a label of the values. */
class Equaliser {
  readonly bars: Entity[] = []
  readonly amplitude: Entity
  readonly text: Entity
  readonly view = { amplitude: 0, bands: new Array<number>(BANDS).fill(0) }

  constructor(readonly name: string, readonly source: Entity, x: number) {
    for (let i = 0; i < BANDS; i++) {
      this.bars.push(box(Vector3.create(x - 1.4 + i * 0.4, 0.5, 11), Vector3.create(0.3, 0.05, 0.3), Color4.create(0.2, 0.6 + i * 0.05, 0.9 - i * 0.1, 1)))
    }
    this.amplitude = box(Vector3.create(x + 2, 0.5, 11), Vector3.create(0.5, 0.05, 0.5), Color4.create(0.95, 0.8, 0.2, 1))
    this.text = label(Vector3.create(x, 3.2, 11), name, 0.8)
  }

  update() {
    if (!AudioAnalysis.tryReadIntoView(this.source, this.view)) {
      TextShape.getMutable(this.text).text = `${this.name}: no analysis`
      return
    }
    const height = (value: number) => Math.max(0.05, Math.min(4, value * 2))
    this.view.bands.forEach((band, i) => {
      const t = Transform.getMutable(this.bars[i]!)
      t.scale.y = height(band)
      t.position.y = t.scale.y / 2
    })
    const t = Transform.getMutable(this.amplitude)
    t.scale.y = height(this.view.amplitude)
    t.position.y = t.scale.y / 2
    TextShape.getMutable(this.text).text = `${this.name}: amp ${this.view.amplitude.toFixed(2)} · ${this.view.bands.map((b) => b.toFixed(2)).join(' ')}`
  }
}

export function main() {
  const ground = engine.addEntity()
  Transform.create(ground, { position: Vector3.create(8, 0, 8), scale: Vector3.create(16, 0.05, 16) })
  MeshRenderer.setBox(ground)
  Material.setPbrMaterial(ground, { albedoColor: Color4.create(0.4, 0.4, 0.42, 1), roughness: 0.9, metallic: 0 })

  const clip = box(Vector3.create(2, 1.5, 8), Vector3.create(1, 1, 1), Color4.create(0.9, 0.3, 0.2, 1))
  AudioSource.create(clip, { audioClipUrl: 'assets/audio/tone_440_loop.mp3', playing: true, loop: true, volume: 1 })
  AudioAnalysis.createAudioAnalysis(clip, PBAudioAnalysisMode.MODE_LOGARITHMIC)

  const clipRaw = box(Vector3.create(6, 1.5, 8), Vector3.create(1, 1, 1), Color4.create(0.9, 0.5, 0.2, 1))
  AudioSource.create(clipRaw, { audioClipUrl: 'assets/audio/tone_440_loop.mp3', playing: true, loop: true, volume: 1 })
  // not `createAudioAnalysis`: the SDK helper reads `mode || MODE_LOGARITHMIC`, and MODE_RAW is 0
  AudioAnalysis.create(clipRaw, { mode: PBAudioAnalysisMode.MODE_RAW, amplitude: 0, band0: 0, band1: 0, band2: 0, band3: 0, band4: 0, band5: 0, band6: 0, band7: 0 })

  const stream = box(Vector3.create(10, 1.5, 8), Vector3.create(1, 1, 1), Color4.create(0.2, 0.7, 0.3, 1))
  AudioStream.create(stream, { url: 'assets/audio/tone_440_loop.mp3', playing: true, volume: 1 })
  AudioAnalysis.createAudioAnalysis(stream, PBAudioAnalysisMode.MODE_LOGARITHMIC)

  const video = box(Vector3.create(14, 1.5, 8), Vector3.create(1.6, 1.2, 0.1), Color4.White())
  VideoPlayer.create(video, { src: 'assets/video/red_tone.mp4', playing: true, loop: true, volume: 1 })
  Material.setBasicMaterial(video, { texture: Material.Texture.Video({ videoPlayerEntity: video }) })
  AudioAnalysis.createAudioAnalysis(video, PBAudioAnalysisMode.MODE_LOGARITHMIC)

  const equalisers = [
    new Equaliser('clip', clip, 2),
    new Equaliser('clip raw', clipRaw, 6),
    new Equaliser('stream', stream, 10),
    new Equaliser('video', video, 14),
  ]

  const pauseButton = box(Vector3.create(3, 0.5, 4), Vector3.create(1.2, 0.6, 0.6), Color4.create(0.3, 0.3, 0.9, 1))
  MeshCollider.setBox(pauseButton)
  const pauseLabel = label(Vector3.create(3, 1.2, 4), 'pause / resume', 0.8)
  pointerEventsSystem.onPointerDown({ entity: pauseButton, opts: { button: InputAction.IA_POINTER, hoverText: 'pause / resume the log clip' } }, () => {
    const source = AudioSource.getMutable(clip)
    source.playing = !source.playing
  })

  const detachButton = box(Vector3.create(13, 0.5, 4), Vector3.create(1.2, 0.6, 0.6), Color4.create(0.9, 0.3, 0.3, 1))
  MeshCollider.setBox(detachButton)
  label(Vector3.create(13, 1.2, 4), 'detach', 0.8)
  pointerEventsSystem.onPointerDown({ entity: detachButton, opts: { button: InputAction.IA_POINTER, hoverText: 'remove the log clip\'s AudioAnalysis' } }, () => {
    AudioAnalysis.deleteFrom(clip)
  })

  let elapsed = 0
  let pausedByTimer = false
  engine.addSystem((dt) => {
    elapsed += dt
    if (!pausedByTimer && elapsed >= PAUSE_AT_S) {
      pausedByTimer = true
      AudioSource.getMutable(clip).playing = false
      TextShape.getMutable(pauseLabel).text = `pause / resume (paused at ${PAUSE_AT_S} s)`
    }
    for (const equaliser of equalisers) equaliser.update()
  })

  setupVisualTest({ lookAtPos: Vector3.create(8, 1.5, 9), cameraPos: Vector3.create(8, 5, 1) })
}
