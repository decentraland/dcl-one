import { act, cleanup, render } from '@testing-library/react'
import { createRef, type ComponentType, type ReactElement } from 'react'
import { hydrateRoot } from 'react-dom/client'
import { renderToString } from 'react-dom/server'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { FigmaView, type ViewNode } from '../src/components/figma-module-runtime'
import { NavBar } from '../src/generated/components/nav-bar/NavBar'

afterEach(() => cleanup())

const view = (name: string): ViewNode => ({
  kind: 'box',
  path: '',
  name,
  className: `view-${name}`,
  children: [{ kind: 'text', path: '0', name: 'Label', className: 'label', text: name }]
})
const views = {
  'size:small|tone:dark': view('small-dark'),
  'size:large|tone:dark': view('large-dark'),
  'size:small|tone:light': view('small-light')
}
const responsive = { prop: 'size', classes: { small: 'show-small', large: 'show-large' } }
const select = (size: string | undefined, tone = 'dark') => ({
  values: { size, tone },
  booleans: {},
  flags: {}
})

const UntypedNavBar = NavBar as unknown as ComponentType<Record<string, unknown>>

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

// Server-renders, then hydrates the same element, and returns what React reported: console errors and recoverable
// errors (a hydration mismatch is reported through both).
async function hydrate(element: ReactElement) {
  const container = document.createElement('div')
  container.innerHTML = renderToString(element)
  const serverMarkup = container.innerHTML
  document.body.append(container)
  const log = vi.spyOn(console, 'error').mockImplementation(() => undefined)
  const recoverable: unknown[] = []
  const root = await act(async () =>
    hydrateRoot(container, element, { onRecoverableError: (error) => recoverable.push(error) })
  )
  const errors = log.mock.calls
  log.mockRestore()
  const result = { serverMarkup, clientMarkup: container.innerHTML, errors, recoverable }
  act(() => root.unmount())
  container.remove()
  return result
}

describe('FigmaView with a responsive axis', () => {
  describe('when the caller leaves the responsive prop unset', () => {
    it('should render every option view in breakpoint order, each root with its responsive class', () => {
      const { container } = render(
        <FigmaView views={views} select={select(undefined)} slots={{}} responsive={responsive} />
      )

      expect([...container.children].map((root) => root.className)).toEqual([
        expect.stringMatching(/view-small-dark .*show-small$/),
        expect.stringMatching(/view-large-dark .*show-large$/)
      ])
    })

    it('should keep the caller attributes and class on every root', () => {
      const { container } = render(
        <FigmaView
          views={views}
          select={select(undefined)}
          slots={{}}
          responsive={responsive}
          attributes={{ className: 'site-nav', 'data-testid': 'nav' } as never}
        />
      )

      for (const root of container.children) {
        expect(root.getAttribute('data-testid')).toBe('nav')
        expect(root.className).toContain('site-nav')
      }
    })

    it('should produce the same markup on the server and on hydration, with no warning', async () => {
      const result = await hydrate(
        <FigmaView views={views} select={select(undefined)} slots={{}} responsive={responsive} />
      )

      expect(result.errors).toEqual([])
      expect(result.recoverable).toEqual([])
      expect(result.clientMarkup).toBe(result.serverMarkup)
    })

    it('should throw when one option has no view for the other axes, instead of rendering the rest', () => {
      expect(
        String(
          renderError(
            <FigmaView
              views={views}
              select={select(undefined, 'light')}
              slots={{}}
              responsive={responsive}
              component="Demo"
            />
          )
        )
      ).toContain('Demo has no captured view for size="large", tone="light"')
    })

    it('should reject an id, which would be repeated on every view', () => {
      expect(
        String(
          renderError(
            <FigmaView
              views={views}
              select={select(undefined)}
              slots={{}}
              responsive={responsive}
              component="Demo"
              attributes={{ id: 'nav' }}
            />
          )
        )
      ).toContain('Demo: an id or a ref names one element, so pass size to render a single view')
    })

    it('should reject a ref, which could attach to a hidden view', () => {
      expect(
        String(
          renderError(
            <FigmaView
              views={views}
              select={select(undefined)}
              slots={{}}
              responsive={responsive}
              attributes={{ ref: createRef() } as never}
            />
          )
        )
      ).toContain('an id or a ref names one element')
    })
  })

  describe('when the caller sets the responsive prop', () => {
    it('should render exactly the markup of a component without the axis', () => {
      const explicit = renderToString(
        <FigmaView
          views={views}
          select={select('large')}
          slots={{}}
          responsive={responsive}
          attributes={{ id: 'nav' }}
        />
      )
      const plain = renderToString(
        <FigmaView views={views} select={select('large')} slots={{}} attributes={{ id: 'nav' }} />
      )

      expect(explicit).toBe(plain)
      expect(explicit).not.toContain('show-')
    })

    it('should still throw for a value the axis does not have', () => {
      expect(
        String(
          renderError(
            <FigmaView
              views={views}
              select={select('medium')}
              slots={{}}
              responsive={responsive}
              component="Demo"
            />
          )
        )
      ).toContain('Demo has no captured view for size="medium"')
    })
  })

  describe('when the component has no responsive axis', () => {
    it('should keep throwing for an unset axis instead of rendering every view', () => {
      expect(
        String(
          renderError(
            <FigmaView views={views} select={select(undefined)} slots={{}} component="Demo" />
          )
        )
      ).toContain('Demo has no captured view for size=undefined')
    })
  })

  describe('when a caller passes responsive as a prop of a generated component', () => {
    it('should not reach the DOM or replace the generated config', () => {
      const { container } = render(
        <UntypedNavBar viewport="mobile" responsive={{ prop: 'theme', classes: {} }} />
      )

      expect(container.children).toHaveLength(1)
      expect(container.firstElementChild!.hasAttribute('responsive')).toBe(false)
    })
  })
})

describe('the generated NavBar without a viewport', () => {
  it('should server-render one navigation per viewport, narrowest first, and hydrate it with no warning', async () => {
    const result = await hydrate(<NavBar signedIn />)
    const container = document.createElement('div')
    container.innerHTML = result.serverMarkup

    expect(
      [...container.querySelectorAll('nav')].map((nav) =>
        [...nav.classList].filter((name) => name.startsWith('responsive'))
      )
    ).toEqual([['responsiveMobile'], ['responsiveDesktop'], ['responsiveLargeDesktop']])
    expect(result.errors).toEqual([])
    expect(result.recoverable).toEqual([])
    expect(result.clientMarkup).toBe(result.serverMarkup)
  })

  it('should render the single captured view when the viewport is given', () => {
    const { container } = render(<NavBar viewport="desktop" signedIn />)

    expect(container.querySelectorAll('nav')).toHaveLength(1)
    expect(container.innerHTML).not.toContain('responsive')
  })
})
