import { copyManagedTree } from './copy.js'
import { CONFIG, SNAPSHOT } from './ownership.js'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import type { GeneratedFile } from '../generators/types.js'
import { resolveContained } from './fs-safe.js'

// Only the output folder and identity metadata are staged; the library and the rest of the repository are not.
type StagedOutput = { stagedDir: string; completeness: 'partial'; cleanup(): Promise<void> }

// Builds what the output folder WOULD be after a sync, without touching it: a copy of the folder from the
// base checkout with the generated files laid over it. The guard compares that copy with the base.
async function stageOutput(
  baseDir: string,
  directory: string,
  files: GeneratedFile[]
): Promise<StagedOutput> {
  const stagedDir = await mkdtemp(path.join(os.tmpdir(), 'figma-stage-'))
  const cleanup = () => rm(stagedDir, { recursive: true, force: true })
  try {
    await copyManagedTree(path.join(baseDir, directory), path.join(stagedDir, directory)).catch(
      (error) => {
        if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error
      }
    )
    // Ownership needs the same stable identity metadata as the base, even when only output is staged.
    for (const metadata of [
      CONFIG,
      `${SNAPSHOT}/components`,
      `${SNAPSHOT}/pages.json`,
      `${SNAPSHOT}/sync-state.json`
    ]) {
      const target = await resolveContained(stagedDir, metadata)
      await mkdir(path.dirname(target), { recursive: true })
      await copyManagedTree(path.join(baseDir, metadata), target).catch((error) => {
        if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error
      })
    }
    for (const file of files) {
      const target = await resolveContained(path.join(stagedDir, directory), file.path)
      await mkdir(path.dirname(target), { recursive: true })
      await writeFile(target, file.content)
    }
    return { stagedDir, completeness: 'partial', cleanup }
  } catch (error) {
    await cleanup()
    throw error
  }
}

export { stageOutput }
export type { StagedOutput }
