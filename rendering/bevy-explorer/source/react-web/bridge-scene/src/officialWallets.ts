// The official wallets: the addresses whose posts carry the official mark. They are the explorer
// feature flags' `alfa-official-wallets` variant, named `wallets`: a CSV whose first line lists
// the addresses. Read once from the deployment's feature-flags service; a failed read marks no one
// and is tried again on a call a minute or more later.
//   from: the feature-flags service's `/explorer.json` (the catalyst mirrors the Foundation's)
// The document is public, so it is read with a plain GET: no signature (nothing of the player's goes
// to a host a link can override) and no custom header, which a CORS preflight could refuse on web.
import { serviceBase } from './http'

const FLAG = 'explorer-alfa-official-wallets'
const VARIANT = 'wallets'

type Payload = { type?: string; value?: string }
type FlagsDocument = { variants?: Record<string, { name?: string; payload?: Payload } | undefined> }

/** The addresses in the document's official-wallets variant, lowercased. The
 *  variant must be named `wallets` with a non-empty `csv` payload, and only its first line counts. */
export function parseOfficialWallets(doc: FlagsDocument | undefined): Set<string> {
  const variant = doc?.variants?.[FLAG]
  const payload = variant?.payload
  if (variant?.name !== VARIANT || payload?.type?.toLowerCase() !== 'csv' || payload.value == null || payload.value === '') return new Set()
  const first = payload.value.split('\n')[0].replace(/\r$/, '')
  return new Set(
    first
      .split(',')
      .filter((a) => a !== '')
      .map((a) => a.toLowerCase())
  )
}

const RETRY_MS = 60_000
let read: Promise<Set<string>> | null = null
let retryAt = 0

export function officialWallets(now = Date.now()): Promise<Set<string>> {
  if (read == null && now < retryAt) return Promise.resolve(new Set())
  read ??= serviceBase('featureFlags')
    .then((b) => fetch(`${b}/explorer.json`))
    .then(async (r) => {
      if (!r.ok) throw new Error(`HTTP ${r.status}`)
      return (await r.json()) as FlagsDocument
    })
    .then(parseOfficialWallets)
    .catch((e: unknown) => {
      console.error('[officialWallets] feature flags read failed', e)
      read = null
      retryAt = now + RETRY_MS
      return new Set<string>()
    })
  return read
}
