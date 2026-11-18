import { useCallback } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { cacheKey, deleteCached, type ValidationIssue, type ValidationResult } from '~/lib/validation'

/**
 * Checks an item again from scratch: drops the persisted results of the given validation queries and refetches
 * them, resolving with their combined fresh issues. Every screen observing those queries updates with it.
 */
export function useRerunValidation() {
  const queryClient = useQueryClient()
  return useCallback(
    async (queryKeys: (readonly unknown[])[]): Promise<ValidationIssue[]> => {
      await deleteCached(queryKeys.map(cacheKey))
      await Promise.all(queryKeys.map(queryKey => queryClient.refetchQueries({ queryKey, exact: true })))
      return queryKeys.flatMap(queryKey => queryClient.getQueryData<ValidationResult>(queryKey)?.issues ?? [])
    },
    [queryClient]
  )
}
