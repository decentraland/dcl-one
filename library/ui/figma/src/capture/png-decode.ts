import { crc32, inflateSync } from 'node:zlib'

// A bounded capture validator, not a full PNG implementation: structural checks (chunk CRCs and order,
// IHDR, PLTE, contiguous IDAT, a terminal IEND, no unknown critical chunk), size budgets checked before
// inflation, and a pixel decode of the sample stream for the formats it accepts. It does not apply colour
// management or ancillary colour/transparency chunks, and it does not certify spec conformance. Ingest
// uses it to reject truncated, forged or oversized rasters; tests use its samples to prove that metadata
// stripping leaves the image unchanged.
const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])
const CHANNELS: Record<number, number> = { 0: 1, 2: 3, 3: 1, 4: 2, 6: 4 }
const DEPTHS: Record<number, number[]> = {
  0: [1, 2, 4, 8, 16],
  2: [8, 16],
  3: [1, 2, 4, 8],
  4: [8, 16],
  6: [8, 16]
}
// Adam7 pass origins and steps: [x0, y0, dx, dy].
const ADAM7 = [
  [0, 0, 8, 8],
  [4, 0, 8, 8],
  [0, 4, 4, 8],
  [2, 0, 4, 4],
  [0, 2, 2, 4],
  [1, 0, 2, 2],
  [0, 1, 1, 2]
] as const

type DecodedPng = {
  width: number
  height: number
  bitDepth: number
  colorType: number
  interlaced: boolean
  // Unfiltered samples in the file's own format (palette indices, grey, RGB(A), at its bit depth):
  // scanlines in row order, or for an interlaced image the seven Adam7 passes in pass order (not
  // deinterlaced). For 8-bit RGBA this is the RGBA buffer.
  pixels: Buffer
}

type Decode = { png: DecodedPng; problem?: undefined } | { png?: undefined; problem: string }

// Critical chunks this decoder implements. Any other critical chunk (uppercase first letter) could
// change how the image decodes, so it is rejected instead of skipped.
const CRITICAL = new Set(['IHDR', 'PLTE', 'IDAT', 'IEND'])

// Budgets checked from IHDR before anything is inflated; inflation is then capped at exactly the
// size the header promises, so an over-expanding IDAT or a huge declared size cannot exhaust memory
// (offline ingest has no download limit). The largest real raster measured is a 1920x1597 Figma
// reference export (about 3.1 million pixels); captured artwork peaks at 1015x1563.
const MAX_DIMENSION = 8192
const MAX_PIXELS = 16 * 1024 * 1024
const MAX_DECODED_BYTES = 128 * 1024 * 1024

// Every palette index in the unfiltered rows must name a PLTE entry.
function paletteProblem(
  rows: Buffer,
  count: number,
  width: number,
  bitDepth: number,
  entries: number
): string | undefined {
  const rowBytes = Math.ceil((width * bitDepth) / 8)
  for (let y = 0; y < count; y += 1)
    for (let x = 0; x < width; x += 1) {
      const bit = x * bitDepth
      const byte = rows[y * rowBytes + (bit >> 3)]!
      const index = (byte >> (8 - bitDepth - (bit & 7))) & ((1 << bitDepth) - 1)
      if (index >= entries) return `palette index ${index} outside a ${entries}-entry PLTE`
    }
  return undefined
}

function paeth(a: number, b: number, c: number) {
  const p = a + b - c
  const pa = Math.abs(p - a)
  const pb = Math.abs(p - b)
  const pc = Math.abs(p - c)
  return pa <= pb && pa <= pc ? a : pb <= pc ? b : c
}

function unfilter(data: Buffer, rows: number, rowBytes: number, bpp: number): Buffer | string {
  const out = Buffer.alloc(rows * rowBytes)
  for (let y = 0; y < rows; y += 1) {
    const filter = data[y * (rowBytes + 1)]!
    if (filter > 4) return `invalid filter type ${filter} on row ${y}`
    const line = data.subarray(y * (rowBytes + 1) + 1, (y + 1) * (rowBytes + 1))
    for (let x = 0; x < rowBytes; x += 1) {
      const left = x >= bpp ? out[y * rowBytes + x - bpp]! : 0
      const up = y > 0 ? out[(y - 1) * rowBytes + x]! : 0
      const corner = x >= bpp && y > 0 ? out[(y - 1) * rowBytes + x - bpp]! : 0
      const predictor =
        filter === 1
          ? left
          : filter === 2
            ? up
            : filter === 3
              ? (left + up) >> 1
              : filter === 4
                ? paeth(left, up, corner)
                : 0
      out[y * rowBytes + x] = (line[x]! + predictor) & 0xff
    }
  }
  return out
}

