import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import {
  EventCard,
  EventCardAction,
  EventCardActionLink,
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
  describe('when rendered with an event', () => {
    it('names the card by a level-3 heading that links to the event', () => {
      setup()
      const heading = screen.getByRole('heading', { level: 3, name: 'Poker Night' })
      expect(heading.querySelector('a')).toHaveAttribute('href', '/events?id=poker')
    })

    it('shows the start time with its machine-readable instant', () => {
      setup()
      expect(screen.getByText('Tomorrow at noon').closest('time')).toHaveAttribute(
        'dateTime',
        '2026-11-13T12:00:00Z'
      )
    })

    it('uses the heading level it is given', () => {
      setup({ headingLevel: 4 })
      expect(screen.getByRole('heading', { level: 4, name: 'Poker Night' })).toBeInTheDocument()
    })

    it('reports the open with the click, so the caller can open the event in place', () => {
      const onOpen = vi.fn((click: { preventDefault: () => void }) => click.preventDefault())
      setup({ onOpen })
      const link = screen.getByRole('link', { name: 'Poker Night' })
      expect(fireEvent.click(link)).toBe(false)
      expect(onOpen).toHaveBeenCalledOnce()
    })

    it('renders the title through the router link it is given', () => {
      const RouterLink = vi.fn((props: EventCardLinkProps) => <a data-router="" {...props} />)
      setup({ linkComponent: RouterLink })
      expect(screen.getByRole('link', { name: 'Poker Night' })).toHaveAttribute('data-router')
      expect(RouterLink).toHaveBeenCalled()
    })
  })

  describe('when it has a host', () => {
    it('links the host to their profile beside the card link', () => {
      setup({ creator: { name: 'Alice', profileHref: '/profile/alice' } })
      expect(screen.getByRole('link', { name: 'Alice' })).toHaveAttribute('href', '/profile/alice')
      expect(screen.getByText(/^by/)).toHaveTextContent('by Alice')
    })

    it('shows the host face when there is one', () => {
      const { container } = setup({ creator: { name: 'Alice', avatar: 'https://example.org/a.png' } })
      expect(container.querySelector('img[src="https://example.org/a.png"]')).toHaveAttribute('alt', '')
    })

    it('shows the initial, hidden from assistive technology, without a face', () => {
      setup({ creator: { name: 'alice' } })
      const initial = screen.getByText('A')
      expect(initial).toHaveAttribute('aria-hidden', 'true')
      expect(screen.queryByRole('link', { name: 'alice' })).toBeNull()
    })

    it('uses the word it is given before the name', () => {
      setup({ creator: { name: 'Alice' }, byLabel: 'By' })
      expect(screen.getByText(/^By/)).toHaveTextContent('By Alice')
    })
  })

  describe('when it has actions', () => {
    it('renders them in the card, reachable by keyboard', () => {
      setup({ actions: <EventCardAction aria-label="Remind me: Poker Night" icon={<svg />} /> })
      expect(screen.getByRole('button', { name: 'Remind me: Poker Night' })).toBeInTheDocument()
      expect(screen.getByRole('article')).toHaveClass('hasActions')
    })

    it('marks no actions when it has none', () => {
      setup({ actions: null })
      expect(screen.getByRole('article')).not.toHaveClass('hasActions')
      expect(screen.queryByRole('button')).toBeNull()
    })

    it('announces an action outcome without showing it', () => {
      setup({ actions: <EventCardAction aria-label="Copy link" icon={<svg />} />, announcement: 'Link copied.' })
      expect(screen.getByRole('status')).toHaveTextContent('Link copied.')
    })

    it('has no status region without an announcement', () => {
      setup()
      expect(screen.queryByRole('status')).toBeNull()
    })
  })

  describe('when the image is missing or unsafe', () => {
    it('keeps the compact cover box without an image', () => {
      const { container } = setup({ image: null })
      expect(container.querySelector('img')).toBeNull()
      expect(container.querySelector('.media')).not.toBeNull()
    })

    it('drops an image that is not https', () => {
      const { container } = setup({ image: 'http://example.org/poker.png' })
      expect(container.querySelector('img')).toBeNull()
    })

    it('leaves the day card without a cover when it has no image and is not live', () => {
      const { container } = setup({ variant: 'day', image: null })
      expect(container.querySelector('.media')).toBeNull()
    })
  })

  describe('when the event is live', () => {
    it('shows LIVE and a known count of people, named for assistive technology', () => {
      setup({ variant: 'day', live: true, players: 12, playersLabel: '12 online', time: undefined })
      expect(screen.getByText('LIVE')).toBeInTheDocument()
      expect(screen.getByText('12 online')).toBeInTheDocument()
      expect(screen.getByText('12')).toHaveAttribute('aria-hidden', 'true')
      expect(screen.getByRole('article')).toHaveClass('isLive')
    })

    it('shows a known zero as 0', () => {
      setup({ variant: 'day', live: true, players: 0, playersLabel: '0 online' })
      expect(screen.getByText('0 online')).toBeInTheDocument()
    })

    it('shows no count when it is unknown', () => {
      const { container } = setup({ variant: 'day', live: true })
      expect(screen.getByText('LIVE')).toBeInTheDocument()
      expect(container.querySelector('.players')).toBeNull()
    })

    it('keeps the cover for its badges even without an image', () => {
      const { container } = setup({ variant: 'day', live: true, image: null })
      expect(container.querySelector('.media')).not.toBeNull()
      expect(container.querySelector('img')).toBeNull()
    })
  })
})

