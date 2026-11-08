import { cleanup, render } from '@testing-library/react'
import type { ComponentType, ReactElement } from 'react'
import { renderToString } from 'react-dom/server'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { FigmaView, type ViewNode } from '../src/components/figma-module-runtime'
import { pickView } from '../src/components/figma-view-state'
import * as generated from '../src/generated/components'
import { Button, buttonCases, buttonViews } from '../src/generated/components/buttons/Button'
import { tabsCases, tabsViews } from '../src/generated/components/tabs-bar/Tabs'
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

  it('should throw for a boolean axis that is not a boolean', () => {
    expect(() =>
      pickView(
        { 'size:md': 'md', 'size:md|signedIn': 'in' },
        { values: { size: 'md' }, booleans: { signedIn: 'yes' as never }, flags: {} },
        'Chip'
      )
    ).toThrow('Chip has no captured view for size="md", signedIn="yes"')
  })

  it('should throw for a component with no views', () => {
    expect(() => pickView({}, select('md'), 'Chip')).toThrow('Chip has no captured view')
  })
})

describe('text roles in the runtime', () => {
  const bell: ViewNode = {
    kind: 'box',
    path: '',
    name: 'root',
    className: '',
    children: [
      {
        kind: 'box',
        path: '0',
        name: 'Bell',
        className: '',
        semantic: { role: 'action', label: 'Notifications', onActivate: 'onNotifications' },
        children: [
          {
            kind: 'text',
            path: '0.0',
            name: '12',
            className: '',
            text: '12',
            slot: 'notificationCount'
          }
        ]
      },
      { kind: 'text', path: '1', name: 'Sign', className: '', text: 'Sign In', slot: 'signInLabel' }
    ]
  }
  const view = (slots: Record<string, unknown>, label?: unknown) => (
    <FigmaView
      views={{ default: bell }}
      component="Bell"
      select={{ values: {}, booleans: {}, flags: {} }}
      slots={slots as Record<string, never>}
      counts={{ notificationCount: label as string | undefined }}
      semanticProps={{ onNotifications: () => undefined }}
    />
  )

  describe('when a count and its label are given', () => {
    it('should show the count and name the action with the label', () => {
      const { container } = render(
        view({ notificationCount: 3, signInLabel: 'Entrar' }, '3 notificaciones')
      )
      const button = container.querySelector('button')!
      expect(button.getAttribute('aria-label')).toBe('3 notificaciones')
      expect(button.textContent).toBe('3')
      expect(container.textContent).toContain('Entrar')
    })

    it('should accept zero', () => {
      const { container } = render(view({ notificationCount: 0 }, 'No notifications'))
      expect(container.querySelector('button')!.getAttribute('aria-label')).toBe('No notifications')
    })
  })

  describe('when no count is given', () => {
    it('should keep the captured text and the configured accessible name', () => {
      const { container } = render(view({}, 'ignored'))
      const button = container.querySelector('button')!
      expect(button.getAttribute('aria-label')).toBe('Notifications')
      expect(button.textContent).toBe('12')
    })
  })

  describe('when an untyped caller passes an invalid count', () => {
    it.each([[-1], [1.5], ['3'], [Number.NaN]])(
      'should throw for %s, on the client and the server',
      (count) => {
        const message = `Bell: notificationCount must be a non-negative integer, got ${JSON.stringify(count)}`
        expect(String(renderError(view({ notificationCount: count }, 'label')))).toContain(message)
        expect(() => renderToString(view({ notificationCount: count }, 'label'))).toThrow(message)
      }
    )
  })

  describe('when a count comes without a usable label', () => {
    it.each([[undefined], [''], ['  ']])('should throw for %j', (label) => {
      expect(String(renderError(view({ notificationCount: 3 }, label)))).toContain(
        'Bell: notificationCount needs a non-empty accessible label'
      )
    })
  })
})

describe('caller isolation through real rendering', () => {
  describe('when an untyped caller passes every runtime parameter name', () => {
    it('should keep the generated element, view, slots and component name', () => {
      const log = vi.spyOn(console, 'error').mockImplementation(() => undefined)
      try {
        const { container } = render(
          <UntypedNavBar
            as="button"
            views={{}}
            select={{ values: { viewport: 'mobile' }, booleans: {}, flags: {} }}
            slots={{ label: 'Injected' }}
            component="Other"
            attributes={{ id: 'injected' }}
            semanticProps={{ logoHref: 'https://example.com' }}
          />
        )
        const root = container.firstElementChild!
        expect(root.tagName).toBe('NAV')
        expect(container.textContent).toContain('Sign In')
        expect(container.textContent).not.toContain('Injected')
        expect(container.querySelector('a[href="https://example.com"]')).toBeNull()
      } finally {
        log.mockRestore()
      }
    })
  })

  describe('when a caller passes standard element props', () => {
    it('should forward className, style, events, aria, data and ref to the root', () => {
      const onClick = vi.fn()
      let element: HTMLElement | null = null
      const { container } = render(
        <UntypedNavBar
          className="consumer"
          style={{ outline: '1px solid red' }}
          onClick={onClick}
          aria-describedby="help"
          data-testid="bar"
          ref={(node: HTMLElement | null) => {
            element = node
          }}
        />
      )
      const root = container.firstElementChild as HTMLElement
      expect(root.classList.contains('consumer')).toBe(true)
      expect(root.style.outline).toBe('1px solid red')
      expect(root.getAttribute('aria-describedby')).toBe('help')
      expect(root.getAttribute('data-testid')).toBe('bar')
      expect(element).toBe(root)
      root.click()
      expect(onClick).toHaveBeenCalledTimes(1)
    })

    it('should disable a generated button and block its click', () => {
      const onClick = vi.fn()
      const { container } = render(<UntypedButton disabled onClick={onClick} />)
      const button = container.querySelector('button')!
      expect(button.disabled).toBe(true)
      button.click()
      expect(onClick).not.toHaveBeenCalled()
    })

    it('should render the same forwarded attributes on the server', () => {
      const html = renderToString(
        <UntypedNavBar className="consumer" data-testid="bar" aria-describedby="help" />
      )
      expect(html).toMatch(/^<nav[^>]*class="[^"]*consumer/)
      expect(html).toContain('data-testid="bar"')
      expect(html).toContain('aria-describedby="help"')
    })
  })
})

