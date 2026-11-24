import { readFileSync } from 'node:fs'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { CommunityCard, type CommunityCardLinkProps } from './CommunityCard'

const base = {
  name: 'Builders Guild',
  href: '/landings/community-detail?id=community-1',
  image: 'https://cdn.example/community-1.png',
  ownerName: 'Jane',
  membersLabel: '42 members',
  description: 'We build things'
}

describe('CommunityCard', () => {
  describe('when rendered with a community', () => {
    it('links the name to the community page', () => {
      render(<CommunityCard {...base} />)

      expect(screen.getByRole('link', { name: 'Builders Guild' })).toHaveAttribute('href', base.href)
    })

    it('shows the owner, the privacy, the member count and the description', () => {
      render(<CommunityCard {...base} />)

      expect(screen.getByText('Jane')).toBeInTheDocument()
      expect(screen.getByText('Public')).toBeInTheDocument()
      expect(screen.getByText('42 members')).toBeInTheDocument()
      expect(screen.getByText('We build things')).toBeInTheDocument()
    })

    it('draws the cover from the image', () => {
      const { container } = render(<CommunityCard {...base} />)

      expect(container.querySelector('img')).toHaveAttribute('src', base.image)
    })

    it('hides the VIEW call to action from assistive technology, since the name link already opens the page', () => {
      render(<CommunityCard {...base} />)

      expect(screen.getByText('View')).toHaveAttribute('aria-hidden', 'true')
      expect(screen.getAllByRole('link')).toHaveLength(1)
    })
  })

  describe('when the community is private', () => {
    it('shows the private label', () => {
      render(<CommunityCard {...base} privacy="private" />)

      expect(screen.getByText('Private')).toBeInTheDocument()
      expect(screen.queryByText('Public')).not.toBeInTheDocument()
    })
  })

  describe('when optional details are missing', () => {
    it('renders no owner, member count or description', () => {
      const { container } = render(<CommunityCard name="Bare" href="/c" ownerName={null} description="   " />)

      // Only the privacy row remains: a blank description is no description.
      const paragraphs = container.querySelectorAll('p')
      expect(paragraphs).toHaveLength(1)
      expect(paragraphs[0]).toHaveTextContent('Public')
    })

    it('renders the name as text without a page to open', () => {
      render(<CommunityCard name="No page" />)

      expect(screen.getByRole('heading', { name: 'No page' })).toBeInTheDocument()
      expect(screen.queryByRole('link')).not.toBeInTheDocument()
    })

    it('shows the image placeholder without a cover', () => {
      const { container } = render(<CommunityCard {...base} image={null} />)

      expect(container.querySelector('img')).toBeNull()
    })
  })

  describe('when the name link is clicked', () => {
    it('calls onOpen with the click', () => {
      const onOpen = vi.fn((event) => event.preventDefault())
      render(<CommunityCard {...base} onOpen={onOpen} />)

      fireEvent.click(screen.getByRole('link', { name: 'Builders Guild' }))

      expect(onOpen).toHaveBeenCalledTimes(1)
    })
  })

  describe('when a link component is given', () => {
    it('renders the name through it', () => {
      const RouterLink = (props: CommunityCardLinkProps) => <a data-router="yes" {...props} />
      render(<CommunityCard {...base} linkComponent={RouterLink} />)

      expect(screen.getByRole('link', { name: 'Builders Guild' })).toHaveAttribute('data-router', 'yes')
    })
  })

  describe('when labels are given', () => {
    it('uses them for the privacy and the call to action', () => {
      render(<CommunityCard {...base} labels={{ public: 'Pública', private: 'Privada', view: 'Ver' }} />)

      expect(screen.getByText('Pública')).toBeInTheDocument()
      expect(screen.getByText('Ver')).toBeInTheDocument()
    })
  })

  describe('when loading', () => {
    it('renders a busy placeholder named for assistive technology instead of the card', () => {
      render(<CommunityCard loading loadingLabel="Loading communities" name="Hidden" />)

      const placeholder = screen.getByRole('progressbar', { name: 'Loading communities' })
      expect(placeholder).toHaveAttribute('aria-busy', 'true')
      expect(screen.queryByText('Hidden')).not.toBeInTheDocument()
    })
  })

  describe('when the card styles reveal the details', () => {
    const file = './CommunityCard.module.css'
    const css = readFileSync(new URL(file, import.meta.url), 'utf8')
    const hoverQuery = css.slice(css.indexOf('@media (hover: hover)'))
    const outsideHover = css.slice(0, css.indexOf('@media (hover: hover)'))
    /** Every rule whose selector ends on the class and is a reveal state (hover or keyboard focus). */
    const revealRules = (source: string, target: string) =>
      [...source.matchAll(/([^{}]+)\{([^{}]*)\}/g)]
        .map(([, selector = '', body = '']) => ({ selector: selector.trim(), body }))
        .filter(({ selector }) => /:hover|:focus-visible/.test(selector) && selector.endsWith(target))
        .map(({ body }) => body)
    const declaration = (body: string, property: string) =>
      new RegExp(`(?:^|[;\\s])${property}:\\s*([^;]+);`).exec(body)?.[1]?.trim()
    const restRule = (target: string) => new RegExp(`\\n  \\${target} \\{([^}]*)\\}`).exec(css)?.[1] ?? ''

    it('reveals them on keyboard focus outside the hover-only query', () => {
      expect(revealRules(outsideHover, '.description')).toHaveLength(1)
      expect(revealRules(outsideHover, '.media')).toHaveLength(1)
    })

    it('does not reveal them on a pointer focus, which is not focus-visible', () => {
      expect(css).not.toMatch(/:focus-within/)
    })

    it('keeps the hover animation for devices that hover', () => {
      expect(revealRules(hoverQuery, '.description')).toHaveLength(1)
      expect(revealRules(hoverQuery, '.media')).toHaveLength(1)
      expect(declaration(restRule('.media'), 'transition')).toBe('height 200ms')
      expect(declaration(restRule('.description'), 'transition')).toMatch(/^height 200ms,\s*opacity 200ms$/)
    })

    it('keeps the card the same height: the cover gives up exactly the room the description takes', () => {
      expect(declaration(restRule('.card'), '--reveal')).toBe('19.286cqw')
      expect(declaration(restRule('.description'), 'height')).toBe('0')
      const rest = declaration(restRule('.media'), 'height') ?? ''
      expect(rest).toBe('92.857cqw')
      for (const body of revealRules(css, '.media')) {
        expect(declaration(body, 'height')).toBe(`calc(${rest} - var(--reveal))`)
      }
      for (const body of revealRules(css, '.description')) {
        expect(declaration(body, 'height')).toBe('var(--reveal)')
        expect(declaration(body, 'max-height')).toBeUndefined()
      }
    })

    it('changes nothing else in the flow on reveal', () => {
      for (const target of ['.cta', '.body', '.name', '.owner', '.meta', '.card']) {
        for (const body of revealRules(css, target)) {
          expect(body).not.toMatch(/(?:^|[;\s])(?:height|max-height|min-height|padding|margin|gap|font-size|line-height):/)
        }
      }
    })
  })
})
