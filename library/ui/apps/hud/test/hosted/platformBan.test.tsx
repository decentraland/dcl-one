import { describe, it, expect, vi } from 'vitest'
import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderSession, enterAsGuest } from '../harness'
import { PLATFORM_BAN_TITLE, banLines, banPeriod, openPlatformBanDialog } from '../../features/session/PlatformBanDialog'
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
