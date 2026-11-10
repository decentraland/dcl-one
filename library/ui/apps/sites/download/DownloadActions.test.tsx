import styles from './DownloadActions.module.css'
import { readFileSync } from 'node:fs'
import path from 'node:path'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { DownloadActions, StoreBadge } from './DownloadActions'

describe('DownloadActions', () => {
  describe('when an alternative has no href', () => {
    it('should render it as a named button that runs its action from the keyboard', async () => {
      const onClick = vi.fn()
      const user = userEvent.setup()
      render(
        <DownloadActions
          alternatives={[
            { icon: 'ios', label: 'iOS', onClick },
            { icon: 'macos', label: 'macOS', href: '/download_success?os=macOS', onClick }
          ]}
        />
      )

      const button = screen.getByRole('button', { name: 'iOS' })
      await user.tab()
      expect(button).toHaveFocus()
      await user.keyboard('{Enter}')
      expect(onClick).toHaveBeenCalledTimes(1)
    })
  })

  describe('when an alternative has an href', () => {
    it('should render it as a named link', () => {
      render(
        <DownloadActions
          alternatives={[
            { icon: 'macos', label: 'macOS', href: '/download_success?os=macOS', onClick: vi.fn() }
          ]}
        />
      )

      expect(screen.getByRole('link', { name: 'macOS' })).toHaveAttribute(
        'href',
        '/download_success?os=macOS'
      )
    })
  })

  describe('when the Epic Games option is offered', () => {
    it('should render it as the white inverse action, as source does', () => {
      render(
        <DownloadActions
          epic={{
            label: 'Download on',
            iconLabel: 'Epic Games',
            href: 'https://epic.example',
            onClick: vi.fn()
          }}
        />
      )
    })
  })

  describe('when a store badge is rendered', () => {
    it('should put the badge on the primary action, named on the link', () => {
      render(
        <StoreBadge
          badge="app_store"
          label="Download on the App Store"
          href="https://apps.example"
          onClick={vi.fn()}
        />
      )

      expect(screen.getByRole('link', { name: 'Download on the App Store' })).not.toHaveClass(
        styles.sourceStore!
      )
      expect(screen.getByRole('link', { name: 'Download on the App Store' })).toHaveAttribute(
        'href',
        'https://apps.example'
      )
    })

    it('should come out at the Button large height by construction: 12px padding around a 32px badge', () => {
      const read = (file: string) => readFileSync(path.resolve(import.meta.dirname, file), 'utf8')
      const rule = (css: string, name: string) =>
        css.match(new RegExp(`\\.${name} \\{([^}]*)\\}`))![1]!
      const button = rule(read('../shared/Button/Button.module.css'), 'root')
      const store = read('DownloadActions.module.css')

      expect(button).toContain('padding: var(--dcl-space-12) var(--dcl-space-24)')
      expect(rule(store, 'badge')).toContain('height: var(--dcl-space-32)')
      expect(rule(store, 'store')).not.toMatch(/height|padding/)
    })
  })

  describe('when a landing page opts into the source appearance', () => {
    it('should retain destination, new-tab semantics and action callbacks', async () => {
      const onClick = vi.fn()
      const user = userEvent.setup()
      render(
        <DownloadActions
          appearance="source"
          primary={{
            label: 'Download for',
            os: 'windows',
            osName: 'Windows',
            href: '/download_success?os=Windows',
            onClick
          }}
          epic={{
            label: 'Download on',
            iconLabel: 'Epic Games',
            href: 'https://epic.example',
            onClick
          }}
          alternatives={[
            { icon: 'ios', label: 'iOS', onClick },
            { icon: 'macos', label: 'macOS', href: '/download_success?os=macOS', onClick }
          ]}
        />
      )
      const primary = screen.getByRole('link', { name: 'Download for Windows' })
      expect(primary).toHaveClass(styles.sourceAction!)
      expect(primary).toHaveAttribute('href', '/download_success?os=Windows')
      const epic = screen.getByRole('link', { name: 'Download on Epic Games' })
      expect(epic).toHaveAttribute('target', '_blank')
      expect(epic).toHaveAttribute('rel', expect.stringContaining('noopener'))
      expect(screen.getByRole('link', { name: 'macOS' })).toHaveClass(styles.sourceAlternative!)
      await user.click(screen.getByRole('button', { name: 'iOS' }))
      expect(onClick).toHaveBeenCalledTimes(1)
    })

    it('should keep the store badge named while opting into source geometry', () => {
      render(
        <StoreBadge
          appearance="source"
          badge="app_store"
          label="Download on the App Store"
          href="https://apps.example"
          onClick={vi.fn()}
        />
      )
      expect(screen.getByRole('link', { name: 'Download on the App Store' })).toHaveClass(
        styles.sourceStore!
      )
    })

    it('should use the measured 64px control geometry without changing default badge rules', () => {
      const css = readFileSync(
        path.resolve(import.meta.dirname, 'DownloadActions.module.css'),
        'utf8'
      )
      const rule = (name: string) => css.match(new RegExp(`\\.${name} \\{([^}]*)\\}`))![1]!
      expect(rule('sourceAction')).toContain('width: 304px')
      expect(rule('sourceAction')).toContain('height: var(--dcl-space-64)')
      expect(rule('sourceStore')).toContain('width: calc(100% - var(--dcl-space-32))')
      expect(rule('sourceStore')).toContain('max-width: 345px')
      expect(rule('sourceStore')).toContain('height: var(--dcl-space-64)')
      expect(rule('sourceBadge')).toContain('height: 40px')
      expect(rule('badge')).toContain('height: var(--dcl-space-32)')
    })
  })

  describe('when nothing is offered', () => {
    it('should render no controls', () => {
      render(<DownloadActions />)

      expect(screen.queryByRole('link')).not.toBeInTheDocument()
      expect(screen.queryByRole('button')).not.toBeInTheDocument()
    })
  })
})
