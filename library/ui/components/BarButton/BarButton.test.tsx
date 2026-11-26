import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { BarButton } from './BarButton'

describe('BarButton', () => {
  describe('when rendered with a label', () => {
    it('is a native button named by its content, a pill of the default size', () => {
      render(<BarButton>Map</BarButton>)
      const button = screen.getByRole('button', { name: 'Map' })
      expect(button).toHaveAttribute('type', 'button')
      expect(button.className).toMatch(/pill/)
      expect(button.className).not.toMatch(/round|lg|open/)
    })

    it('calls onClick and keeps the consumer class', () => {
      const onClick = vi.fn()
      render(
        <BarButton className="extra" onClick={onClick}>
          Places
        </BarButton>
      )
      const button = screen.getByRole('button', { name: 'Places' })
      fireEvent.click(button)
      expect(onClick).toHaveBeenCalledTimes(1)
      expect(button.className).toMatch(/extra/)
    })
  })

  describe('when it is a round button', () => {
    it('is named by aria-label and takes the large size', () => {
      render(<BarButton shape="round" size="lg" aria-label="Close" />)
      const button = screen.getByRole('button', { name: 'Close' })
      expect(button.className).toMatch(/round/)
      expect(button.className).toMatch(/lg/)
      expect(button.className).not.toMatch(/pill/)
    })
  })

  describe('when what it opens is open', () => {
    it('keeps the open look and reports the expanded state the caller passes', () => {
      render(<BarButton shape="round" open aria-label="Notifications" aria-expanded />)
      const button = screen.getByRole('button', { name: 'Notifications', expanded: true })
      expect(button.className).toMatch(/open/)
    })
  })

  describe('when given a badge', () => {
    it('shows the counter inside the button', () => {
      render(<BarButton shape="round" aria-label="Notifications, 3 unread" badge={3} />)
      expect(screen.getByRole('button', { name: 'Notifications, 3 unread' })).toHaveTextContent('3')
    })

    it('shows a zero', () => {
      render(<BarButton shape="round" aria-label="Notifications" badge={0} />)
      expect(screen.getByRole('button')).toHaveTextContent('0')
    })

    it('renders no badge without one', () => {
      const { container } = render(<BarButton shape="round" aria-label="Notifications" />)
      expect(container.querySelector('span')).toBeNull()
    })
  })

  describe('when disabled', () => {
    it('does not call onClick', () => {
      const onClick = vi.fn()
      render(
        <BarButton disabled onClick={onClick}>
          Map
        </BarButton>
      )
      fireEvent.click(screen.getByRole('button', { name: 'Map' }))
      expect(onClick).not.toHaveBeenCalled()
    })
  })
})
