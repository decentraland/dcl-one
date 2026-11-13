import { GENERATED } from './ownership.js'
import { previewCleanupPlan } from './preview-cleanup.js'
import { invalidFingerprints } from './fingerprints.js'
import { sharedInputConflicts } from './shared-inputs.js'
import { planRetirements, componentForArtifact } from './apply-retirements.js'
import {
  inCaptureCache,
  isReparentingRemap,
  retiredArtifactPresent
} from './reparented-variants.js'
import { loadRetirements } from './retirements.js'
import { CACHED_LEDGERS, LEDGER, RETIREMENTS } from './consumers.js'
import { checkLedger, type Ledger } from './orphaned-assets.js'
import { publicExportSet } from './exports.js'
import path from 'node:path'
import { entryBelongsTo, parseAggregate, aggregateKind } from './aggregates.js'
import { LIBRARY_INDEX, LIBRARY_PROMOTION, libraryEntryPresent, retiredNames } from './library.js'
import { listFiles, readBytesIfExists, resolveContained, toPosix } from './fs-safe.js'
import {
  AGGREGATES,
  GUARDED_DIRECTORIES,
  GUARDED_FILES,
  SNAPSHOT,
  buildOwnership,
  describeRemap,
  mergeOwnership,
  ownerOf,
  type Owner,
  type Ownership
} from './ownership.js'
import type { Retirements } from './retirements.js'
import { metadataDiff, type MetadataChange } from './metadata-diagnostics.js'
import {
  COMPONENT_DERIVED_CATALOG_FOUNDATIONS,
  derivedFieldLabel,
  recomputeDerivedCatalogMetadata,
  type DerivedCatalogMetadata
} from './derived-metadata.js'

type RunKind = 'automated' | 'developer'

// What the staged tree contains, declared by whoever built it. A `partial` stage holds only what its run
// publishes: library paths it does not stage keep their base bytes and are never published. A `complete`
// stage is the whole result, so a library path missing from it is a removal.
type StageCompleteness = 'partial' | 'complete'

type ViolationKind =
  | 'removed'
  | 'out-of-scope-change'
  | 'aggregate-entry-changed'
  | 'retirement-edit'
  | 'pending-retirement'
  | 'orphan-ledger'
  | 'unreadable'

type Violation = {
  path: string
  owner: Owner | null
  kind: ViolationKind
  cause: string
  metadataChanges?: MetadataChange[]
}

type ScopeInput = {
  baseDir: string
  stagedDir: string
  capturedPageIds: readonly string[]
  runKind: RunKind
  stage: StageCompleteness
  retirements: Retirements
  // True when the run captured a page that feeds every other one (colours, icons, theme).
  foundationsCaptured?: boolean
  // Narrow the watched roots (repository-relative). A run that can only write one folder checks only it.
  directories?: readonly string[]
  files?: readonly string[]
  // Folders where only removals are checked (the promoted library: promotion changes it, never deletes).
  removalsOnly?: readonly string[]
  // Overrides the base ledger read from baseDir (developer cleanup validates against the committed one).
  baseLedger?: Ledger
  // Explicit developer-only cleanup of excluded generated Preview derivatives, never Figma identities.
  prunePreview?: boolean
  // The repository a workspace stage publishes into, for consumers outside the staged roots.
  consumerRoot?: string
}

type ScopeResult = { ok: boolean; violations: Violation[] }

const RETIREMENTS_FILE = RETIREMENTS
// The committed retirement ledger and fetch-source's copy of it in the cache: automation edits neither.
const RETIREMENT_FILES = new Set([RETIREMENTS, CACHED_LEDGERS[1]!])

async function listGuarded(
  root: string,
  directories: readonly string[],
  files: readonly string[]
): Promise<string[]> {
  const found: string[] = [...files]
  for (const directory of directories)
    for (const file of await listFiles(path.join(root, directory)))
      if (path.basename(file) !== '.gitkeep') found.push(toPosix(path.relative(root, file)))
  return found
}

const same = (a: Buffer | undefined, b: Buffer | undefined) =>
  a === undefined || b === undefined ? a === b : a.equals(b)

