import { redactFileKey } from '../config.js'

// The Figma file key is private. Captured JSON and every report written from it drop `fileKey`
// properties, replace Figma file URLs with the placeholder and redact the configured key wherever
// it appears. Node ids stay: they are not the key.
const FILE_URL =
  /https?:\/\/(?:www\.)?figma\.com\/(?:design|file|proto|board)\/[0-9A-Za-z]+[^\s"'<>)]*/g
const PLACEHOLDER = '<DCL_FIGMA_FILE>'

function stripFileKeyText(text: string, env: NodeJS.ProcessEnv = process.env): string {
  return redactFileKey(text.replace(FILE_URL, PLACEHOLDER), env)
}

function stripFileKey<T>(value: T, env: NodeJS.ProcessEnv = process.env): T {
  if (typeof value === 'string') return stripFileKeyText(value, env) as T
  if (Array.isArray(value)) return value.map((item) => stripFileKey(item, env)) as T
  if (value && typeof value === 'object') {
    const out: Record<string, unknown> = {}
    for (const [key, item] of Object.entries(value)) {
      if (key === 'fileKey') continue
      out[key] = stripFileKey(item, env)
    }
    return out as T
  }
  return value
}

export { stripFileKey, stripFileKeyText }
