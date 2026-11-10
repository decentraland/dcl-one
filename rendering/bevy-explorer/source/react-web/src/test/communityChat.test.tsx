import { beforeEach, describe, it, expect } from 'vitest'
import { act } from '@testing-library/react'
import { renderSession, enterAsGuest, type Harness } from './harness'
import { PREF, getPref, setPref } from '@hud/lib/prefs'
import { pingModeFor, resetPingPrefs, setPingMode } from '@hud/features/chat/ping'
import { setAutoTranslate } from '@hud/features/chat/translation'
import type { CommunityChannel, CommunityMember } from '@hud/engine/protocol'

// DOMAIN: community chat — the user's communities as chat tabs (fed at world entry, kept by
// presence), lines routed to them by their `community:<id>` channel and read from the engine's
// store like a DM's, unread and mention counts, the closed list, the ping sound, and the
// channel set the engine is told to listen to.
const MOJITO = '0x5854cce95d5e25817b41f4c41f06b695a83bc495'
const BOB = '0x2b6d2d8cd70b5e9548e87f871d4642e0d6387cd7'
const FOUNDATION: CommunityChannel = { id: 'c1', name: 'Decentraland Foundation', thumbnail: 'https://cdn.example/c1.png', role: 'member' }
const BUILDERS: CommunityChannel = { id: 'c8', name: 'Community Building DCL', thumbnail: '', role: 'owner' }
const member = (address: string, name: string, role = 'member'): CommunityMember => ({ address, name, role })

const ME = '0x1111111111111111111111111111111111111111'
const MY_NAME = 'Guest'

/** In the world with a profile (the handler needs the user's address and name to tell their own
 *  lines and @-mentions of them), and the communities listed. */
async function enterWithCommunities(h: Harness, communities: CommunityChannel[] = [FOUNDATION, BUILDERS]): Promise<void> {
  await enterAsGuest(h, { keepSent: true })
  act(() => h.driver.emit({ kind: 'profile', profile: { address: ME, name: MY_NAME, hasClaimedName: false, isGuest: true } }))
  act(() => h.driver.emit({ kind: 'communityChannels', communities }))
}

function me(h: Harness): string {
  return h.session().profile.data?.address ?? ''
}

