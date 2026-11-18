// credits-server client (shop credits, USD-denominated: 1 credit = 10 US cents). Requests are
// ADR-44 signed through the lib/auth chokepoint.
import { config } from '~/config'
import { signedFetch } from '~/lib/auth'

export type CreditsBalance = {
  credits: number
  balanceCents: number
}

/** The CreditsManager `ExternalCall` struct, as the server co-signs it. */
export type ExternalCall = {
  target: string
  selector: string
  data: string
  /** Unix seconds; the server schema rejects a string here. */
  expiresAt: number
  salt: string
}

export type PublicationAuthorization = {
  credit: {
    id: string
    amount: string
    expiresAt: string
    signature: string
  }
  externalCallSignature: string
}

export type CreditsServerErrorCode = 'insufficient_credits' | 'generic'

export class CreditsServerError extends Error {
  status: number
  code: CreditsServerErrorCode

  constructor(message: string, status: number) {
    super(message)
    this.name = 'CreditsServerError'
    this.status = status
    this.code = status === 402 ? 'insufficient_credits' : 'generic'
  }
}

const baseUrl = () => config.get('CREDITS_SERVER_URL')

async function parseError(response: Response, fallback: string): Promise<CreditsServerError> {
  const body = (await response.json().catch(() => null)) as { error?: string } | null
  return new CreditsServerError(body?.error ?? `${fallback} (${response.status})`, response.status)
}

/** The creator's shop-credit balance: GET /users/{address}/credits (the `usd` block). */
export async function fetchCreditsBalance(address: string): Promise<CreditsBalance> {
  const response = await signedFetch(address, baseUrl(), `/users/${address}/credits`)
  if (!response.ok) throw await parseError(response, 'credits-server request failed')
  const data = (await response.json()) as { usd?: { balanceCents: number; credits: number } }
  return { credits: data.usd?.credits ?? 0, balanceCents: data.usd?.balanceCents ?? 0 }
}

export type CreditsCheckout = {
  orderId: string
  /** Stripe's hosted checkout page, which returns to this app when the buyer is done. */
  url: string
}

export type CreditsOrderStatus = 'initiated' | 'processing' | 'crediting' | 'credited' | 'failed' | 'abandoned'

export type CreditsOrder = {
  status: CreditsOrderStatus
  creditsGranted?: number
  newBalance?: number
  error?: string
}

/** How the wait for a paid order ended: the server's own statuses, or `pending` when the poll gave up. */
export type CreditsOrderOutcome = CreditsOrder | { status: 'pending' }

/** The buyer's IANA zone, a coarse region hint the server stores with the order; undefined when unknown. */
function buyerTimezone(): string | undefined {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || undefined
  } catch {
    return undefined
  }
}

/**
 * Starts a credit-pack purchase: POST /credits/checkout. `source: 'builder'` is what makes Stripe return
 * the buyer to this app rather than to the shop. The server prices the pack; the client only names it.
 */
export async function createCreditsCheckout(
  address: string,
  selection: { packId: string; quantity: number }
): Promise<CreditsCheckout> {
  const response = await signedFetch(address, baseUrl(), '/credits/checkout', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ ...selection, timezone: buyerTimezone(), source: 'builder' })
  })
  if (!response.ok) throw await parseError(response, 'Failed to start the checkout')
  const data = (await response.json()) as Partial<CreditsCheckout>
  if (!data.orderId || !data.url) {
    throw new CreditsServerError('credits-server answered an incomplete checkout', response.status)
  }
  return { orderId: data.orderId, url: data.url }
}

/**
 * GET /credits/orders/{orderId}. A 404 right after the Stripe return is read as `processing`: the row
 * exists but can lag behind a replica, and the server answers the same 404 for "not yours" while the
 * identity is still being restored. The webhook is the source of truth, so a paid order is never failed here.
 */
export async function fetchCreditsOrder(address: string, orderId: string, signal?: AbortSignal): Promise<CreditsOrder> {
  const response = await signedFetch(address, baseUrl(), `/credits/orders/${encodeURIComponent(orderId)}`, { signal })
  if (response.status === 404) {
    await response.body?.cancel()
    return { status: 'processing' }
  }
  if (!response.ok) throw await parseError(response, 'Failed to read the order')
  return (await response.json()) as CreditsOrder
}

const WAITING_STATUSES: CreditsOrderStatus[] = ['initiated', 'processing', 'crediting']

type PollOptions = { intervalMs?: number; timeoutMs?: number; signal?: AbortSignal }

/**
 * Waits for the webhook to settle a paid order. `initiated` also waits, since the return can beat the
 * webhook that moves the order off it; at the deadline a still-`initiated` order is reported as such
 * (nobody paid) while anything else becomes `pending` (paid, credits on their way).
 */
export async function pollCreditsOrder(
  address: string,
  orderId: string,
  { intervalMs = 1500, timeoutMs = 60_000, signal }: PollOptions = {}
): Promise<CreditsOrderOutcome> {
  const deadline = Date.now() + timeoutMs
  for (;;) {
    if (signal?.aborted) throw new DOMException('Aborted', 'AbortError')
    const order = await fetchCreditsOrder(address, orderId, signal)
    if (!WAITING_STATUSES.includes(order.status)) return order
    if (Date.now() >= deadline) return order.status === 'initiated' ? order : { status: 'pending' }
    await delay(intervalMs, signal)
  }
}

function delay(ms: number, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) {
      reject(new DOMException('Aborted', 'AbortError'))
      return
    }
    const abort = () => {
      clearTimeout(timer)
      reject(new DOMException('Aborted', 'AbortError'))
    }
    const timer = setTimeout(() => {
      signal?.removeEventListener('abort', abort)
      resolve()
    }, ms)
    signal?.addEventListener('abort', abort, { once: true })
  })
}

/**
 * Tells the server the buyer walked away from a checkout. Best effort and silent: the server asks Stripe
 * before retiring the order, and retires it on its own when the session expires anyway.
 */
export async function cancelCreditsOrder(address: string, orderId: string): Promise<void> {
  try {
    const response = await signedFetch(address, baseUrl(), `/credits/orders/${encodeURIComponent(orderId)}/cancel`, {
      method: 'POST'
    })
    await response.body?.cancel()
  } catch {
    // The server retires it on its own; see above.
  }
}

export type AuthorizePublicationParams = {
  usdPriceCents: number
  chainId: number
  creditsManagerAddress: string
  externalCall: ExternalCall
}

/**
 * Pays a publication fee with shop credits: POST /credits/authorize-publication. The server
 * reserves the USD amount and answers a signed ephemeral MANA credit plus the co-signed external
 * call, both consumed by a single CreditsManager.useCredits transaction. 402 = not enough credits.
 */
export async function authorizePublication(
  address: string,
  params: AuthorizePublicationParams
): Promise<PublicationAuthorization> {
  const response = await signedFetch(address, baseUrl(), '/credits/authorize-publication', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(params)
  })
  if (!response.ok) throw await parseError(response, 'Failed to authorize the publication')
  const data = (await response.json()) as Partial<PublicationAuthorization>
  const credit = data.credit
  if (!credit?.id || !credit.amount || !credit.expiresAt || !credit.signature || !data.externalCallSignature) {
    throw new CreditsServerError('credits-server answered an incomplete authorization', response.status)
  }
  return { credit, externalCallSignature: data.externalCallSignature }
}
