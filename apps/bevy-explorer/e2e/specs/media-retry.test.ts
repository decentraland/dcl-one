// Media retry: a scene's VideoPlayer whose source fails is reopened 2, 4, 8, 16 and 32 s after each
// failure, reports `VsLoading` meanwhile, and reports `VsError` once the fifth reopen fails too
// (`rendering/bevy-explorer/docs/guidelines/decisions.md`, "Media reopens on the web").
//
// | Scenario                                                        | Session                      | Needs                      |
// | a video whose source fails twice plays after two reopens        | guest, on the loopback realm | the scene built; web       |
// | a video whose source always fails gives up after five reopens   | guest, on the loopback realm | the scene built; web       |
// | a paused video whose source fails reports VS_ERROR and plays after play | guest, on the loopback realm | the scene built; web |
//
// Each scenario boots a guest on a loopback realm of its own (`driver/realm.ts`) that serves
// `scenes/video` and answers 503 to the first GETs of its media files: `recover.webm` twice,
// `broken.webm` every time, `paused.webm` once. It sets `/media_retry_delay 100`, points the scene commands at the
// scene with `/set_scene`, and reads the scene's `video <name> state=<VideoState>` lines through
// `/scene_logs` and each player's reopen state through `/media_state`; the failure checks use
// `retryFailures`, which does not depend on when the delay was set. The second scenario walks to
// `broken` once `broken` has failed its first open, while it is not yet the nearest video (one
// video plays at a time), so that failure reports `VS_ERROR` and the play after the walk
// `VS_LOADING`. The third walks to `paused`, which the scene sets playing 1 s after it reports
// `VS_ERROR`. Each scenario keeps its realm's requests in its report (`realm-requests.jsonl`)
// and closes its client. Web only: on the standalone all three are todos.
// Build the scene first: `pnpm run build-scenes video`.
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { after } from 'node:test'
import { buildScene, contentHash, readTarget, serveRealm, spec, until, type BuiltScene, type Explorer, type MediaPlayerRow, type RealmServer, type ScenarioOptions } from '../driver/index.ts'
import type { ScenarioReport } from '../driver/report.ts'
import { SCENES_DIR } from '../driver/paths.ts'

const scenario = spec(import.meta.url)

const SCENE_DIR = path.join(SCENES_DIR, 'video')
const SCENE_TITLE = 'video'
const RECOVER_FAILURES = 2
const PAUSED_FAILURES = 1
const MAX_REOPENS = 5
const FIRST_DELAY_MS = 100
/** The longest wait: a first failure before the delay is shortened (2 s), the shortened rest
 *  (3.0 s), the scene's boot and each load, with room for a slow runner. */
const GIVE_UP_TIMEOUT_MS = 60_000
/** The wait for the reopen count to restart once the recover screen plays. */
const RESTART_TIMEOUT_MS = 20_000
/** The scene's base parcel, `0,26`, in DCL world metres. */
const BASE = { x: 0, z: 26 * 16 }

const native = readTarget().transport === 'native'
const webOnly = native ? { todo: 'native: FFmpeg reopens no failed source' } : {}

// A scene that is not built fails these scenarios, not the whole run.
let scene: BuiltScene | undefined
let sceneError: Error | undefined
try {
  scene = await buildScene(SCENE_DIR)
} catch (error) {
  const message = error instanceof Error ? error.message : String(error)
  sceneError = new Error(`the video scene is not ready (${message}); run \`pnpm run build-scenes video\` in apps/bevy-explorer/e2e`, { cause: error })
}

const recoverHash = contentHash(await readFile(path.join(SCENE_DIR, 'assets', 'recover.webm')))
const brokenHash = contentHash(await readFile(path.join(SCENE_DIR, 'assets', 'broken.webm')))
const pausedHash = contentHash(await readFile(path.join(SCENE_DIR, 'assets', 'paused.webm')))

/** A realm, with failure counts of its own, and a guest session on it for one scenario, on an
 *  ephemeral port. */
