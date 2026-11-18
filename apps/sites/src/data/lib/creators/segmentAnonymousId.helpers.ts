const ANONYMOUS_ID_KEY = 'ajs_anonymous_id'
const DOMAIN_PROBE_KEY = '__dcl_segment_domain__'
const ONE_YEAR_MS = 365 * 24 * 60 * 60 * 1000

function generateUuid(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') return crypto.randomUUID()
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, char => {
    const random = Math.floor(Math.random() * 16)
    return (char === 'x' ? random : (random & 0x3) | 0x8).toString(16)
  })
}

function safeParseStoredId(value: string): string | undefined {
  try {
    const parsed: unknown = JSON.parse(value)
    if (typeof parsed === 'number') return String(parsed)
    return typeof parsed === 'string' && parsed !== '' ? parsed : undefined
  } catch {
    return value || undefined
  }
}

function readCookie(key: string): string | undefined {
  try {
    for (const entry of document.cookie.split('; ')) {
      if (!entry.startsWith(`${key}=`)) continue
      try {
        let value = entry.slice(key.length + 1)
        // js-cookie strips quotes before decoding and skips undecodable entries.
        if (value[0] === '"') value = value.slice(1, -1)
        return safeParseStoredId(value.replace(/(%[\dA-F]{2})+/gi, decodeURIComponent))
      } catch {
        // A malformed host cookie must not hide a readable parent cookie.
      }
    }
  } catch {
    // Cookie access can be blocked independently from localStorage.
  }
  return undefined
}

function readStoredId(key: string): string | undefined {
  try {
    const raw = localStorage.getItem(key)
    return raw ? safeParseStoredId(raw) : undefined
  } catch {
    return undefined
  }
}

// Like Analytics.js, probe from the shortest candidate to the full host. Browsers
// reject public suffixes such as vercel.app, so previews stay host-scoped.
function writableCookieDomain(): string | undefined {
  const host = window.location.hostname
  const parts = host.split('.')
  if (/^\d+\.\d+\.\d+\.\d+$/.test(host)) return undefined
  const token = generateUuid()
  const probeKey = `${DOMAIN_PROBE_KEY}${token}`
  for (let index = parts.length - 2; index >= 0; index--) {
    const domain = parts.slice(index).join('.')
    try {
      document.cookie = `${probeKey}=${token}; domain=.${domain}; path=/; SameSite=Lax; max-age=5`
      if (readCookie(probeKey) === token) return domain
    } finally {
      document.cookie = `${probeKey}=; domain=.${domain}; path=/; max-age=0`
    }
  }
  return undefined
}

function persist(id: string, resolveDomain: () => string | undefined): boolean {
  let localPersisted = false
  let cookiePersisted = false
  // Each store can fail independently (privacy settings, sandboxed frames).
  try {
    localStorage.setItem(ANONYMOUS_ID_KEY, JSON.stringify(id))
    localPersisted = readStoredId(ANONYMOUS_ID_KEY) === id
  } catch {
    // A readable cookie still preserves identity when localStorage is blocked.
  }
  try {
    if (readCookie(ANONYMOUS_ID_KEY) === id) return true
    const domain = resolveDomain()
    const domainAttribute = domain ? `; domain=.${domain}` : ''
    document.cookie = `${ANONYMOUS_ID_KEY}=${encodeURIComponent(id)}; path=/; SameSite=Lax; expires=${new Date(Date.now() + ONE_YEAR_MS).toUTCString()}${domainAttribute}`
    cookiePersisted = readCookie(ANONYMOUS_ID_KEY) === id
  } catch {
    // localStorage or the in-page fallback still labels direct events.
  }
  return localPersisted || cookiePersisted
}

/** Matches Analytics.js >=1.84.3 without awaiting its buffered browser facade. */
function createAnonymousIdResolver(getSdkId: () => string | undefined) {
  let memoryId: string | undefined
  let cachedDomain: string | undefined
  let retryDomainAt = -Infinity

  function resolveDomain(): string | undefined {
    if (cachedDomain || Date.now() < retryDomainAt) return cachedDomain
    // Bound blocked-store work without preventing recovery during this page load.
    retryDomainAt = Date.now() + 5000
    cachedDomain = writableCookieDomain()
    return cachedDomain
  }

  function read(): string | undefined {
    let sdkId: string | undefined
    try {
      sdkId = getSdkId()
    } catch {
      // An unavailable SDK must not prevent unload-safe direct events.
    }
    return sdkId || readCookie(ANONYMOUS_ID_KEY) || readStoredId(ANONYMOUS_ID_KEY)
  }

  function ensure(): string {
    const id = read() || memoryId || generateUuid()
    // Cache only when neither store works; do not resurrect an id deleted
    // from otherwise usable stores before the SDK has loaded.
    memoryId = persist(id, resolveDomain) ? undefined : id
    return id
  }

  return { ensure }
}

export { createAnonymousIdResolver, readStoredId }
