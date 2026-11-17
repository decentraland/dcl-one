import { createHash } from 'node:crypto'
import { readdir, rm } from 'node:fs/promises'
import path from 'node:path'
import { z } from 'zod'
import { LEDGER, effectiveTree, findConsumers, type Tree } from './consumers.js'
import { atomicWrite, readBytesIfExists, readIfExists, resolveContained } from './fs-safe.js'
import { GENERATED, SNAPSHOT, buildOwnership, ownerOf, type ComponentOwner } from './ownership.js'

// An asset the generation stopped producing after a recapture, kept byte-identical because automation never
// deletes. The ledger records only what can be checked against the tree: everything else (who owned it, which
// recapture changed them) is derived from the base and head trees, never trusted from the file.

const ASSET_PREFIX = `${GENERATED}/components/assets/`
const SHA256 = /^[0-9a-f]{64}$/

const entrySchema = z.strictObject({ path: z.string().min(1), sha256: z.string().regex(SHA256) })
const ledgerSchema = z
  .strictObject({ schema: z.literal(1), orphaned: z.array(entrySchema) })
  .superRefine((ledger, context) => {
    const paths = ledger.orphaned.map((entry) => entry.path)
    if (new Set(paths).size !== paths.length)
      context.addIssue({ code: 'custom', message: 'Duplicate orphaned asset entry' })
    if (paths.some((value, index) => index > 0 && value < paths[index - 1]!))
      context.addIssue({ code: 'custom', message: 'Orphaned asset entries must be sorted by path' })
  })

type LedgerEntry = z.infer<typeof entrySchema>
type Ledger = z.infer<typeof ledgerSchema>

const sha256 = (bytes: Buffer) => createHash('sha256').update(bytes).digest('hex')

function isAssetPath(file: string): boolean {
  if (!file.startsWith(ASSET_PREFIX)) return false
  const name = file.slice(ASSET_PREFIX.length)
  return name.length > 0 && !name.includes('/') && !name.includes('\\') && name !== '..'
}

async function loadLedger(root: string): Promise<Ledger> {
  const text = await readIfExists(path.join(root, LEDGER))
  if (text === undefined) return { schema: 1, orphaned: [] }
  return ledgerSchema.parse(JSON.parse(text))
}

function serializeLedger(entries: LedgerEntry[]): string {
  const orphaned = [...new Map(entries.map((entry) => [entry.path, entry])).values()].sort(
    (a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0)
  )
  return JSON.stringify(ledgerSchema.parse({ schema: 1, orphaned }), null, 2) + '\n'
}

async function writeLedger(root: string, entries: LedgerEntry[]): Promise<void> {
  await atomicWrite(await resolveContained(root, LEDGER), serializeLedger(entries))
}

// Fails closed on a path that escapes its root or a symlink that leaves it.
async function assetBytes(root: string, file: string): Promise<Buffer | undefined> {
  if (!isAssetPath(file)) throw new Error(`Not a generated asset path: ${file}`)
  return readBytesIfExists(await resolveContained(root, file))
}

// The components whose generated output referenced the asset in that tree: its native owners.
async function nativeOwners(
  root: string,
  file: string
): Promise<{ owners: ComponentOwner[]; foreign: string[] }> {
  const owner = ownerOf(await buildOwnership(root), file)
  if (!owner || !('aggregate' in owner) || owner.aggregate !== 'assets')
    return { owners: [], foreign: [] }
  return { owners: owner.users ?? [], foreign: owner.foreignUsers ?? [] }
}

// Snapshot entries by component ID, so a recapture shows as a changed entry.
async function snapshotEntries(root: string): Promise<Map<string, string>> {
  const directory = path.join(root, SNAPSHOT, 'components')
  const entries = new Map<string, string>()
  let names: string[]
  try {
    names = await readdir(directory)
  } catch {
    return entries
  }
  for (const name of names.filter((value) => value.endsWith('.json'))) {
    const data = JSON.parse((await readIfExists(path.join(directory, name))) ?? '{}') as {
      page?: { id?: string }
      components?: { id?: string }[]
    }
    for (const component of data.components ?? [])
      if (component.id)
        entries.set(`${data.page?.id ?? ''}|${component.id}`, JSON.stringify(component))
  }
  return entries
}

