import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import type { Ctx } from '../../bridge-scene/src/bridge'
import type { CatalogPageParams } from '../../bridge-scene/src/domains/wearables'
import type { AvatarLook } from '../engine/avatarEquip'
import type { PageToScene, SceneToPage, Wearable } from '../engine/protocol'

const MALE = 'urn:decentraland:off-chain:base-avatars:BaseMale'
const FEMALE = 'urn:decentraland:off-chain:base-avatars:BaseFemale'

let player: { userId: string } | null = null
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => player }))

let look: AvatarLook | null = null
vi.mock('../../bridge-scene/src/domains/avatarDraft', () => ({
  currentLook: () => look,
  editLook: (change: Partial<AvatarLook>) => {
    if (look != null) look = { ...look, ...change }
  }
}))

type Page = { items: Wearable[]; total: number; failed: boolean }
const fetchPage = vi.fn<(address: string, p: CatalogPageParams) => Promise<Page>>()
vi.mock('../../bridge-scene/src/domains/wearables', () => ({
  fetchWearablesPage: (address: string, p: CatalogPageParams) => fetchPage(address, p),
  deployableUrn: (urn: string) => urn,
  sendEquipped: async () => {}
}))

const wearable = (urn: string, category: string): Wearable =>
  ({ urn, name: urn, rarity: 'common', category, equipped: false, bodyShapes: [MALE, FEMALE] }) as Wearable
const base = (name: string, category: string): Wearable => wearable(`urn:decentraland:off-chain:base-avatars:${name}`, category)
const owned = (category: string, count: number): Wearable[] =>
  Array.from({ length: count }, (_, i) => wearable(`urn:decentraland:matic:collections-v2:0x${category}:${i}`, category))

const TSHIRT = base('tshirt', 'upper_body')
const BASE = [TSHIRT, base('jeans', 'lower_body'), base('sneakers', 'feet'), base('hair', 'hair')]

function deferred<T>(): { promise: Promise<T>; resolve: (value: T) => void } {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((r) => {
    resolve = r
  })
  return { promise, resolve }
}

const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0))

/**
 * The catalyst over `inventory`: base requests get BASE, single-item requests the category's total
 * (`failTotals` fail), and owned pages slice the requested categories' wearables. `ownedPage`, when
 * given, answers the owned pages instead.
 */
function catalyst(
  inventory: Record<string, Wearable[]>,
  opts: { failTotals?: string[]; ownedPage?: (p: CatalogPageParams) => Promise<Page> } = {}
): void {
  fetchPage.mockImplementation(async (_address, p) => {
    if (p.collectionTypes?.includes('base-wearable') === true) return { items: BASE, total: BASE.length, failed: false }
    if (p.pageSize === 1) {
      if (opts.failTotals?.includes(p.category ?? '') === true) return { items: [], total: 0, failed: true }
      return { items: [], total: inventory[p.category ?? '']?.length ?? 0, failed: false }
    }
    if (opts.ownedPage != null) return await opts.ownedPage(p)
    const items = (p.categories ?? []).flatMap((c) => inventory[c] ?? [])
    return { items: items.slice(p.page * p.pageSize, (p.page + 1) * p.pageSize), total: items.length, failed: false }
  })
}

const calls = (match: (p: CatalogPageParams) => boolean): CatalogPageParams[] =>
  fetchPage.mock.calls.map(([, p]) => p).filter(match)
const isTotal = (p: CatalogPageParams): boolean => p.pageSize === 1
const isBase = (p: CatalogPageParams): boolean => p.collectionTypes?.includes('base-wearable') === true

/** A fresh randomizer module; `random` is fixed before it builds its OutfitRandomizer. */
async function setup(random: number) {
  vi.spyOn(Math, 'random').mockReturnValue(random)
  vi.resetModules()
  const { registerRandomizer } = await import('../../bridge-scene/src/domains/randomizer')
  const { identity } = await import('../../bridge-scene/src/identity')
  const sent: SceneToPage[] = []
  const handlers = new Map<string, Array<(m: PageToScene) => unknown>>()
  const ctx: Ctx = {
    send: (m) => sent.push(m),
    on: (kind, h) => handlers.set(kind, [...(handlers.get(kind) ?? []), h as (m: PageToScene) => unknown]),
    push: () => {}
  }
  registerRandomizer(ctx)
  identity.observe('0xa')
  const req = async (kind: 'randomizeOutfit' | 'undoOutfit' | 'redoOutfit' | 'resetOutfitHistory'): Promise<void> => {
    await Promise.all((handlers.get(kind) ?? []).map((h) => h({ kind } as PageToScene)))
  }
  const history = () => sent.filter((m) => m.kind === 'outfitHistory').at(-1)
  return { req, history, identity }
}

const START: AvatarLook = { bodyShape: MALE, eyes: null, hair: null, skin: null, wearables: [TSHIRT.urn], emotes: [], forceRender: [] }

beforeEach(() => {
  player = { userId: '0xa' }
  look = { ...START }
  fetchPage.mockReset()
})

afterEach(() => {
  vi.restoreAllMocks()
})

