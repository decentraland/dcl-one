// unity-explorer's AudioAnalysis (Explorer/Assets/DCL/SDKComponents/AudioAnalysis, its playground
// scene and the LastAudioFrameBuffer tests), as far as a scene can show it: the `audio` scene
// (`scenes/audio`) analyses a looping clip in log and raw mode, a stream and a video's audio, and
// the spec reads the values the engine writes back into each `AudioAnalysis`.
//
// | Unity test | Scenario | Class |
// | --- | --- | --- |
// | AudioAnalysisPlayground (values follow the audio; raw vs log) | audio analysis reaches the scene | Port |
// | LastAudioFrameBufferShould (one window at a time, drop while unconsumed) | | Gap: engine unit tests (crates/media/src/capture.rs) |
// | MediaPlayerComponentAudioFrameBufferShould (capture attached only with an analysis) | audio analysis reaches the scene | Partial: the stream and video rows report values; attach and detach are the debug panel's `captureEnabled` |
// | The pause keeps the last values (AudioAnalysisSystem skips a paused source) | audio analysis reaches the scene | Port |
//
// The values are read from each entity's `AudioAnalysis` (`/inspect_component <entity>
// AudioAnalysis`), which the scene's own `readIntoView` sees too. Standalone only: the web build
// writes no analysis (Unity has none on WebGL either).
import assert from 'node:assert/strict'
import { sleep, spec, until, type ScenarioOptions } from '../../driver/index.ts'
import { readTarget } from '../../driver/target.ts'
import { visualRealm } from './realm.ts'
import { AUDIO_SCENE } from './scenes.ts'

const scenario = spec(import.meta.url)
const server = await visualRealm()

const SESSION: ScenarioOptions['session'] = { realm: server.realm, position: '0,0', signIn: 'guest', windowSize: [960, 540] }
const SCENARIO_TIMEOUT_MS = 600_000
const SCENE_LIVE_TIMEOUT_MS = 60_000
/** Budget for the clips to load and the streams' first open. */
const VALUES_TIMEOUT_MS = 60_000
/** The scene pauses its log clip at 12 s; sampled after that with a margin. */
const PAUSE_AT_MS = 14_000

/** `PBAudioAnalysisMode` (library/protocol/proto/decentraland/sdk/components/audio_analysis.proto). */
const MODE = { RAW: 0, LOGARITHMIC: 1 } as const

/** A `PbAudioAnalysis` as the engine inspects it. */
interface AudioAnalysis {
  mode: number
  amplitudeGain?: number | null
  bandsGain?: number | null
  amplitude: number
  band0: number
  band1: number
  band2: number
  band3: number
  band4: number
  band5: number
  band6: number
  band7: number
}

const bands = (a: AudioAnalysis): number[] => [a.band0, a.band1, a.band2, a.band3, a.band4, a.band5, a.band6, a.band7]
const describe = (a: AudioAnalysis): string => `mode ${a.mode} amp ${a.amplitude.toFixed(3)} bands ${bands(a).map((b) => b.toFixed(3)).join(' ')}`
const lit = (a: AudioAnalysis): boolean => a.amplitude > 0 && Math.max(...bands(a)) > 0
const unit = (a: AudioAnalysis): boolean => [a.amplitude, ...bands(a)].every((v) => v >= 0 && v <= 1)

scenario(
  'audio analysis reaches the scene',
  async ({ explorer, t, report }) => {
    if (readTarget().transport !== 'native') {
      t.skip('audio analysis runs on the standalone only')
      return
    }
    const [x, y] = AUDIO_SCENE.base
    await explorer.teleport(x, y)
    const analysed = await until(() => explorer.sceneEntities('AudioAnalysis'), (entities) => entities.length === 4, {
      timeoutMs: SCENE_LIVE_TIMEOUT_MS,
      intervalMs: 1000,
      what: `the scene ${AUDIO_SCENE.id} at ${x},${y} to create its four analysed entities`
    })
    const started = Date.now()

    // Every analysed entity reports a non-zero amplitude and band once its audio plays.
    const read = async () => Object.fromEntries(await Promise.all(analysed.map(async (entity) => [entity, await explorer.inspect<AudioAnalysis>(entity, 'AudioAnalysis')] as const)))
    const first = await until(read, (all) => Object.values(all).every(lit), {
      timeoutMs: VALUES_TIMEOUT_MS,
      intervalMs: 500,
      what: 'every analysed entity to report amplitude and bands'
    })
    for (const [entity, a] of Object.entries(first)) t.diagnostic(`${entity}: ${describe(a)}`)
    const byMode = (mode: number) => Object.entries(first).filter(([, a]) => a.mode === mode)
    assert.equal(byMode(MODE.RAW).length, 1, 'one raw-mode analysis')
    assert.equal(byMode(MODE.LOGARITHMIC).length, 3, 'three log-mode analyses')
    for (const [entity, a] of byMode(MODE.LOGARITHMIC)) assert.ok(unit(a), `log mode stays within [0, 1] on ${entity}: ${describe(a)}`)
    const [raw] = byMode(MODE.RAW)
    assert.ok(raw![1].amplitude > 0, `raw amplitude is unbounded but positive: ${describe(raw![1])}`)

    // The window moves: a later read of a playing source differs from the first.
    await sleep(1000)
    const second = await read()
    const moving = Object.entries(second).filter(([entity, a]) => describe(a) !== describe(first[entity]!))
    assert.ok(moving.length > 0, 'at least one playing source reports a new window')

    // The paused log clip keeps its last values: two reads after the scene's own pause agree.
    const untilPause = PAUSE_AT_MS - (Date.now() - started)
    if (untilPause > 0) await sleep(untilPause)
    const paused = await explorer.sceneEntities('AudioSource')
    const stillPlaying = await Promise.all(paused.map(async (entity) => [entity, (await explorer.inspect<{ playing?: boolean }>(entity, 'AudioSource')).playing] as const))
    const pausedClip = stillPlaying.find(([, playing]) => playing === false)?.[0]
    assert.ok(pausedClip, `the scene paused its log clip: ${JSON.stringify(stillPlaying)}`)
    const held1 = await explorer.inspect<AudioAnalysis>(pausedClip, 'AudioAnalysis')
    await sleep(1500)
    const held2 = await explorer.inspect<AudioAnalysis>(pausedClip, 'AudioAnalysis')
    t.diagnostic(`paused clip holds: ${describe(held2)}`)
    assert.equal(describe(held2), describe(held1), 'a paused clip keeps its last analysis')
    assert.ok(lit(held2), 'the held values are the last non-zero ones')

    await report.write({ 'frame.png': await explorer.screenshot() })
  },
  { session: SESSION, timeoutMs: SCENARIO_TIMEOUT_MS }
)
