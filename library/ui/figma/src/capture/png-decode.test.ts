import { crc32, deflateSync } from 'node:zlib'
import { describe, expect, it } from 'vitest'
import { decodePng } from './png-decode.js'

// A minimal spec-conformant PNG writer for test inputs: rows are given as unfiltered sample bytes and
// filtered here with the requested filter type, so decoding must reproduce them exactly.
const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])

function chunk(type: string, data: Buffer) {
  const length = Buffer.alloc(4)
  length.writeUInt32BE(data.length)
  const body = Buffer.concat([Buffer.from(type, 'latin1'), data])
  const crc = Buffer.alloc(4)
  crc.writeUInt32BE(crc32(body))
  return Buffer.concat([length, body, crc])
}

function ihdr(width: number, height: number, bitDepth: number, colorType: number, interlace = 0) {
  const data = Buffer.alloc(13)
  data.writeUInt32BE(width, 0)
  data.writeUInt32BE(height, 4)
  data.set([bitDepth, colorType, 0, 0, interlace], 8)
  return chunk('IHDR', data)
}

function paeth(a: number, b: number, c: number) {
  const p = a + b - c
  const pa = Math.abs(p - a)
  const pb = Math.abs(p - b)
  const pc = Math.abs(p - c)
  return pa <= pb && pa <= pc ? a : pb <= pc ? b : c
}

// Filters each row (spec section 9) and prefixes its filter byte.
function filterRows(rows: Buffer[], bpp: number, filter: number): Buffer {
  return Buffer.concat(
    rows.map((row, y) => {
      const out = Buffer.alloc(row.length + 1)
      out[0] = filter
      for (let x = 0; x < row.length; x += 1) {
        const left = x >= bpp ? row[x - bpp]! : 0
        const up = y > 0 ? rows[y - 1]![x]! : 0
        const corner = x >= bpp && y > 0 ? rows[y - 1]![x - bpp]! : 0
        const predictor = [0, left, up, (left + up) >> 1, paeth(left, up, corner)][filter]!
        out[x + 1] = (row[x]! - predictor) & 0xff
      }
      return out
    })
  )
}

const png = (...chunks: Buffer[]) =>
  Buffer.concat([SIGNATURE, ...chunks, chunk('IEND', Buffer.alloc(0))])
const idat = (data: Buffer) => chunk('IDAT', deflateSync(data))

