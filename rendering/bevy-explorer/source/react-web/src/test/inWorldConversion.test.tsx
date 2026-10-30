// DOMAIN: the page→shell protocol behind the Google Ads conversion — the engine's "First Move In
// World" Segment event (relayed as an `analytics` message) posts `bevy-shell:in-world` with the login
// type and the analytics session, once per page load; reaching the world alone does not.

import { describe, expect, it, vi } from 'vitest'
import { waitFor } from '@testing-library/react'
import { sessionId } from '../lib/analytics'
import { postToShell } from '../lib/shell'
import { enterAsGuest, renderSession } from './harness'

vi.mock('../lib/shell', () => ({ inShell: true, postToShell: vi.fn(), shellRequest: vi.fn(async () => null) }))

const inWorldPosts = (): unknown[] => vi.mocked(postToShell).mock.calls.map(([m]) => m).filter((m) => m.type === 'bevy-shell:in-world')

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
