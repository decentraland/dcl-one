import { useQuery } from '@tanstack/react-query'
import { fetchCollectionListings } from '~/lib/listings'

/** The primary listings of a published collection keyed by item id; off without a contract address. */
export function useCollectionListings(contractAddress: string | undefined) {
  return useQuery({
    queryKey: ['collection-listings', contractAddress],
    queryFn: () => fetchCollectionListings(contractAddress!),
    enabled: !!contractAddress,
    staleTime: 30_000
  })
}
