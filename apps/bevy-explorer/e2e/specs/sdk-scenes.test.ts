// QA checklist `SDK76Minigames` (30): popular Genesis City scenes, SDK7 and SDK6, and the
// minigames, each opened by its parcel.
//
// | Checklist case               | Parcel   | SDK | Class   |
// | Genesis Plaza                | 0,0      | 7   | Partial |
// | Meta gamimall                | 1,95     | 7   | Partial |
// | Wondermine                   | -29,55   | 7   | Partial |
// | Dhingia Club / Unmonday      | 14,-135  | 7   | Partial |
// | DAO HQ                       | 65,13    | 7   | Partial |
// | Casa Roustan                 | 37,-114  | 7   | Partial |
// | The Makumba Social Stage     | 84,40    | 7   | Partial |
// | RustyBucket Street           | 119,80   | 6   | Partial |
// | Apollo NFT Gallery           | 87,-28   | 6   | Partial |
// | EscapeRank                   | 85,-139  | 6   | Partial |
// | CGH Metaverse Gallery        | 35,84    | 6   | Partial |
// | Game of Life                 | 30,-22   | 6   | Partial |
// | Tower of Hanoi               | 61,-22   | 7   | Partial |
// | Car Park Puzzle              | 149,30   | 7   | Partial |
// | Color Pop                    | -121,-60 | 7   | Partial |
// | Chemical Clash               | 139,-20  | 7   | Partial |
// | Gears & Glitches             | -138,-94 | 7   | Partial |
// | Liquid Labyrinth             | -139,-28 | 7   | Partial |
// | Math Kittens                 | -85,-97  | 7   | Partial |
// | Memory Circus                | 50,-88   | 7   | Partial |
// | Memory Grid                  | 81,-30   | 7   | Partial |
// | Minesweeper                  | -30,3    | 7   | Partial |
// | Mosaic Mania                 | -121,-59 | 7   | Partial |
// | Pixel Snake                  | -126,-73 | 7   | Partial |
// | Recall Rush                  | 36,86    | 7   | Partial |
// | Salad Slash                  | 17,-114  | 7   | Partial |
// | Sliding Puzzle               | 79,-30   | 7   | Partial |
// | Tile Tally                   | 43,100   | 7   | Partial |
// | Whack-a-Frog                 | -109,-97 | 7   | Partial |
// | Word Search                  | -75,-46  | 7   | Partial |
//
// Partial, every one: a load smoke, not the game. The checklist plays each scene by hand; here
// the player teleports to the parcel, its scene reaches `Running` in `/scene_loading_state`
// (never `Broken` or `Failed`), it keeps running for a few seconds, and its log (`/scene_logs`)
// has no `SystemError`, the runtime's own failure (a script that threw out of its main loop, a
// worker that died). A scene's own `console.error` is a `SceneError`: listed as a diagnostic,
// not failed, as scenes log their own fetch and asset trouble. The scene's name, SDK and parcel
// count (`/current_scene_state`) are diagnostics too, and the final frame is the report's
// `final.png`.
//
// These scenes are deployed on mainnet only: decentraland.zone's catalyst has none but Genesis
// Plaza. So these scenarios run on a client of their own, booted into `DCL_E2E_GENESIS_REALM`
// (`https://peer.decentraland.org` by default) with the run's account, and the run's default
// client stays on its target. `specs/scene-media.test.ts` shares that client; the last scene
// closes it.
import assert from 'node:assert/strict'
import { sleep, spec, until, type Explorer, type ScenarioOptions } from '../driver/index.ts'

const scenario = spec(import.meta.url)

/** The mainnet realm the checklist's scenes are deployed in. */
const GENESIS_REALM = process.env.DCL_E2E_GENESIS_REALM || 'https://peer.decentraland.org'
const GENESIS_SESSION: ScenarioOptions['session'] = { realm: GENESIS_REALM, position: '0,0' }

/** A teleport across Genesis City loads the destination's scenes before the arrival. */
const ARRIVAL_TIMEOUT_MS = 90_000
/** How long a scene has, after the arrival, to be running. */
const LOAD_TIMEOUT_MS = 60_000
/** How long a running scene is left to run before its log is read. */
const SETTLE_MS = 5_000
const SCENARIO_TIMEOUT_MS = 240_000

/** The stages `/scene_loading_state` ends a load in; `Blocked` may lift, so it is waited out. */
const SETTLED = new Set(['Running', 'Broken', 'Failed'])

interface LoadingState {
  stage: string
  running: number
  broken: number
  failed: number
}

