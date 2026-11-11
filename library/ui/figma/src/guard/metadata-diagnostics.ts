import type { Violation } from './check-scope.js'

type MetadataChange = { field: string; before?: unknown; after?: unknown }

// Arrays are catalog fields, so retain complete values rather than positional edits. Missing
// properties stay absent in JSON; explicit null and other falsy values remain distinguishable.
function metadataDiff(before: unknown, after: unknown, field = ''): MetadataChange[] {
  if (JSON.stringify(before) === JSON.stringify(after)) return []
  const record = (value: unknown): value is Record<string, unknown> =>
    value !== null && typeof value === 'object' && !Array.isArray(value)
  if (record(before) && record(after)) {
    const changes = [...new Set([...Object.keys(before), ...Object.keys(after)])]
      .sort()
      .flatMap((key) =>
        metadataDiff(
          Object.hasOwn(before, key) ? before[key] : undefined,
          Object.hasOwn(after, key) ? after[key] : undefined,
          `${field}/${key.replace(/~/g, '~0').replace(/\//g, '~1')}`
        )
      )
    if (changes.length) return changes
  }
  return [
    {
      field,
      ...(before === undefined ? {} : { before }),
      ...(after === undefined ? {} : { after })
    }
  ]
}

// Values belong in the report artifact and job summary, not in thrown errors printed to job logs.
const scopeErrorSummary = (violations: readonly Violation[]) =>
  violations.map(({ metadataChanges, ...violation }) => ({
    ...violation,
    ...(metadataChanges ? { metadataFields: metadataChanges.map(({ field }) => field) } : {})
  }))

export { metadataDiff, scopeErrorSummary }
export type { MetadataChange }
