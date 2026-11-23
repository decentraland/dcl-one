import { BASE_DOMAIN } from '@hud-host'
import type { DiscoverPlace } from '@explorer-core/features/places/placesApi'
import { withUtm } from '@explorer-core/lib/utm'

export const isWorldPlace = (place: Pick<DiscoverPlace, 'world' | 'world_name'>): boolean => place.world === true || (place.world_name != null && place.world_name !== '')

/** The contact name, else the owner's wallet shortened, else "Unknown". */
export function creatorFallback(place: Pick<DiscoverPlace, 'contact_name' | 'owner'>): string {
  if (place.contact_name != null && place.contact_name !== '') return place.contact_name
  const owner = place.owner ?? ''
  return owner.length >= 10 ? `${owner.slice(0, 5)}...${owner.slice(-5)}` : 'Unknown'
}

export function likeRateText(likeRate: number | string | null | undefined): string {
  const value = typeof likeRate === 'string' ? Number(likeRate) : (likeRate ?? 0)
  return `${Math.round((Number.isFinite(value) ? value : 0) * 100)}%`
}

const compact = new Intl.NumberFormat('en-US', { notation: 'compact', maximumFractionDigits: 1 })
export const compactNumber = (n: number | undefined): string => compact.format(n ?? 0)

/** "dd/MM/yyyy", or "-" without a date. */
export function updatedText(updatedAt: string | null | undefined): string {
  const at = updatedAt != null ? new Date(updatedAt) : null
  if (at == null || Number.isNaN(at.getTime())) return '-'
  const pad = (n: number): string => String(n).padStart(2, '0')
  return `${pad(at.getDate())}/${pad(at.getMonth() + 1)}/${at.getFullYear()}`
}

export function placeJumpLink(place: Pick<DiscoverPlace, 'world' | 'world_name' | 'base_position' | 'positions'>, referrer: string | null): string {
  const base = place.base_position ?? place.positions[0] ?? '0,0'
  const link = isWorldPlace(place) ? `https://${BASE_DOMAIN}/jump/?realm=${encodeURIComponent(place.world_name ?? '')}` : `https://${BASE_DOMAIN}/jump/?position=${base}`
  return referrer != null && referrer !== '' ? `${link}&referrer=${encodeURIComponent(referrer)}` : link
}

export function placeShareUrl(place: Pick<DiscoverPlace, 'title' | 'world' | 'world_name' | 'base_position' | 'positions'>, referrer: string | null): string {
  const text = `Check out ${place.title}, a cool place I found in Decentraland!`
  return `${withUtm(`https://twitter.com/intent/tweet?text=${encodeURIComponent(text)}&hashtags=DCLPlace&url=${encodeURIComponent(placeJumpLink(place, referrer))}`)}&utm_campaign=discover`
}
