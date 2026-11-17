// What a money dialog drawn by the shell may show of a profile or an item read from a service: a name, a `#rgb`-style
// colour and an `https:` picture only, so nothing a profile or an index holds can make the page that holds the wallet
// load anything else. Donate and Gift both draw through it.

import type { DonateCreator } from '@hud/features/donate/types'

/** An `https:` URL with nothing a CSS `url()` or an attribute could be broken out of with. */
export const isSafeImageUrl = (v: unknown): v is string => typeof v === 'string' && /^https:\/\/[^\s"'()\\]+$/.test(v)

/** The person as the dialog may draw them; null without a valid address. */
export function safeCreator(c: Partial<DonateCreator> | null | undefined): DonateCreator | null {
  if (!c || typeof c.address !== 'string' || !/^0x[0-9a-f]{40}$/.test(c.address)) return null
  return {
    address: c.address,
    ...(typeof c.name === 'string' ? { name: c.name } : {}),
    ...(c.claimed === true ? { claimed: true } : {}),
    ...(typeof c.nameColor === 'string' && /^#[0-9a-fA-F]{3,8}$/.test(c.nameColor) ? { nameColor: c.nameColor } : {}),
    ...(isSafeImageUrl(c.face) ? { face: c.face } : {})
  }
}
