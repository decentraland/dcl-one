import type { Exclusion, GalleryEntry } from '../registry'
import { actionEntries } from './actions'
import { cardEntries } from './cards'
import { feedbackEntries } from './feedback'
import { formEntries } from './forms'
import { navigationEntries } from './navigation'

export const entries: readonly GalleryEntry[] = [
  ...actionEntries,
  ...formEntries,
  ...navigationEntries,
  ...cardEntries,
  ...feedbackEntries
]

/**
 * Public component exports the gallery does not render, each with its reason. Internal components are not public
 * exports and are shown through a public parent instead (CardImage through EventCard and CommunityCard).
 */
export const exclusions: readonly Exclusion[] = [
  {
    name: 'ChromeBody',
    reason: 'App chrome frame for a host page; it draws nothing of its own without the host layout.'
  },
  {
    name: 'ChromeLink',
    reason: 'App chrome router link; it renders a plain anchor unless a host provides navigation.'
  },
  {
    name: 'SiteLayout',
    reason: 'App chrome page layout; its content and navigation come from the host page.'
  }
]
