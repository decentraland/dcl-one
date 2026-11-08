import { cleanup, render } from '@testing-library/react'
import type { ComponentType, ReactElement } from 'react'
import { renderToString } from 'react-dom/server'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { pickView } from '../src/components/figma-view-state'
import { Button } from '../src/generated/components/buttons/Button'
import { NavBar, navBarCases } from '../src/generated/components/nav-bar/NavBar'

afterEach(() => cleanup())

// Untyped callers: plain JS or spread props, which the generated union type cannot reach.
const UntypedNavBar = NavBar as unknown as ComponentType<Record<string, unknown>>
const UntypedButton = Button as unknown as ComponentType<Record<string, unknown>>

// React logs the error a render throws; the assertion is on the throw itself.
function renderError(element: ReactElement): unknown {
  const log = vi.spyOn(console, 'error').mockImplementation(() => undefined)
  try {
    render(element)
    return undefined
  } catch (error) {
    return error
  } finally {
    log.mockRestore()
  }
}

describe('the generated runtime', () => {
  describe('when an untyped caller selects a NavBar combination Figma never drew', () => {
    it('should throw an error naming the component and every axis value', () => {
      expect(String(renderError(<UntypedNavBar balances="credits" />))).toContain(
        'NavBar has no captured view for viewport="largeDesktop", balances="credits", theme="dark", signedIn=false'
      )
    })

    it('should not switch to the default view instead', () => {
      expect(
        String(renderError(<UntypedNavBar signedIn balances="credits" theme="light" />))
      ).toMatch(/NavBar has no captured view .*theme="light"/)
    })

    it('should throw for the same selection spread from an untyped object', () => {
      const props: Record<string, unknown> = { signedIn: false, balances: 'creditsMana' }
      expect(String(renderError(<UntypedNavBar {...props} />))).toContain(
        'NavBar has no captured view for viewport="largeDesktop", balances="creditsMana"'
      )
    })

    it('should throw during server rendering too', () => {
      expect(() => renderToString(<UntypedNavBar balances="credits" />)).toThrow(
        'NavBar has no captured view'
      )
    })
  })

  describe('when the selection is captured', () => {
    it('should render every combination Figma drew', () => {
      expect(navBarCases).toHaveLength(18)
      for (const { props } of navBarCases) {
        const { container, unmount } = render(<UntypedNavBar {...props} />)
        expect(container.firstElementChild, JSON.stringify(props)).not.toBeNull()
        unmount()
      }
    })

    it('should keep falling back from an undrawn interaction-flag combination to a drawn view', () => {
      const { container } = render(<UntypedButton disabled loading />)
      expect(container.firstElementChild).not.toBeNull()
    })
  })

  describe('when an untyped caller passes a suppressed text override', () => {
    it('should keep it out of the DOM while forwarding other attributes', () => {
      const { container } = render(
        <UntypedNavBar label="Explore" label8="Hidden" data-testid="bar" aria-label="Main" />
      )
      const root = container.firstElementChild!
      expect(root.getAttribute('data-testid')).toBe('bar')
      expect(root.getAttribute('aria-label')).toBe('Main')
      expect(root.hasAttribute('label')).toBe(false)
      expect(root.hasAttribute('label8')).toBe(false)
      expect(container.innerHTML).not.toContain('Explore')
      expect(container.innerHTML).not.toContain('Hidden')
    })

    it('should still drop them when the caller also passes an empty or undefined suppression list', () => {
      for (const suppressedProps of [[], undefined]) {
        for (const suppressed of [[], undefined]) {
          const { container, unmount } = render(
            <UntypedNavBar
              label="Explore"
              label8="Hidden"
              suppressedProps={suppressedProps}
              suppressed={suppressed}
            />
          )
          const root = container.firstElementChild!
          expect(root.hasAttribute('label')).toBe(false)
          expect(root.hasAttribute('label8')).toBe(false)
          expect(container.innerHTML).not.toContain('Explore')
          expect(container.innerHTML).not.toContain('Hidden')
          unmount()
        }
      }
    })
  })

  describe('when an untyped caller passes props named like the runtime configuration', () => {
    it('should keep the generated views, component name and selection', () => {
      const log = vi.spyOn(console, 'error').mockImplementation(() => undefined)
      try {
        const { container } = render(
          <UntypedNavBar views={{}} component="Other" select={{}} attributes={{ id: 'x' }} />
        )
        expect(container.querySelector('nav')).not.toBeNull()
        expect(container.innerHTML).toContain('Sign In')
      } finally {
        log.mockRestore()
      }
      expect(String(renderError(<UntypedNavBar component="Other" balances="credits" />))).toContain(
        'NavBar has no captured view'
      )
    })
  })
})

describe('pickView', () => {
  const views = { 'size:md': 'md', 'size:md|selected': 'md selected' }
  const select = (size: string, flags = {}) => ({ values: { size }, booleans: {}, flags })

  it('should drop interaction flags before deciding a selection is uncaptured', () => {
    expect(pickView(views, select('md', { selected: true, disabled: true }), 'Chip')).toBe(
      'md selected'
    )
  })

  it('should throw for an uncaptured axis value instead of returning another view', () => {
    expect(() => pickView(views, select('xl'), 'Chip')).toThrow(
      'Chip has no captured view for size="xl"'
    )
  })

  it('should throw for a component with no views', () => {
    expect(() => pickView({}, select('md'), 'Chip')).toThrow('Chip has no captured view')
  })
})
