// Builds the mapping manifest from the live file: one REST read of the nodes the mappings name. It runs only
// where FIGMA_TOKEN and FIGMA_FILE_KEY are set; everything else reads the tracked manifest.
import type { FigmaClient } from '../figma/client.js'
import type { FigmaSet } from './figma-index.js'
import { findMappingFiles } from './library.js'
import { readMappingFile } from './mapping-file.js'

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

const text = (value: unknown, what: string): string => {
  if (typeof value !== 'string') throw new Error(`${what} has no name`)
  return value
}

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
  for (const [name, definition] of Object.entries(node.componentPropertyDefinitions ?? {})) {
    if (typeof definition.type !== 'string')
      throw new Error(`Figma node ${nodeId}: property "${name}" has no type`)
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
      .map((child) => {
        const variantName = text(child.name, `A variant of Figma node ${nodeId}`)
        return {
          id: text(child.id, `A variant of Figma node ${nodeId}`),
          name: variantName,
          props: variantProps(variantName)
        }
      })
    if (variants.length === 0) throw new Error(`Figma node ${nodeId} ("${name}") has no variants`)
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
    `Figma node ${nodeId} ("${name}") is a ${String(node.type)}, not a component or a component set`
  )
}

/** The Figma node of every mapping under the root, each once, in mapping order. */
async function mappedNodeIds(mappingRoot: string): Promise<string[]> {
  const ids: string[] = []
  for (const file of await findMappingFiles(mappingRoot)) {
    const nodeId = (await readMappingFile(file)).figma?.nodeId
    if (nodeId && !ids.includes(nodeId)) ids.push(nodeId)
  }
  return ids
}

/** Reads every node from the live file. Any node that cannot be read fails the whole read. */
async function readMappedSets(
  client: FigmaClient,
  fileKey: string,
  nodeIds: string[]
): Promise<FigmaSet[]> {
  const sets: FigmaSet[] = []
  for (let start = 0; start < nodeIds.length; start += NODE_BATCH) {
    const batch = nodeIds.slice(start, start + NODE_BATCH)
    const response = await client.get<{
      nodes?: Record<string, { document?: RestNode } | null>
    }>(`/v1/files/${fileKey}/nodes?ids=${batch.map(encodeURIComponent).join(',')}&depth=1`)
    for (const nodeId of batch)
      sets.push(figmaSetFromNode(nodeId, response.nodes?.[nodeId]?.document))
  }
  return sets
}

type Difference = { setId: string; name: string; changes: string[] }

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b)

/** What differs between the tracked manifest and a fresh read, per set, in plain words. */
function manifestDifferences(tracked: FigmaSet[], live: FigmaSet[]): Difference[] {
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

export { figmaSetFromNode, manifestDifferences, mappedNodeIds, readMappedSets, variantProps }
export type { Difference, RestNode }
