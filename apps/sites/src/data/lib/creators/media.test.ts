import { describe, expect, it } from 'vitest'
import UPNG from 'upng-js'
import { ImageType, compressPngBlob, dataURLToBlob, getImageType, readPngDimensions } from './media'

// 1x1 transparent PNG
const PNG_DATA_URL =
  'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR4nGNiYAAAAAkAAxkR2eQAAAAASUVORK5CYII='

describe('dataURLToBlob', () => {
  it('decodes a data URL into a typed blob', () => {
    const blob = dataURLToBlob(PNG_DATA_URL)
    expect(blob).not.toBeNull()
    expect(blob!.type).toBe('image/png')
    expect(blob!.size).toBeGreaterThan(0)
  })

  it('returns null for malformed input', () => {
    expect(dataURLToBlob('not-a-data-url')).toBeNull()
  })
})

describe('getImageType', () => {
  it('detects PNG magic bytes regardless of file name', async () => {
    const blob = dataURLToBlob(PNG_DATA_URL)!
    expect(await getImageType(blob)).toBe(ImageType.PNG)
  })

  it('reports unknown for arbitrary bytes', async () => {
    expect(await getImageType(new Blob([new Uint8Array([1, 2, 3, 4])]))).toBe(ImageType.UNKNOWN)
  })
})

describe('compressPngBlob', () => {
  function noisyPng(size = 64): Blob {
    const rgba = new Uint8Array(size * size * 4)
    let seed = 7
    for (let i = 0; i < rgba.length; i++) {
      seed = (seed * 1103515245 + 12345) & 0x7fffffff
      rgba[i] = seed & 0xff
    }
    return new Blob([UPNG.encode([rgba.buffer], size, size, 0)], { type: 'image/png' })
  }

  it('shrinks a true-color PNG to an indexed one that still decodes', async () => {
    const original = noisyPng()
    const compressed = await compressPngBlob(original)
    expect(compressed.size).toBeLessThan(original.size)
    expect(compressed.type).toBe('image/png')
    const decoded = UPNG.decode(await compressed.arrayBuffer())
    expect([decoded.width, decoded.height]).toEqual([64, 64])
  })

  it('leaves a PNG whose header claims a huge canvas untouched instead of decoding it', async () => {
    const header = new Uint8Array(await noisyPng(8).arrayBuffer())
    new DataView(header.buffer).setUint32(16, 30000)
    new DataView(header.buffer).setUint32(20, 30000)
    const huge = new Blob([header], { type: 'image/png' })
    expect(readPngDimensions(header.buffer)).toEqual({ width: 30000, height: 30000 })
    expect(await compressPngBlob(huge)).toBe(huge)
  })

  it('returns the input itself for non-PNGs and for bytes it cannot decode', async () => {
    const jpeg = new Blob([new Uint8Array([0xff, 0xd8, 0xff])], { type: 'image/jpeg' })
    expect(await compressPngBlob(jpeg)).toBe(jpeg)
    const broken = new Blob(['not a png'], { type: 'image/png' })
    expect(await compressPngBlob(broken)).toBe(broken)
  })
})
