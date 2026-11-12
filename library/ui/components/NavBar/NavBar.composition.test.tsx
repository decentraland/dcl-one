import { render, screen, within } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { renderToString } from 'react-dom/server'
import { useState } from 'react'
import { createPortal } from 'react-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'
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

  it('should accept explicitly supplied controls for a signed-out account without showing numeric balances', () => {
    render(<NavBar {...props} account={undefined} signIn={{ href: '/sign-in' }} />)
    expect(screen.queryByRole('link', { name: 'Sign in' })).not.toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Get credits' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Connect wallet' })).toBeInTheDocument()
    expect(screen.queryByText('2')).not.toBeInTheDocument()
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
    const close = within(drawer).getByRole('button', { name: 'Close menu' })
    const wallet = within(drawer).getByRole('button', { name: 'Connect wallet' })
    wallet.focus()
    await user.tab()
    expect(close).toHaveFocus()
    await user.tab({ shift: true })
    expect(wallet).toHaveFocus()
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(menu).toHaveFocus()
    expect(document.querySelectorAll('#wallet-control')).toHaveLength(1)
  })

  it('should tolerate absent or boolean slots and preserve explicit zero content', () => {
    const { rerender } = render(
      <NavBar
        sections={[]}
        signIn={{ href: '/sign-in' }}
        balanceAction={false}
        accountControl={null}
        mobileSecondaryNavigation={undefined}
      />
    )
    expect(screen.getByRole('link', { name: 'Sign in' })).toBeInTheDocument()
    rerender(<NavBar sections={[]} signIn={{ href: '/sign-in' }} balanceAction={0} />)
    expect(screen.getByText('0')).toBeInTheDocument()
  })

  it('should server-render injected controls without a dialog or nested interactive wrappers', () => {
    const container = document.createElement('div')
    container.innerHTML = renderToString(<NavBar {...props} />)
    expect(container.querySelector('#credit-control')).not.toBeNull()
    expect(container.querySelector('#wallet-control')).not.toBeNull()
    expect(container.querySelector('button button, a button, button a, a a')).toBeNull()
    expect(container.querySelector('[role="dialog"]')).toBeNull()
  })
  it('should wrap through app inputs while skipping hidden, disabled and untabbable controls', async () => {
    render(
      <NavBar
        {...props}
        accountControl={
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
        }
      />
    )
    await user.click(screen.getByRole('button', { name: 'Open menu' }))
    const drawer = screen.getByRole('dialog', { name: 'Menu' })
    const close = within(drawer).getByRole('button', { name: 'Close menu' })
    const note = within(drawer).getByRole('textbox', { name: 'Wallet note' })
    note.focus()
    await user.tab()
    expect(close).toHaveFocus()
    await user.tab({ shift: true })
    expect(note).toHaveFocus()
    await user.tab({ shift: true })
    expect(within(drawer).getByRole('combobox', { name: 'Wallet network' })).toHaveFocus()
  })

  it('should preserve app-owned loading and callback updates in either control location', async () => {
    const { rerender } = render(
      <NavBar
        {...props}
        balanceAction={
          <button type="button" aria-busy="true" disabled>
            Loading credits
          </button>
        }
      />
    )
    expect(screen.getByRole('button', { name: 'Loading credits' })).toBeDisabled()
    await user.click(screen.getByRole('button', { name: 'Open menu' }))
    expect(
      within(screen.getByRole('dialog')).getByRole('button', { name: 'Loading credits' })
    ).toHaveAttribute('aria-busy', 'true')
    rerender(
      <NavBar
        {...props}
        balanceAction={
          <button type="button" onClick={onConnect}>
            Buy credits
          </button>
        }
      />
    )
    await user.click(
      within(screen.getByRole('dialog')).getByRole('button', { name: 'Buy credits' })
    )
    expect(onConnect).toHaveBeenCalledOnce()
  })

  it('should hydrate the public component with one instance of each supplied control', async () => {
    const container = document.createElement('div')
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
  it('should leave Escape and Tab ownership with an app-owned modal opened from a slot', async () => {
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
    render(<NavBar {...props} accountControl={<WalletControl />} />)
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
  for (const value of [null, undefined]) {
    it(`should restore standard account and sign-in controls for ${String(value)} overrides`, async () => {
      const { rerender } = render(
        <NavBar
          {...props}
          accountControl={value}
          accountActions={[{ label: 'Profile', href: '/profile' }]}
        />
      )
      expect(screen.getByRole('button', { name: 'Account: Ada' })).toBeInTheDocument()
      await user.click(screen.getByRole('button', { name: 'Open menu' }))
      expect(within(screen.getByRole('dialog')).getByText('Ada')).toBeInTheDocument()
      expect(
        within(screen.getByRole('dialog')).getByRole('link', { name: 'Profile' })
      ).toBeInTheDocument()
      await user.keyboard('{Escape}')
      rerender(<NavBar sections={[]} signIn={{ href: '/sign-in' }} accountControl={value} />)
      expect(screen.getByRole('link', { name: 'Sign in' })).toBeInTheDocument()
      await user.click(screen.getByRole('button', { name: 'Open menu' }))
      expect(
        within(screen.getByRole('dialog')).getByRole('link', { name: 'Sign in' })
      ).toBeInTheDocument()
    })
  }

  it('should treat false as an explicit replacement rather than restoring default account controls', async () => {
    const { rerender } = render(
      <NavBar
        {...props}
        accountControl={false}
        accountActions={[{ label: 'Profile', href: '/profile' }]}
      />
    )
    expect(screen.queryByRole('button', { name: 'Account: Ada' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Open menu' }))
    const drawer = screen.getByRole('dialog')
    expect(within(drawer).queryByText('Ada')).not.toBeInTheDocument()
    expect(within(drawer).queryByRole('link', { name: 'Profile' })).not.toBeInTheDocument()
    expect(within(drawer).getByText('0')).toBeInTheDocument()
    await user.keyboard('{Escape}')
    rerender(<NavBar sections={[]} signIn={{ href: '/sign-in' }} accountControl={false} />)
    expect(screen.queryByRole('link', { name: 'Sign in' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Open menu' }))
    expect(
      within(screen.getByRole('dialog')).queryByRole('link', { name: 'Sign in' })
    ).not.toBeInTheDocument()
  })

  it('should let secondary navigation callbacks close the drawer and use the latest app state', async () => {
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
    const { rerender } = render(
      <NavBar {...props} mobileSecondaryNavigation={renderTabs('Old photos')} />
    )
    expect(screen.queryByRole('navigation', { name: 'Profile tabs' })).not.toBeInTheDocument()
    const menu = screen.getByRole('button', { name: 'Open menu' })
    await user.click(menu)
    rerender(<NavBar {...props} mobileSecondaryNavigation={renderTabs('Current photos')} />)
    expect(screen.queryByRole('link', { name: 'Old photos' })).not.toBeInTheDocument()
    await user.click(
      within(screen.getByRole('dialog')).getByRole('link', { name: 'Current photos' })
    )
    expect(onConnect).toHaveBeenCalledOnce()
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(menu).toHaveFocus()
    expect(document.querySelectorAll('#wallet-control')).toHaveLength(1)
  })

  it('should allow a secondary navigation callback to render nothing', async () => {
    render(<NavBar {...props} mobileSecondaryNavigation={() => null} />)
    await user.click(screen.getByRole('button', { name: 'Open menu' }))
    expect(screen.queryByRole('navigation', { name: 'Profile sections' })).not.toBeInTheDocument()
    expect(screen.getByRole('dialog')).toBeInTheDocument()
  })
  it('should preserve a noninteractive balance action without inventing absent numeric balances', async () => {
    render(
      <NavBar
        {...props}
        balances={undefined}
        balanceAction={<span id="iap-status">IAP credits</span>}
      />
    )
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
