import { describe, it, expect, vi } from 'vitest'

let isNative = false
vi.mock('@hud-host', async (importOriginal) => ({ ...(await importOriginal<object>()), isNativeHud: () => isNative }))
import { act, render, renderHook, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderSession, enterAsGuest } from '../harness'
import { PLATFORM_BAN_TITLE, banLines, banPeriod, openPlatformBanDialog, usePlatformBanNotices } from '../../features/session/PlatformBanDialog'
import type { SessionPhase } from '../../features/session/useEngineSession'
import { PopupHost, closeTopPopup, resetPopups } from '../../design'
import { PLATFORM_BANNED_REFUSAL } from '@explorer-core/engine/protocol'
import type { PlatformBanNotice } from '@explorer-core/engine/generated'

const BAN: PlatformBanNotice = { banned: true, reason: 'spam', customMessage: 'go away', expiresAt: null }
const NOW = Date.parse('2026-11-13T00:00:00.000Z')

// DOMAIN: the platform ban — the engine's notice is kept, and a travel refused for it is told
// apart from any other refused travel.
describe('platform ban domain', () => {
  it('keeps a ban and drops a not-banned notice', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    expect(h.session().platformBan.notice).toBeNull()
    h.driver.emit({ kind: 'platformBan', ban: BAN })
    await waitFor(() => expect(h.session().platformBan.notice).toEqual(BAN))
    h.driver.emit({ kind: 'platformBan', ban: { banned: false, reason: '', customMessage: null, expiresAt: null } })
    await waitFor(() => expect(h.session().platformBan.notice).toBeNull())
  })

  it('a lobby pick refused for the ban stays in the lobby with no travel error', async () => {
    const h = renderSession()
    await waitFor(() => expect(h.session().login.status).not.toBe('loading'))
    act(() => h.session().login.exploreAsGuest())
    await waitFor(() => expect(h.session().phase).toBe('lobby'))
    await waitFor(() => expect(h.session().login.busy).toBe(false))
    h.driver.emit({ kind: 'platformBan', ban: BAN })
    act(() => h.session().pickDestination(null))
    const go = h.driver.last('teleport')!
    h.driver.emit({ kind: 'travelResult', travelId: go.travelId!, realm: go.realm!, ok: false, message: PLATFORM_BANNED_REFUSAL })
    await waitFor(() => expect(h.session().phase).toBe('lobby'))
    expect(h.session().travelError).toBeNull()
    expect(h.session().platformBan.travelRefused).toBe(true)
  })

  it('sign-out after a lobby pick refused for the ban clears the ban and the refusal', async () => {
    const h = renderSession()
    await waitFor(() => expect(h.session().login.status).not.toBe('loading'))
    act(() => h.session().login.exploreAsGuest())
    await waitFor(() => expect(h.session().phase).toBe('lobby'))
    await waitFor(() => expect(h.session().login.busy).toBe(false))
    h.driver.emit({ kind: 'platformBan', ban: BAN })
    act(() => h.session().pickDestination(null))
    const go = h.driver.last('teleport')!
    h.driver.emit({ kind: 'travelResult', travelId: go.travelId!, realm: go.realm!, ok: false, message: PLATFORM_BANNED_REFUSAL })
    await waitFor(() => expect(h.session().platformBan.travelRefused).toBe(true))
    act(() => h.session().logout())
    await waitFor(() => expect(h.session().phase).toBe('login'))
    expect(h.session().platformBan.notice).toBeNull()
    expect(h.session().platformBan.travelRefused).toBe(false)
  })

  it('a ban refusal that answers after sign-out is dropped', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().map.changeRealm('other.dcl.eth'))
    const go = h.driver.last('changeRealm')!
    act(() => h.session().logout())
    await waitFor(() => expect(h.session().phase).toBe('login'))
    h.driver.emit({ kind: 'travelResult', travelId: go.travelId!, realm: go.realm, ok: false, message: PLATFORM_BANNED_REFUSAL })
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(h.session().platformBan.travelRefused).toBe(false)
    expect(h.session().travellingTo).toBeNull()
  })

  it('an in-world travel refused for the ban is not a travel error', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().map.changeRealm('other.dcl.eth'))
    const go = h.driver.last('changeRealm')!
    h.driver.emit({ kind: 'travelResult', travelId: go.travelId!, realm: go.realm, ok: false, message: PLATFORM_BANNED_REFUSAL })
    await waitFor(() => expect(h.session().platformBan.travelRefused).toBe(true))
    expect(h.session().travelError).toBeNull()
    act(() => h.session().platformBan.dismissTravelRefused())
    expect(h.session().platformBan.travelRefused).toBe(false)
  })

  it('any other refusal stays a travel error', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().map.changeRealm('other.dcl.eth'))
    const go = h.driver.last('changeRealm')!
    h.driver.emit({ kind: 'travelResult', travelId: go.travelId!, realm: go.realm, ok: false, message: 'world not found' })
    await waitFor(() => expect(h.session().travelError).toContain('world not found'))
    expect(h.session().platformBan.travelRefused).toBe(false)
  })
})

