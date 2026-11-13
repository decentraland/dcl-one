// Smart wearables: a wearable whose content carries an SDK7 scene, run as a portable experience.
// explorer-automation has no such fixture, so nothing here is a port and no Unity test is mapped.
//
// | Scenario                                                          | Session                         | Needs                          |
// | a realm that previews the fixture runs it as a smart wearable     | guest, on the loopback realm    | the fixture built              |
// | a stopped smart wearable leaves, and its Restart runs it again    | guest, on the loopback realm    | the fixture built              |
// | a scene that disables portable experiences pauses the fixture     | guest, on the disabling realm   | the fixture built              |
// | a published smart wearable runs with no prompt                    | guest, on the target's realm    | `DCL_E2E_SMART_WEARABLE_URN`   |
//
// The first two boot a client on a loopback realm whose `/preview-wearables` lists the fixture in
// `scenes/smart-wearable` (`driver/realm.ts`), the creator preview path: the engine runs every
// listed wearable on realm entry. Nothing is asked up front: `requiredPermissions` is ignored, and
// a permission is asked when the scene uses it, as for any portable. The fixture logs
// `smart wearable fixture started` and shows one UI label, read through the scene commands after
// `/set_scene` points them at it. The stop goes through `/stop_portable`, the console line the
// HUD's `Stop` sends, and the restart through the HUD's `Portable experiences` popover. Build the
// fixture first: `npm run build-scenes -- smart-wearable`.
//
// The third serves the same project as a parcel scene at the spawn, its `scene.json` rewritten to
// `featureToggles.portableExperiences: disabled`: standing in it pauses the fixture (blocked, still
// live) rather than unloading it.
//
// The fourth runs a published smart wearable from the configured backend with
// `/smart_wearable run`, which needs no ownership, so a guest session covers the catalyst
// resolution an equipped wearable goes through: it runs, and no smart wearable dialog opens
// whatever it declares. Skipped unless `DCL_E2E_SMART_WEARABLE_URN` names one.
import assert from 'node:assert/strict'
import path from 'node:path'
import { after } from 'node:test'
import {
  buildPreviewWearable,
  buildScene,
  PREVIEW_WEARABLE_URN,
  serveRealm,
  spec,
  until,
  type Explorer,
  type LiveScene,
  type Portable,
  type PreviewWearable,
  type ScenarioOptions,
  type SmartWearableEntry,
  type SmartWearableState
} from '../driver/index.ts'
import { SCENES_DIR } from '../driver/paths.ts'

const scenario = spec(import.meta.url)

const FIXTURE_DIR = path.join(SCENES_DIR, 'smart-wearable')
/** `display.title` of the fixture's `scene.json`, which `/live_scenes` lists. */
const FIXTURE_TITLE = 'Smart Wearable Fixture'
const FIXTURE_LOG = 'smart wearable fixture started'
const FIXTURE_LABEL = 'Smart wearable running'
/** `wearable.json`'s `name`, which the HUD's popover rows carry. */
const FIXTURE_NAME = 'Smart Wearable Fixture'
const SIDEBAR_BUTTON = { role: 'button', name: 'Portable experiences' } as const
const POPOVER = { role: 'dialog', name: 'Portable experiences' } as const
/** The up-front prompt the engine no longer asks: it must never show. */
const AUTHORIZATION_DIALOG = { role: 'dialog', name: 'Smart wearable permissions' } as const
/** The parcel scene of the disabling realm: the fixture's project under another title. */
const HOST_TITLE = 'Portable Experiences Disabled Host'
const CONFIGURED_URN = process.env.DCL_E2E_SMART_WEARABLE_URN?.trim() || undefined

/** The realm's preview is fetched on entry and the scene then boots: well under a minute on the
 *  loopback realm; a published wearable also downloads its content from the backend. */
const START_TIMEOUT_MS = 90_000
const STOP_TIMEOUT_MS = 20_000

// A fixture that is not built fails these scenarios, not the whole run: the InWorld specs share
// one process, and the other specs do not need it.
let fixture: PreviewWearable | undefined
let fixtureError: Error | undefined
try {
  fixture = await buildPreviewWearable(FIXTURE_DIR, PREVIEW_WEARABLE_URN)
} catch (error) {
  const message = error instanceof Error ? error.message : String(error)
  fixtureError = new Error(`the smart-wearable fixture is not ready (${message}); run \`npm run build-scenes -- smart-wearable\` in apps/bevy-explorer/e2e`, { cause: error })
}

// An ephemeral port: the visual realm holds the configured one in the same process.
const server = await serveRealm({ scenes: [], port: 0, realmName: 'e2e-smart-wearables', previewWearables: fixture ? [fixture] : [] })
after(() => server.close())

