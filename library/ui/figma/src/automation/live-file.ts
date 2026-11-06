import { z } from 'zod'
import { LONG_READ_TIMEOUT_MS, type FigmaClient } from '../figma/client.js'

type LiveNode = {
  id: string
  name: string
  type: string
  children?: LiveNode[]
  [key: string]: unknown
}
const nodeSchema: z.ZodType<LiveNode> = z.lazy(() =>
  z
    .object({
      id: z.string(),
      name: z.string(),
      type: z.string(),
      children: z.array(nodeSchema).optional()
    })
    .passthrough()
)
const fileSchema = z.object({
  document: nodeSchema,
  version: z.string(),
  components: z
    .record(z.string(), z.object({ componentSetId: z.string().optional() }).passthrough())
    .default({}),
  styles: z
    .record(z.string(), z.object({ name: z.string(), styleType: z.string() }).passthrough())
    .default({})
})
type LiveFile = z.infer<typeof fileSchema>
const record = (value: unknown): Record<string, unknown> =>
  value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {}

function readReadiness(file: LiveFile) {
  return (file.document.children ?? []).map((page) => {
    const markers = (page.children ?? []).filter((node) => node.name === 'Sync status')
    const marker = markers[0]
    const property = record(record(marker?.componentProperties).ready)
    const component =
      typeof marker?.componentId === 'string' ? file.components[marker.componentId] : undefined
    const valid =
      markers.length === 1 &&
      marker?.type === 'INSTANCE' &&
      component?.componentSetId === '15973:15' &&
      property.type === 'VARIANT' &&
      ['true', 'false'].includes(String(property.value)) &&
      typeof property.value === 'string'
    return { id: page.id, name: page.name.trim(), ready: valid ? property.value === 'true' : null }
  })
}
async function readLiveFile(client: FigmaClient, fileKey: string) {
  return fileSchema.parse(
    await client.get(`/v1/files/${fileKey}?depth=2`, { timeoutMs: LONG_READ_TIMEOUT_MS })
  )
}
async function readLivePage(client: FigmaClient, fileKey: string, pageId: string, version: string) {
  const response = z
    .object({ nodes: z.record(z.string(), z.object({ document: nodeSchema }).nullable()) })
    .parse(
      await client.get(
        `/v1/files/${fileKey}/nodes?ids=${encodeURIComponent(pageId)}&version=${encodeURIComponent(version)}&geometry=paths`,
        { timeoutMs: LONG_READ_TIMEOUT_MS }
      )
    )
  const page = response.nodes[pageId]?.document
  if (!page || page.type !== 'CANVAS' || page.id !== pageId)
    throw new Error(`Missing Figma page ${pageId}`)
  return page
}
export { readLiveFile, readLivePage, readReadiness, record }
export type { LiveFile, LiveNode }
