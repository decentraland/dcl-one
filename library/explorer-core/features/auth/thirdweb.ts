// thirdweb in-app wallets: the guest's, a custodial wallet reached by an opaque session id, the way
// the Unity and Godot clients create persistent guests (the same session id always resolves the
// same wallet), and an email account's, opened by a code sent to the address. Dependency-free: the
// page calls it on web, the bridge scene on native.

import { ShellError } from '../../lib/shellReply'

export const THIRDWEB_CLIENT_ID = 'e1adce863fe287bb6cf0e3fd90bdb77f'
// Where the thirdweb SDK keeps a guest's session id, so a site on this origin that signs a guest
// in through the SDK reaches the same wallet.
export const GUEST_SESSION_KEY = `thirdweb_guest_session_id_${THIRDWEB_CLIENT_ID}`

const API = 'https://api.thirdweb.com'
// The enclave that signs for the wallet; it takes the user's token, not a secret key.
const ENCLAVE = 'https://embedded-wallet.thirdweb.com'

export interface GuestWallet {
  token: string
  walletAddress: string
}

class ThirdwebHttpError extends Error {
  constructor(
    readonly status: number,
    readonly body: string
  ) {
    super(`thirdweb ${status}: ${body}`)
  }
}

async function post<T>(url: string, headers: Record<string, string>, body: object): Promise<T> {
  const res = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', ...headers },
    body: JSON.stringify(body)
  })
  if (!res.ok) throw new ThirdwebHttpError(res.status, await res.text())
  return (await res.json()) as T
}

/** Sign the guest in. `headers` are added to the request (native sends an allowlisted Origin). */
export async function guestLogin(sessionId: string, headers: Record<string, string> = {}): Promise<GuestWallet> {
  const r = await post<GuestWallet>(`${API}/v1/auth/complete`, { 'x-client-id': THIRDWEB_CLIENT_ID, ...headers }, {
    method: 'guest',
    sessionId
  })
  return { token: r.token, walletAddress: r.walletAddress }
}

/** personal_sign `message` with the guest's wallet. */
export async function guestSign(token: string, message: string, headers: Record<string, string> = {}): Promise<string> {
  const r = await post<{ signature: string }>(
    `${ENCLAVE}/api/v1/enclave-wallet/sign-message`,
    { 'x-thirdweb-client-id': THIRDWEB_CLIENT_ID, Authorization: `Bearer embedded-wallet-token:${token}`, ...headers },
    { messagePayload: { message, isRaw: false, chainId: 1 } }
  )
  return r.signature
}

// --- email sign-in: a code sent to the address, then the in-app wallet it opens ---

/** What a failed email sign-in comes to, for the screens to act on: a code that is wrong or has expired
 *  (try again), an address thirdweb refuses (fix the address), or a right code whose wallet won't sign
 *  here (the sign-in page takes over). Anything else is a connection error. */
export type EmailLoginErrorCode = 'incorrect-code' | 'invalid-email' | 'sign-refused'

/** An email sign-in failure the screens act on. The code is in the message too, so it survives the hops
 *  that carry errors as text (the bridge scene's replies). The address is never in it. */
export class EmailLoginError extends ShellError {
  constructor(code: EmailLoginErrorCode, detail: string) {
    // a ShellError, so the web shell's answer keeps the code
    super(code, `[${code}] ${detail}`)
    this.name = 'EmailLoginError'
  }
}

/** The code of an email sign-in failure, however it arrived (thrown here, from the shell with its
 *  code, or as the bridge scene's text); null for anything else. */
export function emailLoginErrorCode(e: unknown): EmailLoginErrorCode | null {
  const code = (e as { code?: unknown } | null)?.code
  if (code === 'incorrect-code' || code === 'invalid-email' || code === 'sign-refused') return code
  const m = /\[(incorrect-code|invalid-email|sign-refused)\]/.exec(e instanceof Error ? e.message : String(e))
  return m != null ? (m[1] as EmailLoginErrorCode) : null
}

// thirdweb answers a refused address with a 4xx naming it; checking a code, any other refusal (400,
// 401, 403) is the code's: wrong or expired
function emailFailure(e: unknown, checkingCode: boolean): unknown {
  if (!(e instanceof ThirdwebHttpError) || e.status >= 500) return e
  if (/invalid email/i.test(e.body)) return new EmailLoginError('invalid-email', `thirdweb ${e.status}`)
  if (checkingCode && [400, 401, 403].includes(e.status)) return new EmailLoginError('incorrect-code', `thirdweb ${e.status}`)
  return e
}

/** Send a sign-in code to `email`. `headers` as for guestLogin. */
export async function emailLoginStart(email: string, headers: Record<string, string> = {}): Promise<void> {
  try {
    await post<unknown>(`${API}/v1/auth/initiate`, { 'x-client-id': THIRDWEB_CLIENT_ID, ...headers }, { method: 'email', email })
  } catch (e) {
    throw emailFailure(e, false)
  }
}

/** Check the code sent to `email`; the wallet it opens signs like the guest's (guestSign). */
export async function emailLoginComplete(email: string, code: string, headers: Record<string, string> = {}): Promise<GuestWallet> {
  try {
    const r = await post<GuestWallet>(`${API}/v1/auth/complete`, { 'x-client-id': THIRDWEB_CLIENT_ID, ...headers }, { method: 'email', email, code })
    return { token: r.token, walletAddress: r.walletAddress }
  } catch (e) {
    throw emailFailure(e, true)
  }
}

