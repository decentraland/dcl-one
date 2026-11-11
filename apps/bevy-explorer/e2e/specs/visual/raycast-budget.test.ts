// The two raycast tiers (`/raycast_budget`, `/raycast_stats`) on the `raycast-storm` scene
// (`scenes/raycast-storm`), whose 200 continuous raycasts ask for 200 casts every frame. The player
// waits on the empty parcel next to it, so its raycasts are in the capped tier: at a budget of 0 ms
// none runs and all are deferred. Walking into the scene makes it active, and its raycasts then run
// uncapped even at 0 ms, which the scene logs as `raycast-storm complete`. The budget drops to 0
// before the scene loads, so its raycasts never run capped; it returns to `auto` afterwards.
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
/** The empty parcel south of the storm scene, inside its loading range. */
const OUTSIDE: readonly [number, number] = [RAYCAST_STORM_SCENE.base[0], RAYCAST_STORM_SCENE.base[1] - 1]
/** How long the capped raycasts are watched for a cast that should not happen. */
const CAPPED_WATCH_MS = 5_000

scenario(
  'a zero budget holds a nearby scene and the active scene still runs',
  async ({ explorer, t }) => {
    try {
      assert.equal(await explorer.raycastBudget(0), 0)
      await explorer.teleport(...OUTSIDE)
      // inspect the storm scene from outside it
      await until(
        () => explorer.run(`/set_scene ${RAYCAST_STORM_SCENE.id}`).then(() => true, () => false),
        (found) => found,
        { timeoutMs: 60_000, intervalMs: 1000, what: 'the storm scene to load' }
      )
      await until(() => explorer.sceneEntities(), (e) => e.length > RESERVED_ENTITIES, { timeoutMs: 60_000, intervalMs: 1000, what: 'the storm scene' })
      const capped = await until(() => explorer.raycastStats(), (s) => s.deferred > 0, { timeoutMs: 10_000, what: 'deferred raycasts' })
      t.diagnostic(`capped at 0 ms: ${JSON.stringify(capped)}`)
      assert.equal(capped.ran, 0)
      await new Promise((resolve) => setTimeout(resolve, CAPPED_WATCH_MS))
      const held = await explorer.sceneLogs(300)
      assert.ok(!held.some((x) => x.includes('raycast entity=')), `a capped raycast ran at 0 ms: ${held.filter((x) => x.includes('raycast entity=')).join(' | ')}`)

      await explorer.teleport(...RAYCAST_STORM_SCENE.base)
      await until(() => explorer.sceneLogs(300), (l) => l.some((x) => x.includes('raycast-storm complete')), { timeoutMs: 60_000, what: 'every raycast to answer once the scene is active' })
      const active = await explorer.raycastStats()
      t.diagnostic(`active at 0 ms: ${JSON.stringify(active)}`)
      assert.ok(active.uncappedRan > 0)
    } finally {
      await explorer.raycastBudget('auto')
      await explorer.run('/set_scene')
    }
  },
  { session: SESSION, timeoutMs: SCENARIO_TIMEOUT_MS }
)
