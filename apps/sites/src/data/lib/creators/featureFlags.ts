// Decentraland feature flags, read through the site's `/internal/feature-flags` like the rest of Sites: a cached
// fetch behind an async accessor, keyed `${app}-${feature}`.

/** Flag names as they appear in the service, without their application prefix. */
export enum FeatureFlag {
  /**
   * Ceiling for the Unity wearable preview, shared with the shop and the legacy builder: one switch turns
   * Unity off across every dapp. The device heuristic in `lib/pickRenderer` still applies on top.
   */
  UNITY_WEARABLE_PREVIEW = 'unity-wearable-preview',
  /** Marketing campaign surfaces (the event tag hint), shared with the legacy builder. */
  CAMPAIGN = 'campaign',
  /** Platform-wide stop: every surface of this app is replaced with the maintenance notice. */
  MAINTENANCE = 'maintenance',
  /** The item's Utility field, which the catalog surfaces on published items. */
  WEARABLE_UTILITY = 'wearable-utility',
  /** The per-wearable VRM export opt-out. */
  VRM_OPTOUT = 'vrm-optout',
  /** The Blender live preview page (`/live-preview`), fed by the dcl-blender-toolkit add-on. */
  BLENDER_LIVE_PREVIEW = 'blender-live-preview',
  /** Listing items through off-chain public orders (trades), which is the only sale path here. */
  OFFCHAIN_PUBLIC_ITEM_ORDERS = 'offchain-public-item-orders',
  /** Credits as a listing currency; with it off, an item can only be priced in MANA. */
  CREDITS_PRIMARY_LISTINGS = 'credits-primary-listings',
  /** Paying the collection publication fee with shop credits. */
  SHOP_CREDITS_FOR_COLLECTIONS_FEE = 'shop-credits-for-collections-fee',
  /**
   * Items with validation errors block publishing a collection; off, the errors are advisory. A check that could
   * not run (download, worker or rule book failure) is a warning either way: infrastructure never blocks publishing.
   */
  BLOCK_PUBLISH_ON_VALIDATION_ERRORS = 'block-publish-on-validation-errors',
  /** Creating and ending collection discounts from the collection page. Off unless the Shop applies them too. */
  COLLECTION_DISCOUNTS = 'collection-discounts',
  /** The Shop applies creator discounts for buyers: the one switch that takes every discount away. */
  SHOP_CREATOR_SALES = 'shop-creator-sales',
  /** The Shop also discounts MANA listings, so a discount reaches them and not only credits listings. */
  SHOP_LEGACY_COUPONS = 'shop-legacy-coupons',
  /**
   * The automated review as creators and curators see it: the timeline, reason-coded rejections, retries and
   * appeals. Mirrors the server's flag so the app never calls what the server doesn't serve. The reviews the
   * server runs in the background for the curators' channel have their own flag, which the app never reads.
   */
  AUTO_CURATION_REVIEW_FLOW = 'auto-curation-review-flow'
}

/** Each flag lives under the application that owns it, and is fetched from that application's file. */
const APPLICATION: Record<FeatureFlag, string> = {
  [FeatureFlag.UNITY_WEARABLE_PREVIEW]: 'dapps',
  [FeatureFlag.CAMPAIGN]: 'builder',
  [FeatureFlag.MAINTENANCE]: 'builder',
  [FeatureFlag.WEARABLE_UTILITY]: 'dapps',
  [FeatureFlag.VRM_OPTOUT]: 'builder',
  [FeatureFlag.BLENDER_LIVE_PREVIEW]: 'builder',
  [FeatureFlag.OFFCHAIN_PUBLIC_ITEM_ORDERS]: 'dapps',
  [FeatureFlag.CREDITS_PRIMARY_LISTINGS]: 'builder',
  [FeatureFlag.SHOP_CREDITS_FOR_COLLECTIONS_FEE]: 'builder',
  [FeatureFlag.BLOCK_PUBLISH_ON_VALIDATION_ERRORS]: 'builder',
  [FeatureFlag.COLLECTION_DISCOUNTS]: 'builder',
  [FeatureFlag.SHOP_CREATOR_SALES]: 'dapps',
  [FeatureFlag.SHOP_LEGACY_COUPONS]: 'dapps',
  [FeatureFlag.AUTO_CURATION_REVIEW_FLOW]: 'builder'
}

const TTL_MS = 60_000
const TIMEOUT_MS = 3_000

type Snapshot = { flags: Record<string, boolean>; fetchedAt: number }

const snapshots = new Map<string, Snapshot>()
const inFlight = new Map<string, Promise<Snapshot>>()

async function fetchSnapshot(application: string): Promise<Snapshot> {
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), TIMEOUT_MS)
  try {
    // Read through this site: the flag service answers by the asking site, which only the server can name.
    const query = new URLSearchParams({ app: application, variants: '1' })
    const response = await fetch(`/internal/feature-flags?${query}`, { signal: controller.signal })
    if (!response.ok) {
      await response.body?.cancel()
      throw new Error(`feature flags request failed with ${response.status}`)
    }
    const body = (await response.json()) as { flags?: Record<string, boolean> }
    return { flags: body.flags ?? {}, fetchedAt: Date.now() }
  } finally {
    clearTimeout(timer)
  }
}

// A failed fetch is not cached: the next read retries, so an outage never outlives itself.
async function getSnapshot(application: string): Promise<Snapshot> {
  const cached = snapshots.get(application)
  if (cached && Date.now() - cached.fetchedAt < TTL_MS) return cached
  let pending = inFlight.get(application)
  if (!pending) {
    pending = fetchSnapshot(application)
      .then(fresh => {
        snapshots.set(application, fresh)
        return fresh
      })
      .finally(() => {
        inFlight.delete(application)
      })
    inFlight.set(application, pending)
  }
  return await pending
}

/**
 * Local override for dev builds: `VITE_FEATURE_FLAG_OVERRIDES=unity-wearable-preview:true`. Vite drops
 * the branch from production bundles through `import.meta.env.DEV`.
 */
function devOverrideFor(flag: FeatureFlag): boolean | undefined {
  if (!import.meta.env.DEV) return undefined
  const raw = import.meta.env.VITE_FEATURE_FLAG_OVERRIDES
  if (typeof raw !== 'string' || raw.length === 0) return undefined
  for (const entry of raw.split(',')) {
    const separator = entry.indexOf(':')
    if (separator === -1) continue
    if (entry.slice(0, separator).trim() !== String(flag)) continue
    const value = entry.slice(separator + 1).trim()
    if (value === 'true') return true
    if (value === 'false') return false
  }
  return undefined
}

/** Whether a flag is on. Fails closed: an unreachable service, a malformed body or an absent flag read `false`. */
export async function getIsFeatureEnabled(flag: FeatureFlag): Promise<boolean> {
  const override = devOverrideFor(flag)
  if (override !== undefined) return override
  const application = APPLICATION[flag]
  try {
    return (await getSnapshot(application)).flags[`${application}-${flag}`] === true
  } catch {
    return false
  }
}

/** Test seam: drops the cached snapshot so a spec starts from a known state. */
export function resetFeatureFlagsCache(): void {
  snapshots.clear()
  inFlight.clear()
}
