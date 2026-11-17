// DOMAIN: the page→shell protocol behind the Google Ads conversion — the engine's "First Move In
// World" Segment event (relayed as an `analytics` message) posts `bevy-shell:in-world` with the login
// type and the analytics session, once per page load; reaching the world alone does not.

import { describe, expect, it, vi } from 'vitest'
import { act, waitFor } from '@testing-library/react'
import { sessionId } from '@explorer-core/lib/analytics'
import { postToShell } from '@explorer-core/lib/shell'
import { enterAsGuest, renderSession } from '../harness'

vi.mock('@explorer-core/lib/shell', () => ({ inShell: true, postToShell: vi.fn(), shellRequest: vi.fn(async () => null) }))

const inWorldPosts = (): unknown[] => vi.mocked(postToShell).mock.calls.map(([m]) => m).filter((m) => m.type === 'bevy-shell:in-world')
const sessionPosts = (): unknown[] => vi.mocked(postToShell).mock.calls.map(([m]) => m).filter((m) => m.type === 'bevy-shell:session')

// The shell asks before the page goes only while a session runs (apps/ui3 shell/mount.ts).
describe('the session word to the shell', () => {
  it('says no session on the start screen, one from the sign-in on, and none again after the sign-out', async () => {
    vi.mocked(postToShell).mockClear()
    const h = renderSession({ userId: null })
    await waitFor(() => expect(sessionPosts()).toEqual([{ type: 'bevy-shell:session', live: false }]))
    await enterAsGuest(h)
    expect(sessionPosts().at(-1)).toEqual({ type: 'bevy-shell:session', live: true })
    act(() => h.session().logout())
    await waitFor(() => expect(sessionPosts().at(-1)).toEqual({ type: 'bevy-shell:session', live: false }))
    expect(sessionPosts()).toHaveLength(3)
  })
})

describe('in-world conversion', () => {
  it('posts bevy-shell:in-world on the first move, once per page load, with the login type and the session', async () => {
    const h = renderSession({ userId: null })
    await enterAsGuest(h)
    // in world, standing still: Jump In is not the conversion, nor is any other engine event
    h.driver.emit({ kind: 'analytics', event: 'Explorer Move To Parcel', properties: { position: '1,1' } })
    expect(inWorldPosts()).toEqual([])

    h.driver.emit({ kind: 'analytics', event: 'First Move In World', properties: {} })
    expect(inWorldPosts()).toEqual([{ type: 'bevy-shell:in-world', login_type: 'guest', session_id: sessionId() }])

    // the engine reports a first move per login; the page reports once per load
    h.driver.emit({ kind: 'analytics', event: 'First Move In World', properties: {} })
    expect(inWorldPosts()).toHaveLength(1)
  })

  // ?guest=1 skips the welcome page, so nothing has said the terms are accepted on this install
  it('does not post without the terms accepted', async () => {
    vi.mocked(postToShell).mockClear()
    window.history.replaceState(null, '', '/?guest=1&position=10,20')
    const h = renderSession({ userId: null })
    await waitFor(() => expect(h.driver.calls).toContain('loginGuest'))
    h.driver.emit({ kind: 'event', name: 'playerReady' })
    h.driver.emit({ kind: 'sceneLoading', state: { visible: false, realmConnected: true, title: '', pendingAssets: null } })
    await waitFor(() => expect(h.session().phase).toBe('world'))
    h.driver.emit({ kind: 'analytics', event: 'First Move In World', properties: {} })
    expect(inWorldPosts()).toEqual([])
    window.history.replaceState(null, '', '/')
  })
})