describe('the ban notice', () => {
  it('words the period in hours under a day, else days, rounded up', () => {
    expect(banPeriod('2026-11-13T00:30:00.000Z', NOW)).toBe('1 hour')
    expect(banPeriod('2026-11-13T05:10:00.000Z', NOW)).toBe('6 hours')
    expect(banPeriod('2026-11-14T00:00:00.000Z', NOW)).toBe('1 day')
    expect(banPeriod('2026-11-16T01:00:00.000Z', NOW)).toBe('4 days')
    expect(banPeriod('2026-11-12T00:00:00.000Z', NOW)).toBe('expired')
    expect(banPeriod('soon', NOW)).toBe('soon')
  })

  it('shows what the record carries, then where to get help', () => {
    expect(banLines({ ...BAN, expiresAt: '2026-11-15T00:00:00.000Z' }, NOW)).toEqual([
      'Ban period: 2 days.',
      'Reason: spam.',
      'go away',
      'Contact Support for more information.'
    ])
    expect(banLines({ banned: true, reason: '', customMessage: null, expiresAt: null }, NOW)).toEqual(['Contact Support for more information.'])
  })

  it('blocking: Escape stays, Sign Out signs out', async () => {
    resetPopups()
    render(<PopupHost />)
    const signOut = vi.fn()
    act(() => {
      openPlatformBanDialog(BAN, { signOut })
    })
    const dialog = await screen.findByRole('alertdialog', { name: PLATFORM_BAN_TITLE })
    expect(dialog.textContent).toContain('Reason: spam.')
    act(() => closeTopPopup())
    expect(screen.getByRole('alertdialog', { name: PLATFORM_BAN_TITLE })).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'OK' })).toBeNull()
    await userEvent.click(screen.getByRole('button', { name: 'Sign Out' }))
    expect(signOut).toHaveBeenCalledOnce()
    await waitFor(() => expect(screen.queryByRole('alertdialog', { name: PLATFORM_BAN_TITLE })).toBeNull())
  })

  it('an open notice words the period again as time passes', () => {
    vi.useFakeTimers({ now: NOW })
    try {
      resetPopups()
      render(<PopupHost />)
      act(() => {
        openPlatformBanDialog({ ...BAN, expiresAt: '2026-11-13T02:30:00.000Z' }, { signOut: vi.fn() })
      })
      const dialog = screen.getByRole('alertdialog', { name: PLATFORM_BAN_TITLE })
      expect(dialog.textContent).toContain('Ban period: 3 hours.')
      act(() => vi.advanceTimersByTime(90 * 60_000))
      expect(dialog.textContent).toContain('Ban period: 1 hour.')
    } finally {
      vi.useRealTimers()
    }
  })

  it('after a refused travel: OK closes it', async () => {
    resetPopups()
    render(<PopupHost />)
    const onClose = vi.fn()
    act(() => {
      openPlatformBanDialog(BAN, { onClose })
    })
    await screen.findByRole('alertdialog', { name: PLATFORM_BAN_TITLE })
    expect(screen.queryByRole('button', { name: 'Sign Out' })).toBeNull()
    await userEvent.click(screen.getByRole('button', { name: 'OK' }))
    expect(onClose).toHaveBeenCalledOnce()
  })
})

