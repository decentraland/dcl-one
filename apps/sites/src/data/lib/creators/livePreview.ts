// Connector between the Blender add-on's local bridge and the avatar preview. The add-on serves
// `GET /state` (metadata with a `version` that bumps per export, long-polled with `?since=`) and
// `GET /model.glb`; whenever the version moves the fresh GLB is wrapped as a blob definition and the
// preview hot-swaps it. Nothing here touches builder-server.
import {
  BodyPartCategory,
  BodyShape,
  EmoteCategory,
  Locale,
  WearableCategory,
  type EmoteWithBlobs,
  type WearableWithBlobs
} from '@dcl/schemas'

export const DEFAULT_BRIDGE_URL = 'http://localhost:8080'
export const MODEL_KEY = 'model.glb'
/** Base of the definition id; the version is appended so every push looks like a new item. */
export const LIVE_PREVIEW_ITEM_ID = 'live-preview'

const LOCAL_HOSTNAMES = ['localhost', '127.0.0.1', '[::1]']
// Chromium is splitting the original permission name into `local-network` and `loopback-network`
// and keeps the first as an alias.
const LOCAL_NETWORK_PERMISSION_NAMES = ['local-network-access', 'loopback-network']
const BOTH_BODY_SHAPES = [BodyShape.MALE, BodyShape.FEMALE]

export type BridgeState = {
  /** Bumps every time the creator exports from Blender. */
  version: number | string
  type?: 'wearable' | 'emote'
  name?: string
  /** Wearable or emote category, e.g. "hat", "upper_body", "dance". Empty for emotes. */
  category?: string
  /** Defaults to both when omitted. */
  bodyShapes?: BodyShape[]
}

export type LivePreviewErrorCode = 'bridge_response' | 'invalid_state' | 'model_fetch'

export class LivePreviewError extends Error {
  constructor(
    public readonly code: LivePreviewErrorCode,
    public readonly status?: number
  ) {
    super(code)
    this.name = 'LivePreviewError'
  }
}

export function isLocalHostname(hostname: string): boolean {
  return LOCAL_HOSTNAMES.includes(hostname)
}

/**
 * The `?bridge=` param: a bare port (`8081`) or a full URL. Only local origins are trusted, so a
 * crafted link cannot point the page at an external server.
 */
export function resolveBridgeUrl(param: string | null): string {
  if (!param) return DEFAULT_BRIDGE_URL
  if (/^\d+$/.test(param)) {
    const port = Number(param)
    return port >= 1 && port <= 65535 ? `http://localhost:${param}` : DEFAULT_BRIDGE_URL
  }
  try {
    const url = new URL(param)
    if ((url.protocol === 'http:' || url.protocol === 'https:') && isLocalHostname(url.hostname)) {
      return param.replace(/\/$/, '')
    }
  } catch {
    // Not a URL: fall through to the default.
  }
  return DEFAULT_BRIDGE_URL
}

/** With `since`, a long-polling bridge holds the request until its version differs. */
export function buildStateUrl(bridgeUrl: string, since?: BridgeState['version'] | null): string {
  const url = `${bridgeUrl.replace(/\/$/, '')}/state`
  return since === null || since === undefined ? url : `${url}?since=${encodeURIComponent(String(since))}`
}

export async function fetchBridgeState(
  bridgeUrl: string,
  { since, signal }: { since?: BridgeState['version'] | null; signal?: AbortSignal } = {}
): Promise<BridgeState> {
  const response = await fetch(buildStateUrl(bridgeUrl, since), { cache: 'no-store', signal })
  if (!response.ok) {
    await response.body?.cancel()
    throw new LivePreviewError('bridge_response', response.status)
  }
  const state = (await response.json()) as Partial<BridgeState> | null
  if (!state || (typeof state.version !== 'string' && typeof state.version !== 'number')) {
    throw new LivePreviewError('invalid_state')
  }
  return state as BridgeState
}

