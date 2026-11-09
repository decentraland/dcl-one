import path from 'node:path'
import type { App } from './app.js'
import type { GenerateResult } from './generators/index.js'
import { stageWorkspace, publishRetirement } from './guard/workspace-stage.js'
import { applyRetirements, pendingRetirements } from './guard/apply-retirements.js'
import { checkScope } from './guard/check-scope.js'
import { readIfExists } from './guard/fs-safe.js'
import type { Retirements } from './guard/retirements.js'
import { diffFiles, writeFiles } from './sync.js'
import { committedLedger, drainOrphans, ledgerBase, removeDrained } from './orphan-drain.js'
import { writeReviewManifest } from './review-manifest.js'

async function runRetirementSync(
  app: App,
  generated: GenerateResult,
  retirements: Retirements,
  createApp: (cwd: string, file: string, options: { unverifiedSnapshot: boolean }) => Promise<App>
) {
  const root = path.resolve(app.cwd, '..')
  const staged = await stageWorkspace(
    root,
    app.configFile ?? path.join(app.cwd, 'figma.config.json')
  )
  try {
    // The staged copy of the verified cache is about to have retirements applied, so it is read unverified.
    const stagedApp = await createApp(path.dirname(staged.configFile), staged.configFile, {
      unverifiedSnapshot: true
    })
    stagedApp.config.outDir = path.join(staged.stagedDir, 'figma/review/src/generated')
    stagedApp.config.snapshotDir = path.join(staged.stagedDir, 'figma/figma-snapshot')
    const retired = await applyRetirements(staged.stagedDir, retirements)
    const committed = await committedLedger(root)
    const baseLedger = await ledgerBase(root, committed)
    const orphans = await drainOrphans(staged.stagedDir, committed, root)
    const diff = await diffFiles(app.resolveOutDir(), generated.files)
    await writeFiles(stagedApp.resolveOutDir(), generated.files)
    const catalog = path.join(staged.stagedDir, 'figma/review/src/review/catalog.json')
    if ((await readIfExists(catalog)) !== undefined) await writeReviewManifest(stagedApp)
    const pending = await pendingRetirements(staged.stagedDir, retirements)
    if (pending.length) throw new Error(`Retirements remain pending: ${pending.join(', ')}`)
    // Upstream compares against a materialized release-base commit (120e8f1, not imported: the library is
    // versioned by hand). Here the cleanup is checked against the working tree it starts from.
    const result = await checkScope({
      baseDir: root,
      stagedDir: staged.stagedDir,
      capturedPageIds: [],
      runKind: 'developer',
      stage: staged.completeness,
      retirements,
      removalsOnly: ['components'],
      baseLedger,
      consumerRoot: root
    })
    if (!result.ok)
      throw new Error(
        `Retirement cleanup is inconsistent; nothing was written: ${JSON.stringify(result.violations)}`
      )
    // Remove drained files before publishing the ledger so an interrupted drain can be retried.
    await removeDrained(root, orphans.drained)
    await publishRetirement(root, staged.stagedDir, retirements)
    return {
      outDir: app.resolveOutDir(),
      diff: {
        ...diff,
        deleted: retired
          .filter((file) => file.startsWith('figma/review/src/generated/'))
          .map((file) => file.slice('figma/review/src/generated/'.length))
      },
      summary: generated.summary,
      retired,
      ...(orphans.drained.length ? { drained: orphans.drained } : {}),
      ...(orphans.reactivated.length ? { reactivated: orphans.reactivated } : {})
    }
  } finally {
    await staged.cleanup()
  }
}
export { runRetirementSync }
