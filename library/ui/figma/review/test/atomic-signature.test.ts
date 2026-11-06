import assert from 'node:assert/strict'
import { afterEach, beforeEach, describe, it } from 'node:test'
import { copyFile, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import { reviewSignature } from '../scripts/review-signature.mjs'

// The signature reads the render dependencies from the library's pnpm-lock.yaml, two levels above the
// Review (library/ui/figma/review), so the fixture keeps that shape and carries the real lockfile.
const LOCKFILE = path.resolve(import.meta.dirname, '../../../pnpm-lock.yaml')

describe('when an atomic write was interrupted', () => {
  let workspace: string
  let root: string
  beforeEach(async () => {
    workspace = await mkdtemp(path.join(os.tmpdir(), 'atomic-signature-'))
    root = path.join(workspace, 'figma/review')
    await copyFile(LOCKFILE, path.join(workspace, 'pnpm-lock.yaml'))
    for (const directory of [
      'src/components',
      'src/review',
      'src/generated',
      'public/dcl-ui-review/references'
    ])
      await mkdir(path.join(root, directory), { recursive: true })
    for (const file of [
      'StateMatrix.tsx',
      'StateMatrix.module.css',
      'figma-module-runtime.tsx',
      'figma-module-runtime.module.css',
      'figma-view-state.ts',
      'figma-interactions.tsx'
    ])
      await writeFile(path.join(root, 'src/components', file), 'fixture')
  })
  afterEach(async () => {
    await rm(workspace, { recursive: true, force: true })
  })
  it('should preserve the source signature while temporary files remain', async () => {
    const before = await reviewSignature(root)
    for (const directory of ['src/review', 'src/generated', 'public/dcl-ui-review/references'])
      await writeFile(
        path.join(root, directory, '.figma-tmp-00000000-0000-4000-8000-000000000000'),
        'partial'
      )
    assert.equal(await reviewSignature(root), before)
  })
})
