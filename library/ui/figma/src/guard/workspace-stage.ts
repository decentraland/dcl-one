import { copyManagedTree } from './copy.js'
import { mkdir, mkdtemp, rm } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { isMissing, readIfExists, resolveContained } from './fs-safe.js'
import { applyRetirements } from './apply-retirements.js'
import type { Retirements } from './retirements.js'

// Upstream stages its whole component-library and dcl-ui-review packages. Here the library is components/
// plus its entry, index.ts, and the lockfile the Review signature reads sits at the library root
// (layout.ts), so all three are staged.
const WORKSPACE_DIRECTORIES = [
  'figma/figma-snapshot',
  'figma/ledger',
  'figma/review',
  'components',
  'index.ts',
  'pnpm-lock.yaml'
]
async function copyIfPresent(from: string, to: string) {
  await copyManagedTree(from, to).catch((error) => {
    if (!isMissing(error)) throw error
  })
}
async function stageWorkspace(root: string, configFile: string) {
  const stagedDir = await mkdtemp(path.join(os.tmpdir(), 'figma-retire-stage-'))
  const configRelative = path.relative(root, configFile)
  const cleanup = () => rm(stagedDir, { recursive: true, force: true })
  try {
    for (const directory of WORKSPACE_DIRECTORIES)
      await copyIfPresent(path.join(root, directory), path.join(stagedDir, directory))
    for (const file of new Set(['figma/figma.config.json', configRelative])) {
      const target = await resolveContained(stagedDir, file)
      if ((await readIfExists(path.join(root, file))) === undefined) continue
      await mkdir(path.dirname(target), { recursive: true })
      await copyManagedTree(path.join(root, file), target)
    }
    // Every published workspace directory is copied, so a path missing from the stage is a removal.
    return {
      stagedDir,
      configFile: path.join(stagedDir, configRelative),
      completeness: 'complete' as const,
      cleanup
    }
  } catch (error) {
    await cleanup()
    throw error
  }
}
async function publishRetirement(root: string, staged: string, retirements: Retirements) {
  await applyRetirements(root, retirements)
  for (const directory of WORKSPACE_DIRECTORIES)
    await copyIfPresent(path.join(staged, directory), path.join(root, directory))
}
export { stageWorkspace, publishRetirement }
