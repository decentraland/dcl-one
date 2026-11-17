import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { crc32, deflateSync } from 'node:zlib'
import { describe, expect, it } from 'vitest'
import { snapshotDir } from '../../test/snapshot.js'
import {
  rawComponentPageSchema,
  type RawComponentPage,
  type RawNode
} from '../ir/raw-components.js'
import {
  checksum,
  NON_RENDERING,
  pageRasters,
  stripImageMetadata,
  stripPngMetadata
} from './strip-image-metadata.js'
import { encodePageFileName } from './page-file-name.js'

// The synthetic snapshot stores its rasters clean, like every capture. These tests plant editor metadata,
// shaped like what Figma exports really carry (Adobe XMP edit history, EXIF) but with obviously fake
// values, into its PNGs, which keep their colour chunks (iCCP; gAMA).
function chunk(type: string, data: Buffer) {
  const length = Buffer.alloc(4)
  length.writeUInt32BE(data.length)
  const body = Buffer.concat([Buffer.from(type, 'latin1'), data])
  const crc = Buffer.alloc(4)
  crc.writeUInt32BE(crc32(body))
  return Buffer.concat([length, body, crc])
}

function chunks(bytes: Buffer): { type: string; raw: Buffer }[] {
  const found: { type: string; raw: Buffer }[] = []
  let offset = 8
  while (offset + 12 <= bytes.length) {
    const length = bytes.readUInt32BE(offset)
    const type = bytes.subarray(offset + 4, offset + 8).toString('latin1')
    found.push({ type, raw: bytes.subarray(offset, offset + 12 + length) })
    offset += 12 + length
    if (type === 'IEND') break
  }
  return found
}

const XMP = `<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmlns:xmpMM="http://ns.adobe.com/xap/1.0/mm/"
 xmlns:stEvt="http://ns.adobe.com/xap/1.0/sType/ResourceEvent#" xmp:CreatorTool="Example Editor 0.0"
 xmp:CreateDate="2000-01-01T00:00:00Z" xmp:ModifyDate="2000-01-01T00:00:00Z"
 xmpMM:DocumentID="xmp.did:00000000-0000-0000-0000-000000000000"
 xmpMM:InstanceID="xmp.iid:00000000-0000-0000-0000-000000000000">
<xmpMM:History><rdf:Seq><rdf:li stEvt:action="saved" stEvt:when="2000-01-01T00:00:00Z"
 stEvt:softwareAgent="Example Editor 0.0" stEvt:instanceID="xmp.iid:00000000-0000-0000-0000-000000000000"/>
</rdf:Seq></xmpMM:History></rdf:Description></rdf:RDF></x:xmpmeta>`

const PLANTED = [
  chunk(
    'iTXt',
    Buffer.concat([Buffer.from('XML:com.adobe.xmp\0\0\0\0\0', 'latin1'), Buffer.from(XMP)])
  ),
  chunk('eXIf', Buffer.from([0x4d, 0x4d, 0x00, 0x2a, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00])),
  chunk('tEXt', Buffer.from('Software\0Example Exporter', 'latin1')),
  chunk('zTXt', Buffer.concat([Buffer.from('Comment\0\0', 'latin1'), deflateSync('fake comment')])),
  chunk('tIME', Buffer.from([0x07, 0xd0, 1, 1, 0, 0, 0]))
]
const PLANTED_TYPES = ['iTXt', 'eXIf', 'tEXt', 'zTXt', 'tIME']

/** The PNG with every planted chunk inserted right after IHDR (signature 8 bytes, IHDR 25). */
const plant = (png: Buffer) => Buffer.concat([png.subarray(0, 33), ...PLANTED, png.subarray(33)])

const page = async (name: string) =>
  rawComponentPageSchema.parse(
    JSON.parse(
      await readFile(path.join(snapshotDir, 'components', encodePageFileName(name)), 'utf8')
    )
  ) as RawComponentPage

// A captured raster carrying the colour chunk the case is about.
async function capturedPng(name: string, colourChunk: string): Promise<Buffer> {
  const found = pageRasters(await page(name))
    .map((raster) => raster.bytes)
    .find((png) => chunks(png).some((entry) => entry.type === colourChunk))
  if (!found) throw new Error(`no ${colourChunk} raster in the ${name} fixture`)
  return found
}

