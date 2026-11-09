import { rm } from 'node:fs/promises'
import path from 'node:path'
import { aggregateKind, entryBelongsTo, parseAggregate } from './aggregates.js'
import { removeAggregateEntries } from './aggregate-edits.js'
import { inCaptureCache, retiredArtifactPresent } from './reparented-variants.js'
import { listFiles, readBytesIfExists, resolveContained, toPosix } from './fs-safe.js'
import {
  GUARDED_DIRECTORIES,
  GUARDED_FILES,
  GENERATED,
  buildOwnership,
  componentOwner,
  componentNameOfFile,
  ownerOf,
  type Ownership
} from './ownership.js'
import { LIBRARY_GENERATED, libraryOwned, removeLibraryEntries, retiredNames } from './library.js'
import { isRetired, loadRetirements, type Retirements } from './retirements.js'

type Target = { pageId: string; componentId?: string }
type RetirementPlan = { delete: string[]; rejected: { path: string; reason: string }[] }

function componentForArtifact(ownership: Ownership, file: string): string | undefined {
  const owner = ownerOf(ownership, file)
  return owner && 'pageId' in owner && owner.componentId
    ? ownership.componentById.get(owner.componentId)
    : undefined
}

async function retirementArtifacts(
  root: string,
  target: Target,
  allRetirements?: Retirements
): Promise<{ owned: string[]; sharedExclusions: string[] }> {
  const ownership = await buildOwnership(root)
  const names = new Set(retiredNames(ownership, target))
  const owned = new Set<string>()
  const sharedExclusions = new Set<string>()
  const files = new Set<string>(GUARDED_FILES)
  for (const directory of [...GUARDED_DIRECTORIES, LIBRARY_GENERATED])
    for (const file of await listFiles(path.join(root, directory)))
      files.add(toPosix(path.relative(root, file)))
  const existing = allRetirements ?? (await loadRetirements(root))
  for (const file of files) {
    if (path.basename(file) === '.gitkeep') continue
    const bytes = await readBytesIfExists(await resolveContained(root, file))
    if (bytes === undefined) continue
    const kind = aggregateKind(file)
    if (
      kind === 'snapshot' &&
      target.componentId === undefined &&
      ownerOf(ownership, file) &&
      'pageId' in ownerOf(ownership, file)! &&
      (ownerOf(ownership, file) as { pageId: string }).pageId === target.pageId
    )
      owned.add(file)
    else if (kind && kind !== 'retirements') {
      const parsed = parseAggregate(kind, bytes.toString('utf8'), ownership)
      for (const [key, entry] of parsed?.entries ?? [])
        if (entryBelongsTo(key, entry, target, ownership)) owned.add(`${file}#${key}`)
      // The library's assets sit at components/assets, at the root of this layout (layout.ts).
    } else if (`/${file}`.includes('/components/assets/')) {
      const users = ownership.assetUsers.get(file) ?? []
      if (!users.length) continue
      if (!users.some((source) => names.has(componentNameOfFile(ownership, source) ?? ''))) continue
      const allRetired = users.every((source) => {
        const name = componentNameOfFile(ownership, source)
        if (name && names.has(name)) return true
        const owner = componentOwner(ownership, name)
        return owner !== null && isRetired(existing, owner.pageId, owner.componentId)
      })
      if (allRetired) owned.add(file)
      else sharedExclusions.add(file)
    } else {
      const owner = ownerOf(ownership, file)
      if (
        owner &&
        'pageId' in owner &&
        owner.pageId === target.pageId &&
        (target.componentId === undefined || owner.componentId === target.componentId)
      )
        owned.add(file)
    }
  }
  for (const artifact of await libraryOwned(root, ownership, target)) owned.add(artifact)
  return { owned: [...owned].sort(), sharedExclusions: [...sharedExclusions].sort() }
}

async function computeOwned(root: string, target: Target): Promise<string[]> {
  return (await retirementArtifacts(root, target)).owned
}

async function planRetirements(root: string, retirements: Retirements): Promise<RetirementPlan> {
  const ownership = await buildOwnership(root)
  const result: RetirementPlan = { delete: [], rejected: [] }
  for (const record of retirements.retired) {
    const confirmed = new Set((await retirementArtifacts(root, record, retirements)).owned)
    for (const artifact of record.owned) {
      if (!(await retiredArtifactPresent(root, artifact, ownership, record, retirements))) continue
      if (record.sharedExclusions.includes(artifact))
        result.rejected.push({ path: artifact, reason: 'listed as shared with live items' })
      else if (!confirmed.has(artifact))
        result.rejected.push({
          path: artifact,
          reason: 'not owned by the retired page or component'
        })
      else if (!artifact.includes('#')) result.delete.push(artifact)
    }
  }
  return { delete: [...new Set(result.delete)], rejected: result.rejected }
}

async function pendingRetirements(root: string, retirements: Retirements): Promise<string[]> {
  const ownership = await buildOwnership(root)
  const pending: string[] = []
  for (const record of retirements.retired)
    for (const artifact of record.owned)
      if (await retiredArtifactPresent(root, artifact, ownership, record, retirements))
        pending.push(artifact)
  return pending
}

async function applyRetirements(root: string, retirements: Retirements): Promise<string[]> {
  const plan = await planRetirements(root, retirements)
  if (plan.rejected.length)
    throw new Error(
      `Retirement records claim artifacts they do not own:\n${plan.rejected.map((item) => `${item.path}: ${item.reason}`).join('\n')}`
    )
  // Only tracked outputs change: the capture cache keeps the retired identities (inCaptureCache).
  const owned = retirements.retired.flatMap((record) =>
    record.owned.filter(
      (artifact) => !inCaptureCache(artifact) && !record.sharedExclusions.includes(artifact)
    )
  )
  const manifest = (artifact: string) =>
    artifact.split('#')[0] === `${GENERATED}/components/manifest.ts`
  for (const file of plan.delete) await rm(await resolveContained(root, file))
  const entries = [
    ...(await removeAggregateEntries(
      root,
      owned.filter((artifact) => !manifest(artifact))
    )),
    ...(await removeLibraryEntries(root, owned)),
    ...(await removeAggregateEntries(root, owned.filter(manifest)))
  ]
  return [...plan.delete, ...entries]
}
export {
  computeOwned,
  retirementArtifacts,
  planRetirements,
  pendingRetirements,
  applyRetirements,
  componentForArtifact
}
export type { RetirementPlan }
