import { queryOptions, useQuery } from '@tanstack/react-query'
import { useTranslation, type Translate } from '~/intl'
import { captureError } from '~/lib/monitoring'
import {
  ValidationSeverity,
  cachedRun,
  getValidator,
  itemModels,
  type ValidationContext,
  type ValidationResult,
  type ValidationSource
} from '~/lib/validation'

/**
 * Cache identity of a source: the hashes of a saved item's distinct models and their audio, with the body
 * shapes each one is worn by, or the caller's id for in-memory files.
 */
function sourceKey(source: ValidationSource, blobId: string | undefined): unknown {
  if (source.kind === 'blob') return ['blob', blobId ?? source.mainFile]
  const { contents } = source.item
  return [
    'item',
    itemModels(source.item).map(model => [
      contents[model.mainFile] ?? null,
      model.audio.map(path => contents[path] ?? null).sort(),
      [...model.bodyShapes].sort()
    ])
  ]
}

export function modelValidationKey(source: ValidationSource | null, ctx: ValidationContext, blobId?: string) {
  return [
    'model-validation',
    source ? sourceKey(source, blobId) : null,
    ctx.type,
    ctx.category ?? null,
    [...(ctx.hides ?? [])].sort(),
    ctx.loop ?? null
  ] as const
}

/**
 * Query options for a model check, shared by every screen so they share one cache. Saved items' results
 * persist across reloads; a check that could not run is never persisted and never reads as a pass.
 */
export function modelValidationQuery(
  source: ValidationSource | null,
  ctx: ValidationContext,
  t: Translate,
  blobId?: string
) {
  const queryKey = modelValidationKey(source, ctx, blobId)
  return queryOptions<ValidationResult>({
    queryKey,
    queryFn: ({ signal }) =>
      cachedRun(source?.kind === 'item' ? queryKey : null, () =>
        getValidator().validate(source!, ctx, { signal })
      ).catch((error: unknown) => {
        if (signal.aborted) throw error
        captureError(error, { flow: 'model_validation' })
        return {
          issues: [
            { code: 'model', severity: ValidationSeverity.WARNING, message: t('item_editor.validation.check_failed') }
          ]
        }
      }),
    enabled: source !== null,
    staleTime: Infinity,
    retry: false
  })
}

/**
 * Validates every model of an item through `lib/validation`, re-running when its files, category, hides or
 * loop change. A run whose inputs change mid-flight is aborted. `null` source disables the query.
 */
export function useModelValidation(
  source: ValidationSource | null,
  ctx: ValidationContext,
  /** Blob sources have no stable identity of their own: the caller supplies one that changes with the files. */
  blobId?: string
) {
  const { t } = useTranslation()
  return useQuery(modelValidationQuery(source, ctx, t, blobId))
}
