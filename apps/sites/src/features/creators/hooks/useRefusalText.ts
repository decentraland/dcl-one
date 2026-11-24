import { useTranslation } from '@/data/lib/creators/intl'
import type { BoundRefusal } from '@/data/lib/creators/builder'

const MINUTE_SECS = 60
const SAYS_WHEN = /\b(try again|retry)\b/i

/**
 * What the creator reads for the node's refusal at one of its bounds: its words, then when to try again if it
 * said and its words don't already.
 */
export function useRefusalText(): (refusal: BoundRefusal | null) => string | null {
  const { t } = useTranslation()
  return refusal => {
    if (!refusal) return null
    const { message: reason, retryAfterSecs } = refusal
    if (retryAfterSecs === undefined || SAYS_WHEN.test(reason)) return reason
    return retryAfterSecs < MINUTE_SECS
      ? t('builder_refusal.retry_in_seconds', { reason, count: retryAfterSecs })
      : t('builder_refusal.retry_in_minutes', { reason, count: Math.ceil(retryAfterSecs / MINUTE_SECS) })
  }
}
