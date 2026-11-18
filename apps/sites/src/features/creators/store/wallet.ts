import { createStore } from '@/data/lib/creators/store'
import { track } from '@/data/lib/creators/analytics'
import { currentSession, subscribeSession, type Session } from '@/data/lib/creators/auth'
import { setCurrentAddressReader } from '@/data/lib/creators/currentAddress'
import { openSignIn } from '../../components/auth/signin-store'

type WalletState = {
  session: Session | null
  /** False until the browser's session has been read: a page waits for it before treating the visitor as signed out. */
  restored: boolean
  signIn: () => void
  restore: () => void
}

let stopListening: (() => void) | undefined

export const useWallet = createStore<WalletState>((set, get) => {
  // Sites owns the sign-in: this store only mirrors the account it holds, keeping one session object per account.
  const sync = () => {
    const next = currentSession()
    const previous = get().session
    const same = previous?.address === next?.address && previous?.kind === next?.kind
    set({ session: same ? previous : next, restored: true })
  }
  return {
    session: null,
    restored: false,
    signIn: () => {
      track('Login')
      openSignIn()
    },
    restore: () => {
      stopListening ??= subscribeSession(sync)
      sync()
    }
  }
})

setCurrentAddressReader(() => useWallet.getState().session?.address)
