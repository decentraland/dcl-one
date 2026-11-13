// Media retry: a scene's VideoPlayer whose source fails is reopened 2, 4, 8, 16 and 32 s after each
// failure, reports `VsLoading` meanwhile, and reports `VsError` once the fifth reopen fails too
// (`rendering/bevy-explorer/docs/guidelines/decisions.md`, "Media reopens on the web").
// explorer-automation has no such fixture, so nothing here is a port and no Unity test is mapped.
//
// | Scenario                                                        | Session                      | Needs                      |
// | a video whose source fails twice plays after two reopens        | guest, on the loopback realm | the scene built; web       |
// | a video whose source always fails gives up after five reopens   | guest, on the loopback realm | the scene built; web       |
//
// The loopback realm (`driver/realm.ts`) serves `scenes/video` at its own parcel and answers 503
// to the first GETs of its media files: `recover.webm` twice, `broken.webm` every time. The scene
// logs `video <name> state=<VideoState>` on every reported change, read through `/scene_logs`
// after `/set_scene` points the scene commands at it, and `/media_state` gives each player's
// reopen count and last error. `/media_retry_delay 100` shortens the schedule to 0.1 to 1.6 s for
// every source failing from then on; a failure before the first scenario issues it waits its
// 2 s. Only the web backend reopens (native FFmpeg ends at its first error), so on the standalone
// both are todos. Build the scene first: `npm run build-scenes -- video`.
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { after } from 'node:test'
import { buildScene, contentHash, readTarget, serveRealm, spec, until, type BuiltScene, type Explorer, type MediaPlayerRow, type ScenarioOptions } from '../driver/index.ts'
import { SCENES_DIR } from '../driver/paths.ts'

const scenario = spec(import.meta.url)

const SCENE_DIR = path.join(SCENES_DIR, 'video')
const SCENE_TITLE = 'video'
const RECOVER_FAILURES = 2
const MAX_REOPENS = 5
const FIRST_DELAY_MS = 100
/** The longest wait: a first failure before the delay is shortened (2 s), the shortened rest
 *  (3.1 s), the scene's boot and each load, with room for a slow runner. */
const GIVE_UP_TIMEOUT_MS = 60_000

const native = readTarget().transport === 'native'
const webOnly = native ? { todo: 'native: FFmpeg reopens no failed source' } : {}

// A scene that is not built fails these scenarios, not the whole run.
let scene: BuiltScene | undefined
let sceneError: Error | undefined
try {
  scene = await buildScene(SCENE_DIR)
} catch (error) {
  const message = error instanceof Error ? error.message : String(error)
  sceneError = new Error(`the video scene is not ready (${message}); run \`npm run build-scenes -- video\` in apps/bevy-explorer/e2e`, { cause: error })
}

const recoverHash = contentHash(await readFile(path.join(SCENE_DIR, 'media', 'recover.webm')))
const brokenHash = contentHash(await readFile(path.join(SCENE_DIR, 'media', 'broken.webm')))

// An ephemeral port: the visual realm holds the configured one in the same process.
const server = await serveRealm({
  scenes: scene ? [scene] : [],
  port: 0,
  realmName: 'e2e-media-retry',
  contentFailures: new Map([
    [recoverHash, RECOVER_FAILURES],
    [brokenHash, Infinity]
  ])
})
after(() => server.close())

const SESSION: ScenarioOptions['session'] = { realm: server.realm, position: '0,24', signIn: 'guest' }

async function prepare(explorer: Explorer): Promise<void> {
  if (sceneError) throw sceneError
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
  async ({ explorer, restore }) => {
    restore(async () => {
      await explorer.setScene()
      await explorer.mediaRetryDelay(2000)
    })
    await prepare(explorer)
    const logs = await until(() => explorer.sceneLogs(100), (lines) => stateLines(lines, 'recover').some((line) => line.endsWith('state=VS_PLAYING')), {
      timeoutMs: GIVE_UP_TIMEOUT_MS,
      what: 'the recover screen to log VS_PLAYING'
    })
    assert.ok(!stateLines(logs, 'recover').some((line) => line.endsWith('state=VS_ERROR')), `the recover screen reported VS_ERROR while reopening:\n${stateLines(logs, 'recover').join('\n')}`)
    assert.ok((server.contentGets.get(recoverHash) ?? 0) > RECOVER_FAILURES, `recover.webm was requested ${server.contentGets.get(recoverHash) ?? 0} times`)
    const row = await player(explorer, 'media/recover.webm')
    assert.equal(row.retryAttempt, 0, 'the reopen count did not restart once the source opened')
    assert.equal(row.retryGaveUp, false)
    assert.notEqual(row.lastError, null, 'the failures left no last error')
  },
  { ...webOnly, session: SESSION, timeoutMs: GIVE_UP_TIMEOUT_MS + 30_000 }
)

scenario(
  'a video whose source always fails gives up after five reopens',
  async ({ explorer, restore }) => {
    restore(async () => {
      await explorer.setScene()
      await explorer.mediaRetryDelay(2000)
    })
    await prepare(explorer)
    await until(() => explorer.sceneLogs(100), (lines) => stateLines(lines, 'broken').some((line) => line.endsWith('state=VS_ERROR')), {
      timeoutMs: GIVE_UP_TIMEOUT_MS,
      what: 'the broken screen to log VS_ERROR'
    })
    assert.equal(server.contentGets.get(brokenHash), 1 + MAX_REOPENS, 'broken.webm was not opened once and reopened five times')
    const row = await player(explorer, 'media/broken.webm')
    assert.equal(row.retryGaveUp, true)
    assert.equal(row.retryAttempt, MAX_REOPENS)
    assert.equal(row.retryDelaySeconds, null, 'a reopen is still scheduled after giving up')
    assert.equal(row.lastPlaybackState, 'VS_ERROR')
  },
  { ...webOnly, session: SESSION, timeoutMs: GIVE_UP_TIMEOUT_MS + 30_000 }
)
