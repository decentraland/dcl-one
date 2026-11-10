import { describe, it, expect } from 'vitest'
import { act } from '@testing-library/react'
import type { Community } from '../engine/protocol'
import { renderSession, enterAsGuest } from './harness'
import { peekProfile } from '../features/session/profileStore'

// DOMAIN: communities — browse list, join/leave, per-community detail.
describe('communities domain', () => {
  const community: Community = {
    id: 'c1',
    name: 'Builders',
    description: 'we build',
    membersCount: 10,
    role: 'none',
    ownerName: 'Owner',
    privacy: 'public'
  }

  it('opening the panel fetches communities once', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().communities.toggle())
    expect(h.driver.sentOf('getCommunities')).toHaveLength(1)
  })

  it('communities stream populates the list', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit({ kind: 'communities', communities: [community] })
    expect(h.session().communities.list[0].name).toBe('Builders')
  })

  it('join / leave post the community id', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().communities.join('c1'))
    expect(h.driver.last('joinCommunity')).toEqual({ kind: 'joinCommunity', id: 'c1' })
    act(() => h.session().communities.leave('c1'))
    expect(h.driver.last('leaveCommunity')).toEqual({ kind: 'leaveCommunity', id: 'c1' })
  })

  it('loadDetail requests detail and the stream fills the modal', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().communities.loadDetail('c1'))
    expect(h.driver.last('getCommunityDetail')).toEqual({ kind: 'getCommunityDetail', id: 'c1' })
    h.driver.emit({
      kind: 'communityDetail',
      id: 'c1',
      members: [{ address: '0x1', name: 'A', role: 'owner' }],
      posts: [],
      places: [],
      events: [],
      photos: []
    })
    expect(h.session().communities.detail?.id).toBe('c1')
    expect(h.session().communities.detail?.members).toHaveLength(1)
  })
  // A save's answer belongs to the form that asked: one that comes after the form closed is dropped.
  it('drops a save answer that arrives after the form closed', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().communities.create({ name: 'B', description: 'd', privacy: 'public', discoverable: true, placeIds: [] }))
    expect(h.session().communities.save).toEqual({ status: 'saving' })
    act(() => h.session().communities.clearSave())
    act(() => h.driver.emit({ kind: 'communitySaveResult', mode: 'create', id: 'late' }))
    expect(h.session().communities.save).toBeNull()
  })

  it('keeps the answer to the save in flight', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().communities.create({ name: 'B', description: 'd', privacy: 'public', discoverable: true, placeIds: [] }))
    act(() => h.driver.emit({ kind: 'communitySaveResult', mode: 'create', id: 'c-new' }))
    expect(h.session().communities.save).toEqual({ status: 'done', result: { kind: 'communitySaveResult', mode: 'create', id: 'c-new' } })
  })

  // A detail re-read for a card already closed (after a place delete, say) must not replace the open card's.
  it('keeps only the detail of the card asked for last', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    const detail = (id: string) => ({ kind: 'communityDetail' as const, id, members: [], posts: [], places: [], events: [], photos: [] })
    act(() => h.session().communities.loadDetail('cB'))
    act(() => h.driver.emit(detail('cA')))
    expect(h.session().communities.detail).toBeNull()
    act(() => h.driver.emit(detail('cB')))
    expect(h.session().communities.detail?.id).toBe('cB')
  })

  // The card's place delete: sent as asked, and a refusal is said on the card in unity-explorer's words.
  it('unlinks a place, and says so on the card when it fails', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().communities.removePlace('c1', 'pl1'))
    expect(h.driver.sent).toContainEqual({ kind: 'removeCommunityPlace', id: 'c1', placeId: 'pl1' })
    act(() => h.driver.emit({ kind: 'communityActionFailed', id: 'c1', action: 'removePlace', message: 'HTTP 403' }))
    expect(h.session().communities.error).toEqual({ id: 'c1', message: 'There was an error deleting the community place. Please try again.' })
  })

  // The card's member menu: sent as asked, and each refusal said on the card in unity-explorer's words
  // (MembersListController's error texts).
  it.each([
    ['promote', 'There was an error adding moderator to user. Please try again.'],
    ['demote', 'There was an error removing moderator from user. Please try again.'],
    ['transfer', 'There was an error transfering ownership. Please try again.'],
    ['kick', 'There was an error kicking the user. Please try again.'],
    ['ban', 'There was an error banning the user. Please try again.']
  ] as const)('sends a %s on a member, and says so on the card when it fails', async (action, message) => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().communities.memberAction('c1', '0xm', action))
    expect(h.driver.sent).toContainEqual({ kind: 'communityMemberAction', id: 'c1', address: '0xm', action })
    act(() => h.driver.emit({ kind: 'communityActionFailed', id: 'c1', action, message: 'HTTP 403' }))
    expect(h.session().communities.error).toEqual({ id: 'c1', message })
  })

  // The card's members are seeded into the profile store (an unclaimed name with its #tag), so the
  // member menu and the passport show them.
  it("seeds the card's members into the profile store", async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().communities.loadDetail('c1'))
    act(() =>
      h.driver.emit({
        kind: 'communityDetail',
        id: 'c1',
        posts: [],
        places: [],
        events: [],
        photos: [],
        members: [
          { address: '0xclaimed', name: 'Named', role: 'member', hasClaimedName: true, picture: 'n.png' },
          { address: '0xabcd1234', name: 'Plain', role: 'member', hasClaimedName: false }
        ]
      })
    )
    expect(peekProfile('0xclaimed')).toMatchObject({ name: 'Named', picture: 'n.png', hasClaimedName: true })
    expect(peekProfile('0xabcd1234')).toMatchObject({ name: 'Plain#1234', hasClaimedName: false })
  })

  // The form's places are read as it opens; an answer for another form, or after it closed, is dropped.
  it('keeps the place options the open form asked for and drops any other', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    const place = { id: 'p1', isWorld: true, label: 'Mansion (me.dcl.eth)', ownerName: 'Me' }
    act(() => h.session().communities.loadPlaceOptions('c1'))
    expect(h.driver.sent).toContainEqual({ kind: 'getCommunityPlaceOptions', communityId: 'c1' })
    act(() => h.driver.emit({ kind: 'communityPlaceOptions', owned: [place], linked: [], hiddenLinkedIds: [], linkedFailed: false, errors: [] }))
    expect(h.session().communities.placeOptions).toBeNull()
    act(() => h.driver.emit({ kind: 'communityPlaceOptions', communityId: 'c1', owned: [place], linked: [place], hiddenLinkedIds: [], linkedFailed: false, errors: [] }))
    expect(h.session().communities.placeOptions?.linked).toEqual([place])
    act(() => h.session().communities.clearPlaceOptions())
    act(() => h.driver.emit({ kind: 'communityPlaceOptions', communityId: 'c1', owned: [], linked: [], hiddenLinkedIds: [], linkedFailed: false, errors: [] }))
    expect(h.session().communities.placeOptions).toBeNull()
  })

  // The picture's URL does not change on update, so the saved picture is shown from then on.
  it('shows a picture saved by an edit, also after the list is read again', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    const withThumb = { ...community, thumbnail: 'https://cdn/c1.png' }
    act(() => h.driver.emit({ kind: 'communities', communities: [withThumb] }))
    act(() => h.session().communities.update({ id: 'c1', thumbnail: 'data:image/png;base64,AAAA' }))
    act(() => h.driver.emit({ kind: 'communitySaveResult', mode: 'update', id: 'c1' }))
    expect(h.session().communities.list[0].thumbnail).toBe('data:image/png;base64,AAAA')
    act(() => h.driver.emit({ kind: 'communities', communities: [withThumb] }))
    expect(h.session().communities.list[0].thumbnail).toBe('data:image/png;base64,AAAA')
  })

  it('logout forgets the save, the deleted community and saved pictures', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    const withThumb = { ...community, thumbnail: 'https://cdn/c1.png' }
    act(() => h.driver.emit({ kind: 'communities', communities: [withThumb] }))
    act(() => h.session().communities.update({ id: 'c1', thumbnail: 'data:image/png;base64,AAAA' }))
    act(() => h.driver.emit({ kind: 'communitySaveResult', mode: 'update', id: 'c1' }))
    act(() => h.driver.emit({ kind: 'communityDeleted', id: 'c2' }))
    act(() => h.session().logout())
    expect(h.session().communities.save).toBeNull()
    expect(h.session().communities.deleted).toBeNull()
    act(() => h.driver.emit({ kind: 'communities', communities: [withThumb] }))
    expect(h.session().communities.list[0].thumbnail).toBe('https://cdn/c1.png')
  })
})
