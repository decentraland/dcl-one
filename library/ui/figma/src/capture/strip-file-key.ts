// The Figma file key is private. Captured JSON and every text written from it drop `fileKey` properties and
// replace Figma file URLs with the placeholder. It reads no environment: the key guard (key-guard.mjs) is the
// only reader of FIGMA_FILE_KEY, and fetch-source accepts a capture only when it matches the pinned digests.
// Node ids stay: they are not the key.
const FILE_URL =
  /https?:\/\/(?:www\.)?figma\.com\/(?:design|file|proto|board)\/[0-9A-Za-z]+[^\s"'<>)]*/g
const PLACEHOLDER = '<DCL_FIGMA_FILE>'

function stripFileKeyText(text: string): string {
  return text.replace(FILE_URL, PLACEHOLDER)
}

function stripFileKey<T>(value: T): T {
  if (typeof value === 'string') return stripFileKeyText(value) as T
  if (Array.isArray(value)) return value.map((item) => stripFileKey(item)) as T
  if (value && typeof value === 'object') {
    const out: Record<string, unknown> = {}
    for (const [key, item] of Object.entries(value)) {
      if (key === 'fileKey') continue
      out[key] = stripFileKey(item)
    }
    return out as T
  }
  return value
}

export { stripFileKey, stripFileKeyText }
