import { useQueryClient } from '@tanstack/react-query'
import { useEffect } from 'react'
import { captureError } from '@/data/lib/creators/monitoring'
import { domainOf } from '@/data/lib/creators/queryKeys'
import { isExpectedWalletError } from '@/data/lib/creators/walletErrors'

// The site's own queries share the client; their failures are the site's to report.
const SITE_DOMAINS = new Set(['chrome'])

/**
 * Reports the queries and mutations that fail while a creators page is on screen. A failure only becomes
 * `error` state, never an unhandled rejection, so nothing else would see it.
 */
export function useFailureReports(): void {
  const client = useQueryClient()
  useEffect(() => {
    const stopQueries = client.getQueryCache().subscribe(event => {
      if (event.type !== 'updated' || event.action.type !== 'error') return
      const { error } = event.action
      const domain = domainOf(event.query.queryKey)
      const status = (error as { status?: unknown } | null)?.status
      if ((domain && SITE_DOMAINS.has(domain)) || isExpectedWalletError(error)) return
      captureError(error, {
        flow: 'query',
        query_key: domain,
        ...(typeof status === 'number' ? { http_status: status } : {})
      })
    })
    const stopMutations = client.getMutationCache().subscribe(event => {
      if (event.type !== 'updated' || event.action.type !== 'error') return
      const { error } = event.action
      // Dismissing the wallet prompt is a choice; a wallet on the wrong account or network is the creator's to fix.
      if (isExpectedWalletError(error)) return
      captureError(error, { flow: 'mutation', mutation_key: domainOf(event.mutation.options.mutationKey) })
    })
    return () => {
      stopQueries()
      stopMutations()
    }
  }, [client])
}