const EMBEDDED = /data:image\/png;base64,([A-Za-z0-9+/]+={0,2})/g
const plantText = (text: string) =>
  text.replace(
    EMBEDDED,
    (_match, base64: string) =>
      `data:image/png;base64,${plant(Buffer.from(base64, 'base64')).toString('base64')}`
  )

// Plants into every raster of a node, keeping the transport checksum true for the planted body, as an
// unsanitized capture would carry it.
function plantNode(node: RawNode): RawNode {
  const svg = node.svg === undefined ? undefined : plantText(node.svg)
  const png =
    node.png === undefined ? undefined : plant(Buffer.from(node.png, 'base64')).toString('base64')
  const body = svg ?? png
  return {
    ...node,
    ...(svg !== undefined ? { svg } : {}),
    ...(png !== undefined ? { png } : {}),
    ...(node.sum !== undefined && body !== undefined ? { sum: checksum(body) } : {}),
    ...(node.children ? { children: node.children.map(plantNode) } : {})
  }
}

function plantPage(source: RawComponentPage): RawComponentPage {
  return {
    ...source,
    components: source.components.map((component) => ({
      ...component,
      base: { ...component.base, tree: plantNode(component.base.tree) },
      others: component.others.map((other) => ({
        ...other,
        patches: other.patches.map((patch) =>
          patch.replace ? { ...patch, replace: plantNode(patch.replace) } : patch
        )
      }))
    }))
  }
}

describe('stripPngMetadata', () => {
  describe.each([
    ['Demo Nav', 'iCCP'],
    ['Demo Cursors', 'gAMA']
  ])('when a captured %s raster with %s gets planted editor metadata', (name, colourChunk) => {
    it('should remove every planted chunk and give back the original bytes, colour chunks and samples', async () => {
      const original = await capturedPng(name, colourChunk)
      const planted = plant(original)
      expect(chunks(planted).map((entry) => entry.type)).toEqual(
        expect.arrayContaining(PLANTED_TYPES)
      )

      const stripped = stripPngMetadata(planted)

      expect(chunks(stripped).some((entry) => NON_RENDERING.has(entry.type))).toBe(false)
      expect(stripped.equals(original)).toBe(true)
      expect(chunks(stripped).map((entry) => entry.type)).toContain(colourChunk)
    })
  })

  describe('when a PNG names its author in a text chunk', () => {
    it('should remove it', () => {
      const signature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])
      const ihdr = chunk('IHDR', Buffer.from([0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0]))
      const author = chunk('tEXt', Buffer.from('Author\0Jane Doe', 'latin1'))
      const iend = chunk('IEND', Buffer.alloc(0))

      expect(stripPngMetadata(Buffer.concat([signature, ihdr, author, iend]))).toEqual(
        Buffer.concat([signature, ihdr, iend])
      )
    })
  })

  describe('when the bytes are not a complete PNG', () => {
    it('should return them unchanged for validation to reject', () => {
      const truncated = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13])

      expect(stripPngMetadata(truncated)).toBe(truncated)
    })
  })
})

describe('stripImageMetadata', () => {
  describe.each(['Demo Nav', 'Demo Cursors'])(
    'when every raster of the %s capture gets planted editor metadata',
    (name) => {
      it('should give back the clean page exactly, with each transport checksum recomputed', async () => {
        const clean = await page(name)
        const planted = plantPage(clean)
        const rasters = pageRasters(planted)
        expect(rasters.length).toBeGreaterThan(0)
        expect(
          rasters.every((raster) =>
            PLANTED_TYPES.every((type) => chunks(raster.bytes).some((entry) => entry.type === type))
          )
        ).toBe(true)
        expect(JSON.stringify(planted)).not.toBe(JSON.stringify(clean))

        const stripped = stripImageMetadata(planted)

        expect(stripped).toEqual(clean)
        expect(stripImageMetadata(stripped)).toEqual(stripped)
      })
    }
  )
})
