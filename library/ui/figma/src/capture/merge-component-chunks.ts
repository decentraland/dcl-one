import { readFile, writeFile } from 'node:fs/promises'
import { rawComponentPageSchema, type RawComponentPage } from '../ir/raw-components.js'
import { stripFileKey } from './strip-file-key.js'

type Chunk = { page: { id: string; name: string }; labels?: unknown[]; components: unknown[] }

function mergeChunks(chunks: Chunk[]): RawComponentPage {
  const [first] = chunks
  if (!first) throw new Error('Nothing to merge')
  const seen = new Set<string>()
  const components: unknown[] = []
  for (const chunk of chunks) {
    if (chunk.page.id !== first.page.id) {
      throw new Error(`Chunk for page ${chunk.page.name} cannot be merged into ${first.page.name}`)
    }
    for (const component of chunk.components) {
      const id = (component as { id: string }).id
      if (seen.has(id)) throw new Error(`Component ${id} appears in more than one chunk`)
      seen.add(id)
      components.push(component)
    }
  }
  const labels = chunks.find((chunk) => chunk.labels)?.labels
  return rawComponentPageSchema.parse({ page: first.page, labels, components })
}

async function mergeChunkFiles(output: string, inputs: string[]): Promise<RawComponentPage> {
  const chunks = await Promise.all(
    inputs.map(async (input) => JSON.parse(await readFile(input, 'utf8')) as Chunk)
  )
  const merged = mergeChunks(chunks)
  await writeFile(output, `${JSON.stringify(stripFileKey(merged))}\n`)
  return merged
}

export { mergeChunkFiles, mergeChunks }