type LedgerCheck = { violations: { path: string; cause: string }[]; drained: Set<string> }

// Validates the ledger transition from the base tree to the head tree, for any run. New entries must be real
// recapture orphans; existing ones are immutable and leave only by reactivation or drain.
async function checkLedger(input: {
  baseDir: string
  headDir: string
  partial: boolean
  developer: boolean
  // The pages an automated run captured: its additions may only orphan assets those pages owned.
  captured?: ReadonlySet<string>
  // The base ledger when the base tree is a working copy: a drain interrupted after dropping an entry
  // stays authorized by the committed record.
  baseLedger?: Ledger
  // The repository the head stage will be published into. A workspace stage holds only the managed
  // roots, so consumers elsewhere (docs, scripts, foreign code) are read from here.
  consumerRoot?: string
  tree?: Tree
}): Promise<LedgerCheck> {
  const { baseDir, headDir, developer } = input
  const violations: LedgerCheck['violations'] = []
  const drained = new Set<string>()
  const reject = (file: string, cause: string) => violations.push({ path: file, cause })
  let base: Ledger
  let head: Ledger
  try {
    base = input.baseLedger ?? (await loadLedger(baseDir))
    head = await loadLedger(headDir)
  } catch (error) {
    reject(LEDGER, `invalid orphaned asset ledger: ${String(error)}`)
    return { violations, drained }
  }
  const baseEntries = new Map(base.orphaned.map((entry) => [entry.path, entry]))
  const headEntries = new Map(head.orphaned.map((entry) => [entry.path, entry]))
  if (baseEntries.size === 0 && headEntries.size === 0) return { violations, drained }
  let tree: Tree
  try {
    // A partial stage publishes over the base; a complete one is the whole head tree.
    tree =
      input.tree ??
      (input.consumerRoot
        ? await effectiveTree(input.consumerRoot, headDir)
        : input.partial
          ? await effectiveTree(baseDir, headDir)
          : await effectiveTree(headDir))
  } catch (error) {
    for (const file of new Set([...baseEntries.keys(), ...headEntries.keys()]))
      reject(file, `consumers could not be checked: ${String(error)}`)
    return { violations, drained }
  }
  let baseSnapshot: Map<string, string> | undefined
  let headSnapshot: Map<string, string> | undefined

  for (const [file, entry] of headEntries) {
    const previous = baseEntries.get(file)
    if (previous) {
      if (previous.sha256 !== entry.sha256) reject(file, 'an existing ledger entry was edited')
      continue
    }
    if (!isAssetPath(file)) {
      reject(file, 'ledger entry is not a generated asset path')
      continue
    }
    let before: Buffer | undefined
    let after: Buffer | undefined
    try {
      before = await assetBytes(baseDir, file)
      after = await assetBytes(headDir, file)
    } catch (error) {
      reject(file, `ledger entry path is not contained: ${String(error)}`)
      continue
    }
    if (!before || sha256(before) !== entry.sha256) {
      reject(file, 'the base asset bytes do not match the ledgered sha256')
      continue
    }
    if (!after || !after.equals(before)) {
      reject(file, 'a newly ledgered asset must keep its base bytes')
      continue
    }
    const { owners, foreign } = await nativeOwners(baseDir, file)
    if (!owners.length || foreign.length) {
      reject(file, 'the asset had no native generated owner at base')
      continue
    }
    const outside =
      !developer && input.captured
        ? owners.filter((owner) => !input.captured!.has(owner.pageId))
        : []
    if (outside.length) {
      reject(
        file,
        `owned by pages this run did not capture: ${[...new Set(outside.map((owner) => owner.pageId))].join(', ')}`
      )
      continue
    }
    const consumers = findConsumers(tree, file)
    if (consumers.length) {
      reject(file, `the asset is still used by ${consumers.map((item) => item.file).join(', ')}`)
      continue
    }
    baseSnapshot ??= await snapshotEntries(baseDir)
    headSnapshot ??= await snapshotEntries(headDir)
    const unchanged = owners.filter(
      (owner) =>
        baseSnapshot!.get(`${owner.pageId}|${owner.componentId}`) ===
        headSnapshot!.get(`${owner.pageId}|${owner.componentId}`)
    )
    if (unchanged.length)
      reject(
        file,
        `no recapture changed its owner ${unchanged.map((owner) => owner.componentId).join(', ')}`
      )
  }

  for (const [file, entry] of baseEntries) {
    if (headEntries.has(file)) {
      if (!(await assetBytes(headDir, file).catch(() => undefined)))
        reject(file, 'a ledgered asset was deleted without removing its entry')
      continue
    }
    if (!developer) {
      reject(file, 'only a developer reactivation or drain may remove a ledger entry')
      continue
    }
    const before = await assetBytes(baseDir, file).catch(() => undefined)
    const after = await assetBytes(headDir, file).catch(() => undefined)
    const used = findConsumers(tree, file).length > 0
    if (after) {
      if (sha256(after) !== entry.sha256 || !used)
        reject(file, 'a reactivation needs the ledgered bytes and at least one consumer')
      continue
    }
    if (before && sha256(before) !== entry.sha256)
      reject(file, 'the drained asset did not match its base ledger entry')
    else if (used) reject(file, 'a drained asset is still used')
    else drained.add(file)
  }
  return { violations, drained }
}

