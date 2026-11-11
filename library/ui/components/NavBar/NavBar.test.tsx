import { act, fireEvent, render, screen, within } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import type { ReactElement } from 'react'
import { hydrateRoot, type Root } from 'react-dom/client'
import { renderToString } from 'react-dom/server'
import { afterEach, beforeEach, describe, expect, it, vi, type Mock, type MockInstance } from 'vitest'
import { NavBar, type NavBarLinkProps, type NavBarProps, type NavBarSection } from './NavBar'

const sections: NavBarSection[] = [
  {
    label: 'Discover',
    items: [
      { label: 'Jump in', href: '/play/' },
      { label: "What's on", href: '/whats-on', active: true }
    ]
  },
  { label: 'Shop', href: '/shop' },
  { label: 'Create', href: '/create' },
  {
    label: 'Learn',
    items: [
      { label: 'Docs', href: 'https://docs.example.com/' },
      { label: 'Blog', href: '/blog' }
    ]
  }
]

const account = { address: '0x1234567890abcdef1234567890abcdef12345678', name: 'Ada' }

const notifications = () => screen.getByRole('button', { name: /^Notifications/ })

describe('NavBar', () => {
  let user: UserEvent
  beforeEach(() => {
    user = userEvent.setup()
  })

  describe('when signed out with a sign-in callback', () => {
    let onSignIn: Mock<() => void>
    beforeEach(() => {
      onSignIn = vi.fn<() => void>()
      render(<NavBar sections={sections} onSignIn={onSignIn} />)
    })

    it('should render a navigation landmark named Main', () => {
      expect(screen.getByRole('navigation', { name: 'Main' })).toBeInTheDocument()
    })

    it('should link the logo to the home page', () => {
      expect(screen.getByRole('link', { name: 'Decentraland home' })).toHaveAttribute('href', '/')
    })

    it('should render the section links', () => {
      expect(screen.getByRole('link', { name: 'Shop' })).toHaveAttribute('href', '/shop')
    })

    it('should call onSignIn when Sign in is pressed', async () => {
      await user.click(screen.getByRole('button', { name: 'Sign in' }))
      expect(onSignIn).toHaveBeenCalledOnce()
    })

    it('should not render an account trigger', () => {
      expect(screen.queryByRole('button', { name: /^Account/ })).not.toBeInTheDocument()
    })

    it('should not render a notifications button', () => {
      expect(screen.queryByRole('button', { name: /^Notifications/ })).not.toBeInTheDocument()
    })
  })

  describe('when signed out with only a sign-in href', () => {
    beforeEach(() => {
      render(<NavBar sections={sections} signInHref="/sign-in" />)
    })

    it('should render Sign in as a link', () => {
      expect(screen.getByRole('link', { name: 'Sign in' })).toHaveAttribute('href', '/sign-in')
    })
  })

  describe('when signed out without a sign-in action', () => {
    beforeEach(() => {
      render(<NavBar sections={sections} />)
    })

    it('should not render a Sign in control', () => {
      expect(screen.queryByText('Sign in')).not.toBeInTheDocument()
    })
  })

  describe('when signed in with no notification count or balances', () => {
    beforeEach(() => {
      render(<NavBar sections={sections} account={account} onNotificationsClick={() => {}} />)
    })

    it('should name the notifications button without a count', () => {
      expect(notifications()).toHaveAccessibleName('Notifications')
    })

    it('should not render a count badge', () => {
      expect(notifications()).toHaveTextContent(/^$/)
    })

    it('should not render any balance', () => {
      expect(screen.queryByText(/Credits|MANA/)).not.toBeInTheDocument()
    })

    it('should not render Sign in', () => {
      expect(screen.queryByText('Sign in')).not.toBeInTheDocument()
    })

    it('should name the account trigger with the account name', () => {
      expect(screen.getByRole('button', { name: 'Account: Ada' })).toBeInTheDocument()
    })
  })

  describe('when signed in without a name', () => {
    beforeEach(() => {
      render(<NavBar sections={sections} account={{ address: account.address }} />)
    })

    it('should name the account trigger with the shortened address', () => {
      expect(screen.getByRole('button', { name: 'Account: 0x1234...5678' })).toBeInTheDocument()
    })
  })

  describe('when signed in without onNotificationsClick', () => {
    beforeEach(() => {
      render(<NavBar sections={sections} account={account} notificationCount={3} />)
    })

    it('should not render a notifications button', () => {
      expect(screen.queryByRole('button', { name: /^Notifications/ })).not.toBeInTheDocument()
    })
  })

  describe.each([
    ['0', 0],
    ['negative', -2],
    ['not a number', Number.NaN],
    ['infinite', Number.POSITIVE_INFINITY]
  ])('when the notification count is %s', (_, notificationCount) => {
    beforeEach(() => {
      render(
        <NavBar
          sections={sections}
          account={account}
          notificationCount={notificationCount}
          onNotificationsClick={() => {}}
        />
      )
    })

    it('should not render a count badge', () => {
      expect(notifications()).toHaveTextContent(/^$/)
    })

    it('should name the button without a count', () => {
      expect(notifications()).toHaveAccessibleName('Notifications')
    })
  })

  describe('when the notification count is 3', () => {
    beforeEach(() => {
      render(<NavBar sections={sections} account={account} notificationCount={3} onNotificationsClick={() => {}} />)
    })

    it('should show the count in the badge', () => {
      expect(notifications()).toHaveTextContent('3')
    })

    it('should announce the count in the button name', () => {
      expect(notifications()).toHaveAccessibleName('Notifications, 3 unread')
    })
  })

  describe('when the notification count is larger than the badge can show', () => {
    beforeEach(() => {
      render(<NavBar sections={sections} account={account} notificationCount={250} onNotificationsClick={() => {}} />)
    })

    it('should cap the badge at 99+', () => {
      expect(notifications()).toHaveTextContent('99+')
    })

    it('should announce the exact count', () => {
      expect(notifications()).toHaveAccessibleName('Notifications, 250 unread')
    })
  })

  describe('when the notifications button is pressed', () => {
    let onNotificationsClick: Mock<() => void>
    beforeEach(async () => {
      onNotificationsClick = vi.fn<() => void>()
      render(<NavBar sections={sections} account={account} onNotificationsClick={onNotificationsClick} />)
      await user.click(notifications())
    })

    it('should call onNotificationsClick', () => {
      expect(onNotificationsClick).toHaveBeenCalledOnce()
    })
  })

  describe('when balances are present', () => {
    let balances: HTMLElement[]
    beforeEach(() => {
      render(
        <NavBar sections={sections} account={account} balances={{ credits: 30, mana: 1234.567, polygonMana: 0 }} />
      )
      // Balance items share the listitem role with the section links; data-balance tells them apart.
      balances = screen.getAllByRole('listitem').filter((item) => item.hasAttribute('data-balance'))
    })

    it('should render each defined balance with its label', () => {
      expect(balances.map((item) => item.textContent)).toEqual([
        'Credits: 30',
        'MANA: 1,234.57',
        'Polygon MANA: 0'
      ])
    })
  })

  describe('when only some balances are defined', () => {
    beforeEach(() => {
      render(<NavBar sections={sections} account={account} balances={{ credits: 5, mana: Number.NaN }} />)
    })

    it('should not render the missing or invalid balances', () => {
      expect(screen.queryByText(/MANA/)).not.toBeInTheDocument()
    })
  })

  describe('when balances are passed while signed out', () => {
    beforeEach(() => {
      render(<NavBar sections={sections} balances={{ credits: 5 }} onSignIn={() => {}} />)
    })

    it('should not render them', () => {
      expect(screen.queryByText(/Credits/)).not.toBeInTheDocument()
    })
  })

  describe('when the account has an avatar', () => {
    let trigger: HTMLElement
    // The avatar is decorative (alt=""), so it has no role to query by.
    beforeEach(() => {
      render(<NavBar sections={sections} account={{ ...account, avatarUrl: '/face.png' }} />)
      trigger = screen.getByRole('button', { name: 'Account: Ada' })
    })

    it('should render the avatar image', () => {
      expect(trigger.querySelector('img')).toHaveAttribute('src', '/face.png')
    })

    it('should fall back to the Decentraland mark when the image fails', () => {
      fireEvent.error(trigger.querySelector('img')!)
      expect(trigger.querySelector('img')).toBeNull()
    })
  })

  describe('when the account has no avatar', () => {
    beforeEach(() => {
      render(<NavBar sections={sections} account={account} />)
    })

    it('should not render an image', () => {
      expect(screen.getByRole('button', { name: 'Account: Ada' }).querySelector('img')).toBeNull()
    })
  })

  describe('when a dropdown section is focused and opened with the keyboard', () => {
    let trigger: HTMLElement
    beforeEach(async () => {
      render(<NavBar sections={sections} onSignIn={() => {}} />)
      trigger = screen.getByRole('button', { name: 'Discover' })
      trigger.focus()
      await user.keyboard('{Enter}')
    })

    it('should mark the trigger expanded', () => {
      expect(trigger).toHaveAttribute('aria-expanded', 'true')
    })

    it('should control the list it opens', () => {
      expect(document.getElementById(trigger.getAttribute('aria-controls')!)).toBeVisible()
    })

    it('should mark the active item as the current page', () => {
      expect(screen.getByRole('link', { name: "What's on" })).toHaveAttribute('aria-current', 'page')
    })

    describe('and Escape is pressed inside it', () => {
      beforeEach(async () => {
        screen.getByRole('link', { name: 'Jump in' }).focus()
        await user.keyboard('{Escape}')
      })

      it('should collapse the dropdown', () => {
        expect(trigger).toHaveAttribute('aria-expanded', 'false')
      })

      it('should return focus to the trigger', () => {
        expect(trigger).toHaveFocus()
      })
    })

    describe('and focus moves out of it', () => {
      beforeEach(async () => {
        screen.getByRole('link', { name: 'Shop' }).focus()
      })

      it('should collapse the dropdown', () => {
        expect(trigger).toHaveAttribute('aria-expanded', 'false')
      })
    })

    describe('and the page outside it is pressed', () => {
      beforeEach(async () => {
        await user.click(document.body)
      })

      it('should collapse the dropdown', () => {
        expect(trigger).toHaveAttribute('aria-expanded', 'false')
      })
    })
  })

  describe('when a dropdown opened without focus (as Safari does on click) is pressed outside', () => {
    let trigger: HTMLElement
    beforeEach(() => {
      render(<NavBar sections={sections} onSignIn={() => {}} />)
      trigger = screen.getByRole('button', { name: 'Discover' })
      fireEvent.click(trigger)
      fireEvent.pointerDown(document.body)
    })

    it('should collapse the dropdown', () => {
      expect(trigger).toHaveAttribute('aria-expanded', 'false')
    })
  })

  describe('when focus leaves an open dropdown without a new focus target', () => {
    let trigger: HTMLElement
    beforeEach(async () => {
      render(<NavBar sections={sections} onSignIn={() => {}} />)
      trigger = screen.getByRole('button', { name: 'Discover' })
      await user.click(trigger)
      fireEvent.blur(trigger, { relatedTarget: null })
    })

    it('should keep the dropdown open', () => {
      expect(trigger).toHaveAttribute('aria-expanded', 'true')
    })
  })

  describe('when a dropdown section is closed', () => {
    let trigger: HTMLElement
    beforeEach(() => {
      render(<NavBar sections={sections} onSignIn={() => {}} />)
      trigger = screen.getByRole('button', { name: 'Discover' })
    })

    it('should hide its list', () => {
      expect(document.getElementById(trigger.getAttribute('aria-controls')!)).not.toBeVisible()
    })
  })

  describe('when the account menu is opened', () => {
    let onSignOut: Mock<() => void>
    let trigger: HTMLElement
    beforeEach(async () => {
      onSignOut = vi.fn<() => void>()
      render(
        <NavBar
          sections={sections}
          account={account}
          accountActions={[
            { label: 'My profile', href: '/profile' },
            { label: 'Sign out', onSelect: onSignOut }
          ]}
        />
      )
      trigger = screen.getByRole('button', { name: 'Account: Ada' })
      await user.click(trigger)
    })

    it('should mark the trigger expanded', () => {
      expect(trigger).toHaveAttribute('aria-expanded', 'true')
    })

    it('should show the account links', () => {
      expect(screen.getByRole('link', { name: 'My profile' })).toHaveAttribute('href', '/profile')
    })

    describe('and an action is selected', () => {
      beforeEach(async () => {
        await user.click(screen.getByRole('button', { name: 'Sign out' }))
      })

      it('should run the action', () => {
        expect(onSignOut).toHaveBeenCalledOnce()
      })

      it('should close the menu', () => {
        expect(trigger).toHaveAttribute('aria-expanded', 'false')
      })
    })

    describe('and Escape is pressed', () => {
      beforeEach(async () => {
        await user.keyboard('{Escape}')
      })

      it('should close the menu', () => {
        expect(trigger).toHaveAttribute('aria-expanded', 'false')
      })

      it('should return focus to the trigger', () => {
        expect(trigger).toHaveFocus()
      })
    })

    describe('and the page outside it is pressed', () => {
      beforeEach(() => {
        fireEvent.pointerDown(document.body)
      })

      it('should close the menu', () => {
        expect(trigger).toHaveAttribute('aria-expanded', 'false')
      })
    })
  })

  describe('when a dropdown link is clicked', () => {
    let trigger: HTMLElement
    beforeEach(async () => {
      render(<NavBar sections={sections} onSignIn={() => {}} />)
      trigger = screen.getByRole('button', { name: 'Discover' })
      await user.click(trigger)
      fireEvent.click(screen.getByRole('link', { name: 'Jump in' }))
    })

    it('should close the dropdown', () => {
      expect(trigger).toHaveAttribute('aria-expanded', 'false')
    })
  })

  describe('when the mobile menu is opened', () => {
    let menuButton: HTMLElement
    let dialog: HTMLElement
    beforeEach(async () => {
      document.body.style.overflow = 'auto'
      render(
        <NavBar
          sections={sections}
          account={account}
          balances={{ credits: 30 }}
          onNotificationsClick={() => {}}
          accountActions={[{ label: 'Sign out', onSelect: () => {} }]}
        />
      )
      menuButton = screen.getByRole('button', { name: 'Open menu' })
      await user.click(menuButton)
      dialog = screen.getByRole('dialog', { name: 'Menu' })
    })
    afterEach(() => {
      document.body.style.overflow = ''
    })

    it('should mark the menu button expanded', () => {
      expect(menuButton).toHaveAttribute('aria-expanded', 'true')
    })

    it('should be a modal dialog', () => {
      expect(dialog).toHaveAttribute('aria-modal', 'true')
    })

    it('should move focus into the dialog', () => {
      expect(screen.getByRole('button', { name: 'Close menu' })).toHaveFocus()
    })

    it('should lock the page scroll', () => {
      expect(document.body.style.overflow).toBe('hidden')
    })

    it('should list every section item', () => {
      expect(within(dialog).getByRole('link', { name: 'Blog' })).toHaveAttribute('href', '/blog')
    })

    it('should include the account actions', () => {
      expect(within(dialog).getByRole('button', { name: 'Sign out' })).toBeInTheDocument()
    })

    it('should include the balances', () => {
      expect(within(dialog).getByText(/Credits/)).toBeInTheDocument()
    })

    describe('and Tab is pressed on the last control', () => {
      beforeEach(async () => {
        within(dialog).getByRole('button', { name: 'Sign out' }).focus()
        await user.tab()
      })

      it('should wrap focus to the first control', () => {
        expect(screen.getByRole('button', { name: 'Close menu' })).toHaveFocus()
      })
    })

    describe('and Shift+Tab is pressed on the first control', () => {
      beforeEach(async () => {
        await user.tab({ shift: true })
      })

      it('should wrap focus to the last control', () => {
        expect(within(dialog).getByRole('button', { name: 'Sign out' })).toHaveFocus()
      })
    })

    describe('and Escape is pressed', () => {
      beforeEach(async () => {
        await user.keyboard('{Escape}')
      })

      it('should close the dialog', () => {
        expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
      })

      it('should return focus to the menu button', () => {
        expect(menuButton).toHaveFocus()
      })

      it('should restore the page scroll', () => {
        expect(document.body.style.overflow).toBe('auto')
      })
    })

    describe('and the close button is pressed', () => {
      beforeEach(async () => {
        await user.click(screen.getByRole('button', { name: 'Close menu' }))
      })

      it('should return focus to the menu button', () => {
        expect(menuButton).toHaveFocus()
      })
    })
  })

  describe('when an account action is selected in the mobile menu', () => {
    let menuButton: HTMLElement
    beforeEach(async () => {
      render(
        <NavBar sections={sections} account={account} accountActions={[{ label: 'Sign out', onSelect: () => {} }]} />
      )
      menuButton = screen.getByRole('button', { name: 'Open menu' })
      await user.click(menuButton)
      await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Sign out' }))
    })

    it('should return focus to the menu button', () => {
      expect(menuButton).toHaveFocus()
    })
  })

  describe('when the mobile menu backdrop is pressed', () => {
    beforeEach(async () => {
      const { container } = render(<NavBar sections={sections} onSignIn={() => {}} />)
      await user.click(screen.getByRole('button', { name: 'Open menu' }))
      // The backdrop is aria-hidden, so it has no role to query by.
      fireEvent.click(container.querySelector('[aria-hidden="true"]:not(svg):not(span)')!)
    })

    it('should close the dialog', () => {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    })
  })

  describe('when the viewport reaches desktop with the mobile menu open', () => {
    let listeners: ((event: MediaQueryListEvent) => void)[]
    beforeEach(async () => {
      listeners = []
      vi.stubGlobal(
        'matchMedia',
        vi.fn(() => ({
          matches: false,
          addEventListener: (_: string, listener: (event: MediaQueryListEvent) => void) => listeners.push(listener),
          removeEventListener: () => {}
        }))
      )
      render(<NavBar sections={sections} onSignIn={() => {}} />)
      await user.click(screen.getByRole('button', { name: 'Open menu' }))
      act(() => listeners.forEach((listener) => listener({ matches: true } as MediaQueryListEvent)))
    })
    afterEach(() => {
      vi.unstubAllGlobals()
    })

    it('should close the dialog', () => {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    })
  })

  describe('when the viewport stays mobile with the mobile menu open', () => {
    let listeners: ((event: MediaQueryListEvent) => void)[]
    beforeEach(async () => {
      listeners = []
      vi.stubGlobal(
        'matchMedia',
        vi.fn(() => ({
          matches: false,
          addEventListener: (_: string, listener: (event: MediaQueryListEvent) => void) => listeners.push(listener),
          removeEventListener: () => {}
        }))
      )
      render(<NavBar sections={sections} onSignIn={() => {}} />)
      await user.click(screen.getByRole('button', { name: 'Open menu' }))
      act(() => listeners.forEach((listener) => listener({ matches: false } as MediaQueryListEvent)))
    })
    afterEach(() => {
      vi.unstubAllGlobals()
    })

    it('should keep the dialog open', () => {
      expect(screen.getByRole('dialog')).toBeInTheDocument()
    })
  })

  describe('when both onSignIn and signInHref are given', () => {
    beforeEach(() => {
      render(<NavBar sections={sections} onSignIn={() => {}} signInHref="/sign-in" />)
    })

    it('should render Sign in as a button', () => {
      expect(screen.queryByRole('link', { name: 'Sign in' })).not.toBeInTheDocument()
    })
  })

  describe('when a custom logoHref is given', () => {
    beforeEach(() => {
      render(<NavBar sections={sections} logoHref="/home" />)
    })

    it('should link the logo to it', () => {
      expect(screen.getByRole('link', { name: 'Decentraland home' })).toHaveAttribute('href', '/home')
    })
  })

  describe('when an external signInHref is given with a link component', () => {
    beforeEach(() => {
      render(
        <NavBar
          sections={sections}
          signInHref="https://auth.example.com/"
          linkComponent={(props) => <a {...props} data-router="true" />}
        />
      )
    })

    it('should render it as a plain anchor', () => {
      expect(screen.getByRole('link', { name: 'Sign in' })).not.toHaveAttribute('data-router')
    })
  })

  describe('when a locale is given', () => {
    beforeEach(() => {
      render(<NavBar sections={sections} account={account} balances={{ mana: 1234.5 }} locale="de-DE" />)
    })

    it('should format balances for it', () => {
      expect(screen.getByText(/MANA/).parentElement).toHaveTextContent('MANA: 1.234,5')
    })
  })

  describe('when the account label is overridden', () => {
    beforeEach(() => {
      render(<NavBar sections={sections} account={account} labels={{ account: (name) => `Cuenta de ${name}` }} />)
    })

    it('should name the trigger with it', () => {
      expect(screen.getByRole('button', { name: 'Cuenta de Ada' })).toBeInTheDocument()
    })
  })

  describe('when Sign in is pressed in the mobile menu', () => {
    let onSignIn: Mock<() => void>
    beforeEach(async () => {
      onSignIn = vi.fn<() => void>()
      render(<NavBar sections={sections} onSignIn={onSignIn} />)
      await user.click(screen.getByRole('button', { name: 'Open menu' }))
      await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Sign in' }))
    })

    it('should close the dialog', () => {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    })

    it('should call onSignIn', () => {
      expect(onSignIn).toHaveBeenCalledOnce()
    })
  })

  describe('when a Sign in link is followed in the mobile menu', () => {
    beforeEach(async () => {
      render(<NavBar sections={sections} signInHref="/sign-in" />)
      await user.click(screen.getByRole('button', { name: 'Open menu' }))
      fireEvent.click(within(screen.getByRole('dialog')).getByRole('link', { name: 'Sign in' }))
    })

    it('should close the dialog', () => {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    })
  })

  describe('when the mobile menu is opened while signed out', () => {
    let dialog: HTMLElement
    beforeEach(async () => {
      render(<NavBar sections={sections} onSignIn={() => {}} />)
      await user.click(screen.getByRole('button', { name: 'Open menu' }))
      dialog = screen.getByRole('dialog', { name: 'Menu' })
    })

    it('should include Sign in', () => {
      expect(within(dialog).getByRole('button', { name: 'Sign in' })).toBeInTheDocument()
    })
  })

  describe('when a link component is supplied', () => {
    let LinkComponent: Mock<(props: NavBarLinkProps) => ReactElement>
    beforeEach(() => {
      LinkComponent = vi.fn((props: NavBarLinkProps) => <a {...props} data-router="true" />)
      render(<NavBar sections={sections} onSignIn={() => {}} linkComponent={LinkComponent} />)
    })

    it('should render internal links with it', () => {
      expect(screen.getByRole('link', { name: 'Shop' })).toHaveAttribute('data-router', 'true')
    })

    it('should render external links as plain anchors', () => {
      expect(screen.getByRole('link', { name: 'Docs', hidden: true })).not.toHaveAttribute('data-router')
    })

    it('should not pass external hrefs to it', () => {
      expect(LinkComponent.mock.calls.map(([props]) => props.href)).not.toContain('https://docs.example.com/')
    })
  })

  describe('when a protocol-relative href is given with a link component', () => {
    beforeEach(() => {
      render(
        <NavBar
          sections={[{ label: 'Elsewhere', href: '//elsewhere.example.com/' }]}
          linkComponent={(props) => <a {...props} data-router="true" />}
        />
      )
    })

    it('should render it as a plain anchor', () => {
      expect(screen.getByRole('link', { name: 'Elsewhere' })).not.toHaveAttribute('data-router')
    })
  })

  describe('when labels are overridden', () => {
    beforeEach(() => {
      render(
        <NavBar
          sections={sections}
          onSignIn={() => {}}
          labels={{ navigation: 'Principal', signIn: 'Iniciar sesión' }}
        />
      )
    })

    it('should use the overridden labels', () => {
      expect(screen.getByRole('navigation', { name: 'Principal' })).toHaveTextContent('Iniciar sesión')
    })
  })

  describe('when server-rendered markup is hydrated', () => {
    let recoverable: Mock<(error: unknown) => void>
    let consoleError: MockInstance<typeof console.error>
    let container: HTMLElement
    let root: Root
    const props: NavBarProps = {
      sections,
      account: { ...account, avatarUrl: '/face.png' },
      balances: { credits: 30, mana: 12.5 },
      notificationCount: 4,
      onNotificationsClick: () => {},
      accountActions: [{ label: 'Sign out', onSelect: () => {} }]
    }
    beforeEach(async () => {
      recoverable = vi.fn<(error: unknown) => void>()
      consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
      container = document.createElement('div')
      container.innerHTML = renderToString(<NavBar {...props} />)
      document.body.append(container)
      await act(async () => {
        root = hydrateRoot(container, <NavBar {...props} />, { onRecoverableError: recoverable })
      })
    })
    afterEach(() => {
      act(() => root.unmount())
      consoleError.mockRestore()
      container.remove()
    })

    it('should hydrate without a recoverable error', () => {
      expect(recoverable).not.toHaveBeenCalled()
    })

    it('should hydrate without a console error', () => {
      expect(consoleError).not.toHaveBeenCalled()
    })
  })
})
