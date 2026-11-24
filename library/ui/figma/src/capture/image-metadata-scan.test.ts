import { crc32 } from 'node:zlib'
import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { beforeEach, describe, expect, it } from 'vitest'
import { snapshotDir } from '../../test/snapshot.js'
import { rawComponentPageSchema, type RawComponentPage } from '../ir/raw-components.js'
import { stripImageMetadata } from './strip-image-metadata.js'
import { fileMetadata, gateProblems } from './image-metadata-scan.js'

const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])
const chunk = (type: string, data: Buffer) => {
  const length = Buffer.alloc(4)
  length.writeUInt32BE(data.length)
  const body = Buffer.concat([Buffer.from(type, 'latin1'), data])
  const crc = Buffer.alloc(4)
  crc.writeUInt32BE(crc32(body))
  return Buffer.concat([length, body, crc])
}
const png = (...extra: Buffer[]) =>
  Buffer.concat([
    SIGNATURE,
    chunk('IHDR', Buffer.from([0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0])),
    ...extra,
    chunk('IEND', Buffer.alloc(0))
  ])
const tagged = png(
  chunk('tEXt', Buffer.from('Software\0Figma', 'latin1')),
  chunk('eXIf', Buffer.alloc(4))
)
const clean = png(chunk('sRGB', Buffer.from([0])))
const webpChunk = (type: string, data: Buffer) => {
  const length = Buffer.alloc(4)
  length.writeUInt32LE(data.length)
  return Buffer.concat([Buffer.from(type, 'latin1'), length, data, Buffer.alloc(data.length % 2)])
}
const webp = (...chunks: Buffer[]) => {
  const body = Buffer.concat([Buffer.from('WEBP', 'latin1'), ...chunks])
  const size = Buffer.alloc(4)
  size.writeUInt32LE(body.length)
  return Buffer.concat([Buffer.from('RIFF', 'latin1'), size, body])
}
const uri = (bytes: Buffer) => `data:image/png;base64,${bytes.toString('base64')}`