async function realmFor(name: string): Promise<{ server: RealmServer; session: ScenarioOptions['session'] }> {
  const server = await serveRealm({
    scenes: scene ? [scene] : [],
    port: 0,
    realmName: `e2e-media-retry-${name}`,
    contentFailures: new Map([
      [recoverHash, RECOVER_FAILURES],
      [brokenHash, Infinity],
      [pausedHash, PAUSED_FAILURES]
    ])
  })
  after(() => server.close())
  return { server, session: { realm: server.realm, position: '0,26', signIn: 'guest' } }
}

const RECOVER = await realmFor('recover')
const BROKEN = await realmFor('broken')
const PAUSED = await realmFor('paused')
/** A scenario's budget: `prepare` (the scene's boot), its main wait and the rest. */
const SCENARIO_TIMEOUT_MS = 2 * GIVE_UP_TIMEOUT_MS + RESTART_TIMEOUT_MS + 30_000

async function prepare(explorer: Explorer, report: ScenarioReport, server: RealmServer): Promise<void> {
  report.attach('realm-requests.jsonl', () => server.requests.map((request) => `${JSON.stringify(request)}\n`).join(''))
  if (sceneError) throw sceneError
  const published = new Set(scene!.entity.content.map((entry) => entry.hash))
  assert.ok(published.has(recoverHash) && published.has(brokenHash) && published.has(pausedHash), 'the realm does not publish the scene\'s media files')
  await explorer.mediaRetryDelay(FIRST_DELAY_MS)
  await until(() => explorer.liveScenes(), (live) => live.some((live) => live.title === SCENE_TITLE), {
    timeoutMs: GIVE_UP_TIMEOUT_MS,
    intervalMs: 1000,
    what: `/live_scenes to list "${SCENE_TITLE}"`
  })
  await explorer.setScene(SCENE_TITLE)
}

const stateLines = (logs: string[], name: string): string[] => logs.filter((line) => line.includes(`video ${name} state=`))

async function player(explorer: Explorer, source: string): Promise<MediaPlayerRow> {
  const row = (await explorer.mediaState()).players.find((row) => row.source === source)
  assert.ok(row, `/media_state lists no player for ${source}`)
  return row
}

scenario(
  'a video whose source fails twice plays after two reopens',
  async ({ explorer, report, restore }) => {
    restore(() => explorer.setScene())
    await prepare(explorer, report, RECOVER.server)
    // checks the outcome only: a VS_ERROR logged before VS_PLAYING is allowed
    await until(() => explorer.sceneLogs(100), (lines) => stateLines(lines, 'recover').some((line) => line.endsWith('state=VS_PLAYING')), {
      timeoutMs: GIVE_UP_TIMEOUT_MS,
      what: 'the recover screen to log VS_PLAYING'
    })
    // the count restarts only after 5 s of playback, not when a reopen opens
    const playing = await player(explorer, 'assets/recover.webm')
    assert.equal(playing.retryAttempt, RECOVER_FAILURES, `the recover screen's count was ${playing.retryAttempt} once it played`)
    const row = await until(() => player(explorer, 'assets/recover.webm'), (row) => row.retryAttempt === 0, {
      timeoutMs: RESTART_TIMEOUT_MS,
      what: 'the recover screen\'s reopen count to restart after playing'
    })
    assert.equal(row.retryFailures, RECOVER_FAILURES, `the recover screen failed ${row.retryFailures} times`)
    const gets = RECOVER.server.contentGets.get(recoverHash) ?? 0
    assert.ok(gets > RECOVER_FAILURES, `recover.webm was requested ${gets} times, not after each failure`)
    assert.equal(row.retryGaveUp, false)
    assert.notEqual(row.lastError, null, 'the failures left no last error')
  },
  { ...webOnly, session: RECOVER.session, closeSession: true, timeoutMs: SCENARIO_TIMEOUT_MS }
)