async function read(root: string, relative: string): Promise<Buffer | undefined> {
  return readBytesIfExists(await resolveContained(root, relative))
}

const READ_CONCURRENCY = 32

// The library is components/ plus its entry at the library root (layout.ts); upstream keeps both inside
// one component-library directory.
const LIBRARY_ROOT = 'components'
const LIBRARY_ENTRY = 'index.ts'
const isLibraryPath = (file: string) =>
  file === LIBRARY_ENTRY || file === LIBRARY_ROOT || file.startsWith(`${LIBRARY_ROOT}/`)
// Upstream's removals-only library root is its library's src folder: the components and the entry, not the
// promotion record beside it. Here that record sits in components/ and the entry at the root, so a
// removals-only components/ covers index.ts and leaves components/promotion.json to the entry checks.
const removalsOnlyCovers = (removalsOnly: readonly string[], file: string) =>
  file === LIBRARY_PROMOTION
    ? false
    : file === LIBRARY_ENTRY
      ? removalsOnly.includes(LIBRARY_ROOT)
      : removalsOnly.some((prefix) => file.startsWith(`${prefix}/`))

const ownerLabel = (owner: Owner | null) =>
  owner === null
    ? 'no owner'
    : 'pageId' in owner
      ? `page ${owner.pageId}${owner.componentId ? ` component ${owner.componentId}` : ''}`
      : owner.aggregate

// Owned artifacts are written as a path, or as `<aggregate file>#<entry key>` for an aggregate entry.
const authorizedRemovals = (retirements: Retirements) =>
  new Set(retirements.retired.flatMap((entry) => entry.owned))

