import { z } from 'zod'
import { LONG_READ_TIMEOUT_MS, type FigmaClient } from '../figma/client.js'
import { imageDataProblem, pngProblem, svgProblem } from '../capture/ingest-chunks.js'

const HOSTS = ['.figma.com', '.amazonaws.com', '.figmausercontent.com']

// Signed export URLs: https on a Figma or export-storage host only, no redirects, bounded size.
async function download(client: FigmaClient, address: string, label: string): Promise<Buffer> {
  const url = new URL(address)
  if (url.protocol !== 'https:' || !HOSTS.some((suffix) => url.hostname.endsWith(suffix)))
    throw new Error(`Unexpected export host for ${label}`)
  return client.download(url, label, 16 * 1024 * 1024)
}

// Image fills keep their own bytes; only raster formats an SVG <image> may reference are accepted.
function imageType(bytes: Buffer): string | undefined {
  if (bytes.subarray(0, 8).equals(Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])))
    return 'png'
  if (bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff) return 'jpeg'
  if (
    bytes
      .subarray(0, 6)
      .toString('latin1')
      .match(/^GIF8[79]a$/)
  )
    return 'gif'
  if (
    bytes.subarray(0, 4).toString('latin1') === 'RIFF' &&
    bytes.subarray(8, 12).toString('latin1') === 'WEBP'
  )
    return 'webp'
  return undefined
}

async function exportImageFills(client: FigmaClient, fileKey: string, refs: string[]) {
  const result = new Map<string, string>()
  const unique = [...new Set(refs)]
  if (!unique.length) return result
  const response = z
    .object({ meta: z.object({ images: z.record(z.string(), z.string().nullable()) }) })
    .parse(await client.get(`/v1/files/${fileKey}/images`, { timeoutMs: LONG_READ_TIMEOUT_MS }))
  for (const ref of unique) {
    const address = response.meta.images[ref]
    if (!address) throw new Error(`Figma did not provide image fill ${ref}`)
    const bytes = await download(client, address, `image fill ${ref}`)
    const type = imageType(bytes)
    // Only PNG is decoded at ingest; another raster type fails closed instead of being stored unchecked.
    if (type !== 'png')
      throw new Error(
        `Unsupported image fill format: ${ref} (${type ?? 'not an image'}; only PNG is decoded)`
      )
    const uri = `data:image/${type};base64,${bytes.toString('base64')}`
    const problem = imageDataProblem(uri)
    if (problem) throw new Error(`Figma image fill ${ref} is corrupt: ${problem}`)
    result.set(ref, uri)
  }
  return result
}

async function exportNodes(
  client: FigmaClient,
  fileKey: string,
  version: string,
  ids: string[],
  format: 'svg' | 'png',
  useAbsoluteBounds = true
) {
  const result = new Map<string, Buffer>()
  const unique = [...new Set(ids)]
  for (let start = 0; start < unique.length; start += 50) {
    const batch = unique.slice(start, start + 50)
    const query = new URLSearchParams({
      ids: batch.join(','),
      format,
      version,
      scale: '1',
      use_absolute_bounds: String(useAbsoluteBounds)
    })
    const response = z
      .object({ images: z.record(z.string(), z.string().nullable()) })
      .parse(
        await client.get(`/v1/images/${fileKey}?${query}`, { timeoutMs: LONG_READ_TIMEOUT_MS })
      )
    for (const id of batch) {
      const address = response.images[id]
      if (!address) throw new Error(`Figma did not render ${id}`)
      const body = await download(client, address, id)
      const problem =
        format === 'png' ? pngProblem(body.toString('base64')) : svgProblem(body.toString('utf8'))
      if (problem) throw new Error(`Invalid ${format} export for ${id}: ${problem}`)
      result.set(id, body)
    }
  }
  return result
}
export { download, exportImageFills, exportNodes, imageType }
