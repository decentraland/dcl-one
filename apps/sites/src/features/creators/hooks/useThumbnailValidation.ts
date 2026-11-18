import { queryOptions, useQuery } from '@tanstack/react-query'
import { useTranslation, type Translate } from '~/intl'
import { captureError } from '~/lib/monitoring'
import {
  ValidationSeverity,
  cachedRun,
  getValidator,
  type ThumbnailSource,
  type ValidationResult
} from '~/lib/validation'

// Blobs serialize to {} in a query key, so each one gets its own id.
const blobIds = new WeakMap<Blob, number>()
let nextBlobId = 0
function blobId(blob: Blob): number {
  let id = blobIds.get(blob)
  if (id === undefined) {
    id = nextBlobId++
    blobIds.set(blob, id)
  }
  return id
}

export function thumbnailValidationKey(source: ThumbnailSource | null) {
  if (!source) return ['thumbnail-validation', null] as const
  if (source.kind === 'item')
    return ['thumbnail-validation', 'item', source.item.contents[source.item.thumbnail] ?? null] as const
  return ['thumbnail-validation', 'blob', blobId(source.blob)] as const
}

/** Query options for a thumbnail check, shared across screens; saved thumbnails' results persist across reloads. */
export function thumbnailValidationQuery(source: ThumbnailSource | null, t: Translate) {
  const queryKey = thumbnailValidationKey(source)
  return queryOptions<ValidationResult>({
    queryKey,
    // A check that could not run must never read as a pass.
    queryFn: ({ signal }) =>
      cachedRun(source?.kind === 'item' ? queryKey : null, () =>
        getValidator().validateThumbnail(source!, { signal })
      ).catch((error: unknown) => {
        if (signal.aborted) throw error
        captureError(error, { flow: 'thumbnail_validation' })
        return {
          issues: [
            { code: 'thumbnail', severity: ValidationSeverity.WARNING, message: t('thumbnail_modal.check_failed') }
          ]
        }
      }),
    enabled: source !== null,
    staleTime: Infinity,
    retry: false
  })
}

/** Validates a thumbnail through `lib/validation`, re-running when its bytes change. `null` disables the query. */
export function useThumbnailValidation(source: ThumbnailSource | null) {
  const { t } = useTranslation()
  return useQuery(thumbnailValidationQuery(source, t))
}