async function checkScope(input: ScopeInput): Promise<ScopeResult> {
  const { baseDir, stagedDir, runKind, retirements } = input
  if (input.stage !== 'partial' && input.stage !== 'complete')
    throw new Error(`Unknown stage completeness: ${String(input.stage)}`)
  const partial = input.stage === 'partial'
  const captured = new Set(input.capturedPageIds)
  const violations: Violation[] = []
  const add = (violation: Violation) => violations.push(violation)
  // The root holding a path's effective staged bytes. Completeness comes from the caller, never from the
  // files present: only in a declared partial stage does a library path the stage lacks keep base bytes.
  const effectiveRoot = async (file: string) =>
    partial && isLibraryPath(file) && (await read(stagedDir, file)) === undefined
      ? baseDir
      : stagedDir
  const readStaged = async (file: string) => read(await effectiveRoot(file), file)

  const stagedRecords = await loadRetirements(stagedDir)
  const consistencyRetirements = stagedRecords.retired.length ? stagedRecords : retirements
  for (const record of (await loadRetirements(baseDir)).retired)
    if (
      (input.files === undefined || input.files.includes(RETIREMENTS_FILE)) &&
      !stagedRecords.retired.some(
        (next) => next.pageId === record.pageId && next.componentId === record.componentId
      )
    )
      add({
        path: RETIREMENTS_FILE,
        owner: { pageId: record.pageId },
        kind: 'retirement-edit',
        cause: 'permanent retirement record was removed or its identity changed'
      })

  const baseOwnership = await buildOwnership(baseDir)
  const stagedOwnership = await buildOwnership(stagedDir)
  const merged = mergeOwnership(baseOwnership, stagedOwnership)
  const { ownership } = merged
  if (runKind === 'automated')
    for (const conflict of await sharedInputConflicts(
      baseDir,
      stagedDir,
      ownership,
      captured,
      // Unstaged library roots of a partial stage equal the base; staged library bytes fail below.
      (input.directories ?? GUARDED_DIRECTORIES).filter(
        (directory) => !(partial && isLibraryPath(directory))
      )
    ))
      add({
        path: conflict.path,
        owner: ownerOf(ownership, conflict.path) ?? { pageId: conflict.pageId },
        kind: 'out-of-scope-change',
        cause: conflict.cause
      })
  const claims = await planRetirements(baseDir, retirements)
  for (const rejected of claims.rejected)
    add({
      path: rejected.path,
      owner: ownerOf(ownership, rejected.path),
      kind: 'out-of-scope-change',
      cause: rejected.reason
    })
  if (runKind === 'automated')
    for (const remap of [
      ...merged.remaps
        .filter((move) => !isReparentingRemap(move, stagedOwnership, consistencyRetirements))
        .map(describeRemap),
      ...merged.ownership.conflicts
    ])
      add({ path: 'ownership', owner: null, kind: 'out-of-scope-change', cause: remap })
  // A retired identity may leave the capture on any run: the cleanup never removes it, a later capture does.
  const removable = new Set(
    [...authorizedRemovals(retirements)].filter(
      (artifact) => runKind === 'developer' || inCaptureCache(artifact)
    )
  )
  if (input.prunePreview) {
    if (runKind !== 'developer' || partial)
      add({
        path: GENERATED,
        owner: null,
        kind: 'out-of-scope-change',
        cause: 'Preview cleanup requires a complete developer stage'
      })
    else {
      try {
        const plan = await previewCleanupPlan(baseDir, stagedDir, true)
        for (const artifact of plan.artifacts) removable.add(artifact)
      } catch (error) {
        add({
          path: GENERATED,
          owner: null,
          kind: 'out-of-scope-change',
          cause: `Preview cleanup validation failed: ${String(error)}`
        })
      }
    }
  }
  const removalsOnly = input.removalsOnly ?? []
  const directories = [...(input.directories ?? GUARDED_DIRECTORIES), ...removalsOnly]
  // A watched library root includes its entry, as upstream's library src folder does (layout.ts).
  const watched = [
    ...(input.files ?? GUARDED_FILES),
    ...(directories.includes(LIBRARY_ROOT) && !(input.files ?? []).includes(LIBRARY_ENTRY)
      ? [LIBRARY_ENTRY]
      : [])
  ]
  // The orphaned asset ledger is validated against the base on every run that watches it or the snapshot it
  // keeps the books for.
  const ledgerWatched =
    watched.includes(LEDGER) ||
    directories.some((directory) => directory === SNAPSHOT || LEDGER.startsWith(`${directory}/`))
  if (ledgerWatched) {
    const ledger = await checkLedger({
      baseDir,
      headDir: stagedDir,
      partial,
      developer: runKind === 'developer',
      captured,
      baseLedger: input.baseLedger,
      consumerRoot: input.consumerRoot
    })
    for (const violation of ledger.violations)
      add({ path: violation.path, owner: null, kind: 'orphan-ledger', cause: violation.cause })
    if (runKind === 'developer') for (const file of ledger.drained) removable.add(file)
  }
  const files =
    path.resolve(baseDir) === path.resolve(stagedDir) && !consistencyRetirements.retired.length
      ? []
      : [
          ...new Set([
            ...(await listGuarded(baseDir, directories, watched)),
            ...(await listGuarded(stagedDir, directories, watched))
          ])
        ].sort()

  // A partial stage never publishes the library, so any library byte it does stage is out of scope.
  if (partial)
    for (const file of await listGuarded(stagedDir, [LIBRARY_ROOT], []))
      if (!same(await read(baseDir, file), await read(stagedDir, file)))
        add({
          path: file,
          owner: ownerOf(ownership, file),
          kind: 'out-of-scope-change',
          cause: 'a partial stage may not add, change or remove library content'
        })

  let derivedCatalog: DerivedCatalogMetadata | Error | undefined
  const sameTree = path.resolve(baseDir) === path.resolve(stagedDir)
  const readPair = async (file: string) => {
    const before = await read(baseDir, file)
    return { file, before, after: sameTree ? before : await readStaged(file) }
  }
  // Each contained read walks every path segment, so reading the tree one file at a time is bound by
  // syscall latency. Read in bounded windows, concurrent within a window, and still check in order.
  async function* pairs() {
    for (let start = 0; start < files.length; start += READ_CONCURRENCY)
      yield* await Promise.all(files.slice(start, start + READ_CONCURRENCY).map(readPair))
  }
  for await (const { file, before, after } of pairs()) {
    if (after !== undefined)
      for (const record of consistencyRetirements.retired) {
        const component = componentForArtifact(ownership, file)
        if (
          component &&
          retiredNames(ownership, record).includes(component) &&
          !record.owned.includes(file) &&
          !record.sharedExclusions.includes(file)
        )
          add({
            path: file,
            owner: { pageId: record.pageId },
            kind: 'pending-retirement',
            cause: 'retired component artifact remains outside the retirement inventory'
          })
      }
    if (same(before, after)) continue
    // Checked as a whole by checkLedger above.
    // The cache's copy of the ledger is pinned by fetch-source's digest table, not by this guard.
    if (file === LEDGER || file === CACHED_LEDGERS[0]) continue
    // Reported once by the partial-stage library check above.
    if (partial && isLibraryPath(file)) continue
    if (file === 'figma/review/src/review/promotion.json' && after)
      for (const invalid of await invalidFingerprints(stagedDir, file))
        add({
          path: file,
          owner: { aggregate: 'promotion-manifest' },
          kind: 'out-of-scope-change',
          cause: `review fingerprint does not match staged input: ${invalid}`
        })
    const owner = ownerOf(ownership, file)
    if (
      runKind === 'automated' &&
      before !== undefined &&
      after !== undefined &&
      ownerOf(baseOwnership, file) === null
    ) {
      add({
        path: file,
        owner: null,
        kind: 'out-of-scope-change',
        cause:
          'existing artifact has no recorded base ownership; staged claims cannot authorize overwrites'
      })
      continue
    }

    if (removalsOnlyCovers(removalsOnly, file)) {
      if (after === undefined && !removable.has(file))
        add({ path: file, owner, kind: 'removed', cause: 'removal without a retirement record' })
      continue
    }

    if (RETIREMENT_FILES.has(file) && runKind === 'automated') {
      add({
        path: file,
        owner,
        kind: 'retirement-edit',
        cause: 'automation may not edit retirements'
      })
      continue
    }

    if (after === undefined) {
      if (!removable.has(file))
        add({ path: file, owner, kind: 'removed', cause: 'removal without a retirement record' })
      continue
    }

    if (runKind === 'developer') {
      checkEntries(
        file,
        before,
        after,
        owner,
        ownership,
        captured,
        true,
        removable,
        add,
        input,
        undefined,
        baseOwnership,
        stagedOwnership
      )
      continue
    }
    if (aggregateKind(file) === 'catalog' && derivedCatalog === undefined)
      derivedCatalog = await recomputeDerivedCatalogMetadata(stagedDir).catch((error: unknown) =>
        error instanceof Error ? error : new Error(String(error))
      )

    const aggregate = aggregateKind(file)
    if (
      aggregate === 'snapshot' &&
      owner !== null &&
      'pageId' in owner &&
      !captured.has(owner.pageId)
    ) {
      add({
        path: file,
        owner,
        kind: 'out-of-scope-change',
        cause: 'snapshot belongs to an unselected page'
      })
      continue
    }
    if (aggregate && isEntryAggregate(aggregate)) {
      checkEntries(
        file,
        before,
        after,
        owner,
        ownership,
        captured,
        false,
        removable,
        add,
        input,
        derivedCatalog,
        baseOwnership,
        stagedOwnership
      )
      continue
    }
    if (owner !== null && 'aggregate' in owner && owner.aggregate === 'assets') {
      if (
        before === undefined &&
        owner.users?.length &&
        !owner.foreignUsers?.length &&
        owner.users.every((user) => user.componentId !== undefined && captured.has(user.pageId))
      )
        continue
      add({
        path: file,
        owner,
        kind: 'out-of-scope-change',
        cause:
          before === undefined
            ? 'shared asset has unselected or unowned consumers'
            : 'existing shared asset bytes must remain unchanged'
      })
      continue
    }
    if (owner !== null && 'pageId' in owner && captured.has(owner.pageId)) continue
    if (
      owner !== null &&
      'aggregate' in owner &&
      owner.aggregate === 'foundations' &&
      input.foundationsCaptured
    )
      continue
    add({
      path: file,
      owner,
      kind: 'out-of-scope-change',
      cause:
        owner === null
          ? 'artifact has no owner among the captured pages'
          : `owned by ${ownerLabel(owner)}, which was not captured in this run`
    })
  }

  for (const entry of consistencyRetirements.retired)
    for (const artifact of entry.owned) {
      const [file] = artifact.split('#')
      const root = await effectiveRoot(file!)
      const parents = root === stagedDir ? stagedOwnership : baseOwnership
      if (
        await retiredArtifactPresent(
          root,
          artifact,
          ownership,
          entry,
          consistencyRetirements,
          parents
        )
      )
        add({
          path: file!,
          owner: { pageId: entry.pageId },
          kind: 'pending-retirement',
          cause: `retired (${entry.name}) but still present: run sync --developer --apply-retirements`
        })
    }

  if ((input.removalsOnly ?? []).some((directory) => directory.startsWith('components'))) {
    const before = await read(baseDir, LIBRARY_INDEX)
    const after = await readStaged(LIBRARY_INDEX)
    if (before)
      for (const key of publicExportSet(before.toString('utf8')))
        if (
          !publicExportSet(after?.toString('utf8') ?? '').has(key) &&
          !removable.has(`${LIBRARY_INDEX}#${key}`)
        )
          add({
            path: LIBRARY_INDEX,
            owner: { aggregate: 'library-index' },
            kind: 'removed',
            cause: `public export ${key} removed without a retirement`
          })
  }
  await retiredStillListed(stagedDir, ownership, consistencyRetirements, add)
  await retiredLibraryAndExclusions(ownership, consistencyRetirements, add, effectiveRoot)

  return { ok: violations.length === 0, violations }
}

