import { useQuery } from '@tanstack/react-query'
import { fetchBaseWearables } from '~/lib/catalyst'

/** The Catalyst base-avatars catalog the preview mannequin is dressed from; public, so it never depends on the signer. */
export function useBaseWearables() {
  return useQuery({
    queryKey: ['base-wearables'],
    queryFn: fetchBaseWearables,
    staleTime: Infinity
  })
}
