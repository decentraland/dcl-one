import type { RawComponentPage, RawNode } from '../ir/raw-components.js'
import { checksum } from './ingest-chunks.js'

// Raster images inside a capture (Figma's SVG exports embed them, and image fills are data URIs) can
// carry editor metadata: Adobe XMP edit history, document ids, EXIF, timestamps. None of it renders,
// so ingest drops it. Every chunk that affects pixels or colour is kept byte for byte: IHDR, PLTE,
// tRNS, IDAT, IEND and gAMA, cHRM, sRGB, iCCP, sBIT, pHYs.
const NON_RENDERING = new Set(['tEXt', 'iTXt', 'zTXt', 'eXIf', 'tIME'])
const PNG_SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])
const EMBEDDED_PNG = /data:image\/png;base64,([A-Za-z0-9+/]+={0,2})/g

/** The PNG without its non-rendering chunks; anything that is not a well-formed PNG is returned as is. */
function stripPngMetadata(bytes: Buffer): Buffer {
  if (!bytes.subarray(0, 8).equals(PNG_SIGNATURE)) return bytes
  const kept: Buffer[] = [PNG_SIGNATURE]
  let offset = 8
  while (offset + 12 <= bytes.length) {
    const length = bytes.readUInt32BE(offset)
    const end = offset + 12 + length
    if (end > bytes.length) return bytes
    const type = bytes.subarray(offset + 4, offset + 8).toString('latin1')
    if (!NON_RENDERING.has(type)) kept.push(bytes.subarray(offset, end))
    offset = end
    if (type === 'IEND') return Buffer.concat(kept)
  }
  return bytes
}

const stripBase64Png = (base64: string) =>
  stripPngMetadata(Buffer.from(base64, 'base64')).toString('base64')

const stripEmbedded = (text: string) =>
  text.replace(
    EMBEDDED_PNG,
    (_match, base64: string) => `data:image/png;base64,${stripBase64Png(base64)}`
  )

type Artwork = { svg?: string; png?: string; sum?: string }

// Rewrites the artwork bodies and keeps the transport checksum true for the stored body.
function stripArtwork<T extends Artwork>(artwork: T): T {
  const svg = artwork.svg === undefined ? undefined : stripEmbedded(artwork.svg)
  const png = artwork.png === undefined ? undefined : stripBase64Png(artwork.png)
  if (svg === artwork.svg && png === artwork.png) return artwork
  const body = svg ?? png!
  return {
    ...artwork,
    ...(svg !== undefined ? { svg } : {}),
    ...(png !== undefined ? { png } : {}),
    ...(artwork.sum !== undefined ? { sum: checksum(body) } : {})
  }
}

function stripNode(node: RawNode): RawNode {
  const own = stripArtwork(node)
  const mask = node.mask && stripArtwork(node.mask)
  const children = node.children?.map(stripNode)
  return {
    ...own,
    ...(mask ? { mask } : {}),
    ...(children ? { children } : {})
  }
}

type Raster = { location: string; bytes: Buffer }

const embeddedRasters = (text: string, location: string): Raster[] =>
  [...text.matchAll(EMBEDDED_PNG)].map((match) => ({
    location,
    bytes: Buffer.from(match[1]!, 'base64')
  }))

function artworkRasters(artwork: Artwork, location: string): Raster[] {
  return [
    ...(artwork.png !== undefined
      ? [{ location: `${location}.png`, bytes: Buffer.from(artwork.png, 'base64') }]
      : []),
    ...(artwork.svg !== undefined ? embeddedRasters(artwork.svg, `${location}.svg`) : [])
  ]
}

function nodeRasters(node: RawNode, trail: string): Raster[] {
  const here = `${trail}/${node.name ?? node.type}`
  return [
    ...artworkRasters(node, here),
    ...(node.mask ? artworkRasters(node.mask, `${here}/mask`) : []),
    ...(node.children ?? []).flatMap((child) => nodeRasters(child, here))
  ]
}

/**
 * Every raster a captured page stores, by location: node.png, mask png or svg, PNGs embedded in node.svg,
 * and the image-fill map. The same structure stripImageMetadata rewrites, so a check over this list
 * covers exactly what ingest sanitizes.
 */
function pageRasters(page: RawComponentPage): Raster[] {
  return page.components.flatMap((component) => [
    ...nodeRasters(component.base.tree, component.name),
    ...component.others.flatMap((other) =>
      other.patches.flatMap((patch) =>
        patch.replace ? nodeRasters(patch.replace, `${component.name}/${other.id}`) : []
      )
    ),
    ...Object.entries(component.images ?? {}).flatMap(([ref, uri]) =>
      embeddedRasters(uri, `${component.name}/images/${ref}`)
    )
  ])
}

/** Run after validatePage: integrity is checked on the captured bytes, then metadata is dropped. */
function stripImageMetadata(page: RawComponentPage): RawComponentPage {
  return {
    ...page,
    components: page.components.map((component) => ({
      ...component,
      base: { ...component.base, tree: stripNode(component.base.tree) },
      others: component.others.map((other) => ({
        ...other,
        patches: other.patches.map((patch) =>
          patch.replace ? { ...patch, replace: stripNode(patch.replace) } : patch
        )
      })),
      ...(component.images
        ? {
            images: Object.fromEntries(
              Object.entries(component.images).map(([ref, uri]) => [ref, stripEmbedded(uri)])
            )
          }
        : {})
    }))
  }
}

export {
  NON_RENDERING,
  pageRasters,
  stripEmbedded as stripSvgImageMetadata,
  stripImageMetadata,
  stripPngMetadata
}
export type { Raster }
