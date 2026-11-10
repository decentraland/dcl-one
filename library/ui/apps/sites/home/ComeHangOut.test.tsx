import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { ComeHangOut } from './ComeHangOut'

describe('ComeHangOut', () => {
  describe('when rendered on desktop', () => {
    it('should be a named section with the download actions and a decorative background', () => {
      const { container } = render(
        <ComeHangOut
          title="Come Hang Out"
          actions={{
            primary: {
              label: 'Download for',
              os: 'macos',
              osName: 'macOS',
              href: '/download_success?os=macOS&place=come-hang-out',
              onClick: vi.fn()
            }
          }}
        />
      )

      expect(screen.getByRole('region', { name: 'Come Hang Out' })).toBeInTheDocument()
      expect(screen.getByRole('link', { name: 'Download for macOS' })).toHaveAttribute(
        'href',
        '/download_success?os=macOS&place=come-hang-out'
      )
      expect(container.querySelector('img')).toHaveAttribute('alt', '')
    })
  })

  describe('when rendered on a phone', () => {
    it('should show the store badge instead of the actions', () => {
      render(
        <ComeHangOut
          title="Come Hang Out"
          store={{
            badge: 'google_play',
            label: 'Get it on Google Play',
            href: 'https://play.example',
            onClick: vi.fn()
          }}
        />
      )

      expect(screen.getByRole('link', { name: 'Get it on Google Play' })).toBeInTheDocument()
      expect(screen.queryByRole('link', { name: /Download for/ })).not.toBeInTheDocument()
    })
  })

  describe('when its owning route supplies the shared animation', () => {
    it('should keep the decoration outside the accessible foreground controls', () => {
      render(
        <ComeHangOut title="Come Hang Out" background={<span>Shared animated background</span>} />
      )
      expect(screen.getByText('Shared animated background').parentElement).toHaveAttribute(
        'aria-hidden',
        'true'
      )
      expect(screen.getByRole('heading', { name: 'Come Hang Out' })).toBeInTheDocument()
    })
  })
})
