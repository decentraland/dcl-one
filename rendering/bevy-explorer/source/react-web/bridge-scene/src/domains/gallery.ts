// Gallery (camera reel): the local player's in-world photos + per-photo metadata.
//   from: camera-reel-service via BevyApi.kernelFetch (signed) —
//     GET    /api/users/:address/images?limit&offset&compact=true   one page of the grid + storage
//     GET    /api/images/:id/metadata                               one photo's place + people
//   the people's official marks from the engine (BevyApi.getOfficialWallets), their wearables from
//   the catalyst (resolveWearables)
//     PATCH  /api/images/:id/visibility {is_public}                 make a photo public/private
//     DELETE /api/images/:id                                        remove a photo (own only)
import { getPlayer } from '@dcl/sdk/players'
import { BevyApi } from '../bevy-api'
import { serviceBase, signed } from '../http'
import { resolveWearables } from './wearables'
import type { GalleryPhoto, GalleryPhotoMeta } from '@explorer-core/engine/protocol'
import type { Ctx } from '../bridge'

async function base(): Promise<string> {
  return `${await serviceBase('cameraReel')}/api`
}

type CompactImage = { id: string; url?: string; thumbnailUrl?: string; isPublic?: boolean; dateTime?: string }
type CompactResponse = { images?: CompactImage[]; currentImages?: number; maxImages?: number }

type ImageMetadata = {
  userName?: string
  userAddress?: string
  dateTime?: string
  realm?: string
  scene?: { name?: string; location?: { x?: string | number; y?: string | number } }
  visiblePeople?: Array<{ userName?: string; userAddress?: string; isGuest?: boolean; wearables?: unknown }>
}

/** Lowercased; empty on an engine without the read. */
export async function officialWallets(): Promise<Set<string>> {
  const list = await BevyApi.getOfficialWallets?.().catch(() => undefined)
  return new Set((list ?? []).map((a) => a.toLowerCase()))
}
type MetadataResponse = { metadata?: ImageMetadata }

const num = (v: string | number | undefined): number | undefined => {
  if (v == null) return undefined
  const n = typeof v === 'number' ? v : Number(v)
  return Number.isFinite(n) ? n : undefined
}

export function registerGallery(ctx: Ctx): void {
  ctx.on('getGallery', async (msg) => {
    const player = getPlayer()
    if (player == null) {
      ctx.send({ kind: 'gallery', requestId: msg.requestId, ok: false, photos: [], received: 0, current: 0, max: 0 })
      return
    }
    // Every failure, the service base included, still answers this request: the page waits on its
    // requestId, and a bare throw would reach it only as a requestFailed it can't match.
    const r = await base()
      .then((b) => signed<CompactResponse>(`${b}/users/${player.userId}/images?limit=${msg.limit}&offset=${msg.offset}&compact=true`))
      .catch((e: unknown) => {
        console.error('[gallery] page fetch failed', e)
        return null
      })
    const images = r?.images ?? []
    const photos: GalleryPhoto[] = images
      .filter((i) => typeof i.url === 'string')
      .map((i) => ({
        id: i.id,
        url: i.url as string,
        thumbnailUrl: i.thumbnailUrl,
        dateTime: i.dateTime ?? '',
        isPublic: i.isPublic
      }))
    ctx.send({
      kind: 'gallery',
      requestId: msg.requestId,
      ok: r !== null,
      photos,
      received: images.length,
      current: r?.currentImages ?? photos.length,
      max: r?.maxImages ?? 0
    })
  })

  ctx.on('getGalleryPhoto', async (msg) => {
    const [r, official] = await Promise.all([
      base()
        .then((b) => signed<MetadataResponse>(`${b}/images/${encodeURIComponent(msg.id)}/metadata`))
        .catch(() => undefined),
      officialWallets()
    ])
    const m = r?.metadata
    if (m == null) {
      ctx.send({ kind: 'galleryPhoto', id: msg.id, meta: null })
      return
    }
    const meta: GalleryPhotoMeta = {
      dateTime: m.dateTime,
      userName: m.userName,
      userAddress: m.userAddress,
      sceneName: m.scene?.name,
      x: num(m.scene?.location?.x),
      y: num(m.scene?.location?.y),
      realm: m.realm,
      people: (m.visiblePeople ?? [])
        .filter((p) => typeof p.userAddress === 'string')
        .map((p) => ({
          address: p.userAddress as string,
          name: p.userName ?? '',
          isGuest: p.isGuest,
          wearables: Array.isArray(p.wearables) ? p.wearables.filter((w): w is string => typeof w === 'string') : [],
          official: official.has((p.userAddress as string).toLowerCase())
        }))
    }
    ctx.send({ kind: 'galleryPhoto', id: msg.id, meta })
  })

  // Anyone's urns (a photo's people), so resolveWearables, never the own-set indexing.
  ctx.on('getGalleryWearables', async (msg) => {
    const wearables = await resolveWearables(msg.urns, { shopUrls: true, dropFailed: true }).catch((e: unknown) => {
      console.error('[gallery] wearables resolve failed', e)
      return null
    })
    ctx.send({ kind: 'galleryWearables', requestId: msg.requestId, ok: wearables != null, wearables: wearables ?? [] })
  })

  ctx.on('deleteGalleryPhoto', async (msg) => {
    try {
      const r = await signed<{ currentImages?: number; maxImages?: number }>(`${await base()}/images/${encodeURIComponent(msg.id)}`, 'DELETE')
      ctx.send({ kind: 'galleryWrite', requestId: msg.requestId, ok: true, current: r?.currentImages, max: r?.maxImages })
    } catch (e) {
      console.error('[gallery] delete failed', e)
      ctx.send({ kind: 'galleryWrite', requestId: msg.requestId, ok: false })
    }
  })

  ctx.on('setGalleryPhotoPublic', async (msg) => {
    try {
      // The service reads `is_public` (snake case), not the camelCase of its responses.
      await signed(`${await base()}/images/${encodeURIComponent(msg.id)}/visibility`, 'PATCH', { is_public: msg.isPublic })
      ctx.send({ kind: 'galleryWrite', requestId: msg.requestId, ok: true })
    } catch (e) {
      console.error('[gallery] visibility update failed', e)
      ctx.send({ kind: 'galleryWrite', requestId: msg.requestId, ok: false })
    }
  })
}
