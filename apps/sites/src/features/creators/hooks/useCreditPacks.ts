import { useQuery } from '@tanstack/react-query'
import { CREDIT_PACKS, fetchCreditPacks, type CreditPack } from '~/lib/creditPacks'

/**
 * The credit-pack catalogue from credits-server. `packs` is null while it loads; a failed fetch falls
 * back to the bundled copy, so an outage never leaves the picker empty.
 */
export function useCreditPacks(): { packs: CreditPack[] | null; isLoading: boolean } {
  const { data, isLoading, isError } = useQuery({
    queryKey: ['credit-packs'],
    queryFn: fetchCreditPacks,
    staleTime: 5 * 60_000,
    retry: false
  })
  return { packs: data ?? (isError ? CREDIT_PACKS : null), isLoading }
}
