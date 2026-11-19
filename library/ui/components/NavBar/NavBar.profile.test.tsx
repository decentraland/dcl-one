import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { NavBar } from './NavBar'

describe('when the Sites profile panel is open', () => {
  let user: ReturnType<typeof userEvent.setup>
  let trigger: HTMLElement
  let body: HTMLImageElement
  let logout: ReturnType<typeof vi.fn<() => void>>

  beforeEach(async () => {
    user = userEvent.setup()
    logout = vi.fn()
    const { container } = render(
      <NavBar
        sections={[]}
        account={{
          name: 'Ada',
          address: '0x1234567890abcdef1234567890abcdef12345678',
          bodyUrl: '/body.png'
        }}
        accountMenuAppearance="profile"
        accountMenuBalances={{ mana: 101.1, polygonMana: 0 }}
        accountActions={[
          { label: 'View Profile', href: '/profile/ada' },
          { label: 'Logout', onSelect: logout, separated: true }
        ]}
      />
    )
    trigger = screen.getByRole('button', { name: 'Account: Ada' })
    await user.click(trigger)
    body = container.querySelector<HTMLImageElement>('img[src="/body.png"]')!
  })

  it('should render the full-body snapshot and the supplied balances', () => {
    expect(body).toBeInTheDocument()
    expect(screen.getByText('101.1')).toBeVisible()
    expect(screen.getByText('0')).toBeVisible()
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

  it('should drop an unavailable body snapshot', () => {
    fireEvent.error(body)
    expect(body).not.toBeInTheDocument()
  })

  it('should dismiss the profile and restore focus on Escape', async () => {
    await user.tab()
    await user.keyboard('{Escape}')
    expect(trigger).toHaveAttribute('aria-expanded', 'false')
    expect(trigger).toHaveFocus()
  })

  it('should invoke the existing logout action', async () => {
    await user.click(screen.getByRole('button', { name: 'Logout' }))
    expect(logout).toHaveBeenCalledOnce()
  })
})
