import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { PageToScene, SceneToPage } from '@explorer-core/engine/protocol'
import type { SetAvatarData } from '@explorer-core/engine/generated'
import type { Ctx } from '../../bridge-scene/src/bridge'

function deferred<T = number>() {
  let resolve!: (value: T) => void
  let reject!: (error: Error) => void
  const promise = new Promise<T>((res, rej) => {
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
let equipped = true
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/ecs.js', () => ({
  AvatarEquippedData: { has: () => equipped },
  engine: { PlayerEntity: 1 }
}))
vi.mock('~system/RestrictedActions', () => ({ openExternalUrl: vi.fn() }))
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => player }))

const setAvatar = vi.fn<(data: SetAvatarData) => Promise<number>>()
const getUserProfile = vi.fn()
vi.mock('../../bridge-scene/src/bevy-api', () => ({
  BevyApi: {
    setAvatar: (data: SetAvatarData) => setAvatar(data),
    getUserProfile: (address: string) => getUserProfile(address),
    logout: vi.fn()
  }
}))
vi.mock('../../bridge-scene/src/http', () => ({ catalystBase: async () => 'https://catalyst.example', getJson: async () => null }))
const resolveDefs = vi.fn<() => Promise<Map<string, unknown>>>()
vi.mock('../../bridge-scene/src/domains/collections', () => ({ resolveDefsByUrn: () => resolveDefs() }))
vi.mock('../../bridge-scene/src/domains/wearables', () => ({}))
vi.mock('../../bridge-scene/src/domains/emotes', () => ({}))
vi.mock('../../bridge-scene/src/domains/emoteSlots', () => ({}))
const waitFrame = vi.fn<() => Promise<void>>()
vi.mock('../../bridge-scene/src/domains/avatarPreview', () => ({ syncShape: () => {}, waitFrames: () => waitFrame() }))
vi.mock('../../bridge-scene/src/system-helpers', () => ({ relay: () => {} }))

async function bridge() {
  vi.resetModules()
  const draft = await import('../../bridge-scene/src/domains/avatarDraft')
  const { registerProfile } = await import('../../bridge-scene/src/domains/profile')
  const { identity } = await import('../../bridge-scene/src/identity')
  const { registerSession } = await import('../../bridge-scene/src/domains/session')
  const { registerWelcome } = await import('../../bridge-scene/src/domains/welcome')
  const frames: Array<(dt: number) => void> = []
  const handlers = new Map<string, (msg: PageToScene) => void | Promise<void>>()
  const sent: SceneToPage[] = []
  const ctx = {
    send: (msg: SceneToPage) => sent.push(msg),
    on: (kind: string, handler: (msg: PageToScene) => void | Promise<void>) => handlers.set(kind, handler),
    push: (frame: (dt: number) => void) => frames.push(frame)
  } as unknown as Ctx
  registerProfile(ctx)
  registerSession(ctx)
  registerWelcome(ctx)
  draft.registerAvatarDraft(ctx, async () => {})
  identity.observe(player?.userId ?? null)
  return {
    request: async (msg: PageToScene) => await handlers.get(msg.kind)?.(msg),
    frame: () => frames.forEach((frame) => frame(1 / 60)),
    replies: () => sent.filter((msg) => msg.kind === 'profileSaved' || msg.kind === 'avatarSaveFailed' || msg.kind === 'welcomeAccepted'),
    draft,
    identity
  }
}

beforeEach(() => {
  player = account('0xabc')
  equipped = true
  waitFrame.mockReset().mockResolvedValue(undefined)
  resolveDefs.mockReset().mockResolvedValue(new Map())
  setAvatar.mockReset()
  getUserProfile.mockReset()
  getUserProfile.mockResolvedValue({ name: 'After', description: 'New', version: 7 })
})

