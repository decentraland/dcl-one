import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import type { Ctx } from '../../bridge-scene/src/bridge'
import type { AvatarLook } from '@explorer-core/engine/avatarEquip'
import type { OutfitsData } from '@explorer-core/engine/generated'
import type { OutfitsMetadata, PageToScene, SceneToPage } from '@explorer-core/engine/protocol'

const FEMALE = 'urn:decentraland:off-chain:base-avatars:BaseFemale'
const PICTURE = 'data:image/png;base64,AAAA'

let player: { userId: string; isGuest: boolean } | null = null
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => player }))

const deployOutfits = vi.fn<(address: string, outfits: OutfitsData) => Promise<void>>()
vi.mock('../../bridge-scene/src/bevy-api', () => ({
  BevyApi: {
    deployOutfits: (address: string, outfits: OutfitsData) => deployOutfits(address, outfits)
  }
}))

// The scene's localStorage: what it holds, and each picture write in order.
const storage = new Map<string, string>()
const pictureWrites: Array<[string, string | null]> = []
const localStorage = {
  getItem: vi.fn((key: string) => storage.get(key) ?? null),
  setItem: vi.fn((key: string, value: string) => {
    storage.set(key, value)
    if (key.startsWith('outfit-thumb:')) pictureWrites.push([key, value])
  }),
  removeItem: vi.fn((key: string) => {
    storage.delete(key)
    if (key.startsWith('outfit-thumb:')) pictureWrites.push([key, null])
  })
}

const getJson = vi.fn<(url: string) => Promise<unknown>>()
vi.mock('../../bridge-scene/src/http', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../bridge-scene/src/http')>()),
  catalystBase: async () => 'https://catalyst.example',
  getJson: (url: string) => getJson(url)
}))

const readOwnedNames = vi.fn<(address: string) => Promise<string[] | undefined>>()
vi.mock('../../bridge-scene/src/domains/profile', () => ({
  readOwnedNames: (address: string) => readOwnedNames(address)
}))

