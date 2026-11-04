// Smart wearables: a wearable whose content carries an SDK7 scene, run as a portable experience.
// explorer-automation has no such fixture, so nothing here is a port and no Unity test is mapped.
//
// | Scenario                                                       | Session                         | Needs                          |
// | a realm that previews the fixture runs it as a smart wearable  | guest, on the loopback realm    | the fixture built              |
// | a killed smart wearable stops, and a restart runs it again     | guest, on the loopback realm    | the fixture built              |
// | a denied smart wearable does not run                           | the target's account and realm  | `DCL_E2E_SMART_WEARABLE_URN`   |
//
// The first two boot a client on a loopback realm whose `/preview-wearables` lists the fixture in
// `scenes/smart-wearable` (`driver/realm.ts`), the creator preview path: the engine runs every
// listed wearable on realm entry, with no authorization prompt because the fixture declares no
// permission. The fixture logs `smart wearable fixture started` and shows one UI label, read
// through the scene commands after `/set_scene` points them at it. Build it first:
// `npm run build-scenes -- smart-wearable`.
//
// The third runs a published smart wearable from the configured backend with
// `/smart_wearable run`; when it declares a gated permission the authorization dialog opens and
// is denied. Skipped unless `DCL_E2E_SMART_WEARABLE_URN` names one.
import assert from 'node:assert/strict'
import path from 'node:path'
import { after } from 'node:test'
import {
  buildPreviewWearable,
  PREVIEW_WEARABLE_URN,
  serveRealm,
  spec,
  until,
  type Explorer,
  type LiveScene,
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
const SIDEBAR_BUTTON = { role: 'button', name: 'Smart wearables' } as const
const PERMISSIONS_DIALOG = { role: 'dialog', name: 'Smart wearable permissions' } as const
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

const PREVIEW_SESSION: ScenarioOptions['session'] = { realm: server.realm, position: '0,0', signIn: 'guest' }

function requireFixture(): void {
  if (fixtureError) throw fixtureError
}

const isFixtureScene = (scene: LiveScene): boolean => scene.title === FIXTURE_TITLE && scene.smartWearable

function waitState(explorer: Explorer, urn: string, state: SmartWearableState, timeoutMs: number): Promise<SmartWearableEntry | undefined> {
  return until(() => explorer.smartWearable(urn), (entry) => entry?.state === state, { timeoutMs, intervalMs: 1000, what: `${urn} to be ${state} in /smart_wearables` })
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
    assert.ok(button.found, 'the sidebar has no "Smart wearables" button while one runs')
  },
  { session: PREVIEW_SESSION }
)

scenario(
  'a killed smart wearable stops, and a restart runs it again',
  async ({ explorer, restore }) => {
    requireFixture()
    await waitFixtureLive(explorer)
    await waitState(explorer, PREVIEW_WEARABLE_URN, 'running', START_TIMEOUT_MS)
    // Leave the fixture running for whatever runs next on this client, whatever the outcome.
    restore(async () => {
      const entry = await explorer.smartWearable(PREVIEW_WEARABLE_URN)
      if (entry?.state === 'killed') await explorer.smartWearableAction('restart', PREVIEW_WEARABLE_URN)
    })

    await explorer.smartWearableAction('kill', PREVIEW_WEARABLE_URN)
    await until(() => explorer.liveScenes(), (live) => !live.some(isFixtureScene), {
      timeoutMs: STOP_TIMEOUT_MS,
      what: `"${FIXTURE_TITLE}" to leave /live_scenes after the kill`
    })
    await waitState(explorer, PREVIEW_WEARABLE_URN, 'killed', STOP_TIMEOUT_MS)
    // The button stays while a killed wearable can still be restarted from its popover.
    await explorer.hud.wait(SIDEBAR_BUTTON, { timeoutMs: 10_000 })

    await explorer.smartWearableAction('restart', PREVIEW_WEARABLE_URN)
    await waitState(explorer, PREVIEW_WEARABLE_URN, 'running', START_TIMEOUT_MS)
    await waitFixtureLive(explorer)
  },
  { session: PREVIEW_SESSION }
)

scenario(
  'a smart wearable whose permissions are denied does not run',
  async ({ explorer, t, restore }) => {
    const urn = CONFIGURED_URN!
    const smartBefore = new Set((await explorer.liveScenes()).filter((scene) => scene.smartWearable).map((scene) => scene.hash))
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
    if (entry?.state === 'running') {
      // A wearable without USE_FETCH, USE_WEBSOCKET, USE_WEB3_API or OPEN_EXTERNAL_LINK is not
      // asked about: there is nothing to deny.
      assert.equal(await explorer.hud.isVisible(PERMISSIONS_DIALOG), false, 'the permissions dialog opened for a wearable that runs')
      t.diagnostic(`${urn} declares no gated permission: it ran without a prompt, so nothing was denied`)
      return
    }
    assert.equal(entry?.state, 'awaitingAuthorization', `${urn} is ${entry?.state}${entry?.reason ? ` (${entry.reason})` : ''}, not awaiting authorization`)

    await explorer.hud.wait(PERMISSIONS_DIALOG, { timeoutMs: 10_000 })
    await explorer.hud.click({ role: 'button', name: 'Deny' })
    await explorer.hud.waitHidden(PERMISSIONS_DIALOG)
    await waitState(explorer, urn, 'denied', STOP_TIMEOUT_MS)
    const started = (await explorer.liveScenes()).filter((scene) => scene.smartWearable && !smartBefore.has(scene.hash))
    assert.deepEqual(started, [], `a denied smart wearable has a live scene: ${started.map((scene) => scene.title).join(', ')}`)
  },
  { skip: CONFIGURED_URN ? undefined : 'set DCL_E2E_SMART_WEARABLE_URN to a smart wearable on the configured backend' }
)
