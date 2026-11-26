import { cleanup, render } from '@testing-library/react'
import type { ComponentType, ReactElement } from 'react'
import { renderToString } from 'react-dom/server'
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  vi,
  type Mock,
  type MockInstance
} from 'vitest'
import { FigmaView, type ViewNode } from '../src/components/figma-module-runtime'
import { pickView } from '../src/components/figma-view-state'
import * as generated from '../src/generated/components'
import { Button, buttonCases, buttonViews } from '../src/generated/components/buttons/Button'
import { tabsCases, tabsViews } from '../src/generated/components/tabs-bar/Tabs'
import { NavBar, navBarCases } from '../src/generated/components/nav-bar/NavBar'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

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
      expect(
        String(renderError(<UntypedNavBar viewport="largeDesktop" balances="credits" />))
      ).toContain(
        'NavBar has no captured view for viewport="largeDesktop", balances="credits", theme="dark", signedIn=false'
      )
    })

    it('should not switch to the default view instead', () => {
      expect(
        String(renderError(<UntypedNavBar signedIn balances="credits" theme="light" />))
      ).toMatch(/NavBar has no captured view .*theme="light"/)
    })

    it('should throw during server rendering too', () => {
      expect(() => renderToString(<UntypedNavBar balances="credits" />)).toThrow(
        'NavBar has no captured view'
      )
    })

    describe('and the selection is spread from an untyped object', () => {
      let props: Record<string, unknown>

      beforeEach(() => {
        props = {
          viewport: 'largeDesktop',
          signedIn: false,
          balances: 'creditsMana'
        }
      })

      it('should throw for the same selection', () => {
        expect(String(renderError(<UntypedNavBar {...props} />))).toContain(
          'NavBar has no captured view for viewport="largeDesktop", balances="creditsMana"'
        )
      })
    })
  })

  describe('when the selection is captured', () => {
    it('should capture 18 NavBar combinations', () => {
      expect(navBarCases).toHaveLength(18)
    })

    it('should render every combination Figma drew', () => {
      for (const { props } of navBarCases) {
        const { container, unmount } = render(<UntypedNavBar {...props} />)
        expect(container.firstElementChild, JSON.stringify(props)).not.toBeNull()
        unmount()
      }
    })

    describe('and an interaction-flag combination is undrawn', () => {
      it('should keep falling back to a drawn view', () => {
        const { container } = render(<UntypedButton disabled loading />)
        expect(container.firstElementChild).not.toBeNull()
      })
    })
  })

  describe('when an untyped caller passes a suppressed text override', () => {
    let container: HTMLElement

    describe('and other attributes alongside it', () => {
      beforeEach(() => {
        ;({ container } = render(
          <UntypedNavBar label="Explore" label8="Hidden" data-testid="bar" aria-label="Main" />
        ))
      })

      it('should forward the other attributes', () => {
        const root = container.firstElementChild!
        expect(root.getAttribute('data-testid')).toBe('bar')
        expect(root.getAttribute('aria-label')).toBe('Main')
      })

      it('should keep the overrides out of the root attributes', () => {
        const root = container.firstElementChild!
        expect(root.hasAttribute('label')).toBe(false)
        expect(root.hasAttribute('label8')).toBe(false)
      })

      it('should keep the override text out of the DOM', () => {
        expect(container.innerHTML).not.toContain('Explore')
        expect(container.innerHTML).not.toContain('Hidden')
      })
    })

    describe.each([
      [[], []],
      [[], undefined],
      [undefined, []],
      [undefined, undefined]
    ])(
      'and the caller also passes suppressedProps %j and suppressed %j',
      (suppressedProps, suppressed) => {
        beforeEach(() => {
          ;({ container } = render(
            <UntypedNavBar
              label="Explore"
              label8="Hidden"
              suppressedProps={suppressedProps}
              suppressed={suppressed}
            />
          ))
        })

        it('should still drop the overrides from the root attributes', () => {
          const root = container.firstElementChild!
          expect(root.hasAttribute('label')).toBe(false)
          expect(root.hasAttribute('label8')).toBe(false)
        })

        it('should still keep the override text out of the DOM', () => {
          expect(container.innerHTML).not.toContain('Explore')
          expect(container.innerHTML).not.toContain('Hidden')
        })
      }
    )
  })

  describe('when an untyped caller passes props named like the runtime configuration', () => {
    describe('and the selection is captured', () => {
      let container: HTMLElement

      beforeEach(() => {
        vi.spyOn(console, 'error').mockImplementation(() => undefined)
        ;({ container } = render(
          <UntypedNavBar views={{}} component="Other" select={{}} attributes={{ id: 'x' }} />
        ))
      })

      it('should keep the generated views', () => {
        expect(container.querySelector('nav')).not.toBeNull()
      })

      it('should keep the generated selection', () => {
        expect(container.innerHTML).toContain('Sign In')
      })
    })

    describe('and the selection is undrawn', () => {
      it('should keep the generated component name in the error', () => {
        expect(
          String(renderError(<UntypedNavBar component="Other" balances="credits" />))
        ).toContain('NavBar has no captured view')
      })
    })
  })
})

