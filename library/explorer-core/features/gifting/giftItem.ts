// The item a gift notification names. A `transfer_received` notification carries the token's `tokenUri` (an
// ERC-721 metadata document); its name, picture and `rarity` / `category` attributes are what the gift-received
// popup shows. An emote is told from a wearable by its category.

/** The categories an emote's metadata names; any other category is a wearable's. */
const EMOTE_CATEGORIES: ReadonlySet<string> = new Set(['dance', 'stunt', 'greetings', 'fun', 'poses', 'reactions', 'horror', 'miscellaneous'])

export type GiftableKind = 'wearable' | 'emote'

export interface GiftItem {
  name: string
  description: string
  /** The item's thumbnail, else its image; '' when the document names neither. */
  imageUrl: string
  /** Lowercase; 'common' when the document names none. */
  rarity: string
  /** Lowercase; 'wearable' when the document names none. */
  category: string
  kind: GiftableKind
}

const TIMEOUT_MS = 10_000

interface Attribute {
  trait_type?: unknown
  value?: unknown
}

const text = (v: unknown): string => (typeof v === 'string' ? v : '')

/** The item a token metadata document describes. */
export function parseGiftItem(doc: unknown): GiftItem | null {
  if (doc == null || typeof doc !== 'object' || Array.isArray(doc)) return null
  const d = doc as Record<string, unknown>
  let rarity = 'common'
  let category = 'wearable'
  if (Array.isArray(d.attributes)) {
    for (const a of d.attributes as Attribute[]) {
      const trait = text(a?.trait_type).toLowerCase()
      const value = text(a?.value).toLowerCase()
      if (trait === 'rarity') rarity = value
      else if (trait === 'category') category = value
    }
  }
  return {
    name: text(d.name),
    description: text(d.description),
    imageUrl: text(d.thumbnail) || text(d.image),
    rarity,
    category,
    kind: EMOTE_CATEGORIES.has(category) ? 'emote' : 'wearable'
  }
}

/** Reads the token's metadata from its `tokenUri`; null when it is missing, unreadable or not https. */
export async function loadGiftItem(tokenUri: string | undefined, fetchImpl: typeof fetch = fetch, signal?: AbortSignal): Promise<GiftItem | null> {
  if (!tokenUri) return null
  let url: URL
  try {
    url = new URL(tokenUri)
  } catch {
    return null
  }
  if (url.protocol !== 'https:') return null
  try {
    const res = await fetchImpl(url.toString(), { signal: signal ?? AbortSignal.timeout(TIMEOUT_MS) })
    if (!res.ok) {
      await res.body?.cancel().catch(() => undefined)
      return null
    }
    return parseGiftItem(await res.json())
  } catch {
    return null
  }
}
