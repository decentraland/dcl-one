import { useQuery } from '@tanstack/react-query'
import { fetchCreditsBalance } from '~/lib/credits'
import { config } from '~/config'

export function useCreditsBalance(address: string | undefined) {
  return useQuery({
    queryKey: ['credits-balance', address],
    queryFn: () => fetchCreditsBalance(address!),
    enabled: !!address,
    staleTime: 30_000
  })
}

/** Polygon MANA balance in wei. */
export function useManaBalance(address: string | undefined) {
  const chainId = Number(config.get('MATIC_CHAIN_ID'))
  return useQuery({
    queryKey: ['mana-balance', address, chainId],
    // The navbar reads it on every page, the overview included, which must not load the contract registry.
    queryFn: async () => (await import('~/lib/mana')).fetchManaBalance(address!, chainId),
    enabled: !!address,
    staleTime: 30_000
  })
}