let look: AvatarLook | null = null
const editLook = vi.fn<(change: Partial<AvatarLook>) => void>()
vi.mock('../../bridge-scene/src/domains/avatarDraft', () => ({
  currentLook: () => look,
  editLook: (change: Partial<AvatarLook>) => editLook(change)
}))
const captureAvatarThumbnail = vi.fn<() => Promise<string | null>>()
const lookShown = vi.fn<() => Promise<void>>()
vi.mock('../../bridge-scene/src/domains/avatarPreview', () => ({
  captureAvatarThumbnail: () => captureAvatarThumbnail(),
  lookShown: () => lookShown()
}))
let accountChanged: () => void = () => {}
vi.mock('../../bridge-scene/src/identity', () => ({
  identity: {
    onChange: (listener: () => void) => {
      accountChanged = listener
    }
  }
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
// As deployed: other explorers read a missing alpha as 0.
const opaqueGrey = { color: { r: 0.5, g: 0.5, b: 0.5, a: 1 } }
const deployed = (wearables: string[]) => ({ ...outfit(wearables), eyes: opaqueGrey, hair: opaqueGrey, skin: opaqueGrey })
// As the page sees outfits read from the catalyst: their colours made opaque.
const shown = (metadata: OutfitsMetadata) => ({
  ...metadata,
  outfits: metadata.outfits.map(({ slot, outfit }) => ({ slot, outfit: deployed(outfit.wearables) }))
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
  /** What the page is told of the outfits, without the busy marks around each request. */
  const settled = (): SceneToPage[] => sent.filter((m) => m.kind !== 'outfitBusy')
  return { request, lastOutfits, failures, settled, sent }
}

describe('the bridge outfits domain', () => {
  let stored: OutfitsMetadata

  beforeEach(() => {
    look = { bodyShape: FEMALE, wearables: ['urn:a'], forceRender: [] } as unknown as AvatarLook
    stored = { outfits: [{ slot: 0, outfit: outfit(['urn:old']) }], namesForExtraSlots: [] }
    getJson.mockResolvedValue({ metadata: stored })
    vi.stubGlobal('localStorage', localStorage)
    deployOutfits.mockResolvedValue(undefined)
    readOwnedNames.mockResolvedValue([])
    captureAvatarThumbnail.mockResolvedValue(PICTURE)
    lookShown.mockResolvedValue(undefined)
    editLook.mockImplementation((change) => {
      if (look != null) look = { ...look, ...change }
    })
  })

  afterEach(() => {
    vi.clearAllMocks()
    vi.unstubAllGlobals()
    storage.clear()
    pictureWrites.length = 0
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
          { slot: 0, outfit: deployed(['urn:old']) },
          { slot: 1, outfit: deployed(['urn:a']) }
        ],
        namesForExtraSlots: []
      })
      expect(storage.get('outfit-thumb:0xa:1')).toBe(PICTURE)
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
        expect(pictureWrites).toEqual([])
      })

      it('should tell the page the save failed after showing the outfits as they were', async () => {
        const { request, settled } = await bridge()
        await request({ kind: 'saveOutfit', slot: 1 })
        expect(settled().slice(-2)).toEqual([
          { kind: 'outfits', metadata: shown(stored) },
          { kind: 'outfitSaveFailed', slot: 1, action: 'save', message: 'Error: failed to deploy' }
        ])
      })
    })

    describe('and the player was deleting an outfit', () => {
      it('should keep the outfit and tell the page the delete failed', async () => {
        const { request, lastOutfits, failures } = await bridge()
        await request({ kind: 'deleteOutfit', slot: 0 })
        expect(lastOutfits()?.outfits.map((o) => o.slot)).toEqual([0])
        expect(pictureWrites).toEqual([])
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
        expect(pictureWrites).toEqual([])
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
      expect(deployOutfits).toHaveBeenCalledWith('0xA', { outfits: [{ slot: 2, outfit: deployed(['urn:a']) }], namesForExtraSlots: [] })
      expect(failures()).toEqual([])
    })
  })

  describe('when a name owner saves into an extra slot', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      readOwnedNames.mockResolvedValue(['somename'])
    })

    it('should deploy the outfits with the owned names', async () => {
      const { request, lastOutfits } = await bridge()
      await request({ kind: 'saveOutfit', slot: 6 })
      expect(readOwnedNames).toHaveBeenCalledWith('0xA')
      expect(deployOutfits).toHaveBeenCalledWith('0xA', {
        outfits: [
          { slot: 0, outfit: deployed(['urn:old']) },
          { slot: 6, outfit: deployed(['urn:a']) }
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
      const { request, settled } = await bridge()
      await request({ kind: 'saveOutfit', slot: 0 })
      expect(deployOutfits).not.toHaveBeenCalled()
      expect(settled().slice(-2)).toEqual([
        { kind: 'outfits', metadata: shown(stored) },
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
      expect(readOwnedNames).not.toHaveBeenCalled()
    })
  })

  describe('when the stored entity lists a name the player no longer owns', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      stored = { outfits: [{ slot: 6, outfit: outfit(['urn:old']) }], namesForExtraSlots: ['sold'] }
      getJson.mockResolvedValue({ metadata: stored })
      readOwnedNames.mockResolvedValue(['kept'])
    })

    it('should deploy the names the player owns now', async () => {
      const { request } = await bridge()
      await request({ kind: 'saveOutfit', slot: 1 })
      expect(deployOutfits.mock.calls[0]?.[1].namesForExtraSlots).toEqual(['kept'])
    })
  })

  describe('when a save is refused after an earlier one was deployed', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      deployOutfits.mockResolvedValueOnce(undefined).mockRejectedValueOnce(new Error('failed to deploy'))
    })

    it('should build the next save on the earlier one, not on a lagging catalyst read', async () => {
      const { request } = await bridge()
      await request({ kind: 'saveOutfit', slot: 1 })
      await request({ kind: 'saveOutfit', slot: 2 })
      await request({ kind: 'saveOutfit', slot: 3 })
      expect(getJson).toHaveBeenCalledTimes(1)
      expect(deployOutfits.mock.calls[2]?.[1].outfits.map((o) => o.slot)).toEqual([0, 1, 3])
    })
  })

  describe('when the names lookup fails for a player with an extra slot', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      stored = { outfits: [{ slot: 6, outfit: outfit(['urn:old']) }], namesForExtraSlots: ['mine'] }
      getJson.mockResolvedValue({ metadata: stored })
      readOwnedNames.mockResolvedValue(undefined)
    })

    it('should deploy the names read with the outfits', async () => {
      const { request, failures } = await bridge()
      await request({ kind: 'saveOutfit', slot: 1 })
      expect(deployOutfits.mock.calls[0]?.[1].namesForExtraSlots).toEqual(['mine'])
      expect(failures()).toEqual([])
    })
  })

  describe('when a player who owns no name deletes one of two extra-slot outfits', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      stored = {
        outfits: [
          { slot: 0, outfit: outfit(['urn:old']) },
          { slot: 6, outfit: outfit(['urn:six']) },
          { slot: 8, outfit: outfit(['urn:eight']) }
        ],
        namesForExtraSlots: ['sold']
      }
      getJson.mockResolvedValue({ metadata: stored })
      readOwnedNames.mockResolvedValue([])
      storage.set('outfit-thumb:0xa:6', PICTURE)
      storage.set('outfit-thumb:0xa:8', PICTURE)
    })

    it('should deploy without any extra slot and drop their pictures', async () => {
      const { request, failures, lastOutfits } = await bridge()
      await request({ kind: 'deleteOutfit', slot: 6 })
      expect(deployOutfits).toHaveBeenCalledWith('0xA', {
        outfits: [{ slot: 0, outfit: deployed(['urn:old']) }],
        namesForExtraSlots: []
      })
      expect(failures()).toEqual([])
      expect(storage.has('outfit-thumb:0xa:6')).toBe(false)
      expect(storage.has('outfit-thumb:0xa:8')).toBe(false)
      expect(lastOutfits()?.outfits.map((o) => o.slot)).toEqual([0])
    })

    it('should keep both outfits and pictures when the clear-all deploy is refused', async () => {
      deployOutfits.mockRejectedValueOnce(new Error('failed to deploy'))
      const { request, failures, lastOutfits } = await bridge()
      await request({ kind: 'deleteOutfit', slot: 6 })
      expect(failures()).toEqual([
        { kind: 'outfitSaveFailed', action: 'delete', slot: 6, message: 'Error: failed to deploy' }
      ])
      expect(lastOutfits()?.outfits.map((o) => o.slot)).toEqual([0, 6, 8])
      expect(storage.get('outfit-thumb:0xa:6')).toBe(PICTURE)
      expect(storage.get('outfit-thumb:0xa:8')).toBe(PICTURE)
    })
  })

  describe('when the names lookup fails as a player deletes one of two extra-slot outfits', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      stored = {
        outfits: [
          { slot: 6, outfit: outfit(['urn:six']) },
          { slot: 8, outfit: outfit(['urn:eight']) }
        ],
        namesForExtraSlots: ['mine']
      }
      getJson.mockResolvedValue({ metadata: stored })
      readOwnedNames.mockResolvedValue(undefined)
    })

    it('should delete only that outfit when Catalyst discovery throws during the names lookup', async () => {
      readOwnedNames.mockRejectedValueOnce(new Error('Catalyst discovery failed'))
      const { request, failures } = await bridge()
      await request({ kind: 'deleteOutfit', slot: 6 })
      expect(deployOutfits.mock.calls[0]?.[1].outfits.map((o) => o.slot)).toEqual([8])
      expect(failures()).toEqual([])
    })

    it('should delete only that outfit', async () => {
      const { request } = await bridge()
      await request({ kind: 'deleteOutfit', slot: 6 })
      expect(deployOutfits.mock.calls[0]?.[1].outfits.map((o) => o.slot)).toEqual([8])
    })
  })

  describe('when a player who owns a name deletes one of two extra-slot outfits', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      stored = {
        outfits: [
          { slot: 6, outfit: outfit(['urn:six']) },
          { slot: 8, outfit: outfit(['urn:eight']) }
        ],
        namesForExtraSlots: ['mine']
      }
      getJson.mockResolvedValue({ metadata: stored })
      readOwnedNames.mockResolvedValue(['mine'])
    })

    it('should delete only that outfit', async () => {
      const { request } = await bridge()
      await request({ kind: 'deleteOutfit', slot: 6 })
      expect(deployOutfits.mock.calls[0]?.[1].outfits.map((o) => o.slot)).toEqual([8])
    })
  })

  describe('when the same look is saved twice into a slot', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
    })

    it('should deploy once and report no failure', async () => {
      const { request, failures, lastOutfits } = await bridge()
      await request({ kind: 'saveOutfit', slot: 1 })
      await request({ kind: 'saveOutfit', slot: 1 })
      expect(deployOutfits).toHaveBeenCalledTimes(1)
      expect(failures()).toEqual([])
      expect(lastOutfits()?.outfits.map((o) => o.slot)).toEqual([0, 1])
    })
  })

  describe('when the look saved again matches a catalyst outfit in another key order', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      const color = { color: { a: 1, b: 0.5, g: 0.5, r: 0.5 } }
      const read = { wearables: ['urn:a'], forceRender: [], skin: color, hair: color, eyes: color, bodyShape: FEMALE }
      stored = { outfits: [{ slot: 1, outfit: read }], namesForExtraSlots: [] }
      getJson.mockResolvedValue({ metadata: stored })
    })

    it('should deploy nothing and report no failure', async () => {
      const { request, failures } = await bridge()
      await request({ kind: 'saveOutfit', slot: 1 })
      expect(deployOutfits).not.toHaveBeenCalled()
      expect(failures()).toEqual([])
    })
  })

  describe('when a stored outfit color carries its own alpha', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      const eyes = { color: { r: 0.1, g: 0.2, b: 0.3, a: 0.5 } }
      stored = { outfits: [{ slot: 0, outfit: { ...outfit(['urn:old']), eyes } }], namesForExtraSlots: [] }
      getJson.mockResolvedValue({ metadata: stored })
    })

    it('should deploy it unchanged', async () => {
      const { request } = await bridge()
      await request({ kind: 'saveOutfit', slot: 1 })
      expect(deployOutfits.mock.calls[0]?.[1].outfits[0]?.outfit.eyes).toEqual({ color: { r: 0.1, g: 0.2, b: 0.3, a: 0.5 } })
    })
  })

  describe('when the device storage cannot be read', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      localStorage.getItem.mockImplementation(() => {
        throw new Error('storage unavailable')
      })
    })

    afterEach(() => {
      localStorage.getItem.mockImplementation((key: string) => storage.get(key) ?? null)
    })

    it('should show the catalyst outfits without pictures', async () => {
      const { request, lastOutfits } = await bridge()
      await request({ kind: 'getOutfits' })
      expect(lastOutfits()).toEqual(shown(stored))
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
      expect(pictureWrites).toEqual([['outfit-thumb:0xa:0', PICTURE]])
    })

    it('should put its colours on the look without alpha', async () => {
      const { request } = await bridge()
      await request({ kind: 'equipOutfit', slot: 0 })
      expect(editLook).toHaveBeenCalledWith(
        expect.objectContaining({ eyes: grey.color, hair: grey.color, skin: grey.color })
      )
    })

    describe('and the Backpack closes while the wearables load', () => {
      beforeEach(() => {
        captureAvatarThumbnail.mockResolvedValue(null)
      })

      it('should leave the slot without a picture', async () => {
        const { request } = await bridge()
        await request({ kind: 'equipOutfit', slot: 0 })
        expect(pictureWrites).toEqual([])
      })
    })

    describe('and the slot is deleted while the wearables load', () => {
      it('should not picture the deleted slot', async () => {
        const { request } = await bridge()
        lookShown.mockImplementationOnce(async () => {
          await request({ kind: 'deleteOutfit', slot: 0 })
        })
        await request({ kind: 'equipOutfit', slot: 0 })
        expect(pictureWrites).toEqual([['outfit-thumb:0xa:0', null]])
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
        expect(pictureWrites).toEqual([])
      })
    })
  })

  describe('when a guest saves the current look', () => {
    beforeEach(() => {
      player = { userId: '0xB', isGuest: true }
    })

    it('should keep the outfit for the session without reading or deploying', async () => {
      const { request, lastOutfits, failures } = await bridge()
      await request({ kind: 'saveOutfit', slot: 1 })
      expect(getJson).not.toHaveBeenCalled()
      expect(deployOutfits).not.toHaveBeenCalled()
      expect(lastOutfits()?.outfits.map((o) => o.slot)).toEqual([1])
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
      expect(pictureWrites).toEqual([['outfit-thumb:0xa:0', null]])
      expect(lastOutfits()?.outfits).toEqual([])
    })
  })

  describe('when a signed-in player saves and the captured picture is not a PNG data URL', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      captureAvatarThumbnail.mockResolvedValue('https://tracker.example/pixel.gif')
    })

    it('should save the outfit without storing the picture', async () => {
      const { request, lastOutfits } = await bridge()
      await request({ kind: 'saveOutfit', slot: 1 })
      expect(deployOutfits).toHaveBeenCalledOnce()
      expect(storage.has('outfit-thumb:0xa:1')).toBe(false)
      expect(lastOutfits()?.outfits.find((o) => o.slot === 1)?.thumbnail).toBeUndefined()
    })
  })

  describe('when a signed-in player saves and the captured picture is over 1 MiB', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      captureAvatarThumbnail.mockResolvedValue(PICTURE + 'A'.repeat(1024 * 1024))
    })

    it('should save the outfit without storing the picture', async () => {
      const { request } = await bridge()
      await request({ kind: 'saveOutfit', slot: 1 })
      expect(deployOutfits).toHaveBeenCalledOnce()
      expect(storage.has('outfit-thumb:0xa:1')).toBe(false)
    })
  })

  describe('when this device holds pictures from an earlier session', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      storage.set('outfit-thumb:0xa:0', PICTURE)
    })

    it('should show them on the catalyst outfits', async () => {
      const { request, lastOutfits } = await bridge()
      await request({ kind: 'getOutfits' })
      expect(lastOutfits()?.outfits[0]?.thumbnail).toBe(PICTURE)
    })
  })

  describe('when this device holds a picture that is not a PNG data URL', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      storage.set('outfit-thumb:0xa:0', 'https://tracker.example/pixel.gif')
    })

    it('should show the outfit without a picture', async () => {
      const { request, lastOutfits } = await bridge()
      await request({ kind: 'getOutfits' })
      expect(lastOutfits()?.outfits[0]?.thumbnail).toBeUndefined()
    })
  })

  describe('when this device holds a picture over 1 MiB', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      storage.set('outfit-thumb:0xa:0', PICTURE + 'A'.repeat(1024 * 1024))
    })

    it('should show the outfit without a picture', async () => {
      const { request, lastOutfits } = await bridge()
      await request({ kind: 'getOutfits' })
      expect(lastOutfits()?.outfits[0]?.thumbnail).toBeUndefined()
    })
  })

  describe('when the player saves an outfit', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
    })

    it('should tell the page the slot is busy until the outfits are shown', async () => {
      const { request, sent } = await bridge()
      await request({ kind: 'saveOutfit', slot: 1 })
      expect(sent.map((m) => m.kind)).toEqual(['outfitBusy', 'outfits', 'outfitBusy'])
      expect([sent[0], sent.at(-1)]).toEqual([
        { kind: 'outfitBusy', slot: 1, action: 'save', busy: true },
        { kind: 'outfitBusy', slot: 1, action: 'save', busy: false }
      ])
    })

    describe('and the catalyst refuses the deploy', () => {
      beforeEach(() => {
        deployOutfits.mockRejectedValue(new Error('failed to deploy'))
      })

      it('should end the busy state after telling the page the save failed', async () => {
        const { request, sent } = await bridge()
        await request({ kind: 'saveOutfit', slot: 1 })
        expect(sent.slice(-2)).toEqual([
          { kind: 'outfitSaveFailed', slot: 1, action: 'save', message: 'Error: failed to deploy' },
          { kind: 'outfitBusy', slot: 1, action: 'save', busy: false }
        ])
      })
    })

    describe('and taking the picture fails', () => {
      beforeEach(() => {
        captureAvatarThumbnail.mockRejectedValue(new Error('no frame'))
      })

      it('should end the busy state', async () => {
        const { request, sent } = await bridge()
        await expect(request({ kind: 'saveOutfit', slot: 1 })).rejects.toThrow('no frame')
        expect(sent).toEqual([
          { kind: 'outfitBusy', slot: 1, action: 'save', busy: true },
          { kind: 'outfitBusy', slot: 1, action: 'save', busy: false }
        ])
      })
    })
  })

  describe('when an outfit is saved with no player', () => {
    it('should end the busy state it started', async () => {
      const { request, sent } = await bridge()
      await request({ kind: 'saveOutfit', slot: 1 })
      expect(sent).toEqual([
        { kind: 'outfitBusy', slot: 1, action: 'save', busy: true },
        { kind: 'outfitBusy', slot: 1, action: 'save', busy: false }
      ])
    })
  })

  describe('when the player deletes an outfit', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
    })

    it('should tell the page the slot is busy until the outfits are shown', async () => {
      const { request, sent } = await bridge()
      await request({ kind: 'deleteOutfit', slot: 0 })
      expect(sent).toEqual([
        { kind: 'outfitBusy', slot: 0, action: 'delete', busy: true },
        { kind: 'outfits', metadata: { outfits: [], namesForExtraSlots: [] } },
        { kind: 'outfitBusy', slot: 0, action: 'delete', busy: false }
      ])
    })
  })

  describe('when the player equips an outfit', () => {
    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      look = { bodyShape: FEMALE, wearables: ['urn:old'], forceRender: [] } as unknown as AvatarLook
    })

    it('should keep the slot busy until the avatar has loaded the outfit', async () => {
      const { request, sent } = await bridge()
      let toldWhileLoading: SceneToPage[] = []
      lookShown.mockImplementationOnce(async () => {
        toldWhileLoading = [...sent]
      })
      await request({ kind: 'equipOutfit', slot: 0 })
      expect([toldWhileLoading, sent.filter((m) => m.kind === 'outfitBusy')]).toEqual([
        [{ kind: 'outfitBusy', slot: 0, action: 'equip', busy: true }],
        [
          { kind: 'outfitBusy', slot: 0, action: 'equip', busy: true },
          { kind: 'outfitBusy', slot: 0, action: 'equip', busy: false }
        ]
      ])
    })

    it('should picture the outfit only after the busy state ends', async () => {
      const { request, sent } = await bridge()
      let toldBeforePicture: SceneToPage[] = []
      captureAvatarThumbnail.mockImplementationOnce(async () => {
        toldBeforePicture = [...sent]
        return PICTURE
      })
      await request({ kind: 'equipOutfit', slot: 0 })
      expect(toldBeforePicture.at(-1)).toEqual({ kind: 'outfitBusy', slot: 0, action: 'equip', busy: false })
    })

    describe('and the look changes while the picture is taken', () => {
      it('should not keep the picture', async () => {
        const { request } = await bridge()
        captureAvatarThumbnail.mockImplementationOnce(async () => {
          if (look != null) look = { ...look, hair: { r: 1, g: 0, b: 0 } }
          return PICTURE
        })
        await request({ kind: 'equipOutfit', slot: 0 })
        expect(pictureWrites).toEqual([])
      })
    })

    describe('and the slot holds no outfit', () => {
      it('should end the busy state it started', async () => {
        const { request, sent } = await bridge()
        await request({ kind: 'equipOutfit', slot: 3 })
        expect(editLook).not.toHaveBeenCalled()
        expect(sent).toEqual([
          { kind: 'outfitBusy', slot: 3, action: 'equip', busy: true },
          { kind: 'outfitBusy', slot: 3, action: 'equip', busy: false }
        ])
      })
    })
  })

  describe('when the player previews an outfit', () => {
    const before = { bodyShape: FEMALE, eyes: null, hair: null, skin: null, wearables: ['urn:mine'], forceRender: ['hat'] }

    beforeEach(() => {
      player = { userId: '0xA', isGuest: false }
      look = { ...before, emotes: ['urn:wave'] } as unknown as AvatarLook
      stored = { outfits: [{ slot: 0, outfit: outfit(['urn:old']) }, { slot: 1, outfit: outfit(['urn:other']) }], namesForExtraSlots: [] }
      getJson.mockResolvedValue({ metadata: stored })
    })

    it('should put the outfit on the look', async () => {
      const { request } = await bridge()
      await request({ kind: 'previewOutfit', slot: 0 })
      expect(look?.wearables).toEqual(['urn:old'])
    })

    it('should keep the slot busy as a preview until the avatar has loaded the outfit', async () => {
      const { request, sent } = await bridge()
      let toldWhileLoading: SceneToPage[] = []
      lookShown.mockImplementationOnce(async () => {
        toldWhileLoading = [...sent]
      })
      await request({ kind: 'previewOutfit', slot: 0 })
      expect([toldWhileLoading, sent.filter((m) => m.kind === 'outfitBusy')]).toEqual([
        [{ kind: 'outfitBusy', slot: 0, action: 'preview', busy: true }],
        [
          { kind: 'outfitBusy', slot: 0, action: 'preview', busy: true },
          { kind: 'outfitBusy', slot: 0, action: 'preview', busy: false }
        ]
      ])
    })

    it('should picture an outfit that has no picture', async () => {
      const { request } = await bridge()
      await request({ kind: 'previewOutfit', slot: 0 })
      expect(pictureWrites).toEqual([['outfit-thumb:0xa:0', PICTURE]])
    })

    describe('and the preview is restored', () => {
      it('should put back what the outfit changed on the look, and keep the rest', async () => {
        const { request } = await bridge()
        await request({ kind: 'previewOutfit', slot: 0 })
        await request({ kind: 'restoreOutfitPreview' })
        expect(look).toEqual({ ...before, emotes: ['urn:wave'] })
      })

      it('should change nothing when restored again', async () => {
        const { request } = await bridge()
        await request({ kind: 'previewOutfit', slot: 0 })
        await request({ kind: 'restoreOutfitPreview' })
        editLook.mockClear()
        await request({ kind: 'restoreOutfitPreview' })
        expect(editLook).not.toHaveBeenCalled()
      })
    })

    describe('and another outfit is previewed after it', () => {
      it('should restore the look from before the first preview', async () => {
        const { request } = await bridge()
        await request({ kind: 'previewOutfit', slot: 0 })
        await request({ kind: 'previewOutfit', slot: 1 })
        await request({ kind: 'restoreOutfitPreview' })
        expect(look?.wearables).toEqual(['urn:mine'])
      })
    })

    describe('and the outfit is then equipped', () => {
      it('should keep the outfit when restored', async () => {
        const { request } = await bridge()
        await request({ kind: 'previewOutfit', slot: 0 })
        await request({ kind: 'equipOutfit', slot: 0 })
        await request({ kind: 'restoreOutfitPreview' })
        expect(look?.wearables).toEqual(['urn:old'])
      })
    })

    describe('and the look is saved into an empty slot meanwhile', () => {
      it('should save the previewed look', async () => {
        const { request } = await bridge()
        await request({ kind: 'previewOutfit', slot: 0 })
        await request({ kind: 'saveOutfit', slot: 2 })
        expect(deployOutfits.mock.calls.at(-1)?.[1].outfits.find((o) => o.slot === 2)?.outfit.wearables).toEqual(['urn:old'])
      })
    })

    describe('and the preview is restored while its outfit is still being read', () => {
      it('should put nothing on the look', async () => {
        const { request } = await bridge()
        let answer: (value: unknown) => void = () => {}
        getJson.mockReturnValueOnce(new Promise((resolve) => (answer = resolve)))
        const preview = request({ kind: 'previewOutfit', slot: 0 })
        await request({ kind: 'restoreOutfitPreview' })
        answer({ metadata: stored })
        await preview
        expect(look?.wearables).toEqual(['urn:mine'])
      })
    })

    describe('and the look is reverted after a failed deploy', () => {
      it('should not put the reverted look back when restored', async () => {
        const { request } = await bridge()
        await request({ kind: 'previewOutfit', slot: 0 })
        await request({ kind: 'revertAvatar' })
        editLook.mockClear()
        await request({ kind: 'restoreOutfitPreview' })
        expect(editLook).not.toHaveBeenCalled()
      })
    })

    describe('and the look is reverted while its outfit is still being read', () => {
      it('should put nothing on the look', async () => {
        const { request } = await bridge()
        let answer: (value: unknown) => void = () => {}
        getJson.mockReturnValueOnce(new Promise((resolve) => (answer = resolve)))
        const preview = request({ kind: 'previewOutfit', slot: 0 })
        await request({ kind: 'revertAvatar' })
        answer({ metadata: stored })
        await preview
        expect(look?.wearables).toEqual(['urn:mine'])
      })
    })

    describe('and the preview is restored while the avatar loads it, to a look with the same wearables', () => {
      beforeEach(() => {
        look = { ...before, wearables: ['urn:old'], hair: { r: 1, g: 0, b: 0 }, emotes: ['urn:wave'] } as unknown as AvatarLook
      })

      it('should not picture the outfit', async () => {
        const { request } = await bridge()
        lookShown.mockImplementationOnce(async () => {
          await request({ kind: 'restoreOutfitPreview' })
        })
        await request({ kind: 'previewOutfit', slot: 0 })
        expect([captureAvatarThumbnail.mock.calls.length, pictureWrites]).toEqual([0, []])
      })
    })

    describe('and the look changes while the picture is taken', () => {
      it('should not keep the picture', async () => {
        const { request } = await bridge()
        captureAvatarThumbnail.mockImplementationOnce(async () => {
          if (look != null) look = { ...look, hair: { r: 1, g: 0, b: 0 } }
          return PICTURE
        })
        await request({ kind: 'previewOutfit', slot: 0 })
        expect(pictureWrites).toEqual([])
      })
    })

    describe('and the outfit was stored with a colour that is not opaque', () => {
      beforeEach(() => {
        const hair = { color: { r: 0.5, g: 0.5, b: 0.5, a: 0.5 } }
        stored = { outfits: [{ slot: 0, outfit: { ...outfit(['urn:old']), hair } }], namesForExtraSlots: [] }
        getJson.mockResolvedValue({ metadata: stored })
      })

      it('should picture the outfit', async () => {
        const { request } = await bridge()
        await request({ kind: 'previewOutfit', slot: 0 })
        expect(pictureWrites).toEqual([['outfit-thumb:0xa:0', PICTURE]])
      })
    })

    describe('and the account changes', () => {
      it('should not put the earlier account\'s look back', async () => {
        const { request } = await bridge()
        await request({ kind: 'previewOutfit', slot: 0 })
        accountChanged()
        editLook.mockClear()
        await request({ kind: 'restoreOutfitPreview' })
        expect(editLook).not.toHaveBeenCalled()
      })
    })

    describe('and the slot holds no outfit', () => {
      it('should end the busy state it started and change nothing', async () => {
        const { request, sent } = await bridge()
        await request({ kind: 'previewOutfit', slot: 3 })
        expect([editLook.mock.calls, sent]).toEqual([
          [],
          [
            { kind: 'outfitBusy', slot: 3, action: 'preview', busy: true },
            { kind: 'outfitBusy', slot: 3, action: 'preview', busy: false }
          ]
        ])
      })
    })
  })
})