// The same previews, with a parcel scene at the spawn that disables portable experiences.
const host = fixture
  ? await buildScene(FIXTURE_DIR, (metadata) => ({
      ...metadata,
      display: { title: HOST_TITLE },
      scene: { parcels: ['0,0'], base: '0,0' },
      featureToggles: { portableExperiences: 'disabled' }
    }))
  : undefined
const disablingServer = await serveRealm({ scenes: host ? [host] : [], port: 0, realmName: 'e2e-portables-disabled', previewWearables: fixture ? [fixture] : [] })
after(() => disablingServer.close())

const PREVIEW_SESSION: ScenarioOptions['session'] = { realm: server.realm, position: '0,0', signIn: 'guest' }
const DISABLING_SESSION: ScenarioOptions['session'] = { realm: disablingServer.realm, position: '0,0', signIn: 'guest' }

function requireFixture(): void {
  if (fixtureError) throw fixtureError
}

const isFixtureScene = (scene: LiveScene): boolean => scene.title === FIXTURE_TITLE && scene.smartWearable

function waitState(explorer: Explorer, urn: string, state: SmartWearableState, timeoutMs: number): Promise<SmartWearableEntry | undefined> {
  return until(() => explorer.smartWearable(urn), (entry) => entry?.state === state, { timeoutMs, intervalMs: 1000, what: `${urn} to be ${state} in /smart_wearables` })
}

function waitPortable(explorer: Explorer, state: Portable['state'], timeoutMs: number): Promise<Portable[]> {
  const isFixture = (portable: Portable): boolean => portable.kind === 'smartWearable' && portable.name === FIXTURE_NAME
  return until(() => explorer.portables(), (portables) => portables.some((portable) => isFixture(portable) && portable.state === state), {
    timeoutMs,
    intervalMs: 1000,
    what: `"${FIXTURE_NAME}" to be ${state} in /portables`
  })
}

async function waitFixtureLive(explorer: Explorer): Promise<LiveScene> {
  const scenes = await until(() => explorer.liveScenes(), (live) => live.some(isFixtureScene), {
    timeoutMs: START_TIMEOUT_MS,
    intervalMs: 1000,
    what: `/live_scenes to list "${FIXTURE_TITLE}" as a smart wearable`
  })
  return scenes.find(isFixtureScene)!
}

scenario(
  'a realm that previews the fixture runs it as a smart wearable',
  async ({ explorer, restore }) => {
    requireFixture()
    const scene = await waitFixtureLive(explorer)
    assert.ok(scene.portable, `"${FIXTURE_TITLE}" is listed without ", portable"`)
    assert.equal(scene.broken, false, `"${FIXTURE_TITLE}" is broken`)
    const report = await until(() => explorer.smartWearables(), (wearables) => wearables.entries.some((entry) => entry.urn === PREVIEW_WEARABLE_URN && entry.state === 'running'), {
      timeoutMs: START_TIMEOUT_MS,
      intervalMs: 1000,
      what: `${PREVIEW_WEARABLE_URN} to be running in /smart_wearables`
    })
    // No parcel scene at the spawn, so nothing disables portable experiences.
    assert.equal(report.currentSceneAllows, true)

    restore(() => explorer.setScene())
    await explorer.setScene(scene.hash)
    await until(() => explorer.sceneLogs(50), (logs) => logs.some((line) => line.includes(FIXTURE_LOG)), {
      timeoutMs: 10_000,
      what: `the fixture to log "${FIXTURE_LOG}"`
    })
    await until(() => explorer.uiTexts(), (texts) => texts.includes(FIXTURE_LABEL), { timeoutMs: 10_000, what: `the fixture's UI label "${FIXTURE_LABEL}"` })

    const button = await explorer.hud.wait(SIDEBAR_BUTTON, { timeoutMs: 10_000 })
    assert.ok(button.found, 'the sidebar has no "Portable experiences" button while one runs')
    assert.equal(await explorer.hud.isVisible(AUTHORIZATION_DIALOG), false, 'a smart wearable dialog opened')
  },
  { session: PREVIEW_SESSION }
)

