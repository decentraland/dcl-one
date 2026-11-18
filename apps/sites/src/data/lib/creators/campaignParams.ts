// Partner-attribution params, ported from sites' `campaignParams` with the same semantics: read from the
// URL at call time, not persisted, so a partner link must land on the page whose clicks it attributes.
import { currentSearch } from '~/config'

const CAMPAIGN_PARAM_KEYS = ['utm_org', 'utm_source', 'utm_medium', 'utm_campaign', 'utm_content', 'utm_term'] as const

// A hostile or malformed link must not flood the warehouse with an unbounded string.
const MAX_VALUE_LENGTH = 256

/** UTM Builder convention: lowercase, underscores for spaces, only `[a-z0-9_-]`. */
export function normalizeCampaignParamValue(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/\s+/g, '_')
    .replace(/[^a-z0-9_-]/g, '')
    .replace(/_+/g, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, MAX_VALUE_LENGTH)
}

/** The `utm_*` params present on `search` (the current URL by default), normalized; empty ones are left out. */
export function collectCampaignParams(search: string = currentSearch()): Record<string, string> {
  const params = new URLSearchParams(search)
  const collected: Record<string, string> = {}
  for (const key of CAMPAIGN_PARAM_KEYS) {
    const value = params.get(key)
    const normalized = value ? normalizeCampaignParamValue(value) : ''
    if (normalized) collected[key] = normalized
  }
  return collected
}