// A retired page or component must be gone from the catalog, the manifest and the exports as well, not only
// from disk, or the next regeneration would bring it back.
async function retiredStillListed(
  stagedDir: string,
  ownership: Ownership,
  retirements: Retirements,
  add: (violation: Violation) => void
): Promise<void> {
  if (!retirements.retired.length) return
  for (const aggregate of RETIRABLE_AGGREGATES.filter((kind) => kind !== 'snapshot')) {
    const file = Object.entries(AGGREGATES).find(([, name]) => name === aggregate)![0]
    // The capture cache keeps listing retired items; generation drops them through the ledger.
    if (inCaptureCache(file)) continue
    const text = (await read(stagedDir, file))?.toString('utf8')
    if (text === undefined) continue
    let parsed
    try {
      parsed = parseAggregate(aggregate, text, ownership)
    } catch {
      continue
    }
    for (const [key, entry] of parsed?.entries ?? [])
      for (const record of retirements.retired)
        if (entryBelongsTo(key, entry, record, ownership))
          add({
            path: file,
            owner: { pageId: record.pageId },
            kind: 'pending-retirement',
            cause: `retired ${record.name} is still listed as ${key}`
          })
  }
}

// The aggregates a retired page or component can still be listed in. Page listings are not retired items.
// The library must not export or list a retired component, and what a retirement shares with live
// items must still be there: the cleanup removes the retired item, never its neighbours.
async function retiredLibraryAndExclusions(
  ownership: Ownership,
  retirements: Retirements,
  add: (violation: Violation) => void,
  rootForPath: (file: string) => Promise<string>
): Promise<void> {
  for (const record of retirements.retired) {
    for (const name of retiredNames(ownership, record))
      for (const [file, key] of [
        [LIBRARY_INDEX, `value:${name}`],
        [LIBRARY_INDEX, `type:${name}Props`],
        [LIBRARY_PROMOTION, `component:${name}`]
      ] as const)
        if (await libraryEntryPresent(await rootForPath(file), `${file}#${key}`))
          add({
            path: file,
            owner: { pageId: record.pageId },
            kind: 'pending-retirement',
            cause: `retired ${record.name} is still listed as ${key}`
          })
    for (const shared of record.sharedExclusions)
      if (
        !shared.includes('#') &&
        !retirements.retired.some(
          (entry) => entry.owned.includes(shared) && !entry.sharedExclusions.includes(shared)
        ) &&
        (await read(await rootForPath(shared), shared)) === undefined
      )
        add({
          path: shared,
          owner: { pageId: record.pageId },
          kind: 'removed',
          cause: `shared with live items by the retirement of ${record.name}, but missing`
        })
  }
}

