import { rawComponentPageSchema } from '../ir/raw-components.js'
import { NON_RENDERING, pageRasters } from './strip-image-metadata.js'

// Finds non-rendering image metadata (tEXt, iTXt, zTXt, eXIf, tIME) in a file's rasters. A captured
// component page is parsed with the ingest schema and enumerated with pageRasters, the same structure
// ingest sanitizes: node.png, masks, PNGs inside node.svg and the image-fill map. Generated CSS, SVG and
// code are traversed for PNG data URIs, standalone PNGs are read as they are, and any other JSON is
// walked field by field as extra coverage.
const DATA_URI = /data:image\/png;base64,([A-Za-z0-9+/]+={0,2})/g
const PNG_SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])
const TEXT = /\.(svg|css|ts|tsx|mjs|js|html)$/
const CAPTURE_PAGE = /(^|\/)figma-snapshot\/components\/[^/]+\.json$/

type Allowlist = { files: { path: string; reason: string; expires: string }[] }

function metadataChunks(png: Buffer): string[] {
  const found: string[] = []
  let offset = 8
  while (offset + 12 <= png.length) {
    const length = png.readUInt32BE(offset)
    const type = png.subarray(offset + 4, offset + 8).toString('latin1')
    if (NON_RENDERING.has(type)) found.push(type)
    offset += 12 + length
    if (type === 'IEND') break
  }
  return found
}

const dataUris = (text: string) =>
  [...text.matchAll(DATA_URI)].map((match) => Buffer.from(match[1]!, 'base64'))

function captureRasters(value: unknown, key = ''): Buffer[] {
  if (typeof value === 'string') {
    if (key === 'png') return [Buffer.from(value, 'base64')]
    return value.includes('data:image/png;base64,') ? dataUris(value) : []
  }
  if (Array.isArray(value)) return value.flatMap((item) => captureRasters(item))
  if (value && typeof value === 'object')
    return Object.entries(value).flatMap(([name, item]) => captureRasters(item, name))
  return []
}

/** Every raster in a file, by its name and bytes; files of other types hold none. */
function rastersIn(file: string, bytes: Buffer): Buffer[] {
  if (file.endsWith('.png')) return [bytes]
  if (CAPTURE_PAGE.test(file))
    return pageRasters(rawComponentPageSchema.parse(JSON.parse(bytes.toString('utf8')))).map(
      (raster) => raster.bytes
    )
  if (file.endsWith('.json')) return captureRasters(JSON.parse(bytes.toString('utf8')))
  if (TEXT.test(file)) return dataUris(bytes.toString('latin1'))
  return []
}

/** Chunk-type counts of the metadata found in a file, empty when it is clean. */
function fileMetadata(file: string, bytes: Buffer): Record<string, number> {
  const counts: Record<string, number> = {}
  for (const raster of rastersIn(file, bytes))
    if (raster.subarray(0, 8).equals(PNG_SIGNATURE))
      for (const chunk of metadataChunks(raster)) counts[chunk] = (counts[chunk] ?? 0) + 1
  return counts
}

/**
 * Gate problems: metadata outside the allowlist, and allowlist entries that are outside the allowed
 * directory, missing a reason or expiry, or already clean (so they must be removed).
 */
function gateProblems(
  found: Map<string, Record<string, number>>,
  allow: Allowlist,
  allowedPrefix: string
): string[] {
  const problems: string[] = []
  const listed = new Set(allow.files.map((entry) => entry.path))
  for (const [file, counts] of found)
    if (!listed.has(file)) problems.push(`${file}: ${JSON.stringify(counts)} (not allowlisted)`)
  const seen = new Set<string>()
  for (const entry of allow.files) {
    if (seen.has(entry.path)) problems.push(`${entry.path}: allowlisted more than once`)
    seen.add(entry.path)
    if (!entry.path.startsWith(allowedPrefix))
      problems.push(`${entry.path}: allowlist entry outside ${allowedPrefix}`)
    if (!entry.reason?.trim() || !entry.expires?.trim())
      problems.push(`${entry.path}: allowlist entry without a reason or an expiry`)
    if (!found.has(entry.path))
      problems.push(`${entry.path}: allowlisted but clean or missing; remove the entry`)
  }
  return problems
}

export { captureRasters, fileMetadata, gateProblems, rastersIn }
export type { Allowlist }