describe('pickView', () => {
  let views: Record<string, string>
  const select = (size: string, flags = {}) => ({ values: { size }, booleans: {}, flags })

  beforeEach(() => {
    views = { 'size:md': 'md', 'size:md|selected': 'md selected' }
  })

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
  let bell: ViewNode
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

  beforeEach(() => {
    bell = {
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
        {
          kind: 'text',
          path: '1',
          name: 'Sign',
          className: '',
          text: 'Sign In',
          slot: 'signInLabel'
        }
      ]
    }
  })

  describe('when a count and its label are given', () => {
    let container: HTMLElement

    describe('and the count is positive', () => {
      beforeEach(() => {
        ;({ container } = render(
          view({ notificationCount: 3, signInLabel: 'Entrar' }, '3 notificaciones')
        ))
      })

      it('should name the action with the label', () => {
        expect(container.querySelector('button')!.getAttribute('aria-label')).toBe(
          '3 notificaciones'
        )
      })

      it('should show the count', () => {
        expect(container.querySelector('button')!.textContent).toBe('3')
      })

      it('should fill the other text slots', () => {
        expect(container.textContent).toContain('Entrar')
      })
    })

    describe('and the count is zero', () => {
      beforeEach(() => {
        ;({ container } = render(view({ notificationCount: 0 }, 'No notifications')))
      })

      it('should accept it and name the action with the label', () => {
        expect(container.querySelector('button')!.getAttribute('aria-label')).toBe(
          'No notifications'
        )
      })
    })
  })

  describe('when no count is given', () => {
    let container: HTMLElement

    beforeEach(() => {
      ;({ container } = render(view({}, 'ignored')))
    })

    it('should keep the configured accessible name', () => {
      expect(container.querySelector('button')!.getAttribute('aria-label')).toBe('Notifications')
    })

    it('should keep the captured text', () => {
      expect(container.querySelector('button')!.textContent).toBe('12')
    })
  })

  describe('when an untyped caller passes an invalid count', () => {
    it.each([[-1], [1.5], ['3'], [Number.NaN]])('should throw for %s on the client', (count) => {
      expect(String(renderError(view({ notificationCount: count }, 'label')))).toContain(
        `Bell: notificationCount must be a non-negative integer, got ${JSON.stringify(count)}`
      )
    })

    it.each([[-1], [1.5], ['3'], [Number.NaN]])('should throw for %s on the server', (count) => {
      expect(() => renderToString(view({ notificationCount: count }, 'label'))).toThrow(
        `Bell: notificationCount must be a non-negative integer, got ${JSON.stringify(count)}`
      )
    })
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
    let container: HTMLElement

    beforeEach(() => {
      vi.spyOn(console, 'error').mockImplementation(() => undefined)
      ;({ container } = render(
        <UntypedNavBar
          as="button"
          views={{}}
          select={{ values: { viewport: 'mobile' }, booleans: {}, flags: {} }}
          slots={{ label: 'Injected' }}
          component="Other"
          attributes={{ id: 'injected' }}
          semanticProps={{ logoHref: 'https://example.com' }}
        />
      ))
    })

    it('should keep the generated element', () => {
      expect(container.firstElementChild!.tagName).toBe('NAV')
    })

    it('should keep the generated view', () => {
      expect(container.textContent).toContain('Sign In')
    })

    it('should keep the generated slots', () => {
      expect(container.textContent).not.toContain('Injected')
    })

    it('should keep the generated semantic props', () => {
      expect(container.querySelector('a[href="https://example.com"]')).toBeNull()
    })
  })

  describe('when a caller passes standard element props', () => {
    let onClick: Mock
    let element: HTMLElement | null
    let root: HTMLElement

    beforeEach(() => {
      onClick = vi.fn()
      element = null
      const { container } = render(
        <UntypedNavBar
          viewport="largeDesktop"
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
      root = container.firstElementChild as HTMLElement
    })

    it('should forward className to the root', () => {
      expect(root.classList.contains('consumer')).toBe(true)
    })

    it('should forward style to the root', () => {
      expect(root.style.outline).toBe('1px solid red')
    })

    it('should forward aria attributes to the root', () => {
      expect(root.getAttribute('aria-describedby')).toBe('help')
    })

    it('should forward data attributes to the root', () => {
      expect(root.getAttribute('data-testid')).toBe('bar')
    })

    it('should forward the ref to the root', () => {
      expect(element).toBe(root)
    })

    it('should forward events to the root', () => {
      root.click()
      expect(onClick).toHaveBeenCalledTimes(1)
    })
  })

  describe('when a caller disables a generated button', () => {
    let onClick: Mock
    let button: HTMLButtonElement

    beforeEach(() => {
      onClick = vi.fn()
      const { container } = render(<UntypedButton disabled onClick={onClick} />)
      button = container.querySelector('button')!
    })

    it('should disable the button', () => {
      expect(button.disabled).toBe(true)
    })

    it('should block its click', () => {
      button.click()
      expect(onClick).not.toHaveBeenCalled()
    })
  })

  describe('when a caller passes standard element props to a server render', () => {
    let html: string

    beforeEach(() => {
      html = renderToString(
        <UntypedNavBar className="consumer" data-testid="bar" aria-describedby="help" />
      )
    })

    it('should render the forwarded className on the root', () => {
      expect(html).toMatch(/^<nav[^>]*class="[^"]*consumer/)
    })

    it('should render the forwarded data attribute', () => {
      expect(html).toContain('data-testid="bar"')
    })

    it('should render the forwarded aria attribute', () => {
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
  let base: Record<string, string>

  beforeEach(() => {
    base = { variant: 'primary', size: 'md', viewport: 'desktop' }
  })

  it('should return the exact view object of every Button catalog case', () => {
    for (const { key, props } of buttonCases)
      expect(pickView(buttonViews, selectionOf(props), 'Button')).toBe(buttonViews[key])
  })

  it('should return the exact view object of every Tabs catalog case', () => {
    for (const { key, props } of tabsCases)
      expect(pickView(tabsViews, selectionOf(props), 'Tabs')).toBe(tabsViews[key])
  })

  describe('when the interaction flags are omitted, undefined or all false', () => {
    it('should have a drawn base view to resolve to', () => {
      expect(buttonViews['variant:primary|size:md|viewport:desktop']).toBeDefined()
    })

    it.each([
      ['omitted', {}],
      ['undefined', { disabled: undefined, loading: undefined }],
      ['all false', { disabled: false, loading: false }]
    ])('should treat %s flags as the base view', (_, flags) => {
      expect(pickView(buttonViews, { values: base, booleans: {}, flags }, 'Button')).toBe(
        buttonViews['variant:primary|size:md|viewport:desktop']
      )
    })
  })

  describe('when the selected flags combine into an undrawn view', () => {
    it('should start from a disabled and loading combination Figma never drew', () => {
      expect(
        buttonViews['variant:primary|size:md|viewport:desktop|disabled|loading']
      ).toBeUndefined()
    })

    it('should drop flags from the end, in selected, disabled, loading order', () => {
      expect(
        pickView(
          buttonViews,
          { values: base, booleans: {}, flags: { disabled: true, loading: true } },
          'Button'
        )
      ).toBe(buttonViews['variant:primary|size:md|viewport:desktop|disabled'])
    })

    it('should drop disabled before selected on a selected and disabled tab', () => {
      expect(
        pickView(
          tabsViews,
          { values: { device: 'mobile' }, booleans: {}, flags: { selected: true, disabled: true } },
          'Tabs'
        )
      ).toBe(tabsViews['device:mobile|selected'])
    })
  })

  describe('when a single flag has its own drawn view', () => {
    it('should pick that view', () => {
      expect(
        pickView(buttonViews, { values: base, booleans: {}, flags: { loading: true } }, 'Button')
      ).toBe(buttonViews['variant:primary|size:md|viewport:desktop|loading'])
    })
  })

  describe('when an axis value is unknown', () => {
    it('should throw naming every axis value', () => {
      expect(() =>
        pickView(
          buttonViews,
          { values: { ...base, size: 'xxl' }, booleans: {}, flags: {} },
          'Button'
        )
      ).toThrow('Button has no captured view for variant="primary", size="xxl", viewport="desktop"')
    })
  })

  describe('when the selection names an axis the component does not have', () => {
    it('should throw', () => {
      expect(() =>
        pickView(
          buttonViews,
          { values: { ...base, tone: 'warm' }, booleans: {}, flags: {} },
          'Button'
        )
      ).toThrow('Button has no captured view for')
    })
  })

  describe('when a component with no views gets only flags', () => {
    it('should throw', () => {
      expect(() =>
        pickView({}, { values: {}, booleans: {}, flags: { disabled: true } }, 'Empty')
      ).toThrow('Empty has no captured view for no variant axes')
    })
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

describe('reserved runtime names passed by an untyped caller', () => {
  const RESERVED = [
    'as',
    'attributes',
    'component',
    'counts',
    'defaultKey',
    'responsive',
    'select',
    'semanticProps',
    'slots',
    'suppressed',
    'views'
  ]
  let injected: Record<string, unknown>
  // A ref names one element, so the NavBar renders a single view here.
  const SingleNavBar = (props: Record<string, unknown>) => (
    <UntypedNavBar viewport="largeDesktop" {...props} />
  )

  beforeEach(() => {
    injected = Object.fromEntries(
      RESERVED.map((name) => [name, name === 'as' ? 'button' : { injected: true }])
    )
  })

  describe.each([
    ['NavBar', SingleNavBar],
    ['Button', UntypedButton]
  ] as const)('when they reach %s on the client', (_name, Component) => {
    let onClick: Mock
    let element: HTMLElement | null
    let container: HTMLElement
    let root: HTMLElement

    beforeEach(() => {
      onClick = vi.fn()
      element = null
      ;({ container } = render(
        <Component
          {...injected}
          className="consumer"
          style={{ outline: '1px solid red' }}
          aria-describedby="help"
          data-testid="root"
          onClick={onClick}
          forceState="hover"
          ref={(node: HTMLElement | null) => {
            element = node
          }}
        />
      ))
      root = container.firstElementChild as HTMLElement
    })

    it('should never reach the DOM', () => {
      for (const name of RESERVED) {
        expect(root.hasAttribute(name), name).toBe(false)
        expect(container.querySelector(`[${name.toLowerCase()}]`), name).toBeNull()
      }
    })

    it('should keep the public className', () => {
      expect(root.classList.contains('consumer')).toBe(true)
    })

    it('should keep the public style', () => {
      expect(root.style.outline).toBe('1px solid red')
    })

    it('should keep the public aria attribute', () => {
      expect(root.getAttribute('aria-describedby')).toBe('help')
    })

    it('should keep the public data attribute', () => {
      expect(root.getAttribute('data-testid')).toBe('root')
    })

    it('should keep the forced state', () => {
      expect(root.getAttribute('data-force-state')).toBe('hover')
    })

    it('should keep the public ref', () => {
      expect(element).toBe(root)
    })

    it('should keep the public click handler', () => {
      root.click()
      expect(onClick).toHaveBeenCalledTimes(1)
    })
  })

  describe.each([
    ['NavBar', UntypedNavBar],
    ['Button', UntypedButton]
  ] as const)('when they reach %s on the server', (_name, Component) => {
    let html: string

    beforeEach(() => {
      html = renderToString(<Component {...injected} className="consumer" data-testid="root" />)
    })

    it('should never reach the server-rendered HTML', () => {
      for (const name of RESERVED) expect(html, name).not.toMatch(new RegExp(`\\s${name}=`, 'i'))
    })

    it('should keep the public data attribute', () => {
      expect(html).toContain('data-testid="root"')
    })

    it('should keep the public className', () => {
      expect(html).toMatch(/class="[^"]*consumer/)
    })
  })

  describe('when they reach a disabled generated button', () => {
    it('should keep disabled working', () => {
      const { container } = render(<UntypedButton {...injected} disabled />)
      expect(container.querySelector('button')!.disabled).toBe(true)
    })
  })
})

// null skips a destructuring default, unlike undefined, so an untyped null reaches the runtime as written.
describe('null from an untyped caller', () => {
  describe.each([
    ['an enum axis', { viewport: null }, 'viewport=null'],
    ['a boolean axis', { signedIn: null }, 'signedIn=null']
  ])('when it reaches %s', (_case, props, axis) => {
    it('should throw on the client', () => {
      expect(String(renderError(<UntypedNavBar {...props} />))).toContain(
        `NavBar has no captured view for`
      )
    })

    it('should name the null axis in the client error', () => {
      expect(String(renderError(<UntypedNavBar {...props} />))).toContain(axis)
    })

    it('should throw on the server', () => {
      expect(() => renderToString(<UntypedNavBar {...props} />)).toThrow(
        'NavBar has no captured view for'
      )
    })
  })

  describe('when it reaches an interaction flag', () => {
    it('should treat the flag as off on the client', () => {
      const { container } = render(<UntypedButton disabled={null} loading={null} />)
      expect(container.querySelector('button')!.disabled).toBe(false)
    })

    it('should treat the flag as off on the server', () => {
      expect(renderToString(<UntypedButton disabled={null} loading={null} />)).not.toContain(
        'disabled=""'
      )
    })
  })
})

// No generated view embeds another component today, so this control drives the runtime's reference path directly:
// a view that references the generated Button renders it with the reference's props, and an undrawn reference fails.
describe('a nested generated reference (synthetic control)', () => {
  let refProps: Record<string, string | boolean>
  const withRef = (props: Record<string, string | boolean>): ViewNode => ({
    kind: 'box',
    path: '',
    name: 'root',
    className: '',
    children: [
      { kind: 'ref', path: '0', name: 'Button', className: '', ref: { component: 'Button', props } }
    ]
  })
  const host = (props: Record<string, string | boolean>) => (
    <FigmaView
      views={{ default: withRef(props) }}
      component="Host"
      select={{ values: {}, booleans: {}, flags: {} }}
      slots={{}}
    />
  )

  describe('when the reference selects a drawn view', () => {
    beforeEach(() => {
      refProps = { variant: 'primary', size: 'md', viewport: 'desktop' }
    })

    it('should render the referenced component with the reference props on the client', () => {
      const { container } = render(host(refProps))
      expect(container.querySelector('button')).not.toBeNull()
    })

    it('should render the referenced component with the reference props on the server', () => {
      expect(renderToString(host(refProps))).toContain('<button')
    })
  })

  describe('when the reference selects an undrawn view', () => {
    beforeEach(() => {
      refProps = { variant: 'primary', size: 'xxl', viewport: 'desktop' }
    })

    it('should fail the whole client render', () => {
      expect(String(renderError(host(refProps)))).toContain('Button has no captured view for')
    })

    it('should fail the whole server render', () => {
      expect(() => renderToString(host(refProps))).toThrow('Button has no captured view for')
    })
  })
})

// React 19 reports a key spread into JSX inside a props object; the runtime passes keys on their own.
describe('rendering without React key warnings', () => {
  let log: MockInstance<typeof console.error>

  beforeEach(() => {
    log = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    for (const { props } of navBarCases) {
      const { unmount } = render(<UntypedNavBar {...props} />)
      unmount()
    }
    for (const { props } of buttonCases) {
      const { unmount } = render(<UntypedButton {...props} />)
      unmount()
    }
  })

  it('should render every NavBar and Button case without a key-spread warning', () => {
    const messages = log.mock.calls.map((call) => call.map(String).join(' '))
    expect(messages.filter((message) => /key/i.test(message))).toEqual([])
  })
})
