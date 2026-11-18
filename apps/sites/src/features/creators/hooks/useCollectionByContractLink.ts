import { useEffect } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { track } from '~/lib/analytics'
import { fetchCollections } from '~/lib/builder'
import { findCollectionByContract, isContractAddress } from '~/lib/collections'

export const CONTRACT_PARAM = 'contract'

type Outcome = 'found' | 'not_found' | 'invalid' | 'error'

/**
 * Opens the collection named by `?contract=<address>`, the link other apps (marketplace) have only a contract
 * address for. A signed-out visitor keeps the param through sign-in; any other outcome drops it and stays on the list.
 */
export function useCollectionByContractLink(address: string | undefined): { isResolving: boolean } {
  const navigate = useNavigate()
  const [searchParams, setSearchParams] = useSearchParams()
  const contract = searchParams.get(CONTRACT_PARAM)
  const isValid = !!contract && isContractAddress(contract)

  // Always refetched on mount, since a cached answer may predate the collection's deployment.
  const collections = useQuery({
    queryKey: ['collections', address, 'contract', contract?.toLowerCase()],
    queryFn: async () => (await fetchCollections(address!, { contractAddress: contract!, page: 1, limit: 1 })).results,
    enabled: !!address && isValid,
    refetchOnMount: 'always',
    retry: 1
  })
  const isSettled = !collections.isPending && !collections.isFetching

  useEffect(() => {
    if (!contract || !address || (isValid && !isSettled)) return
    // Re-checked here: a builder-server without the contract filter answers with the first collection instead.
    const match = isValid && collections.data ? findCollectionByContract(collections.data, contract) : undefined
    const outcome: Outcome = match ? 'found' : !isValid ? 'invalid' : collections.isError ? 'error' : 'not_found'
    track('Open collection by contract', { outcome, ...(match && { collectionId: match.id }) })
    if (match) {
      navigate(`/collections/${match.id}`, { replace: true })
      return
    }
    setSearchParams(
      prev => {
        const next = new URLSearchParams(prev)
        next.delete(CONTRACT_PARAM)
        return next
      },
      { replace: true }
    )
  }, [contract, address, isValid, isSettled, collections.isError, collections.data, navigate, setSearchParams])

  return { isResolving: !!address && isValid && !isSettled }
}
