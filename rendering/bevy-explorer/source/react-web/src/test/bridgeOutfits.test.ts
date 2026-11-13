import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import type { Ctx } from '../../bridge-scene/src/bridge'
import type { AvatarLook } from '@explorer-core/engine/avatarEquip'
import type { OutfitsData, OutfitThumbnailData } from '@explorer-core/engine/generated'
import type { OutfitsMetadata, PageToScene, SceneToPage } from '@explorer-core/engine/protocol'

const FEMALE = 'urn:decentraland:off-chain:base-avatars:BaseFemale'
const PICTURE = 'data:image/png;base64,AAAA'

let player: { userId: string; isGuest: boolean } | null = null
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => player }))

const deployOutfits = vi.fn<(address: string, outfits: OutfitsData) => Promise<void>>()
const getOutfitThumbnails = vi.fn<(address: string) => Promise<OutfitThumbnailData[]>>()
const setOutfitThumbnail = vi.fn<(address: string, slot: number, thumbnail: string | null) => void>()
vi.mock('../../bridge-scene/src/bevy-api', () => ({
  BevyApi: {
    deployOutfits: (address: string, outfits: OutfitsData) => deployOutfits(address, outfits),
    getOutfitThumbnails: (address: string) => getOutfitThumbnails(address),
    setOutfitThumbnail: (address: string, slot: number, thumbnail: string | null) =>
      setOutfitThumbnail(address, slot, thumbnail)
  }
}))

const getJson = vi.fn<(url: string) => Promise<unknown>>()
vi.mock('../../bridge-scene/src/http', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../bridge-scene/src/http')>()),
  catalystBase: async () => 'https://catalyst.example',
  getJson: (url: string) => getJson(url)
}))

const fetchOwnedNames = vi.fn<(address: string, fresh?: boolean) => Promise<string[]>>()
vi.mock('../../bridge-scene/src/domains/profile', () => ({
  fetchOwnedNames: (address: string, fresh?: boolean) => fetchOwnedNames(address, fresh)
}))

