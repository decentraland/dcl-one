import { describe, it, expect } from 'vitest'
import { act } from '@testing-library/react'
import type { Profile } from '../engine/protocol'
import { renderSession, enterAsGuest } from './harness'
import { peekProfile, requestPassport, subscribeProfile } from '../features/session/profileStore'

// DOMAIN: profile — the local player's passport, fetched on world entry.
describe('profile domain', () => {
  const profile: Profile = {
    address: '0xme',
    name: 'Tester',
    hasClaimedName: true,
    isGuest: false,
    description: 'hi'
  }

  it('requests the profile on world entry', async () => {
    const h = renderSession()
    await enterAsGuest(h, { keepSent: true })
    expect(h.driver.sentOf('getProfile')).toHaveLength(1)
  })

  it('profile stream populates the passport', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit({ kind: 'profile', profile })
    expect(h.session().profile.data).toMatchObject({ address: '0xme', name: 'Tester' })
  })

  it('a passport request goes to the bridge with extras, and the reply lands in the profile store', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => requestPassport('0xKURD'))
    expect(h.driver.last('getUserProfile')).toEqual({ kind: 'getUserProfile', address: '0xkurd', extras: true })
    h.driver.emit({
      kind: 'userProfile',
      address: '0xKURD',
      profile: { address: '0xkurd', name: 'kurd', hasClaimedName: true, isGuest: false, description: 'gm' }
    })
    expect(peekProfile('0xKURD')?.name).toBe('kurd')
  })

  it('saves an edit optimistically, and reverts it when the engine rejects the deploy', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit({ kind: 'profile', profile })
    act(() => requestPassport('0xme'))
    h.driver.emit({ kind: 'userProfile', address: '0xme', profile })

    act(() => h.session().profile.save({ description: 'gm from the beach' }))
    expect(h.driver.last('saveProfile')).toEqual({ kind: 'saveProfile', description: 'gm from the beach' })
    // Shown before the round trip — the deploy takes seconds, the catalyst reindex longer.
    expect(h.session().profile.saving).toBe(true)
    expect(h.session().profile.data?.description).toBe('gm from the beach')
    expect(peekProfile('0xme')?.description).toBe('gm from the beach')

    h.driver.emit({ kind: 'profileSaved', ok: false, error: 'failed to deploy to server.' })
    expect(h.session().profile.saving).toBe(false)
    expect(h.session().profile.saveError).toBe('failed to deploy to server.')
    expect(h.session().profile.data?.description).toBe('hi')
    expect(peekProfile('0xme')?.description).toBe('hi')

    act(() => h.session().profile.dismissSaveError())
    expect(h.session().profile.saveError).toBeNull()
  })

  it('puts a failed save back once, though the engine announced the edit and then its revert', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    const before: Profile = { ...profile, version: 3 }
    h.driver.emit({ kind: 'profile', profile: before })
    // your own passport is open: what it shows, in order
    const shown: (string | undefined)[] = []
    subscribeProfile('0xme', () => shown.push(peekProfile('0xme')?.description))
    act(() => requestPassport('0xme'))
    h.driver.emit({ kind: 'userProfile', address: '0xme', profile: before })
    shown.length = 0

    act(() => h.session().profile.save({ description: 'gm from the beach' }))
    // the engine applies the edit as version 4 and announces it; the store asks for that copy
    h.driver.emit({ kind: 'profileChanged', address: '0xme', version: 4 })
    // the deploy fails: the save is answered, and the engine puts version 3 back and announces it
    h.driver.emit({ kind: 'profileSaved', ok: false, error: 'failed to deploy to server.' })
    h.driver.emit({ kind: 'profileChanged', address: '0xme', version: 3 })
    // the read of version 4 is answered only now, then the one made after the revert
    h.driver.emit({ kind: 'userProfile', address: '0xme', profile: { ...before, version: 4, description: 'gm from the beach' } })
    h.driver.emit({ kind: 'userProfile', address: '0xme', profile: before })

    // the order is the check: a store without the revert mark shows the late read of the edit again;
    // the session's own copy goes back either way
    expect(shown).toEqual(['gm from the beach', 'hi', 'hi'])
    expect(h.session().profile.data?.description).toBe('hi')
  })

  it('keeps a successful edit and holds no error', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit({ kind: 'profile', profile })
    h.driver.emit({ kind: 'ownedNames', names: ['Tester2'] })
    act(() => h.session().profile.save({ name: 'Tester2' }))
    h.driver.emit({ kind: 'profileSaved', ok: true })
    expect(h.session().profile.data).toMatchObject({ name: 'Tester2', hasClaimedName: true })
    expect(h.session().profile.saveError).toBeNull()
    expect(h.session().profile.saving).toBe(false)
  })

  it('relays the claimed names the picker offers', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().profile.requestOwnedNames())
    expect(h.driver.last('getOwnedNames')).toEqual({ kind: 'getOwnedNames' })
    h.driver.emit({ kind: 'ownedNames', names: ['Mojito'] })
    expect(h.session().profile.ownedNames).toEqual(['Mojito'])
  })

  it('toggles the profile panel open/closed', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    expect(h.session().profile.open).toBe(false)
    act(() => h.session().profile.toggle())
    expect(h.session().profile.open).toBe(true)
  })
})
