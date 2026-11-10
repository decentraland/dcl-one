import type { MouseEvent } from 'react'

/** A chrome link the visitor followed, as source's navbar reports it. */
type ChromeLinkClick = {
  /** The navbar area: a menu id, `home`, `learn`, `credits` or `user_menu`. */
  section: string
  /** Which link: the last segment of its label's i18n key, e.g. `creator_documentation`. */
  action: string
  href: string
}

/**
 * Handlers that report a followed link once: a primary click, or a middle click, which opens a new
 * tab without firing `click`. Older browsers fired `click` for the middle button too, hence the
 * button checks.
 */
function followHandlers(report: (() => void) | undefined) {
  if (!report) return {}
  return {
    onClick: (event: MouseEvent) => {
      if (event.button === 0) report()
    },
    onAuxClick: (event: MouseEvent) => {
      if (event.button === 1) report()
    }
  }
}

export { followHandlers }
export type { ChromeLinkClick }
