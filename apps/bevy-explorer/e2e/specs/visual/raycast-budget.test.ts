// The per-frame raycast budget (`/raycast_budget`, `/raycast_stats`) on the `raycast-storm`
// scene (`scenes/raycast-storm`), whose 200 continuous raycasts ask for 200 casts every frame: at
// a budget of 0 ms each frame runs one cast and defers the rest, and every raycast still answers
// in time, which the scene logs as `raycast-storm complete`. The budget is restored afterwards.
import assert from 'node:assert/strict'
import { spec, until, type ScenarioOptions } from '../../driver/index.ts'
import { visualRealm } from './realm.ts'
import { RAYCAST_STORM_SCENE } from './scenes.ts'

const scenario = spec(import.meta.url)
const server = await visualRealm()

const SESSION: ScenarioOptions['session'] = { realm: server.realm, position: '0,0', signIn: 'guest', windowSize: [960, 540] }
const SCENARIO_TIMEOUT_MS = 600_000
/** `root`, `player` and `camera` are listed by `/scene_entities` before the scene adds any. */
const RESERVED_ENTITIES = 3

scenario(
  'a low budget defers raycasts and every one still completes',
  async ({ explorer, t }) => {
    await explorer.teleport(...RAYCAST_STORM_SCENE.base)
    await until(() => explorer.sceneEntities(), (e) => e.length > RESERVED_ENTITIES, { timeoutMs: 60_000, intervalMs: 1000, what: 'the storm scene' })
    const before = await explorer.raycastBudget()
    t.diagnostic(`budget before: ${before} ms, stats: ${JSON.stringify(await explorer.raycastStats())}`)
    try {
      assert.equal(await explorer.raycastBudget(0), 0)
      const stats = await until(() => explorer.raycastStats(), (s) => s.deferred > 0, { timeoutMs: 10_000, what: 'deferred raycasts' })
      t.diagnostic(`stats at 0 ms: ${JSON.stringify(stats)}`)
      assert.equal(stats.ran, 1)
      await until(() => explorer.sceneLogs(300), (l) => l.some((x) => x.includes('raycast-storm complete')), { timeoutMs: 60_000, what: 'every raycast to answer once' })
    } finally {
      await explorer.raycastBudget(before)
    }
  },
  { session: SESSION, timeoutMs: SCENARIO_TIMEOUT_MS }
)
