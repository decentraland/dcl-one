import { useQuery } from '@tanstack/react-query'
import { fetchHotScenes } from '~/lib/hotScenes'

/** The realm provider's hot-scenes feed; per-visit freshness is enough for a marketing rail. */
export function useHotScenes() {
  return useQuery({
    queryKey: ['hot-scenes'],
    queryFn: fetchHotScenes,
    staleTime: 5 * 60_000,
    retry: 1,
    meta: { reportOnlyHttpErrors: true }
  })
}
