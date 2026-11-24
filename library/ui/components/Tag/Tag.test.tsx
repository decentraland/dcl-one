import { render, screen } from '@testing-library/react'
import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { Tag } from './Tag'
import type { TagVariant } from './Tag'

const marks = (node: HTMLElement) => [...node.querySelectorAll('svg')]

describe('Tag', () => {
  describe('when the variant has a default text', () => {
    it.each([
      ['live', 'Live'],
      ['listening', 'Listening'],
      ['voiceStream', 'Live Voice Stream'],
      ['featured', 'Featured'],
      ['public', 'Public'],
      ['private', 'Private']
    ] as const)('shows the %s tag as "%s"', (variant, text) => {
      render(<Tag variant={variant} data-testid="tag" />)
      expect(screen.getByTestId('tag')).toHaveTextContent(text)
      expect(screen.getByTestId('tag')).toHaveAttribute('data-variant', variant)
    })

    it('shows the caller’s word in place of the default', () => {
      render(<Tag variant="live">En vivo</Tag>)
      expect(screen.getByText('En vivo')).toBeVisible()
      expect(screen.queryByText('Live')).toBeNull()
    })
  })

  describe('when the variant shows what the caller gives', () => {
    it.each(['location', 'role', 'realm', 'updated'] as const)('shows the text of a %s tag', (variant) => {
      render(<Tag variant={variant}>genesis</Tag>)
      expect(screen.getByText('genesis')).toBeVisible()
    })

    it('draws no empty text for a location without one', () => {
      render(<Tag variant="location" data-testid="tag" />)
      expect(screen.getByTestId('tag')).toHaveTextContent('')
    })
  })

  describe('when the variant has a mark', () => {
    it.each(['live', 'streaming', 'voiceStream', 'listening', 'featured', 'location', 'public', 'private', 'updated'] as const)(
      'hides the mark of a %s tag from assistive technology',
      (variant) => {
        render(<Tag variant={variant} data-testid="tag" />)
        const drawn = marks(screen.getByTestId('tag'))
        expect(drawn.length).toBeGreaterThan(0)
        drawn.forEach((mark) => expect(mark).toHaveAttribute('aria-hidden', 'true'))
      }
    )

    it.each(['role', 'realm', 'teen'] as const)('draws no mark for a %s tag', (variant) => {
      render(<Tag variant={variant} data-testid="tag">x</Tag>)
      expect(marks(screen.getByTestId('tag'))).toHaveLength(0)
    })
  })

  describe('when it has a count', () => {
    it.each(['online', 'streaming', 'voiceStream', 'realm'] as const)('shows the count of a %s tag', (variant) => {
      render(<Tag variant={variant} count={24} data-testid="tag" />)
      expect(screen.getByTestId('tag')).toHaveTextContent('24')
    })

    it('reads the count label in place of the number alone', () => {
      render(<Tag variant="online" count={24} countLabel="24 players" data-testid="tag" />)
      expect(screen.getByText('24 players')).toBeInTheDocument()
      expect(screen.getByText('24')).toHaveAttribute('aria-hidden', 'true')
    })

    it('shows only the count for online and streaming, whatever the text', () => {
      render(
        <Tag variant="online" count={3} data-testid="tag">
          ignored
        </Tag>
      )
      expect(screen.getByTestId('tag')).toHaveTextContent('3')
      expect(screen.queryByText('ignored')).toBeNull()
    })

    it.each(['live', 'featured', 'location', 'role', 'teen', 'updated'] as TagVariant[])('draws no count for a %s tag', (variant) => {
      render(<Tag variant={variant} count={24} data-testid="tag" />)
      expect(screen.getByTestId('tag')).not.toHaveTextContent('24')
    })

    it.each([['online', 'Online'], ['streaming', 'Streaming']] as const)(
      'shows the status text for %s without a count',
      (variant, label) => {
        const { rerender } = render(<Tag variant={variant} />)
        expect(screen.getByText(label)).toBeVisible()
        rerender(<Tag variant={variant}>En vivo</Tag>)
        expect(screen.getByText('En vivo')).toBeVisible()
      }
    )

    it('draws no count mark without a count', () => {
      render(<Tag variant="realm" data-testid="tag">america-amber</Tag>)
      expect(marks(screen.getByTestId('tag'))).toHaveLength(0)
    })
  })

  describe('when it is an age rating', () => {
    it.each([
      ['teen', 'T', 'Teen'],
      ['adult', 'A', 'Adult'],
      ['restricted', 'R', 'Restricted']
    ] as const)('shows the letter of %s and reads its word', (variant, letter, word) => {
      render(<Tag variant={variant} />)
      expect(screen.getByRole('img', { name: word })).toHaveTextContent(letter)
    })

    it('reads the referenced label for a rating', () => {
      render(<><span id="rating">Adultos</span><Tag variant="adult" aria-labelledby="rating" /></>)
      expect(screen.getByRole('img', { name: 'Adultos' })).toHaveTextContent('A')
    })

    it('reads the caller’s name when one is given', () => {
      render(<Tag variant="teen" aria-label="Adolescentes" />)
      expect(screen.getByRole('img', { name: 'Adolescentes' })).toBeInTheDocument()
    })
  })

  describe('when the caller passes native attributes', () => {
    it('keeps them and adds its class to the caller’s', () => {
      render(<Tag variant="featured" className="mine" title="Featured place" data-testid="tag" />)
      expect(screen.getByTestId('tag')).toHaveClass('mine')
      expect(screen.getByTestId('tag')).toHaveAttribute('title', 'Featured place')
    })
  })

  it('renders on the server without a browser', () => {
    expect(renderToString(<Tag variant="location">0,0</Tag>)).toContain('0,0')
  })
})

describe('when a community supplies a role label', () => {
  it('shows the supplied role as text', () => {
    render(<Tag variant="role">Moderator</Tag>)
    expect(screen.getByText('Moderator')).toBeVisible()
  })

  it('keeps the role text in the emphasized appearance', () => {
    render(<Tag variant="role" appearance="emphasized">Owner</Tag>)
    expect(screen.getByText('Owner')).toHaveAttribute('data-appearance', 'emphasized')
  })
})
