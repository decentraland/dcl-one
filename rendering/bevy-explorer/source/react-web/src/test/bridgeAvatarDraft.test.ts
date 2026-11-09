import { describe, it, expect, vi, beforeEach } from 'vitest'
import type { Ctx } from '../../bridge-scene/src/bridge'
import type { SceneToPage } from '../engine/protocol'

// The bridge's Backpack draft (bridge-scene/src/domains/avatarDraft): a commit that a later
// setAvatar replaced ("cancelled") is settled by that save's outcome.

const BODY = 'urn:decentraland:off-chain:base-avatars:BaseMale'

let player: Record<string, unknown> | null = null
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => player }))

const setAvatar = vi.fn<(data: unknown) => Promise<unknown>>()
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: { setAvatar: (data: unknown) => setAvatar(data) } }))
vi.mock('../../bridge-scene/src/http', () => ({ catalystBase: async () => 'https://catalyst.example', getJson: async () => null }))
vi.mock('../../bridge-scene/src/domains/collections', () => ({ resolveDefsByUrn: async () => new Map() }))

type Handler = (msg: { kind: string }) => unknown

/** A fresh draft module, registered on a bridge whose Backpack `close` sends `commitAvatar`. */
async function backpack() {
  vi.resetModules()
  const draft = await import('../../bridge-scene/src/domains/avatarDraft')
  const handlers = new Map<string, Handler>()
  const sent: SceneToPage[] = []
  const ctx = {
    send: (msg: SceneToPage) => {
      sent.push(msg)
    },
    on: (kind: string, handler: Handler) => {
      handlers.set(kind, handler)
    },
    push: () => {}
  } as unknown as Ctx
  draft.registerAvatarDraft(ctx, async () => {})
  const close = async (): Promise<void> => {
    await handlers.get('commitAvatar')?.({ kind: 'commitAvatar' })
  }
  return { draft, sent, close }
}

describe('a Backpack commit that a later save replaced', () => {
  beforeEach(() => {
    setAvatar.mockReset()
    player = {
      userId: '0xabc',
      name: 'me',
      avatar: { bodyShapeUrn: BODY, eyesColor: null, hairColor: null, skinColor: null },
      wearables: ['urn:shirt'],
      emotes: [],
      forceRender: []
    }
  })

  it('is settled when the profile save that replaced it deploys', async () => {
    const { draft, close } = await backpack()
    draft.editLook({ wearables: ['urn:shirt', 'urn:hat'] })
    setAvatar.mockRejectedValueOnce(new Error('cancelled'))
    await close()
    expect(draft.hasDraft()).toBe(true)

    draft.profileSaveSettled(true)
    expect(draft.hasDraft()).toBe(false)
    // nothing is left to deploy on the next close
    await close()
    expect(setAvatar).toHaveBeenCalledTimes(1)
  })

  it('stays for the next close when that save fails', async () => {
    const { draft, sent, close } = await backpack()
    draft.editLook({ wearables: ['urn:shirt', 'urn:hat'] })
    setAvatar.mockRejectedValueOnce(new Error('cancelled'))
    await close()
    draft.profileSaveSettled(false)
    expect(draft.hasDraft()).toBe(true)

    setAvatar.mockResolvedValueOnce(7)
    await close()
    expect(setAvatar).toHaveBeenCalledTimes(2)
    expect(draft.hasDraft()).toBe(false)
    expect(sent).toEqual([])
  })

  it('keeps the edits made after it', async () => {
    const { draft, close } = await backpack()
    draft.editLook({ wearables: ['urn:shirt', 'urn:hat'] })
    setAvatar.mockRejectedValueOnce(new Error('cancelled'))
    await close()
    draft.editLook({ wearables: ['urn:shirt', 'urn:hat', 'urn:glasses'] })

    draft.profileSaveSettled(true)
    expect(draft.hasDraft()).toBe(true)
    expect(draft.currentLook()?.wearables).toEqual(['urn:shirt', 'urn:hat', 'urn:glasses'])
  })

  it('a profile save leaves a draft no commit sent alone', async () => {
    const { draft } = await backpack()
    draft.editLook({ wearables: ['urn:shirt', 'urn:hat'] })
    draft.profileSaveSettled(true)
    expect(draft.hasDraft()).toBe(true)
  })
})
