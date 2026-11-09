import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import type { Ctx } from '../../bridge-scene/src/bridge'
import type { AvatarLook } from '../engine/avatarEquip'
import type { OutfitsData, OutfitThumbnailData } from '../engine/generated'
import type { OutfitsMetadata, PageToScene, SceneToPage } from '../engine/protocol'

const FEMALE = 'urn:decentraland:off-chain:base-avatars:BaseFemale'
const PICTURE = 'data:image/png;base64,AAAA'

let player: { userId: string; isGuest: boolean } | null = null
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => player }))

const deployOutfits = vi.fn<(outfits: OutfitsData) => Promise<void>>()
const getOutfitThumbnails = vi.fn<() => Promise<OutfitThumbnailData[]>>()
const setOutfitThumbnail = vi.fn<(slot: number, thumbnail: string | null) => void>()
vi.mock('../../bridge-scene/src/bevy-api', () => ({
  BevyApi: {
    deployOutfits: (outfits: OutfitsData) => deployOutfits(outfits),
    getOutfitThumbnails: () => getOutfitThumbnails(),
    setOutfitThumbnail: (slot: number, thumbnail: string | null) => setOutfitThumbnail(slot, thumbnail)
  }
}))

const getJson = vi.fn<(url: string) => Promise<unknown>>()
vi.mock('../../bridge-scene/src/http', () => ({
  catalystBase: async () => 'https://catalyst.example',
  getJson: (url: string) => getJson(url)
}))

let look: AvatarLook | null = null
vi.mock('../../bridge-scene/src/domains/avatarDraft', () => ({
  currentLook: () => look,
  editLook: () => {}
}))
vi.mock('../../bridge-scene/src/domains/avatarPreview', () => ({
  captureAvatarThumbnail: async () => PICTURE,
  waitFrames: async () => {}
}))
vi.mock('../../bridge-scene/src/domains/wearables', () => ({ sendEquipped: async () => {} }))

const grey = { color: { r: 0.5, g: 0.5, b: 0.5 } }
const outfit = (wearables: string[]) => ({
  bodyShape: FEMALE,
  eyes: grey,
  hair: grey,
  skin: grey,
  wearables,
  forceRender: []
})

type Handler = (msg: PageToScene) => Promise<unknown>

/** A fresh outfits module on a bridge that records what it sends to the page. */
async function bridge() {
  vi.resetModules()
  const { registerOutfits } = await import('../../bridge-scene/src/domains/outfits')
  const sent: SceneToPage[] = []
  const handlers = new Map<string, Handler>()
  const ctx: Ctx = {
    send: (m) => sent.push(m),
    on: (kind, h) => handlers.set(kind, h as Handler),
    push: () => {}
  }
  registerOutfits(ctx)
  const request = (msg: PageToScene): Promise<unknown> => handlers.get(msg.kind)!(msg)
  const lastOutfits = (): OutfitsMetadata | undefined =>
    sent.flatMap((m) => (m.kind === 'outfits' ? [m.metadata] : [])).at(-1)
  return { request, lastOutfits }
}

describe('the bridge outfits domain', () => {
  let stored: OutfitsMetadata

  beforeEach(() => {
    look = { bodyShape: FEMALE, wearables: ['urn:a'], forceRender: [] } as unknown as AvatarLook
    stored = { outfits: [{ slot: 0, outfit: outfit(['urn:old']) }], namesForExtraSlots: [] }
    getJson.mockResolvedValue({ metadata: stored })
    getOutfitThumbnails.mockResolvedValue([])
    deployOutfits.mockResolvedValue(undefined)
  })

  afterEach(() => {
    vi.clearAllMocks()
    player = null
  })

  describe('when a signed-in player saves the current look', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
    })

    it('should deploy every slot without the pictures and show the new outfit with its picture', async () => {
      const { request, lastOutfits } = await bridge()
      await request({ kind: 'saveOutfit', slot: 1 })
      expect(deployOutfits).toHaveBeenCalledWith({
        outfits: [
          { slot: 0, outfit: outfit(['urn:old']) },
          { slot: 1, outfit: outfit(['urn:a']) }
        ],
        namesForExtraSlots: []
      })
      expect(setOutfitThumbnail).toHaveBeenCalledWith(1, PICTURE)
      expect(lastOutfits()?.outfits.find((o) => o.slot === 1)?.thumbnail).toBe(PICTURE)
    })

    it('should read the outfits it saved rather than the catalyst again', async () => {
      const { request, lastOutfits } = await bridge()
      await request({ kind: 'saveOutfit', slot: 1 })
      getJson.mockClear()
      await request({ kind: 'getOutfits' })
      expect(getJson).not.toHaveBeenCalled()
      expect(lastOutfits()?.outfits.map((o) => o.slot)).toEqual([0, 1])
    })
  })

  describe('when the catalyst refuses the deploy', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      deployOutfits.mockRejectedValue(new Error('failed to deploy'))
    })

    it('should show the outfits as they were, keep no picture and fail the request', async () => {
      const { request, lastOutfits } = await bridge()
      await expect(request({ kind: 'saveOutfit', slot: 1 })).rejects.toThrow('failed to deploy')
      expect(lastOutfits()?.outfits.map((o) => o.slot)).toEqual([0])
      expect(setOutfitThumbnail).not.toHaveBeenCalled()
    })
  })

  describe('when a guest saves the current look', () => {
    beforeEach(() => {
      player = { userId: '0xB', isGuest: true }
    })

    it('should keep the outfit for the session without deploying it', async () => {
      const { request, lastOutfits } = await bridge()
      await request({ kind: 'saveOutfit', slot: 1 })
      expect(deployOutfits).not.toHaveBeenCalled()
      expect(lastOutfits()?.outfits.map((o) => o.slot)).toEqual([0, 1])
    })
  })

  describe('when the player deletes a saved outfit', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
    })

    it('should deploy the remaining slots and drop the slot picture', async () => {
      const { request, lastOutfits } = await bridge()
      await request({ kind: 'deleteOutfit', slot: 0 })
      expect(deployOutfits).toHaveBeenCalledWith({ outfits: [], namesForExtraSlots: [] })
      expect(setOutfitThumbnail).toHaveBeenCalledWith(0, null)
      expect(lastOutfits()?.outfits).toEqual([])
    })
  })

  describe('when the engine holds pictures from an earlier session', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      getOutfitThumbnails.mockResolvedValue([{ slot: 0, thumbnail: PICTURE }])
    })

    it('should show them on the catalyst outfits', async () => {
      const { request, lastOutfits } = await bridge()
      await request({ kind: 'getOutfits' })
      expect(lastOutfits()?.outfits[0]?.thumbnail).toBe(PICTURE)
    })
  })
})