describe('bridge randomizer', () => {
  it('rolls an owned look, and undo and redo walk back and forth over it', async () => {
    const shirts = owned('upper_body', 2)
    catalyst({ upper_body: shirts })
    const { req, history } = await setup(0)

    await req('randomizeOutfit')
    const rolled = look?.wearables
    expect(rolled).toContain(shirts[0].urn)
    expect(history()).toEqual({ kind: 'outfitHistory', canUndo: true, canRedo: false, rolling: false })

    await req('undoOutfit')
    expect(look?.wearables).toEqual(START.wearables)
    expect(history()).toEqual({ kind: 'outfitHistory', canUndo: false, canRedo: true, rolling: false })

    await req('redoOutfit')
    expect(look?.wearables).toEqual(rolled)
    expect(history()).toEqual({ kind: 'outfitHistory', canUndo: true, canRedo: false, rolling: false })
  })

  it('fetches the page holding the random owned index, one page for the optional categories together', async () => {
    const shirts = owned('upper_body', 65)
    catalyst({ upper_body: shirts, hat: owned('hat', 10), mask: owned('mask', 20) })
    // 0.62 includes the optional categories, skips skin, prefers owned wearables and picks index 40 of 65
    const { req } = await setup(0.62)

    await req('randomizeOutfit')

    const pages = calls((p) => !isTotal(p) && !isBase(p))
    expect(pages).toContainEqual(expect.objectContaining({ page: 1, pageSize: 30, categories: ['upper_body'] }))
    // floor(0.62 * 30) = 18 of the 10 hats and 20 masks: page 0
    expect(pages).toContainEqual(expect.objectContaining({ page: 0, pageSize: 30, categories: ['hat', 'mask'] }))
    expect(look?.wearables).toContain(shirts[40].urn)
  })

  it('reads the totals without entities, and never asks for facial hair on the female body shape', async () => {
    look = { ...START, bodyShape: FEMALE }
    catalyst({})
    const { req } = await setup(0)

    await req('randomizeOutfit')

    const totals = calls(isTotal)
    expect(totals.length).toBeGreaterThan(0)
    expect(totals.every((p) => p.includeEntities === false)).toBe(true)
    expect(totals.map((p) => p.category)).not.toContain('facial_hair')
  })

  it('keeps no owned page that lands after the Backpack closed', async () => {
    const stale = deferred<Page>()
    const fresh = deferred<Page>()
    const pending = [stale, fresh]
    catalyst({ upper_body: owned('upper_body', 1) }, { ownedPage: async () => await pending.shift()!.promise })
    const { req } = await setup(0)

    const first = req('randomizeOutfit')
    await flush()
    await req('resetOutfitHistory')
    const second = req('randomizeOutfit')
    await flush()
    // The closed session's page lands while the new roll waits for its own, which has no usable shirt
    stale.resolve({ items: owned('upper_body', 1), total: 1, failed: false })
    await first
    fresh.resolve({ items: [wearable('urn:decentraland:matic:collections-v2:0xother:0', 'hat')], total: 1, failed: false })
    await second

    expect(look?.wearables).toContain(TSHIRT.urn)
    expect(look?.wearables).not.toContain(owned('upper_body', 1)[0].urn)
  })

  it('keeps the look when the body shape changed while the roll loaded', async () => {
    const page = deferred<Page>()
    const shirts = owned('upper_body', 1)
    catalyst({ upper_body: shirts }, { ownedPage: async () => await page.promise })
    const { req, history } = await setup(0)

    const roll = req('randomizeOutfit')
    await flush()
    look = { ...START, bodyShape: FEMALE }
    page.resolve({ items: shirts, total: 1, failed: false })
    await roll

    expect(look.wearables).toEqual(START.wearables)
    expect(history()).toEqual({ kind: 'outfitHistory', canUndo: false, canRedo: false, rolling: false })
  })

  it('asks again on the next roll for a total whose request failed, and only for it', async () => {
    catalyst({ upper_body: owned('upper_body', 3) }, { failTotals: ['upper_body'] })
    const { req } = await setup(0)

    await req('randomizeOutfit')
    const firstTotals = calls(isTotal).length
    catalyst({ upper_body: owned('upper_body', 3) })
    fetchPage.mockClear()
    await req('randomizeOutfit')

    expect(firstTotals).toBeGreaterThan(1)
    expect(calls(isTotal).map((p) => p.category)).toEqual(['upper_body'])
  })

  it('loads the base set again for a new body shape', async () => {
    catalyst({})
    const { req } = await setup(0)

    await req('randomizeOutfit')
    await req('randomizeOutfit')
    expect(calls(isBase)).toHaveLength(1)

    look = { ...START, bodyShape: FEMALE }
    await req('randomizeOutfit')
    expect(calls(isBase)).toHaveLength(2)
  })

  it('unlocks the bar and drops the roll when the account changes mid-roll', async () => {
    const page = deferred<Page>()
    const shirts = owned('upper_body', 1)
    catalyst({ upper_body: shirts }, { ownedPage: async () => await page.promise })
    const { req, history, identity } = await setup(0)

    const roll = req('randomizeOutfit')
    await flush()
    expect(history()).toMatchObject({ rolling: true })
    identity.observe('0xb')
    expect(history()).toEqual({ kind: 'outfitHistory', canUndo: false, canRedo: false, rolling: false })

    page.resolve({ items: shirts, total: 1, failed: false })
    await roll
    expect(look?.wearables).toEqual(START.wearables)
  })
})