export async function fetchModelBlob(bridgeUrl: string, signal?: AbortSignal): Promise<Blob> {
  const response = await fetch(`${bridgeUrl.replace(/\/$/, '')}/${MODEL_KEY}`, { cache: 'no-store', signal })
  if (!response.ok) {
    await response.body?.cancel()
    throw new LivePreviewError('model_fetch', response.status)
  }
  return response.blob()
}

/**
 * Chromium gates requests from a public page to localhost behind a "Local Network Access" prompt, and a
 * blocked request fails like any network error. Null when no prompt can apply: browsers without the
 * permission, or a page itself served from localhost.
 */
export async function queryLocalNetworkPermission(
  pageHostname: string = window.location.hostname
): Promise<PermissionStatus | null> {
  if (isLocalHostname(pageHostname) || typeof navigator.permissions?.query !== 'function') return null
  for (const name of LOCAL_NETWORK_PERMISSION_NAMES) {
    try {
      return await navigator.permissions.query({ name: name as PermissionName })
    } catch {
      // Unknown name in this browser: try the next alias.
    }
  }
  return null
}

/** Same item, ignoring the version counter. */
export function isSameModelMetadata(a: BridgeState | null, b: BridgeState): boolean {
  return (
    !!a &&
    a.type === b.type &&
    a.name === b.name &&
    a.category === b.category &&
    JSON.stringify([...(a.bodyShapes ?? [])].sort()) === JSON.stringify([...(b.bodyShapes ?? [])].sort())
  )
}

export function isEmoteState(state: BridgeState): boolean {
  return (
    state.type === 'emote' || (!!state.category && (EmoteCategory.schema.enum as string[]).includes(state.category))
  )
}

export type DefinitionOverrides = {
  category?: string | null
  hides?: string[]
  loop?: boolean
}

/**
 * A minimal definition-with-blobs the preview renders straight from the GLB. Every push and a loop
 * toggle change the id: the iframe deep-equals successive updates (two Blobs compare equal) and the
 * Unity renderer caches models and emote clips by id, so a constant id would keep the first GLB forever.
 */
export function buildDefinition(
  state: BridgeState,
  glb: Blob,
  overrides: DefinitionOverrides = {}
): WearableWithBlobs | EmoteWithBlobs {
  const bodyShapes = state.bodyShapes && state.bodyShapes.length > 0 ? state.bodyShapes : BOTH_BODY_SHAPES
  const name = state.name || 'Live preview'
  const base = {
    id: `${LIVE_PREVIEW_ITEM_ID}-${String(state.version)}`,
    name,
    description: '',
    thumbnail: '',
    image: '',
    i18n: [{ code: Locale.EN, text: name }]
  }
  const contents = [{ key: MODEL_KEY, blob: glb }]

  if (isEmoteState(state)) {
    const loop = overrides.loop ?? true
    return {
      ...base,
      id: `${base.id}-${loop ? 'loop' : 'once'}`,
      emoteDataADR74: {
        category: (state.category as EmoteCategory) || EmoteCategory.DANCE,
        representations: [{ bodyShapes, mainFile: MODEL_KEY, contents }],
        tags: [],
        loop
      }
    }
  }

  const category = (overrides.category || state.category || WearableCategory.HAT) as WearableCategory
  const hides = (overrides.hides ?? []) as WearableCategory[]
  // Renderers hide the base hands under any upper body unless told otherwise; mirror what the upload does.
  const removesDefaultHiding =
    category === WearableCategory.UPPER_BODY || hides.includes(WearableCategory.UPPER_BODY)
      ? [BodyPartCategory.HANDS]
      : []
  return {
    ...base,
    data: {
      category,
      hides,
      replaces: [],
      removesDefaultHiding,
      tags: [],
      representations: [{ bodyShapes, mainFile: MODEL_KEY, contents, overrideHides: hides, overrideReplaces: [] }]
    }
  }
}