describe('decodePng', () => {
  describe('when an 8-bit RGBA image uses each filter type', () => {
    it.each([0, 1, 2, 3, 4])('should reproduce the samples through filter %i', (filter) => {
      const rows = [
        Buffer.from([10, 20, 30, 255, 200, 100, 50, 128, 1, 2, 3, 4]),
        Buffer.from([90, 80, 70, 60, 5, 250, 125, 0, 33, 66, 99, 132])
      ]

      const decoded = decodePng(png(ihdr(3, 2, 8, 6), idat(filterRows(rows, 4, filter))))

      expect(decoded.problem).toBeUndefined()
      expect(decoded.png!.pixels).toEqual(Buffer.concat(rows))
    })
  })

  describe('when the image is not 8-bit RGBA', () => {
    it.each([
      ['1-bit greyscale', 1, 0, 10, [Buffer.from([0b10110010, 0b01000000])]],
      ['16-bit greyscale', 16, 0, 2, [Buffer.from([0x12, 0x34, 0xff, 0xfe])]],
      ['8-bit grey with alpha', 8, 4, 2, [Buffer.from([1, 2, 3, 4])]],
      ['16-bit RGB', 16, 2, 1, [Buffer.from([1, 2, 3, 4, 5, 6])]],
      ['16-bit RGBA', 16, 6, 1, [Buffer.from([1, 2, 3, 4, 5, 6, 7, 8])]]
    ] as const)('should decode %s', (_name, depth, colour, width, rows) => {
      const bpp = Math.max(1, (depth * { 0: 1, 2: 3, 4: 2, 6: 4 }[colour]) >> 3)

      const decoded = decodePng(
        png(ihdr(width, rows.length, depth, colour), idat(filterRows([...rows], bpp, 2)))
      )

      expect(decoded.problem).toBeUndefined()
      expect(decoded.png!.pixels).toEqual(Buffer.concat([...rows]))
    })

    it('should decode a 4-bit palette image whose indices are all in its PLTE', () => {
      const plte = chunk('PLTE', Buffer.from([0, 0, 0, 255, 0, 0, 0, 255, 0]))
      const rows = [Buffer.from([0x01, 0x20])]

      const decoded = decodePng(png(ihdr(3, 1, 4, 3), plte, idat(filterRows(rows, 1, 0))))

      expect(decoded.problem).toBeUndefined()
      expect(decoded.png!.pixels).toEqual(rows[0])
    })
  })

  describe('when the image is Adam7 interlaced', () => {
    it('should validate and unfilter every pass, in pass order', () => {
      // 3x3 8-bit grey: passes 1 (1x1), 4 (1x1: x=2,y=0), 5 (2x1: y=2), 6 (1x2: x=1), 7 (3x1: y=1)
      // after skipping the empty passes 2 and 3.
      const passes = [
        [Buffer.from([1])],
        [Buffer.from([2])],
        [Buffer.from([3, 4])],
        [Buffer.from([5]), Buffer.from([6])],
        [Buffer.from([7, 8, 9])]
      ]
      const data = Buffer.concat(passes.map((rows) => filterRows(rows, 1, 1)))

      const decoded = decodePng(png(ihdr(3, 3, 8, 0, 1), idat(data)))

      expect(decoded.problem).toBeUndefined()
      expect(decoded.png!.interlaced).toBe(true)
      expect(decoded.png!.pixels).toEqual(Buffer.from([1, 2, 3, 4, 5, 6, 7, 8, 9]))
    })

    it('should reject passes whose data does not match the image size', () => {
      const decoded = decodePng(png(ihdr(3, 3, 8, 0, 1), idat(Buffer.alloc(5))))

      expect(decoded.problem).toBe('PNG image data does not match its size')
    })
  })

  describe('when the image uses something this decoder does not implement', () => {
    const plain = () => idat(filterRows([Buffer.from([1, 2, 3, 4])], 4, 0))

    it('should reject an unknown critical chunk instead of skipping it', () => {
      expect(
        decodePng(png(ihdr(1, 1, 8, 6), chunk('ABCD', Buffer.alloc(1)), plain())).problem
      ).toBe('unsupported critical PNG chunk ABCD; re-export the image as a plain PNG')
    })

    it('should still accept an unknown ancillary chunk', () => {
      expect(
        decodePng(png(ihdr(1, 1, 8, 6), chunk('abCD', Buffer.alloc(1)), plain())).problem
      ).toBeUndefined()
    })

    it('should reject an interlace method other than none or Adam7', () => {
      expect(decodePng(png(ihdr(1, 1, 8, 6, 2), plain())).problem).toMatch(
        /^unsupported PNG header/
      )
    })

    it('should reject a bit depth the colour type does not allow', () => {
      expect(decodePng(png(ihdr(1, 1, 4, 2), plain())).problem).toMatch(/^unsupported PNG header/)
    })

    // Audit's synthetic reproduction: a 1x1 colour type 3 image, valid IHDR/IDAT/IEND CRCs, no PLTE.
    it('should reject a palette image without PLTE', () => {
      expect(decodePng(png(ihdr(1, 1, 8, 3), idat(Buffer.from([0, 0])))).problem).toBe(
        'palette PNG without PLTE'
      )
    })

    it('should reject a PLTE with more entries than its bit depth can index, or a broken length', () => {
      const three = chunk('PLTE', Buffer.from([0, 0, 0, 1, 1, 1, 2, 2, 2]))
      const ragged = chunk('PLTE', Buffer.from([0, 0, 0, 1]))

      expect(decodePng(png(ihdr(1, 1, 1, 3), three, idat(Buffer.from([0, 0])))).problem).toBe(
        'PNG PLTE of invalid length for 1-bit indices'
      )
      expect(decodePng(png(ihdr(1, 1, 8, 3), ragged, idat(Buffer.from([0, 0])))).problem).toBe(
        'PNG PLTE of invalid length for 8-bit indices'
      )
    })

    it('should reject a palette index outside its PLTE', () => {
      const plte = chunk('PLTE', Buffer.from([0, 0, 0]))

      expect(decodePng(png(ihdr(1, 1, 8, 3), plte, idat(Buffer.from([0, 5])))).problem).toBe(
        'palette index 5 outside a 1-entry PLTE'
      )
    })

    it('should reject a greyscale image with a PLTE chunk', () => {
      const plte = chunk('PLTE', Buffer.from([0, 0, 0]))

      expect(decodePng(png(ihdr(1, 1, 8, 0), plte, idat(Buffer.from([0, 7])))).problem).toBe(
        'greyscale PNG with a PLTE chunk'
      )
    })

    it('should reject a file whose first chunk is not IHDR', () => {
      expect(decodePng(Buffer.concat([SIGNATURE, plain(), ihdr(1, 1, 8, 6)])).problem).toBe(
        'PNG does not start with IHDR'
      )
    })

    it('should reject an invalid filter type', () => {
      expect(decodePng(png(ihdr(1, 1, 8, 6), idat(Buffer.from([7, 1, 2, 3, 4])))).problem).toBe(
        'invalid filter type 7 on row 0'
      )
    })
  })

  describe('when the chunk structure is malformed', () => {
    const body = () => idat(filterRows([Buffer.from([1, 2, 3, 4])], 4, 0))

    it('should reject a second IHDR', () => {
      expect(decodePng(png(ihdr(1, 1, 8, 6), ihdr(1, 1, 8, 6), body())).problem).toBe(
        'PNG with more than one IHDR'
      )
    })

    it('should reject a PLTE after the image data or a repeated PLTE', () => {
      const plte = chunk('PLTE', Buffer.from([0, 0, 0]))
      const index = idat(Buffer.from([0, 0]))

      expect(decodePng(png(ihdr(1, 1, 8, 3), index, plte)).problem).toBe(
        'PNG PLTE repeated or after image data'
      )
      expect(decodePng(png(ihdr(1, 1, 8, 3), plte, plte, index)).problem).toBe(
        'PNG PLTE repeated or after image data'
      )
    })

    it('should reject image data split by another chunk', () => {
      const data = deflateSync(filterRows([Buffer.from([1, 2, 3, 4])], 4, 0))
      const split = [
        chunk('IDAT', data.subarray(0, 4)),
        chunk('tEXt', Buffer.from('a\0b')),
        chunk('IDAT', data.subarray(4))
      ]

      expect(decodePng(png(ihdr(1, 1, 8, 6), ...split)).problem).toBe(
        'PNG image data is not contiguous'
      )
    })

    it('should accept image data split across consecutive IDAT chunks', () => {
      const data = deflateSync(filterRows([Buffer.from([1, 2, 3, 4])], 4, 0))

      expect(
        decodePng(
          png(ihdr(1, 1, 8, 6), chunk('IDAT', data.subarray(0, 4)), chunk('IDAT', data.subarray(4)))
        ).problem
      ).toBeUndefined()
    })

    it('should reject an IEND that carries data, and bytes after IEND', () => {
      const withData = Buffer.concat([
        SIGNATURE,
        ihdr(1, 1, 8, 6),
        body(),
        chunk('IEND', Buffer.from([1]))
      ])
      const trailing = Buffer.concat([png(ihdr(1, 1, 8, 6), body()), Buffer.from([0])])

      expect(decodePng(withData).problem).toBe('PNG IEND with data')
      expect(decodePng(trailing).problem).toBe('PNG with data after IEND')
    })
  })

  describe('when the declared or inflated size is out of bounds', () => {
    // Each IDAT is a few bytes and is never inflated: the header alone is rejected.
    it.each([
      [
        'a side over 8192 px',
        9000,
        1,
        8,
        6,
        'PNG too large to decode (9000x1; limits 8192 px per side, 16777216 pixels)'
      ],
      [
        'more than 16777216 pixels',
        5000,
        5000,
        8,
        6,
        'PNG too large to decode (5000x5000; limits 8192 px per side, 16777216 pixels)'
      ],
      [
        'more than the decoded-byte budget',
        4096,
        4096,
        16,
        6,
        'PNG too large to decode (4096x4096, over 134217728 bytes)'
      ]
    ] as const)(
      'should reject %s before inflating anything',
      (_case, width, height, depth, colour, message) => {
        expect(
          decodePng(png(ihdr(width, height, depth, colour), idat(Buffer.alloc(1)))).problem
        ).toBe(message)
      }
    )

    it('should stop inflating at the declared size when the image data expands further', () => {
      // A 1x1 RGBA image needs 5 bytes; this IDAT inflates to 1 MiB of zeros.
      expect(decodePng(png(ihdr(1, 1, 8, 6), idat(Buffer.alloc(1024 * 1024)))).problem).toBe(
        'PNG image data inflates beyond its size'
      )
    })

    it('should reject image data that inflates to less than the declared size', () => {
      expect(decodePng(png(ihdr(1, 1, 8, 6), idat(Buffer.from([0, 1, 2])))).problem).toBe(
        'PNG image data does not match its size'
      )
    })
  })
})