describe('Passport and Backpack deployments', () => {
  it.each(['0xAaA', ''])('refuses a Passport edit for a different or unknown account (%s)', async (address) => {
    const { request, replies } = await bridge()
    await request({ kind: 'saveProfile', requestId: 12, address, description: 'Account A only' })
    expect(setAvatar).not.toHaveBeenCalled()
    expect(getUserProfile).not.toHaveBeenCalled()
    expect(replies()).toEqual([{ kind: 'profileSaved', requestId: 12, ok: false, accountChanged: true, error: 'The active account changed.' }])
  })

  it('submits Backpack during Passport and relays the success of the deploy that carried both', async () => {
    const { request, replies, draft } = await bridge()
    const passport = deferred()
    const backpack = deferred()
    setAvatar.mockReturnValueOnce(passport.promise).mockReturnValueOnce(backpack.promise)
    const save = request({ kind: 'saveProfile', requestId: 42, address: '0xABC', description: 'New' })
    draft.editLook({ wearables: ['urn:hat'] })
    const close = request({ kind: 'commitAvatar' })
    await vi.waitFor(() => expect(setAvatar).toHaveBeenCalledTimes(2))
    expect(setAvatar.mock.calls.map(([patch]) => patch)).toEqual([
      { profileExtras: { description: 'New' } },
      { equip: { wearableUrns: ['urn:hat'], emoteUrns: [], forceRender: [] } }
    ])
    expect(replies()).toEqual([])
    backpack.resolve(7)
    passport.resolve(7)
    await Promise.all([save, close])
    expect(replies()).toMatchObject([{ kind: 'profileSaved', requestId: 42, ok: true, profile: { description: 'New', version: 7 } }])
    expect(draft.hasDraft()).toBe(false)
  })

  it('reports the committed profile when the later Passport deployment fails and Backpack is still pending', async () => {
    const { request, replies, draft } = await bridge()
    const backpack = deferred()
    setAvatar.mockReturnValueOnce(backpack.promise).mockRejectedValueOnce(Object.assign(
      new Error('deploy failed'), { profile: { name: 'Last good', version: 5, description: 'Committed' } }
    ))
    vi.spyOn(console, 'error').mockImplementation(() => {})
    draft.editLook({ wearables: ['urn:hat'] })
    const close = request({ kind: 'commitAvatar' })
    await vi.waitFor(() => expect(setAvatar).toHaveBeenCalledTimes(1))
    await request({ kind: 'saveProfile', requestId: 12, description: 'New' })
    expect(replies()).toMatchObject([{ kind: 'profileSaved', requestId: 12, ok: false, error: 'deploy failed',
      profile: { version: 5, description: 'Committed' } }])
    expect(getUserProfile).not.toHaveBeenCalled()
    backpack.resolve(6)
    await close
    expect(draft.hasDraft()).toBe(false)
  })

  it('preserves an explicitly absent committed profile on the first deploy failure', async () => {
    const { request, replies } = await bridge()
    vi.spyOn(console, 'error').mockImplementation(() => {})
    setAvatar.mockRejectedValueOnce(Object.assign(new Error('first deploy failed'), { profile: null }))
    await request({ kind: 'saveProfile', description: 'First edit' })
    expect(replies()).toMatchObject([{ kind: 'profileSaved', ok: false, profile: null }])
    expect(getUserProfile).not.toHaveBeenCalled()
  })

  it('submits independent about-me fields and explicit clears, including both birthday spellings', async () => {
    const { request } = await bridge()
    setAvatar.mockResolvedValue(7)
    await request({ kind: 'saveProfile', name: '', info: { country: 'Canada', realName: '' } })
    await request({ kind: 'saveProfile', info: { birthdate: '' } })
    expect(setAvatar.mock.calls.map(([patch]) => patch)).toEqual([
      { profileExtras: { country: 'Canada', realName: null } },
      { profileExtras: { birthDate: null, birthdate: null } }
    ])
  })

  it.each([undefined, 'Welcome name'])('only names the Welcome look when supplied (%s)', async (name) => {
    const { request, draft } = await bridge()
    const passport = deferred()
    setAvatar.mockReturnValueOnce(passport.promise).mockResolvedValueOnce(7)
    const save = request({ kind: 'saveProfile', name: 'Passport name' })
    await vi.waitFor(() => expect(setAvatar).toHaveBeenCalledTimes(1))
    draft.editLook({ wearables: ['urn:hat'] })
    await request({ kind: 'acceptWelcome', name })
    expect(setAvatar.mock.calls[1][0]).toMatchObject({ base: { name: name ?? '' }, equip: { wearableUrns: ['urn:hat'] } })
    expect(setAvatar.mock.calls[1][0].hasClaimedName).toBeUndefined()
    passport.resolve(7)
    await save
  })

  it('waits for a signed-in player before submitting the Welcome stand-in', async () => {
    const { request, replies, frame, draft } = await bridge()
    const look = draft.currentLook()!
    player = null
    equipped = false
    frame()
    await request({ kind: 'showLook', avatar: look })
    frame()
    const deploy = deferred()
    setAvatar.mockReturnValueOnce(deploy.promise)

    const nextFrame = deferred<void>()
    waitFrame.mockReturnValueOnce(nextFrame.promise)
    const accept = request({ kind: 'acceptWelcome', name: 'Welcome name' })
    await vi.waitFor(() => expect(waitFrame).toHaveBeenCalledTimes(1))
    expect(setAvatar).not.toHaveBeenCalled()
    player = account('0xabc')
    equipped = true
    frame()
    nextFrame.resolve()
    await vi.waitFor(() => expect(setAvatar).toHaveBeenCalledTimes(1))
    deploy.resolve(6)
    await accept
    expect(replies()).toEqual([{ kind: 'welcomeAccepted', ok: true }])
  })

  it.each(['logout RPC', 'engine clearing the player'])('answers the invalidated save after %s even for the same wallet', async (signal) => {
    const { request, replies, frame } = await bridge()
    const old = deferred()
    setAvatar.mockReturnValueOnce(old.promise).mockResolvedValueOnce(7)
    const save = request({ kind: 'saveProfile', requestId: 21, address: '0xabc', description: 'Before logout' })
    if (signal === 'logout RPC') {
      await request({ kind: 'rpc:req', id: 'logout', method: 'logout' })
    } else {
      player = null
      frame()
      player = account('0xabc')
      frame()
    }
    await request({ kind: 'saveProfile', requestId: 22, address: '0xabc', description: 'After sign-in' })
    old.reject(new Error('old account'))
    await save
    expect(replies()).toMatchObject([
      { kind: 'profileSaved', requestId: 22, ok: true },
      { kind: 'profileSaved', requestId: 21, ok: false, accountChanged: true }
    ])
  })

  it('answers a successful save whose confirming read outlives the login', async () => {
    const { request, replies, frame } = await bridge()
    const oldRead = deferred<{ name: string }>()
    setAvatar.mockResolvedValue(7)
    getUserProfile.mockReturnValueOnce(oldRead.promise)
    const save = request({ kind: 'saveProfile', address: '0xabc', requestId: 23, description: 'old edit' })
    await vi.waitFor(() => expect(getUserProfile).toHaveBeenCalledWith('0xabc'))
    player = account('0xdef')
    frame()
    oldRead.resolve({ name: 'old account' })
    await save
    expect(replies()).toEqual([{ kind: 'profileSaved', ok: false, requestId: 23,
      accountChanged: true, error: 'The active account changed.' }])
  })

  it('saves a re-seeded draft relative to confirmed colors after an outstanding close fails', async () => {
    const { request, draft } = await bridge()
    const close = deferred()
    setAvatar.mockReturnValueOnce(close.promise).mockResolvedValueOnce(7)
    vi.spyOn(console, 'error').mockImplementation(() => {})
    const red = { r: 1, g: 0, b: 0 }
    draft.editLook({ hair: red })
    const first = request({ kind: 'commitAvatar' })
    await vi.waitFor(() => expect(setAvatar).toHaveBeenCalledTimes(1))
    Object.assign(player!.avatar, { hairColor: red })
    await request({ kind: 'revertAvatar' })
    draft.editLook({ wearables: ['urn:hat'] })
    close.reject(new Error('deploy failed'))
    await first
    player!.avatar.hairColor = null
    await request({ kind: 'commitAvatar' })
    expect(setAvatar.mock.calls[1][0]).toMatchObject({
      base: { hairColor: red }, equip: { wearableUrns: ['urn:hat'] }
    })
  })

  it('sends an undo against an in-flight hat change, including when the undo settles first', async () => {
    const { request, replies, draft } = await bridge()
    const hat = deferred()
    const undo = deferred()
    setAvatar.mockReturnValueOnce(hat.promise).mockReturnValueOnce(undo.promise)
    draft.editLook({ wearables: ['urn:shirt', 'urn:hat'] })
    const add = request({ kind: 'commitAvatar' })
    await vi.waitFor(() => expect(setAvatar).toHaveBeenCalledTimes(1))
    draft.editLook({ wearables: ['urn:shirt'] })
    const remove = request({ kind: 'commitAvatar' })
    await vi.waitFor(() => expect(setAvatar).toHaveBeenCalledTimes(2))
    expect(setAvatar.mock.calls.map(([patch]) => patch.equip?.wearableUrns)).toEqual([['urn:shirt', 'urn:hat'], ['urn:shirt']])
    undo.resolve(7)
    await remove
    hat.resolve(7)
    await add
    expect(replies()).toEqual([])
    expect(draft.hasDraft()).toBe(false)
  })

  it('drops an older fitting result that arrives after the player undoes the unsent edit', async () => {
    const { request, draft } = await bridge()
    const fit = deferred<Map<string, unknown>>()
    resolveDefs.mockReturnValueOnce(fit.promise)
    draft.editLook({ wearables: ['urn:hat'] })
    const add = request({ kind: 'commitAvatar' })
    await vi.waitFor(() => expect(resolveDefs).toHaveBeenCalledTimes(1))
    draft.editLook({ wearables: ['urn:shirt'] })
    await request({ kind: 'commitAvatar' })
    fit.resolve(new Map())
    await add
    expect(setAvatar).not.toHaveBeenCalled()
    expect(draft.hasDraft()).toBe(false)
  })

  it('discards an unsent fitting result when the player reverts the draft', async () => {
    const { request, draft } = await bridge()
    const fit = deferred<Map<string, unknown>>()
    resolveDefs.mockReturnValueOnce(fit.promise)
    draft.editLook({ wearables: ['urn:hat'] })
    const close = request({ kind: 'commitAvatar' })
    await vi.waitFor(() => expect(resolveDefs).toHaveBeenCalledTimes(1))
    await request({ kind: 'revertAvatar' })
    fit.resolve(new Map())
    await close
    expect(setAvatar).not.toHaveBeenCalled()
    expect(draft.hasDraft()).toBe(false)
  })

  it('carries an unconfirmed hat into a later color save even when the hat deployment fails', async () => {
    const { request, draft } = await bridge()
    const hat = deferred()
    const color = deferred()
    setAvatar.mockReturnValueOnce(hat.promise).mockReturnValueOnce(color.promise)
    vi.spyOn(console, 'error').mockImplementation(() => {})
    draft.editLook({ wearables: ['urn:hat'] })
    const add = request({ kind: 'commitAvatar' })
    await vi.waitFor(() => expect(setAvatar).toHaveBeenCalledTimes(1))
    draft.editLook({ hair: { r: 1, g: 0, b: 0 } })
    const tint = request({ kind: 'commitAvatar' })
    await vi.waitFor(() => expect(setAvatar).toHaveBeenCalledTimes(2))
    expect(setAvatar.mock.calls[1][0]).toMatchObject({
      equip: { wearableUrns: ['urn:hat'] }, base: { hairColor: { r: 1, g: 0, b: 0 }, eyesColor: null, bodyShapeUrn: '' }
    })
    hat.reject(new Error('deploy failed'))
    await add
    color.resolve(7)
    await tint
    expect(draft.hasDraft()).toBe(false)
  })

  it('retries from the last successful look after both overlapping changes fail', async () => {
    const { request, draft } = await bridge()
    const hat = deferred()
    const color = deferred()
    setAvatar.mockReturnValueOnce(hat.promise).mockReturnValueOnce(color.promise).mockResolvedValueOnce(8)
    vi.spyOn(console, 'error').mockImplementation(() => {})
    draft.editLook({ wearables: ['urn:hat'] })
    const add = request({ kind: 'commitAvatar' })
    await vi.waitFor(() => expect(setAvatar).toHaveBeenCalledTimes(1))
    draft.editLook({ hair: { r: 1, g: 0, b: 0 } })
    const tint = request({ kind: 'commitAvatar' })
    await vi.waitFor(() => expect(setAvatar).toHaveBeenCalledTimes(2))
    expect(setAvatar.mock.calls[1][0].equip?.wearableUrns).toEqual(['urn:hat'])
    color.reject(new Error('deploy failed'))
    hat.reject(new Error('deploy failed'))
    await Promise.all([add, tint])
    expect(draft.hasDraft()).toBe(true)
    await request({ kind: 'commitAvatar' })
    expect(setAvatar.mock.calls[2][0]).toMatchObject({ base: { hairColor: { r: 1, g: 0, b: 0 } }, equip: { wearableUrns: ['urn:hat'] } })
    expect(draft.hasDraft()).toBe(false)
  })
})
