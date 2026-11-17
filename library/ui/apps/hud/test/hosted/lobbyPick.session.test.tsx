// The startup lobby's pages: a page opened over it that travels picks where the world starts.
import { act, waitFor } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { renderSession } from '../harness'

describe('lobby: a pick from a page over the startup lobby', () => {
  it('picks where the world starts from a page opened over the startup lobby', async () => {
    const h = renderSession()
    await waitFor(() => expect(h.session().login.status).toBe('sign-in-or-guest'))
    act(() => h.session().login.exploreAsGuest())
    await waitFor(() => expect(h.session().phase).toBe('lobby'))
    await waitFor(() => expect(h.session().login.busy).toBe(false))

    act(() => h.session().map.toggle())
    expect(h.session().map.open).toBe(true)
    act(() => h.session().map.teleport(5, 6))
    expect(h.session().map.open).toBe(false)
    expect(h.session().phase).toBe('entering')
    expect(h.driver.last('teleport')).toMatchObject({ x: 5, y: 6 })
  })

  it('refuses a pick from a page over the startup lobby while sign-in finishes, and keeps the page', async () => {
    const h = renderSession()
    Object.assign(h.driver, { loginGuest: () => new Promise(() => undefined), loginWithIdentity: () => new Promise(() => undefined) })
    await waitFor(() => expect(h.session().login.status).toBe('sign-in-or-guest'))
    act(() => h.session().login.exploreAsGuest())
    // an existing account with the terms accepted: no welcome page
    act(() => h.driver.emit({ kind: 'welcome', terms: false, newProfile: false }))
    await waitFor(() => expect(h.session().phase).toBe('lobby'))
    expect(h.session().login.busy).toBe(true)

    act(() => h.session().map.toggle())
    let picked: boolean | undefined
    act(() => {
      picked = h.session().map.teleportToPlace(5, 6)
    })
    expect(picked).toBe(false)
    expect(h.session().map.open).toBe(true)
    expect(h.session().phase).toBe('lobby')
    expect(h.driver.sentOf('teleport')).toHaveLength(0)
  })

  it('changes realm into a world from a page opened over the startup lobby', async () => {
    const h = renderSession()
    await waitFor(() => expect(h.session().login.status).toBe('sign-in-or-guest'))
    act(() => h.session().login.exploreAsGuest())
    await waitFor(() => expect(h.session().phase).toBe('lobby'))
    await waitFor(() => expect(h.session().login.busy).toBe(false))

    act(() => h.session().places.toggle())
    act(() => h.session().map.changeRealm('mana.dcl.eth'))
    expect(h.session().places.open).toBe(false)
    expect(h.session().phase).toBe('entering')
    expect(h.driver.last('changeRealm')).toMatchObject({ realm: 'mana.dcl.eth' })
  })
})
