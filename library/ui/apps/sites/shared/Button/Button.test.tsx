import { readFileSync } from 'node:fs'
import path from 'node:path'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { Button } from './Button'

describe('Button', () => {
  describe('when rendered without props', () => {
    it('should render a non-submitting primary button with its label as accessible name', () => {
      render(<Button>Jump in</Button>)

      const button = screen.getByRole('button', { name: 'Jump in' })
      expect(button).toHaveAttribute('type', 'button')
    })
  })

  describe('when a variant is given', () => {
    it('should not leak a variant attribute to the DOM', () => {
      render(<Button variant="tertiary">Jump in</Button>)

      const button = screen.getByRole('button', { name: 'Jump in' })
      expect(button).not.toHaveAttribute('variant')
    })
  })

  describe('when used as a text link inside a sentence', () => {
    it('should stay a native button styled by the text variant', async () => {
      const onClick = vi.fn()
      render(
        <p>
          Already downloaded?{' '}
          <Button variant="text" onClick={onClick}>
            Jump in
          </Button>
        </p>
      )

      const button = screen.getByRole('button', { name: 'Jump in' })
      expect(button).toHaveAttribute('type', 'button')
      await userEvent.click(button)
      expect(onClick).toHaveBeenCalledTimes(1)
    })

    it('should inherit the sentence font and keep its text-link color token and a 24px hit target', () => {
      const css = readFileSync(path.resolve(import.meta.dirname, 'Button.module.css'), 'utf8')
      const body = css.match(/\.text \{([^}]*)\}/)![1]!
      expect(body).toContain('font: inherit')
      expect(body).toContain('min-height: var(--dcl-space-24)')
      expect(body).toContain('var(--dcl-text-link)')
    })
  })

  describe('when the small size is given', () => {
    it('should not leak the size prop to the DOM', () => {
      render(
        <Button variant="tertiary" size="sm">
          Sign In
        </Button>
      )

      const button = screen.getByRole('button', { name: 'Sign In' })
      expect(button).not.toHaveAttribute('size')
    })
  })

  describe('when activated with the keyboard', () => {
    it('should be reachable with Tab and trigger onClick with Enter and Space', async () => {
      const onClick = vi.fn()
      const user = userEvent.setup()
      render(<Button onClick={onClick}>Jump in</Button>)

      await user.tab()
      expect(screen.getByRole('button', { name: 'Jump in' })).toHaveFocus()
      await user.keyboard('{Enter}')
      await user.keyboard(' ')
      expect(onClick).toHaveBeenCalledTimes(2)
    })
  })

  describe('when disabled', () => {
    it('should be natively disabled, skipped by Tab and ignore clicks', async () => {
      const onClick = vi.fn()
      const user = userEvent.setup()
      render(
        <Button disabled onClick={onClick}>
          Jump in
        </Button>
      )

      const button = screen.getByRole('button', { name: 'Jump in' })
      expect(button).toBeDisabled()
      await user.tab()
      expect(button).not.toHaveFocus()
      await user.click(button)
      expect(onClick).not.toHaveBeenCalled()
    })
  })

  describe('when given an href', () => {
    it('should render a link with no type', () => {
      render(
        <Button href="/download_success?os=macOS" variant="secondary">
          Download
        </Button>
      )

      const link = screen.getByRole('link', { name: 'Download' })
      expect(link).toHaveAttribute('href', '/download_success?os=macOS')
      expect(link).not.toHaveAttribute('type')
      expect(screen.queryByRole('button')).not.toBeInTheDocument()
    })

    it('should be reachable with Tab and follow with Enter', async () => {
      const onClick = vi.fn((event: { preventDefault: () => void }) => event.preventDefault())
      const user = userEvent.setup()
      render(
        <Button href="/x" onClick={onClick}>
          Go
        </Button>
      )

      await user.tab()
      expect(screen.getByRole('link', { name: 'Go' })).toHaveFocus()
      await user.keyboard('{Enter}')
      expect(onClick).toHaveBeenCalledTimes(1)
    })

    it('should add noopener and noreferrer to a new-tab link, keeping other rel tokens', () => {
      render(
        <Button href="https://store.example" target="_blank" rel="external">
          Store
        </Button>
      )

      expect(screen.getByRole('link', { name: 'Store' })).toHaveAttribute(
        'rel',
        'external noopener noreferrer'
      )
    })

    it('should leave rel alone on a same-tab link', () => {
      render(<Button href="/x">Go</Button>)

      expect(screen.getByRole('link', { name: 'Go' })).not.toHaveAttribute('rel')
    })
  })

  describe('when icon-only', () => {
    it('should be named by its aria-label and keep the focus-visible style', () => {
      render(
        <Button variant="iconOnly" href="https://apps.example" target="_blank" aria-label="iOS">
          <img src="/ios.svg" alt="" />
        </Button>
      )

      expect(screen.getByRole('link', { name: 'iOS' })).toBeInTheDocument()
      const css = readFileSync(path.resolve(import.meta.dirname, 'Button.module.css'), 'utf8')
      expect(css).toMatch(/\.root:focus-visible\s*{[^}]*outline: 2px solid/)
      expect(css).toMatch(
        /\.iconOnly\s*{[^}]*min-width: var\(--dcl-space-48\)[^}]*min-height: var\(--dcl-space-48\)/
      )
    })

    it('should not type-check without an aria-label', () => {
      // @ts-expect-error: an icon-only button requires aria-label.
      const unlabeled = <Button variant="iconOnly">x</Button>
      // @ts-expect-error: a link has no type attribute.
      const typedLink = <Button href="/x" type="submit" />
      expect([unlabeled, typedLink]).toHaveLength(2)
    })
  })

  describe('when given a className and a submit type', () => {
    it('should honor the submit type', () => {
      render(
        <Button type="submit" className="extra">
          Send
        </Button>
      )

      const button = screen.getByRole('button', { name: 'Send' })
      expect(button).toHaveAttribute('type', 'submit')
    })
  })
  describe('when styled as the inverse variant', () => {
    it('should fill white with dark text from the neutral palette', () => {
      const css = readFileSync(path.resolve(import.meta.dirname, 'Button.module.css'), 'utf8')
      const properties = [...css.matchAll(/(\.inverse[^{]*)\{([^}]*)\}/g)].flatMap(([, , body]) =>
        [...body!.matchAll(/var\((--[a-z0-9-]+)\)/g)].map((m) => m[1])
      )
      expect(properties).toEqual([
        '--dcl-neutrals-white',
        '--dcl-neutrals-charcoal',
        '--dcl-neutrals-pearl',
        '--dcl-neutrals-gray'
      ])
    })
  })

  describe('when styled as the primary variant', () => {
    it('should take its colors only from the Figma semantic action and text variables', () => {
      const css = readFileSync(path.resolve(import.meta.dirname, 'Button.module.css'), 'utf8')
      const primaryRules = [...css.matchAll(/(\.primary[^{]*)\{([^}]*)\}/g)]
      expect(primaryRules.map(([, selector]) => selector!.trim())).toEqual([
        '.primary',
        '.primary:hover:not(:disabled)',
        '.primary:active:not(:disabled)'
      ])
      const properties = primaryRules.flatMap(([, , body]) =>
        [...body!.matchAll(/var\((--[a-z0-9-]+)\)/g)].map((m) => m[1])
      )
      expect(properties).toEqual([
        '--dcl-action-primary',
        '--dcl-text-on-action',
        '--dcl-action-primary-hover',
        '--dcl-action-primary-pressed'
      ])
    })
  })
})