describe('fileMetadata', () => {
  describe('when the file is a standalone PNG', () => {
    describe('and it carries metadata chunks', () => {
      it('should find the metadata', () => {
        expect(fileMetadata('refs/a.png', tagged)).toEqual({ tEXt: 1, eXIf: 1 })
      })
    })

    describe('and it carries only rendering chunks', () => {
      it('should find nothing', () => {
        expect(fileMetadata('refs/b.png', clean)).toEqual({})
      })
    })
  })

  describe('when the file is a standalone WebP', () => {
    describe('and it carries EXIF and XMP chunks after an odd-length chunk', () => {
      it('should find the metadata', () => {
        const file = webp(
          webpChunk('VP8X', Buffer.alloc(10)),
          webpChunk('VP8 ', Buffer.alloc(5)),
          webpChunk('EXIF', Buffer.alloc(3)),
          webpChunk('XMP ', Buffer.from('<x/>'))
        )
        expect(fileMetadata('assets/a.webp', file)).toEqual({ EXIF: 1, 'XMP ': 1 })
      })
    })

    describe('and it carries only rendering chunks', () => {
      it('should find nothing', () => {
        expect(fileMetadata('assets/b.webp', webp(webpChunk('VP8 ', Buffer.alloc(6))))).toEqual({})
      })
    })

    describe('and it is truncated inside a chunk header', () => {
      it('should report the chunks before the cut and stop', () => {
        const file = webp(webpChunk('EXIF', Buffer.alloc(2)), webpChunk('VP8 ', Buffer.alloc(4)))
        expect(fileMetadata('assets/c.webp', file.subarray(0, file.length - 9))).toEqual({
          EXIF: 1
        })
      })
    })

    describe('and its bytes are not a RIFF WebP container', () => {
      it('should find nothing', () => {
        expect(fileMetadata('assets/d.webp', Buffer.from('EXIF not a container'))).toEqual({})
      })
    })
  })

  describe('when the file is an SVG embedding a PNG', () => {
    let svg: Buffer

    beforeEach(() => {
      svg = Buffer.from(
        `<svg xmlns="http://www.w3.org/2000/svg"><image href="${uri(tagged)}"/></svg>`
      )
    })

    it('should find metadata in the embedded PNG', () => {
      expect(fileMetadata('assets/x.svg', svg)).toEqual({ tEXt: 1, eXIf: 1 })
    })
  })

  describe('when the file is CSS with a PNG data URI', () => {
    let css: Buffer

    beforeEach(() => {
      css = Buffer.from(`.a { background: url("${uri(tagged)}") }`)
    })

    it('should find metadata in the data URI', () => {
      expect(fileMetadata('x.module.css', css)).toEqual({
        tEXt: 1,
        eXIf: 1
      })
    })
  })

  describe('when a captured component page plants the chunk in each location ingest sanitizes', () => {
    const page = async () => {
      const raw = JSON.parse(
        await readFile(path.join(snapshotDir, 'components', 'Demo Cursors.json'), 'utf8')
      )
      // Start from a clean page so only the planted chunk is found.
      return stripImageMetadata(rawComponentPageSchema.parse(raw))
    }
    const write = (value: unknown) => Buffer.from(JSON.stringify(value, null, 2))
    const file = 'library/ui/figma/figma-snapshot/components/Demo Cursors.json'
    let captured: RawComponentPage

    beforeEach(async () => {
      captured = await page()
    })

    describe.each([
      [
        'node.png',
        (node: Record<string, unknown>) => ({ ...node, png: tagged.toString('base64') })
      ],
      [
        'a mask png',
        (node: Record<string, unknown>) => ({ ...node, mask: { png: tagged.toString('base64') } })
      ],
      [
        'a mask svg',
        (node: Record<string, unknown>) => ({
          ...node,
          mask: { svg: `<svg><image href="${uri(tagged)}"/></svg>` }
        })
      ],
      [
        'a PNG inside node.svg',
        (node: Record<string, unknown>) => ({
          ...node,
          svg: `<svg><image href="${uri(tagged)}"/></svg>`
        })
      ]
    ])('and the chunk is planted in %s', (_location, plant) => {
      beforeEach(() => {
        const tree = captured.components[0]!.base.tree as unknown as Record<string, unknown>
        const real = structuredClone(tree)
        delete real.children
        delete real.svg
        delete real.png
        delete real.mask
        captured.components[0]!.base.tree = { ...tree, children: [plant(real)] } as never
      })

      it('should find it', () => {
        expect(fileMetadata(file, write(captured))).toEqual({ tEXt: 1, eXIf: 1 })
      })
    })

    describe('and the chunk is planted in the image-fill map', () => {
      beforeEach(() => {
        captured.components[0]!.images = { photo: uri(tagged) }
      })

      it('should find it', () => {
        expect(fileMetadata(file, write(captured))).toEqual({ tEXt: 1, eXIf: 1 })
      })
    })

    describe('and the page does not match the ingest schema', () => {
      it('should reject the capture page', () => {
        expect(() => fileMetadata(file, write({ components: 'nope' }))).toThrow(
          /expected array, received string/
        )
      })
    })
  })

  describe('when the file is a generated SVG asset', () => {
    let svg: Buffer

    beforeEach(() => {
      svg = Buffer.from(
        `<svg xmlns="http://www.w3.org/2000/svg"><image href="${uri(tagged)}"/></svg>`
      )
    })

    it('should find the metadata', () => {
      expect(
        fileMetadata('library/ui/figma/review/src/generated/components/assets/x.svg', svg)
      ).toEqual({ tEXt: 1, eXIf: 1 })
    })
  })

  describe('when the file is a generated CSS Module', () => {
    let css: Buffer

    beforeEach(() => {
      css = Buffer.from(`.a { background: url("${uri(tagged)}") center / contain }`)
    })

    it('should find the metadata', () => {
      expect(
        fileMetadata('library/ui/figma/review/src/generated/components/x/X.module.css', css)
      ).toEqual({ tEXt: 1, eXIf: 1 })
    })
  })

  describe('when the file is any other JSON holding png fields, SVG bodies and image fills', () => {
    let capture: unknown

    beforeEach(() => {
      capture = {
        components: [
          {
            base: {
              tree: {
                png: tagged.toString('base64'),
                children: [{ svg: `<svg><image href="${uri(tagged)}"/></svg>` }]
              }
            },
            images: { photo: uri(tagged) }
          }
        ]
      }
    })

    it.each([
      ['compact', undefined],
      ['indented', 2]
    ])('should walk the %s JSON and find every raster', (_, indent) => {
      expect(
        fileMetadata(
          'review/src/review/catalog.json',
          Buffer.from(JSON.stringify(capture, null, indent))
        )
      ).toEqual({
        tEXt: 3,
        eXIf: 3
      })
    })
  })

  describe('when a JSON field named png is not a PNG', () => {
    it('should ignore it', () => {
      expect(
        fileMetadata('x.json', Buffer.from(JSON.stringify({ png: 'not base64 png' })))
      ).toEqual({})
    })
  })
})

