import { render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { WhatsOn, type WhatsOnCardData } from './WhatsOn'

const card = (id: string, extra: Partial<WhatsOnCardData> = {}): WhatsOnCardData => ({
  id,
  title: `Place ${id}`,
  image: null,
  href: `/places/${id}`,
  users: 12,
  usersLabel: '12 online',
  ...extra
})

const labels = { live: 'Live', by: 'by' }

describe('WhatsOn', () => {
  describe('when there are cards', () => {
    it('should be a section named by its heading with one list item per card', () => {
      render(
        <WhatsOn
          title="Jump Into What's Happening"
          cards={[card('1'), card('2')]}
          labels={labels}
          viewAll={{ label: 'View All', href: '/whats-on' }}
        />
      )

      const section = screen.getByRole('region', { name: "Jump Into What's Happening" })
      expect(within(section).getAllByRole('listitem')).toHaveLength(2)
      expect(screen.getByRole('link', { name: 'View All' })).toHaveAttribute('href', '/whats-on')
    })

    it('should make the title link cover the card, with badges and a "by" creator link and no Jump In', () => {
      render(
        <WhatsOn
          title="t"
          cards={[card('1', { creator: { name: 'Builder', href: '/profile/0xabc' } })]}
          labels={labels}
          viewAll={{ label: 'View All', href: '/whats-on' }}
        />
      )

      const title = screen.getByRole('link', { name: 'Place 1' })
      expect(title).toHaveAttribute('href', '/places/1')
      expect(screen.getByRole('link', { name: 'Builder' })).toHaveAttribute(
        'href',
        '/profile/0xabc'
      )
      expect(screen.getByText('by', { exact: false })).toBeInTheDocument()
      expect(screen.getByText('Live')).toBeInTheDocument()
      // Read out, not shown: the count is visually hidden (a browser check).
      expect(screen.getByText('12 online')).toBeInTheDocument()
      expect(screen.queryByRole('link', { name: /Jump In/ })).not.toBeInTheDocument()
    })

    it('should show the Decentraland logo for the foundation and an initial otherwise', () => {
      render(
        <WhatsOn
          title="t"
          cards={[
            card('1', { creator: { name: 'Decentraland Foundation', foundation: true } }),
            card('2', { creator: { name: 'builder' } })
          ]}
          labels={labels}
          viewAll={{ label: 'View All', href: '/whats-on' }}
        />
      )

      const [foundation, builder] = screen.getAllByRole('listitem')
      // The cards have no picture, so the only decorative image is the foundation's logo avatar.
      expect(within(foundation!).getAllByRole('presentation')).toHaveLength(1)
      expect(within(builder!).queryAllByRole('presentation')).toEqual([])
      // Anyone else gets their initial, hidden from readers next to the named creator link.
      expect(within(builder!).getByText('B')).toHaveAttribute('aria-hidden', 'true')
    })

    it('should give View All its arrow', () => {
      render(
        <WhatsOn
          title="t"
          cards={[card('1')]}
          labels={labels}
          viewAll={{ label: 'View All', href: '/whats-on' }}
        />
      )

      expect(
        screen.getByRole('link', { name: 'View All' }).querySelector('svg[aria-hidden="true"]')
      ).not.toBeNull()
    })
  })

  describe('when there are no cards', () => {
    it('should render nothing', () => {
      const { container } = render(
        <WhatsOn
          title="t"
          cards={[]}
          labels={labels}
          viewAll={{ label: 'View All', href: '/whats-on' }}
        />
      )

      expect(container).toBeEmptyDOMElement()
    })
  })
  describe('when a card or View All is followed', () => {
    it('should report which one, for analytics', async () => {
      const onCardClick = vi.fn()
      const onViewAll = vi.fn()
      render(
        <WhatsOn
          title="t"
          cards={[card('1')]}
          labels={labels}
          viewAll={{ label: 'View All', href: '#all', onClick: onViewAll }}
          onCardClick={onCardClick}
        />
      )

      screen
        .getByRole('link', { name: 'Place 1' })
        .addEventListener('click', (e) => e.preventDefault())
      screen.getByRole('link', { name: 'Place 1' }).click()
      screen.getByRole('link', { name: 'View All' }).click()
      expect(onCardClick).toHaveBeenCalledWith(expect.objectContaining({ id: '1' }))
      expect(onViewAll).toHaveBeenCalledTimes(1)
    })
  })
})
