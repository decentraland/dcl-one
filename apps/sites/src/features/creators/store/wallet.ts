import { create } from 'zustand'
import { identify, reset as resetAnalytics, track } from '~/lib/analytics'
import { logout, restoreSession, signInRedirect, type Session } from '~/lib/auth'
import { setCurrentAddressReader } from '~/lib/currentAddress'
import { setMonitoringUser } from '~/lib/monitoring'

// The IN-FLIGHT silent restore, so concurrent callers share one pass rather than racing. Cleared once
// it settles — it dedupes overlapping calls, it is not a run-once latch.
let restoring: Promise<void> | undefined

type WalletState = {
  session: Session | null
  /**
   * Has the silent restore finished, whatever its outcome? `session === null` alone can't distinguish
   * "this visitor has no wallet" from "we have not looked yet".
   */
  restored: boolean
  connecting: boolean
  signIn: () => void
  disconnect: () => Promise<void>
  restore: () => Promise<void>
}

/**
 * The wallet names itself over WalletConnect, so the value is the wallet's to choose. Bound the
 * length and drop control/formatting characters, which nothing legitimate needs and which would
 * misrender (or reorder) the string in whatever dashboard shows it.
 */
function safeWalletName(name: string | undefined): string | null {
  const clean = name?.replace(/[\p{Cc}\p{Cf}]/gu, '').slice(0, 64)
  return clean ? clean : null
}

export const useWallet = create<WalletState>((set, get) => ({
  session: null,
  restored: false,
  connecting: false,
  // Redirect to the auth app; the user picks wallet / Magic there.
  signIn: () => {
    // The attempt, not its outcome — the app navigates away and comes back through `restore`.
    track('Login')
    signInRedirect()
  },
  disconnect: async () => {
    track('Logout')
    await logout(get().session?.address)
    set({ session: null })
    // Drops the identity↔anonymousId link so the next account isn't attributed to this one.
    resetAnalytics()
    setMonitoringUser(null)
  },
  // Silent restore on load (reads connection + stored identity, no popup). Deduped so any mount point
  // can fire it freely.
  restore: async () => {
    if (restoring) return restoring
    set({ connecting: true })
    // The catch keeps the store from wedging at connecting:true if restoreSession ever rejects
    // (e.g. the dynamic import of decentraland-connect fails).
    restoring = restoreSession()
      .catch(() => null)
      .then(session => {
        set({ session, restored: true, connecting: false })
        if (session) {
          // `ethAddress` and `chainId` are the legacy builder's traits (decentraland-dapps sends them
          // on every connection), kept so an audience built on them also covers this app.
          identify(session.address, {
            ethAddress: session.address,
            chainId: session.chainId,
            provider_type: session.providerType
          })
          // The legacy builder's connection event, fired the same way: on every load that restores a
          // session, not only on a fresh sign-in.
          track('Connect Wallet', {
            address: session.address,
            chainId: session.chainId,
            providerType: session.providerType,
            walletName: safeWalletName(session.walletName)
          })
          setMonitoringUser(session.address)
        }
      })
      .finally(() => {
        restoring = undefined
      })
    return restoring
  }
}))

// Lets analytics and Sentry stamp the signed-in creator without importing this store back.
setCurrentAddressReader(() => useWallet.getState().session?.address)