scenario(
  'a video whose source always fails gives up after five reopens',
  async ({ explorer, report, restore }) => {
    restore(() => explorer.setScene())
    await prepare(explorer, report, BROKEN.server)
    // broken's first failure, while it is not the nearest video
    await until(() => explorer.mediaState(), (state) => (state.players.find((row) => row.source === 'assets/broken.webm')?.retryFailures ?? 0) >= 1, {
      timeoutMs: GIVE_UP_TIMEOUT_MS,
      what: 'the broken screen\'s first failure'
    })
    await explorer.walkTo(BASE.x + 11, 0, BASE.z + 12)
    const row = await until(() => player(explorer, 'assets/broken.webm'), (row) => row.retryGaveUp === true && row.lastPlaybackState === 'VS_ERROR', {
      timeoutMs: GIVE_UP_TIMEOUT_MS,
      what: 'the broken screen to give up'
    })
    assert.equal(row.retryFailures, 1 + MAX_REOPENS, `the broken screen failed ${row.retryFailures} times, not one open and five reopens`)
    const gets = BROKEN.server.contentGets.get(brokenHash) ?? 0
    assert.ok(gets >= 1 + MAX_REOPENS, `broken.webm was requested ${gets} times, fewer than one open and five reopens`)
    // waits for the logged give-up, then checks for a VS_LOADING after the first VS_ERROR
    const logs = await until(() => explorer.sceneLogs(200), (lines) => stateLines(lines, 'broken').at(-1)?.endsWith('state=VS_ERROR') ?? false, {
      timeoutMs: 10_000,
      what: 'the broken screen to log its give-up'
    })
    const states = stateLines(logs, 'broken')
    const firstErrorAt = states.findIndex((line) => line.endsWith('state=VS_ERROR'))
    assert.ok(states.slice(firstErrorAt + 1).some((line) => line.endsWith('state=VS_LOADING')), `the broken screen did not report VS_LOADING after a VS_ERROR before giving up:\n${states.join('\n')}`)
    assert.equal(row.retryAttempt, MAX_REOPENS)
    assert.equal(row.retryDelaySeconds, null, 'a reopen is still scheduled after giving up')
    assert.equal(row.lastPlaybackState, 'VS_ERROR')
  },
  { ...webOnly, session: BROKEN.session, closeSession: true, timeoutMs: SCENARIO_TIMEOUT_MS }
)

scenario(
  'a paused video whose source fails reports VS_ERROR and plays after play',
  async ({ explorer, report, restore }) => {
    restore(() => explorer.setScene())
    await prepare(explorer, report, PAUSED.server)
    await explorer.walkTo(BASE.x + 2, 0, BASE.z + 12)
    const logs = await until(() => explorer.sceneLogs(200), (lines) => stateLines(lines, 'paused').some((line) => line.endsWith('state=VS_PLAYING')), {
      timeoutMs: GIVE_UP_TIMEOUT_MS,
      what: 'the paused screen to log VS_PLAYING'
    })
    const states = stateLines(logs, 'paused')
    const errorAt = states.findIndex((line) => line.endsWith('state=VS_ERROR'))
    const playingAt = states.findIndex((line) => line.endsWith('state=VS_PLAYING'))
    assert.ok(errorAt >= 0 && errorAt < playingAt, `the paused screen did not report VS_ERROR before VS_PLAYING:\n${states.join('\n')}`)
    assert.ok(logs.some((line) => line.includes('video paused play')), 'the scene never set the paused screen playing')
    const row = await player(explorer, 'assets/paused.webm')
    assert.equal(row.retryFailures, PAUSED_FAILURES, `the paused screen failed ${row.retryFailures} times`)
    assert.equal(row.retryGaveUp, false)
    const gets = PAUSED.server.contentGets.get(pausedHash) ?? 0
    assert.ok(gets > PAUSED_FAILURES, `paused.webm was requested ${gets} times, not reopened after its failure`)
  },
  { ...webOnly, session: PAUSED.session, closeSession: true, timeoutMs: SCENARIO_TIMEOUT_MS }
)
