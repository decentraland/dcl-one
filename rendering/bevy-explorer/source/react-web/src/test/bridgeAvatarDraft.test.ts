import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { PageToScene, SceneToPage } from '@explorer-core/engine/protocol'
import type { SetAvatarData } from '@explorer-core/engine/generated'
import type { Ctx } from '../../bridge-scene/src/bridge'

function deferred() {
  let resolve!: (value: number) => void
  let reject!: (error: Error) => void
  const promise = new Promise<number>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

const BODY = 'urn:decentraland:off-chain:base-avatars:BaseMale'
const account = (userId: string) => ({
  userId,
  name: 'Before',
  isGuest: false,
  avatar: { bodyShapeUrn: BODY, eyesColor: null, hairColor: null, skinColor: null },
  wearables: ['urn:shirt'],
  emotes: [],
  forceRender: []
})
let player: ReturnType<typeof account> | null
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => player }))

const setAvatar = vi.fn<(data: SetAvatarData) => Promise<unknown>>()
const getUserProfile = vi.fn()
vi.mock('../../bridge-scene/src/bevy-api', () => ({
  BevyApi: { setAvatar: (data: SetAvatarData) => setAvatar(data), getUserProfile: (address: string) => getUserProfile(address) }
}))
vi.mock('../../bridge-scene/src/http', () => ({ catalystBase: async () => 'https://catalyst.example', getJson: async () => null }))
vi.mock('../../bridge-scene/src/domains/collections', () => ({ resolveDefsByUrn: async () => new Map() }))
vi.mock('../../bridge-scene/src/domains/wearables', () => ({}))
vi.mock('../../bridge-scene/src/domains/emotes', () => ({}))
vi.mock('../../bridge-scene/src/domains/emoteSlots', () => ({}))
vi.mock('../../bridge-scene/src/system-helpers', () => ({ relay: () => {} }))

async function bridge() {
  vi.resetModules()
  const draft = await import('../../bridge-scene/src/domains/avatarDraft')
  const { registerProfile } = await import('../../bridge-scene/src/domains/profile')
  const { identity } = await import('../../bridge-scene/src/identity')
  const handlers = new Map<string, (msg: PageToScene) => void | Promise<void>>()
  const sent: SceneToPage[] = []
  const ctx = {
    send: (msg: SceneToPage) => sent.push(msg),
    on: (kind: string, handler: (msg: PageToScene) => void | Promise<void>) => handlers.set(kind, handler),
    push: () => {}
  } as unknown as Ctx
  registerProfile(ctx)
  draft.registerAvatarDraft(ctx, async () => {})
  identity.observe(player?.userId ?? null)
  return {
    request: async (msg: PageToScene) => await handlers.get(msg.kind)?.(msg),
    replies: () => sent.filter((msg) => msg.kind === 'profileSaved' || msg.kind === 'avatarSaveFailed'),
    draft,
    identity
  }
}

beforeEach(() => {
  player = account('0xabc')
  setAvatar.mockReset()
  getUserProfile.mockReset()
  getUserProfile.mockResolvedValue({ name: 'After', description: 'New', version: 7 })
})

