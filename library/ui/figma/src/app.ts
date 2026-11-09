import { configuredPages } from './page-identity.js'
import { runRetirementSync } from './retirement-sync.js'
import path from 'node:path'
import { loadConfig, loadConfigFile, requireFileKey, type FigmaConfig } from './config.js'
import { EnvTokenProvider, FigmaClient } from './figma/client.js'
import { generateAll, type GenerateResult } from './generators/index.js'
import { RestSource } from './sources/rest-source.js'
import { SnapshotSource } from './sources/snapshot-source.js'
import { FdsCacheError, verifyFdsCache } from './sources/fds-source.js'
import type { FigmaSource } from './sources/types.js'
import { pendingRetirements } from './guard/apply-retirements.js'
import { checkScope, type RunKind } from './guard/check-scope.js'
import { GENERATED, SNAPSHOT } from './guard/ownership.js'
import { loadRetirements } from './guard/retirements.js'
import { stageOutput } from './guard/stage.js'
import { canonicalPath, validateOutputPath } from './guard/fs-safe.js'
import { diffFiles, writeFiles, type FileDiff } from './sync.js'
import { classifyRemoved, ledgerOrphans } from './orphan-sync.js'

type SourceKind = FigmaSource['name']

type App = {
  config: FigmaConfig
  cwd: string
  configFile?: string
  createSource(kind?: SourceKind): FigmaSource
  resolveOutDir(override?: string): string
}

type AppOptions = {
  // Reads the capture directory without checking it against upstream.lock.json. Only tests over the synthetic
  // capture and the retirement cleanup's staged copy of an already verified cache pass it.
  unverifiedSnapshot?: boolean
}

// `configFile` lets a consuming project point at a config that lives elsewhere. Every path in the config
// (snapshotDir, outDir, contract.mappingRoot) is relative to the config file, not to the process.
async function createApp(
  cwd = process.cwd(),
  configFile?: string,
  options: AppOptions = {}
): Promise<App> {
  const loadedConfigFile = path.resolve(configFile ?? path.join(cwd, 'figma.config.json'))
  const config = configFile ? await loadConfigFile(configFile) : await loadConfig(cwd)
  if (configFile) cwd = path.dirname(path.resolve(configFile))
  return {
    config,
    configFile: loadedConfigFile,
    cwd,
    // `rest` reads the live file for local inspection only (FIGMA_TOKEN and FIGMA_FILE_KEY from the environment).
    // Everything that writes tracked files reads the verified fetch-source cache: see runSync.
    createSource(kind = 'snapshot') {
      if (kind === 'rest')
        return new RestSource(new FigmaClient(new EnvTokenProvider()), requireFileKey())
      const dir = path.resolve(cwd, config.snapshotDir)
      const lock = config.snapshotLock && path.resolve(cwd, config.snapshotLock)
      // Every command, the MCP server included, reads only the cache fetch-source verified, so a config that
      // names no lock fails here instead of reading whatever the directory holds.
      if (!lock && !options.unverifiedSnapshot)
        throw new FdsCacheError(
          `${path.basename(loadedConfigFile)} names no snapshotLock, so the Figma capture cannot be verified against upstream.lock.json: set snapshotLock and run \`pnpm figma fetch-source\``
        )
      return new SnapshotSource(
        dir,
        configuredPages(config)
          .filter((page) => page.id === null)
          .map((page) => page.name),
        lock && !options.unverifiedSnapshot ? () => verifyFdsCache(dir, lock) : undefined
      )
    },
    resolveOutDir: (override) => path.resolve(cwd, override ?? config.outDir)
  }
}

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
    if (
      outDir !== (await canonicalPath(path.join(root, GENERATED))) ||
      (await canonicalPath(path.resolve(app.cwd, app.config.snapshotDir))) !==
        (await canonicalPath(path.join(root, SNAPSHOT)))
    )
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