// The notices the HUD opens for the ban, by phase.
describe('the ban notices', () => {
  type Input = { phase: SessionPhase; notice: PlatformBanNotice | null; travelRefused: boolean }

  function mount(initial: Input) {
    resetPopups()
    render(<PopupHost />)
    const dismissTravelRefused = vi.fn()
    const signOut = vi.fn()
    const openExternalUrl = vi.fn(async (_url: string) => true)
    const hook = renderHook((input: Input) => usePlatformBanNotices({ ...input, dismissTravelRefused, signOut, openExternalUrl }), { initialProps: initial })
    return { ...hook, dismissTravelRefused, signOut, openExternalUrl }
  }

  const dialog = () => screen.queryByRole('alertdialog', { name: PLATFORM_BAN_TITLE })
  const blocking = () => screen.queryByRole('button', { name: 'Sign Out' }) != null

  it('blocks the lobby, and a refused pick under it is dismissed', async () => {
    const h = mount({ phase: 'lobby', notice: BAN, travelRefused: false })
    await waitFor(() => expect(dialog()).not.toBeNull())
    expect(blocking()).toBe(true)
    h.rerender({ phase: 'lobby', notice: BAN, travelRefused: true })
    await waitFor(() => expect(h.dismissTravelRefused).toHaveBeenCalled())
    expect(blocking()).toBe(true)
  })

  it('Sign Out signs out and opens nothing over sign-in', async () => {
    const h = mount({ phase: 'lobby', notice: BAN, travelRefused: true })
    await waitFor(() => expect(dialog()).not.toBeNull())
    await userEvent.click(screen.getByRole('button', { name: 'Sign Out' }))
    expect(h.signOut).toHaveBeenCalledOnce()
    await waitFor(() => expect(dialog()).toBeNull())
    // sign-in before the session has cleared the ban and the refusal
    h.rerender({ phase: 'login', notice: BAN, travelRefused: true })
    expect(dialog()).toBeNull()
  })

  it('Contact Support on the native HUD asks the engine to open the help page', async () => {
    isNative = true
    const open = vi.spyOn(window, 'open')
    try {
      const h = mount({ phase: 'world', notice: BAN, travelRefused: true })
      await waitFor(() => expect(dialog()).not.toBeNull())
      await userEvent.click(screen.getByRole('button', { name: 'Contact Support' }))
      expect(h.openExternalUrl).toHaveBeenCalledOnce()
      expect(h.openExternalUrl.mock.calls[0][0]).toMatch(/\/help\/$/)
      expect(open).not.toHaveBeenCalled()
    } finally {
      open.mockRestore()
      isNative = false
    }
  })

  it('stays open when entry was under way and the world loads', async () => {
    const h = mount({ phase: 'entering', notice: BAN, travelRefused: false })
    await waitFor(() => expect(dialog()).not.toBeNull())
    h.rerender({ phase: 'world', notice: BAN, travelRefused: false })
    expect(dialog()).not.toBeNull()
    expect(blocking()).toBe(true)
    h.rerender({ phase: 'world', notice: null, travelRefused: false })
    await waitFor(() => expect(dialog()).toBeNull())
  })

  it('an in-world travel through the loader opens only the dismissible notice', async () => {
    const h = mount({ phase: 'world', notice: BAN, travelRefused: false })
    h.rerender({ phase: 'entering', notice: BAN, travelRefused: false })
    expect(dialog()).toBeNull()
    h.rerender({ phase: 'world', notice: BAN, travelRefused: true })
    await waitFor(() => expect(dialog()).not.toBeNull())
    expect(blocking()).toBe(false)
  })

  it('a new sign-in blocks again before the world', async () => {
    const h = mount({ phase: 'world', notice: null, travelRefused: false })
    h.rerender({ phase: 'login', notice: null, travelRefused: false })
    h.rerender({ phase: 'lobby', notice: BAN, travelRefused: false })
    await waitFor(() => expect(dialog()).not.toBeNull())
    expect(blocking()).toBe(true)
  })

  it('in the world, a refused travel opens the dismissible notice', async () => {
    mount({ phase: 'world', notice: BAN, travelRefused: true })
    await waitFor(() => expect(dialog()).not.toBeNull())
    expect(blocking()).toBe(false)
  })
})
