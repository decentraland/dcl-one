// The full-screen map's deep-zoom satellite imagery: which detail tiles the view needs, their
// loading through the engine, and the decoded images kept for drawing. A plain store, read with
// useSyncExternalStore, so a tile arriving re-renders only the layer that draws them.
//
// The view says where it looks and how sharp it must be (`setView`). The store plans the tiles
// (atlas.ts), asks for the missing ones nearest the view's centre first with a few requests in
// flight, and publishes what can be drawn: each wanted tile once loaded, otherwise the nearest
// coarser tile already loaded, so a zoom never shows a gap.

import {
  DETAIL_BASE_LEVEL,
  parseSatelliteManifest,
  planDetail,
  satelliteManifestUrl,
  satelliteTilePath,
  tileKey,
  tileParent,
  tileRect,
  type ParcelRect,
  type SatelliteManifest,
  type TileId
} from './atlas'

export interface DetailView {
  rect: ParcelRect
  /** Device px per parcel on screen. */
  pxPerParcel: number
}

/** A decoded tile ready to draw. */
export interface DetailTile {
  key: string
  level: number
  url: string
  /** Parcel-space rect it covers. */
  rect: ParcelRect
}

export interface SatelliteDetailOptions {
  /** One tile as base64 JPEG bytes, or null when it cannot be had. */
  fetchTile: (path: string) => Promise<string | null>
  /** The manifest, or null when the source has no detail tiles; rejects on a network failure. */
  loadManifest?: () => Promise<SatelliteManifest | null>
  /** Tiles requested at once. */
  maxInFlight?: number
  /** Decoded tiles kept. */
  capacity?: number
  /** Wait before a failed tile or manifest is asked for again, ms. */
  retryMs?: number
  now?: () => number
  /** Makes an object URL from a tile's bytes and resolves once it can be drawn. */
  createImage?: (bytes: Blob) => Promise<string>
  revokeUrl?: (url: string) => void
}

const MAX_IN_FLIGHT = 4
const CAPACITY = 128
const RETRY_MS = 30_000
/** Failures in a row, with no tile ever loaded, after which the store stops asking. */
const GIVE_UP_AFTER = 8

export function base64ToBlob(data: string, type = 'image/jpeg'): Blob {
  const binary = atob(data)
  const bytes = new Uint8Array(binary.length)
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i)
  return new Blob([bytes], { type })
}

async function createDecodedUrl(bytes: Blob): Promise<string> {
  const url = URL.createObjectURL(bytes)
  try {
    const img = new Image()
    img.src = url
    // decoded before it is published, so a tile that replaces a coarser one never flashes empty
    if (typeof img.decode === 'function') await img.decode()
    else
      await new Promise<void>((resolve, reject) => {
        img.onload = () => resolve()
        img.onerror = () => reject(new Error('tile did not decode'))
      })
    return url
  } catch (e) {
    URL.revokeObjectURL(url)
    throw e
  }
}

let manifestRequest: Promise<SatelliteManifest | null> | null = null

/** The manifest, fetched once per session. A missing manifest (404) means no detail tiles; a
 *  network failure rejects and is asked for again next time. */
export function loadSatelliteManifest(): Promise<SatelliteManifest | null> {
  manifestRequest ??= fetch(satelliteManifestUrl())
    .then(async (res) => {
      if (res.status === 404) return null
      if (!res.ok) throw new Error(`manifest ${res.status}`)
      return parseSatelliteManifest(await res.json())
    })
    .catch((e: unknown) => {
      manifestRequest = null
      throw e
    })
  return manifestRequest
}

/** Forgets the session's manifest; for tests. */
export function resetSatelliteManifest(): void {
  manifestRequest = null
}

interface Loaded {
  tile: TileId
  url: string
}

export class SatelliteDetail {
  private readonly fetchTile: SatelliteDetailOptions['fetchTile']
  private readonly loadManifest: () => Promise<SatelliteManifest | null>
  private readonly maxInFlight: number
  private readonly capacity: number
  private readonly retryMs: number
  private readonly now: () => number
  private readonly createImage: (bytes: Blob) => Promise<string>
  private readonly revokeUrl: (url: string) => void

  private manifest: SatelliteManifest | null | undefined
  private manifestLoading = false
  private manifestFailedAt = -Infinity
  /** Decoded tiles, least recently drawn first. */
  private readonly cache = new Map<string, Loaded>()
  private readonly inFlight = new Set<string>()
  private readonly failedAt = new Map<string, number>()
  private failures = 0
  private everLoaded = false
  private view: DetailView | null = null
  private wanted: TileId[] = []
  private pending: TileId[] = []
  private snapshot: readonly DetailTile[] = []
  private snapshotKey = ''
  private disposed = false
  /** The one pending retry after a failure, so a still view recovers without moving. */
  private retryTimer: ReturnType<typeof setTimeout> | null = null
  private readonly listeners = new Set<() => void>()

  constructor(options: SatelliteDetailOptions) {
    this.fetchTile = options.fetchTile
    this.loadManifest = options.loadManifest ?? loadSatelliteManifest
    this.maxInFlight = options.maxInFlight ?? MAX_IN_FLIGHT
    this.capacity = options.capacity ?? CAPACITY
    this.retryMs = options.retryMs ?? RETRY_MS
    this.now = options.now ?? Date.now
    this.createImage = options.createImage ?? createDecodedUrl
    this.revokeUrl = options.revokeUrl ?? ((url) => URL.revokeObjectURL(url))
  }

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  getSnapshot = (): readonly DetailTile[] => this.snapshot

  /** Where the map looks now, or null when it needs no detail (another realm, another style). */
  setView(view: DetailView | null): void {
    if (this.disposed) return
    this.view = view
    this.refresh()
  }