describe('Passport and Backpack deployments', () => {
  it('waits for the Passport result before submitting Backpack closes from the same turn', async () => {
    const { request, replies, draft } = await bridge()
    const passport = deferred()
    const backpack = deferred()
    setAvatar.mockReturnValueOnce(passport.promise).mockReturnValueOnce(backpack.promise).mockResolvedValueOnce(8)

    const save = request({ kind: 'saveProfile', description: 'New' })
    draft.editLook({ wearables: ['urn:shirt', 'urn:hat'] })
    const firstClose = request({ kind: 'commitAvatar' })
    draft.editLook({ wearables: ['urn:shirt', 'urn:hat', 'urn:glasses'], hair: { r: 1, g: 0, b: 0 } })
    const secondClose = request({ kind: 'commitAvatar' })
    await vi.waitFor(() => expect(setAvatar).toHaveBeenCalledTimes(1))
    expect(replies()).toEqual([])

    passport.resolve(6)
    await save
    await vi.waitFor(() => expect(setAvatar).toHaveBeenCalledTimes(2))
    expect(replies()).toEqual([{ kind: 'profileSaved', ok: true }])
    backpack.resolve(7)
    await Promise.all([firstClose, secondClose])
    expect(setAvatar).toHaveBeenCalledTimes(3)
    expect(setAvatar.mock.lastCall?.[0]).toMatchObject({
      preserveName: true,
      equip: { wearableUrns: ['urn:shirt', 'urn:hat', 'urn:glasses'] }
    })
    expect(draft.hasDraft()).toBe(false)
  })

  it('keeps a queued Backpack close that restores the earlier look', async () => {
    const { request, draft } = await bridge()
    const passport = deferred()
    setAvatar.mockReturnValueOnce(passport.promise).mockResolvedValue(7)

    const save = request({ kind: 'saveProfile', description: 'New' })
    draft.editLook({ wearables: ['urn:shirt', 'urn:hat'] })
    const equip = request({ kind: 'commitAvatar' })
    draft.editLook({ wearables: ['urn:shirt'] })
    const undo = request({ kind: 'commitAvatar' })

    passport.resolve(6)
    await Promise.all([save, equip, undo])
    expect(setAvatar.mock.calls.slice(1).map(([data]) => data.equip?.wearableUrns))
      .toEqual([['urn:shirt', 'urn:hat'], ['urn:shirt']])
    expect(draft.hasDraft()).toBe(false)
  })

  it('waits for an existing Backpack write and reports a genuine Passport failure', async () => {
    const { request, replies, draft } = await bridge()
    const backpack = deferred()
    setAvatar.mockReturnValueOnce(backpack.promise)
      .mockRejectedValueOnce(new Error('failed to deploy to server.'))
      .mockResolvedValueOnce(8)
    vi.spyOn(console, 'error').mockImplementation(() => {})

    draft.editLook({ wearables: ['urn:shirt', 'urn:hat'] })
    const close = request({ kind: 'commitAvatar' })
    await vi.waitFor(() => expect(setAvatar).toHaveBeenCalledTimes(1))
    const save = request({ kind: 'saveProfile', description: 'New' })
    await Promise.resolve()
    expect(setAvatar).toHaveBeenCalledTimes(1)

    backpack.resolve(6)
    await Promise.all([close, save])
    expect(replies()).toEqual([{ kind: 'profileSaved', ok: false, error: 'failed to deploy to server.' }])
    await request({ kind: 'saveProfile', description: 'Retry' })
    expect(replies().at(-1)).toEqual({ kind: 'profileSaved', ok: true })
  })

  it('abandons queued writes and late results when the account changes', async () => {
    const { request, replies, draft, identity } = await bridge()
    const oldSave = deferred()
    setAvatar.mockReturnValueOnce(oldSave.promise).mockResolvedValueOnce(7)

    const save = request({ kind: 'saveProfile', description: 'Old account' })
    draft.editLook({ wearables: ['urn:hat'] })
    const close = request({ kind: 'commitAvatar' })
    await vi.waitFor(() => expect(setAvatar).toHaveBeenCalledTimes(1))

    player = account('0xdef')
    identity.observe(player.userId)
    await request({ kind: 'saveProfile', description: 'New account' })
    oldSave.resolve(6)
    await Promise.all([save, close])
    expect(setAvatar.mock.calls.map(([data]) => data.profileExtras?.description)).toEqual(['Old account', 'New account'])
    expect(replies()).toEqual([{ kind: 'profileSaved', ok: true }])
  })
})

describe('a Backpack commit that a later save replaced', () => {
  async function overlap() {
    const { draft, replies, request } = await bridge()
    const close = async () => await request({ kind: 'commitAvatar' })
    const first = deferred()
    const later = deferred()
    setAvatar.mockReturnValueOnce(first.promise).mockReturnValueOnce(later.promise)

    draft.editLook({ wearables: ['urn:shirt', 'urn:hat'] })
    const firstClose = close()
    await vi.waitFor(() => expect(setAvatar).toHaveBeenCalledTimes(1))
    const secondClose = close()
    await vi.waitFor(() => expect(setAvatar).toHaveBeenCalledTimes(2))
    first.reject(new Error('cancelled'))
    await firstClose
    expect(draft.hasDraft()).toBe(true)
    expect(replies()).toEqual([])
    return { draft, replies, close, later, secondClose }
  }

  it('clears the draft when the later close deploys', async () => {
    const { draft, later, secondClose, close } = await overlap()
    later.resolve(7)
    await secondClose
    expect(draft.hasDraft()).toBe(false)
    await close()
    expect(setAvatar).toHaveBeenCalledTimes(2)
  })

  it('retains the draft and reports the failure of the later close', async () => {
    const { draft, replies, later, secondClose, close } = await overlap()
    vi.spyOn(console, 'error').mockImplementation(() => {})
    later.reject(new Error('failed to deploy to server.'))
    await secondClose
    expect(draft.hasDraft()).toBe(true)
    expect(replies()).toEqual([{ kind: 'avatarSaveFailed', message: 'Error: failed to deploy to server.' }])

    setAvatar.mockResolvedValueOnce(8)
    await close()
    expect(setAvatar).toHaveBeenCalledTimes(3)
    expect(draft.hasDraft()).toBe(false)
  })

  it('keeps edits made while the later close was deploying', async () => {
    const { draft, later, secondClose } = await overlap()
    draft.editLook({ wearables: ['urn:shirt', 'urn:hat', 'urn:glasses'] })
    later.resolve(7)
    await secondClose
    expect(draft.hasDraft()).toBe(true)
    expect(draft.currentLook()?.wearables).toEqual(['urn:shirt', 'urn:hat', 'urn:glasses'])
  })
})
