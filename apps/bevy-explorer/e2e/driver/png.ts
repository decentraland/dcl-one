// PNG in and out with Node's zlib only: 8-bit greyscale, grey+alpha, RGB and RGBA, no interlace
// (what the engine's `/screenshot` writes and what the references keep). Decoded pixels are RGBA.
import { deflateSync, inflateSync } from 'node:zlib'

export interface Image {
  width: number
  height: number
  /** RGBA, row-major, 4 bytes per pixel. */
  rgba: Uint8Array
}

const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])
const CHANNELS: Record<number, number> = { 0: 1, 2: 3, 4: 2, 6: 4 }

const CRC_TABLE = new Uint32Array(256).map((_, n) => {
  let c = n
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
  return c >>> 0
})

function crc32(bytes: Uint8Array): number {
  let crc = 0xffffffff
  for (const byte of bytes) crc = CRC_TABLE[(crc ^ byte) & 0xff]! ^ (crc >>> 8)
  return (crc ^ 0xffffffff) >>> 0
}

function paeth(a: number, b: number, c: number): number {
  const p = a + b - c
  const pa = Math.abs(p - a)
  const pb = Math.abs(p - b)
  const pc = Math.abs(p - c)
  if (pa <= pb && pa <= pc) return a
  return pb <= pc ? b : c
}

export function decodePng(file: Uint8Array): Image {
  const data = Buffer.from(file.buffer, file.byteOffset, file.byteLength)
  if (!data.subarray(0, 8).equals(SIGNATURE)) throw new Error('not a PNG')
  let width = 0
  let height = 0
  let colorType = -1
  const idat: Buffer[] = []
  let offset = 8
  while (offset + 8 <= data.length) {
    const length = data.readUInt32BE(offset)
    const type = data.toString('latin1', offset + 4, offset + 8)
    const body = data.subarray(offset + 8, offset + 8 + length)
    if (type === 'IHDR') {
      width = body.readUInt32BE(0)
      height = body.readUInt32BE(4)
      const bitDepth = body[8]
      colorType = body[9]!
      if (bitDepth !== 8) throw new Error(`unsupported PNG bit depth ${bitDepth}`)
      if (!(colorType in CHANNELS)) throw new Error(`unsupported PNG colour type ${colorType}`)
      if (body[12] !== 0) throw new Error('interlaced PNG is not supported')
    } else if (type === 'IDAT') {
      idat.push(body)
    } else if (type === 'IEND') {
      break
    }
    offset += 12 + length
  }
  if (colorType < 0) throw new Error('PNG without IHDR')
  const channels = CHANNELS[colorType]!
  const stride = width * channels
  const raw = inflateSync(Buffer.concat(idat))
  if (raw.length < height * (stride + 1)) throw new Error('PNG image data is short')
  const pixels = new Uint8Array(height * stride)
  let previous = new Uint8Array(stride)
  for (let row = 0; row < height; row++) {
    const filter = raw[row * (stride + 1)]
    const line = raw.subarray(row * (stride + 1) + 1, (row + 1) * (stride + 1))
    const current = pixels.subarray(row * stride, (row + 1) * stride)
    for (let i = 0; i < stride; i++) {
      const left = i >= channels ? current[i - channels]! : 0
      const up = previous[i]!
      const upLeft = i >= channels ? previous[i - channels]! : 0
      let value = line[i]!
      switch (filter) {
        case 0:
          break
        case 1:
          value += left
          break
        case 2:
          value += up
          break
        case 3:
          value += (left + up) >> 1
          break
        case 4:
          value += paeth(left, up, upLeft)
          break
        default:
          throw new Error(`unknown PNG filter ${filter} on row ${row}`)
      }
      current[i] = value & 0xff
    }
    previous = current
  }
  const rgba = new Uint8Array(width * height * 4)
  for (let p = 0; p < width * height; p++) {
    const source = p * channels
    switch (channels) {
      case 1:
        rgba.set([pixels[source]!, pixels[source]!, pixels[source]!, 255], p * 4)
        break
      case 2:
        rgba.set([pixels[source]!, pixels[source]!, pixels[source]!, pixels[source + 1]!], p * 4)
        break
      case 3:
        rgba.set([pixels[source]!, pixels[source + 1]!, pixels[source + 2]!, 255], p * 4)
        break
      default:
        rgba.set(pixels.subarray(source, source + 4), p * 4)
    }
  }
  return { width, height, rgba }
}

function chunk(type: string, body: Uint8Array): Buffer {
  const head = Buffer.alloc(8)
  head.writeUInt32BE(body.length, 0)
  head.write(type, 4, 'latin1')
  const crc = Buffer.alloc(4)
  crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), body])), 0)
  return Buffer.concat([head, body, crc])
}

/** An RGBA image as a PNG (colour type 6, filter 0 on every row). `filter` picks another filter
 *  for every row, for tests of the decoder. */
export function encodePng(image: Image, filter: 0 | 1 | 2 | 3 | 4 = 0): Buffer {
  const { width, height, rgba } = image
  if (rgba.length !== width * height * 4) throw new Error('rgba does not match width x height')
  const stride = width * 4
  const raw = Buffer.alloc(height * (stride + 1))
  let previous: Uint8Array = new Uint8Array(stride)
  for (let row = 0; row < height; row++) {
    const current: Uint8Array = rgba.subarray(row * stride, (row + 1) * stride)
    raw[row * (stride + 1)] = filter
    for (let i = 0; i < stride; i++) {
      const left = i >= 4 ? current[i - 4]! : 0
      const up = previous[i]!
      const upLeft = i >= 4 ? previous[i - 4]! : 0
      let predictor = 0
      if (filter === 1) predictor = left
      else if (filter === 2) predictor = up
      else if (filter === 3) predictor = (left + up) >> 1
      else if (filter === 4) predictor = paeth(left, up, upLeft)
      raw[row * (stride + 1) + 1 + i] = (current[i]! - predictor) & 0xff
    }
    previous = current
  }
  const ihdr = Buffer.alloc(13)
  ihdr.writeUInt32BE(width, 0)
  ihdr.writeUInt32BE(height, 4)
  ihdr[8] = 8
  ihdr[9] = 6
  return Buffer.concat([SIGNATURE, chunk('IHDR', ihdr), chunk('IDAT', deflateSync(raw)), chunk('IEND', new Uint8Array())])
}
