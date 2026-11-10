// The pointer pipeline, driven through `/hover_entity` and `/click_entity` on the
// `pointer-targets` scene (`scenes/pointer-targets`): a hover reports the hit and its hover
// entries, a click reaches the scene as a Down and an Up on different ticks, a wall blocks the
// box behind it, a 50 m target with a camera-distance reach hovers, and a glTF model is clicked
// by its container entity.
//
// Each scenario teleports to the scene and waits for its `pointer-targets ready` log line; the
// targets are found by their hover text (`/scene_entities PointerEvents`, then each one's
// `PointerEvents`).
import assert from 'node:assert/strict'
import { spec, until, type Explorer, type ScenarioOptions } from '../../driver/index.ts'
import { visualRealm } from './realm.ts'
import { POINTER_SCENE } from './scenes.ts'

const scenario = spec(import.meta.url)
const server = await visualRealm()

const SESSION: ScenarioOptions['session'] = { realm: server.realm, position: '0,0', signIn: 'guest', windowSize: [960, 540] }
const SCENARIO_TIMEOUT_MS = 600_000
const SCENE_LIVE_TIMEOUT_MS = 60_000
/** `root`, `player` and `camera` are listed by `/scene_entities` before the scene adds any. */
const RESERVED_ENTITIES = 3
/** Enough scene log lines to hold the ready line and a click's two lines. */
const LOG_LINES = 200

/** `PbPointerEvents` as `/inspect_component` reports it. */
interface PointerEventsComponent {
  pointerEvents?: { eventInfo?: { hoverText?: string } }[]
}

/** Teleport to the scene and wait until it has created its targets and said so. */
async function arrive(explorer: Explorer): Promise<void> {
  const [x, y] = POINTER_SCENE.base
  await explorer.teleport(x, y)
  await until(() => explorer.sceneEntities(), (entities) => entities.length > RESERVED_ENTITIES, {
    timeoutMs: SCENE_LIVE_TIMEOUT_MS,
    intervalMs: 1000,
    what: `the scene ${POINTER_SCENE.id} at ${x},${y} to create its entities`
  })
  await until(() => explorer.sceneLogs(LOG_LINES), (lines) => lines.some((line) => line.includes('pointer-targets ready')), {
    timeoutMs: SCENE_LIVE_TIMEOUT_MS,
    intervalMs: 1000,
    what: `the scene ${POINTER_SCENE.id} to log that it is ready`
  })
}

/** The scene entity whose `PointerEvents` carries `hoverText`. */
async function entityNamed(explorer: Explorer, hoverText: string): Promise<number> {
  for (const id of await explorer.sceneEntities('PointerEvents')) {
    if (!/^\d+$/.test(id)) continue
    const component = await explorer.inspect<PointerEventsComponent>(id, 'PointerEvents')
    if (component.pointerEvents?.some((entry) => entry.eventInfo?.hoverText === hoverText)) return Number(id)
  }
  throw new Error(`no entity of ${POINTER_SCENE.id} has the hover text "${hoverText}"`)
}

scenario(
  'hover reports the near box with its hover text',
  async ({ explorer, t }) => {
    await arrive(explorer)
    const near = await entityNamed(explorer, 'Near')
    const report = await explorer.hoverEntity(near, { bounds: true })
    t.diagnostic(JSON.stringify(report))
    assert.equal(report.diagnosis, null)
    assert.equal(report.hit?.entity, near)
    assert.ok(report.entries.some((e) => e.hoverText === 'Near' && e.enabled), `no enabled "Near" entry: ${JSON.stringify(report.entries)}`)
  },
  { session: SESSION, timeoutMs: SCENARIO_TIMEOUT_MS }
)

scenario(
  'click sends Down then Up on different ticks and the scene logs them',
  async ({ explorer, t }) => {
    await arrive(explorer)
    const near = await entityNamed(explorer, 'Near')
    const report = await explorer.clickEntity(near, { bounds: true })
    t.diagnostic(JSON.stringify(report))
    assert.equal(report.diagnosis, null)
    const down = report.events.find((e) => e.event === 'PetDown')
    const up = report.events.find((e) => e.event === 'PetUp')
    assert.ok(down && up && up.tick > down.tick, `expected a PetDown then a PetUp on a later tick: ${JSON.stringify(report.events)}`)
    const logs = await until(() => explorer.sceneLogs(LOG_LINES), (l) => l.some((x) => x.includes(`pointer up entity=${near}`)), {
      timeoutMs: 10_000,
      what: 'the scene to log the click'
    })
    assert.ok(logs.some((x) => x.includes(`pointer down entity=${near}`)), `the scene logged no down for ${near}: ${logs.join(' | ')}`)
  },
  { session: SESSION, timeoutMs: SCENARIO_TIMEOUT_MS }
)

scenario(
  'the box behind the wall is blocked',
  async ({ explorer, t }) => {
    await arrive(explorer)
    const behind = await entityNamed(explorer, 'Behind')
    const report = await explorer.clickEntity(behind, { bounds: true })
    t.diagnostic(JSON.stringify(report))
    assert.match(report.diagnosis ?? '', /^blocked by entity \d+$/)
    assert.deepEqual(report.events, [])
  },
  { session: SESSION, timeoutMs: SCENARIO_TIMEOUT_MS }
)

scenario(
  'the box at 50 m hovers',
  async ({ explorer, t }) => {
    await arrive(explorer)
    const far = await entityNamed(explorer, 'Far')
    const report = await explorer.hoverEntity(far, { bounds: true })
    t.diagnostic(JSON.stringify(report))
    assert.equal(report.diagnosis, null)
    assert.equal(report.hit?.entity, far)
    assert.equal(report.entries[0]?.enabled, true, `the "Far" entry is not enabled: ${JSON.stringify(report.entries)}`)
  },
  { session: SESSION, timeoutMs: SCENARIO_TIMEOUT_MS }
)

scenario(
  'the glTF model is clicked by its container id',
  async ({ explorer, t }) => {
    await arrive(explorer)
    const model = await entityNamed(explorer, 'Model')
    const report = await explorer.clickEntity(model, { bounds: true })
    t.diagnostic(JSON.stringify(report))
    assert.equal(report.diagnosis, null)
    assert.ok(report.events.some((e) => e.event === 'PetDown'), `no PetDown: ${JSON.stringify(report.events)}`)
  },
  { session: SESSION, timeoutMs: SCENARIO_TIMEOUT_MS }
)
