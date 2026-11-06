import { mkdtemp, readdir, readFile, writeFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { crc32, deflateSync } from 'node:zlib'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { rawComponentPageSchema } from '../ir/raw-components.js'
import { stripImageMetadata } from './strip-image-metadata.js'
import { validatePage } from './ingest-chunks.js'
import { snapshotDir } from '../../test/snapshot.js'
import {
  checksum,
  imageDataProblem,
  ingestChunks,
  pngProblem,
  svgProblem,
  type CaptureChunk
} from './ingest-chunks.js'
import { encodePageFileName } from './page-file-name.js'

type Artwork = { svg?: string; png?: string; sum?: string; children?: Artwork[] }

const readPage = async (name: string) =>
  JSON.parse(
    await readFile(path.join(snapshotDir, 'components', encodePageFileName(name)), 'utf8')
  ) as {
    page: { id: string; name: string }
    components: Array<{ id: string; name: string; base: { tree: Artwork }; others: unknown[] }>
  }

const crosshair = async () => {
  const page = await readPage('Cursor')
  const component = page.components.find((entry) => entry.name === 'Crosshair')!
  return { page: page.page, component, svg: component.base.tree.children![0]! }
}

const chunkOf = (page: { id: string; name: string }, component: unknown): CaptureChunk => ({
  pages: [{ page, components: [component] }],
  resume: null
})

describe('ingestChunks', () => {
  describe('when the captured artwork matches its checksum', () => {
    it('should accept the real Crosshair circle and reproduce the checksum the plugin computed', async () => {
      const { svg } = await crosshair()

      expect(checksum(svg.svg!)).toBe(svg.sum)
      expect(svgProblem(svg.svg!)).toBeUndefined()
    })

    it('should accept a real PNG whose CRCs are intact', async () => {
      const cursors = await readPage('Cursor')
      const others = JSON.stringify(cursors)
      const png = /"png":"([^"]+)"/.exec(others)![1]!

      expect(pngProblem(png)).toBeUndefined()
    })
  })

  describe('when the artwork was altered in transit', () => {
    it('should reject an svg whose checksum no longer matches', async () => {
      const { page, component, svg } = await crosshair()
      const tampered = structuredClone(component)
      tampered.base.tree.children![0]!.svg = svg.svg!.replace('r="12.5"', 'r="12.6"')
      const dir = await mkdtemp(path.join(tmpdir(), 'ingest-'))

      await expect(ingestChunks(dir, [chunkOf(page, tampered)])).rejects.toThrow(
        /checksum mismatch/
      )
    })

    it('should reject a truncated svg and a PNG with a broken CRC', async () => {
      const { svg } = await crosshair()
      const cursors = JSON.stringify(await readPage('Cursor'))
      const png = /"png":"([^"]+)"/.exec(cursors)![1]!
      const broken = `${png.slice(0, 120)}A${png.slice(121)}`

      expect(svgProblem(svg.svg!.replace('</svg>', ''))).toMatch(/complete <svg>/)
      expect(pngProblem(broken)).toBeDefined()
    })
  })

  describe('when the same chunk is ingested twice', () => {
    it('should upsert by id and leave the page file byte for byte unchanged', async () => {
      const { page, component } = await crosshair()
      const dir = await mkdtemp(path.join(tmpdir(), 'ingest-'))
      const file = path.join(dir, 'components', 'Cursor.json')

      const first = await ingestChunks(dir, [chunkOf(page, component)])
      const afterFirst = await readFile(file, 'utf8')
      const second = await ingestChunks(dir, [chunkOf(page, component)])

      expect(first).toEqual([{ page: 'Cursor', components: 1, added: 1, replaced: 0, removed: 0 }])
      expect(second).toEqual([{ page: 'Cursor', components: 1, added: 0, replaced: 1, removed: 0 }])
      expect(await readFile(file, 'utf8')).toBe(afterFirst)
    })
  })

  describe('when a page is re-captured after a component was deleted in Figma', () => {
    const twoComponents = async () => {
      const cursors = await readPage('Cursor')
      const [kept, deleted] = cursors.components
      const dir = await mkdtemp(path.join(tmpdir(), 'ingest-'))
      await ingestChunks(dir, [
        { pages: [{ page: cursors.page, components: [kept, deleted] }], resume: null }
      ])
      return { page: cursors.page, kept: kept!, deleted: deleted!, dir }
    }
    const namesIn = async (dir: string, file: string) =>
      (
        JSON.parse(await readFile(path.join(dir, 'components', file), 'utf8')) as {
          components: Array<{ name: string }>
        }
      ).components.map((component) => component.name)

    it('should drop the deleted component when the chunks cover the whole page', async () => {
      const { page, kept, deleted, dir } = await twoComponents()

      const report = await ingestChunks(dir, [
        { pages: [{ page, total: 1, start: 0, components: [kept] }], resume: null }
      ])

      expect(report).toEqual([{ page: 'Cursor', components: 1, added: 0, replaced: 1, removed: 1 }])
      expect(await namesIn(dir, 'Cursor.json')).not.toContain(deleted.name)
    })

    it('should keep the other components when the chunks are a partial re-capture', async () => {
      const { page, kept, dir } = await twoComponents()

      const report = await ingestChunks(dir, [
        { pages: [{ page, total: 2, start: 0, components: [kept] }], resume: null }
      ])

      expect(report).toEqual([{ page: 'Cursor', components: 2, added: 0, replaced: 1, removed: 0 }])
    })
  })

  describe('when components are captured by id into the External page', () => {
    it('should keep the External components that were not part of this run', async () => {
      const external = await readPage('External')
      const [first, second] = external.components
      const dir = await mkdtemp(path.join(tmpdir(), 'ingest-'))
      await ingestChunks(dir, [
        { pages: [{ page: external.page, components: [first, second] }], resume: null }
      ])

      const report = await ingestChunks(dir, [
        { pages: [{ page: external.page, total: 1, start: 0, components: [second] }], resume: null }
      ])

      expect(report).toEqual([
        { page: 'External', components: 2, added: 0, replaced: 1, removed: 0 }
      ])
    })
  })

  describe('when a set was captured in variant parts', () => {
    const partsOf = async () => {
      const cursors = await readPage('Cursor')
      const set = cursors.components.find((component) => component.others.length >= 2)!
      const total = set.others.length
      const part = (start: number, end: number) => ({
        ...set,
        others: set.others.slice(start, end),
        variantRange: { start, total }
      })
      return { page: cursors.page, set, total, part }
    }
    const ingestParts = async (page: { id: string; name: string }, parts: unknown[]) => {
      const dir = await mkdtemp(path.join(tmpdir(), 'ingest-'))
      await ingestChunks(dir, [{ pages: [{ page, components: parts }], resume: null }])
      return JSON.parse(await readFile(path.join(dir, 'components', 'Cursor.json'), 'utf8')) as {
        components: Array<{ id: string; others: unknown[]; variantRange?: unknown }>
      }
    }

    it('should join the parts in variant order whatever order they arrive in', async () => {
      const { page, set, total, part } = await partsOf()

      const written = await ingestParts(page, [part(1, total), part(0, 1)])

      expect(written.components).toHaveLength(1)
      // Ingest stores the joined set with non-rendering image metadata stripped.
      const stripped = stripImageMetadata(rawComponentPageSchema.parse({ page, components: [set] }))
        .components[0]!
      expect(written.components[0]!.others).toEqual(stripped.others)
      expect(written.components[0]!.variantRange).toBeUndefined()
    })

    it('should refuse a set with a missing part instead of keeping fewer variants', async () => {
      const { page, total, part } = await partsOf()

      await expect(ingestParts(page, [part(1, total)])).rejects.toThrow(/missing variants 0-0/)
      await expect(ingestParts(page, [part(0, 1)])).rejects.toThrow(/missing variants 1-/)
    })
  })

  describe('when the page was renamed in Figma', () => {
    it('should write the page under its new name and remove the old file', async () => {
      const { page, component } = await crosshair()
      const dir = await mkdtemp(path.join(tmpdir(), 'ingest-'))
      await ingestChunks(dir, [chunkOf(page, component)])

      const report = await ingestChunks(dir, [chunkOf({ id: page.id, name: 'Cursors' }, component)])

      expect(report).toEqual([
        { page: 'Cursors', components: 1, added: 0, replaced: 1, removed: 0 }
      ])
      expect(await readdir(path.join(dir, 'components'))).toEqual(['Cursors.json'])
    })
  })

  describe('when a component carries image fills', () => {
    const realPng = async () =>
      /"png":"([^"]+)"/.exec(JSON.stringify(await readPage('Cursor')))![1]!
    const withImages = async (images: Record<string, string>) => {
      const { page, component } = await crosshair()
      return { page, components: [{ ...structuredClone(component), images }] } as Parameters<
        typeof validatePage
      >[0]
    }

    it('should accept a complete PNG data URI', async () => {
      const page = await withImages({ photo: `data:image/png;base64,${await realPng()}` })

      expect(() => validatePage(page)).not.toThrow()
    })

    it('should reject a PNG data URI that keeps its signature but is truncated', async () => {
      const png = Buffer.from(await realPng(), 'base64')
      const truncated = png.subarray(0, Math.floor(png.length / 2)).toString('base64')
      const page = await withImages({ photo: `data:image/png;base64,${truncated}` })

      expect(() => validatePage(page)).toThrow(/Crosshair\/images\/photo: truncated PNG chunk/)
    })

    it('should reject a payload that is not an image data URI', async () => {
      const page = await withImages({ photo: 'https://example.com/photo.png' })

      expect(() => validatePage(page)).toThrow(/not a base64 PNG data URI/)
    })

    it('should reject bytes that are not an image, and a Git LFS pointer, behind a PNG data URI', async () => {
      const html = Buffer.from('<!doctype html><title>not an image</title>').toString('base64')
      const pointer = Buffer.from(
        'version https://git-lfs.github.com/spec/v1\noid sha256:0000000000000000000000000000000000000000000000000000000000000000\nsize 12\n'
      ).toString('base64')
      const page = await withImages({
        html: `data:image/png;base64,${html}`,
        pointer: `data:image/png;base64,${pointer}`
      })

      expect(() => validatePage(page)).toThrow(/images\/html: missing PNG signature/)
      expect(() => validatePage(page)).toThrow(/images\/pointer: missing PNG signature/)
    })

    it('should reject a PNG whose chunks and CRCs are valid but whose pixels do not match its size', () => {
      const chunk = (type: string, data: Buffer) => {
        const length = Buffer.alloc(4)
        length.writeUInt32BE(data.length)
        const body = Buffer.concat([Buffer.from(type, 'latin1'), data])
        const crc = Buffer.alloc(4)
        crc.writeUInt32BE(crc32(body))
        return Buffer.concat([length, body, crc])
      }
      // IHDR says 4x4 RGBA, but IDAT inflates to a single 1x1 row.
      const forged = Buffer.concat([
        Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
        chunk('IHDR', Buffer.from([0, 0, 0, 4, 0, 0, 0, 4, 8, 6, 0, 0, 0])),
        chunk('IDAT', deflateSync(Buffer.from([0, 0, 0, 0, 0]))),
        chunk('IEND', Buffer.alloc(0))
      ])

      expect(imageDataProblem(`data:image/png;base64,${forged.toString('base64')}`)).toBe(
        'PNG image data does not match its size'
      )
    })

    it('should reject another raster type, since only PNG is decoded', () => {
      const jpeg = Buffer.from([0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, 0xff, 0xd9]).toString('base64')

      expect(imageDataProblem(`data:image/jpeg;base64,${jpeg}`)).toBe('not a base64 PNG data URI')
    })

    it('should not parse a capture whose image fill is not a PNG data URI', async () => {
      const { page, component } = await crosshair()

      expect(() =>
        rawComponentPageSchema.parse({
          page,
          components: [{ ...component, images: { photo: 'https://example.com/photo.png' } }]
        })
      ).toThrow(/image fill must be a PNG data URI/)
    })
  })
})

