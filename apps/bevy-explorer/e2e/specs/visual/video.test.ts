// unity-explorer's UUAV PlayMode suite (Explorer/Assets/Plugins/UUAV/Packages/UUAV/Tests), as far
// as a scene can show it: the `video` scene (`scenes/video`) plays a committed fixture through
// the engine's player, which is UUAV on the standalone and the browser's video element on the web.
//
// | Unity test | Scenario | Class |
// | --- | --- | --- |
// | OpenValidMediaShould (the state reaches Playing, no error) | the video plays | Port |
// | PlaybackShould.AdvanceClockWithRealTime | the video plays | Port |
// | PlaybackShould.PresentFrames (a red frame of the fixture's size) | the video plays | Partial: the frame's centre pixel is red; its size is the scene's screen, not the fixture's |
// | SeekShould, AudioContinuityShould, HelperProcessRecoveryShould, OpenInvalidMediaShould | | Gap: a scene cannot seek, capture audio or kill the helper; the UUAV workspace's own tests cover them |
//
// The state and the clock are read from the player entity's `VideoEvent`s
// (`/inspect_component <entity> VideoEvent`); the presented frame from `/screenshot`, sampled
// where `/entity_screen_position` puts the screen.
import assert from 'node:assert/strict'
import { decodePng, sleep, spec, until, type Image, type ScenarioOptions, type ScreenPosition } from '../../driver/index.ts'
import { visualRealm } from './realm.ts'
import { VIDEO_SCENE } from './scenes.ts'

const scenario = spec(import.meta.url)
const server = await visualRealm()

const SESSION: ScenarioOptions['session'] = { realm: server.realm, position: '0,0', signIn: 'guest', windowSize: [960, 540] }
const SCENARIO_TIMEOUT_MS = 600_000
const SCENE_LIVE_TIMEOUT_MS = 60_000
/** Budget for the first open: a helper spawn and the fixture's download. */
const PLAYING_TIMEOUT_MS = 60_000
/** The clock must pass this offset: proof it runs, whether or not the 6 s loop wrapped. */
const CLOCK_RUN_S = 1
const CLOCK_TIMEOUT_MS = 20_000

/** `PbVideoState` (library/protocol/proto/decentraland/sdk/components/video_event.proto). */
const VS = { ERROR: 1, PLAYING: 4 } as const

/** A `PbVideoEvent` as the engine inspects it. */
interface VideoEvent {
  timestamp: number
  tickNumber: number
  currentOffset: number
  videoLength: number
  state: number
}

const describe = (events: VideoEvent[]): string => events.map((e) => `tick ${e.tickNumber}: state ${e.state} at ${e.currentOffset.toFixed(2)} s`).join('; ')
const maxOffset = (events: VideoEvent[]): number => Math.max(-1, ...events.map((e) => e.currentOffset))

/** The mean colour of the `(2 * radius + 1)²` block around a window point, in the frame's pixels. */
function sample(frame: Image, at: ScreenPosition, radius = 3): { r: number; g: number; b: number } {
  assert.ok(at.x !== null && at.y !== null, 'the screen has no window position')
  const x = Math.round(((at.x - at.viewport.x) / at.viewport.width) * frame.width)
  const y = Math.round(((at.y - at.viewport.y) / at.viewport.height) * frame.height)
  let r = 0
  let g = 0
  let b = 0
  let n = 0
  for (let dy = -radius; dy <= radius; dy++) {
    for (let dx = -radius; dx <= radius; dx++) {
      const px = x + dx
      const py = y + dy
      if (px < 0 || py < 0 || px >= frame.width || py >= frame.height) continue
      const i = (py * frame.width + px) * 4
      r += frame.rgba[i]!
      g += frame.rgba[i + 1]!
      b += frame.rgba[i + 2]!
      n++
    }
  }
  assert.ok(n > 0, `the screen's centre (${x},${y}) is outside the ${frame.width}x${frame.height} frame`)
  return { r: r / n, g: g / n, b: b / n }
}

/** Red clearly dominates: tolerant of yuv420 rounding and gamma, strict enough to reject black,
 *  the sky and the placeholder, as unity-explorer's VideoProbe.IsDominantChannel. */
const isRed = ({ r, g, b }: { r: number; g: number; b: number }): boolean => r > 90 && r > g + 50 && r > b + 50

scenario(
  'video plays through the engine player',
  async ({ explorer, t, report }) => {
    const [x, y] = VIDEO_SCENE.base
    await explorer.teleport(x, y)
    const screens = await until(() => explorer.sceneEntities('VideoPlayer'), (entities) => entities.length > 0, {
      timeoutMs: SCENE_LIVE_TIMEOUT_MS,
      intervalMs: 1000,
      what: `the scene ${VIDEO_SCENE.id} at ${x},${y} to create its video player`
    })
    assert.equal(screens.length, 1, `one screen expected, got ${screens.join(', ')}`)
    const screen = screens[0]!

    // The state: a Playing event, and no Error event.
    const events = await until(() => explorer.inspect<VideoEvent[]>(screen, 'VideoEvent'), (events) => events.some((e) => e.state === VS.PLAYING || e.state === VS.ERROR), {
      timeoutMs: PLAYING_TIMEOUT_MS,
      intervalMs: 1000,
      what: 'the video to reach the playing state'
    })
    t.diagnostic(`events: ${describe(events)}`)
    assert.ok(!events.some((e) => e.state === VS.ERROR), `the player reported an error: ${describe(events)}`)

    // The clock: a reported offset past the mark.
    const ran = await until(() => explorer.inspect<VideoEvent[]>(screen, 'VideoEvent'), (events) => maxOffset(events) >= CLOCK_RUN_S, {
      timeoutMs: CLOCK_TIMEOUT_MS,
      intervalMs: 500,
      what: `the video clock to pass ${CLOCK_RUN_S} s`
    })
    assert.ok(!ran.some((e) => e.state === VS.ERROR), `the player reported an error while playing: ${describe(ran)}`)

    // The frame: the screen's centre shows the fixture's solid red.
    await sleep(1000)
    const at = await explorer.entityScreenPosition(screen)
    assert.ok(at.onScreen && at.inFront, `the screen is not in view: ${JSON.stringify(at)}`)
    const png = await explorer.screenshot()
    await report.write({ 'frame.png': png })
    const colour = sample(decodePng(png), at)
    t.diagnostic(`screen centre at window ${at.x},${at.y}: rgb(${colour.r.toFixed(0)}, ${colour.g.toFixed(0)}, ${colour.b.toFixed(0)})`)
    assert.ok(isRed(colour), `expected the fixture's red at the screen's centre, sampled rgb(${colour.r.toFixed(0)}, ${colour.g.toFixed(0)}, ${colour.b.toFixed(0)}); see frame.png in ${report.dir}`)
  },
  { session: SESSION, timeoutMs: SCENARIO_TIMEOUT_MS }
)
