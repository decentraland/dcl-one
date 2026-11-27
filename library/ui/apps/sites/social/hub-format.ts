/** An announcement as the hub shows it. */
export type HubPostView = {
  id: string
  authorAddress: string
  authorName?: string | null
  authorProfilePictureUrl?: string | null
  authorHasClaimedName?: boolean
  content: string
  createdAt: string | null
  likesCount: number
  isLikedByUser?: boolean
}

/** "Mar 4" within the month, then "2 months ago", "1 year ago". */
export function relativeDate(iso?: string | null, now: number = Date.now()): string {
  if (!iso) return ''
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return ''
  const today = new Date(now)
  const months =
    (today.getFullYear() - date.getFullYear()) * 12 +
    today.getMonth() -
    date.getMonth() -
    (today.getDate() < date.getDate() ? 1 : 0)
  if (months >= 12) {
    const years = Math.floor(months / 12)
    return years === 1 ? '1 year ago' : `${years} years ago`
  }
  if (months >= 1) return months === 1 ? '1 month ago' : `${months} months ago`
  return date.toLocaleDateString([], { month: 'short', day: 'numeric' })
}

/** The full date and time, for a hover. */
export function fullDate(iso?: string | null): string | undefined {
  if (!iso) return undefined
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return undefined
  return date.toLocaleString([], { dateStyle: 'full', timeStyle: 'short' })
}

/** Oldest first, as a conversation reads; posts with the same time keep the newer one last. */
export function oldestFirst<T extends Pick<HubPostView, 'createdAt'>>(posts: readonly T[]): T[] {
  return posts
    .map((post, index) => ({ post, index, at: Date.parse(post.createdAt ?? '') || 0 }))
    .sort((a, b) => a.at - b.at || b.index - a.index)
    .map((row) => row.post)
}

export const shortAddress = (address: string): string =>
  address.length > 12 ? `${address.slice(0, 6)}…${address.slice(-4)}` : address

export const capitalized = (text: string): string => text.charAt(0).toUpperCase() + text.slice(1)

export const membersCount = (count: number): string =>
  `${count.toLocaleString('en-US')} ${count === 1 ? 'member' : 'members'}`

/**
 * An address an <img> can load: absolute http(s), or a path of this site (not a protocol-relative
 * `//host`). A bare content hash is not one.
 */
export const isImageUrl = (value: string | null | undefined): value is string =>
  !!value && (/^https?:\/\//i.test(value) || (value.startsWith('/') && !value.startsWith('//')))
