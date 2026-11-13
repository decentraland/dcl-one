import { execFile } from 'node:child_process'
import { rm } from 'node:fs/promises'
import path from 'node:path'
import { promisify } from 'node:util'
import type { App } from './app-context.js'
import { createApp } from './app-context.js'
import type { GenerateResult } from './generators/index.js'
import { checkScope } from './guard/check-scope.js'
import { copyManagedTree } from './guard/copy.js'
import { atomicWrite, readBytesIfExists, resolveContained, toPosix } from './guard/fs-safe.js'
import { GENERATED } from './guard/ownership.js'
import { previewCleanupPlan } from './guard/preview-cleanup.js'
import { loadRetirements } from './guard/retirements.js'
import { stageWorkspace } from './guard/workspace-stage.js'
import { writeReviewManifest } from './review-manifest.js'
import { diffFiles, writeFiles } from './sync.js'

const execute = promisify(execFile)

async function runPreviewCleanup(app: App, generated: GenerateResult) {
  const root = path.resolve(app.cwd, '..')
  const configFile = path.join(root, 'figma/figma.config.json')
  if (
    path.resolve(app.configFile ?? configFile) !== configFile ||
    app.resolveOutDir() !== path.join(root, GENERATED)
  )
    throw new Error(
      'Preview cleanup requires the repository default config and generated directory'
    )
  const staged = await stageWorkspace(root, configFile)
  try {
    const lock =
      app.config.snapshotLock &&
      toPosix(path.relative(root, path.resolve(app.cwd, app.config.snapshotLock)))
    if (!lock || lock.startsWith('../'))
      throw new Error('Preview cleanup requires a contained capture lock')
    await copyManagedTree(
      await resolveContained(root, lock),
      await resolveContained(staged.stagedDir, lock)
    )
    // Selection cleanup never edits the capture or lifecycle ledgers, so this copy verifies normally too.
    const stagedApp = await createApp(path.dirname(staged.configFile), staged.configFile)
    await stagedApp.createSource('snapshot').ready?.()
    await writeFiles(stagedApp.resolveOutDir(), generated.files)
    await writeReviewManifest(stagedApp)
    const plan = await previewCleanupPlan(root, staged.stagedDir)
    for (const file of plan.removed) await rm(await resolveContained(staged.stagedDir, file))
    const result = await checkScope({
      baseDir: root,
      stagedDir: staged.stagedDir,
      capturedPageIds: [],
      runKind: 'developer',
      stage: staged.completeness,
      retirements: await loadRetirements(root),
      removalsOnly: ['components'],
      consumerRoot: root,
      prunePreview: true
    })
    if (!result.ok)
      throw new Error(
        `Preview cleanup is inconsistent; nothing was written: ${JSON.stringify(result.violations)}`
      )
    const liveHead = (await execute('git', ['-C', root, 'rev-parse', 'HEAD'])).stdout.trim()
    if (liveHead !== plan.head)
      throw new Error('HEAD changed during preview cleanup; nothing was written')
    // Recheck source/provenance and surviving consumers immediately before publishing the verified stage.
    await previewCleanupPlan(root, staged.stagedDir, true)
    const before = await diffFiles(app.resolveOutDir(), generated.files)
    await writeFiles(app.resolveOutDir(), generated.files)
    const catalog = 'figma/review/src/review/catalog.json'
    const catalogBytes = await readBytesIfExists(await resolveContained(staged.stagedDir, catalog))
    if (!catalogBytes) throw new Error('Fresh Review catalog disappeared')
    await atomicWrite(await resolveContained(root, catalog), catalogBytes)
    // Only exact, recomputed generated derivatives leave. Source identities and public APIs are untouched.
    for (const file of plan.removed) await rm(await resolveContained(root, file))
    return {
      outDir: app.resolveOutDir(),
      diff: { ...before, removed: [] },
      summary: generated.summary,
      pruned: plan.removed
    }
  } finally {
    await staged.cleanup()
  }
}

export { runPreviewCleanup }
