// The creators code's session, on Sites' sign-in: the signed identity Sites keeps, and the wallet that signs
// for it (a browser wallet, or the in-app signer of an email or social account).
import { getIdentity, subscribe } from '../../auth/session'
import { isIdentityExpired } from '../../auth/expiry'
import { browserProvider, currentWalletKind, managedShopProvider } from '../../auth/shop-wallet'
import type { AuthIdentity, AuthLink } from '../../auth/types'
import { WalletError, type Eip1193Provider } from '../../auth/wallet'
import { signAuthChain } from '@ui/data/auth/signerCore'
import { resyncServerClock, serverClockOffset, serverNow } from './serverClock'

export type { Eip1193Provider }
export type AuthChain = AuthLink[]

export type Session = {
  address: string
  /** How the account signs: `web3` with a browser wallet, `managed` through the in-app signer (email and social accounts). */
  kind: 'web3' | 'managed'
  provider: Eip1193Provider
}

// A web3 identity whose wallet is not in this browser (signed in on another device, or through WalletConnect).
const absentWallet: Eip1193Provider = {
  request: async () => {
    throw new WalletError('No browser wallet found for this account.')
  }
}

function activeIdentity(address?: string): AuthIdentity | null {
  const identity = getIdentity()
  if (!identity || isIdentityExpired(identity)) return null
  return address === undefined || identity.signer.toLowerCase() === address.toLowerCase() ? identity : null
}

/** The identity `address` is signed in with, as Sites keeps it; null when it is another account's or expired. */
export function identityOf(address: string): AuthIdentity | null {
  return activeIdentity(address)
}

/** The signed-in account and its wallet, or null for a visitor. */
export function currentSession(): Session | null {
  const identity = activeIdentity()
  if (!identity) return null
  const address = identity.signer.toLowerCase()
  const kind = currentWalletKind(identity) === 'web3' ? 'web3' : 'managed'
  const provider = kind === 'web3' ? (browserProvider() ?? absentWallet) : managedShopProvider(address)
  return { address, kind, provider }
}

/** Calls `listener` whenever the visitor signs in, out, or as another account (in this tab or another). */
export const subscribeSession = subscribe

export function isSocialLogin(session: Pick<Session, 'kind'>): boolean {
  return session.kind === 'managed'
}

/** Signs `payload` (an entity id) with the account's identity, as the content servers expect a deployment signed. */
export async function signWithIdentity(address: string, payload: string): Promise<AuthChain | null> {
  const identity = activeIdentity(address)
  if (!identity) return null
  const { privateKeyToAccount } = await import('viem/accounts')
  const signature = await privateKeyToAccount(identity.ephemeral.privateKey).signMessage({ message: payload })
  return [...identity.authChain, { type: 'ECDSA_SIGNED_ENTITY', payload, signature }]
}

export async function createAuthHeaders(
  address: string,
  method: string,
  path: string,
  metadata: Record<string, string> = {}
): Promise<Record<string, string>> {
  const identity = activeIdentity(address)
  if (!identity) return {}
  const signed = await signAuthChain(
    identity.ephemeral.privateKey,
    identity.authChain,
    method,
    path,
    String(serverNow()),
    JSON.stringify(metadata)
  )
  return signed.headers
}

/**
 * The single signing chokepoint for server requests. `path` is relative to `baseUrl` and may carry
 * a query string; what gets signed is the resulting URL's pathname, which is what the servers verify.
 * `metadata` is signed along and sent as-is for servers that check the caller's `signer`/`intent`.
 */
export async function signedFetch(
  address: string | undefined,
  baseUrl: string,
  path: string,
  init: RequestInit = {},
  metadata: Record<string, string> = {}
): Promise<Response> {
  const method = (init.method ?? 'GET').toUpperCase()
  const url = `${baseUrl}${path}`
  const pathname = new URL(url).pathname
  const send = async () => {
    const authHeaders = address ? await createAuthHeaders(address, method, pathname, metadata) : {}
    return fetch(url, {
      ...init,
      method,
      // Caller headers win by design (e.g. Content-Type); no caller sets x-identity-* keys.
      headers: { ...authHeaders, ...init.headers }
    })
  }

  const signed = Boolean(address && activeIdentity(address))
  const signedWithOffset = serverClockOffset()
  const response = await send()
  if (response.status !== 401 || !signed) return response
  // A signed request refused as unauthenticated may only mean the device clock is off. Bring the clock
  // correction up to date and, when it differs from the one this request was signed with, sign again once.
  // Every other refusal is returned as it came.
  await resyncServerClock()
  if (serverClockOffset() === signedWithOffset) return response
  await response.body?.cancel().catch(() => undefined)
  return send()
}
