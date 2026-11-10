import path from 'node:path'
import type { App } from './app.js'
import type { GeneratedFile } from './generators/types.js'
import { LEDGER, effectiveTree, findConsumers } from './guard/consumers.js'
import { canonicalPath, readBytesIfExists, resolveContained } from './guard/fs-safe.js'
import { loadLedger, recordOrphans, sha256 } from './guard/orphaned-assets.js'
import {
  GENERATED,
  SNAPSHOT,
  buildOwnership,
  ownerOf,
  type ComponentOwner
} from './guard/ownership.js'
import { diffFiles, type FileDiff } from './sync.js'

const ASSETS = 'components/assets/'

// The ledger describes the repository's own generated and snapshot directories; any other output is not
// tracked by it.
async function usesRepositoryDirectories(app: App, root: string, outDir: string) {
  return (
    (await canonicalPath(outDir)) === (await canonicalPath(path.join(root, GENERATED))) &&
    (await canonicalPath(path.resolve(app.cwd, app.config.snapshotDir))) ===
      (await canonicalPath(path.join(root, SNAPSHOT)))
  )
}

// Native owners of the assets this generation is about to stop producing, read before anything is written.
async function ownersBefore(
  app: App,
  root: string,
  outDir: string,
  files: GeneratedFile[]
): Promise<Map<string, ComponentOwner[]> | undefined> {
  if (!(await usesRepositoryDirectories(app, root, outDir))) return undefined
  const stale = (await diffFiles(outDir, files)).removed.filter((file) => file.startsWith(ASSETS))
  if (!stale.length) return new Map()
  const ownership = await buildOwnership(root)
  const owners = new Map<string, ComponentOwner[]>()
  for (const file of stale) {
    const repoPath = `${GENERATED}/${file}`
    const owner = ownerOf(ownership, repoPath)
    if (
      owner &&
      'aggregate' in owner &&
      owner.aggregate === 'assets' &&
      !owner.foreignUsers?.length
    )
      owners.set(repoPath, owner.users ?? [])
  }
  return owners
}

async function record(
  root: string,
  diff: FileDiff,
  owners: Map<string, ComponentOwner[]> | undefined
): Promise<string[]> {
  if (!owners?.size) return []
  const candidates = diff.removed
    .filter((file) => file.startsWith(ASSETS))
    .map((file) => `${GENERATED}/${file}`)
  return recordOrphans(root, candidates, owners)
}

// Splits `removed` into ledgered orphans that still hold (retained) and real drift. A ledger entry whose file
// is gone, changed or used again is reported as an issue, so it can never hide drift.
async function classifyRemoved(
  app: App,
  root: string,
  outDir: string,
  diff: FileDiff
): Promise<{ removed: string[]; retained: string[]; issues: string[] }> {
  if (!(await usesRepositoryDirectories(app, root, outDir)))
    return { removed: diff.removed, retained: [], issues: [] }
  let entries
  try {
    entries = (await loadLedger(root)).orphaned
  } catch (error) {
    return { removed: diff.removed, retained: [], issues: [`${LEDGER}: ${String(error)}`] }
  }
  if (!entries.length) return { removed: diff.removed, retained: [], issues: [] }
  const tree = await effectiveTree(root)
  const stale = new Set(diff.removed)
  const retained: string[] = []
  const issues: string[] = []
  for (const entry of entries) {
    const relative = entry.path.slice(`${GENERATED}/`.length)
    const bytes = await readBytesIfExists(await resolveContained(root, entry.path))
    if (!bytes) issues.push(`${entry.path}: ledgered asset is missing, finish the drain`)
    else if (sha256(bytes) !== entry.sha256)
      issues.push(`${entry.path}: ledgered asset bytes changed`)
    else if (findConsumers(tree, entry.path).length)
      issues.push(`${entry.path}: ledgered asset is used again, reactivate it`)
    else if (!stale.has(relative)) issues.push(`${entry.path}: ledgered asset is generated again`)
    else retained.push(relative)
  }
  return {
    removed: diff.removed.filter((file) => !retained.includes(file)),
    retained,
    issues
  }
}

const ledgerOrphans = { ownersBefore, record }
export { classifyRemoved, ledgerOrphans }