// Ledgers the assets a sync stopped generating, when each had native owners and nothing uses it any more.
// It never deletes and never edits an existing entry; the guard re-checks every addition against the base.
async function recordOrphans(
  root: string,
  candidates: string[],
  ownersBefore: Map<string, ComponentOwner[]>
): Promise<string[]> {
  const ledger = await loadLedger(root)
  const known = new Set(ledger.orphaned.map((entry) => entry.path))
  const pending = candidates.filter((file) => isAssetPath(file) && !known.has(file))
  if (!pending.length) return []
  const tree = await effectiveTree(root)
  const added: LedgerEntry[] = []
  for (const file of pending) {
    if (!ownersBefore.get(file)?.length) continue
    const bytes = await assetBytes(root, file)
    if (!bytes || findConsumers(tree, file).length) continue
    added.push({ path: file, sha256: sha256(bytes) })
  }
  if (added.length) await writeLedger(root, [...ledger.orphaned, ...added])
  return added.map((entry) => entry.path)
}

// The developer transitions. An asset used again (by any consumer, a regenerated component included) is
// reactivated: its entry goes, its bytes stay. An unused one is drained: file and entry go together.
// Recoverable after an interruption in either order: entries come from the working ledger and from the
// committed one, and the file is deleted before its entry is dropped.
async function drainLedger(
  root: string,
  committed: Ledger,
  consumerRoot?: string
): Promise<{ drained: string[]; reactivated: string[] }> {
  const working = await loadLedger(root)
  // Only committed entries authorize a transition. A working-only entry is a pending addition the PR guard
  // has not validated yet: it is left as it is, never drained.
  const entries = new Map(committed.orphaned.map((entry) => [entry.path, entry]))
  // A stage of the managed roots is read over the whole repository it publishes into.
  const tree = consumerRoot ? await effectiveTree(consumerRoot, root) : await effectiveTree(root)
  const drained: string[] = []
  const reactivated: string[] = []
  const keep: LedgerEntry[] = []
  for (const entry of entries.values()) {
    const bytes = await assetBytes(root, entry.path)
    const used = findConsumers(tree, entry.path).length > 0
    if (bytes && sha256(bytes) !== entry.sha256)
      throw new Error(`Ledgered asset bytes changed: ${entry.path}`)
    if (used) {
      if (!bytes) throw new Error(`A used asset is missing: ${entry.path}`)
      reactivated.push(entry.path)
      continue
    }
    if (bytes) await rm(await resolveContained(root, entry.path))
    drained.push(entry.path)
  }
  const remaining = working.orphaned.filter(
    (entry) => !drained.includes(entry.path) && !reactivated.includes(entry.path)
  )
  keep.push(...remaining)
  if (working.orphaned.length !== keep.length || (await readIfExists(path.join(root, LEDGER))))
    await writeLedger(root, keep)
  return { drained, reactivated }
}

export {
  ASSET_PREFIX,
  checkLedger,
  drainLedger,
  isAssetPath,
  loadLedger,
  recordOrphans,
  serializeLedger,
  sha256
}
export type { Ledger, LedgerEntry }
