// The mapping manifest: the identity and property definitions of every Figma component a hand-written
// mapping names, tracked in the library (figma-mapping-manifest.json). check-mappings reads it instead of a
// page capture, so the check runs offline wherever the tree is checked out. It holds node ids and plain names
// only: never the file key, a Figma URL, a component key or image data.
import { readFile, rename, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { z } from 'zod'
import { stripFileKey } from '../capture/strip-file-key.js'
import type { FigmaSet } from './figma-index.js'

const MANIFEST_FILE = 'figma-mapping-manifest.json'
const SCHEMA = 1

const NODE_ID = /^\d+:\d+$/
const FIGMA_URL = /figma\.com\//i
// The shape of a Figma file or component key: 22 or more letters and digits mixing cases and digits.
const KEY_SHAPE =
  /(?<![A-Za-z0-9])(?=[A-Za-z0-9]*[0-9])(?=[A-Za-z0-9]*[a-z])(?=[A-Za-z0-9]*[A-Z])[A-Za-z0-9]{22,}(?![A-Za-z0-9])/

const propertySchema = z
  .object({
    type: z.string().min(1),
    default: z.union([z.string(), z.boolean()]).optional(),
    options: z.array(z.string()).optional()
  })
  .strict()

const variantSchema = z
  .object({
    id: z.string().regex(NODE_ID),
    name: z.string(),
    props: z.record(z.string(), z.string())
  })
  .strict()

const setSchema = z
  .object({
    setId: z.string().regex(NODE_ID),
    name: z.string().min(1),
    page: z.string(),
    kind: z.enum(['set', 'component']),
    properties: z.record(z.string(), propertySchema),
    variants: z.array(variantSchema).min(1)
  })
  .strict()

const manifestSchema = z.object({ schema: z.literal(SCHEMA), sets: z.array(setSchema) }).strict()

/** The manifest of a library: beside its mapping root's parent, where the lock also lives. */
function manifestPath(mappingRoot: string): string {
  return path.resolve(mappingRoot, '..', MANIFEST_FILE)
}

function privateStrings(value: unknown, at: string, found: string[]): void {
  if (typeof value === 'string') {
    if (FIGMA_URL.test(value) || KEY_SHAPE.test(value)) found.push(at)
  } else if (Array.isArray(value)) {
    value.forEach((item, index) => privateStrings(item, `${at}[${index}]`, found))
  } else if (value && typeof value === 'object') {
    for (const [key, item] of Object.entries(value)) {
      privateStrings(key, `${at}.${key} (name)`, found)
      privateStrings(item, `${at}.${key}`, found)
    }
  }
}

/** Parses manifest text. Refuses anything but the schema, a repeated set id and any private identifier. */
function parseMappingManifest(text: string, label = MANIFEST_FILE): FigmaSet[] {
  let raw: unknown
  try {
    raw = JSON.parse(text)
  } catch {
    throw new Error(`${label} is not valid JSON`)
  }
  const parsed = manifestSchema.safeParse(raw)
  if (!parsed.success) {
    const issue = parsed.error.issues[0]
    throw new Error(
      `${label} does not match the mapping manifest schema at ${issue?.path.join('.') || 'the root'}: ${issue?.message}`
    )
  }
  const leaked: string[] = []
  privateStrings(parsed.data.sets, 'sets', leaked)
  if (leaked.length > 0)
    throw new Error(`${label} holds a Figma URL or a key-shaped string at ${leaked[0]}`)
  const seen = new Set<string>()
  for (const set of parsed.data.sets) {
    if (seen.has(set.setId)) throw new Error(`${label} lists Figma node ${set.setId} twice`)
    seen.add(set.setId)
  }
  return parsed.data.sets
}

/** Reads the manifest. A missing file is an error: check-mappings never passes without identities. */
async function loadMappingManifest(file: string): Promise<FigmaSet[]> {
  let text: string
  try {
    text = await readFile(file, 'utf8')
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT')
      throw new Error(
        `${file} is missing: run "pnpm figma manifest refresh" with FIGMA_TOKEN and FIGMA_FILE_KEY set`,
        { cause: error }
      )
    throw error
  }
  return parseMappingManifest(text, path.basename(file))
}

const sortedRecord = <T>(record: Record<string, T>): Record<string, T> =>
  Object.fromEntries(Object.entries(record).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)))

const idOrder = (a: string, b: string) => {
  const [a1, a2] = a.split(':').map(Number)
  const [b1, b2] = b.split(':').map(Number)
  return a1! - b1! || a2! - b2!
}

/** The manifest's bytes: key-stripped, sets by node id and properties by name, so a refresh diffs cleanly. */
function serializeMappingManifest(sets: FigmaSet[]): string {
  const ordered = [...sets]
    .sort((a, b) => idOrder(a.setId, b.setId))
    .map((set) => ({
      setId: set.setId,
      name: set.name,
      page: set.page,
      kind: set.kind,
      properties: sortedRecord(
        Object.fromEntries(
          Object.entries(set.properties).map(([name, property]) => [
            name,
            {
              type: property.type,
              ...(property.default === undefined ? {} : { default: property.default }),
              ...(property.options === undefined ? {} : { options: property.options })
            }
          ])
        )
      ),
      variants: set.variants.map((variant) => ({
        id: variant.id,
        name: variant.name,
        props: sortedRecord(variant.props)
      }))
    }))
  const text = `${JSON.stringify(stripFileKey({ schema: SCHEMA, sets: ordered }), null, 2)}\n`
  // What is written must read back: a refresh that would produce a refused manifest fails before any write.
  parseMappingManifest(text)
  return text
}

/** Replaces the manifest in one step; a failure before the rename leaves the previous file as it was. */
async function writeMappingManifest(file: string, sets: FigmaSet[]): Promise<void> {
  const text = serializeMappingManifest(sets)
  const staged = `${file}.${process.pid}.tmp`
  await writeFile(staged, text, { flag: 'wx' })
  await rename(staged, file)
}

/** Manifest entries no mapping names: stale after a mapping is removed or re-pointed. */
function orphanedSets(sets: FigmaSet[], mappedNodeIds: Iterable<string>): FigmaSet[] {
  const mapped = new Set(mappedNodeIds)
  return sets.filter((set) => !mapped.has(set.setId))
}

export {
  MANIFEST_FILE,
  loadMappingManifest,
  manifestPath,
  orphanedSets,
  parseMappingManifest,
  serializeMappingManifest,
  writeMappingManifest
}