describe('community chat domain', () => {
  beforeEach(() => {
    localStorage.clear()
    resetPingPrefs()
  })

  it('asks for the channels at world entry, lists them, and tells the engine to listen', async () => {
    const h = renderSession()
    await enterWithCommunities(h)
    expect(h.driver.last('getCommunityChannels')).toEqual({ kind: 'getCommunityChannels' })
    expect(me(h)).toBe(ME)
    const chat = h.session().chat
    expect(chat.communities.map((c) => [c.id, c.key, c.name, c.unread, c.mention, c.members])).toEqual([
      ['c1', 'community:c1', 'Decentraland Foundation', 0, false, null],
      ['c8', 'community:c8', 'Community Building DCL', 0, false, null]
    ])
    expect(h.driver.last('setCommunityChannels')).toEqual({ kind: 'setCommunityChannels', ids: ['c1', 'c8'] })
    // the Communities page's own list is untouched
    expect(h.session().communities.list).toEqual([])
  })

  it('a line on a community counts as unread, marks a mention, moves the community up, and pings', async () => {
    const h = renderSession()
    await enterWithCommunities(h)
    const chat = () => h.session().chat
    act(() => h.driver.emit({ kind: 'chat', chat: { sender: MOJITO, message: 'hello builders', channel: 'community:c8', messageId: `${MOJITO}:1` } }))
    expect(chat().communities.map((c) => [c.id, c.unread, c.mention])).toEqual([
      ['c8', 1, false],
      ['c1', 0, false]
    ])
    expect(chat().unread).toBe(1)
    expect(h.driver.last('playUiSound')).toEqual({ kind: 'playUiSound', sound: 'chatMessage' })
    // the line is not on the Nearby list, and no DM tab opened
    expect(chat().messages.some((m) => m.channel === 'community:c8')).toBe(false)
    expect(chat().conversations).toEqual([])
    // a mention of the local user (an unclaimed name carries the wallet's tail) marks the tab and
    // sounds the mention clip
    act(() => h.driver.emit({ kind: 'chat', chat: { sender: MOJITO, message: `@${MY_NAME}#${ME.slice(-4)} are you there?`, channel: 'community:c1', messageId: `${MOJITO}:2` } }))
    expect(chat().communities.map((c) => [c.id, c.unread, c.mention])).toEqual([
      ['c1', 1, true],
      ['c8', 1, false]
    ])
    expect(h.driver.last('playUiSound')).toEqual({ kind: 'playUiSound', sound: 'chatMention' })
  })

  it('the ping setting gates the sound, and the user\'s own lines never sound', async () => {
    const h = renderSession()
    await enterWithCommunities(h)
    setPingMode('community:c1', 'mentions')
    act(() => h.driver.emit({ kind: 'chat', chat: { sender: MOJITO, message: 'no mention here', channel: 'community:c1', messageId: `${MOJITO}:3` } }))
    expect(h.driver.last('playUiSound')).toBeUndefined()
    setPingMode('community:c1', 'all')
    act(() => h.driver.emit({ kind: 'chat', chat: { sender: me(h), message: 'my own words', channel: 'community:c1', messageId: `${me(h)}:4` } }))
    expect(h.driver.last('playUiSound')).toBeUndefined()
    expect(h.session().chat.communities.find((c) => c.id === 'c1')?.unread).toBe(2)
    // a flood of lines on one conversation is one sound, a mention among them another
    for (let i = 0; i < 5; i++) act(() => h.driver.emit({ kind: 'chat', chat: { sender: MOJITO, message: `line ${i}`, channel: 'community:c1', messageId: `${MOJITO}:1${i}` } }))
    act(() => h.driver.emit({ kind: 'chat', chat: { sender: MOJITO, message: `@${MY_NAME}#${ME.slice(-4)} hey`, channel: 'community:c1', messageId: `${MOJITO}:20` } }))
    expect(h.driver.sent.filter((m) => m.kind === 'playUiSound').map((m) => m.sound)).toEqual(['chatMessage', 'chatMention'])
  })

  it('a line on a community that is not open is dropped', async () => {
    const h = renderSession()
    await enterWithCommunities(h, [FOUNDATION])
    act(() => h.driver.emit({ kind: 'chat', chat: { sender: MOJITO, message: 'from nowhere', channel: 'community:c9', messageId: `${MOJITO}:5` } }))
    expect(h.session().chat.unread).toBe(0)
    expect(h.session().chat.communities.map((c) => c.unread)).toEqual([0])
    expect(h.driver.last('playUiSound')).toBeUndefined()
  })

  it('showing a community clears its counts, reads its history and members, and sends there', async () => {
    const h = renderSession()
    await enterWithCommunities(h)
    act(() => h.driver.emit({ kind: 'chat', chat: { sender: MOJITO, message: 'hi', channel: 'community:c1', messageId: `${MOJITO}:6` } }))
    act(() => h.session().chat.select('community:c1'))
    const chat = () => h.session().chat
    expect(chat().channel).toBe('community:c1')
    expect(chat().communities.find((c) => c.id === 'c1')?.unread).toBe(0)
    expect(h.driver.last('dmHistory')).toEqual({ kind: 'dmHistory', address: 'community:c1' })
    expect(h.driver.last('getCommunityOnlineMembers')).toEqual({ kind: 'getCommunityOnlineMembers', id: 'c1' })
    act(() => h.driver.emit({ kind: 'dmHistory', address: 'community:c1', entries: [{ from: MOJITO, message: 'hi', receivedAt: 1000, messageId: `${MOJITO}:6`, reactions: [] }] }))
    expect(chat().messages.map((m) => [m.sender, m.message, m.channel])).toEqual([[MOJITO, 'hi', 'community:c1']])
    // the members list excludes the local user and anyone they blocked; the count follows it
    act(() => h.driver.emit({ kind: 'communityOnlineMembers', id: 'c1', members: [member(me(h), 'Me'), member(MOJITO, 'Mojito', 'moderator'), member(BOB, 'Bob')] }))
    expect(chat().communities.find((c) => c.id === 'c1')?.online).toBe(2)
    expect(chat().communities.find((c) => c.id === 'c1')?.members?.map((m) => m.name)).toEqual(['Mojito', 'Bob'])
    // a member leaving and another arriving; a listed member's update (which names them only, so
    // it carries the default role) keeps their listed role
    act(() => h.driver.emit({ kind: 'communityPresence', communityId: 'c1', member: member(BOB, 'Bob'), online: false }))
    act(() => h.driver.emit({ kind: 'communityPresence', communityId: 'c1', member: member('0x3333333333333333333333333333333333333333', 'Cy'), online: true }))
    act(() => h.driver.emit({ kind: 'communityPresence', communityId: 'c1', member: member(MOJITO, 'Mojito'), online: true }))
    expect(chat().communities.find((c) => c.id === 'c1')?.members?.map((m) => [m.name, m.role])).toEqual([['Cy', 'member'], ['Mojito', 'moderator']])
    // a line landing on the shown tab is appended after the stored rows, neither counted nor read
    // back from the store
    act(() => h.driver.emit({ kind: 'chat', chat: { sender: MOJITO, message: 'again', channel: 'community:c1', messageId: `${MOJITO}:7` } }))
    expect(chat().communities.find((c) => c.id === 'c1')?.unread).toBe(0)
    expect(h.driver.sent.filter((m) => m.kind === 'dmHistory').length).toBe(1)
    expect(chat().messages.map((m) => [m.sender, m.message, m.id])).toEqual([[MOJITO, 'hi', 0], [MOJITO, 'again', 1]])
    // a read answered without that line (it was queued first) keeps the line behind the stored
    // rows; a read that has it settles it
    act(() => h.driver.emit({ kind: 'dmHistory', address: 'community:c1', entries: [{ from: MOJITO, message: 'hi', receivedAt: 1000, messageId: `${MOJITO}:6`, reactions: [] }] }))
    expect(chat().messages.map((m) => [m.message, m.id])).toEqual([['hi', 0], ['again', 1]])
    act(() => h.driver.emit({ kind: 'dmHistory', address: 'community:c1', entries: [{ from: MOJITO, message: 'hi', receivedAt: 1000, messageId: `${MOJITO}:6`, reactions: [] }, { from: MOJITO, message: 'again', receivedAt: 2000, messageId: `${MOJITO}:7`, reactions: [] }] }))
    expect(chat().messages.map((m) => [m.message, m.id, m.ts])).toEqual([['hi', 0, 1000], ['again', 1, 2000]])
    // sends and reactions name the community channel
    act(() => chat().send('hello all'))
    expect(h.driver.last('sendChat')).toEqual({ kind: 'sendChat', message: 'hello all', channel: 'community:c1' })
    act(() => chat().react(`${MOJITO}:6`, '\u{1f44d}', false))
    expect(h.driver.last('sendChatReaction')).toEqual({ kind: 'sendChatReaction', channel: 'community:c1', messageId: `${MOJITO}:6`, emoji: '\u{1f44d}', remove: false })
  })

  it('closing a community hides it, remembers that for the account, and drops its later lines', async () => {
    const h = renderSession()
    await enterWithCommunities(h)
    act(() => h.session().chat.select('community:c1'))
    act(() => h.session().chat.closeCommunity('c1'))
    const chat = () => h.session().chat
    expect(chat().channel).toBe('Nearby')
    expect(chat().communities.map((c) => c.id)).toEqual(['c8'])
    expect(h.driver.last('setCommunityChannels')).toEqual({ kind: 'setCommunityChannels', ids: ['c8'] })
    expect(JSON.parse(localStorage.getItem(PREF.chatClosedCommunities) ?? '{}')).toEqual({ [me(h).toLowerCase()]: ['c1'] })
    act(() => h.driver.emit({ kind: 'chat', chat: { sender: MOJITO, message: 'closed door', channel: 'community:c1', messageId: `${MOJITO}:8` } }))
    expect(chat().unread).toBe(0)
    // opening it again (the community card's Open chat) shows and selects it, and opens chat
    act(() => chat().toggle())
    expect(chat().open).toBe(false)
    act(() => chat().openCommunity(FOUNDATION))
    expect(chat().open).toBe(true)
    expect(chat().channel).toBe('community:c1')
    expect(chat().communities.map((c) => c.id)).toEqual(['c1', 'c8'])
    expect(JSON.parse(localStorage.getItem(PREF.chatClosedCommunities) ?? '{}')).toEqual({ [me(h).toLowerCase()]: [] })
  })

  it('a community closed in an earlier session stays closed; a join opens a chat, a leave removes it', async () => {
    setPref(PREF.chatClosedCommunities, JSON.stringify({ [ME]: ['c8'] }))
    const h = renderSession()
    // the list is asked for once the account is known, so the closed chat is never listened to
    await enterAsGuest(h, { keepSent: true })
    expect(h.driver.last('getCommunityChannels')).toBeUndefined()
    act(() => h.driver.emit({ kind: 'profile', profile: { address: ME, name: MY_NAME, hasClaimedName: false, isGuest: true } }))
    expect(h.driver.last('getCommunityChannels')).toEqual({ kind: 'getCommunityChannels' })
    act(() => h.driver.emit({ kind: 'communityChannels', communities: [FOUNDATION, BUILDERS] }))
    expect(h.session().chat.communities.map((c) => c.id)).toEqual(['c1'])
    expect(h.driver.sent.filter((m) => m.kind === 'setCommunityChannels').map((m) => m.ids)).toEqual([[], ['c1']])
    // the user joined another community (their own online update)
    act(() => h.driver.emit({ kind: 'communityChannelAdded', community: { id: 'c5', name: 'Toxic Events', thumbnail: '', role: 'member' } }))
    expect(h.session().chat.communities.map((c) => c.id)).toEqual(['c1', 'c5'])
    // and left one while it was shown
    act(() => h.session().chat.select('community:c5'))
    act(() => h.driver.emit({ kind: 'communityChannelRemoved', id: 'c5' }))
    expect(h.session().chat.communities.map((c) => c.id)).toEqual(['c1'])
    expect(h.session().chat.channel).toBe('Nearby')
  })

  it('a later list without the shown community returns the chat to Nearby', async () => {
    const h = renderSession()
    await enterWithCommunities(h)
    act(() => h.session().chat.select('community:c8'))
    act(() => h.driver.emit({ kind: 'dmHistory', address: 'community:c8', entries: [{ from: MOJITO, message: 'hi', receivedAt: 1000, messageId: `${MOJITO}:6`, reactions: [] }] }))
    expect(h.session().chat.messages.map((m) => m.channel)).toEqual(['community:c8'])
    // the user left it from another client: the next list lacks it
    act(() => h.driver.emit({ kind: 'communityChannels', communities: [FOUNDATION] }))
    const chat = () => h.session().chat
    expect(chat().communities.map((c) => c.id)).toEqual(['c1'])
    expect(chat().channel).toBe('Nearby')
    expect(chat().messages.some((m) => m.channel === 'community:c8')).toBe(false)
    act(() => chat().send('still here?'))
    expect(h.driver.last('sendChat')).toEqual({ kind: 'sendChat', message: 'still here?', channel: 'Nearby' })
    // a shown community that stays listed is kept shown
    act(() => chat().select('community:c1'))
    act(() => h.driver.emit({ kind: 'communityChannels', communities: [FOUNDATION, BUILDERS] }))
    expect(chat().channel).toBe('community:c1')
  })

  it('a community card opened from the chat fetches that community for the card, and says when that fails', async () => {
    const h = renderSession()
    await enterWithCommunities(h)
    act(() => h.session().communities.fetch('c1'))
    expect(h.driver.last('getCommunity')).toEqual({ kind: 'getCommunity', id: 'c1' })
    act(() => h.driver.emit({ kind: 'communityActionFailed', id: 'c1', action: 'fetch', message: 'HTTP 502' }))
    expect(h.session().communities.error).toEqual({ id: 'c1', message: 'There was an error loading the community. Please try again.' })
    expect(h.session().communities.cards).toEqual({})
    // asking again clears the failure, so a success after it is not shown with the old message
    act(() => h.session().communities.fetch('c1'))
    expect(h.session().communities.error).toBeNull()
    const community = { id: 'c1', name: 'Decentraland Foundation', description: 'The official one', membersCount: 1242, role: 'member', ownerName: 'DCLOfficial', privacy: 'public' }
    act(() => h.driver.emit({ kind: 'community', community }))
    expect(h.session().communities.cards).toEqual({ c1: community })
    // the browse list is the Communities page's: the card's community does not join it
    expect(h.session().communities.list).toEqual([])
    // a re-fetch replaces it
    act(() => h.driver.emit({ kind: 'community', community: { ...community, membersCount: 1243 } }))
    expect(h.session().communities.cards.c1.membersCount).toBe(1243)
  })

  it('a re-read of a store the engine trimmed keeps the ids of the lines shown and numbers only new ones', async () => {
    const h = renderSession()
    await enterWithCommunities(h)
    act(() => h.session().chat.select('community:c1'))
    const chat = () => h.session().chat
    const entry = (n: number) => ({ from: MOJITO, message: `m${n}`, receivedAt: n * 1000, messageId: `${MOJITO}:${n}`, reactions: [] })
    const read = (from: number, to: number) => h.driver.emit({ kind: 'dmHistory', address: 'community:c1', entries: Array.from({ length: to - from + 1 }, (_, i) => entry(from + i)) })
    act(() => read(1, 200))
    expect(chat().messages.map((m) => m.id)).toEqual(Array.from({ length: 200 }, (_, i) => i))
    // the store at its cap: a new message pushed the oldest out, and every row moved down
    act(() => read(2, 201))
    expect(chat().messages[0]).toMatchObject({ message: 'm2', id: 1 })
    expect(chat().messages[199]).toMatchObject({ message: 'm201', id: 200 })
    // a line appended to the shown tab, then read back from the store: the same id
    act(() => h.driver.emit({ kind: 'chat', chat: { sender: MOJITO, message: 'm202', channel: 'community:c1', messageId: `${MOJITO}:202` } }))
    expect(chat().messages[199]).toMatchObject({ message: 'm202', id: 201 })
    act(() => read(3, 202))
    expect(chat().messages[0]).toMatchObject({ message: 'm3', id: 2 })
    expect(chat().messages[199]).toMatchObject({ message: 'm202', id: 201 })
    // a deleted history starts over with fresh ids above the old ones
    act(() => chat().deleteHistory('community:c1'))
    act(() => read(300, 300))
    expect(chat().messages.map((m) => [m.message, m.id])).toEqual([['m300', 202]])
  })

  it('a line appended while the history of a tab is read stays last, with an id above the lines read after it', async () => {
    const h = renderSession()
    await enterWithCommunities(h)
    const chat = () => h.session().chat
    const entry = (n: number) => ({ from: MOJITO, message: `m${n}`, receivedAt: n * 1000, messageId: `${MOJITO}:${n}`, reactions: [] })
    act(() => chat().select('community:c1'))
    act(() => h.driver.emit({ kind: 'dmHistory', address: 'community:c1', entries: [entry(1), entry(2)] }))
    act(() => chat().select('Nearby'))
    // two lines land while the tab is in the background: the store has them, the tab does not
    act(() => h.driver.emit({ kind: 'chat', chat: { sender: BOB, message: 'm3', channel: 'community:c1', messageId: `${BOB}:3` } }))
    act(() => h.driver.emit({ kind: 'chat', chat: { sender: BOB, message: 'm4', channel: 'community:c1', messageId: `${BOB}:4` } }))
    // the tab is shown again: its history is asked for, and a live line lands before the answer
    act(() => chat().select('community:c1'))
    act(() => h.driver.emit({ kind: 'chat', chat: { sender: MOJITO, message: 'm5', channel: 'community:c1', messageId: `${MOJITO}:5` } }))
    act(() => h.driver.emit({ kind: 'dmHistory', address: 'community:c1', entries: [entry(1), entry(2), { from: BOB, message: 'm3', receivedAt: 3000, messageId: `${BOB}:3`, reactions: [] }, { from: BOB, message: 'm4', receivedAt: 4000, messageId: `${BOB}:4`, reactions: [] }] }))
    const shown = chat().messages.map((m) => [m.message, m.id])
    expect(shown.map(([message]) => message)).toEqual(['m1', 'm2', 'm3', 'm4', 'm5'])
    const ids = shown.map(([, id]) => id as number)
    for (let i = 1; i < ids.length; i++) expect(ids[i]).toBeGreaterThan(ids[i - 1])
    // the lines shown before keep their ids
    expect(shown.slice(0, 2)).toEqual([['m1', 0], ['m2', 1]])
  })

  it('two lines sharing a message id keep their own ids across reads', async () => {
    const h = renderSession()
    await enterWithCommunities(h)
    const chat = () => h.session().chat
    const twin = (message: string) => ({ from: MOJITO, message, receivedAt: 1000, messageId: `${MOJITO}:1`, reactions: [] })
    const entries = [twin('first'), twin('second'), { from: BOB, message: 'third', receivedAt: 2000, messageId: `${BOB}:2`, reactions: [] }]
    act(() => chat().select('community:c1'))
    act(() => h.driver.emit({ kind: 'dmHistory', address: 'community:c1', entries }))
    expect(chat().messages.map((m) => [m.message, m.id])).toEqual([['first', 0], ['second', 1], ['third', 2]])
    act(() => h.driver.emit({ kind: 'dmHistory', address: 'community:c1', entries }))
    expect(chat().messages.map((m) => [m.message, m.id])).toEqual([['first', 0], ['second', 1], ['third', 2]])
  })

  it('a reaction on a community line is applied to the line shown, without reading the store back', async () => {
    const h = renderSession()
    await enterWithCommunities(h)
    act(() => h.session().chat.select('community:c1'))
    act(() => h.driver.emit({ kind: 'dmHistory', address: 'community:c1', entries: [{ from: MOJITO, message: 'hi', receivedAt: 1000, messageId: `${MOJITO}:6`, reactions: [] }] }))
    const chat = () => h.session().chat
    const reads = h.driver.sentOf('dmHistory').length
    const thumbs = '\u{1f44d}'
    act(() => h.driver.emit({ kind: 'chatReaction', reaction: { channel: 'community:c1', messageId: `${MOJITO}:6`, emoji: thumbs, from: BOB, remove: false } }))
    expect(chat().messages[0].reactions).toEqual([{ emoji: thumbs, from: [BOB] }])
    act(() => h.driver.emit({ kind: 'chatReaction', reaction: { channel: 'community:c1', messageId: `${MOJITO}:6`, emoji: thumbs, from: ME, remove: false } }))
    expect(chat().messages[0].reactions).toEqual([{ emoji: thumbs, from: [BOB, ME] }])
    act(() => h.driver.emit({ kind: 'chatReaction', reaction: { channel: 'community:c1', messageId: `${MOJITO}:6`, emoji: thumbs, from: BOB, remove: true } }))
    expect(chat().messages[0].reactions).toEqual([{ emoji: thumbs, from: [ME] }])
    // a reaction to a line not shown changes nothing
    const before = chat().messages
    act(() => h.driver.emit({ kind: 'chatReaction', reaction: { channel: 'community:c1', messageId: `${MOJITO}:999`, emoji: thumbs, from: BOB, remove: false } }))
    expect(chat().messages).toBe(before)
    expect(h.driver.sentOf('dmHistory').length).toBe(reads)
  })

  it('the shown community tab follows community chat\'s state, which blocks the input without a router', async () => {
    const h = renderSession()
    await enterWithCommunities(h)
    expect(h.driver.last('dmWatch')).toBeUndefined()
    act(() => h.session().chat.select('community:c1'))
    expect(h.driver.last('dmWatch')).toEqual({ kind: 'dmWatch', address: 'community:c1', on: true })
    const chat = () => h.session().chat
    act(() => h.driver.emit({ kind: 'dmUserState', address: 'community:c1', state: 'noRouter', online: false }))
    expect(chat().communities.find((c) => c.id === 'c1')?.state).toBe('noRouter')
    expect(chat().conversations).toEqual([])
    act(() => h.driver.emit({ kind: 'dmUserState', address: 'community:c1', state: 'connected', online: true }))
    expect(chat().communities.find((c) => c.id === 'c1')?.state).toBe('connected')
    // another tab shown: the watch moves with it
    act(() => chat().select('Nearby'))
    expect(h.driver.last('dmWatch')).toEqual({ kind: 'dmWatch', address: 'community:c1', on: false })
    act(() => chat().select('community:c8'))
    expect(h.driver.last('dmWatch')).toEqual({ kind: 'dmWatch', address: 'community:c8', on: true })
    act(() => chat().toggle())
    expect(h.driver.last('dmWatch')).toEqual({ kind: 'dmWatch', address: 'community:c8', on: false })
  })

  it('showing a community asks who is online each time; presence for another community changes nothing', async () => {
    const h = renderSession()
    await enterWithCommunities(h)
    const chat = () => h.session().chat
    act(() => chat().select('community:c1'))
    act(() => h.driver.emit({ kind: 'communityOnlineMembers', id: 'c1', members: [member(MOJITO, 'Mojito')] }))
    act(() => chat().select('Nearby'))
    act(() => chat().select('community:c1'))
    expect(h.driver.sentOf('getCommunityOnlineMembers').map((m) => m.id)).toEqual(['c1', 'c1'])
    const before = chat().communities
    act(() => h.driver.emit({ kind: 'communityPresence', communityId: 'c8', member: member(BOB, 'Bob'), online: true }))
    expect(chat().communities).toBe(before)
  })

  it('a community the user is no longer in loses its stored history, its settings and its closed entry', async () => {
    const h = renderSession()
    await enterWithCommunities(h)
    expect(JSON.parse(getPref(PREF.chatKnownCommunities) ?? '{}')).toEqual({ [ME.toLowerCase()]: ['c1', 'c8'] })
    setPingMode('community:c8', 'none')
    setAutoTranslate('community:c8', true)
    act(() => h.session().chat.closeCommunity('c8'))
    act(() => h.driver.emit({ kind: 'communityChannelRemoved', id: 'c8' }))
    expect(h.driver.last('dmDelete')).toEqual({ kind: 'dmDelete', address: 'community:c8' })
    expect(pingModeFor('community:c8')).toBe('all')
    expect(JSON.parse(getPref(PREF.chatPingChannels) ?? '{}')).toEqual({})
    expect(JSON.parse(getPref(PREF.chatAutoTranslateChannels) ?? '{}')).toEqual({})
    expect(JSON.parse(getPref(PREF.chatClosedCommunities) ?? '{}')).toEqual({ [ME.toLowerCase()]: [] })
    expect(JSON.parse(getPref(PREF.chatKnownCommunities) ?? '{}')).toEqual({ [ME.toLowerCase()]: ['c1'] })
    // closing a tab, on the other hand, leaves its settings alone
    setPingMode('community:c1', 'mentions')
    act(() => h.session().chat.closeCommunity('c1'))
    expect(pingModeFor('community:c1')).toBe('mentions')
  })

  it('a community left while logged out is forgotten when the first list lacks it', async () => {
    setPref(PREF.chatKnownCommunities, JSON.stringify({ [ME.toLowerCase()]: ['c1', 'c9'] }))
    setPingMode('community:c9', 'none')
    const h = renderSession()
    await enterWithCommunities(h, [FOUNDATION])
    expect(h.driver.sentOf('dmDelete')).toEqual([{ kind: 'dmDelete', address: 'community:c9' }])
    expect(pingModeFor('community:c9')).toBe('all')
    expect(JSON.parse(getPref(PREF.chatKnownCommunities) ?? '{}')).toEqual({ [ME.toLowerCase()]: ['c1'] })
    // a join (the user's own online update) is remembered too
    act(() => h.driver.emit({ kind: 'communityChannelAdded', community: BUILDERS }))
    expect(JSON.parse(getPref(PREF.chatKnownCommunities) ?? '{}')).toEqual({ [ME.toLowerCase()]: ['c1', 'c8'] })
  })

  it('logging out forgets the tabs', async () => {
    const h = renderSession()
    await enterWithCommunities(h)
    act(() => h.session().logout())
    expect(h.session().chat.communities).toEqual([])
  })
})
