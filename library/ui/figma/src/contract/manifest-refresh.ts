// Builds the mapping manifest from the live file: one REST read of the nodes the mappings name, and one read of
// the pages that hold them. It runs only where FIGMA_TOKEN and FIGMA_FILE_KEY are set; everything else reads the
// tracked manifest.
import path from 'node:path'
import type { FigmaClient } from '../figma/client.js'
import type { FigmaSet } from './figma-index.js'
import { findMappingFiles } from './library.js'
import { readMappingFile } from './mapping-file.js'
import { orphanedSets, parseMappingManifest, serializeMappingManifest } from './mapping-manifest.js'

type RestProperty = { type?: unknown; defaultValue?: unknown; variantOptions?: unknown }
type RestNode = {
  id?: unknown
  name?: unknown
  type?: unknown
  componentPropertyDefinitions?: Record<string, RestProperty>
  children?: RestNode[]
}

/** Ids asked for in one request; the API takes them in the query string. */
const NODE_BATCH = 50
// The page read returns every listed node with its whole subtree (about 12 MB for the library's mappings), so it
// gets the client's long-read budget.
const PAGE_READ_TIMEOUT_MS = 240000

// Errors name a node only by the id a mapping gave, and a variant or property only by its position: names read
// from the file are checked only once the set is complete (checkedLiveSets), so they never reach a message.
const text = (value: unknown, what: string): string => {
  if (typeof value !== 'string') throw new Error(`${what} has no name`)
  return value
}

// A node type as the REST API spells it; anything else is not repeated in a message.
const NODE_TYPE = /^[A-Z][A-Z_]{0,39}$/
const typeLabel = (value: unknown) =>
  typeof value === 'string' && NODE_TYPE.test(value) ? value : 'another kind of node'

/** `Size=Large, State=Hover` as Figma names a variant: its properties by name. */
function variantProps(name: string): Record<string, string> {
  const props: Record<string, string> = {}
  for (const part of name.split(',')) {
    const at = part.indexOf('=')
    if (at < 0) continue
    props[part.slice(0, at).trim()] = part.slice(at + 1).trim()
  }
  return props
}

function properties(node: RestNode, nodeId: string): FigmaSet['properties'] {
  const out: FigmaSet['properties'] = {}
  for (const [index, [name, definition]] of Object.entries(
    node.componentPropertyDefinitions ?? {}
  ).entries()) {
    if (typeof definition?.type !== 'string')
      throw new Error(`Figma node ${nodeId}: property ${index + 1} has no type`)
    const value = definition.defaultValue
    const options = definition.variantOptions
    out[name] = {
      type: definition.type,
      ...(typeof value === 'string' || typeof value === 'boolean' ? { default: value } : {}),
      ...(Array.isArray(options) ? { options: options.map(String) } : {})
    }
  }
  return out
}

/** One mapped node as the mapping check reads it. Only a component set or a single component is one. */
function figmaSetFromNode(nodeId: string, node: RestNode | null | undefined): FigmaSet {
  if (!node) throw new Error(`Figma node ${nodeId} is not in the file`)
  const name = text(node.name, `Figma node ${nodeId}`)
  if (node.type === 'COMPONENT_SET') {
    const variants = (node.children ?? [])
      .filter((child) => child.type === 'COMPONENT')
      .map((child, index) => {
        const variantName = text(child.name, `Variant ${index + 1} of Figma node ${nodeId}`)
        return {
          id: text(child.id, `Variant ${index + 1} of Figma node ${nodeId}`),
          name: variantName,
          props: variantProps(variantName)
        }
      })
    if (variants.length === 0) throw new Error(`Figma node ${nodeId} has no variants`)
    return {
      setId: nodeId,
      name,
      page: '',
      kind: 'set',
      properties: properties(node, nodeId),
      variants
    }
  }
  if (node.type === 'COMPONENT')
    return {
      setId: nodeId,
      name,
      page: '',
      kind: 'component',
      properties: properties(node, nodeId),
      variants: [{ id: nodeId, name, props: {} }]
    }
  throw new Error(
    `Figma node ${nodeId} is ${typeLabel(node.type)}, not a component or a component set`
  )
}

type Mapping = { name: string; nodeId: string }

/** Every mapping under the root that names a node, by its file name without `.figma.ts`, in path order. */
async function mappingsUnder(mappingRoot: string): Promise<Mapping[]> {
  const mappings: Mapping[] = []
  for (const file of await findMappingFiles(mappingRoot)) {
    const nodeId = (await readMappingFile(file)).figma?.nodeId
    if (nodeId) mappings.push({ name: path.basename(file, '.figma.ts'), nodeId })
  }
  return mappings
}

/**
 * The nodes a partial refresh reads: each selector is a mapping's file name or a node id some mapping names. A
 * selector that matches nothing is refused, so a typo never turns into a refresh of nothing.
 */
