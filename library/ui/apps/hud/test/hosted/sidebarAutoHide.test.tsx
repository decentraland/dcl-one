import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { act, fireEvent, screen } from '@testing-library/react'
import { Sidebar } from '../../features/sidebar/Sidebar'
import { renderSettled, fakeSession } from '../harness'

const nav = (): HTMLElement => screen.getByRole('navigation', { name: 'Main navigation' })

describe('sidebar auto-hide (Unity SidebarConfigPanel)', () => {
  beforeEach(() => localStorage.clear())
  afterEach(() => vi.useRealTimers())

  it('the ••• button leads the rail and opens the auto-hide setting', async () => {
    await renderSettled(<Sidebar session={fakeSession()} />)
    expect(screen.getAllByRole('button')[0]).toHaveAccessibleName('Sidebar settings')
    fireEvent.click(screen.getByRole('button', { name: 'Sidebar settings' }))
    expect(screen.getByRole('switch', { name: 'Hide Sidebar Automatically' })).not.toBeChecked()
  })

  it('hides 0.3s after the pointer leaves and returns 0.3s after reaching the left edge', async () => {
    vi.useFakeTimers()
    await renderSettled(<Sidebar session={fakeSession()} />)
    fireEvent.click(screen.getByRole('button', { name: 'Sidebar settings' }))
    fireEvent.click(screen.getByRole('switch', { name: 'Hide Sidebar Automatically' }))
    fireEvent.click(screen.getByRole('button', { name: 'Sidebar settings' }))

    fireEvent.pointerLeave(nav())
    act(() => vi.advanceTimersByTime(250))
    expect(nav()).toHaveAttribute('data-hidden', 'false')
    act(() => vi.advanceTimersByTime(100))
    expect(nav()).toHaveAttribute('data-hidden', 'true')

    fireEvent.pointerEnter(screen.getByTestId('sidebar-reveal'))
    act(() => vi.advanceTimersByTime(300))
    expect(nav()).toHaveAttribute('data-hidden', 'false')
  })

  it('hides once the panel closes, even though the pointer left the rail while it was open', async () => {
    vi.useFakeTimers()
    await renderSettled(<Sidebar session={fakeSession()} />)
    fireEvent.pointerEnter(nav())
    fireEvent.click(screen.getByRole('button', { name: 'Sidebar settings' }))
    fireEvent.pointerLeave(nav())
    fireEvent.click(screen.getByRole('switch', { name: 'Hide Sidebar Automatically' }))
    fireEvent.pointerDown(document.body)
    act(() => vi.advanceTimersByTime(350))
    expect(nav()).toHaveAttribute('data-hidden', 'true')
  })

  it('stays up while a panel of the bar is open, and hides once it closes with the pointer away', async () => {
    vi.useFakeTimers()
    await renderSettled(<Sidebar session={fakeSession()} />)
    fireEvent.click(screen.getByRole('button', { name: 'Sidebar settings' }))
    fireEvent.click(screen.getByRole('switch', { name: 'Hide Sidebar Automatically' }))
    fireEvent.click(screen.getByRole('button', { name: 'Sidebar settings' }))

    fireEvent.pointerEnter(nav())
    fireEvent.click(screen.getByRole('button', { name: 'Discover' }))
    fireEvent.pointerLeave(nav())
    act(() => vi.advanceTimersByTime(350))
    expect(screen.getByRole('dialog', { name: 'Discover' })).toBeInTheDocument()
    expect(nav()).toHaveAttribute('data-hidden', 'false')

    fireEvent.pointerDown(document.body)
    expect(screen.queryByRole('dialog', { name: 'Discover' })).toBeNull()
    act(() => vi.advanceTimersByTime(350))
    expect(nav()).toHaveAttribute('data-hidden', 'true')
  })

  it('remembers the choice across remounts (the rail unmounts under full-screen pages)', async () => {
    const { unmount } = await renderSettled(<Sidebar session={fakeSession()} />)
    fireEvent.click(screen.getByRole('button', { name: 'Sidebar settings' }))
    fireEvent.click(screen.getByRole('switch', { name: 'Hide Sidebar Automatically' }))
    unmount()
    await renderSettled(<Sidebar session={fakeSession()} />)
    fireEvent.click(screen.getByRole('button', { name: 'Sidebar settings' }))
    expect(screen.getByRole('switch', { name: 'Hide Sidebar Automatically' })).toBeChecked()
  })
})