function decodePng(bytes: Buffer): Decode {
  if (bytes.length < 8 || !bytes.subarray(0, 8).equals(SIGNATURE))
    return { problem: 'missing PNG signature' }
  let offset = 8
  let header: Buffer | undefined
  let palette: Buffer | undefined
  const idat: Buffer[] = []
  let ended = false
  let previous = ''
  while (offset + 12 <= bytes.length) {
    const length = bytes.readUInt32BE(offset)
    if (offset + 12 + length > bytes.length) return { problem: 'truncated PNG chunk' }
    const type = bytes.subarray(offset + 4, offset + 8).toString('latin1')
    const data = bytes.subarray(offset + 8, offset + 8 + length)
    if (
      crc32(bytes.subarray(offset + 4, offset + 8 + length)) !==
      bytes.readUInt32BE(offset + 8 + length)
    )
      return { problem: 'PNG CRC mismatch' }
    if (!/^[A-Za-z]{4}$/.test(type)) return { problem: 'invalid PNG chunk type' }
    if (offset === 8 && type !== 'IHDR') return { problem: 'PNG does not start with IHDR' }
    if (type === 'IHDR' && header) return { problem: 'PNG with more than one IHDR' }
    if (type === 'PLTE' && (palette || idat.length))
      return { problem: 'PNG PLTE repeated or after image data' }
    if (type === 'IDAT' && idat.length && previous !== 'IDAT')
      return { problem: 'PNG image data is not contiguous' }
    if (type === 'IEND' && length) return { problem: 'PNG IEND with data' }
    if (/^[A-Z]/.test(type) && !CRITICAL.has(type))
      return {
        problem: `unsupported critical PNG chunk ${type}; re-export the image as a plain PNG`
      }
    if (type === 'IHDR') header = data
    else if (type === 'PLTE') palette = data
    else if (type === 'IDAT') idat.push(data)
    offset += 12 + length
    previous = type
    if (type === 'IEND') {
      ended = true
      break
    }
  }
  if (!ended) return { problem: 'PNG without IEND' }
  if (offset !== bytes.length) return { problem: 'PNG with data after IEND' }
  if (!header || header.length !== 13) return { problem: 'PNG without a valid IHDR' }
  const width = header.readUInt32BE(0)
  const height = header.readUInt32BE(4)
  const [bitDepth, colorType, compression, filterMethod, interlace] = header.subarray(8, 13)
  if (!width || !height) return { problem: 'PNG with an empty size' }
  if (width > MAX_DIMENSION || height > MAX_DIMENSION || width * height > MAX_PIXELS)
    return {
      problem: `PNG too large to decode (${width}x${height}; limits ${MAX_DIMENSION} px per side, ${MAX_PIXELS} pixels)`
    }
  if (!DEPTHS[colorType!]?.includes(bitDepth!) || compression || filterMethod || interlace! > 1)
    return {
      problem:
        'unsupported PNG header (colour type, bit depth, compression, filter or interlace method)'
    }
  if (colorType === 3) {
    if (!palette) return { problem: 'palette PNG without PLTE' }
    if (palette.length % 3 || palette.length < 3 || palette.length / 3 > 2 ** bitDepth!)
      return { problem: `PNG PLTE of invalid length for ${bitDepth}-bit indices` }
  } else if (palette && (colorType === 0 || colorType === 4))
    return { problem: 'greyscale PNG with a PLTE chunk' }
  if (!idat.length) return { problem: 'PNG without image data' }
  const bits = bitDepth! * CHANNELS[colorType!]!
  const bpp = Math.max(1, bits >> 3)
  const rowBytes = (w: number) => Math.ceil((w * bits) / 8)
  const passSizes = interlace
    ? ADAM7.map(([x0, y0, dx, dy]) => {
        const w = Math.ceil((width - x0) / dx)
        const h = Math.ceil((height - y0) / dy)
        return w > 0 && h > 0 ? h * (rowBytes(w) + 1) : 0
      })
    : [height * (rowBytes(width) + 1)]
  const expected = passSizes.reduce((total, size) => total + size, 0)
  if (!Number.isSafeInteger(expected) || expected > MAX_DECODED_BYTES)
    return {
      problem: `PNG too large to decode (${width}x${height}, over ${MAX_DECODED_BYTES} bytes)`
    }
  let inflated: Buffer
  try {
    inflated = inflateSync(Buffer.concat(idat), { maxOutputLength: expected })
  } catch (error) {
    return {
      problem:
        error instanceof RangeError
          ? 'PNG image data inflates beyond its size'
          : 'PNG image data does not inflate'
    }
  }
  if (!interlace) {
    if (inflated.length !== height * (rowBytes(width) + 1))
      return { problem: 'PNG image data does not match its size' }
    const pixels = unfilter(inflated, height, rowBytes(width), bpp)
    if (typeof pixels === 'string') return { problem: pixels }
    const indexProblem =
      colorType === 3
        ? paletteProblem(pixels, height, width, bitDepth!, palette!.length / 3)
        : undefined
    if (indexProblem) return { problem: indexProblem }
    return {
      png: { width, height, bitDepth: bitDepth!, colorType: colorType!, interlaced: false, pixels }
    }
  }
  let cursor = 0
  const passes: Buffer[] = []
  for (const [x0, y0, dx, dy] of ADAM7) {
    const w = Math.ceil((width - x0) / dx)
    const h = Math.ceil((height - y0) / dy)
    if (w <= 0 || h <= 0) continue
    const size = h * (rowBytes(w) + 1)
    if (cursor + size > inflated.length)
      return { problem: 'PNG image data does not match its size' }
    const pass = unfilter(inflated.subarray(cursor, cursor + size), h, rowBytes(w), bpp)
    if (typeof pass === 'string') return { problem: pass }
    const indexProblem =
      colorType === 3 ? paletteProblem(pass, h, w, bitDepth!, palette!.length / 3) : undefined
    if (indexProblem) return { problem: indexProblem }
    passes.push(pass)
    cursor += size
  }
  if (cursor !== inflated.length) return { problem: 'PNG image data does not match its size' }
  return {
    png: {
      width,
      height,
      bitDepth: bitDepth!,
      colorType: colorType!,
      interlaced: true,
      pixels: Buffer.concat(passes)
    }
  }
}

export { decodePng }
export type { DecodedPng }