interface SceneState {
  name?: string
  sdk?: string
  parcels?: number
  unavailable?: Record<string, string>
}

interface ChecklistScene {
  name: string
  parcel: [number, number]
}

const SCENES: ChecklistScene[] = [
  { name: 'Genesis Plaza', parcel: [0, 0] },
  { name: 'Meta gamimall', parcel: [1, 95] },
  { name: 'Wondermine', parcel: [-29, 55] },
  { name: 'Dhingia Club | Unmonday', parcel: [14, -135] },
  { name: 'DAO HQ', parcel: [65, 13] },
  { name: 'Casa Roustan', parcel: [37, -114] },
  { name: 'The Makumba Social Stage', parcel: [84, 40] },
  { name: 'RustyBucket Street', parcel: [119, 80] },
  { name: 'Apollo NFT Gallery', parcel: [87, -28] },
  { name: 'EscapeRank', parcel: [85, -139] },
  { name: 'CGH Metaverse Gallery', parcel: [35, 84] },
  { name: 'Game of Life', parcel: [30, -22] },
  { name: 'Tower of Hanoi', parcel: [61, -22] },
  { name: 'Car Park Puzzle', parcel: [149, 30] },
  { name: 'Color Pop', parcel: [-121, -60] },
  { name: 'Chemical Clash', parcel: [139, -20] },
  { name: 'Gears & Glitches', parcel: [-138, -94] },
  { name: 'Liquid Labyrinth', parcel: [-139, -28] },
  { name: 'Math Kittens', parcel: [-85, -97] },
  { name: 'Memory Circus', parcel: [50, -88] },
  { name: 'Memory Grid', parcel: [81, -30] },
  { name: 'Minesweeper', parcel: [-30, 3] },
  { name: 'Mosaic Mania', parcel: [-121, -59] },
  { name: 'Pixel Snake', parcel: [-126, -73] },
  { name: 'Recall Rush', parcel: [36, 86] },
  { name: 'Salad Slash', parcel: [17, -114] },
  { name: 'Sliding Puzzle', parcel: [79, -30] },
  { name: 'Tile Tally', parcel: [43, 100] },
  { name: 'Whack-a-Frog', parcel: [-109, -97] },
  { name: 'Word Search', parcel: [-75, -46] }
]

async function loadingState(explorer: Explorer): Promise<LoadingState> {
  return JSON.parse(await explorer.run('/scene_loading_state')) as LoadingState
}

/** A `/scene_logs` line: `[12.34] SystemError: message`. */
function level(line: string): string | undefined {
  return /^\[[\d.]+\] (\w+): /.exec(line)?.[1]
}

const scenarioOptions = (last: boolean): ScenarioOptions => ({ session: GENESIS_SESSION, timeoutMs: SCENARIO_TIMEOUT_MS, closeSession: last })

for (const [index, scene] of SCENES.entries()) {
  const [x, y] = scene.parcel
  scenario(
    `${scene.name} (${x},${y}) loads and runs with no runtime error`,
    async ({ explorer, t }) => {
      await explorer.teleport(x, y, { timeoutMs: ARRIVAL_TIMEOUT_MS })
      const loaded = await until(() => loadingState(explorer), (state) => SETTLED.has(state.stage), {
        timeoutMs: LOAD_TIMEOUT_MS,
        intervalMs: 1000,
        what: `the scene at ${x},${y} to run (/scene_loading_state stage Running)`
      })
      assert.equal(loaded.stage, 'Running', `the scene at ${x},${y} should run, not end ${loaded.stage}`)
      const info = JSON.parse(await explorer.run('/current_scene_state')) as SceneState
      t.diagnostic(`scene "${info.name ?? '?'}", sdk ${info.sdk ?? '?'}, ${info.parcels ?? '?'} parcels`)

      await sleep(SETTLE_MS)
      const after = await loadingState(explorer)
      assert.equal(after.stage, 'Running', `the scene at ${x},${y} should still run after ${SETTLE_MS} ms, not be ${after.stage}`)

      const logs = await explorer.sceneLogs(200)
      const sceneErrors = logs.filter((line) => level(line) === 'SceneError')
      const systemErrors = logs.filter((line) => level(line) === 'SystemError')
      for (const line of sceneErrors.slice(-5)) t.diagnostic(`scene error: ${line.slice(0, 300)}`)
      if (sceneErrors.length > 5) t.diagnostic(`${sceneErrors.length} scene errors in all`)
      assert.deepEqual(systemErrors.map((line) => line.slice(0, 300)), [], `the scene at ${x},${y} should log no runtime error`)
    },
    scenarioOptions(index === SCENES.length - 1)
  )
}
