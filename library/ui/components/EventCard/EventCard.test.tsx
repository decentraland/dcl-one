import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { fireEvent, render, screen } from '@testing-library/react'
import type { ReactElement } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import {
  EventCard,
  EventCardAction,
  EventCardActionLink,
  EventCardHost,
  type EventCardLinkProps,
  type EventCardProps
} from './EventCard'

const event: EventCardProps = {
  title: 'Poker Night',
  href: '/events?id=poker',
  image: 'https://example.org/poker.png',
  time: { label: 'Tomorrow at noon', dateTime: '2026-11-13T12:00:00Z' }
}
const setup = (props: Partial<EventCardProps> = {}) => render(<EventCard {...event} {...props} />)

describe('EventCard', () => {
  afterEach(() => {
    vi.resetAllMocks()
  })

  describe('when rendered with an event', () => {
    beforeEach(() => {
      setup()
    })

    it('should name the card by a level-3 heading that links to the event', () => {
      const heading = screen.getByRole('heading', { level: 3, name: 'Poker Night' })
      expect(heading.querySelector('a')).toHaveAttribute('href', '/events?id=poker')
    })

    it('should show the start time with its machine-readable instant', () => {
      expect(screen.getByText('Tomorrow at noon').closest('time')).toHaveAttribute(
        'dateTime',
        '2026-11-13T12:00:00Z'
      )
    })

    it('should have no status region without an announcement', () => {
      expect(screen.queryByRole('status')).toBeNull()
    })
  })

  describe('when a heading level is given', () => {
    it('should use the heading level it is given', () => {
      setup({ headingLevel: 4 })
      expect(screen.getByRole('heading', { level: 4, name: 'Poker Night' })).toBeInTheDocument()
    })
  })

  describe('when the caller handles the open', () => {
    let onOpen: Mock

    beforeEach(() => {
      onOpen = vi.fn((click: { preventDefault: () => void }) => click.preventDefault())
      setup({ onOpen })
    })

    it('should report the open with the click, so the caller can open the event in place', () => {
      const link = screen.getByRole('link', { name: 'Poker Night' })
      expect(fireEvent.click(link)).toBe(false)
      expect(onOpen).toHaveBeenCalledOnce()
    })
  })

  describe('when a router link component is given', () => {
    let RouterLink: Mock<(props: EventCardLinkProps) => ReactElement>

    beforeEach(() => {
      RouterLink = vi.fn((props: EventCardLinkProps) => <a data-router="" {...props} />)
      setup({ linkComponent: RouterLink })
    })

    it('should render the title through the router link', () => {
      expect(screen.getByRole('link', { name: 'Poker Night' })).toHaveAttribute('data-router')
      expect(RouterLink).toHaveBeenCalled()
    })
  })

  describe('when it has a host', () => {
    describe('and the host has a profile in a day card', () => {
      beforeEach(() => {
        setup({ variant: 'day', creator: { name: 'Alice', profileHref: '/profile/alice' } })
      })

      it('should link the host to their profile', () => {
        expect(screen.getByRole('link', { name: 'Alice' })).toHaveAttribute(
          'href',
          '/profile/alice'
        )
      })

      it('should credit the host beside the card link', () => {
        expect(screen.getByText(/^by/)).toHaveTextContent('by Alice')
      })
    })

    describe('and the host has a face in a day card', () => {
      it('should show the host face as a decorative image', () => {
        const { container } = setup({
          variant: 'day',
          creator: { name: 'Alice', avatar: 'https://example.org/a.png' }
        })
        expect(container.querySelector('img[src="https://example.org/a.png"]')).toHaveAttribute(
          'alt',
          ''
        )
      })
    })

    describe('and the host has neither a face nor a profile in a day card', () => {
      beforeEach(() => {
        setup({ variant: 'day', creator: { name: 'alice' } })
      })

      it('should show the initial hidden from assistive technology', () => {
        expect(screen.getByText('A')).toHaveAttribute('aria-hidden', 'true')
      })

      it('should not link the host', () => {
        expect(screen.queryByRole('link', { name: 'alice' })).toBeNull()
      })
    })

    describe('and a by-label is given', () => {
      it('should use the word it is given before the name', () => {
        setup({ creator: { name: 'Alice' }, byLabel: 'By' })
        expect(screen.getByText(/^By/)).toHaveTextContent('By Alice')
      })
    })
  })

  describe('when it has actions', () => {
    beforeEach(() => {
      setup({ actions: <EventCardAction aria-label="Remind me: Poker Night" icon={<svg />} /> })
    })

    it('should render them as focusable controls after the title link', () => {
      const action = screen.getByRole('button', { name: 'Remind me: Poker Night' })
      action.focus()
      expect(action).toHaveFocus()
      const link = screen.getByRole('link', { name: 'Poker Night' })
      expect(link.compareDocumentPosition(action) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    })

    it('should mark the card as having actions', () => {
      expect(screen.getByRole('article')).toHaveClass('hasActions')
    })
  })

  describe('when it has no actions', () => {
    beforeEach(() => {
      setup({ actions: null })
    })

    it('should not mark the card as having actions', () => {
      expect(screen.getByRole('article')).not.toHaveClass('hasActions')
    })

    it('should render no button', () => {
      expect(screen.queryByRole('button')).toBeNull()
    })
  })

  describe('when an action outcome is announced', () => {
    beforeEach(() => {
      setup({
        actions: <EventCardAction aria-label="Copy link" icon={<svg />} />,
        announcement: 'Link copied.'
      })
    })

    it('should announce it in a visually hidden status region', () => {
      const status = screen.getByRole('status')
      expect(status).toHaveTextContent('Link copied.')
      expect(status).toHaveClass('visuallyHidden')
    })
  })

  describe('when the image is missing or unsafe', () => {
    describe('and the image is missing', () => {
      it('should keep the small cover box without an image', () => {
        const { container } = setup({ image: null })
        expect(container.querySelector('img')).toBeNull()
        expect(container.querySelector('.media')).not.toBeNull()
      })
    })

    describe('and the image is not https', () => {
      it('should drop the image', () => {
        const { container } = setup({ image: 'http://example.org/poker.png' })
        expect(container.querySelector('img')).toBeNull()
      })
    })

    describe('and a day card has no image and is not live', () => {
      it('should leave the day card without a cover', () => {
        const { container } = setup({ variant: 'day', image: null })
        expect(container.querySelector('.media')).toBeNull()
      })
    })
  })

  describe('when the event is live', () => {
    describe('and the count of people is known', () => {
      beforeEach(() => {
        setup({
          variant: 'day',
          live: true,
          players: 12,
          playersLabel: '12 online',
          time: undefined
        })
      })

      it('should show LIVE', () => {
        expect(screen.getByText('LIVE')).toBeInTheDocument()
      })

      it('should name the count for assistive technology and hide the bare number', () => {
        expect(screen.getByText('12 online')).toBeInTheDocument()
        expect(screen.getByText('12')).toHaveAttribute('aria-hidden', 'true')
      })

      it('should mark the card as live', () => {
        expect(screen.getByRole('article')).toHaveClass('isLive')
      })
    })

    describe('and the known count is zero', () => {
      it('should show a known zero as 0', () => {
        setup({ variant: 'day', live: true, players: 0, playersLabel: '0 online' })
        expect(screen.getByText('0 online')).toBeInTheDocument()
      })
    })

    describe('and the count is unknown', () => {
      it('should show LIVE without a count', () => {
        const { container } = setup({ variant: 'day', live: true })
        expect(screen.getByText('LIVE')).toBeInTheDocument()
        expect(container.querySelector('.players')).toBeNull()
      })
    })

    describe('and it has no image', () => {
      it('should keep the cover for its badges', () => {
        const { container } = setup({ variant: 'day', live: true, image: null })
        expect(container.querySelector('.media')).not.toBeNull()
        expect(container.querySelector('img')).toBeNull()
      })
    })
  })
})

describe('EventCard small', () => {
  describe('when rendered with the default variant', () => {
    it('should be the small card, with the text before the cover', () => {
      const { container } = setup()
      expect(screen.getByRole('article')).toHaveClass('small')
      expect(container.querySelector('.body')?.nextElementSibling).toHaveClass('media')
      expect(screen.getByRole('heading', { level: 3 })).toHaveClass('smallTitle')
    })
  })

  describe('when the host has a face and a profile', () => {
    it('should credit the host as muted plain text with no face', () => {
      const { container } = setup({
        creator: {
          name: 'Alice',
          avatar: 'https://example.org/a.png',
          profileHref: '/profile/alice'
        }
      })
      expect(container.querySelector('.smallBy')).toHaveTextContent('by Alice')
      expect(screen.queryByRole('link', { name: 'Alice' })).toBeNull()
      expect(container.querySelector('img[src="https://example.org/a.png"]')).toBeNull()
    })
  })

  describe('when it has no host', () => {
    it('should leave the host row out', () => {
      const { container } = setup({ creator: undefined })
      expect(container.querySelector('.smallBy')).toBeNull()
    })
  })

  describe('when an upcoming event has actions', () => {
    it('should show the time in a chip that a hover never folds away', () => {
      const { container } = setup({
        actions: <EventCardAction aria-label="Remind me" icon={<svg />} />
      })
      const chip = screen.getByText('Tomorrow at noon').closest('p')
      expect(chip).toHaveClass('smallTime')
      expect(chip?.closest('.reveal')).not.toHaveClass('hideOnHover')
      expect(container.querySelector('.smallBy')).toBeNull()
    })

    it('should open a row of actions under the time on hover', () => {
      setup({ actions: <EventCardAction aria-label="Remind me" icon={<svg />} /> })
      const row = screen.getByRole('button', { name: 'Remind me' }).closest('.reveal')
      expect(row).toHaveClass('showOnHover')
      expect(screen.getByRole('article')).toHaveClass('hasActions')
    })
  })

  describe('when it has no actions', () => {
    it('should have no action row', () => {
      const { container } = setup({ actions: undefined })
      expect(container.querySelector('.showOnHover')).toBeNull()
      expect(screen.getByRole('article')).not.toHaveClass('hasActions')
    })
  })

  describe('when the event is live', () => {
    describe('and the count of people is known', () => {
      it('should mark the card live, fold its time away on hover and name LIVE behind its glyph', () => {
        const { container } = setup({ live: true, players: 16, playersLabel: '16 online' })
        expect(screen.getByRole('article')).toHaveClass('isLive')
        expect(screen.getByText('LIVE')).toHaveClass('liveLabel')
        expect(screen.getByText('16 online')).toBeInTheDocument()
        expect(screen.getByText('Tomorrow at noon').closest('.reveal')).toHaveClass('hideOnHover')
        expect(container.querySelector('.live svg')).toHaveClass('liveIcon')
      })
    })

    describe('and the count is unknown', () => {
      it('should show LIVE without a count', () => {
        const { container } = setup({ live: true })
        expect(screen.getByText('LIVE')).toBeInTheDocument()
        expect(container.querySelector('.players')).toBeNull()
      })
    })

    describe('and it has no image', () => {
      it('should keep the live badges over an empty cover', () => {
        const { container } = setup({ live: true, image: null })
        expect(container.querySelector('img')).toBeNull()
        expect(container.querySelector('.media .badges')).not.toBeNull()
      })
    })
  })

  describe('when it has no time', () => {
    it('should render the card without a time element', () => {
      const { container } = setup({ time: undefined })
      expect(container.querySelector('time')).toBeNull()
      expect(screen.getByRole('heading', { name: 'Poker Night' })).toBeInTheDocument()
    })
  })
})

describe('EventCardAction', () => {
  let onClick: Mock

  beforeEach(() => {
    onClick = vi.fn()
  })

  afterEach(() => {
    vi.resetAllMocks()
  })

  describe('when a toggle action is pressed', () => {
    beforeEach(() => {
      render(
        <EventCardAction aria-label="Remind me" aria-pressed icon={<svg />} onClick={onClick} />
      )
    })

    it('should be a named button that keeps its toggle state', () => {
      const button = screen.getByRole('button', { name: 'Remind me' })
      expect(button).toHaveAttribute('type', 'button')
      expect(button).toHaveAttribute('aria-pressed', 'true')
    })

    it('should report the press once', () => {
      fireEvent.click(screen.getByRole('button', { name: 'Remind me' }))
      expect(onClick).toHaveBeenCalledOnce()
    })
  })

  describe('when it is disabled', () => {
    beforeEach(() => {
      render(<EventCardAction aria-label="Remind me" disabled icon={<svg />} onClick={onClick} />)
    })

    it('should report no press', () => {
      fireEvent.click(screen.getByRole('button', { name: 'Remind me' }))
      expect(onClick).not.toHaveBeenCalled()
    })
  })
})

describe('EventCardAction with a label', () => {
  describe('when a label is given', () => {
    beforeEach(() => {
      render(
        <EventCardAction aria-label="Remind me: Poker Night" label="Remind me" icon={<svg />} />
      )
    })

    it('should show the label after the glyph and keep the accessible name', () => {
      expect(screen.getByRole('button', { name: 'Remind me: Poker Night' })).toHaveTextContent(
        'Remind me'
      )
    })

    it('should mark the action as labelled', () => {
      expect(screen.getByRole('button', { name: 'Remind me: Poker Night' })).toHaveClass(
        'labelled'
      )
    })
  })

  describe('when no label is given', () => {
    it('should be icon-only', () => {
      render(<EventCardAction aria-label="Copy link" icon={<svg />} />)
      expect(screen.getByRole('button', { name: 'Copy link' })).not.toHaveClass('labelled')
    })
  })
})

describe('EventCardHost', () => {
  describe('when rendered outside a card', () => {
    beforeEach(() => {
      render(
        <EventCardHost creator={{ name: 'Alice', profileHref: '/profile/alice' }} byLabel="By" />
      )
    })

    it('should render the host byline', () => {
      expect(screen.getByText(/^By/)).toHaveTextContent('By Alice')
    })

    it('should link the host to their profile', () => {
      expect(screen.getByRole('link', { name: 'Alice' })).toHaveAttribute('href', '/profile/alice')
    })
  })
})

describe('EventCardActionLink', () => {
  describe('when rendered with a target and a relation', () => {
    beforeEach(() => {
      render(
        <EventCardActionLink
          aria-label="Add to calendar"
          href="https://calendar.example/add"
          target="_blank"
          rel="noopener noreferrer"
          icon={<svg />}
        />
      )
    })

    it('should be a named link that keeps its destination', () => {
      expect(screen.getByRole('link', { name: 'Add to calendar' })).toHaveAttribute(
        'href',
        'https://calendar.example/add'
      )
    })

    it('should keep its relation', () => {
      expect(screen.getByRole('link', { name: 'Add to calendar' })).toHaveAttribute(
        'rel',
        'noopener noreferrer'
      )
    })
  })
})

/** The module's text, and the declarations of the rule whose selector list is exactly `selector`. */
// Vitest runs from library/ui (its config's root), where jsdom gives no file URL for this module.
const css = readFileSync(join(process.cwd(), 'components/EventCard/EventCard.module.css'), 'utf8')
const blockOf = (source: string, selector: string): string => {
  // The rule starts a line, so `.actions` does not find `.small .actions`.
  const match = new RegExp(`\\n *${selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')} \\{`).exec(
    source
  )
  if (!match) throw new Error(`no rule ${selector}`)
  const start = match.index + 1
  return source.slice(start, source.indexOf('}', start))
}
const mediaOf = (query: string): string => {
  const start = css.indexOf(`@media ${query} {`)
  if (start < 0) throw new Error(`no @media ${query}`)
  // The block ends at the first line that closes it at its own indentation.
  return css.slice(start, css.indexOf('\n  }\n', start))
}

describe('EventCard dimmed', () => {
  describe.each(['small', 'day'] as const)('when a %s card is dimmed', variant => {
    beforeEach(() => {
      setup({ variant, dimmed: true })
    })

    it('should fade its cover alone, keeping the title, host and time at full contrast', () => {
      const media = screen.getByRole('article').querySelector('[data-dimmed]')
      expect(media?.querySelector('img')).toBeTruthy()
      expect(media?.contains(screen.getByRole('link', { name: 'Poker Night' }))).toBe(false)
      expect(blockOf(css, '.media[data-dimmed]')).toContain('opacity: 0.5')
      expect(css).not.toMatch(/\.card[^{]*\{[^}]*opacity: 0\.5/)
    })
  })

  describe('when a card is not dimmed', () => {
    beforeEach(() => {
      setup({ variant: 'day' })
    })

    it('should not fade its cover', () => {
      expect(screen.getByRole('article').querySelector('[data-dimmed]')).toBeNull()
    })
  })
})

describe('EventCard motion', () => {
  const hover = mediaOf('(hover: hover) and (min-width: 768px)')

  describe('when a device hovers from tablet width', () => {
    it('should lift and light the small card over 200ms on hover and keyboard focus', () => {
      expect(blockOf(css, '.card')).toMatch(/transition:\s*transform 200ms,\s*box-shadow 200ms/)
      const lifted = blockOf(hover, '.small:hover,\n    .small:focus-within')
      expect(lifted).toContain('transform: translateY(-4px)')
      expect(lifted).toContain('box-shadow: 0 2px 12px 12px')
    })

    it("should fade a day card's time out and its actions up 8px into its place", () => {
      expect(blockOf(hover, '.day.hasActions .time')).toContain('transition: opacity 200ms')
      expect(
        blockOf(hover, '.day.hasActions:hover .time,\n    .day.hasActions:focus-within .time')
      ).toContain('opacity: 0')
      const rest = blockOf(hover, '.day.hasActions:not(.isLive) .actions')
      expect(rest).toContain('opacity: 0')
      expect(rest).toContain('transform: translateY(8px)')
      expect(rest).toMatch(/transition:\s*opacity 200ms,\s*transform 200ms/)
      const shown = blockOf(
        hover,
        '.day.hasActions:hover .actions,\n    .day.hasActions:focus-within .actions'
      )
      expect(shown).toContain('opacity: 1')
      expect(shown).toContain('transform: translateY(0)')
    })

    it('should hide the waiting actions without removing them, so they stay in the accessibility tree', () => {
      for (const selector of [
        '.day.hasActions:not(.isLive) .actions',
        '.day.hasActions:not(.isLive) .action',
        '.showOnHover'
      ]) {
        expect(blockOf(hover, selector)).not.toMatch(/visibility:\s*hidden|display:\s*none/)
      }
      expect(blockOf(hover, '.day.hasActions:not(.isLive) .action')).toContain(
        'pointer-events: none'
      )
      expect(
        blockOf(hover, '.day.hasActions:hover .action,\n    .day.hasActions:focus-within .action')
      ).toContain('pointer-events: auto')
    })

    it("should fold a small card's host and open its 32px actions row together over 200ms", () => {
      expect(blockOf(hover, '.small .action')).toMatch(/width: 32px;\s*height: 32px/)
      // A row never grows past the text column, so a long time ends in an ellipsis beside the cover.
      expect(blockOf(hover, '.reveal')).toContain('grid-template-columns: minmax(0, 1fr)')
      expect(blockOf(hover, '.reveal')).toMatch(
        /transition:\s*grid-template-rows 200ms,\s*opacity 200ms,\s*transform 200ms/
      )
      const waiting = blockOf(hover, '.showOnHover')
      expect(waiting).toContain('grid-template-rows: 0fr')
      expect(waiting).toContain('opacity: 0')
      expect(waiting).toContain('transform: translateY(8px)')
      const shown = blockOf(
        hover,
        '.hasActions:hover .showOnHover,\n    .hasActions:focus-within .showOnHover'
      )
      expect(shown).toContain('grid-template-rows: 1fr')
      expect(shown).toContain('opacity: 1')
      const folded = blockOf(
        hover,
        '.hasActions:hover .hideOnHover,\n    .hasActions:focus-within .hideOnHover'
      )
      expect(folded).toContain('grid-template-rows: 0fr')
      expect(folded).toContain('opacity: 0')
    })
  })

  describe('when reduced motion is requested', () => {
    const reduced = mediaOf('(prefers-reduced-motion: reduce)')

    it('should show every state at once, in place', () => {
      expect(
        blockOf(
          reduced,
          '.card,\n    .day.hasActions .time,\n    .day.hasActions .actions,\n    .reveal'
        )
      ).toContain('transition: none')
      expect(
        blockOf(
          reduced,
          '.card:hover,\n    .card:focus-within,\n    .day.hasActions .actions,\n    .day.hasActions:hover .actions,\n    .day.hasActions:focus-within .actions,\n    .showOnHover'
        )
      ).toContain('transform: none')
    })
  })

  describe('when the screen is touch-only or narrow', () => {
    it('should keep the actions shown, unmoved and under the time', () => {
      // The hidden state exists only for a device that hovers from tablet width; at rest the actions show.
      expect(blockOf(css, '.actions')).not.toMatch(/opacity|transform|position/)
      expect(blockOf(css, '.footer')).toContain('flex-direction: column')
      expect(blockOf(css, '.action')).toMatch(/width: 44px;\s*height: 44px/)
      expect(blockOf(css, '.small .action')).not.toMatch(/width|height/)
      expect(blockOf(css, '.small .media')).toMatch(/width: 135px;\s*height: 135px/)
      // The small card's folding rows exist only for a device that hovers.
      expect(blockOf(css, '.small .body')).not.toMatch(/grid-template-rows/)
      const outsideHover = css
        .replace(mediaOf('(hover: hover) and (min-width: 768px)'), '')
        .replace(mediaOf('(prefers-reduced-motion: reduce)'), '')
      expect(outsideHover).not.toMatch(/\n\s*\.(showOnHover|reveal) \{/)
    })
  })

  describe('when a day card is drawn', () => {
    it('should draw no border, outline or inset line around it, at rest or on hover', () => {
      const outsideForced = css.replace(/@media \(forced-colors: active\) \{[\s\S]*?\n {2}\}\n/, '')
      const dayRules = [...outsideForced.matchAll(/\n *([^\n{}@]*\.day[^\n{}]*) \{([^}]*)\}/g)]
      expect(dayRules.length).toBeGreaterThan(0)
      for (const [, selector, body] of dayRules) {
        if (/\.day\s+\./.test(selector ?? '')) continue
        expect(body, selector).not.toMatch(/(^|\s)(border|outline|box-shadow)\s*:/)
      }
    })

    it('should give it a system-colour boundary only when forced colours drop its background', () => {
      expect(blockOf(mediaOf('(forced-colors: active)'), '.day')).toMatch(
        /outline: 1px solid CanvasText;\s*outline-offset: -1px/
      )
    })
  })

  describe('when a small card is live', () => {
    it('should mark it with its surface and a 5px live bar', () => {
      expect(blockOf(css, '.small.isLive')).toContain('#680d4a')
      const bar = blockOf(css, '.small.isLive::before')
      expect(bar).toContain('width: 5px')
      expect(bar).toContain('background: var(--dcl-system-red)')
    })
  })
})
