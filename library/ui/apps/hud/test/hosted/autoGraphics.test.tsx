import { describe, it, expect, vi, afterEach } from 'vitest'
import { act, render, renderHook, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderSession, enterAsGuest } from '../harness'
import { ToastHost, dismissToast } from '../../design'
import { getSettingsTab, setSettingsTab } from '../../features/settings/settingsTab'
import {
  AUTO_GRAPHICS_TITLE,
  AUTO_GRAPHICS_TOAST_KEY,
  AUTO_GRAPHICS_TOAST_MS,
  autoGraphicsSummary,
  useAutoGraphicsToast
} from '../../features/autoGraphics/autoGraphicsToast'
import type { AutoGraphicsNotice } from '@explorer-core/engine/generated'

const NOTICE: AutoGraphicsNotice = { shadowsOff: true, renderScalePercent: 70, bloomOff: false }
const ALL: AutoGraphicsNotice = { shadowsOff: true, renderScalePercent: 50, bloomOff: true }

const toast = () => screen.queryByRole('button', { name: 'Open graphics settings' })

afterEach(() => {
  act(() => dismissToast(AUTO_GRAPHICS_TOAST_KEY))
  setSettingsTab(null)
  vi.useRealTimers()
})

// DOMAIN: Auto Graphics — each notice from the engine is kept with a new id, and sign-out clears it.
describe('auto graphics domain', () => {
  it('keeps the latest notice, gives each one a new id, and dismisses it', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    expect(h.session().autoGraphics.notice).toBeNull()
    h.driver.emit({ kind: 'autoGraphics', notice: NOTICE })
    await waitFor(() => expect(h.session().autoGraphics.notice).toMatchObject(NOTICE))
    const first = h.session().autoGraphics.notice!.id
    h.driver.emit({ kind: 'autoGraphics', notice: NOTICE })
    await waitFor(() => expect(h.session().autoGraphics.notice!.id).toBeGreaterThan(first))
    act(() => h.session().autoGraphics.dismiss())
    expect(h.session().autoGraphics.notice).toBeNull()
  })

  it('sign-out clears the notice', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit({ kind: 'autoGraphics', notice: NOTICE })
    await waitFor(() => expect(h.session().autoGraphics.notice).not.toBeNull())
    act(() => h.session().logout())
    await waitFor(() => expect(h.session().phase).toBe('login'))
    expect(h.session().autoGraphics.notice).toBeNull()
  })

  it('openOn opens Settings on the given tab, and changes the tab of Settings already open', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().settings.openOn('Performance'))
    await waitFor(() => expect(h.session().settings.open).toBe(true))
    expect(getSettingsTab()).toBe('Performance')
    act(() => h.session().settings.openOn('Audio'))
    expect(h.session().settings.open).toBe(true)
    expect(getSettingsTab()).toBe('Audio')
  })
})

describe('the auto graphics toast', () => {
  const withId = (notice: AutoGraphicsNotice, id = 1) => ({ ...notice, id })

  function mount(notice: AutoGraphicsNotice | null, active = true) {
    const dismiss = vi.fn()
    const openSettings = vi.fn()
    render(<ToastHost />)
    const hook = renderHook(
      ({ notice, active }) => useAutoGraphicsToast({ notice: notice && withId(notice), dismiss }, active, openSettings),
      { initialProps: { notice, active } }
    )
    return { dismiss, openSettings, ...hook }
  }

  it('words only the parts that changed', () => {
    expect(autoGraphicsSummary(NOTICE)).toBe('Shadows off · Resolution 70%')
    expect(autoGraphicsSummary(ALL)).toBe('Shadows off · Resolution 50% · Bloom off')
    expect(autoGraphicsSummary({ shadowsOff: false, renderScalePercent: null, bloomOff: true })).toBe('Bloom off')
  })

  it('shows nothing without a notice', () => {
    mount(null)
    expect(toast()).toBeNull()
  })

  it('shows the title, what changed and that saved settings are unchanged, and takes the notice', () => {
    const h = mount(NOTICE)
    const status = screen.getByRole('status')
    expect(status.textContent).toContain(AUTO_GRAPHICS_TITLE)
    expect(status.textContent).toContain('Shadows off · Resolution 70%')
    expect(status.textContent).not.toContain('Bloom off')
    expect(status.textContent).toContain("Your saved settings haven't changed")
    expect(h.dismiss).toHaveBeenCalledOnce()
  })

  it('drops a notice that arrives outside the world or with the chrome hidden', () => {
    const h = mount(NOTICE, false)
    expect(toast()).toBeNull()
    expect(h.dismiss).toHaveBeenCalledOnce()
    // the notice is gone, so it does not pop up when the chrome returns
    h.rerender({ notice: null, active: true })
    expect(toast()).toBeNull()
  })

  it('takes the toast down when the chrome goes away', () => {
    const h = mount(NOTICE)
    expect(toast()).not.toBeNull()
    h.rerender({ notice: null, active: false })
    expect(toast()).toBeNull()
  })

  it('Settings opens the Performance tab and takes the toast down', async () => {
    const h = mount(NOTICE)
    await userEvent.click(screen.getByRole('button', { name: 'Open graphics settings' }))
    expect(h.openSettings).toHaveBeenCalledExactlyOnceWith('Performance')
    expect(toast()).toBeNull()
  })

  it('Dismiss takes the toast down without opening settings', async () => {
    const h = mount(NOTICE)
    await userEvent.click(screen.getByRole('button', { name: 'Dismiss' }))
    expect(toast()).toBeNull()
    expect(h.openSettings).not.toHaveBeenCalled()
  })

  it('goes after 15 seconds', () => {
    vi.useFakeTimers()
    mount(NOTICE)
    act(() => vi.advanceTimersByTime(AUTO_GRAPHICS_TOAST_MS - 1))
    expect(toast()).not.toBeNull()
    act(() => vi.advanceTimersByTime(1))
    expect(toast()).toBeNull()
  })

  it('a new notice replaces the toast and restarts the wait', () => {
    vi.useFakeTimers()
    const h = mount(NOTICE)
    act(() => vi.advanceTimersByTime(10_000))
    h.rerender({ notice: ALL, active: true })
    expect(screen.getAllByRole('status')).toHaveLength(1)
    expect(screen.getByRole('status').textContent).toContain('Bloom off')
    act(() => vi.advanceTimersByTime(10_000))
    expect(toast()).not.toBeNull()
    act(() => vi.advanceTimersByTime(5_000))
    expect(toast()).toBeNull()
  })
})
