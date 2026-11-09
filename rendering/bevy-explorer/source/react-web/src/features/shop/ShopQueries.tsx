// The Shop's server-state cache: one QueryClient for each mounted Shop and account, never shared between
// hosts or sessions. Another account mounts a fresh one, and the one it replaces is cleared, so no account's
// purchases, balances or favourites outlive its session. Query keys still name the account, chain or
// endpoint they read.

import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useEffect, useState, type ReactNode } from 'react'

function AccountQueries({ children }: { children: ReactNode }): React.JSX.Element {
  // The reference defaults (docs/web-app-standards.md, "QueryClient"), as /play's in-world Shop uses them.
  const [client] = useState(() => new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: 30_000 } } }))
  useEffect(() => () => client.clear(), [client])
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>
}

export function ShopQueries({ account, children }: { account: string | null | undefined; children: ReactNode }): React.JSX.Element {
  return <AccountQueries key={account?.toLowerCase() ?? ''}>{children}</AccountQueries>
}