describe('pickView over generated views', () => {
  const FLAGS = ['selected', 'disabled', 'loading']
  const selectionOf = (props: Record<string, unknown>) => ({
    values: Object.fromEntries(
      Object.entries(props).filter(
        ([name, value]) => !FLAGS.includes(name) && typeof value === 'string'
      )
    ) as Record<string, string>,
    booleans: Object.fromEntries(
      Object.entries(props).filter(
        ([name, value]) => !FLAGS.includes(name) && typeof value === 'boolean'
      )
    ) as Record<string, boolean>,
    flags: Object.fromEntries(
      FLAGS.filter((flag) => flag in props).map((flag) => [flag, props[flag]])
    ) as {
      selected?: boolean
      disabled?: boolean
      loading?: boolean
    }
  })
  const base = { variant: 'primary', size: 'md', viewport: 'desktop' }

  it('should return the exact view object of every catalog case', () => {
    for (const { key, props } of buttonCases)
      expect(pickView(buttonViews, selectionOf(props), 'Button')).toBe(buttonViews[key])
    for (const { key, props } of tabsCases)
      expect(pickView(tabsViews, selectionOf(props), 'Tabs')).toBe(tabsViews[key])
  })

  it('should treat omitted, undefined and all-false flags as the base view', () => {
    const expected = buttonViews['variant:primary|size:md|viewport:desktop']
    expect(expected).toBeDefined()
    for (const flags of [
      {},
      { disabled: undefined, loading: undefined },
      { disabled: false, loading: false }
    ])
      expect(pickView(buttonViews, { values: base, booleans: {}, flags }, 'Button')).toBe(expected)
  })

  it('should drop flags from the end, in selected, disabled, loading order', () => {
    expect(buttonViews['variant:primary|size:md|viewport:desktop|disabled|loading']).toBeUndefined()
    expect(
      pickView(
        buttonViews,
        { values: base, booleans: {}, flags: { disabled: true, loading: true } },
        'Button'
      )
    ).toBe(buttonViews['variant:primary|size:md|viewport:desktop|disabled'])
    expect(
      pickView(buttonViews, { values: base, booleans: {}, flags: { loading: true } }, 'Button')
    ).toBe(buttonViews['variant:primary|size:md|viewport:desktop|loading'])
    expect(
      pickView(
        tabsViews,
        { values: { device: 'mobile' }, booleans: {}, flags: { selected: true, disabled: true } },
        'Tabs'
      )
    ).toBe(tabsViews['device:mobile|selected'])
  })

  it('should throw for an unknown axis value and for an axis the component does not have', () => {
    expect(() =>
      pickView(buttonViews, { values: { ...base, size: 'xxl' }, booleans: {}, flags: {} }, 'Button')
    ).toThrow('Button has no captured view for variant="primary", size="xxl", viewport="desktop"')
    expect(() =>
      pickView(
        buttonViews,
        { values: { ...base, tone: 'warm' }, booleans: {}, flags: {} },
        'Button'
      )
    ).toThrow('Button has no captured view for')
  })

  it('should throw for a component with no views even when only flags are set', () => {
    expect(() =>
      pickView({}, { values: {}, booleans: {}, flags: { disabled: true } }, 'Empty')
    ).toThrow('Empty has no captured view for no variant axes')
  })

  // Per component: every generated component with interaction flags, every catalog case drawn without them.
  it('should resolve omitted, undefined and all-false flags to the exact case view in every generated component', () => {
    const exports = generated as Record<string, unknown>
    let checked = 0
    for (const [name, value] of Object.entries(exports)) {
      if (!name.endsWith('Views')) continue
      const views = value as Record<string, unknown>
      const cases = (exports[`${name.slice(0, -5)}Cases`] ?? []) as {
        key: string
        props: Record<string, unknown>
      }[]
      const flags = FLAGS.filter((flag) => cases.some((item) => flag in item.props))
      if (!flags.length) continue
      for (const item of cases.filter((entry) =>
        flags.every((flag) => entry.props[flag] !== true)
      )) {
        const selection = selectionOf(item.props)
        for (const variant of [
          {},
          Object.fromEntries(flags.map((flag) => [flag, undefined])),
          Object.fromEntries(flags.map((flag) => [flag, false]))
        ]) {
          expect(
            pickView(views, { ...selection, flags: variant }, name),
            `${name} ${item.key}`
          ).toBe(views[item.key])
          checked += 1
        }
      }
    }
    expect(checked).toBeGreaterThan(0)
  })
})
