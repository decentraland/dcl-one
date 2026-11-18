import { useEffect } from 'react'
import { ProviderType } from '@dcl/schemas/dist/dapps/provider-type'
import { reset as resetAnalytics } from '~/lib/analytics'
import { useWallet } from '~/store/wallet'

// When the user switches (or disconnects) the account in an injected wallet, everything already fetched
// and every signed request belongs to the previous account. Rather than purging each query and store by
// hand, reload the page: that re-runs the silent restore for the now-active account from scratch.
// Only injected wallets emit accountsChanged; Magic sessions don't switch accounts this way.
export function useAccountWatcher() {
  const session = useWallet(s => s.session)

  useEffect(() => {
    if (!session || session.providerType !== ProviderType.INJECTED) return

    const { provider } = session
    if (!provider.on || !provider.removeListener) return

    const current = session.address.toLowerCase()

    const onAccountsChanged = (...args: unknown[]) => {
      const accounts = (args[0] as string[] | undefined) ?? []
      const next = accounts[0]?.toLowerCase()
      if (next === current) return
      // Drop the identity↔anonymousId link first: the reload re-identifies, and without this the two
      // accounts would end up sharing one anonymous visitor.
      resetAnalytics()
      window.location.reload()
    }

    provider.on('accountsChanged', onAccountsChanged)
    return () => provider.removeListener?.('accountsChanged', onAccountsChanged)
  }, [session])
}
