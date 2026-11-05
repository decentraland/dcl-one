// unity-explorer's LivekitPlayer tests (Explorer/Assets/DCL/SDKComponents/MediaStream/Tests/
// LivekitPlayerShould.cs), as far as a scene can show them: the `livekit` scene (`scenes/livekit`)
// has a current-stream screen, a screen pinned to whichever `pinned-*` participant the scene API
// lists, and a screen with a malformed address.
//
// | Unity test | Scenario | Class |
// | --- | --- | --- |
// | Invalid or unknown stream targets do not throw | livekit addresses resolve without a room | Port: a malformed address errors, an unknown participant loads without an error |
// | Pinned user stream plays its participant's track | a pinned participant's track plays | Port, opt-in: needs livekit-server and lk (scripts/livekit-room.sh) |
// | Pinned participant leaves: fallback to the first available | a pinned participant's track plays | Port: the publisher is stopped and the screen reports Loading |
// | Participant returns: the stream is reopened | a pinned participant's track plays | Partial: the republished track has a new sid, so the scene re-reads the list and re-sets the source |
// | Screenshare switching, presentation bot, reconnect release and reopen | | Gap: one publisher, no bot, no reconnect in the harness |
//
// States are read from each screen's `VideoEvent`s (`/inspect_component <entity> VideoEvent`);
// the picture from `/screenshot` at the pinned screen's centre.
import assert from 'node:assert/strict'
import { spawn, type ChildProcess } from 'node:child_process'
import { decodePng, sleep, spec, until, type Image, type ScenarioOptions, type ScreenPosition } from '../../driver/index.ts'
import { readTarget } from '../../driver/target.ts'
import { visualRealm } from './realm.ts'
import { LIVEKIT_SCENE } from './scenes.ts'

const scenario = spec(import.meta.url)
const server = await visualRealm()

const SESSION: ScenarioOptions['session'] = { realm: server.realm, position: '0,0', signIn: 'guest', windowSize: [960, 540] }
const SCENARIO_TIMEOUT_MS = 600_000
const SCENE_LIVE_TIMEOUT_MS = 60_000
const STATE_TIMEOUT_MS = 30_000
const ROOM_TIMEOUT_MS = 60_000

/** `VideoState` (library/protocol/proto/decentraland/sdk/components/video_event.proto). */
const VS = { ERROR: 1, LOADING: 2, PLAYING: 4 } as const

interface VideoEvent {
  timestamp: number
  tickNumber: number
  state: number
}

const describe = (events: VideoEvent[]): string => events.map((e) => `tick ${e.tickNumber}: state ${e.state}`).join('; ')
const has = (events: VideoEvent[], state: number): boolean => events.some((e) => e.state === state)

/** The scene's three screens, by the src each was created with, in creation order. */
async function screens(explorer: { sceneEntities: (c?: string) => Promise<string[]>; inspect: <T>(e: string, c: string) => Promise<T> }) {
  const players = await explorer.sceneEntities('VideoPlayer')
  const bySrc = new Map<string, string>()
  for (const entity of players) {
    const player = await explorer.inspect<{ src: string }>(entity, 'VideoPlayer')
    bySrc.set(player.src.startsWith('livekit-video://current-stream') ? 'current' : player.src === 'livekit-video://broken' ? 'invalid' : 'pinned', entity)
  }
  return bySrc
}

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
  assert.ok(n > 0, 'the sample is outside the frame')
  return { r: r / n, g: g / n, b: b / n }
}

const isPlaceholder = ({ r, g, b }: { r: number; g: number; b: number }): boolean => r > 200 && g < 60 && b > 200
const isBlack = ({ r, g, b }: { r: number; g: number; b: number }): boolean => r < 20 && g < 20 && b < 20

scenario(
  'livekit addresses resolve without a room',
  async ({ explorer, t }) => {
    const [x, y] = LIVEKIT_SCENE.base
    await explorer.teleport(x, y)
    await until(() => explorer.sceneEntities('VideoPlayer'), (entities) => entities.length === 3, {
      timeoutMs: SCENE_LIVE_TIMEOUT_MS,
      intervalMs: 1000,
      what: `the scene ${LIVEKIT_SCENE.id} at ${x},${y} to create its three screens`
    })
    const byName = await screens(explorer)
    const invalid = byName.get('invalid')!
    const pinned = byName.get('pinned')!
    const current = byName.get('current')!

    const invalidEvents = await until(() => explorer.inspect<VideoEvent[]>(invalid, 'VideoEvent'), (events) => has(events, VS.ERROR), {
      timeoutMs: STATE_TIMEOUT_MS,
      intervalMs: 500,
      what: 'the malformed address to report an error'
    })
    t.diagnostic(`invalid: ${describe(invalidEvents)}`)

    // An unknown participant and no room are not errors: both players wait in Loading.
    await sleep(5000)
    for (const [name, entity] of [['pinned', pinned], ['current', current]] as const) {
      const events = await explorer.inspect<VideoEvent[]>(entity, 'VideoEvent')
      t.diagnostic(`${name}: ${describe(events)}`)
      assert.ok(has(events, VS.LOADING), `${name} reached Loading: ${describe(events)}`)
      assert.ok(!has(events, VS.ERROR), `${name} reported no error: ${describe(events)}`)
    }
  },
  { session: SESSION, timeoutMs: SCENARIO_TIMEOUT_MS }
)

