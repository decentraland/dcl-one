import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { NavBar, type NavBarProps } from './NavBar'

describe('when the Sites profile panel is open', () => {
  let user: ReturnType<typeof userEvent.setup>
  let trigger: HTMLElement
  let body: HTMLImageElement
  let logout: ReturnType<typeof vi.fn<() => void>>
  let onOpenChange: ReturnType<typeof vi.fn<(open: boolean) => void>>
  let widget: ReturnType<typeof render>
  let props: NavBarProps
  let panel: HTMLElement

  beforeEach(async () => {
    user = userEvent.setup()
    logout = vi.fn()
    onOpenChange = vi.fn()
    props = {
      sections: [],
      account: {
        name: 'Ada',
        address: '0x1234567890abcdef1234567890abcdef12345678',
        bodyUrl: '/body.png'
      },
      accountMenuAppearance: 'profile',
      accountMenuBalances: { mana: 101.1, polygonMana: 0 },
      balances: { credits: 0 },
      accountActions: [
        { label: 'View Profile', href: '/profile/ada' },
        { label: 'Logout', onSelect: logout, separated: true }
      ],
      onAccountMenuOpenChange: onOpenChange
    }
    widget = render(<NavBar {...props} />)
    trigger = screen.getByRole('button', { name: 'Account: Ada' })
    await user.click(trigger)
    panel = document.getElementById(trigger.getAttribute('aria-controls')!)!
    body = panel.querySelector<HTMLImageElement>('img[src="/body.png"]')!
  })

  it('should render only the supplied profile balances beside the body snapshot', () => {
    expect(body).toBeInTheDocument()
    expect(panel.querySelector('[data-balance="mana"]')).toHaveTextContent('MANA: 101.1')
    expect(panel.querySelector('[data-balance="polygonMana"]')).toHaveTextContent('Polygon MANA: 0')
    expect(panel.querySelector('[data-balance="credits"]')).toBeNull()
  })

  it('should leave the panel without balances when only the header has Credits', () => {
    widget.rerender(<NavBar {...props} accountMenuBalances={undefined} />)
    expect(panel.querySelector('[data-balance]')).toBeNull()
    expect(widget.container.querySelector('[data-balance="credits"]')).toBeInTheDocument()
  })

  it('should copy the full user ID and announce success', async () => {
    const button = screen.getByRole('button', { name: 'Copy user ID' })
    expect(screen.getByText('0x1234...5678')).toBeVisible()
    await user.click(button)
    expect(await navigator.clipboard.readText()).toBe('0x1234567890abcdef1234567890abcdef12345678')
    expect(screen.getByRole('status')).toHaveTextContent('User ID copied')
    expect(screen.getByText('Address copied!')).toBeVisible()
    expect(screen.queryByText('0x1234...5678')).toBeNull()
    expect(screen.getByRole('button', { name: 'Copy user ID' })).toBe(button)
    expect(screen.queryByText('Copied')).toBeNull()
    await user.keyboard('{Escape}')
    await user.click(trigger)
    expect(screen.queryByText('Address copied!')).toBeNull()
    expect(screen.getByText('0x1234...5678')).toBeVisible()
  })

  it('should show the localized copy confirmation', async () => {
    widget.rerender(<NavBar {...props} labels={{ addressCopied: 'Dirección copiada' }} />)
    await user.click(screen.getByRole('button', { name: 'Copy user ID' }))
    expect(screen.getByText('Dirección copiada')).toBeVisible()
  })

  it('should drop an unavailable body snapshot', () => {
    fireEvent.error(body)
    expect(body).not.toBeInTheDocument()
  })

  it('should dismiss the profile and restore focus on Escape', async () => {
    await user.tab()
    await user.keyboard('{Escape}')
    expect(trigger).toHaveAttribute('aria-expanded', 'false')
    expect(trigger).toHaveFocus()
    expect(onOpenChange.mock.calls).toEqual([[true], [false]])
  })

  it('should invoke the existing logout action', async () => {
    await user.click(screen.getByRole('button', { name: 'Logout' }))
    expect(logout).toHaveBeenCalledOnce()
    expect(onOpenChange.mock.calls).toEqual([[true], [false]])
  })

  it('should report a closed panel after an outside press', () => {
    fireEvent.pointerDown(document.body)
    expect(onOpenChange.mock.calls).toEqual([[true], [false]])
  })

  it('should report a closed panel after clicking the trigger again', async () => {
    await user.click(trigger)
    expect(onOpenChange.mock.calls).toEqual([[true], [false]])
  })

  it('should report a closed panel after removing an open account menu', () => {
    widget.rerender(<NavBar sections={[]} signIn={{ href: '/login' }} />)
    expect(onOpenChange.mock.calls).toEqual([[true], [false]])
  })
})
