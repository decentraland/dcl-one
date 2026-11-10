import { z } from 'zod'
import { diagnostic, type Diagnostic } from './diagnostics.js'
import {
  resolveComponent,
  type ResolveContext,
  type ResolveInput,
  type ResolveResult
} from './resolve.js'

const record = z.record(z.string(), z.string())
const entrySchema = z
  .object({
    nodeId: z.string().min(1).optional(),
    name: z.string().min(1).optional(),
    variantProperties: record.optional(),
    texts: record.optional()
  })
  .strict()

type BatchEntry = { index: number; input: unknown; result: ResolveResult }
type BatchResult =
  | { ok: false; errors: Diagnostic[] }
  | {
      ok: boolean
      results: BatchEntry[]
      summary: {
        requested: number
        resolved: number
        // Known library components without a public mapping.
        unmapped: { index: number; name?: string; nodeId?: string }[]
        // Every other failure, with its first error code.
        failed: { index: number; code: string }[]
      }
    }

const invalid = (message: string, details?: Record<string, unknown>): ResolveResult => ({
  ok: false,
  errors: [diagnostic('invalid-input', message, details)],
  warnings: []
})

// Resolves a list of instances in input order. The list comes from the caller's reading of the design: the
// batch echoes every entry so an omission can be checked, but it cannot detect one.
async function resolveBatch(ctx: ResolveContext, entries: unknown): Promise<BatchResult> {
  if (!Array.isArray(entries) || entries.length === 0)
    return {
      ok: false,
      errors: [diagnostic('invalid-input', 'A batch is a non-empty JSON array of instances.')]
    }
  const seen = new Map<string, number>()
  const results: BatchEntry[] = []
  for (const [index, input] of entries.entries()) {
    const parsed = entrySchema.safeParse(input)
    if (!parsed.success) {
      results.push({
        index,
        input,
        result: invalid(
          `Entry ${index} must hold nodeId or name, and string variantProperties and texts only.`,
          { index }
        )
      })
      continue
    }
    const entry = parsed.data as ResolveInput
    const identity = JSON.stringify([
      entry.nodeId,
      entry.name,
      Object.entries(entry.variantProperties ?? {}).sort(),
      Object.entries(entry.texts ?? {}).sort()
    ])
    const first = seen.get(identity)
    if (first !== undefined) {
      results.push({
        index,
        input,
        result: invalid(`Entry ${index} repeats entry ${first}.`, { index, repeats: first })
      })
      continue
    }
    seen.set(identity, index)
    results.push({ index, input, result: await resolveComponent(ctx, entry) })
  }
  const failures = results.filter((entry) => !entry.result.ok)
  const code = (entry: BatchEntry) => (entry.result.ok ? '' : entry.result.errors[0]!.code)
  return {
    ok: failures.length === 0,
    results,
    summary: {
      requested: entries.length,
      resolved: results.length - failures.length,
      unmapped: failures
        .filter((entry) => code(entry) === 'mapping-missing')
        .map(({ index, input }) => {
          const { name, nodeId } = input as ResolveInput
          return { index, ...(name ? { name } : {}), ...(nodeId ? { nodeId } : {}) }
        }),
      failed: failures
        .filter((entry) => code(entry) !== 'mapping-missing')
        .map((entry) => ({ index: entry.index, code: code(entry) }))
    }
  }
}

export { resolveBatch }
export type { BatchResult }