function selectNodeIds(mappings: Mapping[], selectors: string[]): string[] {
  if (selectors.length === 0) throw new Error('--only takes at least one mapping name or node id')
  const ids: string[] = []
  for (const selector of selectors) {
    const matched = mappings.filter(
      (mapping) => mapping.name === selector || mapping.nodeId === selector
    )
    if (matched.length === 0)
      throw new Error(`--only: no mapping is named or names the node "${selector}"`)
    for (const { nodeId } of matched) if (!ids.includes(nodeId)) ids.push(nodeId)
  }
  return ids
}

/**
 * Why a mapped node gives no set. `missing`: the file answers null for it. `deleted-from-library`: the file still
 * resolves it by id (Figma keeps deleted components that instances reference) but it is on no page. `invalid`:
 * it is on a page but is not a component or a component set with variants and typed properties.
 */
type NodeProblemKind = 'missing' | 'deleted-from-library' | 'invalid'
type NodeProblem = { nodeId: string; kind: NodeProblemKind; reason: string }
type LiveRead = { sets: FigmaSet[]; problems: NodeProblem[] }

type FileTree = { id?: unknown; children?: FileTree[] }

/** The listed ids the tree holds: the file read with `ids` returns only the pages and frames leading to them. */
function idsOnPages(tree: FileTree | undefined, wanted: ReadonlySet<string>): Set<string> {
  const found = new Set<string>()
  const pending: FileTree[] = tree ? [tree] : []
  while (pending.length > 0) {
    const node = pending.pop()!
    if (typeof node.id === 'string' && wanted.has(node.id)) found.add(node.id)
    if (Array.isArray(node.children)) pending.push(...node.children)
  }
  return found
}

const query = (ids: string[]) => ids.map(encodeURIComponent).join(',')

/**
 * Reads the nodes from the live file and sorts each into a set or a problem, in the order asked. An HTTP failure
 * (a rate limit, an expired token) fails the whole read: it says nothing about any one node.
 */
async function readLiveNodes(
  client: FigmaClient,
  fileKey: string,
  nodeIds: string[]
): Promise<LiveRead> {
  const sets: FigmaSet[] = []
  const problems: NodeProblem[] = []
  for (let start = 0; start < nodeIds.length; start += NODE_BATCH) {
    const batch = nodeIds.slice(start, start + NODE_BATCH)
    const response = await client.get<{
      nodes?: Record<string, { document?: RestNode } | null>
    }>(`/v1/files/${fileKey}/nodes?ids=${query(batch)}&depth=1`)
    const resolved = batch.filter((nodeId) => response.nodes?.[nodeId]?.document)
    const onPages =
      resolved.length === 0
        ? new Set<string>()
        : idsOnPages(
            (
              await client.get<{ document?: FileTree }>(
                `/v1/files/${fileKey}?ids=${query(resolved)}`,
                { timeoutMs: PAGE_READ_TIMEOUT_MS }
              )
            ).document,
            new Set(resolved)
          )
    for (const nodeId of batch) {
      const node = response.nodes?.[nodeId]?.document
      if (!node) {
        problems.push({
          nodeId,
          kind: 'missing',
          reason: `Figma node ${nodeId} is not in the file`
        })
        continue
      }
      if (!onPages.has(nodeId)) {
        problems.push({
          nodeId,
          kind: 'deleted-from-library',
          reason: `Figma node ${nodeId} is deleted from the library: the file resolves it but no page holds it`
        })
        continue
      }
      try {
        sets.push(figmaSetFromNode(nodeId, node))
      } catch (error) {
        // figmaSetFromNode names the node by id and its parts by position only.
        problems.push({ nodeId, kind: 'invalid', reason: (error as Error).message })
      }
    }
  }
  return { sets, problems }
}

/** Fails on any node that gives no set, naming every one of them; otherwise the sets in the order asked. */
function requireLiveSets(read: LiveRead): FigmaSet[] {
  if (read.problems.length > 0)
    throw new Error(
      `${read.problems.length} mapped Figma node(s) cannot be read:\n${read.problems
        .map((problem) => `- [${problem.kind}] ${problem.reason}`)
        .join('\n')}`
    )
  return read.sets
}

/**
 * The tracked sets with the refreshed ones in place: a refreshed id replaces its entry or is added, every other
 * entry is kept as it was, and an entry no mapping names any more (left behind by a re-pointed mapping) is dropped.
 */
function mergeRefreshed(
  tracked: FigmaSet[],
  refreshed: FigmaSet[],
  mappedIds: Iterable<string>
): { sets: FigmaSet[]; dropped: string[] } {
  const fresh = new Map(refreshed.map((set) => [set.setId, set]))
  const orphans = new Set(orphanedSets(tracked, mappedIds))
  const dropped = [...orphans].map((set) => set.setId)
  const kept = tracked.filter((set) => !orphans.has(set)).map((set) => fresh.get(set.setId) ?? set)
  const added = refreshed.filter((set) => !tracked.some((entry) => entry.setId === set.setId))
  return { sets: [...kept, ...added], dropped }
}

