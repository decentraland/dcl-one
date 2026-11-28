import type { MouseEvent } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import { fireEvent, render, screen, within } from '@testing-library/react'

import { JumpInModal, type JumpInModalPlace } from './JumpInModal'

const labels = {
  close: 'Close',
  jumpIn: 'Jump in',
  copyLink: 'Copy link',
  copied: 'Copied!',
  by: 'By',
  whatToExpect: 'What to expect'
}
const JUMP = '/play/?position=1%2C95'
const place: JumpInModalPlace = {
  title: 'Quiet',
  image: null,
  coords: '1,95',
  jumpHref: JUMP,
  shareUrl: 'https://example.test/places/quiet'
}

describe('JumpInModal', () => {
  let onClose: Mock<() => void>
  let onJumpIn: Mock<(event: MouseEvent<HTMLAnchorElement>) => void>
  let dialog: HTMLElement

  beforeEach(() => {
    onClose = vi.fn()
    onJumpIn = vi.fn((event) => event.preventDefault())
  })

  afterEach(() => {
    vi.resetAllMocks()
  })

  describe('when it opens for a place with a jump link', () => {
    beforeEach(() => {
      render(<JumpInModal place={place} labels={labels} onClose={onClose} onJumpIn={onJumpIn} />)
      dialog = screen.getByRole('dialog', { name: 'Quiet' })
    })

    it('should link Jump in to the scene', () => {
      expect(within(dialog).getByRole('link', { name: 'Jump in' })).toHaveAttribute('href', JUMP)
    })

    it('should hand the Jump in click to the host', () => {
      fireEvent.click(within(dialog).getByRole('link', { name: 'Jump in' }))
      expect(onJumpIn).toHaveBeenCalledOnce()
    })

    it('should draw the copy button as the shared outlined icon button', () => {
      const copy = within(dialog).getByRole('button', { name: 'Copy link' })
      expect(copy).toHaveAttribute('data-variant', 'outline')
      expect(copy).toHaveAttribute('data-icon-only', 'true')
    })

    describe('and the visitor presses the close button', () => {
      beforeEach(() => {
        fireEvent.click(within(dialog).getByRole('button', { name: 'Close' }))
      })

      it('should ask the host to close it', () => {
        expect(onClose).toHaveBeenCalledOnce()
      })
    })
  })

  describe('when the place has no jump link', () => {
    beforeEach(() => {
      render(
        <JumpInModal
          place={{ ...place, jumpHref: undefined }}
          labels={labels}
          onClose={onClose}
          onJumpIn={onJumpIn}
        />
      )
      dialog = screen.getByRole('dialog', { name: 'Quiet' })
    })

    it('should offer no Jump in', () => {
      expect(within(dialog).queryByRole('link', { name: 'Jump in' })).toBeNull()
      expect(within(dialog).queryByRole('button', { name: 'Jump in' })).toBeNull()
    })

    it('should still offer the close and copy buttons', () => {
      expect(within(dialog).getByRole('button', { name: 'Close' })).toBeInTheDocument()
      expect(within(dialog).getByRole('button', { name: 'Copy link' })).toBeInTheDocument()
    })
  })
})
