import { MutationCache, QueryCache, QueryClient } from '@tanstack/react-query'
import { HttpError } from '~/lib/http'
import { captureError } from '~/lib/monitoring'
import { isExpectedWalletError } from '~/lib/walletErrors'

/**
 * The app's QueryClient. react-query swallows every query and mutation failure (it becomes `error`
 * state, never an unhandled rejection), so without these cache-level hooks nothing that fails while
 * talking to builder-server or the chain would ever reach Sentry.
 */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: { queries: { staleTime: 30_000, refetchOnWindowFocus: false } },
    queryCache: new QueryCache({
      onError: (error, query) => {
        if (isExpectedWalletError(error)) return
        // Third-party feeds report only a server's error response: anything else is the visitor's network
        // (offline, ad blockers, CORS, timeouts).
        if (query.meta?.reportOnlyHttpErrors && !(error instanceof HttpError)) return
        captureError(error, {
          flow: 'query',
          query_key: nameOf(query.queryKey),
          ...(error instanceof HttpError ? { http_status: error.status } : {})
        })
      }
    }),
    mutationCache: new MutationCache({
      onError: (error, _variables, _context, mutation) => {
        // Dismissing the wallet prompt is a choice; a wallet on the wrong account or network is the creator's to fix.
        if (isExpectedWalletError(error)) return
        captureError(error, { flow: 'mutation', mutation_key: nameOf(mutation.options.mutationKey) })
      }
    })
  })
}

/**
 * Only the key's first segment, the query's name. The rest carries ids, addresses and free-text input
 * such as a search term, none of which belongs in a third party's error log.
 */
function nameOf(key: readonly unknown[] | undefined): string | undefined {
  const name = key?.[0]
  return typeof name === 'string' ? name : undefined
}
