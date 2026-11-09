import { getRealm } from '~system/Runtime'

const STORAGE_SERVER_ORG = 'https://storage.decentraland.org'
const STORAGE_SERVER_ZONE = 'https://storage.decentraland.zone'
const STAGING_TLD = '.zone'

/** The host of a URL, without scheme, credentials, port, path or query; the scene runtime has no URL class. */
function hostnameOf(url: string): string {
  const match = /^[a-z][a-z0-9+.-]*:\/\/(?:[^@/?#]*@)?(\[[^\]]*\]|[^:/?#]*)/i.exec(url)
  return match ? match[1].toLowerCase() : ''
}

async function resolveStorageServerUrl(): Promise<string> {
  const { realmInfo } = await getRealm({})

  if (!realmInfo) {
    throw new Error('Unable to retrieve realm information')
  }

  // Local development / preview mode
  if (realmInfo.isPreview) {
    return realmInfo.baseUrl
  }

  // Staging / testing environment: the realm's host is under .zone
  if (hostnameOf(realmInfo.baseUrl).endsWith(STAGING_TLD)) {
    return STORAGE_SERVER_ZONE
  }

  // Production environment
  return STORAGE_SERVER_ORG
}

let memoizedUrl: Promise<string> | null = null

/**
 * Determines the correct storage server URL based on the current realm.
 *
 * - If `isPreview` is true, uses the realm's baseUrl (localhost)
 * - If the realm's host name ends with `.zone`, uses storage.decentraland.zone
 * - Otherwise, uses storage.decentraland.org (production)
 *
 * The realm never changes mid-session, so the result is memoized; a failed
 * resolution is not memoized so transient getRealm errors can be retried.
 *
 * @returns The storage server base URL
 */
export function getStorageServerUrl(): Promise<string> {
  if (!memoizedUrl) {
    memoizedUrl = resolveStorageServerUrl()
    memoizedUrl.catch(() => {
      memoizedUrl = null
    })
  }
  return memoizedUrl
}
