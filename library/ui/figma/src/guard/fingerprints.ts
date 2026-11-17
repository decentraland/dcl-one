import { createHash } from 'node:crypto'
import { readBytesIfExists, readIfExists, resolveContained } from './fs-safe.js'

async function invalidFingerprints(root: string, file: string): Promise<string[]> {
  const text = await readIfExists(await resolveContained(root, file))
  if (text === undefined) return []
  const data = JSON.parse(text) as { files?: Record<string, string> }
  if (!data.files) return []
  const invalid: string[] = []
  for (const [input, hash] of Object.entries(data.files)) {
    const bytes = await readBytesIfExists(await resolveContained(root, input))
    if (!bytes || createHash('sha256').update(bytes).digest('hex') !== hash) invalid.push(input)
  }
  return invalid
}
export { invalidFingerprints }
