import { readFileSync } from 'node:fs'
import path from 'node:path'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { DownloadPage, type DownloadPageProps } from './DownloadPage'

const noop = () => undefined
const desktop: DownloadPageProps = {
  title: 'Download Decentraland',
  reserveHeaderRow: true,
  jumpIn: { text: 'Already downloaded?', label: 'Jump in', onClick: noop },
  primary: {
    label: 'Download for',
    os: 'macos',
    osName: 'macOS',
    href: '/download_success?os=macOS',
    onClick: noop
  },
  epic: {
    label: 'Download on',
    iconLabel: 'Epic Games',
    href: 'https://store.epicgames.example/decentraland',
    onClick: noop
  },
  downloads: '+400K downloads',
  alternatives: [
    { icon: 'windows', label: 'Windows', href: '/download_success?os=Windows', onClick: noop },
    { icon: 'ios', label: 'iOS', href: 'https://apps.example/dcl', external: true, onClick: noop }
  ]
}

describe('DownloadPage', () => {
  describe('when rendered for a desktop visitor', () => {
    it('should expose one main landmark and the title as the page heading', () => {
      render(<DownloadPage {...desktop} />)

      expect(screen.getByRole('main')).toBeInTheDocument()
      expect(
        screen.getByRole('heading', { level: 1, name: 'Download Decentraland' })
      ).toBeInTheDocument()
    })

    it('should offer the installer as a link named with the OS', () => {
      render(<DownloadPage {...desktop} />)

      expect(screen.getByRole('link', { name: 'Download for macOS' })).toHaveAttribute(
        'href',
        '/download_success?os=macOS'
      )
    })

    it('should open Epic in a new tab, safely', () => {
      render(<DownloadPage {...desktop} />)

      const epic = screen.getByRole('link', { name: 'Download on Epic Games' })
      expect(epic).toHaveAttribute('target', '_blank')
      expect(epic).toHaveAttribute('rel', 'noopener noreferrer')
    })

    it('should list the other platforms as named icon-only links', () => {
      render(<DownloadPage {...desktop} />)

      const list = within(screen.getByRole('list'))
      expect(list.getByRole('link', { name: 'Windows' })).toHaveAttribute(
        'href',
        '/download_success?os=Windows'
      )
      expect(list.getByRole('link', { name: 'iOS' })).toHaveAttribute('target', '_blank')
    })

    it('should reach the options and then Jump In with Tab and run them from the keyboard', async () => {
      const onJumpIn = vi.fn()
      const user = userEvent.setup()
      render(<DownloadPage {...desktop} jumpIn={{ ...desktop.jumpIn!, onClick: onJumpIn }} />)

      await user.tab()
      expect(screen.getByRole('link', { name: 'Download for macOS' })).toHaveFocus()
      // Jump In sits under the options, as in source, so it is the last stop.
      const jumpIn = screen.getByRole('button', { name: 'Jump in' })
      // The shared Button's text variant, not a separately styled control.
      for (let i = 0; i < 10 && document.activeElement !== jumpIn; i++) await user.tab()
      expect(jumpIn).toHaveFocus()
      await user.keyboard(' ')
      expect(onJumpIn).toHaveBeenCalledTimes(1)
    })
  })

  describe('when given a store badge as well as the installers', () => {
    const store = {
      badge: 'google_play' as const,
      label: 'Get it on Google Play',
      href: 'https://play.example/dcl',
      onClick: vi.fn()
    }

    it('should render the store badge in the narrow layout, named on the link with a decorative image', () => {
      render(<DownloadPage {...desktop} store={store} />)

      const badge = screen.getByRole('link', { name: 'Get it on Google Play' })
      expect(badge).toHaveAttribute('target', '_blank')
      expect(badge.querySelector('img')).toHaveAttribute('alt', '')
    })

    it('should render both layouts for the viewport to pick: installers and Jump In wide, title on top narrow', () => {
      render(<DownloadPage {...desktop} store={store} />)

      const titles = screen.getAllByRole('heading', { level: 1 })
      expect(titles).toHaveLength(2)
      const [wideTitle] = titles
      expect(wideTitle!.parentElement).toContainElement(
        screen.getByRole('link', { name: 'Download for macOS' })
      )
    })
  })

  describe('when signed in', () => {
    it('should show the pre-title and the preview, without the signed-out header', () => {
      render(
        <DownloadPage
          {...desktop}
          reserveHeaderRow={false}
          preTitle="Alex is Ready!"
          preview={<div>avatar</div>}
        />
      )

      expect(screen.getByText('Alex is Ready!')).toBeInTheDocument()
      expect(screen.getByText('avatar')).toBeInTheDocument()
      expect(screen.queryByTestId('download-header-row')).toBeNull()
    })
  })

  describe('when the compact Sites header sits over the page', () => {
    it('should keep an empty, hidden first row for it, with no header of its own', () => {
      render(<DownloadPage {...desktop} />)

      // An empty aria-hidden row has no role or label, so it carries a test id.
      const row = screen.getByTestId('download-header-row')
      expect(row).toBe(screen.getByRole('main').firstElementChild)
      expect(row).toHaveAttribute('aria-hidden', 'true')
      expect(row).toBeEmptyDOMElement()
      expect(screen.queryByRole('banner')).toBeNull()
      expect(screen.queryByRole('button', { name: 'Sign In' })).toBeNull()
    })

    it('should fail without the reserved row (control)', () => {
      render(<DownloadPage {...desktop} reserveHeaderRow={false} />)

      expect(screen.queryByTestId('download-header-row')).toBeNull()
    })

    it('should size the row as the header and drop it from desktop up', () => {
      const css = readFileSync(path.resolve(import.meta.dirname, 'DownloadPage.module.css'), 'utf8')
      expect(css).toMatch(/\.headerRow \{\s*grid-column: 1 \/ -1;\s*height: var\(--dcl-space-48\);/)
      expect(css).toMatch(/min-width: 1024px\)[\s\S]*\.headerRow \{\s*display: none;/)
    })
  })

  describe('when Jump In found no installed app', () => {
    it('should open a dialog whose action closes it', async () => {
      const onClose = vi.fn()
      const user = userEvent.setup()
      render(
        <DownloadPage
          {...desktop}
          launcherMissing={{
            titleLines: ["You don't have the", 'Decentraland app yet'],
            cta: 'Download',
            closeLabel: 'Close',
            onClose
          }}
        />
      )

      const dialog = screen.getByRole('dialog')
      expect(dialog).toHaveTextContent("You don't have theDecentraland app yet")
      await user.click(within(dialog).getByRole('button', { name: 'Download' }))
      expect(onClose).toHaveBeenCalledTimes(1)
    })
  })

  describe('when the viewport sits between breakpoints', () => {
    it('should hide exactly one layout at any width, fractional ones included', () => {
      const css = readFileSync(path.resolve(import.meta.dirname, 'DownloadPage.module.css'), 'utf8')
      // The narrow query is the negation of the wide one, so no width shows both or neither.
      expect(css).toMatch(
        /@media not all and \(min-width: 1024px\) \{\s*\.wideOnly \{\s*display: none;/
      )
      expect(css).toMatch(/@media \(min-width: 1024px\) \{[^@]*\.narrowOnly \{\s*display: none;/)
      expect(css).not.toContain('max-width: 1023px')
    })
  })
})
