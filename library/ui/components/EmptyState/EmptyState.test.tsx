import '@testing-library/jest-dom/vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { EmptyState } from './EmptyState'

describe('EmptyState', () => {
  describe('when it has only a title', () => {
    it('writes the title and nothing else', () => {
      const { container } = render(<EmptyState title="Nothing here" />)
      const root = container.firstElementChild!
      expect(screen.getByText('Nothing here').tagName).toBe('P')
      expect(root.children).toHaveLength(1)
      expect(root).not.toHaveAttribute('data-variant')
      expect(root).not.toHaveAttribute('data-tone')
      expect(root).not.toHaveAttribute('style')
    })
  })

  describe('when a Shop page supplies its own heading and region', () => {
    it('exposes the heading level and forwards the region attributes', () => {
      render(
        <EmptyState
          appearance="shop"
          titleAs="h2"
          variant="inline"
          title="No listings"
          role="region"
          aria-label="Listings"
          data-variant="custom"
        />
      )
      const region = screen.getByRole('region', { name: 'Listings' })
      expect(region).toHaveAttribute('data-variant', 'custom')
      expect(region).toContainElement(
        screen.getByRole('heading', { level: 2, name: 'No listings' })
      )
    })
  })

  describe('when it has an icon and a subtitle', () => {
    it('hides the icon from assistive technology and writes the subtitle after the title', () => {
      render(
        <EmptyState
          variant="card"
          icon={<i data-testid="icon" />}
          title="No friends yet"
          subtitle="Add the people you meet."
          className="mine"
          style={{ margin: 4 }}
        />
      )
      expect(screen.getByTestId('icon').parentElement).toHaveAttribute('aria-hidden', 'true')
      const title = screen.getByText('No friends yet')
      expect(title.nextElementSibling).toHaveTextContent('Add the people you meet.')
      const root = title.parentElement!
      expect(root).toHaveAttribute('data-variant', 'card')
      expect(root).toHaveClass('mine')
      expect(root).toHaveStyle({ margin: '4px' })
    })
  })

  describe('when it has a list of actions', () => {
    it('renders a button for each and calls its handler', () => {
      const retry = vi.fn()
      render(
        <EmptyState
          title="Couldn't load"
          actions={[
            { label: 'Retry', onClick: retry },
            { label: 'Later', variant: 'ghost' }
          ]}
        />
      )
      const button = screen.getByRole('button', { name: 'Retry' })
      expect(button).toHaveAttribute('type', 'button')
      expect(button).toHaveAttribute('data-variant', 'solid')
      expect(screen.getByRole('button', { name: 'Later' })).toHaveAttribute('data-variant', 'ghost')
      fireEvent.click(button)
      expect(retry).toHaveBeenCalledTimes(1)
    })

    it('renders a link for an action with a destination, with its icon hidden', () => {
      render(
        <EmptyState
          title="Nothing on sale"
          actions={[
            {
              label: 'Open the Shop',
              href: '/shop',
              variant: 'outline',
              icon: <i data-testid="icon" />
            }
          ]}
        />
      )
      const link = screen.getByRole('link', { name: 'Open the Shop' })
      expect(link).toHaveAttribute('href', '/shop')
      expect(link).toHaveAttribute('data-variant', 'outline')
      expect(screen.getByTestId('icon').parentElement).toHaveAttribute('aria-hidden', 'true')
    })

    it('keeps a navigation handler on a link action', async () => {
      const onClick = vi.fn()
      const user = userEvent.setup()
      render(<EmptyState title="Shop" actions={[{ label: 'Browse', href: '#browse', onClick }]} />)
      await user.tab()
      expect(screen.getByRole('link', { name: 'Browse' })).toHaveFocus()
      await user.keyboard('{Enter}')
      expect(onClick).toHaveBeenCalledOnce()
    })

    it('renders no actions row for an empty list', () => {
      const { container } = render(<EmptyState title="Nothing here" actions={[]} />)
      expect(container.firstElementChild!.children).toHaveLength(1)
    })
  })

  describe('when the caller passes its own actions', () => {
    it('renders them as they are', () => {
      render(<EmptyState title="Nothing here" actions={<a href="/help">Help</a>} />)
      expect(screen.getByRole('link', { name: 'Help' })).toHaveAttribute('href', '/help')
    })
  })

  describe('when it reports an error', () => {
    it('marks the tone and moves the actions 40px away', () => {
      const { container } = render(<EmptyState tone="error" title="Couldn't load" />)
      const root = container.firstElementChild as HTMLElement
      expect(root).toHaveAttribute('data-tone', 'error')
      expect(root.style.getPropertyValue('--es-actions-gap')).toBe('40px')
    })

    it('takes the gap it is given, as pixels or as a length', () => {
      const { container, rerender } = render(
        <EmptyState tone="error" title="Couldn't load" actionsGap={12} />
      )
      const root = container.firstElementChild as HTMLElement
      expect(root.style.getPropertyValue('--es-actions-gap')).toBe('12px')
      rerender(<EmptyState title="Couldn't load" actionsGap="1rem" />)
      expect(root.style.getPropertyValue('--es-actions-gap')).toBe('1rem')
    })
  })
})
