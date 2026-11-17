// The signed-in identity, kept by the shell for the app (src/lib/shell.ts shellRequest): the app
// gets the public identity and asks for signatures, and never sees the key.
import { createGuestIdentity, webGuestBackendFor } from '@explorer-core/features/auth/guest'
import {
  clearGuestLogin,
  clearStoredLogins,
  getStoredLogin,
  loginExpired,
  publicLogin,
  rootAddress,
  storeAccountLogin,
  storeGuestLogin,
  type AuthIdentity,
  type StoredLogin
} from '@explorer-core/features/auth/sso'
import { completeEmailSignIn, startEmailSignIn } from '@explorer-core/shell/emailLogin'
import { personalSign } from '@explorer-core/shell/sign'

// The login handed to the app, key included. It is the only one the shell signs with, and once it
// has signed it is kept for the session, so signing out or in again in another tab doesn't pull it
// from under the running engine. Until then (the login screen), or once it expires, storage is
// read again.
let held: StoredLogin | null = null
let pinned = false
/** The login the shell holds now, key included: the in-world Shop's buyer (shop/purchase). */
export function currentLogin(): StoredLogin | null {
  if (!pinned || !held || loginExpired(held)) {
    held = getStoredLogin()
    pinned = false
  }
  return held
}

export async function request(method: unknown, params: unknown): Promise<unknown> {
  switch (method) {
    case 'identity':
      return publicLogin(currentLogin())
    case 'sign': {
      const { signer, message } = (params ?? {}) as { signer?: unknown; message?: unknown }
      if (typeof signer !== 'string' || typeof message !== 'string') throw new Error('bad sign request')
      const login = currentLogin()
      const ephemeral = login?.identity.ephemeralIdentity
      if (!login || !ephemeral?.privateKey || ephemeral.address.toLowerCase() !== signer.toLowerCase()) {
        throw new Error(`not signed in as ${signer}`)
      }
      pinned = true
      // Signing in with an account replaces the guest (an expired one included).
      if (!login.guest) clearGuestLogin()
      return personalSign(ephemeral.privateKey, message)
    }
    // "Explore as guest": a guest account that persists across visits (features/auth/guest.ts),
    // created and kept here like a sign-in. Any earlier guest is replaced.
    case 'createGuest': {
      const identity = await createGuestIdentity(webGuestBackendFor(location.hostname))
      storeGuestLogin(identity)
      held = { address: rootAddress(identity).toLowerCase(), identity, guest: true }
      pinned = true
      return publicLogin(held)
    }
    // Email sign-in (features/startup): the code is sent, then checked here, and the account's
    // identity kept like the sign-in page's, key included.
    case 'emailStart':
      await startEmailSignIn(params)
      return null
    case 'emailLogin': {
      const { identity, wallet } = await completeEmailSignIn(params)
      storeAccountLogin(identity)
      // the session state the sign-in page leaves for an email account, which the Shop reads
      recordInAppSession(identity, wallet)
      held = { address: rootAddress(identity).toLowerCase(), identity }
      pinned = true
      return publicLogin(held)
    }
    // Sign out. The guest is kept: it is only reachable through its stored session id.
    case 'logout':
      clearStoredLogins()
      forgetInAppSession()
      held = null
      pinned = false
      return null
    default:
      throw new Error(`unknown request ${String(method)}`)
  }
}

// Sites' keys for an in-app (thirdweb) sign-in: its token, for the Shop to sign with, and that the
// account is a managed one, not a browser wallet's.
export const THIRDWEB_SESSION_KEY = 'dcl:auth:thirdweb:v1'
export const SIGN_IN_KIND_KEY = 'dcl:auth:signin-kind:v1'

function recordInAppSession(identity: AuthIdentity, wallet: { token: string; walletAddress: string }): void {
  try {
    localStorage.setItem(THIRDWEB_SESSION_KEY, JSON.stringify({ token: wallet.token, address: wallet.walletAddress }))
    localStorage.setItem(SIGN_IN_KIND_KEY, JSON.stringify({ signer: rootAddress(identity).toLowerCase(), kind: 'managed' }))
  } catch {
    // without them the Shop infers the kind of account as before
  }
}

/** Sign-out drops them too, as the sign-in page's does. */
function forgetInAppSession(): void {
  try {
    localStorage.removeItem(THIRDWEB_SESSION_KEY)
    localStorage.removeItem(SIGN_IN_KIND_KEY)
  } catch {
    // nothing kept
  }
}

