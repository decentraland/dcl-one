import { describe, it, expect, vi, afterEach } from 'vitest'
import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderSession, enterAsGuest } from '../harness'
import { AUTO_GRAPHICS_DISMISS_MS, AUTO_GRAPHICS_TITLE, AutoGraphicsBanner, autoGraphicsSummary } from '../../features/autoGraphics/AutoGraphicsBanner'
import type { AutoGraphicsNotice } from '@explorer-core/engine/generated'

const NOTICE: AutoGraphicsNotice = { shadowsOff: true, renderScalePercent: 70, bloomOff: false }
const ALL: AutoGraphicsNotice = { shadowsOff: true, renderScalePercent: 50, bloomOff: true }

const banner = () => screen.queryByRole('status', { name: 'Graphics lowered' })

afterEach(() => vi.useRealTimers())

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
})

describe('the auto graphics banner', () => {
  const withId = (notice: AutoGraphicsNotice, id = 1) => ({ ...notice, id })

  function mount(notice: AutoGraphicsNotice | null, settingsOpen = false) {
    const dismiss = vi.fn()
    const openSettings = vi.fn()
    const view = render(<AutoGraphicsBanner notice={notice && withId(notice)} dismiss={dismiss} settingsOpen={settingsOpen} openSettings={openSettings} />)
    return { dismiss, openSettings, ...view }
  }

  it('words only the parts that changed', () => {
    expect(autoGraphicsSummary(NOTICE)).toBe('Shadows off · Resolution 70%')
    expect(autoGraphicsSummary(ALL)).toBe('Shadows off · Resolution 50% · Bloom off')
    expect(autoGraphicsSummary({ shadowsOff: false, renderScalePercent: null, bloomOff: true })).toBe('Bloom off')
  })

  it('shows nothing without a notice', () => {
    mount(null)
    expect(banner()).toBeNull()
  })

  it('shows the title, what changed and that saved settings are unchanged', () => {
    mount(NOTICE)
    const status = banner()!
    expect(status.textContent).toContain(AUTO_GRAPHICS_TITLE)
    expect(status.textContent).toContain('Shadows off · Resolution 70%')
    expect(status.textContent).toContain("Your saved settings haven't changed")
    expect(status.getAttribute('aria-live')).toBe('polite')
  })

  it('OK dismisses it without opening settings', async () => {
    const h = mount(NOTICE)
    await userEvent.click(screen.getByRole('button', { name: 'OK' }))
    expect(h.dismiss).toHaveBeenCalledOnce()
    expect(h.openSettings).not.toHaveBeenCalled()
  })

  it('Settings opens settings and dismisses it', async () => {
    const h = mount(NOTICE)
    await userEvent.click(screen.getByRole('button', { name: 'Settings' }))
    expect(h.openSettings).toHaveBeenCalledOnce()
    expect(h.dismiss).toHaveBeenCalledOnce()
  })

  it('Settings does not toggle settings shut when they are already open', async () => {
    const h = mount(NOTICE, true)
    await userEvent.click(screen.getByRole('button', { name: 'Settings' }))
    expect(h.openSettings).not.toHaveBeenCalled()
    expect(h.dismiss).toHaveBeenCalledOnce()
  })

  it('dismisses itself after 15 seconds', () => {
    vi.useFakeTimers()
    const h = mount(NOTICE)
    act(() => vi.advanceTimersByTime(AUTO_GRAPHICS_DISMISS_MS - 1))
    expect(h.dismiss).not.toHaveBeenCalled()
    act(() => vi.advanceTimersByTime(1))
    expect(h.dismiss).toHaveBeenCalledOnce()
  })

  it('a new notice restarts the wait', () => {
    vi.useFakeTimers()
    const h = mount(NOTICE)
    act(() => vi.advanceTimersByTime(10_000))
    h.rerender(<AutoGraphicsBanner notice={withId(NOTICE, 2)} dismiss={h.dismiss} settingsOpen={false} openSettings={h.openSettings} />)
    act(() => vi.advanceTimersByTime(10_000))
    expect(h.dismiss).not.toHaveBeenCalled()
    act(() => vi.advanceTimersByTime(5_000))
    expect(h.dismiss).toHaveBeenCalledOnce()
  })
})