let look: AvatarLook | null = null
vi.mock('../../bridge-scene/src/domains/avatarDraft', () => ({
  currentLook: () => look,
  editLook: () => {}
}))
const captureAvatarThumbnail = vi.fn<() => Promise<string | null>>()
const waitFrames = vi.fn<(frames: number) => Promise<void>>()
vi.mock('../../bridge-scene/src/domains/avatarPreview', () => ({
  captureAvatarThumbnail: () => captureAvatarThumbnail(),
  waitFrames: (frames: number) => waitFrames(frames)
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
  const failures = (): SceneToPage[] => sent.filter((m) => m.kind === 'outfitSaveFailed')
  return { request, lastOutfits, failures, sent }
}

describe('the bridge outfits domain', () => {
  let stored: OutfitsMetadata

  beforeEach(() => {
    look = { bodyShape: FEMALE, wearables: ['urn:a'], forceRender: [] } as unknown as AvatarLook
    stored = { outfits: [{ slot: 0, outfit: outfit(['urn:old']) }], namesForExtraSlots: [] }
    getJson.mockResolvedValue({ metadata: stored })
    getOutfitThumbnails.mockResolvedValue([])
    deployOutfits.mockResolvedValue(undefined)
    fetchOwnedNames.mockResolvedValue([])
    captureAvatarThumbnail.mockResolvedValue(PICTURE)
    waitFrames.mockResolvedValue(undefined)
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
      expect(deployOutfits).toHaveBeenCalledWith('0xA', {
        outfits: [
          { slot: 0, outfit: outfit(['urn:old']) },
          { slot: 1, outfit: outfit(['urn:a']) }
        ],
        namesForExtraSlots: []
      })
      expect(setOutfitThumbnail).toHaveBeenCalledWith('0xA', 1, PICTURE)
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

    describe('and the player was saving an outfit', () => {
      it('should show the outfits as they were and keep no picture', async () => {
        const { request, lastOutfits } = await bridge()
        await request({ kind: 'saveOutfit', slot: 1 })
        expect(lastOutfits()?.outfits.map((o) => o.slot)).toEqual([0])
        expect(setOutfitThumbnail).not.toHaveBeenCalled()
      })

      it('should tell the page the save failed after showing the outfits as they were', async () => {
        const { request, sent } = await bridge()
        await request({ kind: 'saveOutfit', slot: 1 })
        expect(sent.slice(-2)).toEqual([
          { kind: 'outfits', metadata: stored },
          { kind: 'outfitSaveFailed', slot: 1, action: 'save', message: 'Error: failed to deploy' }
        ])
      })
    })

    describe('and the player was deleting an outfit', () => {
      it('should keep the outfit and tell the page the delete failed', async () => {
        const { request, lastOutfits, failures } = await bridge()
        await request({ kind: 'deleteOutfit', slot: 0 })
        expect(lastOutfits()?.outfits.map((o) => o.slot)).toEqual([0])
        expect(setOutfitThumbnail).not.toHaveBeenCalled()
        expect(failures()).toEqual([
          { kind: 'outfitSaveFailed', slot: 0, action: 'delete', message: 'Error: failed to deploy' }
        ])
      })
    })
  })

  describe('when the catalyst read fails', () => {
    beforeEach(async () => {
      player = { userId: '0xA', isGuest: false }
      const { HttpError } = await import('../../bridge-scene/src/http')
      getJson.mockRejectedValue(new HttpError(503, 'unavailable'))
    })

    describe('and the player saves an outfit', () => {
      it('should deploy nothing and tell the page the save failed', async () => {
        const { request, failures } = await bridge()
        await request({ kind: 'saveOutfit', slot: 4 })
        expect(deployOutfits).not.toHaveBeenCalled()
        expect(setOutfitThumbnail).not.toHaveBeenCalled()
        expect(failures()).toEqual([
          { kind: 'outfitSaveFailed', slot: 4, action: 'save', message: 'Error: HTTP 503: unavailable' }
        ])
      })
    })

    describe('and the player deletes an outfit', () => {
      it('should deploy nothing and tell the page the delete failed', async () => {
        const { request, failures } = await bridge()
        await request({ kind: 'deleteOutfit', slot: 0 })
        expect(deployOutfits).not.toHaveBeenCalled()
        expect(failures()).toEqual([
          { kind: 'outfitSaveFailed', slot: 0, action: 'delete', message: 'Error: HTTP 503: unavailable' }
        ])
      })
    })

    describe('and the read succeeds by the next save', () => {
      it('should keep every stored outfit', async () => {
        const { request } = await bridge()
        await request({ kind: 'getOutfits' })
        getJson.mockResolvedValue({ metadata: stored })
        await request({ kind: 'saveOutfit', slot: 4 })
        expect(deployOutfits.mock.calls[0]?.[1].outfits.map((o) => o.slot)).toEqual([0, 4])
      })
    })
  })

  describe('when the player has never saved an outfit', () => {
    beforeEach(async () => {
      player = { userId: '0xA', isGuest: false }
      const { HttpError } = await import('../../bridge-scene/src/http')
      getJson.mockRejectedValue(new HttpError(404, '{"error":"Not Found","message":"Outfits not found"}'))
    })

    it('should deploy the new outfit alone', async () => {
      const { request, failures } = await bridge()
      await request({ kind: 'saveOutfit', slot: 2 })
      expect(deployOutfits).toHaveBeenCalledWith('0xA', { outfits: [{ slot: 2, outfit: outfit(['urn:a']) }], namesForExtraSlots: [] })
      expect(failures()).toEqual([])
    })
  })

  describe('when a name owner saves into an extra slot', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      fetchOwnedNames.mockResolvedValue(['somename'])
    })

    it('should deploy the outfits with the owned names', async () => {
      const { request, lastOutfits } = await bridge()
      await request({ kind: 'saveOutfit', slot: 6 })
      expect(fetchOwnedNames).toHaveBeenCalledWith('0xA', true)
      expect(deployOutfits).toHaveBeenCalledWith('0xA', {
        outfits: [
          { slot: 0, outfit: outfit(['urn:old']) },
          { slot: 6, outfit: outfit(['urn:a']) }
        ],
        namesForExtraSlots: ['somename']
      })
      expect(lastOutfits()?.namesForExtraSlots).toEqual(['somename'])
    })
  })

  describe('when the first read finishes after a save', () => {
    let finishRead: (value: unknown) => void

    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      getJson.mockImplementationOnce(() => new Promise((resolve) => (finishRead = resolve)))
    })

    it('should keep the saved outfit', async () => {
      const { request, lastOutfits } = await bridge()
      const reading = request({ kind: 'getOutfits' })
      await request({ kind: 'saveOutfit', slot: 1 })
      finishRead({ metadata: stored })
      await reading
      expect(lastOutfits()?.outfits.map((o) => o.slot)).toEqual([0, 1])
      deployOutfits.mockClear()
      await request({ kind: 'saveOutfit', slot: 2 })
      expect(deployOutfits.mock.calls[0]?.[1].outfits.map((o) => o.slot)).toEqual([0, 1, 2])
    })
  })

  describe('when the player saves into a slot that holds an outfit', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
    })

    it('should deploy nothing, show the stored outfits and tell the page the save failed', async () => {
      const { request, sent } = await bridge()
      await request({ kind: 'saveOutfit', slot: 0 })
      expect(deployOutfits).not.toHaveBeenCalled()
      expect(sent.slice(-2)).toEqual([
        { kind: 'outfits', metadata: stored },
        { kind: 'outfitSaveFailed', slot: 0, action: 'save', message: 'Error: slot 0 holds an outfit' }
      ])
    })
  })

  describe('when a first read answers 404 after a save', () => {
    let failRead: (reason: unknown) => void

    beforeEach(async () => {
      player = { userId: '0xA', isGuest: false }
      const { HttpError } = await import('../../bridge-scene/src/http')
      getJson.mockImplementationOnce(() => new Promise((_resolve, reject) => (failRead = reject)))
      getJson.mockRejectedValue(new HttpError(404, 'Outfits not found'))
    })

    it('should keep showing the saved outfit', async () => {
      const { request, lastOutfits } = await bridge()
      const reading = request({ kind: 'getOutfits' })
      await request({ kind: 'saveOutfit', slot: 1 })
      const { HttpError } = await import('../../bridge-scene/src/http')
      failRead(new HttpError(404, 'Outfits not found'))
      await reading
      expect(lastOutfits()?.outfits.map((o) => o.slot)).toEqual([1])
    })
  })

  describe('when the stored entity lists names but uses no extra slot', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      stored = { ...stored, namesForExtraSlots: ['sold'] }
      getJson.mockResolvedValue({ metadata: stored })
    })

    it('should deploy without names', async () => {
      const { request } = await bridge()
      await request({ kind: 'saveOutfit', slot: 1 })
      expect(deployOutfits.mock.calls[0]?.[1].namesForExtraSlots).toEqual([])
      expect(fetchOwnedNames).not.toHaveBeenCalled()
    })
  })

  describe('when the stored entity lists a name the player no longer owns', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      stored = { outfits: [{ slot: 6, outfit: outfit(['urn:old']) }], namesForExtraSlots: ['sold'] }
      getJson.mockResolvedValue({ metadata: stored })
      fetchOwnedNames.mockResolvedValue(['kept'])
    })

    it('should deploy the names the player owns now', async () => {
      const { request } = await bridge()
      await request({ kind: 'saveOutfit', slot: 1 })
      expect(deployOutfits.mock.calls[0]?.[1].namesForExtraSlots).toEqual(['kept'])
    })
  })

  describe('when a refused deploy is followed by another save', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      deployOutfits.mockRejectedValueOnce(new Error('failed to deploy'))
    })

    it('should read the catalyst again for the next save', async () => {
      const { request } = await bridge()
      await request({ kind: 'saveOutfit', slot: 1 })
      await request({ kind: 'saveOutfit', slot: 2 })
      expect(getJson).toHaveBeenCalledTimes(2)
      expect(deployOutfits.mock.calls[1]?.[1].outfits.map((o) => o.slot)).toEqual([0, 2])
    })
  })

  describe('when the capture is larger than the engine keeps', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      captureAvatarThumbnail.mockResolvedValue(`data:image/png;base64,${'A'.repeat(1024 * 1024)}`)
    })

    it('should neither show nor store a picture for the slot', async () => {
      const { request, lastOutfits } = await bridge()
      await request({ kind: 'saveOutfit', slot: 1 })
      expect(setOutfitThumbnail).toHaveBeenCalledWith('0xA', 1, null)
      expect(lastOutfits()?.outfits.find((o) => o.slot === 1)?.thumbnail).toBeUndefined()
    })
  })

  describe('when the player equips an outfit without a picture', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      look = { bodyShape: FEMALE, wearables: ['urn:old'], forceRender: [] } as unknown as AvatarLook
    })

    it('should picture it once its wearables loaded', async () => {
      const { request } = await bridge()
      await request({ kind: 'equipOutfit', slot: 0 })
      expect(setOutfitThumbnail).toHaveBeenCalledWith('0xA', 0, PICTURE)
    })

    describe('and the slot is deleted while the wearables load', () => {
      it('should not picture the deleted slot', async () => {
        const { request } = await bridge()
        waitFrames.mockImplementationOnce(async () => {
          await request({ kind: 'deleteOutfit', slot: 0 })
        })
        await request({ kind: 'equipOutfit', slot: 0 })
        expect(setOutfitThumbnail.mock.calls).toEqual([['0xA', 0, null]])
      })
    })

    describe('and the catalyst read fails', () => {
      beforeEach(async () => {
        const { HttpError } = await import('../../bridge-scene/src/http')
        getJson.mockRejectedValue(new HttpError(503, 'unavailable'))
      })

      it('should equip nothing without failing', async () => {
        const { request } = await bridge()
        await expect(request({ kind: 'equipOutfit', slot: 0 })).resolves.toBeUndefined()
        expect(setOutfitThumbnail).not.toHaveBeenCalled()
      })
    })
  })

  describe('when a guest saves the current look', () => {
    beforeEach(() => {
      player = { userId: '0xB', isGuest: true }
    })

    it('should keep the outfit for the session without deploying it', async () => {
      const { request, lastOutfits, failures } = await bridge()
      await request({ kind: 'saveOutfit', slot: 1 })
      expect(deployOutfits).not.toHaveBeenCalled()
      expect(lastOutfits()?.outfits.map((o) => o.slot)).toEqual([0, 1])
      expect(failures()).toEqual([])
    })
  })

  describe('when the player deletes a saved outfit', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
    })

    it('should deploy the remaining slots and drop the slot picture', async () => {
      const { request, lastOutfits } = await bridge()
      await request({ kind: 'deleteOutfit', slot: 0 })
      expect(deployOutfits).toHaveBeenCalledWith('0xA', { outfits: [], namesForExtraSlots: [] })
      expect(setOutfitThumbnail).toHaveBeenCalledWith('0xA', 0, null)
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
