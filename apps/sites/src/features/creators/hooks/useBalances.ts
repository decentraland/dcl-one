import { privateKey } from '@/data/lib/creators/queryKeys'
import { useQuery } from '@tanstack/react-query'
import { fetchCreditsBalance } from '@/data/lib/creators/credits'
import { config } from '@/data/lib/creators/config'

export function useCreditsBalance(address: string | undefined) {
  return useQuery({
    queryKey: privateKey(address, 'credits-balance'),
    queryFn: () => fetchCreditsBalance(address!),
    enabled: !!address,
    staleTime: 30_000
  })
}

/** Polygon MANA balance in wei. */
export function useManaBalance(address: string | undefined) {
  const chainId = Number(config.get('MATIC_CHAIN_ID'))
  return useQuery({
    queryKey: privateKey(address, 'mana-balance', chainId),
    // The navbar reads it on every page, the overview included, which must not load the contract registry.
    queryFn: async () => (await import('@/data/lib/creators/mana')).fetchManaBalance(address!, chainId),
    enabled: !!address,
    staleTime: 30_000
  })
}
