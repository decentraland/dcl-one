import { fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import type { PlaceCardLinkProps } from '../../../index'
import { PhotoViewer } from '../profile/PhotoViewer'
import { ReelsGallery, ReelsPage } from './ReelsPage'

function Link({ to, ...props }: PlaceCardLinkProps) {
  return <a href={to} {...props} />
}

describe('Reels', () => {
  describe('when a Reel is opened as a page', () => {
    it('should show its actions and allow keyboard access to photo information', async () => {
      const user = userEvent.setup()
      render(
        <ReelsPage title="Photo at the Plaza">
          <PhotoViewer
            variant="page"
            imageUrl="/photo.jpg"
            alt="Plaza"
            label="Photo at the Plaza"
            details={{
              place: { label: 'Plaza', href: '/places/plaza' },
              people: [{ name: 'Guest', isGuest: true, wearables: [{ id: 'hat', label: 'Hat' }] }]
            }}
            actions={{ shareHref: 'https://x.com/intent/post', onCopy: () => {} }}
            linkComponent={Link}
          />
        </ReelsPage>
      )
      expect(screen.queryByRole('dialog')).toBeNull()
      expect(screen.getByRole('heading', { name: 'Photo at the Plaza' })).toBeInTheDocument()
      expect(screen.getByRole('link', { name: 'Share to X' })).toHaveAttribute('target', '_blank')
      const info = screen.getByRole('button', { name: 'Information' })
      info.focus()
      await user.keyboard('{Enter}')
      expect(info).toHaveAttribute('aria-expanded', 'false')
      expect(screen.queryByRole('link', { name: 'Plaza' })).toBeNull()
      await user.keyboard(' ')
      expect(info).toHaveAttribute('aria-expanded', 'true')
      expect(screen.getByRole('link', { name: 'Plaza' })).toHaveAttribute('href', '/places/plaza')
      fireEvent.error(screen.getByRole('img', { name: 'Plaza' }))
      expect(screen.getByText('Photo could not be loaded.')).toBeInTheDocument()
    })
  })

  describe('when the thumbnail cannot load', () => {
    it('should keep the gallery link and use the original image', () => {
      render(
        <ReelsGallery
          total={25}
          photos={[
            {
              id: '1',
              href: '/reels/1',
              label: 'Photo in a world',
              thumbnail: '/thumb.jpg',
              image: '/full.jpg'
            }
          ]}
          linkComponent={Link}
        />
      )
      expect(screen.getByText('Showing 1 of 25 public photos')).toBeInTheDocument()
      const link = screen.getByRole('link', { name: 'Photo in a world' })
      const thumbnail = within(link).getByAltText('')
      expect(thumbnail).toHaveAttribute('src', '/thumb.jpg')
      fireEvent.error(thumbnail)
      expect(within(link).getByAltText('')).toHaveAttribute('src', '/full.jpg')
      expect(link).toHaveAttribute('href', '/reels/1')
    })
  })

  describe('when a host listens to the controls', () => {
    it('should report each control before following it', async () => {
      const user = userEvent.setup()
      const events = {
        onShare: vi.fn(),
        onDetailsToggle: vi.fn(),
        onTakenByClick: vi.fn(),
        onPlaceClick: vi.fn(),
        onJumpInClick: vi.fn(),
        onPersonClick: vi.fn(),
        onWearablesToggle: vi.fn(),
        onWearableClick: vi.fn()
      }
      const onHomeClick = vi.fn()
      const block = (event: MouseEvent) => event.preventDefault()
      document.addEventListener('click', block)
      render(
        <ReelsPage title="Photo at the Plaza" onHomeClick={onHomeClick}>
          <PhotoViewer
            variant="page"
            imageUrl="/photo.jpg"
            alt="Plaza"
            label="Photo at the Plaza"
            details={{
              takenBy: { name: 'Alice', href: '/profile/alice' },
              place: { label: 'Plaza', href: '/places/plaza', jumpInHref: '/play/?position=0,0' },
              people: [
                { name: 'Guest', isGuest: true },
                {
                  name: 'Bob',
                  href: '/profile/bob',
                  isGuest: false,
                  wearables: [
                    { id: 'cap', label: 'Cap' },
                    { id: 'hat', label: 'Hat', href: '/shop/hat' }
                  ]
                }
              ]
            }}
            actions={{ shareHref: 'https://x.com/intent/post' }}
            events={events}
            linkComponent={Link}
          />
        </ReelsPage>
      )
      await user.click(screen.getByRole('link', { name: 'Decentraland home' }))
      await user.click(screen.getByRole('link', { name: 'Share to X' }))
      await user.click(screen.getByRole('link', { name: 'Alice' }))
      await user.click(screen.getByRole('link', { name: 'Plaza' }))
      await user.click(screen.getByRole('link', { name: 'Jump in to Plaza' }))
      await user.click(screen.getByRole('link', { name: 'Bob' }))
      await user.click(screen.getByText('Collectible wearables'))
      await user.click(screen.getByRole('link', { name: 'Hat' }))
      await user.click(screen.getByRole('button', { name: 'Information' }))
      document.removeEventListener('click', block)
      expect(onHomeClick).toHaveBeenCalledTimes(1)
      expect(events.onShare).toHaveBeenCalledTimes(1)
      expect(events.onTakenByClick).toHaveBeenCalledTimes(1)
      expect(events.onPlaceClick).toHaveBeenCalledTimes(1)
      expect(events.onJumpInClick).toHaveBeenCalledTimes(1)
      expect(events.onPersonClick.mock.calls).toEqual([[1]])
      expect(events.onWearablesToggle.mock.calls).toEqual([[1, true]])
      expect(events.onWearableClick.mock.calls).toEqual([[1, 1]])
      expect(events.onDetailsToggle.mock.calls).toEqual([[false]])
    })

    it('should report the opened gallery photo by its id', async () => {
      const onPhotoClick = vi.fn()
      const block = (event: MouseEvent) => event.preventDefault()
      document.addEventListener('click', block)
      render(
        <ReelsGallery
          total={1}
          photos={[
            {
              id: 'p-1',
              href: '/reels/p-1',
              label: 'Photo one',
              thumbnail: '/t.jpg',
              image: '/i.jpg'
            }
          ]}
          linkComponent={Link}
          onPhotoClick={onPhotoClick}
        />
      )
      await userEvent.click(screen.getByRole('link', { name: 'Photo one' }))
      document.removeEventListener('click', block)
      expect(onPhotoClick.mock.calls).toEqual([['p-1']])
    })
  })
})