scenario(
  'a stopped smart wearable leaves, and its Restart runs it again',
  async ({ explorer, restore }) => {
    requireFixture()
    const scene = await waitFixtureLive(explorer)
    await waitState(explorer, PREVIEW_WEARABLE_URN, 'running', START_TIMEOUT_MS)
    // Leave the fixture running for whatever runs next on this client, whatever the outcome.
    restore(async () => {
      if (await explorer.hud.isVisible(POPOVER)) await explorer.hud.click(SIDEBAR_BUTTON)
      const entry = await explorer.smartWearable(PREVIEW_WEARABLE_URN)
      if (entry?.state === 'stopped') await explorer.smartWearableAction('restart', PREVIEW_WEARABLE_URN)
    })

    await explorer.stopPortable(scene.hash)
    await until(() => explorer.liveScenes(), (live) => !live.some(isFixtureScene), {
      timeoutMs: STOP_TIMEOUT_MS,
      what: `"${FIXTURE_TITLE}" to leave /live_scenes after the stop`
    })
    await waitState(explorer, PREVIEW_WEARABLE_URN, 'stopped', STOP_TIMEOUT_MS)
    const [stopped] = (await waitPortable(explorer, 'stopped', STOP_TIMEOUT_MS)).filter((portable) => portable.name === FIXTURE_NAME)
    assert.equal(stopped!.hash, null, 'a stopped smart wearable still lists a live hash')

    // A stopped wearable is still equipped: the rail keeps it, and its row offers Restart.
    await explorer.hud.click(SIDEBAR_BUTTON)
    await explorer.hud.wait(POPOVER, { timeoutMs: 10_000 })
    assert.equal(await explorer.hud.isVisible({ role: 'button', name: `Stop ${FIXTURE_NAME}` }), false, 'a stopped smart wearable offers Stop')
    await explorer.hud.click({ role: 'button', name: `Restart ${FIXTURE_NAME}` })
    await waitState(explorer, PREVIEW_WEARABLE_URN, 'running', START_TIMEOUT_MS)
    await waitFixtureLive(explorer)
    await explorer.hud.wait({ role: 'button', name: `Stop ${FIXTURE_NAME}` }, { timeoutMs: 10_000 })
  },
  { session: PREVIEW_SESSION }
)

scenario(
  'a scene that disables portable experiences pauses the fixture',
  async ({ explorer }) => {
    requireFixture()
    const scene = await waitFixtureLive(explorer)
    const report = await until(() => explorer.smartWearables(), (wearables) => wearables.entries.some((entry) => entry.urn === PREVIEW_WEARABLE_URN && entry.state === 'blocked'), {
      timeoutMs: START_TIMEOUT_MS,
      intervalMs: 1000,
      what: `${PREVIEW_WEARABLE_URN} to be blocked in /smart_wearables while the player stands in "${HOST_TITLE}"`
    })
    assert.equal(report.currentSceneAllows, false)
    await waitPortable(explorer, 'blocked', STOP_TIMEOUT_MS)
    // Paused, not unloaded: the scene stays live and keeps its state for when the player leaves.
    const live = (await explorer.liveScenes()).find(isFixtureScene)
    assert.equal(live?.hash, scene.hash, `"${FIXTURE_TITLE}" left /live_scenes while paused`)
    assert.ok((await explorer.liveScenes()).some((candidate) => candidate.title === HOST_TITLE && !candidate.portable), `"${HOST_TITLE}" is not the parcel scene`)
  },
  { session: DISABLING_SESSION }
)

scenario(
  'a published smart wearable runs with no prompt',
  async ({ explorer, restore }) => {
    const urn = CONFIGURED_URN!
    // The run is session state, not account state; stop it if it got as far as running.
    restore(async () => {
      if ((await explorer.smartWearable(urn))?.state === 'running') await explorer.smartWearableAction('kill', urn)
    })

    await explorer.smartWearableAction('run', urn)
    const entry = await until(() => explorer.smartWearable(urn), (found) => found !== undefined && found.state !== 'loading', {
      timeoutMs: START_TIMEOUT_MS,
      intervalMs: 1000,
      what: `${urn} to leave the loading state`
    })
    assert.equal(entry?.state, 'running', `${urn} is ${entry?.state}${entry?.reason ? ` (${entry.reason})` : ''}, not running`)
    // Whatever its scene.json declares, nothing is asked before it runs: a permission is asked
    // when the scene uses it.
    assert.equal(await explorer.hud.isVisible(AUTHORIZATION_DIALOG), false, 'a smart wearable dialog opened')
    const live = await until(() => explorer.liveScenes(), (scenes) => scenes.some((scene) => scene.smartWearable && !scene.broken), {
      timeoutMs: START_TIMEOUT_MS,
      intervalMs: 1000,
      what: `${urn} to have a live scene`
    })
    assert.ok(live.some((scene) => scene.smartWearable && scene.portable))
  },
  { session: { signIn: 'guest' }, skip: CONFIGURED_URN ? undefined : 'set DCL_E2E_SMART_WEARABLE_URN to a smart wearable on the configured backend' }
)
