import { render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { DownloadDialog } from './DownloadDialog'

describe('DownloadDialog', () => {
  describe('when open on desktop', () => {
    it('should be a named dialog offering the download actions', () => {
      render(
        <DownloadDialog
          title="Download Decentraland to Jump In"
          closeLabel="Close"
          onClose={vi.fn()}
          actions={{
            primary: {
              label: 'Download for',
              os: 'windows',
              osName: 'Windows',
              href: '/download_success?os=Windows&place=download-modal',
              onClick: vi.fn()
            }
          }}
        />
      )

      const dialog = screen.getByRole('dialog', { name: 'Download Decentraland to Jump In' })
      expect(within(dialog).getByRole('link', { name: 'Download for Windows' })).toHaveAttribute(
        'href',
        '/download_success?os=Windows&place=download-modal'
      )
    })
  })

  describe('when open on a phone', () => {
    it('should offer the store badge', () => {
      render(
        <DownloadDialog
          title="t"
          closeLabel="Close"
          onClose={vi.fn()}
          store={{
            badge: 'app_store',
            label: 'Download on the App Store',
            href: 'https://apps.example',
            onClick: vi.fn()
          }}
        />
      )

      expect(screen.getByRole('link', { name: 'Download on the App Store' })).toBeInTheDocument()
    })
  })
})
