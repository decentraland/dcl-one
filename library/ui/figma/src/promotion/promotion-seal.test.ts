import { cp, mkdtemp, readFile, rm } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { spawnSync } from 'node:child_process'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { createApp, runSync, type App } from '../app.js'
import { checkScope } from '../guard/check-scope.js'
import { GEN, SNAP, json, lifecycleFixture, put } from '../guard/lifecycle-fixture.js'
import { emptyRetirements } from '../guard/retirements.js'
import { PROMOTION_MANIFEST, staleSeal, writePromotionManifest } from './manifest.js'

const CSS = `${GEN}/page/First.module.css`
const TSX = path.resolve(import.meta.dirname, '../../../node_modules/.bin/tsx')
const CLI = path.resolve(import.meta.dirname, '../cli.ts')
// A local developer run: CI variables would switch the guard to its release provenance checks.
const LOCAL_ENV = Object.fromEntries(
  Object.entries(process.env).filter(([key]) => key !== 'CI' && !key.startsWith('GITHUB_'))
)
const temps: string[] = []
afterEach(async () => {
  for (const dir of temps.splice(0)) await rm(dir, { recursive: true, force: true })
})
const copyOf = async (from: string) => {
  const to = await mkdtemp(path.join(os.tmpdir(), 'seal-head-'))
  temps.push(to)
  await cp(from, to, { recursive: true })
  return { root: to, app: await createApp(path.join(to, 'figma')) }
}
// A developer edit of promoted generated output, without running the seal step.
const editOutput = async (root: string) =>
  put(root, CSS, `${await readFile(path.join(root, CSS), 'utf8')}\n.edited { color: red; }\n`)

describe('when the guard checks the review promotion seal', () => {
  let root: string
  let app: App
  beforeEach(async () => {
    ;({ root, app } = await lifecycleFixture())
    temps.push(root)
  })

  it('should pass an unchanged tree', async () => {
    expect(await staleSeal(app)).toEqual([])
  })

  it('should fail a tree whose sealed source changed without a reseal', async () => {
    await editOutput(root)
    expect(await staleSeal(app)).toEqual([`file:${CSS}`])
  })

  it('should pass the same change once the manifest is resealed', async () => {
    await editOutput(root)
    await writePromotionManifest(app)
    expect(await staleSeal(app)).toEqual([])
  })

  it('should report a component spec that no longer matches its sealed hash', async () => {
    const manifest = await json(root, PROMOTION_MANIFEST)
    manifest.components[0].hash = '0'.repeat(64)
    await put(root, PROMOTION_MANIFEST, manifest)
    expect(await staleSeal(app)).toEqual([`component:${manifest.components[0].name}`])
  })

  // A recapture of page 1:1 that changes every component's artwork, then the seal step.
  const recaptureAndSeal = async () => {
    const head = await copyOf(root)
    const page = await json(head.root, `${SNAP}/components/Page.json`)
    for (const component of page.components)
      component.base.tree.svg = component.base.tree.svg.replaceAll(
        /<path d="[^"]*"\/>/g,
        '<path d="M2 2h16v16H2z"/>'
      )
    await put(head.root, `${SNAP}/components/Page.json`, page)
    await runSync(head.app, { source: 'snapshot', capturedPageIds: ['1:1'], ledgerOrphans: true })
    await writePromotionManifest(head.app)
    return head
  }
  const guard = (stagedDir: string, capturedPageIds: string[]) =>
    checkScope({
      baseDir: root,
      stagedDir,
      capturedPageIds,
      runKind: 'automated',
      stage: 'complete',
      retirements: emptyRetirements()
    })

  it('should pass a legitimate automated proposal whose seal step ran', async () => {
    const head = await recaptureAndSeal()

    expect(await staleSeal(head.app)).toEqual([])
    const { violations } = await guard(head.root, ['1:1'])
    expect(violations.filter((item) => item.path === PROMOTION_MANIFEST)).toEqual([])
  })

  it('should still reject resealed manifest entries for a page the run did not capture', async () => {
    const head = await recaptureAndSeal()

    expect(await staleSeal(head.app)).toEqual([])
    const { violations } = await guard(head.root, [])
    expect(
      violations.filter(
        (item) => item.path === PROMOTION_MANIFEST && item.kind === 'aggregate-entry-changed'
      ).length
    ).toBeGreaterThan(0)
  })

  it('should fail the developer guard command on a stale seal', async () => {
    const head = await copyOf(root)
    await editOutput(head.root)
    const run = () =>
      spawnSync(
        TSX,
        [CLI, 'guard', '--base', root, '--staged', head.root, '--run-kind', 'developer'],
        {
          cwd: path.join(head.root, 'figma'),
          encoding: 'utf8',
          env: LOCAL_ENV
        }
      )
    const stale = run()
    expect(stale.stdout).toContain(`sealed file:${CSS} does not match the tree`)
    expect(stale.status).toBe(1)

    await writePromotionManifest(head.app)
    expect(run().stdout).not.toContain('does not match the tree')
  })
})
