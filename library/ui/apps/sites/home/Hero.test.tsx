import { readFileSync } from 'node:fs'
import path from 'node:path'
import { act, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { Hero } from './Hero'

const actions = {
  primary: {
    label: 'Download for',
    os: 'windows' as const,
    osName: 'Windows',
    href: '/download_success?os=Windows',
    onClick: vi.fn()
  }
}

// Live media queries: the motion preference and the wide (desktop) viewport. flip() changes one and
// notifies its listeners.
function setMedia({ reduce = false, wide = true }: { reduce?: boolean; wide?: boolean } = {}) {
  const make = (media: string, matches: boolean) => {
    const listeners = new Set<() => void>()
    const query = {
      matches,
      media,
      addEventListener: (_: string, listener: () => void) => listeners.add(listener),
      removeEventListener: (_: string, listener: () => void) => listeners.delete(listener)
    }
    return {
      query,
      flip(next: boolean) {
        query.matches = next
        for (const listener of listeners) listener()
      }
    }
  }
  const motion = make('(prefers-reduced-motion: reduce)', reduce)
  const viewport = make('(min-width: 1024px)', wide)
  vi.stubGlobal(
    'matchMedia',
    vi.fn((media: string) => (media === viewport.query.media ? viewport.query : motion.query))
  )
  return { motion, viewport }
}

function setReducedMotion(reduce: boolean) {
  return setMedia({ reduce }).motion
}

beforeEach(() => {
  vi.useFakeTimers()
  vi.stubGlobal('requestIdleCallback', undefined)
})

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe('Hero', () => {
  describe('when rendered on desktop', () => {
    it('should show the title as the page heading, a described image and the download actions', () => {
      setReducedMotion(false)
      render(<Hero title="Hang out" imageAlt="A virtual world" actions={actions} />)

      expect(screen.getByRole('heading', { level: 1, name: 'Hang out' })).toBeInTheDocument()
      expect(screen.getByRole('img', { name: 'A virtual world' })).toBeInTheDocument()
      expect(screen.getByRole('link', { name: 'Download for Windows' })).toBeInTheDocument()
    })

    it('should add the muted, hidden ambient video only after an idle moment', () => {
      setReducedMotion(false)
      const { container } = render(<Hero title="Hang out" imageAlt="x" actions={actions} />)

      expect(container.querySelector('video')).toBeNull()
      act(() => vi.advanceTimersByTime(4000))
      const video = container.querySelector('video')!
      expect(video.muted).toBe(true)
      expect(video).toHaveAttribute('aria-hidden', 'true')
    })

    it('should never play the video when the visitor prefers reduced motion', () => {
      setReducedMotion(true)
      const { container } = render(<Hero title="Hang out" imageAlt="x" actions={actions} />)

      act(() => vi.advanceTimersByTime(10_000))
      expect(container.querySelector('video')).toBeNull()
    })
  })

  describe('when given both layouts', () => {
    const mobile = {
      title: 'Hang Out From Anywhere',
      subtitle: 'Now on mobile',
      store: {
        badge: 'app_store' as const,
        label: 'Download on the App Store',
        href: 'https://apps.example',
        onClick: vi.fn()
      }
    }

    it('should render the wide and the narrow layout, each marked for the viewport to pick', () => {
      setMedia()
      render(<Hero title="Hang out" imageAlt="x" actions={actions} mobile={mobile} />)

      const wide = screen.getByRole('heading', { level: 1, name: 'Hang out' }).parentElement!
      const narrow = screen.getByRole('heading', {
        level: 1,
        name: 'Hang Out From Anywhere'
      }).parentElement!
      expect(wide).toContainElement(screen.getByRole('link', { name: 'Download for Windows' }))
      expect(narrow).toContainElement(screen.getByText('Now on mobile'))
      expect(narrow).toContainElement(
        screen.getByRole('link', { name: 'Download on the App Store' })
      )
    })

    it('should not play the video in a narrow viewport, and start it once the viewport widens', () => {
      const { viewport } = setMedia({ wide: false })
      const { container } = render(
        <Hero title="Hang out" imageAlt="x" actions={actions} mobile={mobile} />
      )

      act(() => vi.advanceTimersByTime(10_000))
      expect(container.querySelector('video')).toBeNull()
      act(() => viewport.flip(true))
      act(() => vi.advanceTimersByTime(4000))
      expect(container.querySelector('video')).not.toBeNull()
      act(() => viewport.flip(false))
      expect(container.querySelector('video')).toBeNull()
    })
  })

  describe('when the motion preference changes after mount', () => {
    it('should cancel the pending start when reduced motion turns on before it', () => {
      const motion = setReducedMotion(false)
      const { container } = render(<Hero title="Hang out" imageAlt="x" actions={actions} />)

      act(() => vi.advanceTimersByTime(2000))
      act(() => motion.flip(true))
      act(() => vi.advanceTimersByTime(10_000))
      expect(container.querySelector('video')).toBeNull()
    })

    it('should remove a playing video when reduced motion turns on, and bring it back when it turns off', () => {
      const motion = setReducedMotion(false)
      const { container } = render(<Hero title="Hang out" imageAlt="x" actions={actions} />)

      act(() => vi.advanceTimersByTime(4000))
      expect(container.querySelector('video')).not.toBeNull()
      act(() => motion.flip(true))
      expect(container.querySelector('video')).toBeNull()
      act(() => motion.flip(false))
      act(() => vi.advanceTimersByTime(4000))
      expect(container.querySelector('video')).not.toBeNull()
    })
  })

  describe('when the production hero treatment is applied', () => {
    it('should retain the measured desktop and mobile gradient and padding contracts', () => {
      const css = readFileSync(path.resolve(import.meta.dirname, 'Hero.module.css'), 'utf8')
      const desktop = css.slice(css.indexOf('@media (min-width: 1024px)'))
      expect(css).toContain('--sites-hero-purple: #39055c')
      expect(css).not.toContain('--dcl-brand-purple-dark')
      expect(css).toContain('height: 24.3%')
      expect(css).toMatch(/\.shadeBottom \{[^}]*height: 85%/)
      expect(css).toContain('var(--sites-hero-purple) 30%')
      expect(css).toContain('#39055c00 100%')
      expect(desktop).toMatch(/\.shadeBottom \{[^}]*height: 60%/)
      expect(desktop).toContain(
        'linear-gradient(0deg, var(--sites-hero-purple) 0%, transparent 100%)'
      )
      expect(css).toContain('padding: 0 var(--dcl-space-24) var(--dcl-space-64)')
      expect(css).toContain('max-width: 393px')
      expect(desktop).toContain('padding: 0 0 calc(var(--dcl-space-96) + var(--dcl-space-24))')
      expect(desktop).toContain('margin-bottom: 36px')
    })
  })

  describe('when the viewport sits between breakpoints', () => {
    it('should hide exactly one layout at any width, fractional ones included', () => {
      const css = readFileSync(path.resolve(import.meta.dirname, 'Hero.module.css'), 'utf8')
      // The narrow query is the negation of the wide one, so no width shows both or neither.
      expect(css).toMatch(
        /@media not all and \(min-width: 1024px\) \{\s*\.wideOnly \{\s*display: none;/
      )
      expect(css).toMatch(/@media \(min-width: 1024px\) \{[^@]*\.narrowOnly \{\s*display: none;/)
      expect(css).not.toContain('max-width: 1023px')
    })
  })
})
