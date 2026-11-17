import { crc32 } from 'node:zlib'
import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import { snapshotDir } from '../../test/snapshot.js'
import { rawComponentPageSchema } from '../ir/raw-components.js'
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
const uri = (bytes: Buffer) => `data:image/png;base64,${bytes.toString('base64')}`

describe('fileMetadata', () => {
  it('should find metadata in a standalone PNG', () => {
    expect(fileMetadata('refs/a.png', tagged)).toEqual({ tEXt: 1, eXIf: 1 })
    expect(fileMetadata('refs/b.png', clean)).toEqual({})
  })

  it('should find metadata in a PNG embedded in an SVG', () => {
    const svg = Buffer.from(
      `<svg xmlns="http://www.w3.org/2000/svg"><image href="${uri(tagged)}"/></svg>`
    )

    expect(fileMetadata('assets/x.svg', svg)).toEqual({ tEXt: 1, eXIf: 1 })
  })

  it('should find metadata in a CSS data URI', () => {
    expect(
      fileMetadata('x.module.css', Buffer.from(`.a { background: url("${uri(tagged)}") }`))
    ).toEqual({
      tEXt: 1,
      eXIf: 1
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

    it.each([
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
    ])('should find it in %s', async (_location, plant) => {
      const captured = await page()
      const tree = captured.components[0]!.base.tree as unknown as Record<string, unknown>
      const real = structuredClone(tree)
      delete real.children
      delete real.svg
      delete real.png
      delete real.mask
      captured.components[0]!.base.tree = { ...tree, children: [plant(real)] } as never

      expect(fileMetadata(file, write(captured))).toEqual({ tEXt: 1, eXIf: 1 })
    })

    it('should find it in the image-fill map', async () => {
      const captured = await page()
      captured.components[0]!.images = { photo: uri(tagged) }

      expect(fileMetadata(file, write(captured))).toEqual({ tEXt: 1, eXIf: 1 })
    })

    it('should reject a capture page that does not match the ingest schema', () => {
      expect(() => fileMetadata(file, write({ components: 'nope' }))).toThrow()
    })
  })

  it('should find it in a generated SVG asset and a generated CSS Module', () => {
    const svg = Buffer.from(
      `<svg xmlns="http://www.w3.org/2000/svg"><image href="${uri(tagged)}"/></svg>`
    )
    const css = Buffer.from(`.a { background: url("${uri(tagged)}") center / contain }`)

    expect(
      fileMetadata('library/ui/figma/review/src/generated/components/assets/x.svg', svg)
    ).toEqual({ tEXt: 1, eXIf: 1 })
    expect(
      fileMetadata('library/ui/figma/review/src/generated/components/x/X.module.css', css)
    ).toEqual({ tEXt: 1, eXIf: 1 })
  })

  it('should walk any other JSON, whatever its whitespace, for png fields, SVG bodies and image fills', () => {
    const capture = {
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

    for (const text of [JSON.stringify(capture), JSON.stringify(capture, null, 2)])
      expect(fileMetadata('review/src/review/catalog.json', Buffer.from(text))).toEqual({
        tEXt: 3,
        eXIf: 3
      })
  })

  it('should ignore a field named png that is not a PNG', () => {
    expect(fileMetadata('x.json', Buffer.from(JSON.stringify({ png: 'not base64 png' })))).toEqual(
      {}
    )
  })
})

describe('gateProblems', () => {
  const prefix = 'library/ui/components/assets/'
  const entry = (path: string, reason = 'NavBar, owner-pending', expires = 'later') => ({
    path,
    reason,
    expires
  })
  const found = new Map([[`${prefix}a.svg`, { iTXt: 1 }]])

  it('should pass metadata that is exactly allowlisted', () => {
    expect(gateProblems(found, { files: [entry(`${prefix}a.svg`)] }, prefix)).toEqual([])
  })

  it('should report metadata that is not allowlisted', () => {
    expect(gateProblems(found, { files: [] }, prefix)).toEqual([
      `${prefix}a.svg: {"iTXt":1} (not allowlisted)`
    ])
  })

  it('should report an allowlisted file that has become clean, so the exception is removed', () => {
    expect(
      gateProblems(found, { files: [entry(`${prefix}a.svg`), entry(`${prefix}b.svg`)] }, prefix)
    ).toEqual([`${prefix}b.svg: allowlisted but clean or missing; remove the entry`])
  })

  it('should report an entry outside the public assets, or without a reason or an expiry', () => {
    const outside = new Map([['library/ui/figma/x.png', { tEXt: 1 }]])

    expect(gateProblems(outside, { files: [entry('library/ui/figma/x.png')] }, prefix)).toEqual([
      `library/ui/figma/x.png: allowlist entry outside ${prefix}`
    ])
    expect(gateProblems(found, { files: [entry(`${prefix}a.svg`, '', '')] }, prefix)).toEqual([
      `${prefix}a.svg: allowlist entry without a reason or an expiry`
    ])
  })

  it('should report a path allowlisted more than once', () => {
    expect(
      gateProblems(found, { files: [entry(`${prefix}a.svg`), entry(`${prefix}a.svg`)] }, prefix)
    ).toEqual([`${prefix}a.svg: allowlisted more than once`])
  })
})

describe('escaped JSON', () => {
  // JSON may escape "/" and any character; only a parsed walk sees what a raw pattern misses.
  it('should find a PNG data URI written as data:image\\/png inside a JSON SVG string', () => {
    const escaped = `{"node":{"svg":"<svg><image href=\\"${uri(tagged).replace('/', '\\/')}\\"/></svg>"}}`
    expect(escaped).toContain('data:image\\/png')

    expect(fileMetadata('review/src/review/catalog.json', Buffer.from(escaped))).toEqual({
      tEXt: 1,
      eXIf: 1
    })
  })

  it('should find a png field whose name is written as \\u0070ng', () => {
    const escaped = `{"node":{"\\u0070ng":"${tagged.toString('base64')}"}}`

    expect(fileMetadata('review/src/review/catalog.json', Buffer.from(escaped))).toEqual({
      tEXt: 1,
      eXIf: 1
    })
  })

  it('should find both escapes in a captured component page parsed by the ingest schema', async () => {
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
    const text = JSON.stringify(clean)
      .replaceAll('data:image/png', 'data:image\\/png')
      .replaceAll('"png":', '"\\u0070ng":')
    expect(text).toContain('\\u0070ng')

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