describe('gateProblems', () => {
  const prefix = 'library/ui/components/assets/'
  const entry = (path: string, reason = 'NavBar, owner-pending', expires = 'later') => ({
    path,
    reason,
    expires
  })
  let found: Map<string, Record<string, number>>

  beforeEach(() => {
    found = new Map([[`${prefix}a.svg`, { iTXt: 1 }]])
  })

  describe('when the metadata is exactly allowlisted', () => {
    it('should pass it', () => {
      expect(gateProblems(found, { files: [entry(`${prefix}a.svg`)] }, prefix)).toEqual([])
    })
  })

  describe('when the metadata is not allowlisted', () => {
    it('should report it', () => {
      expect(gateProblems(found, { files: [] }, prefix)).toEqual([
        `${prefix}a.svg: {"iTXt":1} (not allowlisted)`
      ])
    })
  })

  describe('when an allowlisted file has become clean', () => {
    it('should report it so the exception is removed', () => {
      expect(
        gateProblems(found, { files: [entry(`${prefix}a.svg`), entry(`${prefix}b.svg`)] }, prefix)
      ).toEqual([`${prefix}b.svg: allowlisted but clean or missing; remove the entry`])
    })
  })

  describe('when an entry is outside the public assets', () => {
    let outside: Map<string, Record<string, number>>

    beforeEach(() => {
      outside = new Map([['library/ui/figma/x.png', { tEXt: 1 }]])
    })

    it('should report the entry', () => {
      expect(gateProblems(outside, { files: [entry('library/ui/figma/x.png')] }, prefix)).toEqual([
        `library/ui/figma/x.png: allowlist entry outside ${prefix}`
      ])
    })
  })

  describe('when an entry has no reason or expiry', () => {
    it('should report the entry', () => {
      expect(gateProblems(found, { files: [entry(`${prefix}a.svg`, '', '')] }, prefix)).toEqual([
        `${prefix}a.svg: allowlist entry without a reason or an expiry`
      ])
    })
  })

  describe('when a path is allowlisted more than once', () => {
    it('should report the duplicate', () => {
      expect(
        gateProblems(found, { files: [entry(`${prefix}a.svg`), entry(`${prefix}a.svg`)] }, prefix)
      ).toEqual([`${prefix}a.svg: allowlisted more than once`])
    })
  })
})

describe('escaped JSON', () => {
  // JSON may escape "/" and any character; only a parsed walk sees what a raw pattern misses.
  describe('when a JSON SVG string writes the PNG data URI as data:image\\/png', () => {
    let escaped: string

    beforeEach(() => {
      escaped = `{"node":{"svg":"<svg><image href=\\"${uri(tagged).replace('/', '\\/')}\\"/></svg>"}}`
    })

    it('should hold the escaped data URI', () => {
      expect(escaped).toContain('data:image\\/png')
    })

    it('should find the PNG', () => {
      expect(fileMetadata('review/src/review/catalog.json', Buffer.from(escaped))).toEqual({
        tEXt: 1,
        eXIf: 1
      })
    })
  })

  describe('when a png field name is written as \\u0070ng', () => {
    let escaped: string

    beforeEach(() => {
      escaped = `{"node":{"\\u0070ng":"${tagged.toString('base64')}"}}`
    })

    it('should find the PNG', () => {
      expect(fileMetadata('review/src/review/catalog.json', Buffer.from(escaped))).toEqual({
        tEXt: 1,
        eXIf: 1
      })
    })
  })

  describe('when a captured component page parsed by the ingest schema carries both escapes', () => {
    let text: string

    beforeEach(async () => {
      const clean = stripImageMetadata(
        rawComponentPageSchema.parse(
          JSON.parse(
            await readFile(path.join(snapshotDir, 'components', 'Demo Cursors.json'), 'utf8')
          )
        )
      )
      const tree = clean.components[0]!.base.tree as unknown as Record<string, unknown>
      const real = structuredClone(tree)
      delete real.children
      delete real.svg
      delete real.png
      delete real.mask
      clean.components[0]!.base.tree = {
        ...tree,
        children: [
          { ...real, svg: `<svg><image href="${uri(tagged)}"/></svg>` },
          { ...real, png: tagged.toString('base64') }
        ]
      } as never
      text = JSON.stringify(clean)
        .replaceAll('data:image/png', 'data:image\\/png')
        .replaceAll('"png":', '"\\u0070ng":')
    })

    it('should hold the escaped field name', () => {
      expect(text).toContain('\\u0070ng')
    })

    it('should find both escapes', () => {
      expect(
        fileMetadata(
          'library/ui/figma/figma-snapshot/components/Demo Cursors.json',
          Buffer.from(text)
        )
      ).toEqual({
        tEXt: 2,
        eXIf: 2
      })
    })
  })
})
