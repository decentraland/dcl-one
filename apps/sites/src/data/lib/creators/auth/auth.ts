// lib/auth is the ONLY place allowed to import decentraland-connect, @dcl/single-sign-on-client or @dcl/crypto,
// or to hold an EIP-1193 provider (see CONVENTIONS.md, host-portability seams). Everything else uses
// the wallet store plus the helpers exported from its index.
import { type ChainId } from '@dcl/schemas/dist/dapps/chain-id'
import { ProviderType } from '@dcl/schemas/dist/dapps/provider-type'
import { type AuthChain, type AuthIdentity } from '@dcl/crypto'
import {
  localStorageClearIdentity,
  localStorageGetIdentity,
  localStorageStoreIdentity
} from '@dcl/single-sign-on-client'
import { config } from '~/config'
import { resyncServerClock, serverClockOffset, serverNow } from './serverClock'

// ~31 days, same as the legacy builder and marketplace webapps.
const IDENTITY_EXPIRATION_MINUTES = 31 * 24 * 60

const AUTH_CHAIN_HEADER_PREFIX = 'x-identity-auth-chain-'

// decentraland-connect drags in the whole wallet-modal stack (@reown/appkit, coinbase, magic, ...).
// Load it on demand so none of that lands in the initial bundle — it's only needed to (re)connect,
// sign in, or sign out, all of which are already async.
async function getConnection() {
  return (await import('decentraland-connect')).connection
}

/** A wallet's raw EIP-1193 provider; providers without events (Magic) leave `on` out. */
export type Eip1193Provider = {
  request: (args: { method: string; params?: readonly unknown[] | object }) => Promise<unknown>
  on?: (event: string, listener: (...args: unknown[]) => void) => void
  removeListener?: (event: string, listener: (...args: unknown[]) => void) => void
}

export type Session = {
  address: string
  chainId: number
  provider: Eip1193Provider
  identity: AuthIdentity
  providerType: ProviderType
  /** The wallet behind the provider ("Trust Wallet" over WalletConnect, …), when it names itself. */
  walletName?: string
}

async function toSession(res: {
  account: string | null
  provider: unknown
  chainId: ChainId
  providerType: ProviderType
}): Promise<Session> {
  if (!res.account) throw new Error('No account returned by the wallet')
  const address = res.account.toLowerCase()
  const provider = res.provider as Eip1193Provider

  // Reuse a valid stored identity, otherwise create one (a single wallet signature).
  let identity = localStorageGetIdentity(address)
  if (!identity) {
    const { createIdentity } = await import('./signer')
    identity = await createIdentity(provider, address, IDENTITY_EXPIRATION_MINUTES)
    localStorageStoreIdentity(address, identity)
  }

  return { address, chainId: res.chainId, provider, identity, providerType: res.providerType }
}

/**
 * Social login (Google, Discord, Apple, email) runs through Magic, whose custodial key signs without any
 * wallet prompt. Use it to decide whether the UI must explain a pending wallet confirmation.
 */
export function isSocialLogin(session: Pick<Session, 'providerType'>): boolean {
  return session.providerType === ProviderType.MAGIC || session.providerType === ProviderType.MAGIC_TEST
}

// Redirects to the auth app (method chooser). On return, restoreSession() rebuilds the session.
export function signInRedirect(): void {
  const redirectTo = encodeURIComponent(window.location.href)
  window.location.replace(`${config.get('AUTH_URL')}/login?redirectTo=${redirectTo}`)
}

// Silent: never asks the wallet to sign, so a visitor without a stored identity stays signed out.
export async function restoreSession(): Promise<Session | null> {
  try {
    const connection = await getConnection()
    const res = await connection.tryPreviousConnection()
    if (!res.account || !localStorageGetIdentity(res.account.toLowerCase())) return null
    const session = await toSession(res)
    return { ...session, walletName: connection.getWalletName() }
  } catch {
    return null
  }
}

export async function logout(address?: string): Promise<void> {
  try {
    const connection = await getConnection()
    await connection.disconnect()
  } catch {
    // ignore
  }
  // The ephemeral signing identity must not outlive an explicit sign-out.
  if (address) localStorageClearIdentity(address.toLowerCase())
}

export function getIdentity(address: string): AuthIdentity | null {
  return localStorageGetIdentity(address.toLowerCase())
}

// The curve code behind it is only needed once there is an identity to sign with, never on a visitor's first load.
async function signPayload(identity: AuthIdentity, payload: string): Promise<AuthChain> {
  const { Authenticator } = await import('@dcl/crypto')
  return Authenticator.signPayload(identity, payload)
}

/** Signs `payload` (a Catalyst entity id) with the wallet's ephemeral identity; null when the session has none. */
export async function signWithIdentity(address: string, payload: string): Promise<AuthChain | null> {
  const identity = getIdentity(address)
  return identity ? signPayload(identity, payload) : null
}

const AUTH_TIMESTAMP_HEADER = 'x-identity-timestamp'
const AUTH_METADATA_HEADER = 'x-identity-metadata'

/**
 * ADR-44 signed-request headers: the auth chain signs `{method}:{path}:{timestamp}:{metadata}`
 * lowercased, and the timestamp/metadata travel as headers so the server can rebuild the payload.
 * `path` is the full URL pathname (API base included), without the query string.
 */
export async function createAuthHeaders(
  address: string,
  method: string,
  path: string,
  metadata: Record<string, string> = {}
): Promise<Record<string, string>> {
  const headers: Record<string, string> = {}
  const identity = getIdentity(address)
  if (!identity) return headers
  // Corrected by the measured server clock offset: see serverClock.
  const timestamp = String(serverNow())
  const metadataJson = JSON.stringify(metadata)
  const payload = `${method}:${path}:${timestamp}:${metadataJson}`.toLowerCase()
  const authChain = await signPayload(identity, payload)
  for (let i = 0; i < authChain.length; i++) {
    headers[`${AUTH_CHAIN_HEADER_PREFIX}${i}`] = JSON.stringify(authChain[i])
  }
  headers[AUTH_TIMESTAMP_HEADER] = timestamp
  headers[AUTH_METADATA_HEADER] = metadataJson
  return headers
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

  const signed = Boolean(address && getIdentity(address))
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
