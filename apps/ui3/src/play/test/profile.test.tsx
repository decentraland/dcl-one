import { describe, it, expect } from 'vitest'
import { act, waitFor } from '@testing-library/react'
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

  it('a sign-out leaves no announced version behind to hold back the next session\'s profile', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit({ kind: 'profile', profile: { ...profile, version: 3 } })
    // a chat line of yours keeps your profile shown
    subscribeProfile('0xme', () => {})
    // a Backpack edit is announced and read, and the account signs out before its deploy result
    h.driver.emit({ kind: 'profileChanged', address: '0xme', version: 4 })
    h.driver.emit({ kind: 'userProfile', address: '0xme', profile: { ...profile, version: 4, description: 'edited' } })
    act(() => h.session().logout())
    await waitFor(() => expect(h.session().phase).toBe('login'))
    // the same account signs in again, and the engine loads the version the failed deploy left
    await enterAsGuest(h)
    h.driver.emit({ kind: 'profile', profile: { ...profile, version: 3 } })
    expect(peekProfile('0xme')).toMatchObject({ version: 3, description: 'hi' })
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

  it('saves a name colour quietly: shown at once, and a failure only puts the old colour back', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit({ kind: 'profile', profile })
    act(() => requestPassport('0xme'))
    h.driver.emit({ kind: 'userProfile', address: '0xme', profile })

    act(() => h.session().profile.saveNameColor({ r: 1, g: 0, b: 0 }))
    expect(h.driver.last('saveProfile')).toEqual({ kind: 'saveProfile', nameColor: { r: 1, g: 0, b: 0 } })
    expect(h.session().profile.data?.nameColor).toEqual({ r: 1, g: 0, b: 0 })
    expect(peekProfile('0xme')?.nameColor).toEqual({ r: 1, g: 0, b: 0 })
    // Not the edit form's save: its SAVE button and error stay out of it.
    expect(h.session().profile.saving).toBe(false)

    h.driver.emit({ kind: 'profileSaved', ok: false, error: 'failed to deploy to server.' })
    expect(h.session().profile.saveError).toBeNull()
    expect(h.session().profile.data?.nameColor).toBeUndefined()
    expect(peekProfile('0xme')?.nameColor).toBeUndefined()
  })

  it('holds a save asked for while another deploys, so the engine never cancels the first', async () => {
    const h = renderSession()
    await enterAsGuest(h, { keepSent: true })
    h.driver.emit({ kind: 'profile', profile })
    act(() => h.session().profile.save({ description: 'gm' }))
    act(() => h.session().profile.saveNameColor({ r: 0, g: 1, b: 0 }))
    expect(h.driver.sentOf('saveProfile')).toHaveLength(1)
    expect(h.session().profile.saving).toBe(true)

    h.driver.emit({ kind: 'profileSaved', ok: true })
    expect(h.driver.sentOf('saveProfile')).toHaveLength(2)
    expect(h.driver.last('saveProfile')).toEqual({ kind: 'saveProfile', nameColor: { r: 0, g: 1, b: 0 } })
    expect(h.session().profile.data).toMatchObject({ description: 'gm', nameColor: { r: 0, g: 1, b: 0 } })
    // The colour's own deploy is quiet: the form is no longer saving while it runs.
    expect(h.session().profile.saving).toBe(false)

    h.driver.emit({ kind: 'profileSaved', ok: true })
    expect(h.session().profile.saveError).toBeNull()
  })

  it('a rejected save is not carried into the save queued behind it', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit({ kind: 'profile', profile })
    act(() => h.session().profile.save({ description: 'gm' }))
    act(() => h.session().profile.saveNameColor({ r: 0, g: 0, b: 1 }))

    h.driver.emit({ kind: 'profileSaved', ok: false, error: 'failed to deploy to server.' })
    expect(h.session().profile.saveError).toBe('failed to deploy to server.')
    expect(h.session().profile.data).toMatchObject({ description: 'hi', nameColor: { r: 0, g: 0, b: 1 } })

    h.driver.emit({ kind: 'profileSaved', ok: true })
    expect(h.session().profile.data).toMatchObject({ description: 'hi', nameColor: { r: 0, g: 0, b: 1 } })
  })

  it('a save the bridge failed to handle counts as failed, so the queue behind it still runs', async () => {
    const h = renderSession()
    await enterAsGuest(h, { keepSent: true })
    h.driver.emit({ kind: 'profile', profile })
    act(() => h.session().profile.save({ description: 'gm' }))
    act(() => h.session().profile.saveNameColor({ r: 1, g: 0, b: 1 }))

    h.driver.emit({ kind: 'requestFailed', request: 'saveProfile', error: 'boom' })
    expect(h.session().profile.saveError).toBe('Could not save your profile.')
    expect(h.session().profile.data?.description).toBe('hi')
    expect(h.driver.last('saveProfile')).toEqual({ kind: 'saveProfile', nameColor: { r: 1, g: 0, b: 1 } })
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
