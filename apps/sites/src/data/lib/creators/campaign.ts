// The live marketing campaign, read from Decentraland's Contentful proxy exactly as the legacy
// builder does (decentraland-dapps' campaign module): one "admin" entry points at the campaign entry
// of the moment, which carries the name and the tag creators are asked to use.
import { config } from '~/config'

const CAMPAIGN_CONTENT_TYPE = 'marketingCampaign'
const LOCALE = 'en-US'
const TIMEOUT_MS = 5_000

export type Campaign = {
  name: string
  /** The tag an item must carry to take part in the campaign. */
  mainTag: string
}

type EntryLink = { sys: { type: 'Link'; linkType: 'Entry'; id: string } }
type Entry = {
  sys: { id: string; contentType?: { sys: { id: string } } }
  fields?: Record<string, unknown>
}

function isEntryLink(value: unknown): value is EntryLink {
  const sys = (value as EntryLink | undefined)?.sys
  return sys?.type === 'Link' && sys.linkType === 'Entry' && typeof sys.id === 'string'
}

async function fetchEntry(id: string, signal: AbortSignal): Promise<Entry | null> {
  const base = `${config.get('CMS_API_URL')}/spaces/${config.get('CONTENTFUL_SPACE_ID')}/environments/${config.get('CONTENTFUL_ENVIRONMENT')}`
  const response = await fetch(`${base}/entries/${id}?locale=${LOCALE}`, { signal })
  if (!response.ok) {
    await response.body?.cancel()
    return null
  }
  return (await response.json()) as Entry
}

function toCampaign(fields: Record<string, unknown> | undefined): Campaign | null {
  const name = fields?.name
  const mainTag = fields?.mainTag
  if (typeof name !== 'string' || typeof mainTag !== 'string' || !name || !mainTag) return null
  return { name, mainTag }
}

/**
 * The campaign the admin entry currently links to, or null when there is none (or the CMS is down:
 * a missing campaign is a hidden hint, never an error the creator sees).
 */
export async function fetchCampaign(): Promise<Campaign | null> {
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), TIMEOUT_MS)
  try {
    const admin = await fetchEntry(config.get('CONTENTFUL_CAMPAIGN_ENTITY_ID'), controller.signal)
    if (!admin?.fields) return null
    const linked = await Promise.all(
      Object.values(admin.fields)
        .filter(isEntryLink)
        .map(link => fetchEntry(link.sys.id, controller.signal))
    )
    const campaign = linked.find(entry => entry?.sys.contentType?.sys.id === CAMPAIGN_CONTENT_TYPE)
    return toCampaign(campaign?.fields)
  } catch {
    return null
  } finally {
    clearTimeout(timer)
  }
}