/** `lk room join` publishing the CLI's demo video and audio as `identity`. */
function publish(url: string, room: string, identity: string): ChildProcess {
  return spawn('lk', ['room', 'join', '--url', url, '--api-key', 'devkey', '--api-secret', 'secret', '--room', room, '--identity', identity, '--publish-demo'], { stdio: ['ignore', 'ignore', 'inherit'] })
}

function stop(child: ChildProcess): Promise<void> {
  return new Promise((resolve) => {
    if (child.exitCode !== null) return resolve()
    child.once('exit', () => resolve())
    child.kill('SIGINT')
  })
}

scenario(
  "a pinned participant's track plays",
  async ({ explorer, t, report }) => {
    const url = process.env.DCL_E2E_LIVEKIT_URL
    const room = process.env.DCL_E2E_LIVEKIT_ROOM ?? 'e2e'
    if (readTarget().transport !== 'native' || !url) {
      t.skip('needs a local LiveKit room on the standalone: run scripts/livekit-room.sh')
      return
    }
    const [x, y] = LIVEKIT_SCENE.base
    await explorer.teleport(x, y)
    await until(() => explorer.sceneEntities('VideoPlayer'), (entities) => entities.length === 3, {
      timeoutMs: SCENE_LIVE_TIMEOUT_MS,
      intervalMs: 1000,
      what: 'the scene to create its three screens'
    })

    const publisher = publish(url, room, 'pinned-a')
    try {
      // The scene re-reads the list and points its pinned screen at pinned-a's track.
      const { entity: pinned } = await until(
        async () => {
          const entity = (await screens(explorer)).get('pinned')!
          const player = await explorer.inspect<{ src: string }>(entity, 'VideoPlayer')
          return { entity, src: player.src }
        },
        ({ src }) => src.startsWith('livekit-video://pinned-a/'),
        { timeoutMs: ROOM_TIMEOUT_MS, intervalMs: 1000, what: "the scene to list pinned-a's track and pin it" }
      )
      const playing = await until(() => explorer.inspect<VideoEvent[]>(pinned, 'VideoEvent'), (events) => has(events, VS.PLAYING) || has(events, VS.ERROR), {
        timeoutMs: ROOM_TIMEOUT_MS,
        intervalMs: 500,
        what: "pinned-a's track to play"
      })
      t.diagnostic(`pinned: ${describe(playing)}`)
      assert.ok(!has(playing, VS.ERROR), `the pinned player reported an error: ${describe(playing)}`)

      // The picture: neither the placeholder nor black, and moving.
      await sleep(1000)
      const at = await explorer.entityScreenPosition(pinned)
      assert.ok(at.onScreen && at.inFront, `the pinned screen is not in view: ${JSON.stringify(at)}`)
      const first = await explorer.screenshot()
      await report.write({ 'pinned.png': first })
      const colour = sample(decodePng(first), at)
      t.diagnostic(`pinned screen centre: rgb(${colour.r.toFixed(0)}, ${colour.g.toFixed(0)}, ${colour.b.toFixed(0)})`)
      assert.ok(!isPlaceholder(colour), 'the pinned screen left the placeholder')
      assert.ok(!isBlack(colour), 'the pinned screen is not black')
      await sleep(1000)
      const later = sample(decodePng(await explorer.screenshot()), at)
      assert.ok(Math.abs(later.r - colour.r) + Math.abs(later.g - colour.g) + Math.abs(later.b - colour.b) > 3, 'the demo video moves between two samples')

      // The publisher leaves: the pinned screen falls back and reports Loading.
      const before = (await explorer.inspect<VideoEvent[]>(pinned, 'VideoEvent')).length
      await stop(publisher)
      const fallen = await until(() => explorer.inspect<VideoEvent[]>(pinned, 'VideoEvent'), (events) => events.length > before && events.at(-1)?.state === VS.LOADING, {
        timeoutMs: STATE_TIMEOUT_MS,
        intervalMs: 500,
        what: 'the pinned screen to report Loading after its participant left'
      })
      t.diagnostic(`after leaving: ${describe(fallen)}`)
      await report.write({ 'fallback.png': await explorer.screenshot() })
    } finally {
      await stop(publisher)
    }

    // The publisher returns with a new sid: the scene re-sets the source and it plays again.
    const again = publish(url, room, 'pinned-a')
    try {
      const pinned = (await screens(explorer)).get('pinned')!
      const marker = (await explorer.inspect<VideoEvent[]>(pinned, 'VideoEvent')).length
      const replayed = await until(() => explorer.inspect<VideoEvent[]>(pinned, 'VideoEvent'), (events) => events.length > marker && events.at(-1)?.state === VS.PLAYING, {
        timeoutMs: ROOM_TIMEOUT_MS,
        intervalMs: 500,
        what: 'the pinned screen to play the republished track'
      })
      t.diagnostic(`after returning: ${describe(replayed)}`)
    } finally {
      await stop(again)
    }
  },
  { session: SESSION, timeoutMs: SCENARIO_TIMEOUT_MS }
)
