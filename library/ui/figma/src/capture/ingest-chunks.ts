import { mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { decodePng } from './png-decode.js'
import { stripImageMetadata } from './strip-image-metadata.js'
import { z } from 'zod'
import {
  rawComponentPageSchema,
  type RawComponentPage,
  type RawNode
} from '../ir/raw-components.js'
import { stripFileKey } from './strip-file-key.js'

type PageChunk = {
  page: { id: string; name: string }
  labels?: unknown[]
  // Number of components on the page and index of the first one in this chunk, as the plugin reports them.
  total?: number
  start?: number
  components: unknown[]
}

type CaptureChunk = { pages: PageChunk[]; resume: unknown }

type IngestReport = {
  page: string
  components: number
  added: number
  replaced: number
  removed: number
}

// Chunks travel through a chat transcript before they reach disk, so a truncated or mistyped picture has to
// be caught here instead of surfacing as a broken component later.
function pngProblem(base64: string): string | undefined {
  return decodePng(Buffer.from(base64, 'base64')).problem
}

// Image fills are stored as data URIs in a component's `images` table and get the same check as
// exported artwork: the bounded PNG capture validator. Another raster type fails closed, since nothing
// here validates it.
const PNG_DATA_URI = /^data:image\/png;base64,([A-Za-z0-9+/]+={0,2})$/

function imageDataProblem(uri: string): string | undefined {
  const match = uri.match(PNG_DATA_URI)
  if (!match) return 'not a base64 PNG data URI'
  return pngProblem(match[1]!)
}

function svgProblem(svg: string): string | undefined {
  const text = svg.trim()
  if (!/^<svg[\s>]/.test(text) || !text.endsWith('</svg>')) return 'not a complete <svg> document'
  const opened = (text.match(/<(?!\/|!|\?)[^>]*[^/]>/g) ?? []).length
  const closed = (text.match(/<\/[a-zA-Z]/g) ?? []).length
  return opened === closed ? undefined : `unbalanced tags (${opened} opened, ${closed} closed)`
}

// Must match `checksum` in capture/extract-components.figma.js.
function checksum(text: string): string {
  let hash = 0x811c9dc5
  for (let index = 0; index < text.length; index += 1) {
    hash ^= text.charCodeAt(index)
    hash = Math.imul(hash, 0x01000193)
  }
  return (hash >>> 0).toString(36)
}

function sumProblem(node: RawNode): string | undefined {
  const body = node.svg ?? node.png
  if (body === undefined || node.sum === undefined) return undefined
  return checksum(body) === node.sum
    ? undefined
    : 'checksum mismatch, the picture was altered in transit'
}

function assetProblems(node: RawNode, trail: string, found: string[] = []): string[] {
  const here = `${trail}/${node.name ?? node.type}`
  const problem =
    sumProblem(node) ??
    (node.png !== undefined ? pngProblem(node.png) : undefined) ??
    (node.svg !== undefined ? svgProblem(node.svg) : undefined)
  if (problem) found.push(`${here}: ${problem}`)
  if (node.mask) {
    const maskProblem =
      sumProblem({ ...node, svg: undefined, png: undefined, sum: undefined, ...node.mask }) ??
      (node.mask.png
        ? pngProblem(node.mask.png)
        : node.mask.svg
          ? svgProblem(node.mask.svg)
          : undefined)
    if (maskProblem) found.push(`${here}/mask: ${maskProblem}`)
  }
  node.children?.forEach((child) => assetProblems(child, here, found))
  return found
}

function validatePage(page: RawComponentPage): void {
  const problems: string[] = []
  for (const component of page.components) {
    const trees = [component.base.tree]
    for (const other of component.others) {
      for (const patch of other.patches) if (patch.replace) trees.push(patch.replace)
    }
    for (const tree of trees) problems.push(...assetProblems(tree, component.name))
    for (const [ref, uri] of Object.entries(component.images ?? {})) {
      const problem = imageDataProblem(uri)
      if (problem) problems.push(`${component.name}/images/${ref}: ${problem}`)
    }
  }
  if (problems.length > 0) throw new Error(`Corrupt captured artwork:\n${problems.join('\n')}`)
}

function groupByPage(chunks: CaptureChunk[]): Map<string, PageChunk[]> {
  const grouped = new Map<string, PageChunk[]>()
  for (const chunk of chunks) {
    for (const entry of chunk.pages) {
      grouped.set(entry.page.id, [...(grouped.get(entry.page.id) ?? []), entry])
    }
  }
  return grouped
}

function fileName(pageName: string): string {
  return `${pageName.replace(/[\\/]/g, '-')}.json`
}

async function readExisting(file: string): Promise<RawComponentPage | undefined> {
  const raw = await readFile(file, 'utf8').catch(() => undefined)
  return raw === undefined ? undefined : rawComponentPageSchema.parse(JSON.parse(raw))
}

// A page renamed in Figma keeps its id, so its previous file is found by id rather than by name.
async function findExisting(
  directory: string,
  pageId: string,
  file: string
): Promise<{ file: string; page: RawComponentPage } | undefined> {
  const byName = await readExisting(file)
  if (byName) return { file, page: byName }
  for (const name of await readdir(directory)) {
    if (!name.endsWith('.json')) continue
    const candidate = path.join(directory, name)
    const page = await readExisting(candidate)
    if (page?.page.id === pageId) return { file: candidate, page }
  }
  return undefined
}

type VariantPart = {
  id: string
  others: unknown[]
  variantRange?: { start: number; total: number }
}

// A set captured in parts (VARIANT_LIMIT in the plugin) repeats its base in every part and splits `others`.
// The parts are joined here because the schema would drop `variantRange` and keep a part as if it were the
// whole set.
function joinVariantParts(components: unknown[]): unknown[] {
  const parts = new Map<string, VariantPart[]>()
  const joined: unknown[] = []
  for (const component of components as VariantPart[]) {
    if (!component.variantRange) {
      joined.push(component)
      continue
    }
    if (!parts.has(component.id)) joined.push(component.id)
    parts.set(component.id, [...(parts.get(component.id) ?? []), component])
  }
  return joined.map((item) => {
    if (typeof item !== 'string') return item
    const sorted = [...(parts.get(item) ?? [])].sort(
      (a, b) => a.variantRange!.start - b.variantRange!.start
    )
    const [first] = sorted
    const total = first!.variantRange!.total
    let covered = 0
    for (const part of sorted) {
      if (part.variantRange!.start !== covered) {
        throw new Error(
          `Set ${item} is missing variants ${covered}-${part.variantRange!.start - 1} of ${total}`
        )
      }
      covered += part.others.length
    }
    if (covered !== total)
      throw new Error(`Set ${item} is missing variants ${covered}-${total - 1} of ${total}`)
    const whole: VariantPart = { ...first!, others: sorted.flatMap((part) => part.others) }
    delete whole.variantRange
    return whole
  })
}

// The chunks hold the whole page when they start at 0 and together carry every component the plugin counted.
// The External page is never whole: the plugin counts only the EXTERNAL_IDS of that run, not the page.
function isComplete(entries: PageChunk[], capturedIds: Set<string>): boolean {
  if (entries[0]?.page.id === 'external') return false
  const total = entries.find((entry) => entry.start === 0)?.total
  return total !== undefined && capturedIds.size === total
}

// Components are upserted by id, so a page can be captured in several runs and re-captured partially. A
// complete capture replaces the page instead, so components deleted in Figma leave the snapshot.
async function ingestChunks(snapshotDir: string, chunks: CaptureChunk[]): Promise<IngestReport[]> {
  const reports: IngestReport[] = []
  const directory = path.join(snapshotDir, 'components')
  await mkdir(directory, { recursive: true })
  for (const entries of groupByPage(chunks).values()) {
    const [first] = entries
    if (!first) continue
    const file = path.join(directory, fileName(first.page.name.trim()))
    const existing = await findExisting(directory, first.page.id, file)
    const previous = new Map(
      (existing?.page.components ?? []).map((component) => [component.id, component])
    )
    const captured = new Map<string, RawComponentPage['components'][number]>()
    const parsed = rawComponentPageSchema.parse({
      page: first.page,
      components: joinVariantParts(entries.flatMap((entry) => entry.components))
    })
    for (const component of parsed.components) captured.set(component.id, component)
    const capturedIds = new Set(captured.keys())
    const replaced = [...capturedIds].filter((id) => previous.has(id)).length
    const complete = isComplete(entries, capturedIds)
    const merged = complete ? captured : new Map([...previous, ...captured])
    const removed = complete ? [...previous.keys()].filter((id) => !capturedIds.has(id)).length : 0
    const labels = entries.find((entry) => entry.labels)?.labels ?? existing?.page.labels
    const page = rawComponentPageSchema.parse({
      page: { id: first.page.id, name: first.page.name.trim() },
      labels,
      components: [...merged.values()]
    })
    validatePage(page)
    await writeFile(file, `${JSON.stringify(stripFileKey(stripImageMetadata(page)))}\n`)
    if (existing && existing.file !== file) await rm(existing.file)
    reports.push({
      page: page.page.name,
      components: page.components.length,
      added: capturedIds.size - replaced,
      replaced,
      removed
    })
  }
  return reports
}

async function ingestChunkFiles(snapshotDir: string, inputs: string[]): Promise<IngestReport[]> {
  const chunks = decodeCaptureChunks(
    await Promise.all(
      inputs.map(async (input) => JSON.parse(await readFile(input, 'utf8')) as unknown)
    )
  )
  return ingestChunks(snapshotDir, chunks)
}

const transportSchema = z.object({
  offset: z.number().int().nonnegative(),
  total: z.number().int().positive(),
  checksum: z.string(),
  part: z.string().min(1)
})

function decodeCaptureChunks(inputs: unknown[]): CaptureChunk[] {
  const chunks: CaptureChunk[] = []
  const groups = new Map<string, z.infer<typeof transportSchema>[]>()
  for (const input of inputs) {
    if (typeof input === 'object' && input !== null && 'transport' in input) {
      const part = transportSchema.parse(input.transport)
      const key = `${part.checksum}:${part.total}`
      groups.set(key, [...(groups.get(key) ?? []), part])
    } else chunks.push(input as CaptureChunk)
  }
  for (const parts of groups.values()) {
    let raw = ''
    for (const part of parts.sort((a, b) => a.offset - b.offset)) {
      if (part.offset !== raw.length)
        throw new Error('Capture transport has missing, duplicate or overlapping parts')
      raw += part.part
    }
    if (raw.length !== parts[0]!.total || checksum(raw) !== parts[0]!.checksum)
      throw new Error('Capture transport length or checksum mismatch')
    const chunk = JSON.parse(raw) as CaptureChunk
    if (!Array.isArray(chunk.pages))
      throw new Error('Capture transport does not contain a page capture')
    chunks.push(chunk)
  }
  return chunks
}

export {
  checksum,
  decodeCaptureChunks,
  ingestChunkFiles,
  ingestChunks,
  imageDataProblem,
  pngProblem,
  svgProblem,
  validatePage
}
export type { CaptureChunk, IngestReport }
