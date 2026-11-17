import { runRetirementSync } from './retirement-sync.js'
import path from 'node:path'
import { createApp, type App, type AppOptions, type SourceKind } from './app-context.js'
import { generateAll, type GenerateResult } from './generators/index.js'
import { pendingRetirements } from './guard/apply-retirements.js'
import { checkScope, type RunKind } from './guard/check-scope.js'
import { GENERATED, SNAPSHOT } from './guard/ownership.js'
import { loadRetirements } from './guard/retirements.js'
import { stageOutput } from './guard/stage.js'
import { canonicalPath, validateOutputPath } from './guard/fs-safe.js'
import { diffFiles, writeFiles, type FileDiff } from './sync.js'
import { classifyRemoved, ledgerOrphans } from './orphan-sync.js'

// `developer` is true only for the local CLI flag. Every automated path (MCP, CI, promotion) runs guarded.
type SyncOptions = {
  source: SourceKind
  outDir?: string
  developer?: boolean
  capturedPageIds?: readonly string[]
  foundationsCaptured?: boolean
  applyRetirements?: boolean
  // Ledger the assets this sync stops generating. Only live capture sets it: its guard validates the
  // ledger against the real base before anything is published. Any other sync leaves them `removed`.
  ledgerOrphans?: boolean
}

type SyncReport = {
  outDir: string
  diff: FileDiff
  summary: GenerateResult['summary']
  retired?: string[]
  // Ledgered assets removed after their committed orphan entries were authorized for draining.
  drained?: string[]
  // Ledger entries removed while preserving asset bytes because the assets are used again.
  reactivated?: string[]
  // Ledgered orphans: no longer generated, kept byte-identical, unused. Not drift.
  retained?: string[]
  // Ledger entries that no longer hold (missing, edited or used again). Drift.
  ledgerIssues?: string[]
  // Assets this sync ledgered because it stopped generating them.
  orphaned?: string[]
}

const repositoryRoot = (app: App) => path.resolve(app.cwd, '..')

async function runSync(app: App, options: SyncOptions): Promise<SyncReport> {
  // Shipped output comes from the pinned capture only: a live read may be inspected or diffed, never written.
  if (options.source !== 'snapshot')
    throw new Error(
      'sync writes tracked files and reads only the verified fetch-source cache; use diff to compare against the live file'
    )
  if (options.applyRetirements && !options.developer)
    throw new Error('Applying retirements is a developer action: pass --developer')
  // Verify the cache before anything derived from the tree (ownership, retirements) is read.
  const source = app.createSource('snapshot')
  await source.ready?.()
  const root = repositoryRoot(app)
  // Ownership, retirements and the guard read the capture at figma/figma-snapshot, so that exact directory must be
  // the one just verified: a config naming another snapshotDir would verify one cache and guard against another.
  if (
    (await canonicalPath(path.resolve(app.cwd, app.config.snapshotDir))) !==
    (await canonicalPath(path.join(root, SNAPSHOT)))
  )
    throw new Error(
      `sync reads the capture at ${SNAPSHOT}, but the config names another snapshotDir`
    )
  const runKind: RunKind = options.developer ? 'developer' : 'automated'
  const outDir = await validateOutputPath(
    root,
    app.resolveOutDir(options.outDir),
    runKind === 'automated' ? GENERATED : undefined
  )
  const retirements = await loadRetirements(root)

  // A retirement whose output is still present is half applied. Nothing is published until it is cleaned up.
  const pending = options.applyRetirements ? [] : await pendingRetirements(root, retirements)
  if (pending.length)
    throw new Error(
      `Retirements are pending, run sync --developer --apply-retirements:\n  ${pending.join('\n  ')}`
    )

  const generated = await generateAll(source, {}, app.config)
  if (options.applyRetirements) {
    if (outDir !== (await canonicalPath(path.join(root, GENERATED))))
      throw new Error(
        'Retirement cleanup requires the repository snapshot and generated directories'
      )
    return runRetirementSync(app, generated, retirements, createApp)
  }
  if (runKind === 'automated') {
    const staged = await stageOutput(root, GENERATED, generated.files)
    try {
      const result = await checkScope({
        baseDir: root,
        stagedDir: staged.stagedDir,
        capturedPageIds: options.capturedPageIds ?? [],
        foundationsCaptured: options.foundationsCaptured,
        runKind,
        stage: staged.completeness,
        retirements,
        directories: [GENERATED],
        files: []
      })
      if (!result.ok)
        throw new Error(
          `The sync would change output it does not own, nothing was written:\n${result.violations
            .map((item) => `  ${item.kind}: ${item.path} (${item.cause})`)
            .join('\n')}`
        )
    } finally {
      await staged.cleanup()
    }
  }

  const owners = options.ledgerOrphans
    ? await ledgerOrphans.ownersBefore(app, root, outDir, generated.files)
    : undefined
  const diff = await writeFiles(outDir, generated.files)
  const orphaned = await ledgerOrphans.record(root, diff, owners)
  const report: SyncReport = {
    outDir,
    diff,
    summary: generated.summary,
    ...(orphaned.length ? { orphaned } : {})
  }
  return report
}

async function runDiff(app: App, options: SyncOptions): Promise<SyncReport> {
  const outDir = await validateOutputPath(repositoryRoot(app), app.resolveOutDir(options.outDir))
  const generated = await generateAll(app.createSource(options.source), {}, app.config)
  const diff = await diffFiles(outDir, generated.files)
  const ledger = await classifyRemoved(app, repositoryRoot(app), outDir, diff)
  return {
    outDir,
    diff: { ...diff, removed: ledger.removed },
    summary: generated.summary,
    retained: ledger.retained,
    ledgerIssues: ledger.issues
  }
}

export { createApp, runDiff, runSync }
export type { App, AppOptions, SourceKind, SyncOptions, SyncReport }