const RETIRABLE_AGGREGATES = [
  'snapshot',
  'promotion-manifest',
  'components-index',
  'components-manifest',
  'catalog',
  'inventory'
]
const ENTRY_AGGREGATES = [
  'library-index',
  'library-promotion',
  'snapshot',
  'promotion-manifest',
  'components-index',
  'components-manifest',
  'catalog',
  'inventory',
  'sync-state',
  'pages',
  'live-source'
]
const isEntryAggregate = (aggregate: string) => ENTRY_AGGREGATES.includes(aggregate)

// The catalog's non-entry metadata may change in an automated run only in two ways: an explicitly listed
// component-derived field equals its recomputation over the full staged inventory, and every other
// foundations field changes only when foundations were captured. Returns whether the rest may differ;
// a derived field that does not match its recomputation is reported here.
function catalogMetadataAllowed(
  file: string,
  beforeRest: string,
  afterRest: string,
  owner: Owner | null,
  foundationsCaptured: boolean,
  derived: DerivedCatalogMetadata | Error | undefined,
  add: (violation: Violation) => void
): boolean {
  const before = JSON.parse(beforeRest) as Record<string, unknown>
  const after = JSON.parse(afterRest) as Record<string, unknown>
  const foundations = (data: Record<string, unknown>) =>
    (data.foundations ?? {}) as Record<string, unknown>
  for (const field of COMPONENT_DERIVED_CATALOG_FOUNDATIONS) {
    const next = JSON.stringify(foundations(after)[field])
    if (JSON.stringify(foundations(before)[field]) === next) continue
    const metadataChanges = metadataDiff(
      foundations(before)[field],
      foundations(after)[field],
      `/foundations/${field}`
    )
    if (derived instanceof Error || derived === undefined) {
      add({
        path: file,
        owner,
        kind: 'aggregate-entry-changed',
        cause: `${derivedFieldLabel(field)} changed but could not be recomputed: ${derived?.message ?? 'no staged inventory'}`,
        metadataChanges
      })
    } else if (JSON.stringify(derived[field]) !== next) {
      add({
        path: file,
        owner,
        kind: 'aggregate-entry-changed',
        cause: `${derivedFieldLabel(field)} does not match its recomputation over the full inventory`,
        metadataChanges
      })
    }
  }
  const remainder = (data: Record<string, unknown>) => {
    const copy = { ...data }
    if (foundationsCaptured) delete copy.foundations
    else {
      const rest = { ...foundations(data) }
      for (const field of COMPONENT_DERIVED_CATALOG_FOUNDATIONS) delete rest[field]
      if (data.foundations !== undefined) copy.foundations = rest
    }
    return JSON.stringify(copy)
  }
  // A mismatched derived field is already reported; the remainder decides the generic violation.
  return remainder(before) === remainder(after)
}

