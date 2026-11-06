import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import {
  copyFile,
  cp,
  mkdir,
  mkdtemp,
  readdir,
  readFile,
  rm,
  symlink,
  writeFile
} from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { promisify } from 'node:util'
import { afterEach, beforeEach, describe, it } from 'node:test'
import { pendingCommit, withStagedOutput } from '../scripts/capture-retry.mjs'

const run = promisify(execFile)
const scripts = path.resolve(import.meta.dirname, '../scripts')
// The copied scripts sign the Review from the library's pnpm-lock.yaml and resolve their dependencies
// (yaml) from the library's node_modules, both at the workspace root here as in library/ui.
const library = path.resolve(import.meta.dirname, '../../..')
const json = (value: unknown) => JSON.stringify(value) + '\n'
const evidence = (sourceId: string) =>
  json({ cases: { 'Second-0': { sourceId, image: '/dcl-ui-review/renders/Second-0.png' } } })

// Runs the real capture script in a copy of the Review layout. It stops at the missing review build, after
// recovery and frozen-evidence loading, which is the startup sequence under test.
describe('when review:capture starts after an interrupted publication', () => {
  let workspace: string
  let review: string
  let output: string
  beforeEach(async () => {
    workspace = await mkdtemp(path.join(os.tmpdir(), 'capture-startup-'))
    review = path.join(workspace, 'figma/review')
    output = path.join(review, 'public/dcl-ui-review')
    // The library root pins Node through engines.node, as library/ui/package.json does.
    await writeFile(
      path.join(workspace, 'package.json'),
      json({ engines: { node: `>=${process.versions.node}` } })
    )
    await copyFile(path.join(library, 'pnpm-lock.yaml'), path.join(workspace, 'pnpm-lock.yaml'))
    await symlink(path.join(library, 'node_modules'), path.join(workspace, 'node_modules'), 'dir')
    await mkdir(path.join(workspace, 'figma'), { recursive: true })
    await writeFile(
      path.join(workspace, 'figma/figma.config.json'),
      json({ pages: { '1:1': { id: '1:1', name: 'Alpha' }, '1:2': { id: '1:2', name: 'Beta' } } })
    )
    await cp(scripts, path.join(review, 'scripts'), { recursive: true })
    await mkdir(path.join(review, 'src/review'), { recursive: true })
    await writeFile(
      path.join(review, 'src/review/catalog.json'),
      json({
        components: [
          { name: 'First', page: 'Alpha', variants: [{ id: '10:1' }] },
          { name: 'Second', page: 'Beta', variants: [{ id: '10:2' }] }
        ]
      })
    )
    await writeFile(path.join(review, 'src/review/references.json'), json({}))
    await mkdir(path.join(output, 'renders'), { recursive: true })
    // The old pair predates the frozen component's current source; only the new pair matches it.
    await writeFile(path.join(output, 'fidelity.json'), evidence('10:old'))
    await writeFile(path.join(output, 'renders/Second-0.png'), 'old render')
  })
  afterEach(async () => {
    await rm(workspace, { recursive: true, force: true })
  })

  for (const step of ['backup fidelity.json', 'publish renders'])
    it(`should complete the commit left at ${step} before loading frozen evidence`, async () => {
      await assert.rejects(
        withStagedOutput(
          { output, scratch: review },
          async (staging: string) => {
            await writeFile(path.join(staging, 'renders/Second-0.png'), 'new render')
            await writeFile(path.join(staging, 'fidelity.json'), evidence('10:2'))
          },
          async (at: string) => {
            if (at === step) throw new Error('interrupted')
          }
        )
      )
      assert.equal(await pendingCommit(review), true)

      const capture = run(process.execPath, [path.join(review, 'scripts/capture-review.mjs')], {
        cwd: review,
        env: { ...process.env, REVIEW_CAPTURE_PAGE_IDS: '1:1' }
      })

      await assert.rejects(capture, (error: { stderr: string }) => {
        assert.match(error.stderr, /review-dist[\\/]dcl-ui-review[\\/]source-signature\.json/)
        assert.doesNotMatch(error.stderr, /Missing frozen visual evidence|fidelity\.json/)
        return true
      })
      assert.equal(await readFile(path.join(output, 'fidelity.json'), 'utf8'), evidence('10:2'))
      assert.equal(await readFile(path.join(output, 'renders/Second-0.png'), 'utf8'), 'new render')
      assert.equal(await pendingCommit(review), false)
      assert.deepEqual(
        (await readdir(review)).filter((entry) => entry.startsWith('.capture')),
        []
      )
    })
})
