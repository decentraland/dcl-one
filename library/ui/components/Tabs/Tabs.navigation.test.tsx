import { fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, type Mock, vi } from 'vitest'
import { Tabs } from './Tabs'

describe('when a captured Tabs item navigates a route', () => {
  describe('and it is the selected item', () => {
    beforeEach(() => {
      render(<Tabs mode="navigation" href="/profile" label="Overview" selected />)
    })

    it('should use a native link with an explicit current-page state', () => {
      expect(screen.getByRole('link')).toHaveAttribute('href', '/profile')
      expect(screen.getByRole('link')).toHaveAttribute('aria-current', 'page')
      expect(screen.queryByRole('tab')).not.toBeInTheDocument()
    })
  })

  describe.each([true, false])('and a category link is selected: %s', (selected) => {
    beforeEach(() => {
      render(
        <Tabs
          mode="navigation"
          href="/blog/platform"
          label="Platform"
          selected={selected}
          current="true"
        />
      )
    })

    it('should mark only the selected link as current', () => {
      const link = screen.getByRole('link', { name: 'Platform' })
      if (selected) expect(link).toHaveAttribute('aria-current', 'true')
      else expect(link).not.toHaveAttribute('aria-current')
    })
  })

  describe('and the caller cancels a modifier click', () => {
    let click: Mock<(event: { preventDefault(): void }) => void>

    beforeEach(() => {
      click = vi.fn((event: { preventDefault(): void }) => event.preventDefault())
      render(<Tabs mode="navigation" href="/profile" label="Overview" onClick={click} />)
    })

    afterEach(() => {
      vi.resetAllMocks()
    })

    it('should preserve modifier clicks and caller cancellation', () => {
      const result = fireEvent.click(screen.getByRole('link'), { ctrlKey: true })
      expect(click).toHaveBeenCalledOnce()
      expect(result).toBe(false)
      expect(screen.getByRole('link')).not.toHaveAttribute('aria-current')
    })
  })

  describe('and it renders its label', () => {
    beforeEach(() => {
      render(<Tabs mode="navigation" href="/profile" label="Overview" />)
    })

    it('should expose one visible label to native navigation and event consumers', () => {
      const link = screen.getByRole('link', { name: 'Overview' })
      expect(link.textContent).toBe('Overview')
    })
  })

  describe('and an untyped caller provides HTML overrides', () => {
    let override: { dangerouslySetInnerHTML: { __html: string } }

    beforeEach(() => {
      override = {
        dangerouslySetInnerHTML: { __html: '<b>Injected content</b>' }
      }
      render(<Tabs mode="navigation" href="/profile" label="Overview" {...override} />)
    })

    it('should preserve its owned content', () => {
      expect(screen.getByRole('link')).toHaveTextContent('Overview')
      expect(screen.queryByText('Injected content')).toBeNull()
    })
  })

  describe('and it sets a link target', () => {
    for (const [target, rel, expected] of [
      ['_blank', undefined, 'noopener noreferrer'],
      ['named-window', 'nofollow', 'nofollow noopener noreferrer'],
      ['_BLANK', 'noopener noopener', 'noopener noreferrer'],
      ['_self', 'nofollow', 'nofollow'],
      [undefined, undefined, undefined]
    ] as const)
      it(`should preserve safe native link relations for ${target ?? 'the current window'}`, () => {
        render(
          <Tabs mode="navigation" href="/profile" label="Overview" target={target} rel={rel} />
        )
        const link = screen.getByRole('link')
        if (expected === undefined) expect(link).not.toHaveAttribute('rel')
        else expect(link).toHaveAttribute('rel', expected)
        expect(link).toHaveAttribute('href', '/profile')
      })
  })

  describe('and no caller handler is provided', () => {
    beforeEach(() => {
      render(<Tabs mode="navigation" href="#profile" label="Overview" />)
    })

    for (const modifier of [{ ctrlKey: true }, { metaKey: true }])
      it(`should preserve native modified navigation for ${Object.keys(modifier)[0]}`, () => {
        expect(fireEvent.click(screen.getByRole('link'), modifier)).toBe(true)
      })
  })
})