// Entry-level comparison of a shared aggregate. Unselected entries must be byte-identical and keep their
// relative order; nothing may disappear without a retirement record.
function checkEntries(
  file: string,
  before: Buffer | undefined,
  after: Buffer,
  owner: Owner | null,
  ownership: Ownership,
  captured: ReadonlySet<string>,
  developer: boolean,
  removable: ReadonlySet<string>,
  add: (violation: Violation) => void,
  input: ScopeInput,
  derivedCatalog?: DerivedCatalogMetadata | Error,
  baseOwnership: Ownership = ownership,
  stagedOwnership: Ownership = ownership
): void {
  const aggregate = aggregateKind(file)
  let parsedAfter, parsedBefore
  try {
    parsedAfter = aggregate
      ? parseAggregate(aggregate, after.toString('utf8'), stagedOwnership)
      : undefined
    parsedBefore =
      aggregate && before
        ? parseAggregate(aggregate, before.toString('utf8'), baseOwnership)
        : undefined
  } catch (error) {
    add({
      path: file,
      owner,
      kind: 'unreadable',
      cause: `aggregate could not be parsed: ${String(error)}`
    })
    return
  }

  if (!aggregate || !parsedAfter) {
    if (!developer)
      add({
        path: file,
        owner,
        kind: 'out-of-scope-change',
        cause: 'shared artifact changed outside an entry-level merge'
      })
    return
  }
  if (!parsedBefore) {
    const metadata =
      aggregate === 'live-source'
        ? (JSON.parse(parsedAfter.rest) as Record<string, unknown>)
        : undefined
    if (
      metadata &&
      metadata.schema === 1 &&
      Object.keys(metadata).length === 1 &&
      [...parsedAfter.entries.values()].every(
        (entry) => entry.owner !== null && captured.has(entry.owner)
      )
    )
      return
    // An aggregate that did not exist is not a merge: an automated run may not create one.
    if (!developer)
      add({
        path: file,
        owner,
        kind: 'out-of-scope-change',
        cause: 'aggregate created by an automated run'
      })
    return
  }

  if (
    !developer &&
    parsedBefore.rest !== parsedAfter.rest &&
    !(aggregate === 'snapshot' && owner !== null && 'pageId' in owner && captured.has(owner.pageId))
  ) {
    const restAllowed =
      aggregate === 'catalog' &&
      catalogMetadataAllowed(
        file,
        parsedBefore.rest,
        parsedAfter.rest,
        owner,
        !!input.foundationsCaptured,
        derivedCatalog,
        add
      )
    if (!restAllowed)
      add({
        path: file,
        owner,
        kind: 'aggregate-entry-changed',
        cause: 'non-entry part of the aggregate changed',
        ...(aggregate === 'catalog'
          ? {
              metadataChanges: metadataDiff(
                JSON.parse(parsedBefore.rest),
                JSON.parse(parsedAfter.rest)
              )
            }
          : {})
      })
  }

  for (const [key, entry] of parsedBefore.entries) {
    const entryOwner =
      ownerOf(ownership, `${file}#${key}`) ?? (entry.owner ? { pageId: entry.owner } : owner)
    const next = parsedAfter.entries.get(key)
    if (next === undefined) {
      if (aggregate === 'inventory' && key.includes(':component:')) {
        const page = parsedAfter.entries.get(key.split(':component:')[0]!)
        if (
          page &&
          (JSON.parse(page.value) as { componentIds?: string[] }).componentIds === undefined
        )
          continue
      }
      if (!removable.has(`${file}#${key}`))
        add({
          path: file,
          owner: entryOwner,
          kind: 'removed',
          cause: `entry ${key} was removed without a retirement record`
        })
    } else if (
      !developer &&
      next.value !== entry.value &&
      !(entry.owner !== null && captured.has(entry.owner))
    )
      add({
        path: file,
        owner: entryOwner,
        kind: 'aggregate-entry-changed',
        cause: `entry ${key} belongs to ${entry.owner ?? 'no page'}, which was not captured`
      })
  }
  if (developer) return

  for (const [key, entry] of parsedAfter.entries)
    if (!parsedBefore.entries.has(key) && !(entry.owner !== null && captured.has(entry.owner)))
      add({
        path: file,
        owner,
        kind: 'aggregate-entry-changed',
        cause: `new entry ${key} belongs to ${entry.owner ?? 'no page'}, which was not captured`
      })

  // Each key is classified once, by its committed entry when it has one: both sides are parsed with
  // their own ownership, and a renamed component's old keys have no staged owner. Staged ownership
  // never frees a committed entry whose owner is null or uncaptured.
  const committed = parsedBefore
  const proposed = parsedAfter
  const isFrozen = (key: string) => {
    const entryOwner = (committed.entries.get(key) ?? proposed.entries.get(key))?.owner
    return !(entryOwner !== null && entryOwner !== undefined && captured.has(entryOwner))
  }
  const keptAfter = new Set(proposed.order)
  const orderBefore = committed.order.filter((key) => isFrozen(key) && keptAfter.has(key))
  const orderAfter = proposed.order.filter((key) => isFrozen(key) && committed.entries.has(key))
  if (orderBefore.join('\n') !== orderAfter.join('\n'))
    add({
      path: file,
      owner,
      kind: 'aggregate-entry-changed',
      cause: 'the order of unselected entries changed'
    })
}

export { checkScope }
export type { RunKind, ScopeInput, ScopeResult, StageCompleteness, Violation, ViolationKind }
