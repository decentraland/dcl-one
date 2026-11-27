import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { DownloadActions, StoreBadge } from './DownloadActions'

describe('DownloadActions', () => {
  describe('when the visitor has an installer and the Epic store', () => {
    it('should name the installer link with its platform', () => {
      render(
        <DownloadActions
          appearance="source"
          primary={{ label: 'Download for', os: 'windows', osName: 'Windows', href: '/win.exe' }}
        />
      )
      expect(screen.getByRole('link', { name: 'Download for Windows' }).getAttribute('href')).toBe(
        '/win.exe'
      )
    })

    it('should open the Epic store in a new tab without a handle on this page', () => {
      render(
        <DownloadActions
          appearance="source"
          epic={{ label: 'Download on', iconLabel: 'Epic Games', href: 'https://epic.example' }}
        />
      )
      const epic = screen.getByRole('link', { name: 'Download on Epic Games' })
      expect(epic.getAttribute('target')).toBe('_blank')
      expect(epic.getAttribute('rel')).toBe('noopener noreferrer')
    })
  })

  describe('when no installer runs on the visitor desktop', () => {
    it('should name the link by its label alone', () => {
      render(<DownloadActions primary={{ label: 'Download', href: '/download' }} />)
      expect(screen.getByRole('link', { name: 'Download' })).toBeTruthy()
    })
  })

  describe('when neither an installer nor the Epic store is offered', () => {
    it('should render no call to action', () => {
      render(<DownloadActions downloads="+400K" />)
      expect(screen.queryByRole('link')).toBeNull()
      expect(screen.getByText('+400K')).toBeTruthy()
    })
  })

  describe('when an alternative opens a dialog instead of a link', () => {
    it('should call its handler from a button named by the platform', async () => {
      const onClick = vi.fn()
      render(<DownloadActions alternativeGroups={[[{ icon: 'ios', label: 'iOS', onClick }]]} />)
      await userEvent.click(screen.getByRole('button', { name: 'iOS' }))
      expect(onClick).toHaveBeenCalledOnce()
    })
  })
})

describe('StoreBadge', () => {
  it('should open the store in a new tab named by its label', () => {
    render(
      <StoreBadge badge="app_store" label="Download on the App Store" href="https://apps.example" />
    )
    const badge = screen.getByRole('link', { name: 'Download on the App Store' })
    expect(badge.getAttribute('target')).toBe('_blank')
    expect(badge.getAttribute('rel')).toBe('noopener noreferrer')
  })
})