  /** Revokes every tile and stops; replies still in flight are discarded. */
  dispose(): void {
    this.disposed = true
    this.clearRetry()
    for (const { url } of this.cache.values()) this.revokeUrl(url)
    this.cache.clear()
    this.listeners.clear()
    this.pending = []
  }

  private refresh(): void {
    const view = this.view
    this.wanted = view == null || this.gaveUp() ? [] : this.plan(view)
    if (this.wanted.length > 0) this.ensureManifest()
    const manifest = this.manifest
    const usable = manifest == null ? [] : this.wanted.filter((t) => satelliteTilePath(manifest, t) != null)
    this.pending = usable.filter((t) => !this.cache.has(tileKey(t)) && !this.inFlight.has(tileKey(t)) && !this.recentlyFailed(tileKey(t)))
    this.publish(usable)
    this.pump()
  }

  /** The wanted tiles, the ones nearest the view's centre first. */
  private plan(view: DetailView): TileId[] {
    const { tiles } = planDetail(view.rect, view.pxPerParcel)
    const cx = (view.rect.xMin + view.rect.xMax) / 2
    const cy = (view.rect.yMin + view.rect.yMax) / 2
    const distance = (t: TileId): number => {
      const r = tileRect(t)
      return Math.hypot((r.xMin + r.xMax) / 2 - cx, (r.yMin + r.yMax) / 2 - cy)
    }
    return tiles.map((t) => ({ t, d: distance(t) })).sort((a, b) => a.d - b.d).map(({ t }) => t)
  }

  /** Failures in a row, manifest and tiles alike, with no tile ever loaded: the source is out of reach. */
  private gaveUp(): boolean {
    return this.failures >= GIVE_UP_AFTER && !this.everLoaded
  }

  private clearRetry(): void {
    if (this.retryTimer !== null) clearTimeout(this.retryTimer)
    this.retryTimer = null
  }

  /** After a failure, look again once the wait is over: a failed tile or manifest is asked for
   *  again only by a refresh, and a view that stays still makes none. */
  private failed(): void {
    this.failures++
    this.clearRetry()
    if (this.gaveUp()) return
    this.retryTimer = setTimeout(() => {
      this.retryTimer = null
      if (!this.disposed) this.refresh()
    }, this.retryMs)
  }

  private recentlyFailed(key: string): boolean {
    const at = this.failedAt.get(key)
    return at !== undefined && this.now() - at < this.retryMs
  }

  private ensureManifest(): void {
    if (this.manifest !== undefined || this.manifestLoading || this.now() - this.manifestFailedAt < this.retryMs) return
    this.manifestLoading = true
    void this.loadManifest().then(
      (manifest) => {
        this.manifestLoading = false
        if (this.disposed) return
        this.manifest = manifest
        this.refresh()
      },
      () => {
        this.manifestLoading = false
        if (this.disposed) return
        this.manifestFailedAt = this.now()
        this.failed()
      }
    )
  }

  private pump(): void {
    while (!this.disposed && this.inFlight.size < this.maxInFlight && this.pending.length > 0) {
      const tile = this.pending.shift() as TileId
      this.load(tile)
    }
  }

  private load(tile: TileId): void {
    const manifest = this.manifest
    const path = manifest == null ? null : satelliteTilePath(manifest, tile)
    if (path == null) return
    const key = tileKey(tile)
    this.inFlight.add(key)
    void this.fetchTile(path)
      .then(async (data) => {
        if (data == null) throw new Error('no tile')
        const url = await this.createImage(base64ToBlob(data))
        if (this.disposed) {
          this.revokeUrl(url)
          return
        }
        this.inFlight.delete(key)
        this.failures = 0
        this.everLoaded = true
        this.failedAt.delete(key)
        this.cache.set(key, { tile, url })
        this.evict()
      })
      .catch(() => {
        this.inFlight.delete(key)
        if (this.disposed) return
        this.failedAt.set(key, this.now())
        this.failed()
      })
      .finally(() => {
        if (!this.disposed) this.refresh()
      })
  }

  /** Drops the least recently drawn tiles beyond capacity, never one the view wants. */
  private evict(): void {
    if (this.cache.size <= this.capacity) return
    const keep = new Set(this.wanted.map(tileKey))
    for (const [key, { url }] of this.cache) {
      if (this.cache.size <= this.capacity) break
      if (keep.has(key)) continue
      this.revokeUrl(url)
      this.cache.delete(key)
    }
  }

  /** Each wanted tile once loaded, else its nearest loaded ancestor above the base level, coarse
   *  before fine so the finer tiles draw on top. */
  private publish(usable: readonly TileId[]): void {
    const chosen = new Map<string, Loaded>()
    for (const tile of usable) {
      let t: TileId = tile
      for (;;) {
        const hit = this.cache.get(tileKey(t))
        if (hit != null) {
          chosen.set(tileKey(t), hit)
          break
        }
        if (t.level <= DETAIL_BASE_LEVEL + 1) break
        t = tileParent(t)
      }
    }
    const list = [...chosen.entries()].sort(([, a], [, b]) => a.tile.level - b.tile.level)
    const key = list.map(([k]) => k).join('|')
    // drawn tiles move to the back of the eviction order
    for (const [k, loaded] of list) {
      this.cache.delete(k)
      this.cache.set(k, loaded)
    }
    if (key === this.snapshotKey) return
    this.snapshotKey = key
    this.snapshot = list.map(([k, { tile, url }]) => ({ key: k, level: tile.level, url, rect: tileRect(tile) }))
    for (const listener of [...this.listeners]) listener()
  }
}