describe('when matching capture pages by portable filename and stable identity', () => {
  let dir: string
  let component: unknown
  beforeEach(async () => {
    dir = await mkdtemp(path.join(tmpdir(), 'capture-identity-'))
    component = (await crosshair()).component
  })
  afterEach(async () => {
    await rm(dir, { recursive: true, force: true })
  })
  it('should reject a name occupied by a different page ID without overwriting it', async () => {
    await ingestChunks(dir, [chunkOf({ id: '1:1', name: 'Shared' }, component)])
    const file = path.join(dir, 'components/Shared.json')
    const before = await readFile(file, 'utf8')
    await expect(
      ingestChunks(dir, [chunkOf({ id: '2:1', name: 'Shared' }, component)])
    ).rejects.toThrow(/identity/i)
    expect(await readFile(file, 'utf8')).toBe(before)
  })
  it('should refuse an ambiguous ID before renaming either capture', async () => {
    await ingestChunks(dir, [chunkOf({ id: '1:1', name: 'First' }, component)])
    await writeFile(
      path.join(dir, 'components/Second.json'),
      await readFile(path.join(dir, 'components/First.json'))
    )
    await expect(
      ingestChunks(dir, [chunkOf({ id: '1:1', name: 'Renamed' }, component)])
    ).rejects.toThrow(/Ambiguous/)
    expect((await readdir(path.join(dir, 'components'))).sort()).toEqual([
      'First.json',
      'Second.json'
    ])
  })
  it('should preserve a legacy page sharing the canonical page ID during rename', async () => {
    await ingestChunks(dir, [chunkOf({ id: '1:1', name: 'Legacy' }, component)])
    const legacy = await readFile(path.join(dir, 'components/Legacy.json'), 'utf8')
    await ingestChunks(dir, [chunkOf({ id: '1:1', name: 'Canonical' }, component)], ['Legacy'])
    await ingestChunks(dir, [chunkOf({ id: '1:1', name: 'Renamed' }, component)], ['Legacy'])
    expect(await readFile(path.join(dir, 'components/Legacy.json'), 'utf8')).toBe(legacy)
    expect((await readdir(path.join(dir, 'components'))).sort()).toEqual([
      'Legacy.json',
      'Renamed.json'
    ])
  })
  it('should allow recapturing the same NFC name on a normalizing filesystem', async () => {
    await ingestChunks(dir, [chunkOf({ id: '1:1', name: 'Café' }, component)])
    await expect(
      ingestChunks(dir, [chunkOf({ id: '1:1', name: 'Café' }, component)])
    ).resolves.toHaveLength(1)
  })
  it.each([
    ['Icons', 'icons'],
    ['Café', 'Cafe\u0301']
  ])('should reject a portable name collision between %s and %s', async (first, second) => {
    await ingestChunks(dir, [chunkOf({ id: '1:1', name: first }, component)])
    await expect(
      ingestChunks(dir, [chunkOf({ id: '2:1', name: second }, component)])
    ).rejects.toThrow(/collision/i)
  })
})
