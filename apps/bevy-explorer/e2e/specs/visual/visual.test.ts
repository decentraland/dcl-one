// explorer-automation's Visual suite (17 fixtures, one `Default` test each): the scene's frame,
// from its virtual camera, matches the reference pixel for pixel within a budget.
//
// The Unity suite loaded each scene into one host parcel; here every scene is built, served by
// one loopback realm at its own parcel (`12 * k, 0`), and the player is teleported to it with the
// scene's own spawn point. The frame is `/screenshot`: the canvas, without the web HUD.
//
// A scene is ready when it has created entities beyond the three reserved ones; what then still
// moves (animations, glyph atlases, wearables, the trigger scene's retry window) has nothing to
// observe, so each scene has a fixed settle time (`scenes.ts`). A missing reference fails the
// scenario, unless the run records (`DCL_E2E_RECORD=1`), which writes it and passes.
import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { after } from 'node:test'
import { buildScene, comparePng, readTarget, serveRealm, sleep, spec, until, type Comparison, type ScenarioOptions } from '../../driver/index.ts'
import { REFERENCES_DIR, SCENES_DIR } from '../../driver/paths.ts'
import type { BuiltScene } from '../../driver/realm.ts'
import { SCENES } from './scenes.ts'

const scenario = spec(import.meta.url)
const target = readTarget()

/** `root`, `player` and `camera` are listed by `/scene_entities` before the scene adds any. */
const RESERVED_ENTITIES = 3
const SCENE_LIVE_TIMEOUT_MS = 60_000
/** The first scenario boots the client (up to 300 s for the world) and every one waits for its
 *  arrival (up to 180 s), the entities (60 s) and the settle time (15 s at most): the scenario's
 *  own limit is above their sum, so a stuck wait reports itself. */
const SCENARIO_TIMEOUT_MS = 600_000
const WINDOW_SIZE: [number, number] = [960, 540]

async function build(id: string): Promise<BuiltScene> {
  try {
    return await buildScene(path.join(SCENES_DIR, id))
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    if (message.includes('build the scene first')) {
      throw new Error(`the visual scene "${id}" is not built (${message}); run \`npm run build-scenes\` in apps/bevy-explorer/e2e`, { cause: error })
    }
    throw error
  }
}

const server = await serveRealm({ scenes: await Promise.all(SCENES.map(({ id }) => build(id))), port: target.ports.realm })
after(() => server.close())

const SESSION: ScenarioOptions['session'] = { realm: server.realm, position: '0,0', signIn: 'guest', windowSize: WINDOW_SIZE }

async function readReference(file: string): Promise<Buffer | undefined> {
  try {
    return await readFile(file)
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return undefined
    throw error
  }
}

/** What a checkout without `git lfs pull` leaves in place of a reference. */
function isLfsPointer(file: Buffer): boolean {
  return file.toString('latin1', 0, 40).startsWith('version https://git-lfs')
}

for (const scene of SCENES) {
  scenario(
    `${scene.id} matches its reference`,
    async ({ explorer, t, report }) => {
      const [x, y] = scene.base
      await explorer.teleport(x, y)
      await until(() => explorer.sceneEntities(), (entities) => entities.length > RESERVED_ENTITIES, {
        timeoutMs: SCENE_LIVE_TIMEOUT_MS,
        intervalMs: 1000,
        what: `the scene ${scene.id} at ${x},${y} to create its entities`
      })
      await sleep(scene.settleMs)
      const frame = await explorer.screenshot()

      const file = path.join(REFERENCES_DIR, `${scene.id}.png`)
      const reference = await readReference(file)
      if (!reference) {
        await report.write({ 'frame.png': frame })
        if (!target.record) assert.fail(`no reference for ${scene.id}: ${file} is missing; record it with DCL_E2E_RECORD=1 (in CI, on the suite's runner)`)
        await mkdir(REFERENCES_DIR, { recursive: true })
        await writeFile(file, frame)
        t.diagnostic('recorded the reference')
        return
      }
      if (isLfsPointer(reference)) assert.fail(`${file} is a Git LFS pointer, not a PNG: run \`git lfs pull\``)

      let comparison: Comparison
      try {
        comparison = comparePng(reference, frame, { maxDifferentRatio: scene.maxDifferentRatio })
      } catch (error) {
        await report.write({ 'frame.png': frame, 'reference.png': reference })
        throw new Error(`${scene.id}: cannot compare the frame with ${file}: ${error instanceof Error ? error.message : String(error)}`, { cause: error })
      }
      if (!comparison.pass) await report.write({ 'frame.png': frame, 'reference.png': reference, 'diff.png': comparison.diff })
      const { differing, total, ratio } = comparison
      t.diagnostic(`${differing}/${total} pixels differ (ratio ${ratio.toFixed(5)}, budget ${scene.maxDifferentRatio})`)
      assert.ok(
        comparison.pass,
        `${scene.id}: ${differing}/${total} pixels differ (ratio ${ratio.toFixed(5)}), above the ${scene.maxDifferentRatio} budget; see frame.png, reference.png and diff.png in ${report.dir}`
      )
    },
    { session: SESSION, timeoutMs: SCENARIO_TIMEOUT_MS }
  )
}
