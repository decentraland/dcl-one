// PLAY AS GUEST in the web shells, which keep the guest like a sign-in (each host's shell/login.ts).
import { createGuestIdentity, type GuestBackend, type GuestLogin } from '../features/auth/guest'
import { resumeGuestLogin, rootAddress, storeGuestLogin } from '../features/auth/sso'

/** The kept guest signed in again, as unity-explorer brings back a guest that signed out; else a guest
 *  identity, kept from now on. `wallet` says which wallet step it took. */
export async function playAsGuest(backend: GuestBackend): Promise<GuestLogin> {
  const kept = resumeGuestLogin()
  if (kept != null) return { ...kept, wallet: null }
  const wallet = backend.keptSessionId?.() ? 'existing' : 'new'
  const identity = await createGuestIdentity(backend)
  storeGuestLogin(identity)
  return { address: rootAddress(identity).toLowerCase(), identity, guest: true, wallet }
}