type Difference = { setId: string; name: string; changes: string[] }

// Both sides are compared as the manifest stores them: a read keeps Figma's order of a variant's properties,
// the manifest sorts them, and that order is not a difference.
const canonical = (sets: FigmaSet[]): FigmaSet[] =>
  parseMappingManifest(serializeMappingManifest(sets))

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b)

/** Live sets that passed the manifest's own checks, in its canonical form; a refused read names no value. */
function checkedLiveSets(live: FigmaSet[]): FigmaSet[] {
  try {
    return canonical(live)
  } catch (error) {
    throw new Error(
      `The live file gave data the manifest refuses: ${(error as Error).message.replace(/^\S+ /, '')}`,
      { cause: error }
    )
  }
}

/** What differs between the tracked manifest and a fresh read, per set, in plain words. */
function manifestDifferences(trackedSets: FigmaSet[], liveSets: FigmaSet[]): Difference[] {
  const tracked = canonical(trackedSets)
  const live = checkedLiveSets(liveSets)
  const differences: Difference[] = []
  const trackedById = new Map(tracked.map((set) => [set.setId, set]))
  const liveIds = new Set(live.map((set) => set.setId))
  for (const set of live) {
    const before = trackedById.get(set.setId)
    if (!before) {
      differences.push({ setId: set.setId, name: set.name, changes: ['not in the manifest'] })
      continue
    }
    const changes: string[] = []
    if (before.name !== set.name) changes.push(`renamed from "${before.name}"`)
    if (before.kind !== set.kind) changes.push(`kind ${before.kind} is now ${set.kind}`)
    const names = new Set([...Object.keys(before.properties), ...Object.keys(set.properties)])
    for (const name of [...names].sort()) {
      const was = before.properties[name]
      const now = set.properties[name]
      if (!was) changes.push(`property "${name}" added`)
      else if (!now) changes.push(`property "${name}" removed`)
      else if (!same(was, now)) changes.push(`property "${name}" changed`)
    }
    if (!same(before.variants, set.variants)) changes.push('variants changed')
    if (changes.length > 0) differences.push({ setId: set.setId, name: set.name, changes })
  }
  for (const set of tracked)
    if (!liveIds.has(set.setId))
      differences.push({ setId: set.setId, name: set.name, changes: ['no mapping names it'] })
  return differences
}

type NamedNode = { setId: string; name: string }
type VerifyReport = {
  ok: boolean
  sets: number
  clean: string[]
  changed: Difference[]
  deletedFromLibrary: NamedNode[]
  missing: NamedNode[]
  invalid: (NamedNode & { reason: string })[]
}

/**
 * The live file against the tracked manifest, one class per mapped node: clean, changed, deleted from the
 * library, missing or invalid. A node that gives no set is named as the tracked manifest or its mapping names it,
 * never by a name read from the file.
 */
function verifyReport(tracked: FigmaSet[], read: LiveRead, mappings: Mapping[]): VerifyReport {
  const failed = new Set(read.problems.map((problem) => problem.nodeId))
  const changed = manifestDifferences(
    tracked.filter((set) => !failed.has(set.setId)),
    read.sets
  )
  const differing = new Set(changed.map((difference) => difference.setId))
  const nameOf = (nodeId: string) =>
    tracked.find((set) => set.setId === nodeId)?.name ??
    mappings.find((mapping) => mapping.nodeId === nodeId)?.name ??
    nodeId
  const named = (kind: NodeProblemKind) =>
    read.problems
      .filter((problem) => problem.kind === kind)
      .map((problem) => ({ setId: problem.nodeId, name: nameOf(problem.nodeId) }))
  return {
    ok: read.problems.length === 0 && changed.length === 0,
    sets: read.sets.length + read.problems.length,
    clean: read.sets.map((set) => set.setId).filter((setId) => !differing.has(setId)),
    changed,
    deletedFromLibrary: named('deleted-from-library'),
    missing: named('missing'),
    invalid: read.problems
      .filter((problem) => problem.kind === 'invalid')
      .map((problem) => ({
        setId: problem.nodeId,
        name: nameOf(problem.nodeId),
        reason: problem.reason
      }))
  }
}

export {
  checkedLiveSets,
  figmaSetFromNode,
  manifestDifferences,
  mappingsUnder,
  mergeRefreshed,
  readLiveNodes,
  requireLiveSets,
  selectNodeIds,
  variantProps,
  verifyReport
}
export type { Difference, LiveRead, Mapping, NodeProblem, RestNode, VerifyReport }
