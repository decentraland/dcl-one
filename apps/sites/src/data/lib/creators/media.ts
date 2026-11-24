// Image/blob helpers for the add-items flow, ported from the legacy builder's modules/media/utils.
import { Rarity, WearableCategory } from '@dcl/schemas'

/** Palette size thumbnails and catalyst images are quantized to (legacy THUMBNAIL_PALETTE_COLORS). */
export const THUMBNAIL_PALETTE_COLORS = 256

export enum ImageType {
  PNG = 'png',
  GIF = 'gif',
  BMP = 'bmp',
  JPEG = 'jpeg',
  UNKNOWN = 'unknown'
}

export function dataURLToBlob(dataUrl: string): Blob | null {
  const arr = dataUrl.split(',')
  const boxedMime = /:(.*?);/.exec(arr[0])
  if (!boxedMime) return null
  const bstr = atob(arr[1])
  let n = bstr.length
  const u8arr = new Uint8Array(n)
  while (n--) {
    u8arr[n] = bstr.charCodeAt(n)
  }
  return new Blob([u8arr], { type: boxedMime[1] })
}

export async function blobToDataURL(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(reader.result as string)
    reader.onerror = () => reject(new Error('Could not read the file'))
    reader.readAsDataURL(blob)
  })
}

/**
 * Decodes a video's metadata; rejects when the browser can't play the file. Chrome defers media
 * loading in hidden tabs, so a stalled decode resolves without a duration instead of hanging.
 */
export function loadVideoMetadata(blob: Blob, timeoutMs = 15000): Promise<{ duration: number | null }> {
  return new Promise((resolve, reject) => {
    const video = document.createElement('video')
    const url = URL.createObjectURL(blob)
    let done = false
    const timer = setTimeout(() => finish({ duration: null }), timeoutMs)
    const finish = (result: { duration: number | null } | Error) => {
      if (done) return
      done = true
      clearTimeout(timer)
      video.removeAttribute('src')
      URL.revokeObjectURL(url)
      if (result instanceof Error) reject(result)
      else resolve(result)
    }
    video.preload = 'metadata'
    video.onloadedmetadata = () => finish({ duration: video.duration })
    video.onerror = () => finish(new Error('Invalid video'))
    video.src = url
  })
}

/** True type of an image, from its magic bytes — the file extension can lie. */
export async function getImageType(image: Blob): Promise<ImageType> {
  const buffer = await image.arrayBuffer()
  if (buffer.byteLength < 2) return ImageType.UNKNOWN
  const dv = new DataView(buffer, 0, 2)
  const magic = dv.getUint8(0).toString(16) + dv.getUint8(1).toString(16)
  switch (magic) {
    case '8950':
      return ImageType.PNG
    case '4749':
      return ImageType.GIF
    case '424d':
      return ImageType.BMP
    case 'ffd8':
      return ImageType.JPEG
    default:
      return ImageType.UNKNOWN
  }
}

async function loadImage(blob: Blob): Promise<HTMLImageElement> {
  const image = new Image()
  const loaded = new Promise<void>((resolve, reject) => {
    image.onload = () => resolve()
    image.onerror = () => reject(new Error('Could not load the image'))
  })
  image.src = await blobToDataURL(blob)
  await loaded
  return image
}

/** Draws the image onto a square canvas of the given size and returns it as a PNG blob. */
export async function resizeImage(blob: Blob, width = 256, height = 256): Promise<Blob> {
  const image = await loadImage(blob)
  const canvas = document.createElement('canvas')
  canvas.width = width
  canvas.height = height
  const ctx = canvas.getContext('2d')
  if (!ctx) throw new Error('Could not create a canvas context')
  ctx.drawImage(image, 0, 0, width, height)
  return new Promise((resolve, reject) => {
    canvas.toBlob(result => (result ? resolve(result) : reject(new Error('Could not encode the image'))), 'image/png')
  })
}

/** Largest side quantized in the page; the PNG decoder allocates width × height × 4 bytes up front. */
export const MAX_QUANTIZE_DIMENSION = 2048

