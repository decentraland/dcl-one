// unity-explorer's AudioEventsSystem tests (Explorer/Assets/DCL/SDKComponents/AudioSources/Tests/
// AudioEventsSystemShould.cs), as far as a scene can show them: the `audio` scene (`scenes/audio`)
// plays a clip that ends, a stream and a stream nothing serves, and the spec reads the `AudioEvent`s
// the engine appends and the `AudioSource` it writes back.
//
// | Unity test | Scenario | Class |
// | --- | --- | --- |
// | PropagateAudioSourceStateChanges (Playing, then Ready, no repeat) | audio events reach the scene | Partial: states are sampled from the latest event, so Loading (gone within a frame) is asserted by the engine's unit tests, not here |
// | WriteBackPlayingFalseOnNaturalFinish (once, the other fields kept) | audio events reach the scene | Port |
// | ReportErrorOnFailedLoad | audio events reach the scene | Partial: a stream's unreachable url, since a scene cannot name a clip its manifest lacks |
// | PropagateMediaPlayerState (a stream reaches Playing) | audio events reach the scene | Port |
// | NotWriteBackOnSceneStop, NotWriteBackWhenLooping, NoEventForVideoPlayerOnlyEntities | | Gap: engine unit tests (crates/av/src/audio_events.rs) cover them; the scene shows one clip |
//
// The states are read from each entity's `AudioEvent`s (`/inspect_component <entity> AudioEvent`
// shows the set's latest event), the write-back from its `AudioSource`.
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
/** Budget for the clip to load, play its 3 s and end, and for the streams' first open (a helper spawn). */
const EVENTS_TIMEOUT_MS = 60_000

/** `MediaState` (library/protocol/proto/decentraland/sdk/components/common/media_state.proto). */
const MS = { NONE: 0, ERROR: 1, LOADING: 2, READY: 3, PLAYING: 4 } as const

/** A `PbAudioEvent` as the engine inspects it. */
interface AudioEvent {
  state: number
  timestamp: number
}

/** A `PbAudioSource` as the engine inspects it. */
interface AudioSource {
  audioClipUrl: string
  playing?: boolean
  loop?: boolean
}

const describe = (events: AudioEvent[]): string => events.map((e) => `tick ${e.timestamp}: state ${e.state}`).join('; ')
const has = (events: AudioEvent[], state: number): boolean => events.some((e) => e.state === state)

/** The latest events of an entity, as the polls saw them, one entry per change. */
class Seen {
  readonly events: AudioEvent[] = []

  add(events: AudioEvent[]): void {
    const latest = events.at(-1)
    const last = this.events.at(-1)
    if (latest && (!last || last.state !== latest.state || last.timestamp !== latest.timestamp)) this.events.push(latest)
  }

  states(): number[] {
    return this.events.map((e) => e.state)
  }
}

scenario(
  'audio events reach the scene',
  async ({ explorer, t }) => {
    const [x, y] = AUDIO_SCENE.base
    await explorer.teleport(x, y)
    const clips = await until(() => explorer.sceneEntities('AudioSource'), (entities) => entities.length > 0, {
      timeoutMs: SCENE_LIVE_TIMEOUT_MS,
      intervalMs: 1000,
      what: `the scene ${AUDIO_SCENE.id} at ${x},${y} to create its audio source`
    })
    assert.equal(clips.length, 1, `one clip expected, got ${clips.join(', ')}`)
    const clip = clips[0]!
    const streams = await explorer.sceneEntities('AudioStream')
    assert.equal(streams.length, 2, `two streams expected, got ${streams.join(', ')}`)

    // The page starts audio only after an input (crates/media/src/html/host.rs retries a refused
    // autoplay on the next key); one key press stands for the user's first interaction.
    if (readTarget().transport === 'web') {
      await explorer.key('ShiftLeft', 'down')
      await explorer.key('ShiftLeft', 'up')
    }

    // Every entity's latest event, sampled together from the start: the clip plays for 3 s and the
    // stream for 20 s, so each Playing is seen before it ends.
    const seen = new Map<string, Seen>([clip, ...streams].map((entity) => [entity, new Seen()]))
    const poll = async () => {
      await Promise.all([...seen].map(async ([entity, states]) => states.add(await explorer.inspect<AudioEvent[]>(entity, 'AudioEvent'))))
      return seen
    }
    const settled = (states: Seen) => has(states.events, MS.ERROR) || has(states.events, MS.PLAYING)
    const clipDone = (states: Seen) => has(states.events, MS.ERROR) || (has(states.events, MS.PLAYING) && states.events.at(-1)?.state === MS.READY)
    await until(poll, (seen) => clipDone(seen.get(clip)!) && streams.every((entity) => settled(seen.get(entity)!)), {
      timeoutMs: EVENTS_TIMEOUT_MS,
      intervalMs: 200,
      what: 'the clip to play and end, and both streams to play or fail'
    })
    await sleep(2000)
    await poll()
    for (const [entity, states] of seen) t.diagnostic(`${entity === clip ? 'clip' : `stream ${entity}`} events: ${describe(states.events)}`)

    // The clip: Playing, then Ready once it ends, never Error, and no replay afterwards.
    const clipEvents = seen.get(clip)!.events
    assert.ok(!has(clipEvents, MS.ERROR), `the clip reported an error: ${describe(clipEvents)}`)
    assert.equal(clipEvents.filter((e) => e.state === MS.PLAYING).length, 1, `the clip played once: ${describe(clipEvents)}`)
    assert.equal(clipEvents.at(-1)?.state, MS.READY, `the clip stays ready after its end: ${describe(clipEvents)}`)
    for (let i = 1; i < clipEvents.length; i++) {
      assert.ok(clipEvents[i]!.timestamp > clipEvents[i - 1]!.timestamp, `event ticks increase: ${describe(clipEvents)}`)
    }

    // The write-back: the clip's AudioSource reads playing=false with its other fields kept.
    const source = await explorer.inspect<AudioSource>(clip, 'AudioSource')
    t.diagnostic(`clip source after the finish: ${JSON.stringify(source)}`)
    assert.equal(source.playing, false, 'the engine wrote playing=false back after the clip ended')
    assert.equal(source.audioClipUrl, 'assets/audio/tone_440_3s.mp3', 'the write-back kept the clip url')

    // The streams: the served tone reaches Playing with no error; the unreachable url reaches Error.
    const outcomes = streams.map((entity) => seen.get(entity)!.events)
    const playing = outcomes.filter((events) => has(events, MS.PLAYING) && !has(events, MS.ERROR))
    const failed = outcomes.filter((events) => has(events, MS.ERROR))
    assert.equal(playing.length, 1, 'exactly one stream plays: the served tone')
    assert.equal(failed.length, 1, 'exactly one stream fails: the unreachable url')
  },
  { session: SESSION, timeoutMs: SCENARIO_TIMEOUT_MS }
)
