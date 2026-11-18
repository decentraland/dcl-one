const IMAGES_HOST = 'images.ctfassets.net'

type ContentfulImageOptions = {
  /** Rendered width in CSS px; the request asks for twice that so high-density screens stay sharp. */
  width: number
  quality?: number
}

/**
 * Asks Contentful's Images API for a resized WebP of `url`, so the browser never downloads the uploaded
 * original. Any other URL (another host, an unparsable string) is returned untouched.
 */
export function contentfulImage(url: string, { width, quality = 80 }: ContentfulImageOptions): string {
  let parsed: URL
  try {
    parsed = new URL(url)
  } catch {
    return url
  }
  if (parsed.hostname !== IMAGES_HOST) return url
  parsed.searchParams.set('w', String(Math.round(width * 2)))
  parsed.searchParams.set('fm', 'webp')
  parsed.searchParams.set('q', String(quality))
  return parsed.toString()
}
