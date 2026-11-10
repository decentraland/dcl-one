import { readFileSync } from 'node:fs'
import path from 'node:path'
import { fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { LegalPageLayout, type LegalPageLayoutProps } from './LegalPageLayout'

const SIDEBAR: LegalPageLayoutProps['sidebar'] = [
  { href: '/brand', label: 'Logo and Name', icon: 'favorite', active: false },
  { href: '/content', label: 'Content Policy', icon: 'park', active: false },
  { href: '/privacy', label: 'Privacy Policy', icon: 'vpnKey', active: true },
  { href: '/terms', label: 'Terms of Use', icon: 'explore', active: false }
]

function props(overrides: Partial<LegalPageLayoutProps> = {}): LegalPageLayoutProps {
  return {
    title: 'Privacy Policy',
    toc: [
      { id: 'introduction', label: '1. Introduction' },
      { id: 'section-1-1', label: '1.1 Scope', depth: 1 }
    ],
    intro: [{ type: 'p', content: ['Last updated: January 1, 2000'] }],
    sections: [
      {
        id: 'introduction',
        blocks: [
          { type: 'h2', content: ['1. Introduction'] },
          { type: 'h3', id: 'section-1-1', content: ['1.1 Scope'] },
          {
            type: 'p',
            content: [
              'Write to ',
              {
                type: 'link',
                href: 'mailto:jane.doe@example.com',
                children: ['jane.doe@example.com']
              },
              ' or read ',
              {
                type: 'link',
                href: 'https://example.com/terms',
                target: '_blank',
                rel: 'noopener noreferrer',
                children: ['the terms']
              },
              '.'
            ]
          },
          { type: 'ul', items: [['First', { type: 'strong', children: [' item'] }], ['Second']] }
        ]
      }
    ],
    sidebar: SIDEBAR,
    sidebarLabel: 'Legal documents',
    ...overrides
  }
}

describe('LegalPageLayout', () => {
  describe('when it renders a document', () => {
    it('should show one h1, the contents linking to each anchor, and the sections with their ids', () => {
      const { container } = render(<LegalPageLayout {...props()} />)

      expect(screen.getAllByRole('heading', { level: 1 })).toHaveLength(1)
      expect(screen.getByRole('link', { name: '1. Introduction' })).toHaveAttribute(
        'href',
        '#introduction'
      )
      expect(screen.getByRole('link', { name: '1.1 Scope' })).toHaveAttribute(
        'href',
        '#section-1-1'
      )
      expect(container.querySelector('section#introduction')).not.toBeNull()
      expect(screen.getByRole('heading', { level: 3, name: '1.1 Scope' })).toHaveAttribute(
        'id',
        'section-1-1'
      )
    })

    it('should render links exactly as given, with target and rel only when set', () => {
      render(<LegalPageLayout {...props()} />)

      const mail = screen.getByRole('link', { name: 'jane.doe@example.com' })
      expect(mail).toHaveAttribute('href', 'mailto:jane.doe@example.com')
      expect(mail).not.toHaveAttribute('target')
      expect(mail).not.toHaveAttribute('rel')
      const terms = screen.getByRole('link', { name: 'the terms' })
      expect(terms).toHaveAttribute('target', '_blank')
      expect(terms).toHaveAttribute('rel', 'noopener noreferrer')
    })

    it('should indent a nested contents entry by its depth', () => {
      render(<LegalPageLayout {...props()} />)

      const nested = screen.getByRole('link', { name: '1.1 Scope' }).closest('li')!
      expect(nested.style.getPropertyValue('--legal-toc-depth')).toBe('1')
    })
  })

  describe('when the sidebar renders', () => {
    it('should list the documents in order and mark only the active one as the current page', () => {
      render(<LegalPageLayout {...props()} />)

      const links = within(
        screen.getByRole('navigation', { name: 'Legal documents' })
      ).getAllByRole('link')
      expect(links.map((link) => link.textContent)).toEqual([
        'Logo and Name',
        'Content Policy',
        'Privacy Policy',
        'Terms of Use'
      ])
      expect(links.map((link) => link.getAttribute('aria-current'))).toEqual([
        null,
        null,
        'page',
        null
      ])
    })

    it('should be reachable from the keyboard', async () => {
      render(<LegalPageLayout {...props()} />)

      await userEvent.tab()

      expect(screen.getByRole('link', { name: 'Logo and Name' })).toHaveFocus()
    })
  })

  describe('when a sidebar link is clicked plainly', () => {
    it('should hand the navigation to the page instead of loading a document', async () => {
      const onNavigate = vi.fn()
      render(<LegalPageLayout {...props({ onNavigate })} />)

      await userEvent.click(screen.getByRole('link', { name: 'Terms of Use' }))

      expect(onNavigate).toHaveBeenCalledWith('/terms')
    })
  })

  describe('when a sidebar link is clicked with a modifier', () => {
    it('should leave it to the browser, e.g. to open a new tab', () => {
      const onNavigate = vi.fn()
      render(<LegalPageLayout {...props({ onNavigate })} />)

      const event = fireEvent.click(screen.getByRole('link', { name: 'Terms of Use' }), {
        metaKey: true
      })

      expect(onNavigate).not.toHaveBeenCalled()
      expect(event).toBe(true)
    })
  })

  describe('when the document has a call to action', () => {
    it('should render it as a link, not a button', () => {
      render(
        <LegalPageLayout
          {...props({
            toc: [],
            sections: [],
            cta: { href: 'mailto:jane.doe@example.com', label: 'Report a security issue' }
          })}
        />
      )

      expect(screen.getByRole('link', { name: 'Report a security issue' })).toHaveAttribute(
        'href',
        'mailto:jane.doe@example.com'
      )
      expect(screen.queryByRole('button')).not.toBeInTheDocument()
      expect(screen.queryByRole('list')).not.toBeInTheDocument()
    })
  })

  describe('when the document language differs from the page', () => {
    it('should mark the sidebar and document with it', () => {
      const { container } = render(<LegalPageLayout {...props({ lang: 'en' })} />)

      expect(container.firstElementChild?.firstElementChild).toHaveAttribute('lang', 'en')
    })
  })

  describe('when the page is in the document language', () => {
    it('should add no lang attribute', () => {
      const { container } = render(<LegalPageLayout {...props()} />)

      expect(container.querySelector('[lang]')).toBeNull()
    })
  })

  describe('the stylesheet', () => {
    const css = readFileSync(path.join(import.meta.dirname, 'LegalPageLayout.module.css'), 'utf8')

    it('should take colours only from tokens', () => {
      expect(css).not.toMatch(/#[0-9a-f]{3,8}\b|rgba?\(/i)
    })

    it('should take type from the shared text styles and sizes, never a raw font value', () => {
      expect(css).not.toMatch(
        /font-(size|weight):\s*\d|line-height:\s*\d|letter-spacing|font-family/
      )
    })

    // Allowed literals: the layout-width gap (1140, 220), the focus ring (2), the breakpoint (1024)
    // and the icon's fixed size (20).
    it('should space with the shared scale only', () => {
      const rules = css.replace(/\/\*[\s\S]*?\*\//g, '')
      expect(new Set(rules.match(/\d+px/g))).toEqual(
        new Set(['1140px', '2px', '1024px', '220px', '20px'])
      )
    })

    it('should show keyboard focus on every link', () => {
      expect(css).toMatch(
        /\.sidebarLink:focus-visible,\s*\.content a:focus-visible \{\s*outline: 2px solid/
      )
      expect(css).toMatch(/\.cta:focus-visible \{\s*outline: 2px solid/)
    })
  })
})
