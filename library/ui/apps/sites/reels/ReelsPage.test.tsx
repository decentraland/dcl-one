import { fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'

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
})
