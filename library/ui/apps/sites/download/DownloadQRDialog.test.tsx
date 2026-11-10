import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { DownloadQRDialog } from './DownloadQRDialog'

describe('DownloadQRDialog', () => {
  describe('when open', () => {
    it('should be a named dialog with a described code and the counter', () => {
      render(
        <DownloadQRDialog
          os="ios"
          title="Download Decentraland App for iOS"
          qrLabel="QR code to download Decentraland for iOS"
          downloads="Total Downloads: +400K"
          closeLabel="Close"
          onClose={vi.fn()}
        />
      )

      const dialog = screen.getByRole('dialog', { name: 'Download Decentraland App for iOS' })
      expect(
        within(dialog).getByRole('img', { name: 'QR code to download Decentraland for iOS' })
      ).toBeInTheDocument()
      expect(dialog).toHaveTextContent('Total Downloads: +400K')
    })

    it('should close with its close button and with Escape', async () => {
      const onClose = vi.fn()
      const user = userEvent.setup()
      render(
        <DownloadQRDialog
          os="android"
          title="Android"
          qrLabel="QR code"
          closeLabel="Close"
          onClose={onClose}
        />
      )

      await user.click(screen.getByRole('button', { name: 'Close' }))
      await user.keyboard('{Escape}')
      expect(onClose).toHaveBeenCalledTimes(2)
    })
  })
})
