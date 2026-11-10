import type { MouseEvent } from 'react'

/** Follows an in-app href without a document load. */
export type BlogNavigate = (href: string) => void

/**
 * Click props for an in-app link: a plain primary click goes through `onNavigate`; a modified or
 * non-primary click (new tab, new window, download) keeps the browser's own behaviour.
 */
export function inAppLink(href: string, onNavigate: BlogNavigate | undefined) {
  if (!onNavigate) return {}
  return {
    onClick: (event: MouseEvent<HTMLAnchorElement>) => {
      if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey)
        return
      event.preventDefault()
      onNavigate(href)
    }
  }
}

/** As source (shared/blog/utils/date.ts): en-US, UTC, e.g. "Aug 25, 2020"; a date-only value is read as UTC midnight; an unparseable one is shown as given. */
export function formatBlogDate(value?: string): string {
  if (!value) return ''
  const date = new Date(value.includes('T') ? value : `${value}T00:00:00Z`)
  if (Number.isNaN(date.getTime())) return value
  return DATE.format(date)
}

const DATE = new Intl.DateTimeFormat('en-US', {
  timeZone: 'UTC',
  month: 'short',
  day: '2-digit',
  year: 'numeric'
})

/** As source (shared/blog/utils/locations.ts), with the post's absolute public URL passed in. */
export const shareLinks = (title: string, absoluteUrl: string) => ({
  x: `https://x.com/intent/post?text=${encodeURIComponent(title)}&url=${encodeURIComponent(absoluteUrl)}`,
  facebook: `https://www.facebook.com/sharer/sharer.php?u=${encodeURIComponent(absoluteUrl)}`
})
