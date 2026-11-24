// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { UpcomingEventCardProps } from '../events/types'
import {
  CommunityEventsList,
  type CommunityEventsListProps,
  type CommunityListMore
} from './CommunityLists'

afterEach(cleanup)

const more: CommunityListMore = {
  state: 'none',
  manual: false,
  sentinelRef: null,
  onLoadMore: () => {}
}

const card = (over: Partial<UpcomingEventCardProps> = {}): UpcomingEventCardProps => ({
  id: 'ev-1',
  title: 'Takeover Tuesday',
  image: null,
  href: '/events?id=ev-1',
  time: { label: 'Tomorrow, 6:00 PM' },
  ...over
})

describe('CommunityEventsList', () => {
  let onRetry: ReturnType<typeof vi.fn>

  const show = (props: Partial<CommunityEventsListProps>) => {
    onRetry = vi.fn()
    render(
      <CommunityEventsList status="ready" cards={[]} onRetry={onRetry} more={more} {...props} />
    )
  }

  describe('when the first page is loading', () => {
    beforeEach(() => {
      show({ status: 'pending' })
    })

    it('should mark the section busy and show a loader', () => {
      expect(
        screen.getByRole('region', { name: 'Upcoming Events' }).getAttribute('aria-busy')
      ).toBe('true')
      expect(screen.getByRole('progressbar', { name: 'Loading events' })).toBeTruthy()
    })
  })

  describe('when the community has no upcoming events', () => {
    beforeEach(() => {
      show({})
    })

    it('should say so', () => {
      expect(screen.getByText('No Upcoming Events')).toBeTruthy()
      expect(screen.queryByRole('list')).toBeNull()
    })
  })

  describe('when the first page fails', () => {
    beforeEach(() => {
      show({ status: 'error' })
      fireEvent.click(screen.getByRole('button', { name: 'Try again' }))
    })

    it('should say so and read it again on Try again', () => {
      expect(screen.getByText("We couldn't load the events right now.")).toBeTruthy()
      expect(onRetry).toHaveBeenCalledTimes(1)
    })
  })

  describe('when a later page fails with events already shown', () => {
    let onLoadMore: ReturnType<typeof vi.fn>

    beforeEach(() => {
      onLoadMore = vi.fn()
      show({ status: 'error', cards: [card()], more: { ...more, state: 'error', onLoadMore } })
      fireEvent.click(screen.getByRole('button', { name: 'Try again' }))
    })

    it('should keep the events and read the next page again', () => {
      expect(screen.getByRole('link', { name: 'Takeover Tuesday' })).toBeTruthy()
      expect(screen.getByText("We couldn't load more events.")).toBeTruthy()
      expect(onLoadMore).toHaveBeenCalledTimes(1)
      expect(onRetry).not.toHaveBeenCalled()
    })
  })

  describe('when the community has events', () => {
    let onOpen: ReturnType<typeof vi.fn>

    beforeEach(() => {
      onOpen = vi.fn((click: MouseEvent) => click.preventDefault())
      show({
        cards: [card({ onOpen }), card({ id: 'ev-2', title: 'Art Week', href: '/events?id=ev-2' })]
      })
    })

    it('should show one card per event, each linking to it', () => {
      expect(screen.getAllByRole('listitem')).toHaveLength(2)
      expect(screen.getByRole('link', { name: 'Art Week' }).getAttribute('href')).toBe(
        '/events?id=ev-2'
      )
    })

    describe('and the visitor clicks a card', () => {
      beforeEach(() => {
        fireEvent.click(screen.getByRole('link', { name: 'Takeover Tuesday' }))
      })

      it("should hand the click to the page's opener", () => {
        expect(onOpen).toHaveBeenCalledTimes(1)
      })
    })
  })

  describe('when a Remind me failed', () => {
    beforeEach(() => {
      show({ cards: [card()], notice: "Couldn't update your reminder. Try again." })
    })

    it('should say so', () => {
      expect(screen.getByRole('alert').textContent).toBe(
        "Couldn't update your reminder. Try again."
      )
    })
  })
})
