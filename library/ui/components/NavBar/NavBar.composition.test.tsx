import { fireEvent, render, screen, within } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { renderToString } from 'react-dom/server'
import { useState } from 'react'
import { createPortal } from 'react-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { NavBar, type NavBarProps } from '../../index'

describe('when NavBar accepts app-owned composition controls', () => {
  let user: UserEvent
  let onConnect: ReturnType<typeof vi.fn<() => void>>
  let props: NavBarProps
  beforeEach(() => {
    user = userEvent.setup()
    onConnect = vi.fn()
    props = {
      sections: [],
      account: { address: '0x1234', name: 'Ada' },
      balances: { credits: 0, mana: 2 },
      balanceAction: (
        <a id="credit-control" href="/credits">
          Get credits
        </a>
      ),
      accountControl: (
        <button id="wallet-control" type="button" onClick={onConnect}>
          Connect wallet
        </button>
      ),
      mobileSecondaryNavigation: (
        <nav aria-label="Profile sections">
          <a href="/profile/photos">Photos</a>
        </nav>
      )
    }
  })

  afterEach(() => {
    vi.resetAllMocks()
  })

  it('should recover unclaimed outside focus while respecting an explicitly consumed key', async () => {
    render(
      <>
        <button>Outside</button>
        <NavBar {...props} />
      </>
    )
    await user.click(screen.getByRole('button', { name: 'Open menu' }))
    const outside = screen.getByRole('button', { name: 'Outside' })
    outside.focus()
    expect(fireEvent.keyDown(outside, { key: 'Tab' })).toBe(false)
    expect(screen.getByRole('link', { name: 'Decentraland home' })).toHaveFocus()
    outside.focus()
    outside.addEventListener('keydown', (event) => event.preventDefault(), { once: true })
    fireEvent.keyDown(outside, { key: 'Escape' })
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    expect(outside).toHaveFocus()
    fireEvent.keyDown(outside, { key: 'Escape' })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Open menu' })).toHaveFocus()
  })

  describe('and the account control is a closed native disclosure', () => {
    beforeEach(() => {
      props = {
        ...props,
        accountControl: (
          <details>
            <summary>Wallet options</summary>
            <button>Hidden wallet action</button>
          </details>
        )
      }
    })

    it('should cycle through native summary while excluding closed disclosure content', async () => {
      render(<NavBar {...props} />)
      await user.click(screen.getByRole('button', { name: 'Open menu' }))
      screen.getByRole('link', { name: 'Decentraland home' }).focus()
      await user.tab({ shift: true })
      expect(screen.getByText('Wallet options')).toHaveFocus()
      await user.tab()
      expect(screen.getByRole('link', { name: 'Decentraland home' })).toHaveFocus()
    })
  })

  describe('and the reduced-motion preference is observed', () => {
    type MediaQueryStub = {
      matches: boolean
      addEventListener: ReturnType<typeof vi.fn>
      removeEventListener: ReturnType<typeof vi.fn>
    }
    let reduced: MediaQueryStub
    let desktop: MediaQueryStub

    beforeEach(() => {
      reduced = { matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() }
      desktop = { matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() }
      vi.stubGlobal('matchMedia', (query: string) =>
        query.includes('prefers-reduced-motion') ? reduced : desktop
      )
    })

    afterEach(() => {
      vi.unstubAllGlobals()
    })

    it('should release the reduced-motion observer after the open menu leaves', async () => {
      render(<NavBar {...props} />)
      await user.click(screen.getByRole('button', { name: 'Open menu' }))
      expect(reduced.addEventListener).toHaveBeenCalledOnce()
      const listener = reduced.addEventListener.mock.calls[0]![1]
      await user.click(screen.getByRole('button', { name: 'Close menu' }))
      expect(reduced.removeEventListener).toHaveBeenCalledExactlyOnceWith('change', listener)
    })
  })

  it('should keep the one real header inside the open menu modal ownership', async () => {
    render(<NavBar {...props} />)
    const header = screen.getByRole('navigation', { name: 'Main' })
    const trigger = screen.getByRole('button', { name: 'Open menu' })
    await user.click(trigger)
    const modal = screen.getByRole('dialog', { name: 'Menu' })
    expect(modal).toContainElement(header)
    expect(within(modal).getAllByRole('navigation', { name: 'Main' })).toHaveLength(1)
    expect(trigger).toHaveAccessibleName('Close menu')
    expect(screen.queryByRole('heading', { name: 'Menu' })).not.toBeInTheDocument()
    await user.click(trigger)
    expect(screen.queryByRole('dialog', { name: 'Menu' })).not.toBeInTheDocument()
    expect(trigger).toHaveFocus()
  })

  it('should retain numeric balances while replacing the standard account trigger', async () => {
    render(<NavBar {...props} />)
    expect(screen.getByText('0')).toBeInTheDocument()
    expect(screen.getByText('2')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Account: Ada' })).not.toBeInTheDocument()
    const credits = screen.getByRole('link', { name: 'Get credits' })
    const wallet = screen.getByRole('button', { name: 'Connect wallet' })
    expect(credits.parentElement?.closest('a, button')).toBeNull()
    expect(wallet.parentElement?.closest('a, button')).toBeNull()
    await user.click(wallet)
    expect(onConnect).toHaveBeenCalledOnce()
  })

  describe('and the account is signed out', () => {
    beforeEach(() => {
      props = { ...props, account: undefined, signIn: { href: '/sign-in' } }
    })

    it('should accept explicitly supplied controls without showing numeric balances', () => {
      render(<NavBar {...props} />)
      expect(screen.queryByRole('link', { name: 'Sign in' })).not.toBeInTheDocument()
      expect(screen.getByRole('link', { name: 'Get credits' })).toBeInTheDocument()
      expect(screen.getByRole('button', { name: 'Connect wallet' })).toBeInTheDocument()
      expect(screen.queryByText('2')).not.toBeInTheDocument()
    })
  })

  it('should render secondary navigation only inside an open mobile dialog', async () => {
    render(<NavBar {...props} />)
    expect(screen.queryByRole('navigation', { name: 'Profile sections' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Open menu' }))
    const drawer = screen.getByRole('dialog', { name: 'Menu' })
    expect(within(drawer).getByRole('navigation', { name: 'Profile sections' })).toBeInTheDocument()
    expect(within(drawer).getByRole('link', { name: 'Photos' })).toHaveAttribute(
      'href',
      '/profile/photos'
    )
  })

  it('should mount only one instance of each custom control while the mobile dialog is open', async () => {
    render(<NavBar {...props} />)
    await user.click(screen.getByRole('button', { name: 'Open menu' }))
    const drawer = screen.getByRole('dialog', { name: 'Menu' })
    expect(document.querySelectorAll('#credit-control')).toHaveLength(1)
    expect(document.querySelectorAll('#wallet-control')).toHaveLength(1)
    expect(within(drawer).getByRole('link', { name: 'Get credits' })).toBeInTheDocument()
    await user.click(within(drawer).getByRole('button', { name: 'Connect wallet' }))
    expect(onConnect).toHaveBeenCalledOnce()
  })

  it('should include injected controls in the drawer focus wrap and return focus on Escape', async () => {
    render(<NavBar {...props} />)
    const menu = screen.getByRole('button', { name: 'Open menu' })
    await user.click(menu)
    const drawer = screen.getByRole('dialog', { name: 'Menu' })
    const first = within(drawer).getByRole('link', { name: 'Decentraland home' })
    const wallet = within(drawer).getByRole('button', { name: 'Connect wallet' })
    wallet.focus()
    await user.tab()
    expect(first).toHaveFocus()
    await user.tab({ shift: true })
    expect(wallet).toHaveFocus()
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(menu).toHaveFocus()
    expect(document.querySelectorAll('#wallet-control')).toHaveLength(1)
  })

  describe('and the slots are absent, boolean or zero', () => {
    let emptySlotProps: NavBarProps
    let zeroSlotProps: NavBarProps

    beforeEach(() => {
      emptySlotProps = {
        sections: [],
        signIn: { href: '/sign-in' },
        balanceAction: false,
        accountControl: null,
        mobileSecondaryNavigation: undefined
      }
      zeroSlotProps = { sections: [], signIn: { href: '/sign-in' }, balanceAction: 0 }
    })

    it('should tolerate absent or boolean slots and preserve explicit zero content', () => {
      const { rerender } = render(<NavBar {...emptySlotProps} />)
      expect(screen.getByRole('link', { name: 'Sign in' })).toBeInTheDocument()
      rerender(<NavBar {...zeroSlotProps} />)
      expect(screen.getByText('0')).toBeInTheDocument()
    })
  })

  describe('and the account control holds app inputs and untabbable controls', () => {
    beforeEach(() => {
      props = {
        ...props,
        accountControl: (
          <>
            <label>
              Wallet network
              <select defaultValue="main">
                <option value="main">Main</option>
              </select>
            </label>
            <label>
              Wallet note
              <textarea />
            </label>
            <button type="button" disabled>
              Unavailable
            </button>
            <button type="button" tabIndex={-1}>
              Programmatic only
            </button>
            <div hidden>
              <button type="button">Hidden ancestor</button>
            </div>
            <div style={{ display: 'none' }}>
              <button type="button">Hidden display</button>
            </div>
            <div style={{ visibility: 'hidden' }}>
              <button type="button">Hidden visibility</button>
            </div>
            <div inert>
              <button type="button">Inert ancestor</button>
            </div>
            <input type="hidden" />
          </>
        )
      }
    })

    it('should wrap through app inputs while skipping hidden, disabled and untabbable controls', async () => {
      render(<NavBar {...props} />)
      await user.click(screen.getByRole('button', { name: 'Open menu' }))
      const drawer = screen.getByRole('dialog', { name: 'Menu' })
      const first = within(drawer).getByRole('link', { name: 'Decentraland home' })
      const note = within(drawer).getByRole('textbox', { name: 'Wallet note' })
      note.focus()
      await user.tab()
      expect(first).toHaveFocus()
      await user.tab({ shift: true })
      expect(note).toHaveFocus()
      await user.tab({ shift: true })
      expect(within(drawer).getByRole('combobox', { name: 'Wallet network' })).toHaveFocus()
    })
  })

  describe('and the app-owned balance action moves from loading to ready', () => {
    let loadingProps: NavBarProps
    let readyProps: NavBarProps

    beforeEach(() => {
      loadingProps = {
        ...props,
        balanceAction: (
          <button type="button" aria-busy="true" disabled>
            Loading credits
          </button>
        )
      }
      readyProps = {
        ...props,
        balanceAction: (
          <button type="button" onClick={onConnect}>
            Buy credits
          </button>
        )
      }
    })

    it('should preserve app-owned loading and callback updates in either control location', async () => {
      const { rerender } = render(<NavBar {...loadingProps} />)
      expect(screen.getByRole('button', { name: 'Loading credits' })).toBeDisabled()
      await user.click(screen.getByRole('button', { name: 'Open menu' }))
      expect(
        within(screen.getByRole('dialog')).getByRole('button', { name: 'Loading credits' })
      ).toHaveAttribute('aria-busy', 'true')
      rerender(<NavBar {...readyProps} />)
      await user.click(
        within(screen.getByRole('dialog')).getByRole('button', { name: 'Buy credits' })
      )
      expect(onConnect).toHaveBeenCalledOnce()
    })
  })

  describe('and a slot opens an app-owned modal', () => {
    function WalletControl() {
      const [open, setOpen] = useState(false)
      return (
        <>
          <button type="button" onClick={() => setOpen(true)}>
            Wallet dialog
          </button>
          {open
            ? createPortal(
                <div role="dialog" aria-modal="true" aria-label="Wallet">
                  <button
                    type="button"
                    autoFocus
                    onKeyDown={(event) => {
                      if (event.key === 'Tab') event.preventDefault()
                      if (event.key === 'Escape') {
                        event.preventDefault()
                        setOpen(false)
                      }
                    }}
                  >
                    Wallet account
                  </button>
                </div>,
                document.body
              )
            : null}
        </>
      )
    }

    beforeEach(() => {
      props = { ...props, accountControl: <WalletControl /> }
    })

    it('should leave Escape and Tab ownership with the app-owned modal', async () => {
      render(<NavBar {...props} />)
      await user.click(screen.getByRole('button', { name: 'Open menu' }))
      await user.click(
        within(screen.getByRole('dialog', { name: 'Menu' })).getByRole('button', {
          name: 'Wallet dialog'
        })
      )
      const walletAccount = within(screen.getByRole('dialog', { name: 'Wallet' })).getByRole(
        'button',
        { name: 'Wallet account' }
      )
      expect(walletAccount).toHaveFocus()
      await user.tab()
      expect(walletAccount).toHaveFocus()
      await user.keyboard('{Escape}')
      expect(screen.queryByRole('dialog', { name: 'Wallet' })).not.toBeInTheDocument()
      expect(screen.getByRole('dialog', { name: 'Menu' })).toBeInTheDocument()
    })
  })
  for (const value of [null, undefined]) {
    describe(`and the account control override is ${String(value)}`, () => {
      let signedInProps: NavBarProps
      let signedOutProps: NavBarProps

      beforeEach(() => {
        signedInProps = {
          ...props,
          accountControl: value,
          accountActions: [{ label: 'Profile', href: '/profile' }]
        }
        signedOutProps = { sections: [], signIn: { href: '/sign-in' }, accountControl: value }
      })

      it(`should restore standard account and sign-in controls for ${String(value)} overrides`, async () => {
        const { rerender } = render(<NavBar {...signedInProps} />)
        expect(screen.getByRole('button', { name: 'Account: Ada' })).toBeInTheDocument()
        await user.click(screen.getByRole('button', { name: 'Open menu' }))
        await user.click(screen.getByRole('button', { name: 'Account: Ada' }))
        expect(within(screen.getByRole('dialog')).getByText('Ada')).toBeVisible()
        expect(
          within(screen.getByRole('dialog')).getByRole('link', { name: 'Profile' })
        ).toBeInTheDocument()
        await user.keyboard('{Escape}')
        expect(screen.getByRole('dialog', { name: 'Menu' })).toBeInTheDocument()
        await user.keyboard('{Escape}')
        rerender(<NavBar {...signedOutProps} />)
        expect(screen.getByRole('link', { name: 'Sign in' })).toBeInTheDocument()
        await user.click(screen.getByRole('button', { name: 'Open menu' }))
        expect(
          within(screen.getByRole('dialog')).getByRole('link', { name: 'Sign in' })
        ).toBeInTheDocument()
      })
    })
  }

  describe('and the account control override is false', () => {
    let signedInProps: NavBarProps
    let signedOutProps: NavBarProps

    beforeEach(() => {
      signedInProps = {
        ...props,
        accountControl: false,
        accountActions: [{ label: 'Profile', href: '/profile' }]
      }
      signedOutProps = { sections: [], signIn: { href: '/sign-in' }, accountControl: false }
    })

    it('should treat false as an explicit replacement rather than restoring default account controls', async () => {
      const { rerender } = render(<NavBar {...signedInProps} />)
      expect(screen.queryByRole('button', { name: 'Account: Ada' })).not.toBeInTheDocument()
      await user.click(screen.getByRole('button', { name: 'Open menu' }))
      const drawer = screen.getByRole('dialog')
      expect(within(drawer).queryByText('Ada')).not.toBeInTheDocument()
      expect(within(drawer).queryByRole('link', { name: 'Profile' })).not.toBeInTheDocument()
      expect(within(drawer).getByText('0')).toBeInTheDocument()
      await user.keyboard('{Escape}')
      rerender(<NavBar {...signedOutProps} />)
      expect(screen.queryByRole('link', { name: 'Sign in' })).not.toBeInTheDocument()
      await user.click(screen.getByRole('button', { name: 'Open menu' }))
      expect(
        within(screen.getByRole('dialog')).queryByRole('link', { name: 'Sign in' })
      ).not.toBeInTheDocument()
    })
  })

  describe('and the secondary navigation is a render callback', () => {
    let oldTabsProps: NavBarProps
    let currentTabsProps: NavBarProps

    beforeEach(() => {
      const renderTabs =
        (label: string) =>
        ({ closeDrawer }: { closeDrawer: () => void }) => (
          <nav aria-label="Profile tabs">
            <a
              href="/profile/photos"
              onClick={(event) => {
                event.preventDefault()
                onConnect()
                closeDrawer()
              }}
            >
              {label}
            </a>
          </nav>
        )
      oldTabsProps = { ...props, mobileSecondaryNavigation: renderTabs('Old photos') }
      currentTabsProps = { ...props, mobileSecondaryNavigation: renderTabs('Current photos') }
    })

    it('should let secondary navigation callbacks close the drawer and use the latest app state', async () => {
      const { rerender } = render(<NavBar {...oldTabsProps} />)
      expect(screen.queryByRole('navigation', { name: 'Profile tabs' })).not.toBeInTheDocument()
      const menu = screen.getByRole('button', { name: 'Open menu' })
      await user.click(menu)
      rerender(<NavBar {...currentTabsProps} />)
      expect(screen.queryByRole('link', { name: 'Old photos' })).not.toBeInTheDocument()
      await user.click(
        within(screen.getByRole('dialog')).getByRole('link', { name: 'Current photos' })
      )
      expect(onConnect).toHaveBeenCalledOnce()
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
      expect(menu).toHaveFocus()
      expect(document.querySelectorAll('#wallet-control')).toHaveLength(1)
    })
  })

  describe('and the secondary navigation callback renders nothing', () => {
    beforeEach(() => {
      props = { ...props, mobileSecondaryNavigation: () => null }
    })

    it('should open the drawer without secondary navigation', async () => {
      render(<NavBar {...props} />)
      await user.click(screen.getByRole('button', { name: 'Open menu' }))
      expect(screen.queryByRole('navigation', { name: 'Profile sections' })).not.toBeInTheDocument()
      expect(screen.getByRole('dialog')).toBeInTheDocument()
    })
  })

  describe('and the balance action is noninteractive without numeric balances', () => {
    beforeEach(() => {
      props = {
        ...props,
        balances: undefined,
        balanceAction: <span id="iap-status">IAP credits</span>
      }
    })

    it('should preserve a noninteractive balance action without inventing absent numeric balances', async () => {
      render(<NavBar {...props} />)
      expect(screen.queryByText('0')).not.toBeInTheDocument()
      expect(screen.queryByText('2')).not.toBeInTheDocument()
      expect(screen.getByText('IAP credits').closest('a,button')).toBeNull()
      await user.click(screen.getByRole('button', { name: 'Open menu' }))
      const drawer = screen.getByRole('dialog')
      expect(within(drawer).getByText('IAP credits').closest('a,button')).toBeNull()
      expect(within(drawer).queryByText('0')).not.toBeInTheDocument()
      expect(document.querySelectorAll('#iap-status')).toHaveLength(1)
    })
  })

  it('should default to the standard surface and expose an explicit overlay surface without changing content', () => {
    const { rerender } = render(<NavBar {...props} />)
    expect(screen.getByRole('navigation', { name: 'Main' })).toHaveAttribute(
      'data-surface',
      'solid'
    )
    rerender(<NavBar {...props} surface="overlay" />)
    expect(screen.getByRole('navigation', { name: 'Main' })).toHaveAttribute(
      'data-surface',
      'overlay'
    )
    expect(screen.getByRole('button', { name: 'Connect wallet' })).toBeInTheDocument()
  })

  describe('and the markup is server-rendered', () => {
    let container: HTMLElement

    beforeEach(() => {
      container = document.createElement('div')
    })

    it('should server-render injected controls without a dialog or nested interactive wrappers', () => {
      container.innerHTML = renderToString(<NavBar {...props} />)
      expect(container.querySelector('#credit-control')).not.toBeNull()
      expect(container.querySelector('#wallet-control')).not.toBeNull()
      expect(container.querySelector('button button, a button, button a, a a')).toBeNull()
      expect(container.querySelector('[role="dialog"]')).toBeNull()
    })

    it('should hydrate the public component with one instance of each supplied control', async () => {
      container.innerHTML = renderToString(<NavBar {...props} />)
      document.body.append(container)
      render(<NavBar {...props} />, { container, hydrate: true })
      expect(container.querySelectorAll('#credit-control')).toHaveLength(1)
      expect(container.querySelectorAll('#wallet-control')).toHaveLength(1)
      await user.click(screen.getByRole('button', { name: 'Open menu' }))
      expect(
        within(screen.getByRole('dialog')).getByRole('link', { name: 'Photos' })
      ).toBeInTheDocument()
      expect(container.querySelectorAll('#credit-control')).toHaveLength(1)
    })

    it('should server-render the owner-selected overlay surface', () => {
      container.innerHTML = renderToString(<NavBar {...props} surface="overlay" />)
      expect(container.querySelector('nav')).toHaveAttribute('data-surface', 'overlay')
      expect(container.querySelectorAll('#wallet-control')).toHaveLength(1)
    })

    for (const surface of ['solid', 'overlay'] as const) {
      describe(`and the default account renders with a 44px balance control on ${surface}`, () => {
        beforeEach(() => {
          props = {
            ...props,
            accountControl: undefined,
            surface,
            balanceAction: (
              <a
                id="ssr-credit-action"
                href="/shop/credits"
                style={{ display: 'inline-flex', height: 44 }}
              >
                0
              </a>
            )
          }
        })

        it(`should server-render the default account and one 44px balance control on ${surface}`, () => {
          container.innerHTML = renderToString(<NavBar {...props} />)
          expect(container.querySelector('nav')).toHaveAttribute('data-surface', surface)
          expect(container.querySelectorAll('#ssr-credit-action')).toHaveLength(1)
          expect(container.querySelector('#ssr-credit-action')).toHaveAttribute(
            'href',
            '/shop/credits'
          )
          expect(container.querySelector('button[aria-label="Account: Ada"]')).not.toBeNull()
          expect(container.querySelector('[role="dialog"]')).toBeNull()
        })
      })
    }
  })
})