let upng: Promise<typeof import('upng-js')> | null = null

/** upng-js (and the pako it bundles) load lazily, on the first compression. */
function loadUPNG(): Promise<typeof import('upng-js')> {
  upng ??= import('upng-js').then(module => module.default)
  return upng
}

/** Width and height from the IHDR chunk, which every PNG starts with, or null when the bytes are not a PNG. */
export function readPngDimensions(buffer: ArrayBuffer): { width: number; height: number } | null {
  if (buffer.byteLength < 24) return null
  const view = new DataView(buffer)
  const isPng = view.getUint32(0) === 0x89504e47 && view.getUint32(4) === 0x0d0a1a0a
  const isIhdr = view.getUint32(12) === 0x49484452
  if (!isPng || !isIhdr) return null
  return { width: view.getUint32(16), height: view.getUint32(20) }
}

/**
 * Re-encodes a PNG with an indexed palette, keeping alpha. There is no perceptual floor, only a size one: the
 * original blob is returned by reference when the result is not smaller, when decoding fails, for non-PNGs and
 * for images too large to decode in the page.
 */
export async function compressPngBlob(blob: Blob, colors = THUMBNAIL_PALETTE_COLORS): Promise<Blob> {
  if (blob.type && blob.type !== 'image/png') return blob
  try {
    const buffer = await blob.arrayBuffer()
    // Decoding is unbounded: a tiny file whose header claims a huge canvas would hang the tab.
    const size = readPngDimensions(buffer)
    if (!size || size.width > MAX_QUANTIZE_DIMENSION || size.height > MAX_QUANTIZE_DIMENSION) return blob
    const UPNG = await loadUPNG()
    const image = UPNG.decode(buffer)
    const frames = UPNG.toRGBA8(image)
    const encoded = UPNG.encode(frames, image.width, image.height, colors)
    const compressed = new Blob([encoded], { type: 'image/png' })
    return compressed.size > 0 && compressed.size < blob.size ? compressed : blob
  } catch {
    return blob
  }
}

/**
 * The catalyst image (legacy generateImage): the thumbnail drawn over the rarity's radial
 * gradient at 512x512. Without a canvas context (tests) the thumbnail itself is returned.
 */
export async function generateCatalystImage(thumbnail: Blob, rarity: string, size = 512): Promise<Blob> {
  const canvas = document.createElement('canvas')
  canvas.width = size
  canvas.height = size
  const context = canvas.getContext('2d')
  if (!context || !Rarity.validate(rarity)) return thumbnail

  const gradient = context.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 1.75)
  const [from, to] = Rarity.getGradient(rarity)
  gradient.addColorStop(0, from)
  gradient.addColorStop(1, to)
  context.fillStyle = gradient
  context.fillRect(0, 0, size, size)
  context.drawImage(await loadImage(thumbnail), 0, 0, size, size)

  return new Promise((resolve, reject) => {
    canvas.toBlob(
      blob => (blob ? resolve(blob) : reject(new Error('Could not generate the catalyst image'))),
      'image/png'
    )
  })
}

/**
 * Converts a facial-feature texture (eyes / eyebrows / mouth) into the 1024x1024 thumbnail,
 * padded from the top per category so the texture sits centered in the square.
 */
export async function convertImageIntoWearableThumbnail(
  blob: Blob,
  category: WearableCategory = WearableCategory.EYES
): Promise<string> {
  const image = await loadImage(blob)
  let padding = 128
  switch (category) {
    case WearableCategory.EYEBROWS:
    case WearableCategory.MOUTH:
      padding = 160
      break
    case WearableCategory.EYES:
      padding = 128
      break
  }
  const canvas = document.createElement('canvas')
  canvas.width = 1024
  canvas.height = 1024
  const ctx = canvas.getContext('2d')
  if (!ctx) throw new Error('Could not create a canvas context')
  ctx.drawImage(image, 0, padding, canvas.width, canvas.height)
  return canvas.toDataURL()
}