describe('EventCardAction', () => {
  it('is a named button that reports a press and keeps its toggle state', () => {
    const onClick = vi.fn()
    render(<EventCardAction aria-label="Remind me" aria-pressed icon={<svg />} onClick={onClick} />)
    const button = screen.getByRole('button', { name: 'Remind me' })
    expect(button).toHaveAttribute('type', 'button')
    expect(button).toHaveAttribute('aria-pressed', 'true')
    fireEvent.click(button)
    expect(onClick).toHaveBeenCalledOnce()
  })

  it('reports no press while disabled', () => {
    const onClick = vi.fn()
    render(<EventCardAction aria-label="Remind me" disabled icon={<svg />} onClick={onClick} />)
    fireEvent.click(screen.getByRole('button', { name: 'Remind me' }))
    expect(onClick).not.toHaveBeenCalled()
  })
})

describe('EventCardActionLink', () => {
  it('is a named link that keeps its target and relation', () => {
    render(
      <EventCardActionLink
        aria-label="Add to calendar"
        href="https://calendar.example/add"
        target="_blank"
        rel="noopener noreferrer"
        icon={<svg />}
      />
    )
    const link = screen.getByRole('link', { name: 'Add to calendar' })
    expect(link).toHaveAttribute('href', 'https://calendar.example/add')
    expect(link).toHaveAttribute('rel', 'noopener noreferrer')
  })
})

/** The module's text, and the declarations of the rule whose selector list is exactly `selector`. */
// Vitest runs from library/ui (its config's root), where jsdom gives no file URL for this module.
const css = readFileSync(join(process.cwd(), 'components/EventCard/EventCard.module.css'), 'utf8')
const blockOf = (source: string, selector: string): string => {
  const start = source.indexOf(`${selector} {`)
  if (start < 0) throw new Error(`no rule ${selector}`)
  return source.slice(start, source.indexOf('}', start))
}
const mediaOf = (query: string): string => {
  const start = css.indexOf(`@media ${query} {`)
  if (start < 0) throw new Error(`no @media ${query}`)
  // The block ends at the first line that closes it at its own indentation.
  return css.slice(start, css.indexOf('\n  }\n', start))
}

describe('EventCard motion', () => {
  const hover = mediaOf('(hover: hover) and (min-width: 768px)')

  it('lifts and lights the card over 200ms on hover and keyboard focus', () => {
    expect(blockOf(css, '.card')).toMatch(/transition:\s*transform 200ms,\s*box-shadow 200ms/)
    const lifted = blockOf(hover, '.card:hover,\n    .card:focus-within')
    expect(lifted).toContain('transform: translateY(-4px)')
    expect(lifted).toContain('box-shadow: 0 2px 12px 12px')
  })

  it('fades the time out and the actions up 8px into its place', () => {
    expect(blockOf(hover, '.hasActions .time')).toContain('transition: opacity 200ms')
    expect(blockOf(hover, '.hasActions:hover .time,\n    .hasActions:focus-within .time')).toContain(
      'opacity: 0'
    )
    const rest = blockOf(hover, '.hasActions .actions')
    expect(rest).toContain('opacity: 0')
    expect(rest).toContain('transform: translateY(8px)')
    expect(rest).toMatch(/transition:\s*opacity 200ms,\s*transform 200ms/)
    const shown = blockOf(hover, '.hasActions:hover .actions,\n    .hasActions:focus-within .actions')
    expect(shown).toContain('opacity: 1')
    expect(shown).toContain('transform: translateY(0)')
  })

  it('hides the waiting actions by opacity only, so they stay in the accessibility tree', () => {
    expect(hover).not.toMatch(/visibility:\s*hidden|display:\s*none/)
    expect(blockOf(hover, '.hasActions .action')).toContain('pointer-events: none')
    expect(blockOf(hover, '.hasActions:hover .action,\n    .hasActions:focus-within .action')).toContain(
      'pointer-events: auto'
    )
  })

  it('shows every state at once, in place, under reduced motion', () => {
    const reduced = mediaOf('(prefers-reduced-motion: reduce)')
    expect(blockOf(reduced, '.card,\n    .hasActions .time,\n    .hasActions .actions')).toContain(
      'transition: none'
    )
    expect(
      blockOf(
        reduced,
        '.card:hover,\n    .card:focus-within,\n    .hasActions .actions,\n    .hasActions:hover .actions,\n    .hasActions:focus-within .actions'
      )
    ).toContain('transform: none')
  })

  it('keeps the actions shown, unmoved and under the time on a touch or narrow screen', () => {
    // The hidden state exists only for a device that hovers from tablet width; at rest the actions show.
    expect(blockOf(css, '.actions')).not.toMatch(/opacity|transform|position/)
    expect(blockOf(css, '.footer')).toContain('flex-direction: column')
    expect(blockOf(css, '.action')).toMatch(/width: 44px;\s*height: 44px/)
    expect(mediaOf('(hover: none), (max-width: 767px)')).toContain('min-height: 176px')
  })
})
